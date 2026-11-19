/// The floating panel's life, without its windows, so the rules can be tested on their own.
///
///     hidden ── show ──▶ budding ── unfold ──▶ unfolded
///        ▲                  │                     │
///        │◀── close ────────┘                     │ close (animated)
///        │                                        ▼
///        └──────────── fold finished ────────── folding
///
/// Only `budding` and `unfolded` count as showing. While the panel folds away it's already
/// gone as far as the shortcut is concerned, so pressing it then opens a new panel instead
/// of being taken as a second close. A fold that finishes after a new panel was shown never
/// hides that new one: every show has its own `generation`.
public struct PanelLife: Equatable, Sendable {
    public enum Stage: Equatable, Sendable {
        /// Off screen.
        case hidden
        /// The bud stands on the text box; the panel is on screen but invisible and folded,
        /// taking keys, until there's something to read.
        case budding
        /// Open out of the bud.
        case unfolded
        /// Folding back into the flower; taken off screen when that's done.
        case folding
    }

    /// What `close` asks the window to do.
    public enum Close: Equatable, Sendable {
        /// It was already gone.
        case nothing
        /// Take it off screen now.
        case hideNow
        /// Play the fold, then call `foldFinished(generation)`.
        case fold(generation: Int)
    }

    public private(set) var stage = Stage.hidden
    /// Counts panels shown.
    public private(set) var generation = 0

    public init() {}

    public var isShowing: Bool { stage == .budding || stage == .unfolded }

    /// A new panel for a new text box, folded into its bud. Also replaces one folding away.
    public mutating func show() {
        generation += 1
        stage = .budding
    }

    /// Opens the panel out of the bud. Returns false when there's nothing to open (it already
    /// is, or it has gone).
    public mutating func unfold() -> Bool {
        guard stage == .budding else { return false }
        stage = .unfolded
        return true
    }

    /// `immediately`: Iris is about to paste, so the panel must go at once. `reduceMotion`:
    /// no fold either.
    public mutating func close(immediately: Bool, reduceMotion: Bool) -> Close {
        switch stage {
        case .hidden:
            return .nothing
        case .unfolded where !immediately && !reduceMotion:
            stage = .folding
            return .fold(generation: generation)
        case .budding, .unfolded, .folding:
            stage = .hidden
            return .hideNow
        }
    }

    /// Ends a fold under way at once (the shortcut was pressed again: the folding panel still
    /// has the keyboard, and the text box underneath must get it back before it's read).
    /// Returns whether there was one; take the window off screen if so.
    public mutating func finishFold() -> Bool {
        guard stage == .folding else { return false }
        stage = .hidden
        return true
    }

    /// The fold for `generation` has played. Returns whether to take the window off screen:
    /// false when a newer panel is showing in it.
    public mutating func foldFinished(_ generation: Int) -> Bool {
        guard generation == self.generation, stage == .folding else { return false }
        stage = .hidden
        return true
    }
}

/// Tells the click that closed a menu from a new click on its trigger.
///
/// A click on a menu's own trigger (the menu bar flower, a picker) closes the open menu on the
/// press, because the menu loses focus, and only then runs the trigger's action on the
/// release. Without this the release would open the menu again.
///
/// It goes by time, measured from when the click's press began, so however long the press is
/// held doesn't matter. The menu notes when it closed; the trigger's action asks with its own
/// press's timestamp. The menu bar flower acts on the press itself, and macOS takes focus from
/// the slip a moment before that press's timestamp (about 10 ms, seen by `Iris --self-check`),
/// so a close up to `lead` before the press still counts as this click's.
public struct ClosingPress: Equatable, Sendable {
    /// How long before a press's timestamp its own click may already have closed the menu.
    /// Far shorter than closing a menu with Esc and then clicking its trigger on purpose.
    public static let lead = 0.25

    private var closedAt: Double?

    public init() {}

    /// The menu closed at `time` (seconds since the Mac started, like event timestamps).
    public mutating func closed(at time: Double) {
        closedAt = time
    }

    /// Whether the click whose press began at `pressBegan` is the one that closed the menu;
    /// nil when the trigger wasn't clicked (a key). The answer is used up, so the next click
    /// always counts.
    public mutating func isClosingClick(pressBegan: Double?) -> Bool {
        defer { closedAt = nil }
        guard let closedAt, let pressBegan else { return false }
        return closedAt >= pressBegan - Self.lead
    }
}
