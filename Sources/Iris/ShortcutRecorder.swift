import AppKit
import Carbon.HIToolbox
import SwiftUI

/// Click the keys, press the new combination. Esc cancels; ⌫ turns it off (when allowed).
/// The keys are printed like a Mac's (symbol at the top right, word along the bottom) in the
/// pink plate, off register. While listening, Iris's own shortcuts are paused so the current
/// combo can be re-recorded, and the keys go back to bare paper waiting for the new ones.
struct ShortcutRecorder: View {
    let combo: KeyCombo?
    let other: KeyCombo?
    let action: HotkeyCenter.Action
    var allowsClear = false
    /// Snapshots only.
    var forcedRecording = false
    let onChange: (KeyCombo?) -> Void

    /// The key watcher while listening; listening is exactly when there is one.
    @State private var monitor: Any?
    private var recording: Bool { monitor != nil }
    @State private var problem: ShortcutProblem?
    @State private var pendingWarning: KeyCombo?

    var body: some View {
        let listening = recording || forcedRecording
        VStack(alignment: .trailing, spacing: 6) {
            Button(action: toggle) {
                HStack(spacing: 5) {
                    if listening {
                        let waiting = combo.map { Array($0.keycaps.dropLast()) } ?? [("⌃", "control"), ("⌥", "option")]
                        ForEach(Array(waiting.enumerated()), id: \.offset) { i, key in
                            Keycap(key: key.word, size: .labeled, seed: 70 + i * 7, symbol: key.symbol)
                        }
                        Keycap(key: "…", size: .labeled, seed: 97)
                    } else if let combo {
                        ForEach(Array(combo.keycaps.enumerated()), id: \.offset) { i, key in
                            Keycap(key: key.word, size: .labeled, seed: 11 + i * 13 + Int(action.rawValue) * 5,
                                   symbol: key.symbol, printed: true)
                        }
                    } else {
                        Text("Off").font(DS.Font.body).foregroundStyle(DS.Ink.i10)
                            .frame(height: 32)
                    }
                }
                .padding(4)
                .contentShape(Rectangle())
            }
            .buttonStyle(SqueezeStyle(scale: 0.98))
            .accessibilityLabel(combo.map { "Shortcut \($0.spoken). Click to change." } ?? "No shortcut. Click to set.")

            // The keys carry their names, so the lines under them say what to do: before, what
            // a click does; while listening, what to press, with the way out as keycaps.
            if listening {
                VStack(alignment: .trailing, spacing: 5) {
                    HStack(spacing: 4) {
                        Text("Now press your new keys, like").font(DS.Font.caption).foregroundStyle(DS.Hue.pinkText)
                        Keycap(key: "⌃"); Keycap(key: "⌥"); Keycap(key: "K")
                    }
                    HStack(spacing: 4) {
                        Keycap(key: "esc")
                        Text("keeps the old ones").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                        if allowsClear {
                            Text("·").font(DS.Font.caption).foregroundStyle(DS.Ink.i9)
                            Keycap(key: "⌫")
                            Text("turns it off").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                        }
                    }
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Now press your new keys. Escape keeps the old ones." + (allowsClear ? " Delete turns it off." : ""))
            } else {
                Text(combo == nil ? "Click to choose keys" : "Click to set your own keys")
                    .font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
            }

            if let problem {
                HStack(spacing: 8) {
                    Badge(text: problem.isBlocking ? "Can't use" : "Heads up", tone: problem.isBlocking ? .red : .warn)
                    Text(problem.message).font(DS.Font.caption).foregroundStyle(DS.Ink.i11)
                    if let pendingWarning, !problem.isBlocking {
                        TactileButton(title: "Use anyway", role: .ghost, compact: true) {
                            self.problem = nil
                            self.pendingWarning = nil
                            save(pendingWarning)
                        }
                    }
                }
            }
        }
        .onDisappear(perform: stop)
        // Clicking away while it listens ends the listening: the shortcuts are let go for it,
        // and would otherwise stay off until someone came back and pressed a key.
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in stop() }
    }

    private func toggle() { recording ? stop() : start() }

    private func start() {
        problem = nil
        pendingWarning = nil
        HotkeyCenter.shared.setPaused(true)
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            handle(event)
            return nil
        }
    }

    private func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
        HotkeyCenter.shared.setPaused(false)
    }

    private func handle(_ event: NSEvent) {
        let plain = event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty
        if plain && Int(event.keyCode) == kVK_Escape { return stop() }
        if plain && allowsClear && (Int(event.keyCode) == kVK_Delete || Int(event.keyCode) == kVK_ForwardDelete) {
            stop()
            onChange(nil)
            return
        }
        guard let newCombo = KeyCombo(event: event) else { return }
        if let found = ShortcutProblem.check(newCombo, other: other) {
            problem = found
            if !found.isBlocking { pendingWarning = newCombo; stop() }
            return  // blocking: keep listening so they can try again
        }
        stop()
        save(newCombo)
    }

    /// Saves keys another app doesn't hold (a warning that was waved through included), so a
    /// saved shortcut is never one that silently does nothing.
    private func save(_ newCombo: KeyCombo) {
        if newCombo != combo, !HotkeyCenter.shared.isAvailable(newCombo) {
            problem = .takenByAnotherApp
            return
        }
        onChange(newCombo)
    }
}
