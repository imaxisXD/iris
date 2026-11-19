/// What the panel's bottom row shows: a note on the left, the app your text is in, and
/// the one button. `PanelView` only draws it, so every state can be tested here.
public struct PanelFooter: Equatable, Sendable {
    /// Where the panel's answer is.
    public enum Stage: Equatable, Sendable {
        /// Waiting for the model, or text streaming in.
        case working(fix: Bool)
        /// `verdict` is Fix's (nil for a tone).
        case done(fix: Bool, verdict: FixGuard.Verdict.Kind?)
        case failed(canRetry: Bool)
    }

    /// The left of the row: what you need to know right now, otherwise the keys.
    public enum Note: Equatable, Sendable {
        case copied
        case retry(String)
        case looksGood
        case translated
        case bigRewrite
        case keys(nextTone: Bool, tryAgain: Bool)
        case none
    }

    public enum Action: Equatable, Sendable {
        case checking  // Fix at work
        case writing  // a tone at work
        case replace
        case copy
        case done  // nothing to fix: closes, pastes nothing
        case tryAgain
        case none
    }

    public let note: Note
    /// The app beside the button, named the same way in Fix and every tone.
    public enum AppLabel: Equatable, Sendable {
        /// "into Slack": where Replace pastes.
        case pastesInto(String)
        /// "nothing to change in Slack": Fix found nothing, and Done pastes nothing.
        case nothingToChange(String)

        public var app: String {
            switch self {
            case .pastesInto(let app), .nothingToChange(let app): app
            }
        }
    }

    /// Nil while there's no answer yet, after a failure, for text that can only be copied,
    /// or when Iris doesn't know the app.
    public let app: AppLabel?
    public let action: Action

    public init(stage: Stage, appName: String?, canReplace: Bool, translated: Bool,
                copied: Bool, retryNote: String?, toneCount: Int) {
        let clean: Bool
        if case .done(true, .unchanged) = stage { clean = true } else { clean = false }

        switch stage {
        case .working(let fix): action = fix ? .checking : .writing
        case .failed(let canRetry): action = canRetry ? .tryAgain : .none
        case .done(_, let verdict): action = verdict == .unchanged ? .done : canReplace ? .replace : .copy
        }

        // Beside the button, every tone and Fix alike: where Replace will paste, or, when
        // there's nothing to fix, that your text in that app stays as it is.
        switch (action, appName) {
        case (.replace, let name?): app = .pastesInto(name)
        case (.done, let name?): app = .nothingToChange(name)
        default: app = nil
        }

        if copied {
            note = .copied
        } else if let retryNote {
            note = .retry(retryNote)
        } else if clean {
            note = .looksGood
        } else if case .done(let fix, let verdict) = stage {
            note = translated ? .translated : fix && verdict == .heavy ? .bigRewrite
                : .keys(nextTone: toneCount > 1, tryAgain: true)
        } else if case .failed = stage {
            // A failure offers Try again as a button on the right instead.
            note = .keys(nextTone: toneCount > 1, tryAgain: false)
        } else {
            note = .none
        }
    }
}
