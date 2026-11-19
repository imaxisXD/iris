import AppKit
import Combine
import IrisCore
import IrisEngine
import SwiftUI

/// Setup's facts, read from the Mac, turned into `SetupProgress`. The window applies what
/// that says: one step at a time, in order, each ticked by the real thing happening.
@MainActor
final class OnboardingModel: ObservableObject {
    /// Snapshots and the setup preview: made-up facts, so every state draws without touching
    /// permission or the model.
    struct Frozen {
        var facts: SetupProgress.Facts
        var practiceText = OnboardingModel.practiceSample
        /// Another app holds the shortcut (Try it says so).
        var shortcutTaken = false
    }
    /// Made-up facts to draw instead of the Mac's; nil in the app.
    let frozen: Frozen?

    nonisolated static let practiceSample = "hey, i have checked the PR and its look fine but one thing need to change before merge"
    /// The practice text fixed, for the design previews that play a replace.
    nonisolated static let practiceFixed = "Hey, I have checked the PR and it looks fine, but one thing needs to change before merge"

    @Published private(set) var trusted = TextBridge.isTrusted
    @Published private(set) var modelOnDisk = AppSettings.shared.model.isDownloaded
    @Published var practiceText = OnboardingModel.practiceSample {
        didSet { if awaitingPaste, practiceText != Self.practiceSample { awaitingPaste = false; tried = true } }
    }
    /// Iris replaced the practice text this session (not typing: see `irisPastedIntoIris`).
    @Published private(set) var tried = false
    /// Came back from System Settings and Iris still isn't allowed.
    @Published private(set) var backFromSettingsUntrusted = false
    @Published private(set) var engineState: EngineState = EngineMonitor.shared.state

    private var awaitingPaste = false
    private var sentToSettings = false
    private var timer: Timer?
    private var watchers: [AnyCancellable] = []

    init(frozen: Frozen? = nil) {
        self.frozen = frozen
        if let frozen {
            trusted = frozen.facts.trusted
            modelOnDisk = frozen.facts.modelOnDisk
            tried = frozen.facts.tried
            backFromSettingsUntrusted = frozen.facts.backFromSettingsUntrusted
            practiceText = frozen.practiceText
            return
        }
        watchers.append(EngineMonitor.shared.$state.sink { [weak self] state in
            self?.engineState = state
            self?.refreshDisk()
        })
        // A model deleted in Settings: its files go after the engine has already said idle.
        watchers.append(EngineMonitor.shared.objectWillChange.sink { [weak self] _ in
            DispatchQueue.main.async { self?.refreshDisk() }
        })
        watchers.append(NotificationCenter.default.publisher(for: .irisPastedIntoIris).sink { [weak self] _ in
            self?.awaitingPaste = true
        })
        // Back from System Settings (or Storage): look again at once, and if Iris still isn't
        // allowed after a moment, say what to try.
        watchers.append(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification).sink { [weak self] _ in
            self?.cameBack()
        })
        watchers.append(AppSettings.shared.$model.dropFirst().sink { [weak self] _ in
            DispatchQueue.main.async { self?.refreshDisk() }
        })
    }

    /// Permission is granted in System Settings; look while the window is open, and stop
    /// looking when it closes. Closing the window drops it, and this model with it (SwiftUI
    /// doesn't call onDisappear for a window that is only closed): a timer whose model has gone
    /// stops itself.
    func startWatching() {
        guard frozen == nil, timer == nil else { return }
        timer = Timer.scheduledTimer(withTimeInterval: 0.8, repeats: true) { [weak self] timer in
            guard let self else { return timer.invalidate() }
            Task { @MainActor in self.poll() }
        }
        poll()
    }

    func stopWatching() {
        timer?.invalidate()
        timer = nil
    }

    /// Setup's window is open (see `startWatching`).
    private var watching: Bool { timer != nil }

    private func poll() {
        let now = TextBridge.isTrusted
        if now != trusted { trusted = now }
        if now { backFromSettingsUntrusted = false }
        preloadIfReady()
    }

    private func refreshDisk() {
        guard frozen == nil else { return }
        let onDisk = AppSettings.shared.model.isDownloaded
        if onDisk != modelOnDisk { modelOnDisk = onDisk }
        preloadIfReady()
    }

    private func cameBack() {
        // Only while setup's window is open: with it closed ("Set up later"), coming back to
        // Iris for anything else (Settings, say) must not start a download or wake the model.
        guard frozen == nil, watching else { return }
        poll()
        if sentToSettings, !trusted {
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(1.5))
                if !self.trusted { self.backFromSettingsUntrusted = true }
            }
        }
        // Space freed in Storage: try the download again by itself.
        if case .noRoom = engineState.failure { download() }
    }

    /// Read from the Mac as the app reads it (`Facts.now`); `trusted`, `modelOnDisk` and
    /// `engineState` are published so the window redraws when any of them changes.
    var progress: SetupProgress {
        if let frozen = frozen {
            var facts = frozen.facts
            facts.tried = tried || facts.tried
            return SetupProgress(facts)
        }
        return SetupProgress(.now(engine: engineState, tried: tried, backFromSettingsUntrusted: backFromSettingsUntrusted))
    }

    /// At Try it, wake the model on disk so the practice check is quick. Never a download.
    private func preloadIfReady() {
        // Not with the window closed: freeing the model from the slip would wake it again.
        guard watching, progress.preloads else { return }
        Task { try? await Engine.shared.load(AppSettings.shared.model) }
    }

    func openSystemSettings() {
        sentToSettings = true
        TextBridge.requestTrust()
        TextBridge.openAccessibilitySettings()
    }

    /// Setup's Download: the one place, with Settings, that's allowed to download.
    func download() {
        guard frozen == nil else { return }
        let choice = AppSettings.shared.model
        Task { try? await Engine.shared.load(choice, allowDownload: true) }
    }

    /// The fix for a permission macOS won't count: open Iris again, and quit this one once the
    /// new one is running. If it doesn't open, this one stays, so Iris never just vanishes.
    func relaunch() {
        let config = NSWorkspace.OpenConfiguration()
        config.createsNewApplicationInstance = true
        NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: config) { app, error in
            guard app != nil, error == nil else { return DispatchQueue.main.async { NSSound.beep() } }
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
    }

    /// Setup is complete once Iris has fixed the practice text; remember it straight away,
    /// so closing the window then doesn't leave setup unfinished.
    func noteTried() {
        if tried, frozen == nil, !AppSettings.shared.onboarded { AppSettings.shared.onboarded = true }
    }
}
