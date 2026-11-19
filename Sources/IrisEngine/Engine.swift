import Foundation
import IrisCore
import HuggingFace
import MLX
import MLXHuggingFace
import MLXLLM
import MLXLMCommon
import MLXVLM
import Tokenizers
import os

/// The models Iris can use. `best` and `lighter` are Iris's own: Gemma 4 trained on writing
/// lessons, published on Hugging Face. `e4b` and `e2b` are Google's untrained Gemma 4, which
/// Iris used before; people who downloaded one keep it until they switch, and new Macs are
/// never offered them (`offered`).
public enum ModelChoice: String, CaseIterable, Codable, Sendable, Identifiable {
    case best
    case lighter
    case e4b
    case e2b

    public var id: String { rawValue }

    /// Iris's own trained models, asked with the short prompts they learned with.
    public var isTrained: Bool { self == .best || self == .lighter }

    /// How the model is asked (`Prompts`): trained models already know the jobs.
    public var promptStyle: PromptStyle { isTrained ? .trained : .taught }

    /// The weights, and how to read their chat template. Iris's own are pinned to the
    /// commit that was tested, so a later upload can't change them under anyone.
    public var configuration: ModelConfiguration {
        switch self {
        case .best: ModelConfiguration(id: "ImAXISOG/iris-gemma-4-e4b-it-mlx",
                                       revision: "d7b746bea65153a385e18cd9a0f5549a2cf7e0bf", extraEOSTokens: ["<turn|>"])
        case .lighter: ModelConfiguration(id: "ImAXISOG/iris-gemma-4-e2b-it-mlx",
                                          revision: "b0a6390530ea4580ef7607967d1134e1b2ada58e", extraEOSTokens: ["<turn|>"])
        case .e4b: LLMRegistry.gemma4_e4b_it_4bit
        // Google's quantization-aware build: in our side-by-side tests it beat the plain
        // 4-bit E2B on Fix (76% vs 72% acceptable) and tones (76% vs 68% sendable).
        case .e2b: ModelConfiguration(id: "mlx-community/gemma-4-E2B-it-qat-4bit", extraEOSTokens: ["<turn|>"])
        }
    }

    /// Google's MTP drafter trained for these weights (see `Engine.drafter`). The trained
    /// models share the drafter of the Gemma they were trained from.
    var drafter: ModelConfiguration {
        switch self {
        case .best, .e4b: ModelConfiguration(id: "mlx-community/gemma-4-E4B-it-assistant-bf16")
        case .lighter, .e2b: ModelConfiguration(id: "mlx-community/gemma-4-E2B-it-qat-assistant-bf16")
        }
    }

    /// Bytes of the model itself, for the free-space check (the drafter follows in the
    /// background once the model is ready).
    public var modelBytes: Int64 {
        switch self {
        case .best: 4_580_000_000
        case .lighter: 3_770_000_000
        case .e4b: 5_180_000_000
        case .e2b: 4_360_000_000
        }
    }
    private static let drafterBytes: Int64 = 200_000_000

    /// Everything it puts on disk, the model and its drafter: what the download button
    /// promises, what progress counts towards and what Delete frees.
    public var downloadBytes: Int64 { modelBytes + Self.drafterBytes }

    /// `downloadBytes` as people read it: ("5.4", "GB").
    public var downloadFigure: (value: String, unit: String) {
        (String(format: "%.1f", Double(downloadBytes) / 1e9), "GB")
    }
    /// "5.4 GB".
    public var downloadSize: String { "\(downloadFigure.value) \(downloadFigure.unit)" }

    /// The best model wants a Mac with at least this much memory, alongside everything else.
    public static let roomyMemoryGB = 16

    /// This Mac's memory in whole gigabytes, as Apple sells it (8, 16, 24...).
    public static var macMemoryGB: Int { Int((Double(ProcessInfo.processInfo.physicalMemory) / 1_073_741_824).rounded()) }

    /// The model that suits a Mac with this much memory.
    public static func recommended(memoryGB: Int) -> ModelChoice { memoryGB >= roomyMemoryGB ? .best : .lighter }

    /// The model that suits this Mac.
    public static var recommended: ModelChoice { recommended(memoryGB: macMemoryGB) }

    /// What Settings and setup list: Iris's own models, and an earlier model only while it's
    /// on this Mac (someone who used Iris before), so it can still be used or deleted.
    public static func offered(onDisk: (ModelChoice) -> Bool) -> [ModelChoice] {
        allCases.filter { $0.isTrained || onDisk($0) }
    }
    public static var offered: [ModelChoice] { offered(onDisk: \.isDownloaded) }

    /// The model to use when none was ever chosen. Someone who set Iris up before this
    /// release has Google's model on disk and keeps it (the size Iris picked for them then)
    /// until they choose another; everyone else starts on the one that suits this Mac.
    public static func starting(setUpBefore: Bool, onDisk: (ModelChoice) -> Bool, memoryGB: Int) -> ModelChoice {
        if setUpBefore {
            let earlier: ModelChoice = memoryGB >= 24 ? .e4b : .e2b
            if onDisk(earlier) { return earlier }
            if let other = [ModelChoice.e4b, .e2b].first(where: onDisk) { return other }
        }
        return recommended(memoryGB: memoryGB)
    }

    /// Where the Hugging Face client keeps these weights.
    public var cacheDirectory: URL { Self.cacheDirectory(for: configuration.name) }

    /// Every folder deleting this model frees: the model, and its drafter unless another
    /// model on this Mac still needs it (an earlier model and Iris's own share one).
    public func cacheDirectories(keepingFor onDisk: (ModelChoice) -> Bool) -> [URL] {
        let drafterShared = Self.allCases.contains { $0 != self && $0.drafter.name == drafter.name && onDisk($0) }
        return drafterShared ? [cacheDirectory] : [cacheDirectory, Self.cacheDirectory(for: drafter.name)]
    }
    public var cacheDirectories: [URL] { cacheDirectories(keepingFor: \.isDownloaded) }

    private static func cacheDirectory(for repoID: String) -> URL {
        HubCache.default.cacheDirectory.appendingPathComponent("models--" + repoID.replacingOccurrences(of: "/", with: "--"))
    }

    /// True once every weight file is on disk. A download that stopped part way doesn't
    /// count (see `ModelFiles`).
    public var isDownloaded: Bool { ModelFiles.complete(in: cacheDirectory) }

    /// Where Iris's own models keep their weights once downloaded: the pinned commit's
    /// `model.safetensors`. Loaded text-only from it (`Gemma4TextOnly`): the per-layer word
    /// table is read from this file row by row and the picture encoder left out, about 1.6 to
    /// 1.9 GB less memory with the same answers. Nil for Google's models, loaded as they were.
    var wordTableFile: URL? {
        guard isTrained, case .id(_, let revision) = configuration.id else { return nil }
        return cacheDirectory.appendingPathComponent("snapshots/\(revision)/model.safetensors")
    }

    /// MLX's buffer cache while this model is loaded. Iris's own models were measured with
    /// 128 MB: the same answers, 0.2 to 0.4 GB less memory. Google's keep the 512 MB they
    /// always had.
    var cacheLimitBytes: Int { (isTrained ? 128 : 512) * 1024 * 1024 }
}

public enum EngineState: Equatable, Sendable {
    case idle
    case downloading(Double)
    case loading
    case ready(ModelChoice)
    /// Getting the model failed, and why, in the reader's terms.
    case failed(ModelFailure)

    /// Why the last load failed, while the state says it did.
    public var failure: ModelFailure? {
        if case .failed(let failure) = self { return failure }
        return nil
    }

    /// A model is in memory, whichever one.
    public var isReady: Bool {
        if case .ready = self { return true }
        return false
    }
}

public enum EngineError: Error, LocalizedError, Equatable {
    case notEnoughDisk(needed: String, free: String)
    /// Asked to load a model that isn't on this Mac by something that mustn't download it
    /// (a check, a prewarm). Only setup and Settings download, and only when asked to.
    case notDownloaded

    /// The same sentence the screens show for it.
    public var errorDescription: String? { ModelFailure(self).sentence }
}

extension ModelFailure {
    /// What went wrong getting the model, sorted by what the reader can do about it.
    public init(_ error: Error) {
        switch error {
        case EngineError.notEnoughDisk(let needed, let free): self = .noRoom(needed: needed, free: free)
        case EngineError.notDownloaded: self = .notDownloaded
        case _ where ModelFailure.isOffline(error): self = .offline
        default: self = .other
        }
    }
}

extension ProblemNote {
    /// What a failed check says, wherever it says it: a check's own error, getting the model,
    /// or anything else, never in the system's words.
    public init(_ error: Error, draft: String) {
        if let rewrite = error as? RewriteError {
            self.init(rewrite, draft: draft)
        } else if error is EngineError || ModelFailure.isOffline(error) {
            self.init(ModelFailure(error))
        } else {
            self = .unknown
        }
    }
}

/// Timing for one generation, for the eval tool and logs.
public struct GenerationStats: Sendable, Equatable {
    public var promptTokens = 0
    /// Prompt tokens served from the prefix cache instead of being prefilled again.
    public var reusedTokens = 0
    public var generatedTokens = 0
    public var prefillMs = 0
    public var firstTokenMs = 0
    public var totalMs = 0
    public var prefixHit: Bool { reusedTokens > 0 }
    public init() {}
}

/// Holds the single model in memory and streams generations.
///
/// Lifecycle (modeled on what FluidVoice does, plus the pieces it leaves out):
/// - **Load once, share it.** Concurrent callers await the same load task. Each load is
///   stamped with a generation ID so a late result from a replaced load is ignored.
/// - **Prewarm early.** The app calls `prewarm` the moment the shortcut is pressed, before
///   text capture finishes, so loading and the fixed-prompt prefill overlap with capture.
/// - **Reuse the fixed prompt.** Each mode's system prompt and examples are prefilled once
///   into a KV cache; requests copy it and only prefill the user's text.
/// - **Unload when idle, never mid-request.** An activity counter holds the model while any
///   request runs; the idle timer starts only when the count reaches zero and re-checks
///   before unloading.
/// - **Give memory back under pressure.** A memory-pressure warning drops the prompt caches;
///   a critical event unloads the model if nothing is running.
public actor Engine: TextGenerator {
    public static let shared = Engine()
    private static let log = Logger(subsystem: "local.iris.app", category: "engine")

    // Residency
    /// What's in memory: nothing, a load under way (stamped so a late result from a replaced
    /// load is ignored), or a loaded model. One value, so a model can't be both.
    private enum Residency {
        case empty
        case loading(ModelChoice, Task<ModelContainer, Error>, id: UUID)
        case loaded(ModelChoice, ModelContainer)
    }
    private var residency = Residency.empty

    private var isLoaded: Bool {
        if case .loaded = residency { return true }
        return false
    }

    /// Whether the load stamped `id` is still the one under way.
    private func isCurrentLoad(_ id: UUID) -> Bool {
        if case .loading(_, _, let current) = residency { return current == id }
        return false
    }

    // Activity and idle unload
    private var activeRequests = 0
    private var lastActivity = Date()
    private var idleTask: Task<Void, Never>?
    public var idleUnloadAfter: Duration = .seconds(30 * 60)

    // Generation
    private let gate = AsyncGate()
    let prefixes = PrefixStore(capacity: 4)
    /// How the last request went, for local tools that measure the pipeline.
    private(set) var lastStats: GenerationStats?

    // State fan-out
    private var stateObservers: [UUID: @Sendable (EngineState) -> Void] = [:]
    public private(set) var state: EngineState = .idle {
        didSet { for observer in stateObservers.values { observer(state) } }
    }

    /// Off makes every request prefill its whole prompt. Always on in the app.
    var prefixCacheEnabled = true

    /// Google's MTP drafter for the loaded Gemma, once it's ready: a ~190 MB model that
    /// proposes a few tokens at a time, which Gemma checks in one pass. The answers are the
    /// same; they're written about twice as fast. Nil until loaded, or if it couldn't load
    /// (first use offline), and then requests decode the usual way.
    private var drafter: (any MTPDrafterModel)?
    /// The drafter load under way, stamped with the model load it belongs to.
    var drafterTask: Task<Void, Never>?
    private var drafterLoad: UUID?
    /// Switch for speculative decoding with the drafter. Off loads Gemma as before.
    var speculativeDecoding = true

    /// The drafter trained for these weights: a shipped model's, or none for others.
    static func drafterConfiguration(for modelID: String) -> ModelConfiguration? {
        ModelChoice.allCases.first { $0.configuration.name == modelID }?.drafter
    }

    /// The model the app wants to use. Set from settings; requests load it on demand.
    public var preferredChoice: ModelChoice = .recommended

    /// Other weights to load in place of the shipped ones, for local tools that compare
    /// models through the same pipeline. Always nil in the app.
    var configurationOverride: ModelConfiguration?

    private let pressureSource: DispatchSourceMemoryPressure

    public init() {
        pressureSource = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .global(qos: .utility))
        pressureSource.setEventHandler { [weak self, source = pressureSource] in
            let event = source.data
            Task { await self?.handleMemoryPressure(event) }
        }
        pressureSource.activate()
    }

    // MARK: State

    @discardableResult
    public func observe(_ observer: @escaping @Sendable (EngineState) -> Void) -> UUID {
        let id = UUID()
        stateObservers[id] = observer
        observer(state)
        return id
    }

    public func setIdleUnload(minutes: Int) {
        idleUnloadAfter = .seconds(max(1, minutes) * 60)
        scheduleIdleUnload()
    }

    public func setPreferred(_ choice: ModelChoice) { preferredChoice = choice }

    // MARK: Load and unload

    /// Loads the model, downloading it first only when `allowDownload` (setup's Download,
    /// Settings' Load). Checks and prewarms never download: a model that isn't on disk
    /// throws `EngineError.notDownloaded` and the state stays as it was.
    /// Safe to call repeatedly and concurrently.
    @discardableResult
    public func load(_ choice: ModelChoice, allowDownload: Bool = false) async throws -> ModelContainer {
        switch residency {
        case .loaded(let loaded, let container) where loaded == choice: return container
        case .loading(let loading, let task, _) where loading == choice: return try await task.value
        default: break
        }

        if configurationOverride == nil, !allowDownload, !choice.isDownloaded { throw EngineError.notDownloaded }
        if configurationOverride == nil {
            do { try checkDiskSpace(for: choice) } catch {
                state = .failed(ModelFailure(error))
                throw error
            }
        }
        unload()
        // Keep MLX's buffer cache modest; the weights themselves are not affected.
        // Set here, not in init, so creating the engine never touches the GPU.
        MLX.Memory.cacheLimit = choice.cacheLimitBytes

        let id = UUID()
        state = configurationOverride != nil || choice.isDownloaded ? .loading : .downloading(0)
        let configuration = configurationOverride ?? choice.configuration
        // The drafter only works with Gemma loaded through MLXVLM, whose Gemma 4 hands the
        // drafter its hidden state and K/V; other models load as plain language models.
        let drafterConfiguration = speculativeDecoding ? Self.drafterConfiguration(for: configuration.name) : nil
        let wordTableFile = configurationOverride == nil ? choice.wordTableFile : nil
        let task = Task<ModelContainer, Error> {
            let progressHandler: @Sendable (Progress) -> Void = { progress in
                let fraction = progress.fractionCompleted
                Task { await self.reportDownload(fraction, load: id) }
            }
            if drafterConfiguration != nil {
                // Read when the model is built, after the download: Iris's own load text-only.
                Gemma4TextOnly.wordTableFile = wordTableFile
                defer { Gemma4TextOnly.wordTableFile = nil }
                return try await VLMModelFactory.shared.loadContainer(
                    from: #hubDownloader(), using: #huggingFaceTokenizerLoader(),
                    configuration: configuration, progressHandler: progressHandler)
            }
            // Labelled on purpose: the macro only reads a `progressHandler:` argument and
            // silently drops a trailing closure, which left the download stuck at 0%.
            return try await #huggingFaceLoadModelContainer(
                configuration: configuration, progressHandler: progressHandler)
        }
        residency = .loading(choice, task, id: id)
        do {
            let loaded = try await task.value
            guard isCurrentLoad(id) else { throw CancellationError() }  // replaced meanwhile
            residency = .loaded(choice, loaded)
            state = .ready(choice)
            lastActivity = Date()
            scheduleIdleUnload()
            if let drafterConfiguration { loadDrafter(drafterConfiguration, load: id) }
            Self.log.info("loaded \(choice.rawValue, privacy: .public)")
            return loaded
        } catch {
            if isCurrentLoad(id) {
                residency = .empty
                if error is CancellationError {
                    state = .idle
                } else {
                    Self.log.error("load failed: \(error.localizedDescription, privacy: .public)")
                    state = .failed(ModelFailure(error))
                }
            }
            throw error
        }
    }

    /// Fetches (first time) and loads the drafter after Gemma is ready, so it never delays
    /// the first request; requests use it from the moment it's in.
    private func loadDrafter(_ configuration: ModelConfiguration, load id: UUID) {
        drafterLoad = id
        drafterTask = Task {
            await Gemma4AssistantRegistration.register()
            do {
                let context = try await MTPDrafterModelFactory.shared.load(
                    from: #hubDownloader(), using: #huggingFaceTokenizerLoader(), configuration: configuration)
                guard drafterLoad == id else { return }  // the model was unloaded or replaced
                drafter = context.model
                Self.log.info("drafter ready: \(configuration.name, privacy: .public)")
            } catch {
                Self.log.error("drafter unavailable, decoding normally: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private func reportDownload(_ fraction: Double, load id: UUID) {
        guard isCurrentLoad(id) else { return }
        // The last stretch is reading weights into memory, shown as "loading".
        state = fraction >= 0.999 ? .loading : .downloading(fraction)
    }

    private func checkDiskSpace(for choice: ModelChoice) throws {
        guard !choice.isDownloaded else { return }
        let home = FileManager.default.homeDirectoryForCurrentUser
        let free = (try? home.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey]))?
            .volumeAvailableCapacityForImportantUsage ?? Int64.max
        let needed = choice.downloadBytes + 1_000_000_000
        if free < needed {
            let format = { (bytes: Int64) in ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file) }
            throw EngineError.notEnoughDisk(needed: format(needed), free: format(free))
        }
    }

    /// Frees the model. Waits for nothing: callers that need it simply load again.
    public func unload() {
        idleTask?.cancel()
        idleTask = nil
        // A load still in flight is cancelled, and no longer current when it returns.
        if case .loading(_, let task, _) = residency { task.cancel() }
        let hadModel = isLoaded
        residency = .empty
        drafterTask?.cancel()
        drafterTask = nil
        drafterLoad = nil
        drafter = nil
        prefixes.clear()
        if hadModel { MLX.Memory.clearCache() }
        state = .idle
    }

    /// Makes sure the preferred model is loaded; one that isn't on disk throws `notDownloaded`.
    /// Not part of any time limit.
    public func ensureReady() async throws {
        beginActivity()
        defer { endActivity() }
        _ = try await load(preferredChoice)
    }

    // MARK: Prewarm

    /// Loads the model and prefills the fixed part of a prompt, so the next request with
    /// the same `cacheKey` only has to read the user's text. Cheap to call repeatedly.
    public func prewarm(_ turns: [PromptTurn], cacheKey: String) async {
        beginActivity()
        defer { endActivity() }
        do {
            let container = try await load(preferredChoice)
            await gate.acquire()
            defer { gate.release() }
            let store = prefixes
            try await container.perform { context in
                _ = try await Self.prefixEntry(for: Self.messages(turns), key: cacheKey, store: store, context: context)
            }
        } catch {
            Self.log.debug("prewarm skipped: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: Generate

    public nonisolated func stream(
        _ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?
    ) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    try await self.run(turns, temperature: temperature, maxTokens: maxTokens, cacheKey: cacheKey) {
                        continuation.yield($0)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    private func run(
        _ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?,
        onChunk: @escaping @Sendable (String) -> Void
    ) async throws {
        beginActivity()
        defer { endActivity() }

        let container = try await load(preferredChoice)
        // One generation on the GPU at a time; a superseded request is cancelled by its
        // caller and gives up its turn here.
        await gate.acquire()
        defer { gate.release() }
        try Task.checkCancellation()

        let store = prefixes
        let cacheKey = prefixCacheEnabled ? cacheKey : nil
        let parameters = GenerateParameters(
            maxTokens: maxTokens, temperature: temperature, topP: temperature == 0 ? 1.0 : 0.95)

        nonisolated(unsafe) let drafter = self.drafter  // used only inside the serialized perform below
        let stats = try await container.perform { context in
            let messages = Self.messages(turns)
            let started = Date()
            var stats = GenerationStats()
            let full = try await context.processor.prepare(
                input: Self.input(messages))
            let fullTokens = full.text.tokens.asArray(Int32.self).map(Int.init)
            stats.promptTokens = fullTokens.count

            var input = full
            var cache: [KVCache]? = nil
            if let cacheKey,
                let entry = try await Self.prefixEntry(for: messages, key: cacheKey, store: store, context: context),
                fullTokens.count > entry.tokens.count, fullTokens.starts(with: entry.tokens)
            {
                cache = entry.cache.map { $0.copy() }
                input = LMInput(tokens: MLXArray(fullTokens[entry.tokens.count...].map(Int32.init)))
                stats.reusedTokens = entry.tokens.count
            }

            let prefillStart = Date()
            var first = true
            var decoder = ScalarStreamDecoder(tokenizer: context.tokenizer)
            let tokens = try drafter.map {
                try generateTokens(input: input, cache: cache, parameters: parameters, context: context,
                                   mtpDrafter: $0, blockSize: 4)
            } ?? generateTokens(input: input, cache: cache, parameters: parameters, context: context)
            for await generation in tokens {
                if Task.isCancelled { break }
                switch generation {
                case .token(let token):
                    if first {
                        stats.firstTokenMs = Int(Date().timeIntervalSince(started) * 1000)
                        stats.prefillMs = Int(Date().timeIntervalSince(prefillStart) * 1000)
                        first = false
                    }
                    if let text = decoder.append(token) { onChunk(text) }
                case .info(let info):
                    stats.generatedTokens = info.generationTokenCount
                default:
                    break
                }
            }
            stats.totalMs = Int(Date().timeIntervalSince(started) * 1000)
            return stats
        }
        try Task.checkCancellation()
        lastStats = stats
        Self.log.info(
            "gen prompt=\(stats.promptTokens) reused=\(stats.reusedTokens) first=\(stats.firstTokenMs)ms total=\(stats.totalMs)ms")
    }

    /// Finds or builds the cached prefill for the fixed part of these messages.
    ///
    /// The fixed part is found without knowing the chat template: render the same turns
    /// with two different final user messages and keep the tokens they share. Everything
    /// up to the start of the user's text is identical across requests, so it's cached.
    private static func prefixEntry(
        for messages: [Chat.Message], key: String, store: PrefixStore, context: ModelContext
    ) async throws -> PrefixStore.Entry? {
        if let entry = store[key] { return entry }
        guard messages.count >= 2 else { return nil }

        func tokens(lastUser text: String) async throws -> [Int] {
            var probe = messages
            probe[probe.count - 1] = .user(text)
            let prepared = try await context.processor.prepare(
                input: Self.input(probe))
            return prepared.text.tokens.asArray(Int32.self).map(Int.init)
        }
        let a = try await tokens(lastUser: "alpha")
        let b = try await tokens(lastUser: "omega")
        let shared = zip(a, b).prefix { $0 == $1 }.count
        guard shared >= 16 else { return nil }
        let prefix = Array(a.prefix(shared))

        let cache = try context.model.newCache(parameters: nil)
        _ = try TokenIterator(
            input: LMInput(tokens: MLXArray(prefix.map(Int32.init))), model: context.model, cache: cache,
            parameters: GenerateParameters(maxTokens: 1, temperature: 0))
        eval(cache)
        let entry = PrefixStore.Entry(tokens: prefix, cache: cache)
        store[key] = entry
        log.info("prefilled prefix \(key, privacy: .public): \(prefix.count) tokens")
        return entry
    }

    private static func messages(_ turns: [PromptTurn]) -> [Chat.Message] {
        turns.map { turn in
            switch turn.role {
            case .system: .system(turn.content)
            case .user: .user(turn.content)
            case .assistant: .assistant(turn.content)
            }
        }
    }

    /// A chat as the model reads it. The prefix probe and the real request must build it the
    /// same way, or the prefilled cache never matches.
    private static func input(_ chat: [Chat.Message]) -> UserInput {
        UserInput(chat: chat, additionalContext: ["enable_thinking": false])
    }

    // MARK: Activity and idle unload

    private func beginActivity() {
        activeRequests += 1
        lastActivity = Date()
        idleTask?.cancel()
        idleTask = nil
    }

    private func endActivity() {
        activeRequests = max(0, activeRequests - 1)
        lastActivity = Date()
        if activeRequests == 0 { scheduleIdleUnload() }
    }

    private func scheduleIdleUnload() {
        idleTask?.cancel()
        guard isLoaded, activeRequests == 0 else { return }
        let delay = idleUnloadAfter
        idleTask = Task { [weak self] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            await self?.unloadIfStillIdle()
        }
    }

    private func unloadIfStillIdle() {
        let idleFor = Date().timeIntervalSince(lastActivity)
        let limit = Double(idleUnloadAfter.components.seconds)
        guard activeRequests == 0, idleFor >= limit - 1 else { return scheduleIdleUnload() }
        Self.log.info("unloading after \(Int(idleFor))s idle")
        unload()
    }

    private func handleMemoryPressure(_ event: DispatchSource.MemoryPressureEvent) {
        guard activeRequests == 0 else { return }
        if event.contains(.critical), isLoaded {
            Self.log.info("critical memory pressure: unloading")
            unload()
        } else if event.contains(.warning) {
            prefixes.clear()
            MLX.Memory.clearCache()
        }
    }
}

/// Turns streamed token IDs into text deltas.
///
/// The library's NaiveStreamingDetokenizer measures what's new in Characters. In scripts
/// like Devanagari a vowel sign joins the consonant before it into one Character, so a
/// token that adds only the sign looks like no change and the sign is dropped
/// ("मीटिंग" came out as "मटग"). Comparing Unicode scalars keeps every one.
struct ScalarStreamDecoder {
    let tokenizer: any MLXLMCommon.Tokenizer
    private var tokens: [Int] = []
    private var emitted = 0  // Unicode scalars already handed out

    init(tokenizer: any MLXLMCommon.Tokenizer) { self.tokenizer = tokenizer }

    mutating func append(_ token: Int) -> String? {
        tokens.append(token)
        let scalars = Array(tokenizer.decode(tokenIds: tokens).unicodeScalars)
        // A token that ends partway through a multi-byte character decodes to U+FFFD; wait for the rest.
        guard scalars.last != "\u{FFFD}", scalars.count > emitted else { return nil }
        var text = String.UnicodeScalarView()
        text.append(contentsOf: scalars[emitted...])
        emitted = scalars.count
        return String(text)
    }
}

/// Prefilled KV caches for the fixed part of each mode's prompt. Read and filled inside
/// `ModelContainer.perform` while the engine's gate is held; `clear()` is also called from
/// the engine itself (unload, memory pressure), outside the gate.
final class PrefixStore: @unchecked Sendable {
    struct Entry {
        let tokens: [Int]
        let cache: [KVCache]
    }

    private let capacity: Int
    private var entries: [String: Entry] = [:]
    private var order: [String] = []
    private let lock = NSLock()

    init(capacity: Int) { self.capacity = capacity }

    subscript(key: String) -> Entry? {
        get {
            lock.withLock {
                guard let entry = entries[key] else { return nil }
                order.removeAll { $0 == key }
                order.append(key)
                return entry
            }
        }
        set {
            lock.withLock {
                entries[key] = newValue
                order.removeAll { $0 == key }
                if newValue != nil { order.append(key) }
                while order.count > capacity { entries[order.removeFirst()] = nil }
            }
        }
    }

    func clear() {
        lock.withLock {
            entries.removeAll()
            order.removeAll()
        }
    }
}

/// A one-at-a-time async lock, so two generations never share the GPU.
final class AsyncGate: @unchecked Sendable {
    private let lock = NSLock()
    private var busy = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func acquire() async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            lock.lock()
            if busy {
                waiters.append(continuation)
                lock.unlock()
            } else {
                busy = true
                lock.unlock()
                continuation.resume()
            }
        }
    }

    func release() {
        lock.lock()
        if waiters.isEmpty {
            busy = false
            lock.unlock()
        } else {
            let next = waiters.removeFirst()
            lock.unlock()
            next.resume()
        }
    }
}
