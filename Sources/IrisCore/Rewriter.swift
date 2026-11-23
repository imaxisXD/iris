import Foundation

/// Anything that can turn chat turns into streamed text. The MLX engine conforms;
/// tests use a fake.
public protocol TextGenerator: Sendable {
    /// Streams text deltas. `cacheKey` names the fixed part of the prompt (everything before
    /// the user's text) so a generator can reuse its prefill across requests.
    func stream(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?) -> AsyncThrowingStream<String, Error>
    /// Gets the generator ready (loaded into memory) before the timed part of a request starts.
    func ensureReady() async throws
    /// Prefills the fixed part of a prompt ahead of time.
    func prewarm(_ turns: [PromptTurn], cacheKey: String) async
}

public extension TextGenerator {
    func ensureReady() async throws {}
    func prewarm(_ turns: [PromptTurn], cacheKey: String) async {}
}

public struct RewriteResult: Equatable, Sendable {
    public let mode: Mode
    public let original: String
    public let text: String
    public let segments: [WordDiff.Segment]
    /// How much a proofread changed: set for Fix, nil for tones and for a translation (which
    /// has nothing in common with the draft to measure).
    public let verdict: FixGuard.Verdict.Kind?
    /// The draft was mostly another language: Fix translated it, and a tone wrote it in English.
    public let translated: Bool
    public let latencyMs: Int

    public init(mode: Mode, original: String, text: String, segments: [WordDiff.Segment],
                verdict: FixGuard.Verdict.Kind?, translated: Bool = false, latencyMs: Int) {
        self.mode = mode
        self.original = original
        self.text = text
        self.segments = segments
        self.verdict = verdict
        self.translated = translated
        self.latencyMs = latencyMs
    }

    /// Words changed, counted the way `FixGuard` judges a fix ("fixed 3 words").
    public var changedWords: Int { FixGuard.changedWords(in: segments) }
}

/// Whatever answers a check: the model through `Rewriter`, or a stand-in that answers without
/// it (canned answers for the panel check, none at all for the self-check).
public protocol Checker: Sendable {
    /// Whether answers wait on the model loading, so the panel says it's waking up.
    var usesModel: Bool { get }
    /// `retry`: the user asked for another version, so sample more freely; a repeat of the
    /// deterministic answer would look like nothing happened.
    func run(_ text: String, mode: Mode, retry: Bool) -> AsyncThrowingStream<RewriteEvent, Error>
    /// Gets ready for a check in `mode` ahead of time (load, prefill). Nothing for stand-ins.
    func prewarm(mode: Mode) async
}

public extension Checker {
    var usesModel: Bool { false }
    func prewarm(mode: Mode) async {}

    /// The finished answer, without the text as it streams.
    func check(_ text: String, mode: Mode) async throws -> RewriteResult {
        for try await event in run(text, mode: mode, retry: false) {
            if case .finished(let result) = event { return result }
        }
        throw CancellationError()
    }
}

public enum RewriteEvent: Equatable, Sendable {
    case partial(String)
    case finished(RewriteResult)
}

public enum RewriteError: Error, Equatable, LocalizedError {
    case emptyInput
    case tooLong(limit: Int)
    case lostProtectedText
    case emptyOutput
    case timedOut
    /// The model answered something other than the message (reasoning, an explanation),
    /// typically because the input isn't a message at all ("sdsaiofsi").
    case offTask

    /// The words people see for it (`ProblemNote`), for anything that prints the error.
    public var errorDescription: String? { ProblemNote(self).detail }
}

/// Protect -> generate -> restore -> guard. One pipeline for Fix and for tones;
/// only the prompt, temperature, and the guard differ.
public struct Rewriter: Checker {
    public let generator: any TextGenerator
    public let protector: Protector
    /// The writer's own style rules, followed by every tone (Fix only corrects mistakes).
    public let styleRules: [String]
    /// Decides whether a draft is mostly another language, so Fix translates it with
    /// `Prompts.translateTurns` instead of proofreading. Gets the masked text, so mentions,
    /// links and glossary words are placeholders. The app passes LanguageCheck.
    let isForeign: (@Sendable (String) async -> Bool)?
    /// Finds the words the spell checker doesn't know in a Fix draft (masked too). The app
    /// passes SpellCheck.
    let spellingHints: (@Sendable (String) async -> [SpellingHint])?
    /// How the model is asked: Iris's trained models get the short prompts they learned with.
    public let promptStyle: PromptStyle
    /// Scales the per-attempt time limit. Tests shrink it; the app leaves it at 1.
    var timeLimitScale: Double = 1

    public init(generator: any TextGenerator, glossary: [String] = [], styleRules: [String] = [],
                isForeign: (@Sendable (String) async -> Bool)? = nil,
                spellingHints: (@Sendable (String) async -> [SpellingHint])? = nil,
                promptStyle: PromptStyle = .taught) {
        self.generator = generator
        self.promptStyle = promptStyle
        self.protector = Protector(glossary: glossary)
        self.styleRules = styleRules
        self.isForeign = isForeign
        self.spellingHints = spellingHints
    }

    public var usesModel: Bool { true }

    public func run(_ text: String, mode: Mode, retry: Bool = false) -> AsyncThrowingStream<RewriteEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let result = try await perform(text, mode: mode, retry: retry) { partial in
                        continuation.yield(.partial(partial))
                    }
                    continuation.yield(.finished(result))
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    /// Longest text checked in one go. Chat messages are far shorter; this keeps a pasted
    /// document from tying up the model for minutes.
    public static let maxInputCharacters = 6000
    /// Upper bound on Unicode scalars so combining-mark/Zalgo text can't smuggle far more
    /// scalars and tokens past the grapheme-cluster limit above.
    static let maxInputScalars = maxInputCharacters * 4

    /// Starts loading the model and prefilling this mode's fixed prompt, so the request
    /// that follows only pays for the user's own words.
    public func prewarm(mode: Mode) async {
        let turns = Self.turns(for: "x", mode: mode, styleRules: styleRules, style: promptStyle)
        await generator.prewarm(turns, cacheKey: Self.cacheKey(for: turns, mode: mode))
    }

    static func turns(for body: String, mode: Mode, styleRules: [String] = [], hints: [SpellingHint] = [],
                      style: PromptStyle = .taught) -> [PromptTurn] {
        mode.isFix
            ? Prompts.fixTurns(for: body, hints: hints, style: style)
            : Prompts.toneTurns(for: body, mode: mode, styleRules: styleRules, style: style)
    }

    /// Names the fixed part of the prompt: the mode plus a hash of every turn before the
    /// user's text, so editing a tone's instruction invalidates its cache.
    static func cacheKey(for turns: [PromptTurn], mode: Mode) -> String {
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for turn in turns.dropLast() {
            for byte in (turn.role.rawValue + "\u{1}" + turn.content).utf8 {
                hash = (hash ^ UInt64(byte)) &* 0x100_0000_01b3
            }
        }
        return "\(mode.id):\(String(hash, radix: 16))"
    }

    /// Time allowed for one attempt once the model is loaded: a fixed allowance plus a
    /// per-token budget. Loading is never counted.
    static func deadline(maxTokens: Int, mode: Mode) -> Duration {
        .milliseconds((mode.isFix ? 4000 : 6000) + maxTokens * 45)
    }

    public func perform(_ text: String, mode: Mode, retry: Bool = false,
                        onPartial: @escaping @Sendable (String) -> Void) async throws -> RewriteResult {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw RewriteError.emptyInput }
        guard text.count <= Self.maxInputCharacters, text.unicodeScalars.count <= Self.maxInputScalars else {
            throw RewriteError.tooLong(limit: Self.maxInputCharacters)
        }

        let masked = protector.mask(text)
        let body = masked.text.trimmingCharacters(in: .whitespacesAndNewlines)
        let foreign = await isForeign?(body) ?? false
        // A tone writes English whatever it's given; Fix has to be told to translate.
        let translating = mode.isFix && foreign
        // One word on its own is as often a name or random letters as a typo: no hints. Trained
        // models were taught without them.
        let hints = mode.isFix && !translating && promptStyle == .taught && FixGuard.wordCount(body) >= 2
            ? Array((await spellingHints?(body) ?? []).prefix(SpellingHint.limit)) : []
        let turns = translating
            ? Prompts.translateTurns(for: body)
            : Self.turns(for: body, mode: mode, styleRules: styleRules, hints: hints, style: promptStyle)
        let maxTokens = Prompts.maxTokens(for: body, mode: mode)
        let cacheKey = Self.cacheKey(for: turns, mode: mode)

        try await generator.ensureReady()
        let started = Date()

        // Fix is deterministic; a second attempt gets a little randomness so it can differ.
        // "Try again" samples more freely from the start.
        let temperatures: [Float] = switch (mode.isFix, retry) {
        case (true, false): [0, 0.3]
        case (true, true): [0.6, 0.6]
        // Trained models were taught one best answer per tone and judged at 0.
        case (false, false): promptStyle == .trained ? [0, 0.3] : [0.7, 0.7]
        case (false, true): [0.95, 0.95]
        }
        var restored: String?
        var offTask = false, lostText = false
        for temperature in temperatures {
            let output = try await generateWithDeadline(
                turns, temperature: temperature, maxTokens: maxTokens, cacheKey: cacheKey,
                deadline: Self.deadline(maxTokens: maxTokens, mode: mode) * timeLimitScale
            ) { partial in
                onPartial(protector.restoringWhatsThere(Self.answer(in: partial, mode: mode, draft: text), from: masked))
            }
            var cleaned = Self.answer(in: output, mode: mode, draft: text)
            guard !cleaned.isEmpty else { continue }
            if mode.isFix, !translating { cleaned = FixGuard.capitalizingI(cleaned) }
            guard !Self.isOffTask(cleaned, input: body, mode: mode) else {
                offTask = true
                continue
            }
            if let value = try? protector.restore(cleaned, from: masked) {
                restored = value
                break
            }
            lostText = true
        }
        guard let finalText = restored else {
            if lostText { throw RewriteError.lostProtectedText }
            throw offTask ? RewriteError.offTask : RewriteError.emptyOutput
        }

        let latency = Int(Date().timeIntervalSince(started) * 1000)
        if mode.isFix, !translating {
            let verdict = FixGuard.evaluate(
                original: text, corrected: FixGuard.replacingAddedDashes(in: finalText, keeping: text))
            return RewriteResult(mode: mode, original: text, text: verdict.text, segments: verdict.segments,
                                 verdict: verdict.kind, latencyMs: latency)
        }
        // A tone, or a translation: no verdict, and none of the model's own dashes or emoji.
        let toned = FixGuard.preservingOuterWhitespace(
            of: text, in: FixGuard.replacingAddedDashes(
                in: FixGuard.removingAddedEmoji(from: finalText, keeping: text), keeping: text))
        return RewriteResult(mode: mode, original: text, text: toned, segments: WordDiff.diff(text, toned),
                             verdict: nil, translated: foreign, latencyMs: latency)
    }

    /// Runs one generation, racing it against a deadline. On timeout the generation is
    /// cancelled (which frees the GPU) and the caller gets `.timedOut`.
    private func generateWithDeadline(
        _ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String,
        deadline: Duration, onPartial: @escaping @Sendable (String) -> Void
    ) async throws -> String {
        let generator = self.generator
        return try await withThrowingTaskGroup(of: String?.self) { group in
            group.addTask {
                var output = ""
                var lastEmit = ContinuousClock.Instant.now - .seconds(1)
                for try await delta in generator.stream(turns, temperature: temperature, maxTokens: maxTokens, cacheKey: cacheKey) {
                    try Task.checkCancellation()
                    output += delta
                    // First delta fires immediately (TTFT); burst tokens within a single speculative
                    // round coalesce to ~60Hz so regex cleaning and JSON IPC don't run 16× per ms.
                    let now = ContinuousClock.Instant.now
                    if now - lastEmit >= .milliseconds(16) {
                        lastEmit = now
                        onPartial(output)
                    }
                }
                // A cancelled stream ends early without throwing: never hand back half an answer.
                try Task.checkCancellation()
                return output
            }
            group.addTask {
                try await Task.sleep(for: deadline)
                return nil
            }
            defer { group.cancelAll() }
            guard let first = try await group.next(), let output = first else { throw RewriteError.timedOut }
            try Task.checkCancellation()
            return output
        }
    }

    /// The message in what the model wrote, as it streams and when it's done: no reasoning,
    /// no wrappers, no tone label.
    static func answer(in output: String, mode: Mode, draft: String) -> String {
        FixGuard.removingLabel(FixGuard.clean(stripThinking(output), keeping: draft), label: mode.label, keeping: draft)
    }

    private static let controlTokenRegex = try! NSRegularExpression(
        pattern: #"<\|[a-z_]+\|?>|<[a-z_]+\|>|</?(start|end)_of_turn>"#
    )

    /// Some chat templates can emit a reasoning block; never show it as the answer.
    static func stripThinking(_ text: String) -> String {
        guard text.contains("<") else { return text }
        var t = text
        for (open, close) in [("<think>", "</think>"), ("<thought>", "</thought>"), ("<|channel>", "<channel|>")] {
            while let start = t.range(of: open), let end = t.range(of: close, range: start.upperBound..<t.endIndex) {
                t.removeSubrange(start.lowerBound..<end.upperBound)
            }
            // A closing tag with no opening one ("reasoning</think>answer"): keep the answer.
            if t.range(of: open) == nil, let end = t.range(of: close, options: .backwards) {
                t = String(t[end.upperBound...])
            }
            // An opening tag that never closed (the answer ran out of room mid-thought, or
            // it's still streaming): everything after it is reasoning.
            if let start = t.range(of: open) {
                t = String(t[..<start.lowerBound])
            }
        }
        // Any other control token the template leaks ("<|turn>", "<end_of_turn>").
        // Only the model's own tokens: they carry a pipe, or are Gemma's turn markers, so a
        // user's "<Product>" survives.
        if t.contains("<") {
            let ns = t as NSString
            t = controlTokenRegex.stringByReplacingMatches(
                in: t, range: NSRange(location: 0, length: ns.length), withTemplate: "")
        }
        return t
    }

    /// A proofread or rewrite is about as long as the message. Many more words, or the
    /// shape of an explanation ("Thinking Process:", "**Analyze"), means the model talked
    /// about the text instead of returning it.
    static func isOffTask(_ output: String, input: String, mode: Mode) -> Bool {
        let lower = output.lowercased()
        let markers = ["thinking process", "**analyze", "**check for", "the input is", "the user wants", "the user is asking",
                       "no text provided", "nothing to correct", "please provide", "appears to be gibberish",
                       "not a complete sentence", "doesn't appear to be", "does not appear to be", "random letters",
                       // A refusal, or a reply to the writer, in place of the rewrite.
                       "i cannot fulfill", "i can't fulfill", "i cannot rewrite", "i can't rewrite", "i cannot help", "i can't help",
                       "i can only rewrite", "not sure how to respond", "not sure what you mean", "could you please clarify",
                       "i do not have information", "i don't have information", "i don't have any information", "as an ai"]
        // An answer that still carries the draft's own words is the draft, corrected: "user want
        // dark mode" comes back as "the user wants dark mode", which only looks like a marker.
        if !keepsTheDraft(output, input: input, mode: mode) {
            for marker in markers where lower.contains(marker) && !input.lowercased().contains(marker) {
                return true
            }
        }
        // "You're doing great" answered as "I'm doing great": the model took the writer's
        // message as said to it. The draft speaks to "you" and never of "I"; the answer does the reverse.
        if flipsYouToI(output, input: input) { return true }
        // A whole answer in parentheses or brackets is a note to the user, not the message.
        let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
        if let first = trimmed.first, let last = trimmed.last, (first, last) == ("(", ")") || (first, last) == ("[", "]"),
            !input.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix(String(first)) {
            return true
        }
        let inWords = max(FixGuard.wordCount(input), 1)
        let outWords = FixGuard.wordCount(output)
        let allowed = mode.isFix ? inWords * 2 + 6 : inWords * 3 + 12
        return outWords > allowed
    }

    static func flipsYouToI(_ output: String, input: String) -> Bool {
        func words(_ s: String) -> Set<String> {
            Set(s.lowercased().replacingOccurrences(of: "\u{2019}", with: "'")
                .split(whereSeparator: { !($0.isLetter || $0 == "'") }).map(String.init))
        }
        let you: Set<String> = ["you", "your", "you're", "youre", "yours", "u", "ur", "you've", "you'll", "you'd"]
        let me: Set<String> = ["i", "i'm", "im", "my", "me", "mine", "i've", "i'll", "i'd", "myself"]
        let inWords = words(input), outWords = words(output)
        return !inWords.isDisjoint(with: you) && inWords.isDisjoint(with: me)
            && !outWords.isDisjoint(with: me) && outWords.isDisjoint(with: you)
    }

    /// Whether enough of a draft's words are still in the answer for it to be that draft,
    /// fixed or reworded: most of them for Fix, some for a tone, which rewrites freely. A draft
    /// of a word or two is never enough to go by (an explanation quotes it back).
    static func keepsTheDraft(_ output: String, input: String, mode: Mode) -> Bool {
        func words(_ s: String) -> [String] {
            s.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init)
        }
        let draft = words(input)
        guard draft.count >= 3 else { return false }
        let answer = Set(words(output))
        let kept = draft.filter(answer.contains).count
        return Double(kept) / Double(draft.count) >= (mode.isFix ? 0.6 : 0.3)
    }
}
