import AppKit

/// The one way Iris puts its words in your text box: the panel's Replace and Quick Fix both
/// paste through here, so they say and do the same things after.
@MainActor
enum Paste {
    /// Puts `result` in the box Iris read `original` from and, once it has landed, says so
    /// with `message` and starts watching for a word put back (`LearnWords`). If the app it
    /// came from is no longer in front, nothing is pasted: the result is on the clipboard, and
    /// the message says so.
    static func replace(_ captured: TextBridge.Captured, original: String, with result: String, saying message: Message) async {
        let pasted = await TextBridge.replace(captured, with: result) {
            LearnWords.watch(captured, original: original, result: result)
            Toast.shared.show(message, near: captured.fieldFrame)
        }
        if !pasted { Toast.shared.show(.copied) }
    }
}
