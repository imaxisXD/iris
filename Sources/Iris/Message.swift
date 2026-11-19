import AppKit
import IrisCore
import IrisEngine

/// Every message Iris shows by your text: its words, its kind, how long it stays and the
/// switch in Settings that turns it off. The app shows them from here and `--toast-states`
/// draws them from here, so the design check can't drift from what people see.
@MainActor
struct Message {
    /// Iris's word, lowercase and short ("replaced", "nothing to read").
    let lead: String
    /// The plain sentence after it; `keys` are drawn as keycaps at its end.
    var detail: String? = nil
    var keys: [String] = []
    /// A name printed as an ink label after the detail ("1Password").
    var label: String? = nil
    /// An app icon shown crossed out in brush instead of the usual mark (Iris is off there).
    var crossed: NSImage? = nil
    /// A second, quieter line under the rest ("Turn it back on in Settings › Apps.").
    var extra: String? = nil
    var kind: Toast.Kind = .neutral
    /// How long it stays up. A problem stays until the next click or key press, up to a
    /// minute: it says what to do next, and nobody should have to read that against a timer.
    var hold: Duration = DS.Motion.toastHold
    /// The switch in Settings › General › When Iris speaks up that turns it off.
    var note: AppSettings.Note? = nil

    var stays: Duration { kind == .warning ? .seconds(60) : hold }

    // MARK: Done

    /// After a paste, the app's own ⌘Z takes it back: the message says so (`undoKeys`, as
    /// Settings has it unless given).
    static func replaced(undoKeys: Bool? = nil) -> Message {
        undoable("replaced", undoKeys)
    }

    /// Quick Fix pasted its fix.
    static func fixed(_ words: Int, undoKeys: Bool? = nil) -> Message {
        undoable(words == 1 ? "fixed 1 word" : "fixed \(words) words", undoKeys)
    }

    private static func undoable(_ lead: String, _ undoKeys: Bool?) -> Message {
        let undoKeys = undoKeys ?? AppSettings.shared.shows(.undoKeys)
        return Message(lead: lead, detail: undoKeys ? "Undo with" : nil, keys: undoKeys ? ["⌘", "Z"] : [], kind: .success,
                       note: .replaced)
    }

    static let looksGood = Message(lead: "looks good", detail: "Nothing to fix.", kind: .success, note: .clean)

    /// Copy on text that can't be edited where it is.
    static let copied = Message(lead: "copied", detail: "Paste it where you need it.", keys: ["⌘", "V"], kind: .success)

    /// You put a word back after a replace, so Iris learned it.
    static func kept(_ word: String) -> Message {
        Message(lead: "kept", detail: "Iris won't change \u{201C}\(word)\u{201D} again.", kind: .success)
    }

    // MARK: Quiet notes

    /// Quick Fix started. Stays until the result replaces it: the check may take a while if
    /// the model is still waking up, and the note mustn't say it's over before it is.
    static let checking = Message(lead: "checking…", detail: "Reading the box you're in.", hold: .seconds(120), note: .checking)

    /// Quick Fix finished after the box changed, so it pasted nothing.
    static let leftAlone = Message(lead: "left it alone", detail: "Your text changed while Iris was checking, so it pasted nothing.",
                                   hold: .seconds(4))

    /// The shortcut in an app Iris is turned off in.
    static func turnedOff(in app: NSRunningApplication?) -> Message {
        turnedOff(name: app?.nameForPeople, icon: app?.icon)
    }

    static func turnedOff(name: String?, icon: NSImage?) -> Message {
        if let name {
            return Message(lead: "turned off here", detail: "Iris doesn't read text in", label: name, crossed: icon,
                           extra: "Turn it back on in Settings › Apps.", note: .excluded)
        }
        return Message(lead: "turned off here", detail: "Iris doesn't read text in this app.",
                       extra: "Turn it back on in Settings › Apps.", note: .excluded)
    }

    // MARK: Problems

    /// A failed capture or check, said the same way everywhere: the same words as the panel's
    /// red-pen note (`ProblemNote`), never the system's.
    static func problem(_ error: Error, draft: String = "") -> Message {
        switch error {
        case TextBridge.CaptureError.nothingToCheck:
            return Message(lead: "nothing to read", detail: "Click in a text box or select text, then press",
                           keys: AppSettings.shared.openShortcut.symbols, kind: .warning)
        case TextBridge.CaptureError.secureField:
            return Message(lead: "password field", detail: "Iris never reads those.")
        default:
            let problem = ProblemNote(error, draft: draft)
            return Message(lead: problem.lead, detail: problem.detail, kind: .warning)
        }
    }
}
