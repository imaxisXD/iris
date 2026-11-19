import IrisCore
import IrisEngine
import SwiftUI

/// The model's state for every screen. One watcher of the engine, so the panel, the menu bar
/// slip, setup and Settings all follow the same value instead of each watching the engine
/// (and each leaving a watcher behind when its screen goes away).
@MainActor
final class EngineMonitor: ObservableObject {
    static let shared = EngineMonitor()

    @Published private(set) var state: EngineState = .idle

    /// A model was deleted from disk: redraw what shows which models are downloaded.
    func diskChanged() { objectWillChange.send() }

    private init() {
        Task {
            await Engine.shared.observe { state in
                Task { @MainActor in EngineMonitor.shared.state = state }
            }
        }
    }
}
