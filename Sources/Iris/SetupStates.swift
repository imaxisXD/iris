import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// Design check: every state setup can be in, with made-up facts so nothing touches the
/// permission or the model.
///   Iris --setup-states <dir>       writes each state as a PNG, the whole page and the window
///   Iris --setup-preview <state>    opens the real setup window in that state (no menu bar
///                                   flower, no shortcuts), to see and click it live
@MainActor
enum SetupStates {
    typealias Facts = SetupProgress.Facts
    static let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge"

    static let states: [(id: String, frozen: OnboardingModel.Frozen)] = [
        ("1-allow", .init(facts: Facts(trusted: false, modelOnDisk: false))),
        ("1-allow-stuck", .init(facts: Facts(trusted: false, modelOnDisk: false, backFromSettingsUntrusted: true))),
        ("1-allow-returning", .init(facts: Facts(trusted: false, modelOnDisk: true, finishedBefore: true))),
        ("2-model", .init(facts: Facts(trusted: true, modelOnDisk: false))),
        ("2-downloading", .init(facts: Facts(trusted: true, modelOnDisk: false, activity: .downloading(0.37)))),
        ("2-loading", .init(facts: Facts(trusted: true, modelOnDisk: false, activity: .loading))),
        ("2-no-room", .init(facts: Facts(trusted: true, modelOnDisk: false,
                                         activity: .failed(.noRoom(needed: "6.2 GB", free: "3.1 GB"))))),
        ("2-offline", .init(facts: Facts(trusted: true, modelOnDisk: false, activity: .failed(.offline)))),
        ("2-returning", .init(facts: Facts(trusted: true, modelOnDisk: false, finishedBefore: true))),
        ("3-try", .init(facts: Facts(trusted: true, modelOnDisk: true, activity: .ready))),
        ("3-try-shortcut-taken", .init(facts: Facts(trusted: true, modelOnDisk: true, activity: .ready), shortcutTaken: true)),
        ("done", .init(facts: Facts(trusted: true, modelOnDisk: true, activity: .ready, tried: true), practiceText: fixed)),
    ]

    static func render(into dir: String) {
        FontRegistry.registerAll()
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        for state in states {
            ComponentSnapshot.hosted(state.id, width: OnboardingView.width, dir: dir, prefix: "setup") {
                AnyView(OnboardingView(frozen: state.frozen, scrolls: false, onFinish: {}))
            }
            ComponentSnapshot.hosted(state.id + "-window", width: OnboardingView.width, dir: dir, prefix: "setup") {
                AnyView(OnboardingView(frozen: state.frozen, scrolls: false, onFinish: {}).frame(height: OnboardingView.height, alignment: .top).clipped())
            }
        }
    }

    /// The real window, as the app makes it, in a made-up state.
    static func preview(_ id: String) {
        guard let state = states.first(where: { $0.id == id }) else {
            print("states: " + states.map(\.id).joined(separator: ", "))
            exit(1)
        }
        let window = PaperWindow.make(title: "Set up Iris", content: SheetHost {
            OnboardingView(frozen: state.frozen, onFinish: { NSApp.terminate(nil) }, onLater: { NSApp.terminate(nil) })
        })
        keep = window
        // On the test display (`OnDisplay`): there, never in front of anyone's work.
        if OnDisplay.isOn {
            OnDisplay.place(window)
            window.orderFrontRegardless()
            return OnDisplay.snapshotWhenSettled(window)
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }
    private static var keep: NSWindow?
}
