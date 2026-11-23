// The model behind the brain, whichever runs it: Gemma on MLX on the Mac (`IrisEngine`), Gemma
// through llama.cpp's server on Windows (`IrisLlama`). main.swift speaks only to `BrainEngine`,
// shaped like the JSON it sends the app; each adapter keeps its engine's own types to itself.
//
// The Mac's shipped brain always runs MLX. A debug build can run llama.cpp instead
// (IRIS_ENGINE=llama), to try the Windows engine on a Mac; a release build can't.

import Foundation
import IrisCore
#if canImport(IrisEngine)
import IrisEngine
#endif
#if canImport(IrisLlama)
import IrisLlama
#endif
#if canImport(IrisSpelling)
import IrisSpelling
#endif

/// A model as `hello` reports it.
struct ModelInfo {
    let id: String
    let downloadSize: String
    let downloadBytes: Int64
    let modelBytes: Int64
    let downloaded: Bool
}

enum BrainState {
    case idle
    case downloading(Double)
    case loading
    case ready(String)
    case failed(ModelFailure)
}

protocol BrainEngine: Sendable {
    var generator: any TextGenerator { get }
    func promptStyle() async -> PromptStyle
    /// The model chosen in Settings (or the one Iris starts on).
    func chosen() async -> String
    /// Chooses the model with this id; false (and nothing happens) for an id this engine
    /// doesn't know.
    func choose(_ id: String) async -> Bool
    /// No model chosen yet: the one the app would start on.
    func chooseStarting(setUpBefore: Bool) async
    func setIdleUnload(minutes: Int) async
    func load(_ id: String?, allowDownload: Bool) async throws
    func isNotDownloaded(_ error: Error) -> Bool
    func unload() async
    /// Lets go of a model and its files (Settings' Delete).
    func delete(_ id: String) async
    func models() -> [ModelInfo]
    var recommended: String { get }
    func observe(_ f: @escaping @Sendable (BrainState) -> Void) async
    func problem(_ error: Error, draft: String) -> ProblemNote
    /// Whether the engine has a process of its own to stop before the brain exits.
    var stopsBeforeExit: Bool { get }
    /// Whether a draft is mostly another language, and the spelling hints for it.
    func looksForeign(_ text: String) async -> Bool
    func spellingHints(_ text: String) async -> [SpellingHint]
}

/// The engine this brain runs.
let engine: any BrainEngine = {
    #if canImport(IrisEngine)
    #if DEBUG && canImport(IrisLlama)
    if ProcessInfo.processInfo.environment["IRIS_ENGINE"] == "llama" { return LlamaBrainEngine() }
    #endif
    return MLXBrainEngine()
    #else
    return LlamaBrainEngine()
    #endif
}()

// MARK: - The Mac: Gemma on MLX

#if canImport(IrisEngine)
struct MLXBrainEngine: BrainEngine {
    var generator: any TextGenerator { Engine.shared }

    func promptStyle() async -> PromptStyle { await Engine.shared.preferredChoice.promptStyle }

    func chosen() async -> String { await Engine.shared.preferredChoice.rawValue }

    func choose(_ id: String) async -> Bool {
        guard let choice = ModelChoice(rawValue: id) else { return false }
        await Engine.shared.setPreferred(choice)
        return true
    }

    func chooseStarting(setUpBefore: Bool) async {
        await Engine.shared.setPreferred(ModelChoice.starting(setUpBefore: setUpBefore, onDisk: \.isDownloaded, memoryGB: ModelChoice.macMemoryGB))
    }

    func setIdleUnload(minutes: Int) async { await Engine.shared.setIdleUnload(minutes: minutes) }

    func load(_ id: String?, allowDownload: Bool) async throws {
        let preferred = await Engine.shared.preferredChoice
        let choice = id.flatMap(ModelChoice.init(rawValue:)) ?? preferred
        try await Engine.shared.load(choice, allowDownload: allowDownload)
    }

    func isNotDownloaded(_ error: Error) -> Bool {
        if case EngineError.notDownloaded = error { return true }
        return false
    }

    func unload() async { await Engine.shared.unload() }

    // `ModelDialogs.delete`: the chosen model is let go of first, then its folders (and
    // its drafter's, unless another model on disk still needs it) are removed.
    func delete(_ id: String) async {
        guard let choice = ModelChoice(rawValue: id) else { return }
        if await Engine.shared.preferredChoice == choice { await Engine.shared.unload() }
        for folder in choice.cacheDirectories {
            do { try FileManager.default.removeItem(at: folder) } catch CocoaError.fileNoSuchFile {
            } catch { log("delete: couldn't remove \(folder.path): \(error)") }
        }
    }

    func models() -> [ModelInfo] {
        ModelChoice.offered.map {
            ModelInfo(id: $0.rawValue, downloadSize: $0.downloadSize, downloadBytes: $0.downloadBytes,
                      modelBytes: $0.modelBytes, downloaded: $0.isDownloaded)
        }
    }

    var recommended: String { ModelChoice.recommended.rawValue }

    func observe(_ f: @escaping @Sendable (BrainState) -> Void) async {
        await Engine.shared.observe { state in
            switch state {
            case .idle: f(.idle)
            case .downloading(let x): f(.downloading(x))
            case .loading: f(.loading)
            case .ready(let m): f(.ready(m.rawValue))
            case .failed(let failure): f(.failed(failure))
            }
        }
    }

    func problem(_ error: Error, draft: String) -> ProblemNote { ProblemNote(error, draft: draft) }

    var stopsBeforeExit: Bool { false }

    func looksForeign(_ text: String) async -> Bool { await LanguageCheck.looksNonEnglish(text) }

    func spellingHints(_ text: String) async -> [SpellingHint] { await SpellCheck.hints(for: text) }
}
#endif

// MARK: - Windows: Gemma through llama.cpp's server

#if canImport(IrisLlama)
struct LlamaBrainEngine: BrainEngine {
    var generator: any TextGenerator { LlamaGenerator() }

    // The same trained models as the Mac's Best and Lighter.
    func promptStyle() async -> PromptStyle { .trained }

    func chosen() async -> String { await LlamaEngine.shared.preferredModel.rawValue }

    func choose(_ id: String) async -> Bool {
        guard let m = LlamaModel(rawValue: id) else { return false }
        await LlamaEngine.shared.setPreferred(m)
        return true
    }

    func chooseStarting(setUpBefore: Bool) async {
        await LlamaEngine.shared.setPreferred(LlamaModel.starting(onDisk: \.isDownloaded))
    }

    func setIdleUnload(minutes: Int) async { await LlamaEngine.shared.setIdleUnload(minutes: minutes) }

    func load(_ id: String?, allowDownload: Bool) async throws {
        let preferred = await LlamaEngine.shared.preferredModel
        try await LlamaEngine.shared.load(id.flatMap(LlamaModel.init(rawValue:)) ?? preferred, allowDownload: allowDownload)
    }

    func isNotDownloaded(_ error: Error) -> Bool { (error as? LlamaError) == .notDownloaded }

    func unload() async { await LlamaEngine.shared.unload() }

    func delete(_ id: String) async {
        if let m = LlamaModel(rawValue: id) { await LlamaEngine.shared.delete(m) }
    }

    func models() -> [ModelInfo] {
        LlamaModel.allCases.map {
            ModelInfo(id: $0.rawValue, downloadSize: $0.downloadSize, downloadBytes: $0.downloadBytes,
                      modelBytes: $0.modelBytes, downloaded: $0.isDownloaded)
        }
    }

    var recommended: String { LlamaModel.recommended.rawValue }

    func observe(_ f: @escaping @Sendable (BrainState) -> Void) async {
        await LlamaEngine.shared.observe { state in
            switch state {
            case .idle: f(.idle)
            case .downloading(let x): f(.downloading(x))
            case .loading: f(.loading)
            case .ready(let m): f(.ready(m.rawValue))
            case .failed(let failure): f(.failed(failure))
            }
        }
    }

    func problem(_ error: Error, draft: String) -> ProblemNote { ProblemNote(llama: error, draft: draft) }

    // llama-server is a process of its own: stopped here, and by the app's Job Object on Windows.
    var stopsBeforeExit: Bool { true }

    // Windows' own spelling and language checks come with the app's request (later); until then
    // a draft is taken as English and Fix gets no hints.
    func looksForeign(_ text: String) async -> Bool {
        #if canImport(IrisSpelling)
        await LanguageCheck.looksNonEnglish(text)
        #else
        false
        #endif
    }

    func spellingHints(_ text: String) async -> [SpellingHint] {
        #if canImport(IrisSpelling)
        await SpellCheck.hints(for: text)
        #else
        []
        #endif
    }
}
#endif
