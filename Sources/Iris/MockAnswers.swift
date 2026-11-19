import Foundation
import IrisCore
import IrisSpelling

/// `Iris --mock-answers`: canned answers instead of the model, so the whole panel can be driven
/// in a real app (`scripts/panel-check.sh`) without loading the model, the same every time.
/// Iris still reads the real text box, names the real app and pastes for real; only the
/// writing is faked.
///
/// What you type decides the answer:
/// - mistakes from `fixes` ("i have", "its look", "need to"): Fix corrects them (a small fix)
/// - "rewrite all of this": Fix comes back as a different sentence (a big rewrite)
/// - text with none of those: Fix finds nothing to change ("looks good")
/// - any tone: the fixed text with the tone's name in front, so every tone has something to paste
struct MockAnswers: Checker {
    /// How long an answer takes, so the panel goes through its working state first.
    static let delay: Duration = .milliseconds(350)

    static let fixes = [("i have", "I have"), ("its look", "it looks"), ("need to", "needs to"),
                        ("teh ", "the "), ("recieve", "receive")]
    static let rewriteCue = "rewrite all of this"

    func run(_ text: String, mode: Mode, retry: Bool) -> AsyncThrowingStream<RewriteEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                try? await Task.sleep(for: Self.delay)
                guard !Task.isCancelled else { return continuation.finish(throwing: CancellationError()) }
                let foreign = await LanguageCheck.looksNonEnglish(text)
                continuation.yield(.finished(Self.answer(text, mode: mode, translated: foreign)))
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    /// `translated`: the draft is mostly another language, so it comes back marked as a
    /// translation, with no verdict, as `Rewriter` marks it.
    static func answer(_ text: String, mode: Mode, translated: Bool = false) -> RewriteResult {
        var fixed = text
        for (wrong, right) in fixes { fixed = fixed.replacingOccurrences(of: wrong, with: right) }
        if text.lowercased().contains(rewriteCue) {
            fixed = "Here is a completely different sentence that says the same thing in other words."
        }
        let latency = Int(delay.components.attoseconds / 1_000_000_000_000_000)
        if mode.isFix, !translated {
            let verdict = FixGuard.evaluate(original: text, corrected: fixed)
            return RewriteResult(mode: mode, original: text, text: verdict.text, segments: verdict.segments,
                                 verdict: verdict.kind, latencyMs: latency)
        }
        let toned = mode.isFix ? fixed : "\(mode.label): \(fixed)"
        return RewriteResult(mode: mode, original: text, text: toned, segments: WordDiff.diff(text, toned),
                             verdict: nil, translated: translated, latencyMs: latency)
    }
}
