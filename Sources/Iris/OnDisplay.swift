import AppKit
import IrisCore

/// `--on-display <screen name>`: Iris opened on one named screen (the test display) for
/// `scripts/verify.sh`, which captures and drives its real windows there without touching
/// anyone's work. Sealed, so a check can never cause a problem of its own:
/// - it opens only on that screen, never the main one, and quits if the screen is missing;
/// - it never comes to the front, takes the keyboard, adds a menu bar flower or registers a
///   shortcut, so it never fights the installed Iris;
/// - it never touches the clipboard, never posts a key press, and only reads the text box of the
///   app it's told to (`Driven`), never whatever has focus on the person's screen;
/// - it never saves a setting, and reads none of this Mac's (every setting at its default);
/// - it quits the moment its screen starts to go away, before macOS can move its windows onto
///   the main screen; when the run that started it ends (`IRIS_VERIFY_PARENT`, a pid); and after
///   `IRIS_VERIFY_MAX_SECONDS`, so a stuck check never leaves an Iris behind.
/// Anything that would break one of these is refused and reported as a tripwire (`trip`), which
/// fails the check: the run says what happened instead of letting it reach the person's screen.
@MainActor
enum OnDisplay {
    nonisolated static let name: String? = LaunchMode.onDisplay(in: CommandLine.arguments)
    nonisolated static var isOn: Bool { name != nil }
    /// Settings at their defaults, and nothing saved (`AppSettings`).
    nonisolated static var sealed: Bool { isOn }

    /// The named screen, if it's connected.
    static var screen: NSScreen? {
        guard let name else { return nil }
        return NSScreen.screens.first { $0.localizedName == name }
    }

    /// Every setting at its default, and nothing written (see `AppSettings`).
    static let sealedDefaults = UserDefaults(suiteName: "local.iris.sealed")!

    /// Call before any window opens. Quits (status 3) when the named screen isn't there.
    static func start() {
        guard let name else { return }
        guard let screen else { quit(3, "no screen named “\(name)”: open it first (claude-display up)") }
        watched = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID ?? 0
        // Called before the displays change and again after: hide everything at once on the way
        // in, so nothing can land on the main screen, and quit if ours is gone.
        CGDisplayRegisterReconfigurationCallback({ display, flags, _ in
            MainActor.assumeIsolated {
                if flags.contains(.beginConfigurationFlag) {
                    OnDisplay.hideForReconfiguration()
                    return
                }
                guard display == OnDisplay.watchedDisplay else { return }
                if flags.contains(.removeFlag) || flags.contains(.disabledFlag) || flags.contains(.mirrorFlag) {
                    OnDisplay.quitNow(3, "its screen went away")
                }
            }
        }, nil)
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated {
                if OnDisplay.screen == nil { OnDisplay.quit(3, "its screen went away") }
                OnDisplay.showAfterReconfiguration()
            }
        }
        let env = ProcessInfo.processInfo.environment
        let parent = env["IRIS_VERIFY_PARENT"].flatMap { pid_t($0) }
        let deadline = env["IRIS_VERIFY_MAX_SECONDS"].flatMap(Double.init).map { Date().addingTimeInterval($0) }
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { _ in
            if let parent, kill(parent, 0) != 0 { OnDisplay.quitNow(4, "the run that started it ended") }
            if let deadline, Date() > deadline { OnDisplay.quitNow(5, "it ran past its time limit") }
        }
        watchForTrips()
        // Asked to stop (`scripts/verify`): a clean exit, so a coverage build writes what ran.
        signal(SIGTERM, SIG_IGN)
        let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
        term.setEventHandler { OnDisplay.quitNow(0, "asked to stop") }
        term.resume()
        stopSource = term
    }
    private static var stopSource: DispatchSourceSignal?

    // MARK: Displays changing

    /// The windows a reconfiguration took off screen, with their alpha, to put back if ours stays.
    private static var hidden: [(window: NSWindow, alpha: CGFloat)] = []

    /// Off screen entirely, not just transparent: a window ordered out isn't on any screen, so
    /// macOS has nothing to move onto the main one if the test display goes.
    private static func hideForReconfiguration() {
        say("displays changing: windows taken off screen")
        for window in NSApp.windows where window.isVisible && !hidden.contains(where: { $0.window === window }) {
            hidden.append((window, window.alphaValue))
            window.alphaValue = 0
            window.orderOut(nil)
        }
    }

    private static func showAfterReconfiguration() {
        guard !hidden.isEmpty else { return }
        for (window, alpha) in hidden {
            window.alphaValue = alpha
            window.orderFrontRegardless()
        }
        hidden = []
        say("displays changed: windows back on the test display")
    }

    // MARK: Tripwires

    /// Refuses, and reports, anything that would reach the person's screen, keyboard or
    /// clipboard: `scripts/verify` fails the step it was on.
    static func trip(_ what: String) {
        say("tripwire \(what)")
    }

    private static func watchForTrips() {
        let center = NotificationCenter.default
        center.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated {
                // cua-driver's background clicks reach Iris with an "activated" event while the
                // person's app stays in front with the keyboard: that's not Iris coming forward.
                // Only the front app actually being Iris is.
                let front = NSWorkspace.shared.frontmostApplication
                guard front?.processIdentifier == ProcessInfo.processInfo.processIdentifier else {
                    return OnDisplay.say("activated in the background by an accessibility action (front app still \(front?.localizedName ?? "?"))")
                }
                OnDisplay.trip("Iris came to the front")
                NSApp.hide(nil)
            }
        }
        center.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil, queue: .main) { note in
            MainActor.assumeIsolated {
                guard let window = note.object as? NSWindow else { return }
                OnDisplay.trip("a window took the keyboard: \(window.title.isEmpty ? String(describing: type(of: window)) : window.title)")
                window.resignKey()
            }
        }
        for name in [NSWindow.didMoveNotification, NSWindow.didChangeScreenNotification, NSWindow.didUpdateNotification] {
            center.addObserver(forName: name, object: nil, queue: .main) { note in
                MainActor.assumeIsolated {
                    guard let window = note.object as? NSWindow, window.isVisible, window.alphaValue > 0,
                          let ours = OnDisplay.screen, !ours.frame.intersects(window.frame) || window.screen != ours
                    else { return }
                    OnDisplay.trip("a window opened off the test display at \(NSStringFromRect(window.frame))")
                    window.orderOut(nil)
                }
            }
        }
    }

    // MARK: Telling the run

    /// One line on stdout for `scripts/verify`, which waits on these instead of sleeping.
    nonisolated static func say(_ line: String) {
        guard isOn else { return }
        FileHandle.standardOutput.write(Data("iris: \(line)\n".utf8))
    }

    /// Where a window opens: centred on the named screen.
    static func place(_ window: NSWindow) {
        guard let screen else { return }
        let frame = screen.visibleFrame
        window.setFrameOrigin(NSPoint(x: frame.midX - window.frame.width / 2, y: frame.midY - window.frame.height / 2))
    }

    /// Where `scripts/verify.sh` wants the snapshot renderer's picture of what a preview shows
    /// (`IRIS_VERIFY_SNAPSHOT`), to set beside its capture of the live window.
    static let snapshotPath = ProcessInfo.processInfo.environment["IRIS_VERIFY_SNAPSHOT"]

    /// Settings and setup: once the window has drawn, says it's ready and draws its content
    /// again the way `--snapshot` draws (`ComponentSnapshot.render`'s cacheDisplay), at the same size.
    static func snapshotWhenSettled(_ window: NSWindow, as name: String = "window", drawing: Bool = true) {
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(1.2))
            if drawing, let path = snapshotPath { render(window, to: path) }
            say("ready \(name)")
        }
    }

    static func render(_ window: NSWindow, to path: String) {
        guard let view = window.contentView, let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
        NSAppearance(named: .aqua)!.performAsCurrentDrawingAppearance { view.cacheDisplay(in: view.bounds, to: rep) }
        if let png = rep.representation(using: .png, properties: [:]) { ComponentSnapshot.write(png, to: path) }
    }

    nonisolated(unsafe) private static var watched: CGDirectDisplayID = 0
    nonisolated static var watchedDisplay: CGDirectDisplayID { watched }

    static func quit(_ status: Int32, _ why: String) -> Never { quitNow(status, why) }

    nonisolated static func quitNow(_ status: Int32, _ why: String) -> Never {
        say("quit \(status) \(why)")
        FileHandle.standardError.write(Data("Iris --on-display: \(why)\n".utf8))
        // A coverage run (`scripts/verify` learning which code each journey runs): a full exit,
        // so the profiling runtime writes what ran. Otherwise at once, nothing else to wait for.
        if ProcessInfo.processInfo.environment["LLVM_PROFILE_FILE"] != nil { exit(status) }
        _exit(status)
    }
}

