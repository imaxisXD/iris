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
import IrisEngine
import IrisSpelling

// MARK: - Out

/// Writes one line of JSON to stdout, one writer at a time.
final class Out: @unchecked Sendable {
    static let shared = Out()
    private let lock = NSLock()
    func send(_ object: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) else { return }
        lock.lock()
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([0x0A]))
        lock.unlock()
    }
}

func log(_ s: String) {
    FileHandle.standardError.write(("brain: " + s + "\n").data(using: .utf8)!)
}

// MARK: - Encoding

func encode(_ state: EngineState) -> [String: Any] {
    switch state {
    case .idle: return ["event": "state", "state": "idle"]
    case .downloading(let f): return ["event": "state", "state": "downloading", "fraction": f]
    case .loading: return ["event": "state", "state": "loading"]
    case .ready(let m): return ["event": "state", "state": "ready", "model": m.rawValue]
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
        let model = await Engine.shared.preferredChoice
        return Rewriter(generator: Engine.shared,
                        glossary: req["glossary"] as? [String] ?? [],
                        styleRules: req["styleRules"] as? [String] ?? [],
                        isForeign: { await LanguageCheck.looksNonEnglish($0) },
                        spellingHints: { await SpellCheck.hints(for: $0) },
                        promptStyle: model.promptStyle)
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
            if let raw = req["model"] as? String, let choice = ModelChoice(rawValue: raw) {
                await Engine.shared.setPreferred(choice)
            } else {
                let setUpBefore = req["onboarded"] as? Bool ?? false
                await Engine.shared.setPreferred(ModelChoice.starting(setUpBefore: setUpBefore, onDisk: \.isDownloaded, memoryGB: ModelChoice.macMemoryGB))
            }
            if let minutes = req["idleMinutes"] as? Int {
                await Engine.shared.setIdleUnload(minutes: minutes)
            }
            await sendModels()

        case "load":
            let preferred = await Engine.shared.preferredChoice
            let choice = (req["model"] as? String).flatMap(ModelChoice.init(rawValue:)) ?? preferred
            let allow = req["allowDownload"] as? Bool ?? false
            Task {
                do { try await Engine.shared.load(choice, allowDownload: allow) } catch {
                    log("load: \(error)")
                    if case EngineError.notDownloaded = error {
                        Out.shared.send(["event": "state", "state": "failed", "failure": encode(ModelFailure.notDownloaded)])
                    }
                }
            }

        case "unload":
            await Engine.shared.unload()

        case "delete":
            // `ModelDialogs.delete`: the chosen model is let go of first, then its folders (and
            // its drafter's, unless another model on disk still needs it) are removed.
            guard let raw = req["model"] as? String, let choice = ModelChoice(rawValue: raw) else { return }
            if await Engine.shared.preferredChoice == choice { await Engine.shared.unload() }
            for folder in choice.cacheDirectories {
                do { try FileManager.default.removeItem(at: folder) } catch CocoaError.fileNoSuchFile {
                } catch { log("delete: couldn't remove \(folder.path): \(error)") }
            }
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
                } catch is CancellationError {
                } catch {
                    if Task.isCancelled { return }
                    log("check \(id): \(error)")
                    // Whether the check itself failed (another try can help) or getting the model did.
                    Out.shared.send(["event": "failed", "id": id, "problem": encode(ProblemNote(error, draft: text)),
                                     "checkFailed": error is RewriteError])
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

/// Which models there are, which are on disk, and the one chosen (setup's step 2 asks).
func sendModels() async {
    let models: [[String: Any]] = ModelChoice.offered.map {
        ["id": $0.rawValue, "downloadSize": $0.downloadSize, "downloadBytes": $0.downloadBytes,
         "modelBytes": $0.modelBytes, "downloaded": $0.isDownloaded]
    }
    let chosen = await Engine.shared.preferredChoice
    Out.shared.send(["event": "hello", "models": models, "recommended": ModelChoice.recommended.rawValue,
                     "chosen": chosen.rawValue])
}

// MARK: - Main

let brain = Brain()
Task {
    await Engine.shared.observe { state in
        Out.shared.send(encode(state))
        Task { await sendModels() }
    }
}
// Requests in order, each handed to the brain as it arrives; the brain answers when it can.
let input = Thread {
    while let line = readLine(strippingNewline: true) {
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            log("not JSON: \(line.prefix(80))")
            continue
        }
        let sem = DispatchSemaphore(value: 0)
        Task { await brain.handle(req); sem.signal() }
        sem.wait()
    }
    // The app went away: so does the brain.
    exit(0)
}
input.start()
dispatchMain()
