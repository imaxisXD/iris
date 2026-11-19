import AppKit
import IrisCore

/// `Iris --mock-answers --on-display <screen>`: this build of Iris, sealed on the test display
/// (`OnDisplay`), driven end to end by `scripts/verify` while the person keeps working and the
/// installed Iris keeps running. Canned answers (`MockAnswers`), no shortcuts registered (so
/// the two never fight over one), no menu bar flower.
///
/// The run clicks the panel's tabs and buttons, the slip's rows and Settings' controls itself,
/// through the accessibility tree, as a person's clicks reach them. What a person does with a
/// global key press it can't do in the background (the press goes to the app in front, which
/// is the person's), so it asks here instead, one line per command on stdin, and each command
/// enters where that key press's handler does:
///
///     open-panel <pid>   the open shortcut, reading the text box of the app with this pid
///                        (`AppDelegate.openPanel`, past its "which app has the keyboard" guess)
///     quick-fix <pid>    the Quick Fix shortcut, the same way
///     key <name>         a key in the open panel (tab, shift-tab, escape, return, cmd-r, 1…5),
///                        handed to the panel's own key handler (`PanelController.handle`)
///     slip               the menu bar flower clicked: the slip opens under a stand-in flower
///     settings <page>    Settings asked for at a page, the way setup's Change shortcut asks
///     engine <state>     the model's state as the slip and panel hear it (idle, loading, ready)
///     snapshot <path>    the open panel, slip or Settings drawn again by the snapshot renderer
///
/// Answers on stdout ("iris: ok <command>", "iris: error <why>"), with what happened as it
/// happens ("iris: panel open", "iris: replaced …"): the run waits on these lines, never on a sleep.
@MainActor
enum Driven {
    struct Hooks {
        let openPanel: (NSRunningApplication) -> Void
        let quickFix: (NSRunningApplication) -> Void
        let key: (String) -> Bool
        let slip: () -> Void
        let engine: (String) -> Bool
        let snapshot: (String) -> Bool
    }

    static func listen(_ hooks: Hooks) {
        let thread = Thread {
            while let line = readLine(strippingNewline: true) {
                let command = line
                DispatchQueue.main.async { MainActor.assumeIsolated { run(command, hooks) } }
            }
            // stdin closed: the run that started this Iris is gone.
            OnDisplay.quitNow(4, "the run's command line closed")
        }
        thread.start()
        OnDisplay.say("ready driven")
    }

    private static func run(_ line: String, _ hooks: Hooks) {
        let words = line.split(separator: " ", maxSplits: 1).map(String.init)
        guard let verb = words.first else { return }
        let arg = words.count > 1 ? words[1] : ""
        switch verb {
        case "open-panel", "quick-fix":
            guard let pid = pid_t(arg), let app = NSRunningApplication(processIdentifier: pid) else {
                return OnDisplay.say("error \(verb): no app with pid \(arg)")
            }
            target = pid
            OnDisplay.say("ok \(line)")
            verb == "open-panel" ? hooks.openPanel(app) : hooks.quickFix(app)
        case "key":
            OnDisplay.say(hooks.key(arg) ? "ok \(line)" : "error key \(arg): no panel to take it")
        case "settings":
            // Settings asked for at a page, as setup's Change shortcut asks (`irisOpenSettings`).
            NotificationCenter.default.post(name: .irisOpenSettings, object: arg.isEmpty ? nil : arg)
            OnDisplay.say("ok \(line)")
        case "slip":
            hooks.slip()
            OnDisplay.say("ok slip")
        case "engine":
            OnDisplay.say(hooks.engine(arg) ? "ok \(line)" : "error engine: unknown state \(arg)")
        case "snapshot":
            OnDisplay.say(hooks.snapshot(arg) ? "ok \(line)" : "error snapshot: nothing open to draw")
        case "quit":
            OnDisplay.quitNow(0, "asked to")
        default:
            OnDisplay.say("error unknown command \(verb)")
        }
    }

    /// The app the run asked to read: a capture from any other app is refused (`TextBridge`).
    static var target: pid_t?
}
