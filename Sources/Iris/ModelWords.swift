import AppKit
import IrisCore
import IrisEngine
import os

/// The model in the words people see, the same in setup and in Settings › Model & memory.
/// The technical names stay in the code.
extension ModelChoice {
    var plainName: String {
        switch self {
        case .best: "Best quality"
        case .lighter: "Lighter"
        case .e4b: "Earlier best quality"
        case .e2b: "Earlier faster model"
        }
    }

    /// One short line on what each is for, under its drawing.
    var shortLine: String {
        switch self {
        case .best: "Trained for Iris. The best in English and Hindi. For Macs with \(Self.roomyMemoryGB) GB or more."
        case .lighter: "Trained for Iris. Nearly as good in English, weaker in Hindi. For Macs with 8 GB."
        case .e4b, .e2b: "The model Iris used before. Not trained for Iris."
        }
    }

    /// "2.7 of 5.4 GB": how much is in, out of the size the download button promised. The
    /// engine's fraction is of the model alone (its drafter follows once the model is ready),
    /// so that's what's counted, never more than is really there.
    func downloaded(_ fraction: Double) -> String {
        String(format: "%.1f of %.1f GB", Double(modelBytes) * fraction / 1e9, Double(downloadBytes) / 1e9)
    }
}

/// The questions Iris asks before changing what's on disk, from setup and Settings alike.
@MainActor
enum ModelDialogs {
    /// Stopping keeps what's in: the next download picks up where this one stopped.
    static func stop() -> ConfirmRequest {
        ConfirmRequest(
            title: "Stop downloading the model?",
            message: "What's downloaded so far stays on this Mac, and the download picks up where it stopped the next time.",
            confirmTitle: EngineState.stopTitle) { Task { await Engine.shared.unload() } }
    }

    static func delete(_ choice: ModelChoice) -> ConfirmRequest {
        ConfirmRequest(
            title: "Delete the \(choice.plainName.lowercased()) model from disk?",
            message: "This frees \(choice.downloadSize) of disk space, not memory (Iris frees memory by itself when you're not using it). To use it again, download it from this page.",
            confirmTitle: "Delete from disk") {
                Task {
                    if AppSettings.shared.model == choice { await Engine.shared.unload() }
                    for folder in choice.cacheDirectories {
                        do { try FileManager.default.removeItem(at: folder) } catch CocoaError.fileNoSuchFile {
                        } catch {
                            // What's left still counts as on disk, so the page keeps saying so.
                            log.error("couldn't delete \(folder.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
                        }
                    }
                    EngineMonitor.shared.diskChanged()
                }
            }
    }

    private static let log = Logger(subsystem: "local.iris.app", category: "model")
}

/// The model's state in Iris's words and garden, the same in the menu bar slip and in
/// Settings › Model & memory. Change them here, never per screen.
extension EngineState {
    var title: String {
        switch self {
        case .idle: "Resting"
        case .downloading: "Downloading"
        case .loading: "Waking up"
        case .ready: "Ready"
        case .failed: Self.troubleTitle
        }
    }

    /// The title when something stops Iris, the model or anything else (a taken shortcut).
    static let troubleTitle = "Something's off"

    var detail: String {
        switch self {
        case .idle: "The model loads on your first check."
        case .downloading: "Getting the model. One time only."
        case .loading: "Getting ready. A few seconds."
        case .ready: "Runs on this Mac. Nothing leaves it."
        case .failed(let failure): failure.sentence
        }
    }

    /// What setup reads as the chosen model's activity: another model being ready is no
    /// help to it.
    func activity(for chosen: ModelChoice) -> SetupProgress.ModelActivity {
        switch self {
        case .failed(let failure): .failed(failure)
        case .downloading(let fraction): .downloading(fraction)
        case .loading: .loading
        case .ready(let choice) where choice == chosen: .ready
        default: .none
        }
    }

    var gardenStage: GardenView.Stage {
        switch self {
        case .idle, .failed: .bud
        case .downloading: .watering
        case .loading: .warming
        case .ready: .ready
        }
    }

    /// The command that applies now; the same words as the slip's model row.
    static let loadTitle = "Load model now"
    static let freeTitle = "Free model memory"
    static let stopTitle = "Stop download"
}
