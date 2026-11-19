import IrisCore
import IrisEngine
import IrisSpelling

/// Everything a check depends on besides the text and the tone, read from Settings. The
/// panel, Quick Fix and prewarming all build their checker here, so they ask the model the
/// same way (and so share its prefilled prompts), and the panel's memory of earlier answers
/// is keyed by exactly what an answer depends on.
@MainActor
struct CheckSetup: Equatable {
    let glossary: [String]
    let styleRules: [String]
    let model: ModelChoice

    static var current: CheckSetup {
        let settings = AppSettings.shared
        return CheckSetup(glossary: settings.glossary, styleRules: settings.styleRules, model: settings.model)
    }

    /// The model, or the stand-in a check run asked for (`Iris --mock-answers`, the self-check).
    var checker: any Checker {
        switch LaunchMode.current.answers {
        case .model:
            Rewriter(generator: Engine.shared, glossary: glossary, styleRules: styleRules,
                     isForeign: { await LanguageCheck.looksNonEnglish($0) },
                     spellingHints: { await SpellCheck.hints(for: $0) },
                     promptStyle: model.promptStyle)
        case .canned: MockAnswers()
        case .never: NoAnswers()
        }
    }

    /// Names one answer: this text in this tone, asked this way.
    func key(for text: String, mode: Mode) -> String {
        [mode.id, mode.instruction, glossary.joined(separator: "\u{1}"), styleRules.joined(separator: "\u{1}"),
         model.rawValue, text].joined(separator: "\u{2}")
    }
}

/// Answers that never come, for the self-check: it tests the panel, not the model, so the
/// panel waits as if an answer were on its way.
struct NoAnswers: Checker {
    func run(_ text: String, mode: Mode, retry: Bool) -> AsyncThrowingStream<RewriteEvent, Error> {
        AsyncThrowingStream { _ in }
    }
}
