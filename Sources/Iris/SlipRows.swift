import AppKit
import IrisEngine

/// The menu bar slip's rows for where Iris stands. The app builds them here each time the slip
/// opens, and `--snapshot` draws them from here, so the picture shows what people get.
enum SlipRows {
    struct Facts {
        var openShortcut: [String]
        var quickFixShortcut: [String]?
        /// Another app has the shortcut that opens the panel.
        var shortcutTaken = false
        var setupComplete = true
        /// The model is in memory, so its row frees it rather than loading it.
        var modelReady = false
    }

    /// What each row does. The snapshot passes nothing; its rows do nothing.
    struct Actions {
        var check: () -> Void = {}
        var quickFix: () -> Void = {}
        var changeShortcut: () -> Void = {}
        var setup: () -> Void = {}
        var settings: () -> Void = {}
        var checkForUpdates: () -> Void = {}
        var loadModel: () -> Void = {}
        var freeModel: () -> Void = {}
        var quit: () -> Void = {}
    }

    /// The slip's line under "Something's off" when another app holds the shortcut.
    static func shortcutTakenLine(_ shortcut: String) -> String {
        "Another app uses \(shortcut), so it does nothing in Iris. Change it here."
    }

    static func build(_ facts: Facts, _ act: Actions = Actions()) -> [[StatusMenu.Command]] {
        typealias C = StatusMenu.Command
        let settings = C(id: "settings", title: "Settings…", keys: ["⌘", ","], icon: .general, run: act.settings)
        let update = C(id: "update", title: "Check for Updates…", keepsOpen: true, icon: .updates, run: act.checkForUpdates)
        let quit = [C(id: "quit", title: "Quit Iris", keys: ["⌘", "Q"], run: act.quit)]
        // Setup not finished: the slip's header names the step, and the one way on is back to
        // it. No check rows (they'd only lead to setup) and no Load row (it could download).
        guard facts.setupComplete else {
            return [[C(id: "setup", title: "Finish setup…", icon: .tray, run: act.setup)], [settings, update], quit]
        }
        var check = [C(id: "check", title: "Check text", keys: facts.openShortcut, icon: .tones, run: act.check)]
        if facts.shortcutTaken {
            check.insert(C(id: "shortcut", title: "Change shortcut…", icon: .shortcuts, run: act.changeShortcut), at: 0)
        }
        if let quick = facts.quickFixShortcut {
            check.append(C(id: "quick", title: "Quick Fix", keys: quick, icon: .wand, run: act.quickFix))
        }
        // Only the model action that applies now, never a greyed-out one. One row id for
        // both, so the row stays dimmed while loading turns it into "Free model memory".
        let model = facts.modelReady
            ? C(id: StatusMenu.modelRow, title: EngineState.freeTitle, keepsOpen: true, icon: .model, run: act.freeModel)
            : C(id: StatusMenu.modelRow, title: EngineState.loadTitle, keepsOpen: true, icon: .model, run: act.loadModel)
        let app = [C(id: "setup", title: "Setup…", icon: .tray, run: act.setup), settings, update]
        return [check, [model], app, quit]
    }
}
