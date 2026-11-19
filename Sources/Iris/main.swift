import AppKit
import Combine
import IrisCore
import IrisEngine
import SwiftUI

/// Menu-bar app: no Dock icon, two global shortcuts, one floating panel.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let panel = PanelController()
    private let settings = AppSettings.shared
    private var onboardingWindow: NSWindow?
    /// Lets setup's window go when it closes (see `showOnboarding`).
    private var onboardingClosing: NSObjectProtocol?
    private var settingsWindow: NSWindow?
    /// The update check that answers: a newer one (from Settings or the slip) replaces it.
    private var updateCheckID: UUID?
    private let statusMenu = StatusMenu()
    /// Feeds the model's state to the slip.
    private var engineWatch: AnyCancellable?
    /// A Quick Fix under way; the shortcut does nothing until it ends.
    private var quickFixTask: Task<Void, Never>?
    /// A download ran since the model last became ready, so its arrival is news.
    private var downloadedThisRun = false

    /// Whether the slip shows the model as loaded, which decides Load or Free in its model row.
    /// Read from the slip's own state so the design preview's made-up states drive it too.
    private var modelReady: Bool { statusMenu.model.engine.isReady }

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Iris is a paper app: light only, whatever the system appearance.
        NSApp.appearance = NSAppearance(named: .aqua)
        FontRegistry.registerAll()
        // A preview on the test display quits here if that screen isn't there (`OnDisplay`).
        OnDisplay.start()
        MousePresses.start()
        // Each mode gets as much of the real launch as it needs, and no more: the design
        // previews never register shortcuts or note an update as finished.
        switch LaunchMode.current {
        case .setupPreview(let state):
            return SetupStates.preview(state)
        case .toastWorkbench(let start):
            return ToastStates.openWindow(start: start)
        case .settingsPreview(let page):
            // The real window, as `openSettings` makes it, but never brought to the front.
            let window = PaperWindow.make(title: "Iris Settings", content: SheetHost { SettingsView() }, remembers: "settings")
            settingsWindow = window
            window.orderFrontRegardless()
            // Never asks GitHub: Check for updates answers with made-up news, a different one
            // each click (up to date; couldn't check, after 4 s; new changes; a new release),
            // so Updates can be watched end to end.
            UpdateCheck.shared.start = UpdateCheck.madeUpChecks([
                .upToDate, .failed("Couldn't reach GitHub. Check your connection and try again."),
                .behind(2, ["The menu bar flower in pen", "Names each fix in the margin"]),
                .newRelease("v0.2.4", page: Updater.releases),
            ], slowLooks: [1])
            if let page {
                DispatchQueue.main.async { NotificationCenter.default.post(name: .irisTurnSettingsPage, object: page) }
            }
            return
        case .panelPreview:
            guard let screen = NSScreen.main else { return }
            let visible = screen.visibleFrame
            return panel.previewLoop(field: CGRect(x: visible.midX - 280, y: visible.minY + 140, width: 560, height: 64))
        case .statusMenuPreview(let row):
            setUpStatusItem()
            watchEngine()  // like the real app, so loading notes show live
            return previewStatusMenu(row)
        case .selfCheck:
            setUpStatusItem()
            listenForWindowRequests()
            let registered = registerShortcuts()
            return SelfCheck.run(.init(panel: panel, statusMenu: statusMenu, statusButton: statusItem.button,
                                       shortcut: settings.openShortcut, shortcutRegistered: registered))
        case .render, .practiceBox:
            return  // handled before the app starts (see the end of this file)
        case .app, .mockAnswers:
            if OnDisplay.isOn { return startDriven() }
        }
        Updater.noteLaunch()
        setUpStatusItem()
        let shortcutRegistered = registerShortcuts()
        if !shortcutRegistered {
            // Said under the menu bar flower, where Iris lives, and kept in the slip after; a moment
            // after launch, once the status item has a place in the menu bar to hang it from.
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(1))
                showMenuBarHint(lead: "shortcut taken",
                                detail: "Another app uses \(settings.openShortcut.display). Open me to change it.")
            }
        }

        listenForWindowRequests()

        watchEngine()
        Task {
            // The saved size reaches the engine before anything loads, or a prewarm started
            // first loaded (and could download) the size this Mac would get by default.
            await settings.applyToEngine()
            if !setup.isComplete {
                showOnboarding()
            } else if settings.loadAtLaunch {
                // Load and prefill Fix in the background so the first check is fast. Never a
                // download: the model is on disk, or setup would be open.
                prewarm(modeID: settings.startingMode(for: nil))
            }
        }
    }

    /// Design check, `Iris --preview-status-menu [row]`: opens the slip from the real menu bar
    /// (over whatever is on screen) and keeps it open, so it can be screenshotted. `row`
    /// highlights a row, or plays a scene with made-up states (nothing loads or installs):
    /// "model:load" / "model:free" the model waking or being freed, "check:up" / "check:new" /
    /// "check:slow" the Updates page (slow: GitHub takes 4 s), "updating" an update's
    /// progress sped up, "act:<id>" that row's command.
    private func previewStatusMenu(_ row: String?) {
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(1))
            // Under a stand-in flower at the top centre of the main screen, so it shows
            // even when the menu bar is hidden by a full-screen app. IRIS_PREVIEW_SCREEN names
            // another screen to open on (the test display), to watch it without covering anyone's work.
            let named = ProcessInfo.processInfo.environment["IRIS_PREVIEW_SCREEN"]
            guard let screen = NSScreen.screens.first(where: { $0.localizedName == named }) ?? NSScreen.main else { return }
            let anchor = CGRect(x: screen.frame.midX - 12, y: screen.frame.maxY - 24, width: 24, height: 24)
            statusMenu.show(under: anchor, on: screen)
            try? await Task.sleep(for: .seconds(0.6))
            switch row {
            case "model:load", "model:free":
                let ready = EngineState.ready(settings.model)
                engineChanged(row == "model:load" ? .idle : ready)
                try? await Task.sleep(for: .seconds(1.2))
                statusMenu.model.busy.insert(StatusMenu.modelRow)
                if row == "model:load" {
                    engineChanged(.loading)
                    try? await Task.sleep(for: .seconds(3))
                    engineChanged(ready)
                } else {
                    try? await Task.sleep(for: .seconds(0.3))
                    engineChanged(.idle)
                }
            case let row? where row.hasPrefix("check:"):
                let kind = row.dropFirst(6)
                runUpdateCheck {
                    try? await Task.sleep(for: .seconds(kind == "slow" ? 4 : 0.2))
                    return kind == "new" ? .behind(2, ["The menu bar flower in pen", "Names each fix in the margin"]) : .upToDate
                }
            case "updating":
                trackUpdate(pace: 4)
            case let row? where row.hasPrefix("act:"):
                statusCommands().flatMap { $0 }.first { $0.id == row.dropFirst(4) }?.run()
            default:
                statusMenu.model.highlighted = row
            }
        }
    }

    /// Setup and Settings ask for each other (Change shortcut, Open setup).
    private func listenForWindowRequests() {
        NotificationCenter.default.addObserver(forName: .irisShowOnboarding, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.showOnboarding() }
        }
        NotificationCenter.default.addObserver(forName: .irisOpenSettings, object: nil, queue: .main) { [weak self] note in
            let page = note.object as? String
            MainActor.assumeIsolated { self?.openSettings(page: page) }
        }
        // Settings › Updates checks and installs the way the slip does: one path, one state.
        UpdateCheck.shared.start = { [weak self] in self?.checkForUpdates() }
        UpdateCheck.shared.install = { [weak self] in self?.installUpdate() }
    }

    /// Both shortcuts, as Settings has them. Returns whether the shortcut that opens the panel
    /// is Iris's (another app may have it).
    private func registerShortcuts() -> Bool {
        HotkeyCenter.shared.handler = { [weak self] action in
            switch action {
            case .open: self?.openPanel()
            case .quickFix: self?.quickFix()
            }
        }
        HotkeyCenter.shared.apply(settings)
        return !settings.openShortcutTaken
    }

    // MARK: Setup

    /// Where setup stands. Until it's complete, the shortcut, Quick Fix and the slip lead back
    /// to setup's current step: nothing checks text or downloads a model on its own.
    private var setup: SetupProgress { SetupProgress(.now()) }
    private func setup(engine: EngineState) -> SetupProgress { SetupProgress(.now(engine: engine)) }

    // MARK: Shortcuts

    /// Starts loading the model and prefilling this mode's prompt right away, in parallel
    /// with reading the text box. By the time the text arrives most of the work is done.
    private func prewarm(modeID: String) {
        let mode = settings.modes.first { $0.id == modeID } ?? .fix
        let checker = CheckSetup.current.checker
        Task.detached(priority: .userInitiated) { await checker.prewarm(mode: mode) }
    }

    /// `target`: the app whose text box to read, given by `scripts/verify` (`Driven`) in place of
    /// "the app with the keyboard"; everything after that guess is the shortcut's own path.
    private func openPanel(target: NSRunningApplication? = nil) {
        if panel.isShowing { return panel.close() }
        // Not set up yet: setup at its current step, never a panel that downloads.
        guard setup.canCheck else { return showOnboarding() }
        // A quick guess at the app, so prewarming needn't wait for Accessibility; the menu bar's
        // owner is right more often than the frontmost app (see `TextBridge.focusedApp`).
        let guess = NSWorkspace.shared.menuBarOwningApplication ?? NSWorkspace.shared.frontmostApplication
        // Not in an app Iris is turned off in: nothing will be checked, so nothing loads.
        if !settings.isExcluded(guess?.bundleIdentifier) {
            prewarm(modeID: settings.startingMode(for: guess?.bundleIdentifier))
        }
        // Pressed again while the last panel folds away: that panel still has the keyboard, so
        // end the fold and let the app underneath have it back before reading its text box.
        let wasFolding = panel.finishFolding()
        Task {
            if wasFolding, target == nil { await focusLeavesIris() }
            guard let captured = await captureOrExplain(target: target) else { return }
            panel.show(for: captured, startingMode: settings.startingMode(for: captured.app.bundleIdentifier))
        }
    }

    /// Waits (up to about 0.3 s) until the keyboard is in another app than Iris.
    private func focusLeavesIris() async {
        let me = ProcessInfo.processInfo.processIdentifier
        for _ in 0..<15 {
            if await TextBridge.focusedApp()?.processIdentifier != me { return }
            try? await Task.sleep(for: .milliseconds(20))
        }
    }

    /// Fix and replace in one step. Opens the panel only when a human should look:
    /// a big rewrite, or a result that couldn't keep @mentions/links intact.
    private func quickFix(target: NSRunningApplication? = nil) {
        guard quickFixTask == nil else { return }
        guard setup.canCheck else { return showOnboarding() }
        // As the panel does: in an app Iris is turned off in nothing is checked, so nothing loads.
        let guess = NSWorkspace.shared.menuBarOwningApplication ?? NSWorkspace.shared.frontmostApplication
        if !settings.isExcluded(guess?.bundleIdentifier) { prewarm(modeID: Mode.fix.id) }
        let pressed = Date()
        quickFixTask = Task {
            defer { quickFixTask = nil }
            guard let captured = await captureOrExplain(target: target) else { return }
            Toast.shared.show(.checking, near: captured.fieldFrame)
            do {
                let result = try await CheckSetup.current.checker.check(captured.text, mode: .fix)
                // Pastes only into the same box, still holding the text Iris read: a slow
                // check must never select-all and paste over what you've moved on to.
                let landing = QuickFixLanding.decide(
                    unchanged: result.verdict == .unchanged, heavy: result.verdict == .heavy,
                    canReplace: captured.canReplace, translated: result.translated,
                    stillThere: await TextBridge.stillHolds(captured, since: pressed))
                SelfCheck.note("quick fix landing \(landing)")
                switch landing {
                case .clean:
                    Toast.shared.show(.looksGood, near: captured.fieldFrame)
                case .panel:
                    // A big rewrite, a translation, or text that can only be copied: show it.
                    Toast.shared.dismiss()
                    panel.show(for: captured, startingMode: Mode.fix.id, answer: result)
                case .leftAlone:
                    Toast.shared.show(.leftAlone, near: captured.fieldFrame)
                case .paste:
                    await Paste.replace(captured, original: captured.text, with: result.text,
                                        saying: .fixed(result.changedWords))
                }
            } catch {
                // Nothing cancels a Quick Fix, so a cancellation here came from the model (its
                // load was replaced or it was deleted meanwhile): a problem like any other.
                // Another try can work: show it in the panel, which has Try again, instead of a
                // message that only says what went wrong.
                if QuickFixLanding.failureOpensPanel(checkFailed: error is RewriteError,
                                                     canRetry: ProblemNote(error, draft: captured.text).canRetry) {
                    Toast.shared.dismiss()
                    panel.show(for: captured, startingMode: Mode.fix.id)
                } else {
                    Toast.shared.show(.problem(error, draft: captured.text), near: captured.fieldFrame)
                }
            }
        }
    }

    /// One read at a time: a second press while the first is still reading (a browser's first
    /// read can take seconds) would have both borrowing the clipboard at once.
    private var capturing = false

    private func captureOrExplain(target: NSRunningApplication? = nil) async -> TextBridge.Captured? {
        guard !capturing else { return nil }
        capturing = true
        defer { capturing = false }
        // On the test display: only ever the app the run named (the slip's Check text too).
        let named = Driven.target.flatMap { NSRunningApplication(processIdentifier: $0) }
        let app: NSRunningApplication?
        if let target { app = target } else if OnDisplay.isOn { app = named } else { app = await TextBridge.focusedApp() }
        SelfCheck.note("capture from \(app?.localizedName ?? "nil") pid \(app?.processIdentifier ?? 0)")
        if settings.isExcluded(app?.bundleIdentifier) {
            explainTurnedOff(app)
            return nil
        }
        do {
            let captured = try await TextBridge.capture(from: app)
            // The text box can belong to another app than the one asked (see `capture`).
            if settings.isExcluded(captured.app.bundleIdentifier) {
                explainTurnedOff(captured.app, near: captured.fieldFrame)
                return nil
            }
            return captured
        } catch TextBridge.CaptureError.notTrusted {
            showOnboarding()
        } catch {
            OnDisplay.say("capture failed: \(error.localizedDescription)")
            SelfCheck.note("capture failed: \(error)")
            NSSound.beep()
            Toast.shared.show(.problem(error))
        }
        return nil
    }

    /// Says Iris is off in this app, on the text box like every other note when Iris knows
    /// where the box is.
    private func explainTurnedOff(_ app: NSRunningApplication?, near box: CGRect? = nil) {
        Toast.shared.show(.turnedOff(in: app), near: box)
    }

    // MARK: Menu bar

    private func setUpStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.image = MenuBarIcon.flower
        statusItem.button?.setAccessibilityLabel("Iris")
        statusItem.button?.setAccessibilityHelp("Grammar and tone. Open the menu for Check text, Quick Fix and Settings.")

        statusItem.button?.target = self
        statusItem.button?.action = #selector(toggleStatusMenu)
        statusMenu.commands = { [weak self] in self?.statusCommands() ?? [] }
    }

    @objc private func toggleStatusMenu() {
        guard let button = statusItem.button else { return }
        // A second click closes it, like any menu bar menu. That click usually closes the slip
        // on the press already (it loses focus), so don't reopen it on the release.
        MenuBarHint.shared.close()
        SelfCheck.note("flower clicked, slip open \(statusMenu.isOpen), event \(NSApp.currentEvent.map { "\($0.type.rawValue) @\($0.timestamp)" } ?? "none"), press began \(MousePresses.pressOfThisClick.map { "\($0)" } ?? "nil")")
        if statusMenu.isOpen {
            statusMenu.close()
        } else if !statusMenu.takeClosedByThisClick() {
            statusMenu.toggle(below: button)
        }
    }

    /// The slip's commands, rebuilt each time it opens so shortcuts and model state are current.
    private func statusCommands() -> [[StatusMenu.Command]] {
        refreshSlipHeader()
        let facts = SlipRows.Facts(openShortcut: settings.openShortcut.symbols, quickFixShortcut: settings.quickFixShortcut?.symbols,
                                   shortcutTaken: settings.openShortcutTaken, setupComplete: setup.isComplete,
                                   modelReady: modelReady)
        return SlipRows.build(facts, .init(
            check: { [weak self] in self?.fromMenu { $0.openPanel() } },
            quickFix: { [weak self] in self?.fromMenu { $0.quickFix() } },
            changeShortcut: { [weak self] in self?.openSettings(page: SettingsView.Page.shortcuts.rawValue) },
            setup: { [weak self] in self?.showOnboarding() },
            settings: { [weak self] in self?.openSettings() },
            checkForUpdates: { [weak self] in self?.checkForUpdates() },
            loadModel: { [weak self] in self?.loadModel() },
            freeModel: { [weak self] in self?.unloadModel() },
            quit: { NSApp.terminate(nil) }))
    }

    /// What the slip's header says besides the model: which step of setup, or what else stops
    /// Iris. Kept current as the model changes, not only when the slip opens.
    private func refreshSlipHeader(engine: EngineState? = nil) {
        let progress = engine.map(setup(engine:)) ?? setup
        statusMenu.model.setupLine = progress.slipLine
        statusMenu.model.problemLine = settings.openShortcutTaken
            ? SlipRows.shortcutTakenLine(settings.openShortcut.display) : nil
        // Until setup is done the slip's garden tells setup's story, as setup's window does.
        statusMenu.model.setupGarden = progress.isComplete ? nil : progress.garden
    }

    private func watchEngine() {
        engineWatch = EngineMonitor.shared.$state.sink { [weak self] state in self?.engineChanged(state) }
    }

    private func engineChanged(_ state: EngineState) {
        // The model came in while setup's window was closed: say so from the menu bar flower
        // instead of opening a window that would take the keyboard from whatever you're in.
        if case .downloading = state { downloadedThisRun = true }
        if state.isReady, downloadedThisRun, onboardingWindow?.isVisible != true, !setup(engine: state).isComplete {
            downloadedThisRun = false
            showMenuBarHint(lead: "model's in", detail: "Open the menu bar flower to finish setup.")
        }
        statusMenu.model.engine = state
        refreshSlipHeader(engine: state)
        statusMenu.refresh()
        // The header and its garden show the model's state; the model row only dims while
        // its action runs, and comes back (as Load or Free) once the state settles.
        switch state {
        case .downloading, .loading: statusMenu.model.busy.insert(StatusMenu.modelRow)
        default: statusMenu.model.busy.remove(StatusMenu.modelRow)
        }
    }

    /// A check picked in the slip: once the slip has closed and the app underneath has the
    /// keyboard back, so Iris reads that app's text box.
    private func fromMenu(_ action: @escaping (AppDelegate) -> Void) {
        Task { [weak self] in
            await self?.focusLeavesIris()
            if let self { action(self) }
        }
    }

    /// The slip stays open: its header turns to Waking up and the garden's sun rises, then
    /// Ready, in bloom with butterflies.
    private func loadModel() {
        statusMenu.model.busy.insert(StatusMenu.modelRow)
        prewarm(modeID: settings.startingMode(for: nil))
    }

    /// The header goes back to Resting: the butterflies fly off, the sun sets, the bloom closes.
    private func unloadModel() {
        statusMenu.model.busy.insert(StatusMenu.modelRow)
        Task { await Engine.shared.unload() }
    }

    /// The slip turns to an Updates page: a bud sways and a butterfly loops round it while
    /// Iris looks on GitHub. The loop always plays at least once; the answer waits for it to
    /// come round, then the ending plays (the bud opens, the butterfly lands) under the
    /// answer: up to date, what's new (with Update & reopen), or what went wrong. No toast.
    /// If a new Iris is already building, there's nothing to offer: the header goes straight
    /// to Updating and follows that build instead.
    private func checkForUpdates() {
        // A check on the test display never asks GitHub: it's always up to date.
        if OnDisplay.isOn { return runUpdateCheck { .upToDate } }
        if Updater.runningUpdateStarted != nil { return followRunningUpdate() }
        runUpdateCheck { await Updater.check() }
    }

    private func runUpdateCheck(_ check: @escaping () async -> Updater.Status) {
        let menu = statusMenu.model
        let updates = UpdateCheck.shared
        guard menu.updating == nil else { return }
        let started = Date()
        let id = UUID()
        func scene(_ answer: UpdateCheckScene.Answer? = nil) -> AnyView {
            AnyView(UpdateCheckScene(started: started, answer: answer))
        }
        // Asked from Settings with the slip closed: Settings shows the answer, and the slip
        // isn't left turned to a page nobody asked it for.
        if statusMenu.isOpen {
            withAnimation(DS.easeOut) {
                menu.showing = .page(StatusMenu.SlipPage(id: id, title: "Updates", lead: "Looking for a new Iris…", art: scene()))
            }
        }
        updates.looking(since: started)
        updateCheckID = id
        Task {
            let status = await check()
            try? await Task.sleep(for: .seconds(UpdateCheckScene.wait(after: Date().timeIntervalSince(started))))
            // A newer check started meanwhile (from Settings or the slip): it answers instead.
            guard updateCheckID == id else { return }
            let at = Date()
            // Settings › Updates shows the same answer, in the same words.
            updates.answered(status, started: started, at: at)
            let words = UpdateCheck.Words(status)
            FloatingNote.announce([words.title, words.lead].joined(separator: ". "))
            // Turned back while it looked: leave the slip alone.
            guard menu.page?.id == id else { return }
            // The page changes in place, so the scene carries straight on into its ending.
            @MainActor func answer(_ content: (some View)? = Optional<EmptyView>.none) {
                menu.showing = .page(StatusMenu.SlipPage(id: id, title: words.title, lead: words.lead, warning: words.warning,
                                                         art: scene(.init(outcome: words.outcome, at: at)),
                                                         content: content.map { c in AnyView(AfterAnswer(at: at, delay: UpdateCheckScene.Beat.button) { EmptyView() } after: { c }) },
                                                         answeredAt: at, wordsWhileLooking: ("Updates", "Looking for a new Iris…")))
            }
            switch status {
            case .upToDate:
                answer()
                // Long enough to read the letter after it opens, then the page turns back.
                try? await Task.sleep(for: .seconds(UpdateCheckScene.ending + 2))
                if menu.page?.id == id { withAnimation(DS.easeOut) { menu.showing = .commands } }
            case .behind(_, let changes):
                answer(UpdateChanges(changes: changes) { [weak self] in self?.installUpdate() })
            case .newRelease(_, let page):
                answer(TactileButton(title: "Download", role: .primary, compact: true) { NSWorkspace.shared.open(page) })
            case .noCheckout, .noRemote:
                answer()
            case .failed:
                answer(TactileButton(title: "Try again", role: .secondary, compact: true) { [weak self] in self?.checkForUpdates() })
            }
        }
    }

    /// Update & reopen: the slip turns back, its header says Updating and the garden grows in
    /// the rain while the new Iris builds. The script quits and reopens Iris when it's ready.
    private func installUpdate() {
        if OnDisplay.isOn { return OnDisplay.trip("an update would have been installed") }
        guard let offset = Updater.install() else { return updateFailed("The update couldn't start.") }
        let began = Date()
        trackUpdate { [weak self] in
            // A script that died without a word (killed, the Mac slept through it) never logs
            // a failure: past the age update.sh itself calls a run dead, stop waiting.
            let failure = Updater.failure(since: offset)
                ?? (Date().timeIntervalSince(began) > Updater.givesUpAfter ? UpdateRun.Failure.stopped : nil)
            return failure.map { failure in
                // Another one got there first (Update clicked while a build from a terminal
                // was running): this one stopped, so follow that one instead of saying it failed.
                if failure == .lockedOut, Updater.runningUpdateStarted != nil {
                    self?.followRunningUpdate()
                } else {
                    self?.updateFailed(failure.message)
                }
                return failure.message
            }
        }
    }

    /// Shows a build already under way as Updating, timed from when it began. A build that
    /// succeeds quits this Iris and opens the new one. If it ends and this Iris is still
    /// here, it didn't replace anything, so look again: that answers up to date or offers
    /// the update.
    private func followRunningUpdate() {
        guard let started = Updater.runningUpdateStarted else { return checkForUpdates() }
        let menu = statusMenu.model
        trackUpdate(since: started) { [weak self] in
            guard Updater.runningUpdateStarted == nil else { return nil }
            withAnimation(DS.easeOut) { menu.showing = .commands }
            self?.checkForUpdates()
            return "ended"
        }
    }

    /// Moves the update's progress along until Iris is replaced, or `failed` reports a problem.
    /// `failed` runs about once a second; `pace` speeds it up for the design preview.
    private func trackUpdate(since start: Date = Date(), pace: Double = 1, failed: @escaping () -> String? = { nil }) {
        let menu = statusMenu.model
        withAnimation(DS.easeOut) {
            menu.showing = .updating(Updater.progress(after: Date().timeIntervalSince(start) * pace))
        }
        UpdateCheck.shared.updating(Updater.progress(after: Date().timeIntervalSince(start) * pace))
        Task { @MainActor in
            var tick = 0
            while menu.updating != nil {
                try? await Task.sleep(for: .milliseconds(100))
                guard menu.updating != nil else { return }
                menu.showing = .updating(Updater.progress(after: Date().timeIntervalSince(start) * pace))
                UpdateCheck.shared.updating(Updater.progress(after: Date().timeIntervalSince(start) * pace))
                tick += 1
                if tick % 10 == 0, failed() != nil { return }
            }
        }
    }

    private func updateFailed(_ message: String) {
        let menu = statusMenu.model
        Updater.forgetStart()
        UpdateCheck.shared.installFailed(message)
        withAnimation(DS.easeOut) {
            menu.showing = .page(StatusMenu.SlipPage(id: UUID(), title: "Update failed", warning: true, content: AnyView(
                UpdateTrouble(message: message + " Iris is unchanged. The log is in \(Updater.logPath.replacingOccurrences(of: NSHomeDirectory(), with: "~")).") { [weak self] in
                    self?.checkForUpdates()
                })))
        }
    }

    /// Opens Settings, at `page` when one is named ("shortcuts").
    private func openSettings(page: String? = nil) {
        if settingsWindow == nil {
            // On the test display General's note demos hold still at the moment the snapshots
            // draw them, so a capture is of one moment (`OnDisplay`).
            settingsWindow = PaperWindow.make(title: "Iris Settings",
                                              content: SheetHost { SettingsView().environment(\.designState, OnDisplay.isOn ? .snapshot : nil) },
                                              remembers: OnDisplay.isOn ? nil : "settings")
        }
        present(settingsWindow!)
        if let page {
            // After the window has drawn, so its page can follow.
            DispatchQueue.main.async { NotificationCenter.default.post(name: .irisTurnSettingsPage, object: page) }
        }
    }

    private func showOnboarding() {
        if onboardingWindow == nil {
            let finish: () -> Void = { [weak self] in
                guard let self else { return }
                self.onboardingWindow?.close()
                // Iris has no Dock icon or window, so say where it went: a note hanging from
                // the menu bar flower by its tail, like the slip.
                self.showMenuBarHint()
            }
            // Setup stays unfinished; the slip keeps saying where it stopped.
            let later: () -> Void = { [weak self] in self?.onboardingWindow?.close() }
            let window = PaperWindow.make(title: "Set up Iris", content: SheetHost { OnboardingView(onFinish: finish, onLater: later) })
            onboardingWindow = window
            // Closed (finished, Set up later, its close button): let the window go, and setup's
            // view and model with it, so nothing keeps watching for permission or waking the
            // model. A window that is only closed keeps its view, and SwiftUI never tells it it
            // went (no onDisappear). The next open starts afresh.
            onboardingClosing = NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if let closing = self.onboardingClosing { NotificationCenter.default.removeObserver(closing) }
                    self.onboardingClosing = nil
                    self.onboardingWindow = nil
                }
            }
        }
        present(onboardingWindow!)
    }

    /// A note hanging from the menu bar flower: "find me up here" with the keys, or `lead`.
    private func showMenuBarHint(lead: String? = nil, detail: String? = nil) {
        guard let item = statusItem, let button = item.button, let window = button.window else { return }
        MenuBarHint.shared.show(under: window.convertToScreen(button.convert(button.bounds, to: nil)), on: window.screen,
                                keys: lead == nil ? settings.openShortcut.symbols : [], lead: lead, detail: detail)
    }

    private func present(_ window: NSWindow) {
        // On the test display: there, and never in front of anyone's work.
        if OnDisplay.isOn {
            if window.screen != OnDisplay.screen { OnDisplay.place(window) }
            window.orderFrontRegardless()
            return OnDisplay.snapshotWhenSettled(window, as: window.title, drawing: false)
        }
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }

    /// `Iris --mock-answers --on-display <screen>`: this build sealed on the test display, for
    /// `scripts/verify` to drive end to end (see `Driven`). No shortcuts (the installed Iris keeps
    /// its own), no menu bar flower, no update bookkeeping; the slip and the panel hear the
    /// model's state as the app does, and the run can set it.
    private func startDriven() {
        BudShow.forced = false  // nothing saved about the bud's arrivals
        // A Mac that has finished setup, as a person's is (in memory only: nothing is saved).
        settings.onboarded = true
        statusMenu.commands = { [weak self] in self?.statusCommands() ?? [] }
        watchEngine()
        listenForWindowRequests()
        Driven.listen(.init(
            openPanel: { [weak self] in self?.openPanel(target: $0) },
            quickFix: { [weak self] in self?.quickFix(target: $0) },
            key: { [weak self] in self?.panel.press($0) ?? false },
            slip: { [weak self] in
                guard let self, let screen = OnDisplay.screen else { return }
                if self.statusMenu.isOpen { return self.statusMenu.close() }
                let anchor = CGRect(x: screen.frame.midX - 12, y: screen.frame.maxY - 24, width: 24, height: 24)
                self.statusMenu.show(under: anchor, on: screen)
                // Said once its first frames have drawn in (measured: it changes once in the
                // first half second, then holds still), as the panel says when it has settled.
                Task { @MainActor in
                    try? await Task.sleep(for: .seconds(0.8))
                    OnDisplay.say("slip open window \(self.statusMenu.windowNumber ?? 0)")
                }
            },
            engine: { [weak self] state in
                guard let self else { return false }
                switch state {
                case "idle": self.engineChanged(.idle)
                case "loading": self.engineChanged(.loading)
                case "ready": self.engineChanged(.ready(self.settings.model))
                default: return false
                }
                return true
            },
            snapshot: { [weak self] path in
                guard let self else { return false }
                if self.panel.isShowing { self.panel.renderSnapshot(to: path); return true }
                if self.statusMenu.isOpen { self.statusMenu.renderSnapshot(to: path); return true }
                if let window = self.settingsWindow, window.isVisible { OnDisplay.render(window, to: path); return true }
                return false
            }))
    }
}

MainActor.assumeIsolated {
    // A drawing flag with no folder draws nothing: say so, rather than start the real app.
    if let flag = LaunchMode.renderFlagMissingFolder(in: CommandLine.arguments) {
        FileHandle.standardError.write(Data("Iris: \(flag) needs a folder to draw into, e.g. Iris \(flag) /tmp/iris\n".utf8))
        exit(2)
    }
    switch LaunchMode.current {
    case .practiceBox:
        PracticeBox.run()
    case .render(let what, let dir):
        // Iris is a paper app: light only, whatever the system appearance.
        NSApplication.shared.appearance = NSAppearance(named: .aqua)
        switch what {
        case .icon: Snapshot.icon(to: dir)
        case .rigFrames: RigFrames.run(into: dir)
        case .readmeImages: ReadmeImages.render(into: dir)
        case .setupStates: SetupStates.render(into: dir)
        case .toastStates: ToastStates.render(into: dir)
        case .panelStates: PanelStates.run(into: dir)
        case .snapshot: Snapshot.run(into: dir)
        }
        exit(0)
    default:
        break
    }
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    app.setActivationPolicy(.accessory)
    app.run()
}
