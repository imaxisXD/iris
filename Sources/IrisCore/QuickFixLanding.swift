import Foundation

/// Where a Quick Fix result goes. Quick Fix replaces text without showing it, so it only
/// pastes when that's certainly what you meant: a small fix, into a box that can take it,
/// that still holds exactly the text Iris read. A model that took long to wake gives you time
/// to move on; pasting then (select all, paste) would overwrite whatever you're in now.
public enum QuickFixLanding: Equatable, Sendable {
    /// Nothing to change: say "looks good".
    case clean
    /// Show the result in the panel instead: a person should look at it.
    case panel
    /// Paste it.
    case paste
    /// The box changed while Iris worked: paste nothing and say so.
    case leftAlone

    /// - Parameters:
    ///   - unchanged: Fix found nothing to change.
    ///   - heavy: a big rewrite rather than a few fixes.
    ///   - canReplace: the text can be edited where it is.
    ///   - translated: the draft was mostly another language.
    ///   - stillThere: the same text box still holds the same text Iris read.
    public static func decide(unchanged: Bool, heavy: Bool, canReplace: Bool, translated: Bool,
                              stillThere: Bool) -> QuickFixLanding {
        if unchanged { return .clean }
        if heavy || !canReplace || translated { return .panel }
        return stillThere ? .paste : .leftAlone
    }
}

extension QuickFixLanding {
    /// Where a Quick Fix that failed says so: in the panel, which has Try again, when the check
    /// itself failed and another try can work (ran out of time, came back empty); otherwise in
    /// a message by your text. Getting the model failing stays a message even when a retry can
    /// help (no internet): the panel would only open on the same wait.
    public static func failureOpensPanel(checkFailed: Bool, canRetry: Bool) -> Bool { checkFailed && canRetry }

    /// With no box to read again (text copied out of an app that hides it), Quick Fix can't
    /// tell whether you moved on, so it only pastes if its answer came quickly.
    public static let unreadableGrace = 4.0

    /// Whether a box Iris can't read again still counts as unchanged, `elapsed` seconds after
    /// the keys were pressed.
    public static func unreadableStillThere(after elapsed: Double) -> Bool { elapsed < unreadableGrace }
}
