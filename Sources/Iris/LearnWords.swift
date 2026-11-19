import AppKit
import IrisCore
import SwiftUI

/// Learning words: after a Replace, Iris looks at the field for a little while. If the writer
/// puts back a word Iris changed (a name, or a spelling they meant), Iris asks once, beside
/// the field, whether to leave that word alone from now on.
@MainActor
enum LearnWords {
    private static var task: Task<Void, Never>?
    /// How long to keep looking after a paste: long enough for ⌘Z or a quick retype.
    private static let watchSeconds = 25

    /// Starts looking, once Iris's version has been pasted (`Paste.replace`).
    static func watch(_ captured: TextBridge.Captured, original: String, result: String) {
        let settings = AppSettings.shared
        let candidates = WordWatch.candidates(FixNotes.changes(WordDiff.diff(original, result)), ignoring: settings.glossary)
        guard !candidates.isEmpty, captured.element != nil else { return }
        task?.cancel()
        task = Task { @MainActor in
            // First see Iris's version land, so a paste that failed isn't read as "put back".
            var landed = false
            for _ in 0..<watchSeconds {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled else { return }
                guard let text = await TextBridge.currentText(of: captured) else { continue }
                if !landed {
                    landed = WordWatch.landed(candidates, in: text)
                    continue
                }
                if let change = WordWatch.reverted(candidates, in: text) {
                    KeepWordNote.shared.ask(word: change.from, near: captured.fieldFrame)
                    return
                }
            }
        }
    }
}

/// "keep 'Abhijyan'?" with Keep it and No: a paper note that takes clicks without taking focus
/// from the app you're typing in.
@MainActor
final class KeepWordNote {
    static let shared = KeepWordNote()
    private var panel: NSPanel?
    private var hideTask: Task<Void, Never>?

    func ask(word: String, near anchor: CGRect?) {
        close()
        let view = KeepWordView(word: word, keep: { [weak self] in
            AppSettings.shared.learn(word)
            self?.close()
            Toast.shared.show(.kept(word), near: anchor)
        }, dismiss: { [weak self] in self?.close() })
        // In the message's place by the text box, so the message gives way to the question.
        Toast.shared.dismiss()
        let panel = FloatingNote.panel(view.padding(14), level: .statusBar, clickable: true)
        panel.setFrameOrigin(FloatingNote.origin(for: panel.frame.size, by: anchor))
        FloatingNote.fadeIn(panel)
        self.panel = panel
        FloatingNote.announce("Keep \(word) as you wrote it? You changed it back. Iris won't touch it again.")
        // Asked once; if it's ignored, it goes away on its own.
        hideTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(12))
            guard !Task.isCancelled else { return }
            self?.close()
        }
    }

    func close() {
        hideTask?.cancel()
        guard let panel else { return }
        self.panel = nil
        FloatingNote.fadeOut(panel)
    }
}

struct KeepWordView: View {
    let word: String
    let keep: () -> Void
    let dismiss: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Text("?").font(DS.Font.brush(20)).foregroundStyle(DS.Hue.pinkText)
                .frame(width: 24, height: 24)
                .overlay(BrushRing(rgb: Riso.pinkText, seed: 4))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text("keep \u{201C}\(word)\u{201D}?").font(DS.Font.brush(22)).foregroundStyle(DS.Ink.i12).lineLimit(1)
                Text("You changed it back. Iris won't touch it again.").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
            }
            TactileButton(title: "Keep it", role: .secondary, compact: true, action: keep)
            TactileButton(title: "No", role: .ghost, compact: true, action: dismiss)
        }
        .notePaper()
        .accessibilityElement(children: .contain)
    }
}
