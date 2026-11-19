import Foundation

/// What this run of Iris is for, read once from its command line: the app itself, or one of
/// the checks and design tools that drive it without the model or anyone at the Mac. Every
/// flag that changes how Iris launches is here, with how much of the real launch it gets
/// (`main.swift` and `AppDelegate` switch on it); the scripts in `scripts/` pass these same
/// strings (`LaunchModeTests` reads `scripts/lib.sh` to keep them the same).
/// (`--sample-changelog`, in SettingsUpdates.swift, only swaps what Updates reads.)
public enum LaunchMode: Equatable, Sendable {
    /// The menu bar app.
    case app
    /// The app with canned answers instead of the model (`scripts/panel-check.sh`).
    case mockAnswers
    /// Drives the real app with keys and clicks, then quits (`scripts/self-check.sh`).
    case selfCheck
    /// A text box for the self-check to type in, in a process of its own.
    case practiceBox
    /// Draws pictures into a folder and quits: no menu bar flower, no shortcuts.
    case render(Render, into: String)
    /// Opens setup in a made-up state (see SetupStates.swift).
    case setupPreview(String)
    /// Opens Settings alone, at a page when one is named ("model"): no menu bar flower, no
    /// shortcuts, and without coming to the front, so it can be looked at on another display.
    case settingsPreview(String?)
    /// Opens every message by your text in a window with a switcher (see ToastStates.swift).
    case toastWorkbench(start: String?)
    /// Opens the slip from the real menu bar flower and keeps it open, optionally playing one
    /// of its scenes (see `AppDelegate.previewStatusMenu`).
    case statusMenuPreview(String?)
    /// Plays the bud and the panel unfolding over a stand-in text box, over and over.
    case panelPreview

    public enum Render: String, CaseIterable, Sendable {
        case icon = "--icon", rigFrames = "--rig-frames", readmeImages = "--readme-images", setupStates = "--setup-states",
             toastStates = "--toast-states", panelStates = "--panel-states", snapshot = "--snapshot"
    }

    public static let mockAnswersFlag = "--mock-answers"
    public static let selfCheckFlag = "--self-check"
    public static let practiceBoxFlag = "--practice-box"
    public static let onDisplayFlag = "--on-display"

    /// `--on-display <screen name>`: this run on that screen only, sealed for `scripts/verify`
    /// (see `OnDisplay` in the app). Nil when not given.
    public static func onDisplay(in arguments: [String]) -> String? {
        guard let index = arguments.firstIndex(of: onDisplayFlag), index + 1 < arguments.count else { return nil }
        return arguments[index + 1]
    }

    public static let current = LaunchMode(arguments: CommandLine.arguments)

    public init(arguments: [String]) {
        func value(after flag: String) -> String?? {
            guard let index = arguments.firstIndex(of: flag) else { return nil }
            // The next word, unless it's another flag (`--settings-preview --on-display X`).
            return .some(arguments.dropFirst(index + 1).first.flatMap { $0.hasPrefix("--") ? nil : $0 })
        }
        if arguments.contains(Self.practiceBoxFlag) { self = .practiceBox; return }
        for render in Render.allCases {
            if let folder = value(after: render.rawValue), let folder { self = .render(render, into: folder); return }
        }
        if let state = value(after: "--setup-preview") { self = .setupPreview(state ?? ""); return }
        if let page = value(after: "--settings-preview") { self = .settingsPreview(page); return }
        if let start = value(after: "--toast-workbench") { self = .toastWorkbench(start: start); return }
        if let row = value(after: "--preview-status-menu") { self = .statusMenuPreview(row); return }
        if arguments.contains("--preview-panel") { self = .panelPreview; return }
        if arguments.contains(Self.selfCheckFlag) { self = .selfCheck; return }
        if arguments.contains(Self.mockAnswersFlag) { self = .mockAnswers; return }
        self = .app
    }

    /// A drawing flag given without its folder, if there is one.
    public static func renderFlagMissingFolder(in arguments: [String]) -> String? {
        Render.allCases.map(\.rawValue).first { flag in
            arguments.firstIndex(of: flag).map { $0 == arguments.count - 1 } ?? false
        }
    }

    /// Who writes the answers.
    public enum Answers: Sendable { case model, canned, never }
    public var answers: Answers {
        switch self {
        case .mockAnswers: .canned
        case .selfCheck: .never
        default: .model
        }
    }

    /// Checks need the model on this Mac: setup isn't done without it.
    public var needsModel: Bool { answers == .model }
}
