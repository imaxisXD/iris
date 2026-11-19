import IrisEngine
import SwiftUI

/// What a design check draws instead of the Mac's real state, handed down the view tree like
/// any other environment value. Nil in the app. (Setup takes its made-up facts through
/// `OnboardingView(frozen:)`, since its model is made before the environment exists.)
struct DesignState {
    /// Settings › Model & memory: the engine's state, the chosen model, what's on disk.
    var model: (state: EngineState, chosen: ModelChoice, onDisk: Set<ModelChoice>)?
    /// The model whose Delete the pointer is on.
    var crossing: ModelChoice?
    /// Settings › General's note demos, held still at this moment.
    var noteAt: Double?

    /// Every snapshot: the note demos held with their note on the box.
    static var snapshot: DesignState { DesignState(noteAt: 2.6) }
}

extension EnvironmentValues {
    @Entry var designState: DesignState? = nil
}
