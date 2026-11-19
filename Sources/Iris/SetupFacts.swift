import IrisCore
import IrisEngine

extension SetupProgress.Facts {
    /// What the Mac says now: the permission, the chosen model on disk (or no need for one, in
    /// a check run with stand-in answers) and what the model is doing. Setup's window adds
    /// what happened in it: the practice box fixed, coming back from System Settings.
    /// `engine`: the state just reported, when called while it changes (a `$state` sink runs
    /// before `EngineMonitor.state` holds the new value).
    @MainActor
    static func now(engine: EngineState? = nil, tried: Bool = false, backFromSettingsUntrusted: Bool = false) -> Self {
        let settings = AppSettings.shared
        return Self(trusted: TextBridge.isTrusted,
                    modelOnDisk: !LaunchMode.current.needsModel || settings.model.isDownloaded,
                    activity: (engine ?? EngineMonitor.shared.state).activity(for: settings.model),
                    tried: tried, finishedBefore: settings.onboarded,
                    backFromSettingsUntrusted: backFromSettingsUntrusted)
    }
}
