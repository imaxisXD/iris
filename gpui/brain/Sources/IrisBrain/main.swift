// The brain: the Swift app's engine and rewriter, for the GPUI app. One JSON object a line on
// stdin (a request), one a line on stdout (an answer or news). Nothing on stdout but these;
// logs go to stderr.
//
// Requests (`op`):
//   hello                      → {event:"hello", models:[…], recommended, chosen}
//   configure {model, idleMinutes, onboarded} → hello again (the chosen model may change)
//   load {model?, allowDownload} (news comes as `state` events)
//   unload
//   delete {model}             → hello (frees the model's folders on disk, as Settings' Delete does)
//   check {id, text, mode:{id,label,instruction}, retry, glossary, styleRules}
//                              → {event:"partial", id, text} … then finished | failed
//                                (finished's result carries `keep`: words to watch after a Replace)
//   cancel {id}
//   prewarm {mode, glossary, styleRules}
//   tones {customTones, toneOrder, hiddenTones, defaultMode, appModes, bundleID}
//                              → {event:"tones", shown:[modes], starting}
// News: {event:"state", state:"idle"|"downloading"|"loading"|"ready"|"failed", fraction?, model?,
//        failure?:{sentence, canRetry, kind}}, each followed by hello (a download puts a model
//        on disk).

import Foundation
import IrisCore
#if os(Windows)
import ucrt
#endif

// MARK: - Out

/// Writes one line of JSON to the original stdout pipe, one writer at a time.
/// Duplicates fd 1 and redirects fd 1 → stderr so stray `print()` calls from any library
/// can never corrupt the JSON-lines IPC stream.
final class Out: @unchecked Sendable {
    static let shared = Out()
    private let lock = NSLock()
    private let pipe: FileHandle

    private init() {
        #if os(Windows)
        // The C runtime's names; it has no STDOUT_FILENO (stdout is 1, stderr 2).
        let dupFd = _dup(1)
        if dupFd >= 0 {
            _ = _dup2(2, 1)
            pipe = FileHandle(fileDescriptor: dupFd, closeOnDealloc: true)
        } else {
            pipe = FileHandle.standardOutput
        }
        #else
        let dupFd = dup(STDOUT_FILENO)
        if dupFd >= 0 {
            _ = dup2(STDERR_FILENO, STDOUT_FILENO)
            pipe = FileHandle(fileDescriptor: dupFd, closeOnDealloc: true)
        } else {
            pipe = FileHandle.standardOutput
        }
        #endif
    }

    func send(_ object: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) else { return }
        lock.lock()
        pipe.write(data)
        pipe.write(Data([0x0A]))
        lock.unlock()
    }
}

func log(_ s: String) {
    FileHandle.standardError.write(("brain: " + s + "\n").data(using: .utf8)!)
}
// The engine (`Engines.swift`): MLX on the Mac, llama.cpp on Windows.

// MARK: - Encoding

func encode(_ state: BrainState) -> [String: Any] {
    switch state {
    case .idle: return ["event": "state", "state": "idle"]
    case .downloading(let f): return ["event": "state", "state": "downloading", "fraction": f]
    case .loading: return ["event": "state", "state": "loading"]
    case .ready(let m): return ["event": "state", "state": "ready", "model": m]
    case .failed(let failure):
        return ["event": "state", "state": "failed", "failure": encode(failure)]
    }
}

func encode(_ failure: ModelFailure) -> [String: Any] {
    let kind: String
    var extra: [String: Any] = [:]
    switch failure {
    case .noRoom(let needed, let free): kind = "noRoom"; extra = ["needed": needed, "free": free]
    case .offline: kind = "offline"
    case .notDownloaded: kind = "notDownloaded"
    case .other: kind = "other"
    }
    return ["kind": kind, "sentence": failure.sentence, "canRetry": failure.canRetry].merging(extra) { a, _ in a }
}

func encode(_ note: ProblemNote) -> [String: Any] {
    ["note": note.note, "lead": note.lead, "detail": note.detail, "canRetry": note.canRetry, "circled": note.circled]
}

func encode(_ segment: WordDiff.Segment) -> [String] {
    switch segment {
    case .same(let s): return ["same", s]
    case .removed(let s): return ["removed", s]
    case .added(let s): return ["added", s]
    }
}

func encode(_ r: RewriteResult) -> [String: Any] {
    var out: [String: Any] = [
        "mode": r.mode.id, "original": r.original, "text": r.text,
        "segments": r.segments.map(encode), "translated": r.translated,
        "latencyMs": r.latencyMs, "changedWords": r.changedWords,
    ]
    if let v = r.verdict {
        switch v {
        case .unchanged: out["verdict"] = "unchanged"
        case .minor: out["verdict"] = "minor"
        case .heavy: out["verdict"] = "heavy"
        }
    }
    return out
}

func encode(_ mode: Mode) -> [String: Any] {
    ["id": mode.id, "label": mode.label, "instruction": mode.instruction]
}

func mode(_ any: Any?) -> Mode? {
    guard let d = any as? [String: Any], let id = d["id"] as? String else { return nil }
    if id == Mode.fix.id { return .fix }
    return Mode(id: id, label: d["label"] as? String ?? id, instruction: d["instruction"] as? String ?? "")
}

func decode<T: Decodable>(_ type: T.Type, _ any: Any?) -> T? {
    guard let any, !(any is NSNull), let data = try? JSONSerialization.data(withJSONObject: any, options: [.fragmentsAllowed]) else { return nil }
    return try? JSONDecoder().decode(T.self, from: data)
}

// MARK: - The brain

actor Brain {
    private var checks: [Int: Task<Void, Never>] = [:]

    /// The rewriter the app's `CheckSetup` builds: the engine, the writer's words and rules,
    /// the language and spelling checks, and the prompt style of the model in use.
    func rewriter(_ req: [String: Any]) async -> Rewriter {
        let style = await engine.promptStyle()
        // Windows' app sends its own language check and unknown words (it has the dictionaries;
        // the brain there doesn't). The Mac's app sends neither: the brain asks the Mac's own.
        let foreign = req["looksForeign"] as? Bool
        let misspelled = (req["misspelled"] as? [[String: Any]]).map { flagged($0, in: req["text"] as? String ?? "") }
        return Rewriter(generator: engine.generator,
                        glossary: req["glossary"] as? [String] ?? [],
                        styleRules: req["styleRules"] as? [String] ?? [],
                        isForeign: { text in
                            if let foreign { return foreign }
                            return await engine.looksForeign(text)
                        },
                        spellingHints: { text in
                            if let misspelled { return misspelled.filter { text.contains($0.word) } }
                            return await engine.spellingHints(text)
                        },
                        promptStyle: style)
    }

    func handle(_ req: [String: Any]) async {
        let op = req["op"] as? String ?? ""
        switch op {
        case "hello":
            await sendModels()

        case "configure":
            // The model chosen in Settings, or (none chosen yet) the one the app would start
            // on: a Google model already on this Mac for someone set up before, else the one
            // that suits this Mac (`AppSettings.startingModel`).
            if let raw = req["model"] as? String, await engine.choose(raw) {
            } else {
                let setUpBefore = req["onboarded"] as? Bool ?? false
                await engine.chooseStarting(setUpBefore: setUpBefore)
            }
            if let minutes = req["idleMinutes"] as? Int {
                await engine.setIdleUnload(minutes: minutes)
            }
            await sendModels()

        case "load":
            let id = req["model"] as? String
            let allow = req["allowDownload"] as? Bool ?? false
            Task {
                do { try await engine.load(id, allowDownload: allow) } catch {
                    log("load: \(error)")
                    if engine.isNotDownloaded(error) {
                        Out.shared.send(["event": "state", "state": "failed", "failure": encode(ModelFailure.notDownloaded)])
                    }
                }
            }

        case "unload":
            await engine.unload()

        case "delete":
            guard let raw = req["model"] as? String else { return }
            await engine.delete(raw)
            await sendModels()

        case "check":
            guard let id = req["id"] as? Int, let text = req["text"] as? String, let m = mode(req["mode"]) else { return }
            let retry = req["retry"] as? Bool ?? false
            let rw = await rewriter(req)
            checks[id]?.cancel()
            checks[id] = Task {
                do {
                    for try await event in rw.run(text, mode: m, retry: retry) {
                        if Task.isCancelled { return }
                        switch event {
                        case .partial(let p): Out.shared.send(["event": "partial", "id": id, "text": p])
                        case .finished(let r):
                            // The words a writer might put back (names, spellings they meant), for
                            // `LearnWords` to watch for after a Replace.
                            var result = encode(r)
                            let glossary = req["glossary"] as? [String] ?? []
                            result["keep"] = WordWatch.candidates(FixNotes.changes(r.segments), ignoring: glossary)
                                .map { ["from": $0.from, "to": $0.to] }
                            Out.shared.send(["event": "finished", "id": id, "result": result])
                        }
                    }
                } catch {
                    // Cancelled by the app (`cancel`, or the same id asked again): it isn't
                    // waiting. Anything else, the engine stopping it too (the model was switched
                    // or unloaded mid-check), is said, so the panel never waits for ever.
                    if Task.isCancelled { return }
                    log("check \(id): \(error)")
                    // Whether the check itself failed (another try can help) or getting the model did.
                    Out.shared.send(["event": "failed", "id": id, "problem": encode(engine.problem(error, draft: text)),
                                     "checkFailed": error is RewriteError || error is CancellationError])
                }
                await self.done(id)
            }

        case "cancel":
            if let id = req["id"] as? Int {
                checks[id]?.cancel()
                checks[id] = nil
            }

        case "prewarm":
            guard let m = mode(req["mode"]) else { return }
            let rw = await rewriter(req)
            Task { await rw.prewarm(mode: m) }

        case "tones":
            let fresh = ToneList.newMac
            let list = ToneList(custom: decode([Mode].self, req["customTones"]) ?? fresh.custom,
                                order: req["toneOrder"] as? [String] ?? fresh.order,
                                hidden: (req["hiddenTones"] as? [String]).map(Set.init) ?? fresh.hidden,
                                defaultID: req["defaultMode"] as? String ?? fresh.defaultID,
                                perApp: decode([String: String].self, req["appModes"]) ?? fresh.perApp)
            Out.shared.send(["event": "tones", "shown": list.shown.map(encode), "all": list.all.map(encode),
                             "starting": list.starting(for: req["bundleID"] as? String)])

        default:
            log("unknown request: \(op)")
        }
    }

    func done(_ id: Int) { checks[id] = nil }
}

/// The app's unknown words as hints, picked as `SpellCheck.hints` picks the Mac's: each word once,
/// only the ones worth asking about (not a name in mid-sentence), only with guesses, at most
/// `SpellingHint.limit`. `start` counts UTF-16 units into `text`.
func flagged(_ words: [[String: Any]], in text: String) -> [SpellingHint] {
    var hints: [SpellingHint] = []
    var seen = Set<String>()
    let utf16 = Array(text.utf16)
    for w in words where hints.count < SpellingHint.limit {
        guard let word = w["word"] as? String, let guesses = w["guesses"] as? [String], !guesses.isEmpty,
              seen.insert(word).inserted else { continue }
        let start = min(max(0, w["start"] as? Int ?? 0), utf16.count)
        let before = String(decoding: utf16[..<start], as: UTF16.self)
        guard SpellingHint.isWorthAsking(word, atSentenceStart: SpellingHint.startsSentence(after: before)) else { continue }
        hints.append(SpellingHint(word, Array(guesses.prefix(3))))
    }
    return hints
}

/// Which models there are, which are on disk, and the one chosen (setup's step 2 asks).
func sendModels() async {
    let models: [[String: Any]] = engine.models().map {
        ["id": $0.id, "downloadSize": $0.downloadSize, "downloadBytes": $0.downloadBytes,
         "modelBytes": $0.modelBytes, "downloaded": $0.downloaded]
    }
    let chosen = await engine.chosen()
    Out.shared.send(["event": "hello", "models": models, "recommended": engine.recommended,
                     "chosen": chosen])
}

// MARK: - Main

// Isolate stdout for JSON-lines IPC before anything in MLX/Foundation can call `print()`.
_ = Out.shared
let brain = Brain()
Task {
    await engine.observe { state in
        Out.shared.send(encode(state))
        Task { await sendModels() }
    }
}
// Requests in order, each handed to the brain as it arrives; the brain answers when it can.
// On macOS, a raw Foundation Thread has no RunLoop to drain autoreleased Objective-C objects
// (e.g. from JSONSerialization), so each request runs inside its own autoreleasepool.
let processLine: (String) -> Void = { line in
    guard let data = line.data(using: .utf8),
          let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        log("not JSON: \(line.prefix(80))")
        return
    }
    let sem = DispatchSemaphore(value: 0)
    Task { await brain.handle(req); sem.signal() }
    sem.wait()
}
let input = Thread {
    while let line = readLine(strippingNewline: true) {
        #if os(macOS)
        autoreleasepool { processLine(line) }
        #else
        processLine(line)
        #endif
    }
    // The app went away: so does the brain (and llama.cpp's server with it).
    if engine.stopsBeforeExit {
        Task { await engine.unload(); exit(0) }
        Thread.sleep(forTimeInterval: 4)  // `Server.end` gives a server 3 s before killing it
    }
    #if os(macOS)
    // Without the C++ teardown `exit` runs: MLX's Metal queues destroyed while a check is still
    // on the GPU trap (1 in 5 quits during a check). The system frees all of it either way.
    _exit(0)
    #else
    exit(0)
    #endif
}
input.start()
dispatchMain()
