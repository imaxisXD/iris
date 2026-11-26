// The model on Windows: llama.cpp's own server (llama-server, prebuilt) as a child process,
// speaking to the same rewriter the Mac's MLX engine does (`TextGenerator`). No MLX here, and
// nothing only the Mac has: Foundation, a process and HTTP on 127.0.0.1.
//
// Measured before it was written (gpui/windows/PLAN.md):
// - one cache per slot, never --kv-unified: with it, one slot's work evicts another's
//   sliding-window cache and no tone ever reuses its prompt;
// - llama-server keeps up to 8 GB of old prompts in RAM by default: capped at 512 MB;
// - a tone's fixed prompt, filled once, is saved to disk and restored in milliseconds, so a
//   tone is slow once per install, never once per launch. Only warm-ups are saved (their only
//   words are the prompt's): a person's draft never goes to disk.
// - FoundationNetworking's URLSession on Windows polled libcurl at ~50% CPU while streaming
//   SSE; loopback requests to llama-server go over direct blocking sockets (LoopbackSocket.swift).

import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import IrisCore

// MARK: - The models

/// A model Iris offers on Windows: the same two trained Gemmas as on the Mac, as GGUF files.
public enum LlamaModel: String, CaseIterable, Sendable {
    case best
    case lighter

    var repo: String {
        switch self {
        case .best: "ImAXISOG/iris-gemma-4-e4b-it-gguf"
        case .lighter: "ImAXISOG/iris-gemma-4-e2b-it-gguf"
        }
    }

    var fileName: String {
        switch self {
        case .best: "iris-gemma-4-e4b-it-Q4_K_M.gguf"
        case .lighter: "iris-gemma-4-e2b-it-Q4_K_M.gguf"
        }
    }

    /// The file's size: a download that ends at another size is thrown away.
    public var downloadBytes: Int64 {
        switch self {
        case .best: 4_575_609_696
        case .lighter: 2_810_565_568
        }
    }

    /// Memory in use while checking, measured on Windows (the server's working set): Lighter
    /// 2.0 to 2.9 GB, Best 5.5 GB.
    public var modelBytes: Int64 {
        switch self {
        case .best: 5_500_000_000
        case .lighter: 2_900_000_000
        }
    }

    /// The file's SHA-256, as Hugging Face lists it: a download that doesn't match is thrown away.
    var sha256: String {
        switch self {
        case .best: "467320c9e7dc9fd5a2935a90ac4cd433d90d6f3b6b057044de9b303afee0c54d"
        case .lighter: "729a89ccf950f17cdf85daef79cd8d474ccad52ac049ef37da51e5b21ee93bdd"
        }
    }

    public var downloadSize: String { String(format: "%.1f GB", Double(downloadBytes) / 1_000_000_000) }

    var url: URL { URL(string: "https://huggingface.co/\(repo)/resolve/main/\(fileName)")! }

    public var file: URL { LlamaEngine.modelsDirectory.appendingPathComponent(fileName) }

    public var isDownloaded: Bool {
        let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?.int64Value
        return size == downloadBytes
    }

    /// Without a graphics card measured fast enough, Lighter: Best on a CPU came within 4 s of
    /// its time limit on a 3,700-character draft, on a fast Mac.
    public static var recommended: LlamaModel { .lighter }

    public static var memoryGB: Int { Int((Double(ProcessInfo.processInfo.physicalMemory) / 1_073_741_824).rounded()) }

    /// The model to start on: one already on this PC, else the recommended one.
    public static func starting(onDisk: (LlamaModel) -> Bool) -> LlamaModel {
        if onDisk(recommended) { return recommended }
        return allCases.first(where: onDisk) ?? recommended
    }
}

public enum LlamaState: Equatable, Sendable {
    case idle
    case downloading(Double)
    case loading
    case ready(LlamaModel)
    case failed(ModelFailure)
}

public enum LlamaError: Error, Equatable {
    case notDownloaded
    case noServer(String)
    case serverDidNotStart(String)
    case badDownload
}

// MARK: - The engine

public actor LlamaEngine {
    public static let shared = LlamaEngine()

    /// Where the models live: IRIS_LLAMA_MODELS, else %LOCALAPPDATA%\Iris\models on Windows,
    /// ~/.cache/iris-gguf on a Mac (only a development brain uses this engine there).
    public static var modelsDirectory: URL {
        let env = ProcessInfo.processInfo.environment
        if let dir = env["IRIS_LLAMA_MODELS"] { return URL(fileURLWithPath: dir) }
        #if os(Windows)
        let base = env["LOCALAPPDATA"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        return base.appendingPathComponent("Iris").appendingPathComponent("models")
        #else
        return URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".cache/iris-gguf")
        #endif
    }

    /// llama-server: IRIS_LLAMA_SERVER_BIN, else beside the brain (llama/llama-server.exe as the
    /// installer lays it out), else Homebrew's on a Mac.
    static var serverBinary: URL? {
        let env = ProcessInfo.processInfo.environment
        if let path = env["IRIS_LLAMA_SERVER_BIN"] { return URL(fileURLWithPath: path) }
        let here = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        #if os(Windows)
        let candidates = [here.appendingPathComponent("llama").appendingPathComponent("llama-server.exe"),
                          here.appendingPathComponent("llama-server.exe")]
        #else
        let candidates = [here.appendingPathComponent("llama/llama-server"), URL(fileURLWithPath: "/opt/homebrew/bin/llama-server")]
        #endif
        return candidates.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    private(set) var state: LlamaState = .idle
    private var preferred: LlamaModel = LlamaModel.starting(onDisk: \.isDownloaded)
    private var observers: [@Sendable (LlamaState) -> Void] = []
    private var server: Server?
    private var loading: Task<Void, Error>?
    /// Bumped by every load that starts its own task: a cancelled load ends on idle only when no
    /// newer one has started (one that already failed has set its own state).
    private var loads = 0
    private var idleMinutes = 30
    private var lightMode = LlamaBudget.recommendedLightMode(memoryGB: LlamaModel.memoryGB)
    private var lastUse = Date()
    private var idleTimer: Task<Void, Never>?
    let slots = SlotMap()

    public var preferredModel: LlamaModel { preferred }

    private var currentBudget: LlamaBudget {
        LlamaBudget.forMachine(memoryGB: LlamaModel.memoryGB, lightMode: lightMode)
    }

    public func setPreferred(_ model: LlamaModel) async {
        guard model != preferred else { return }
        preferred = model
        // A model still downloading or starting is let go of too, so the new one starts its own.
        if server != nil || loading != nil { await unload() }
    }

    public func setLightMode(_ on: Bool) async {
        guard on != lightMode else { return }
        lightMode = on
        // If a running server was started with a different KV / cache-ram budget, unload it so
        // the leaner (or roomier) configuration takes effect cleanly on the next load.
        if let server, server.budget != currentBudget {
            await unload()
        }
    }

    public func setIdleUnload(minutes: Int) { idleMinutes = max(1, minutes) }

    public func observe(_ f: @escaping @Sendable (LlamaState) -> Void) { observers.append(f) }

    private func set(_ s: LlamaState) {
        state = s
        for o in observers { o(s) }
    }

    /// Gets the model on disk (when allowed) and the server running with it.
    public func load(_ model: LlamaModel, allowDownload: Bool) async throws {
        if model != preferred { await setPreferred(model) }
        if case .ready(let m) = state, m == model, server != nil { return }
        if let loading { return try await loading.value }
        loads += 1
        let task = Task { try await self.bringUp(model, allowDownload: allowDownload) }
        loading = task
        // `unload` clears `loading` itself, and a newer load may have set its own by now.
        defer { if loading == task { loading = nil } }
        try await task.value
    }

    private func bringUp(_ model: LlamaModel, allowDownload: Bool) async throws {
        // The load this is: `load` sets it before this task gets the actor.
        let mine = loading
        let seq = loads
        do {
            if !model.isDownloaded {
                guard allowDownload else { throw LlamaError.notDownloaded }
                set(.downloading(0))
                try await Download.fetch(model) { [weak self] f in Task { await self?.downloaded(f, by: mine) } }
            }
            // A download that finished as it was cancelled doesn't say "loading" after `unload`.
            try Task.checkCancellation()
            set(.loading)
            let started = try await Server.start(model: model, budget: currentBudget)
            if Task.isCancelled {
                started.stop()
                throw CancellationError()
            }
            server = started
            await slots.reset(for: started)
            // Unloaded meanwhile: `unload` stopped this server; it isn't ready.
            try Task.checkCancellation()
            set(.ready(model))
            touch()
        } catch {
            // Cancelled by `unload` (a switch, Delete, quitting, idle): it stopped the server.
            // A newer load owns the state and the server: touch neither. Nothing newer: end idle,
            // over anything this load said after `unload` did.
            if Task.isCancelled || error is CancellationError {
                if loads == seq && loading == nil && server == nil { set(.idle) }
                throw error
            }
            server?.stop()
            server = nil
            set(.failed(ModelFailure(llama: error)))
            throw error
        }
    }

    /// A download's progress, only while it's still the load in hand: a cancelled one's last
    /// callbacks arrive after `unload` said idle, or while a newer load is showing its own.
    private func downloaded(_ fraction: Double, by load: Task<Void, Error>?) {
        guard let load, loading == load else { return }
        set(.downloading(fraction))
    }

    public func unload() async {
        let inFlight = loading
        loading?.cancel()
        loading = nil
        server?.stop()
        server = nil
        idleTimer?.cancel()
        set(.idle)
        // Wait for a cancelled download or start to let go: curl still writes ".part" until it
        // exits (on Windows holding it open, so Delete can't remove it), and a starting server is
        // reaped by its own task. Then a Delete, a new download or quitting follows safely.
        _ = try? await inFlight?.value
    }

    /// Lets go of a model's file, any partial download, and its saved slot caches (Settings' Delete).
    public func delete(_ model: LlamaModel) async {
        // `unload` waits for a cancelled download to let go of its file.
        if preferred == model { await unload() }
        let fm = FileManager.default
        try? fm.removeItem(at: model.file)
        try? fm.removeItem(at: model.file.appendingPathExtension("part"))
        let slotsRoot = Self.modelsDirectory.appendingPathComponent("slots")
        if let dirs = try? fm.contentsOfDirectory(at: slotsRoot, includingPropertiesForKeys: nil) {
            let prefix = "\(model.rawValue)-"
            for dir in dirs where dir.lastPathComponent.hasPrefix(prefix) {
                try? fm.removeItem(at: dir)
            }
        }
    }

    /// Ready to answer: the model loaded if it's on disk (never downloaded here).
    func ready() async throws -> Server {
        if let server, case .ready = state {
            if server.process.isRunning { touch(); return server }
            // The server went (ended by hand, or it crashed): start another.
            self.server = nil
            set(.idle)
        }
        try await load(preferred, allowDownload: false)
        guard let server else { throw LlamaError.notDownloaded }
        touch()
        return server
    }

    private func touch() {
        lastUse = Date()
        idleTimer?.cancel()
        let minutes = idleMinutes
        idleTimer = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(minutes) * 60 * 1_000_000_000)
            guard !Task.isCancelled else { return }
            await self?.unloadIfIdle()
        }
    }

    private func unloadIfIdle() async {
        // Never under a check still answering (a slow one on the CPU can outlast a short wait).
        guard await slots.isQuiet() else { return touch() }
        if Date().timeIntervalSince(lastUse) >= Double(idleMinutes * 60) - 1 { await unload() }
    }

    /// Stops the server when the brain goes (the app's Job Object stops it too).
    public nonisolated func shutdown() {
        Task { await self.unload() }
    }
}

// MARK: - The generator

/// The rewriter's model, through llama-server.
public struct LlamaGenerator: TextGenerator {
    public init() {}

    public func ensureReady() async throws { _ = try await LlamaEngine.shared.ready() }

    public func prewarm(_ turns: [PromptTurn], cacheKey: String) async {
        guard let server = try? await LlamaEngine.shared.ready() else { return }
        let (slot, restored) = await LlamaEngine.shared.slots.slot(for: cacheKey, server: server)
        if restored { return }
        await Self.fillAndSave(turns, cacheKey: cacheKey, slot: slot, server: server)
    }

    /// Fills a slot with a prompt whose last turn is "x" (the rewriter's own warm-up) and
    /// keeps it on disk: the prompt alone, never a person's words.
    static func fillAndSave(_ turns: [PromptTurn], cacheKey: String, slot: Int, server: Server) async {
        let stream = server.chat(turns, temperature: 0, maxTokens: 1, slot: slot)
        do { for try await _ in stream {} } catch { return }
        await LlamaEngine.shared.slots.save(cacheKey, slot: slot, server: server)
    }

    /// A tone used before its prompt was saved (only the tone the panel opens on is warmed up
    /// ahead): once things are quiet, its prompt is filled again with "x" for the draft and
    /// saved, so the next launch starts warm. Once per tone, per install.
    static func saveLater(_ turns: [PromptTurn], cacheKey: String, server: Server) {
        guard var last = turns.last else { return }
        last = PromptTurn(last.role, "x")
        let promptOnly = Array(turns.dropLast()) + [last]
        Task.detached(priority: .background) {
            guard await LlamaEngine.shared.slots.claimSave(cacheKey) else { return }
            for _ in 0..<40 {
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                if !server.process.isRunning { return }
                if await LlamaEngine.shared.slots.isQuiet() { break }
            }
            guard server.process.isRunning else { return }
            let (slot, _) = await LlamaEngine.shared.slots.slot(for: cacheKey, server: server)
            await fillAndSave(promptOnly, cacheKey: cacheKey, slot: slot, server: server)
        }
    }

    public func stream(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let server = try await LlamaEngine.shared.ready()
                    var slot = -1
                    if let cacheKey { slot = await LlamaEngine.shared.slots.slot(for: cacheKey, server: server).0 }
                    await LlamaEngine.shared.slots.begin()
                    defer { Task { await LlamaEngine.shared.slots.end() } }
                    for try await piece in server.chat(turns, temperature: temperature, maxTokens: maxTokens, slot: slot) {
                        continuation.yield(piece)
                    }
                    continuation.finish()
                    if let cacheKey, await !LlamaEngine.shared.slots.isSaved(cacheKey) {
                        Self.saveLater(turns, cacheKey: cacheKey, server: server)
                    }
                } catch { continuation.finish(throwing: error) }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

// MARK: - Slots

/// Which server slot holds which fixed prompt (the rewriter's cacheKey), least recently used
/// first, as the Mac's prefix cache keeps four. A slot given a prompt saved before is restored
/// from disk instead of filled.
actor SlotMap {
    private(set) var count = 4
    private var order: [String] = []
    private var slotOf: [String: Int] = [:]
    private var saved: Set<String> = []
    private var directory: URL?

    private let maxSavedFiles = 20

    func reset(for server: Server) async {
        count = server.budget.slots
        order = []
        slotOf = [:]
        claimed.removeAll()
        directory = server.slotDirectory
        let names = (try? FileManager.default.contentsOfDirectory(atPath: server.slotDirectory.path)) ?? []
        saved = Set(names.filter { $0.hasSuffix(".bin") })
        pruneSaved()
    }

    private func pruneSaved() {
        guard saved.count > maxSavedFiles, let directory else { return }
        let fm = FileManager.default
        let files = saved.compactMap { name -> (String, URL, Date)? in
            let url = directory.appendingPathComponent(name)
            let modified = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? .distantPast
            return (name, url, modified)
        }.sorted { $0.2 > $1.2 }
        for (name, url, _) in files.dropFirst(maxSavedFiles) {
            try? fm.removeItem(at: url)
            saved.remove(name)
        }
    }

    static func fileName(_ key: String) -> String {
        String(key.map { $0.isLetter || $0.isNumber ? $0 : "-" }) + ".bin"
    }

    /// The slot for `key`, and whether it was just restored from disk.
    func slot(for key: String, server: Server) async -> (Int, Bool) {
        if let s = slotOf[key] {
            order.removeAll { $0 == key }
            order.append(key)
            return (s, false)
        }
        let used = Set(slotOf.values)
        var s = (0..<count).first { !used.contains($0) }
        if s == nil, let oldest = order.first {
            s = slotOf[oldest]
            slotOf[oldest] = nil
            order.removeFirst()
        }
        let slot = s ?? 0
        slotOf[key] = slot
        order.append(key)
        let file = Self.fileName(key)
        if saved.contains(file), await server.slotAction("restore", slot: slot, file: file) {
            return (slot, true)
        }
        return (slot, false)
    }

    func save(_ key: String, slot: Int, server: Server) async {
        let file = Self.fileName(key)
        if await server.slotAction("save", slot: slot, file: file) {
            saved.insert(file)
            pruneSaved()
        }
    }

    func isSaved(_ key: String) -> Bool { saved.contains(Self.fileName(key)) }

    // One save per prompt, and none while a check is running.
    private var claimed: Set<String> = []
    private var running = 0
    private var lastEnd = Date()

    func claimSave(_ key: String) -> Bool { claimed.insert(key).inserted }
    func begin() { running += 1 }
    func end() { running -= 1; lastEnd = Date() }
    func isQuiet() -> Bool { running == 0 && Date().timeIntervalSince(lastEnd) > 2 }
}

// MARK: - The server

/// One running llama-server: its process, port and key.
final class Server: @unchecked Sendable {
    let process: Process
    let port: Int
    let key: String
    let slotDirectory: URL
    let budget: LlamaBudget

    private init(process: Process, port: Int, key: String, slotDirectory: URL, budget: LlamaBudget) {
        self.process = process
        self.port = port
        self.key = key
        self.slotDirectory = slotDirectory
        self.budget = budget
    }

    static func start(model: LlamaModel, budget: LlamaBudget) async throws -> Server {
        guard let binary = LlamaEngine.serverBinary else { throw LlamaError.noServer("llama-server not found") }
        let key = (0..<4).map { _ in String(UInt64.random(in: 0...UInt64.max), radix: 36) }.joined()
        // Saved prompts belong to this model file and this llama.cpp: an update starts afresh.
        let build = (try? FileManager.default.attributesOfItem(atPath: binary.path)[.size] as? NSNumber)?.stringValue ?? "0"
        let slotsRoot = LlamaEngine.modelsDirectory.appendingPathComponent("slots")
        let dirName = "\(model.rawValue)-\(model.downloadBytes)-\(build)"
        let slotDirectory = slotsRoot.appendingPathComponent(dirName)
        let fm = FileManager.default
        try? fm.createDirectory(at: slotDirectory, withIntermediateDirectories: true)
        // Prune stale slot directories left by older llama-server builds or older model revisions.
        if let siblings = try? fm.contentsOfDirectory(at: slotsRoot, includingPropertiesForKeys: nil) {
            let prefix = "\(model.rawValue)-"
            for dir in siblings where dir.lastPathComponent.hasPrefix(prefix) && dir.lastPathComponent != dirName {
                try? fm.removeItem(at: dir)
            }
        }
        var lastError = "no port"
        for _ in 0..<5 {
            try Task.checkCancellation()
            let port = Int.random(in: 49_152...65_000)
            let p = Process()
            p.executableURL = binary
            var args = ["-m", model.file.path, "--host", "127.0.0.1", "--port", String(port), "--api-key", key,
                        "-np", String(budget.slots), "-c", String(budget.totalContext), "--cache-ram", String(budget.cacheRamMB),
                        "--slot-save-path", slotDirectory.path, "--no-webui"]
            let env = ProcessInfo.processInfo.environment
            // CPU unless told otherwise (a graphics card only after a self-test proves it: later).
            args += ["-ngl", env["IRIS_LLAMA_GPU_LAYERS"] ?? "0"]
            #if os(macOS)
            if env["IRIS_LLAMA_GPU_LAYERS"] == nil { args += ["-dev", "none"] }
            #endif
            // Below 12 GB (or in light mode), weights aren't repacked in RAM: ~0.9 GB less memory.
            if budget.noRepack { args.append("--no-repack") }
            p.arguments = args
            p.standardOutput = FileHandle.nullDevice
            p.standardError = FileHandle.nullDevice
            try p.run()
            let server = Server(process: p, port: port, key: key, slotDirectory: slotDirectory, budget: budget)
            // Loading a model file takes about a second from disk; give it up to two minutes (by
            // the clock: a check that gets no answer takes its own two seconds). Terminate and reap
            // `p` if startup is cancelled so no multi-GB `llama-server` process is leaked.
            do {
                let deadline = Date().addingTimeInterval(120)
                while Date() < deadline {
                    try Task.checkCancellation()
                    if !p.isRunning { break }
                    if await server.healthy() {
                        if Task.isCancelled { throw CancellationError() }
                        return server
                    }
                    try await Task.sleep(nanoseconds: 250_000_000)
                }
            } catch {
                Self.end(p)
                throw error
            }
            lastError = p.isRunning ? "never became ready" : "exited (\(p.terminationStatus))"
            Self.end(p)
        }
        throw LlamaError.serverDidNotStart(lastError)
    }

    func stop() {
        Self.end(process)
    }

    /// Asks the server to quit and reaps it; one that hasn't gone in 3 s is killed, so whoever
    /// is waiting (the engine's actor) never waits for ever. Windows' terminate already kills.
    static func end(_ p: Process) {
        if p.isRunning { p.terminate() }
        let deadline = Date().addingTimeInterval(3)
        while p.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        #if !os(Windows)
        if p.isRunning { kill(p.processIdentifier, SIGKILL) }
        #endif
        p.waitUntilExit()
    }

    func healthy() async -> Bool {
        await LoopbackClient.status(port: port, method: "GET", path: "/health", key: key, within: 2) == 200
    }

    func slotAction(_ action: String, slot: Int, file: String) async -> Bool {
        let body = try? JSONSerialization.data(withJSONObject: ["filename": file])
        return await LoopbackClient.status(
            port: port,
            method: "POST",
            path: "/slots/\(slot)?action=\(action)",
            key: key,
            body: body,
            within: 60
        ) == 200
    }

    /// Streams the answer's text, piece by piece, from llama-server's server-sent events.
    func chat(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, slot: Int) -> AsyncThrowingStream<String, Error> {
        var body: [String: Any] = [
            "messages": turns.map { ["role": $0.role.rawValue, "content": $0.content] },
            "stream": true, "temperature": temperature, "top_p": temperature == 0 ? 1.0 : 0.95,
            "max_tokens": maxTokens, "cache_prompt": true,
            "chat_template_kwargs": ["enable_thinking": false],
        ]
        if slot >= 0 { body["id_slot"] = slot }
        let payload = (try? JSONSerialization.data(withJSONObject: body)) ?? Data("{}".utf8)
        return LoopbackClient.stream(port: port, path: "/v1/chat/completions", key: key, body: payload)
    }
}

// MARK: - The download

/// A model file from Hugging Face, with progress, into the models folder; a file that ends at
/// another size is thrown away.
final class Download: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    private let model: LlamaModel
    private let progress: @Sendable (Double) -> Void
    private var continuation: CheckedContinuation<Void, Error>?
    private var moved = false
    private var lastReport = 0.0

    private init(_ model: LlamaModel, _ progress: @escaping @Sendable (Double) -> Void) {
        self.model = model
        self.progress = progress
    }

    static func fetch(_ model: LlamaModel, progress: @escaping @Sendable (Double) -> Void) async throws {
        try FileManager.default.createDirectory(at: LlamaEngine.modelsDirectory, withIntermediateDirectories: true)
        if let free = try? LlamaEngine.modelsDirectory.resourceValues(forKeys: [.volumeAvailableCapacityKey]).volumeAvailableCapacity,
           Int64(free) < model.downloadBytes + 500_000_000 {
            let gb = { (b: Int64) in String(format: "%.1f GB", Double(b) / 1_000_000_000) }
            throw ModelDownloadRoom(needed: gb(model.downloadBytes), free: gb(Int64(free)))
        }
        if let curl = curlBinary {
            try await withCurl(curl, model: model, progress: progress)
            return
        }
        let d = Download(model, progress)
        let session = URLSession(configuration: .default, delegate: d, delegateQueue: nil)
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
                d.continuation = c
                session.downloadTask(with: model.url).resume()
            }
        } onCancel: { session.invalidateAndCancel() }
        guard model.isDownloaded else {
            try? FileManager.default.removeItem(at: model.file)
            throw LlamaError.badDownload
        }
        try checkHash(model)
    }

    /// Windows' own curl.exe (in every Windows 10 since 1803), or the Mac's. Swift's networking
    /// on Windows had Hugging Face's CDN reset the connection in a Windows 11 VM where curl.exe
    /// fetched the same file; curl also picks up where a broken download stopped.
    static var curlBinary: URL? {
        #if os(Windows)
        let root = ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows"
        let path = URL(fileURLWithPath: root).appendingPathComponent("System32").appendingPathComponent("curl.exe")
        #else
        let path = URL(fileURLWithPath: "/usr/bin/curl")
        #endif
        return FileManager.default.fileExists(atPath: path.path) ? path : nil
    }

    /// The file through curl into "<name>.part" (kept, so another try resumes), progress read from
    /// its size, then renamed once it's whole.
    static func withCurl(_ curl: URL, model: LlamaModel, progress: @escaping @Sendable (Double) -> Void) async throws {
        let part = model.file.appendingPathExtension("part")
        let p = Process()
        p.executableURL = curl
        p.arguments = ["-L", "--fail", "--silent", "--show-error", "--retry", "5", "--retry-delay", "2",
                       "-C", "-", "-o", part.path, model.url.absoluteString]
        // curl's error line goes to a file read once it has exited: nothing to drain while it
        // runs (a pipe left unread can fill and stall it on Windows) and no reader racing a close.
        let fm = FileManager.default
        let errorsURL = fm.temporaryDirectory.appendingPathComponent("iris-curl-\(UUID().uuidString).txt")
        fm.createFile(atPath: errorsURL.path, contents: nil)
        let errors = try FileHandle(forWritingTo: errorsURL)
        var errorsOpen = true
        defer {
            if errorsOpen { try? errors.close() }
            try? fm.removeItem(at: errorsURL)
        }
        p.standardError = errors
        p.standardOutput = FileHandle.nullDevice
        try p.run()
        do {
            try await withTaskCancellationHandler {
                var last = -1.0
                while p.isRunning {
                    try await Task.sleep(nanoseconds: 300_000_000)
                    let have = (try? FileManager.default.attributesOfItem(atPath: part.path)[.size] as? NSNumber)?.int64Value ?? 0
                    let f = min(1, Double(have) / Double(model.downloadBytes))
                    if f - last >= 0.005 { last = f; progress(f) }
                }
            } onCancel: { p.terminate() }
        } catch {
            if p.isRunning { p.terminate() }
            p.waitUntilExit()
            throw error
        }
        p.waitUntilExit()
        // Closed before reading: Windows may not share a file still open for writing.
        try? errors.close()
        errorsOpen = false
        let said = String(decoding: ((try? Data(contentsOf: errorsURL)) ?? Data()).prefix(4096), as: UTF8.self)
        guard p.terminationStatus == 0 else {
            // curl's own codes for no connection: 6 (host), 7 (connect), 28 (timed out), 35 (TLS), 56 (receive).
            if [6, 7, 28, 35, 56].contains(p.terminationStatus) {
                throw URLError(.notConnectedToInternet)
            }
            throw LlamaError.noServer("download: curl \(p.terminationStatus) \(said.prefix(200))")
        }
        try? FileManager.default.removeItem(at: model.file)
        try FileManager.default.moveItem(at: part, to: model.file)
        guard model.isDownloaded else {
            try? FileManager.default.removeItem(at: model.file)
            throw LlamaError.badDownload
        }
        try checkHash(model)
    }

    /// The whole file's SHA-256 against the one Hugging Face lists, with the system's own tool
    /// (certutil on Windows, shasum on a Mac): a file that doesn't match is thrown away.
    static func checkHash(_ model: LlamaModel) throws {
        let p = Process()
        #if os(Windows)
        let root = ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows"
        p.executableURL = URL(fileURLWithPath: root).appendingPathComponent("System32").appendingPathComponent("certutil.exe")
        p.arguments = ["-hashfile", model.file.path, "SHA256"]
        #else
        p.executableURL = URL(fileURLWithPath: "/usr/bin/shasum")
        p.arguments = ["-a", "256", model.file.path]
        #endif
        let out = Pipe()
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        try p.run()
        let text = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self).lowercased()
        p.waitUntilExit()
        guard text.replacingOccurrences(of: " ", with: "").contains(model.sha256) else {
            try? FileManager.default.removeItem(at: model.file)
            throw LlamaError.badDownload
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData _: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        let f = Double(totalBytesWritten) / Double(model.downloadBytes)
        if f - lastReport >= 0.005 { lastReport = f; progress(min(f, 1)) }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        try? FileManager.default.removeItem(at: model.file)
        moved = (try? FileManager.default.moveItem(at: location, to: model.file)) != nil
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        session.finishTasksAndInvalidate()
        if let error { continuation?.resume(throwing: error) }
        else if !moved { continuation?.resume(throwing: LlamaError.badDownload) }
        else { continuation?.resume() }
        continuation = nil
    }
}

/// Not enough room on disk for the model (`ModelFailure.noRoom`).
public struct ModelDownloadRoom: Error {
    public let needed: String
    public let free: String
}

// MARK: - What failed, in the reader's terms

extension ModelFailure {
    /// Getting the model failed, sorted by what the reader can do (the MLX engine's
    /// `ModelFailure(_:)`, for this engine's errors).
    public init(llama error: Error) {
        switch error {
        case let room as ModelDownloadRoom: self = .noRoom(needed: room.needed, free: room.free)
        case LlamaError.notDownloaded: self = .notDownloaded
        case _ where ModelFailure.isOffline(error): self = .offline
        default: self = .other
        }
    }
}

extension ProblemNote {
    /// What a failed check says: the check's own error, getting the model, or anything else.
    public init(llama error: Error, draft: String) {
        if let rewrite = error as? RewriteError {
            self.init(rewrite, draft: draft)
        } else if error is LlamaError || error is ModelDownloadRoom || ModelFailure.isOffline(error) {
            self.init(ModelFailure(llama: error))
        } else {
            self = .unknown
        }
    }
}
