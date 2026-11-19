/// How far the garden has grown: the setup story, and the model's state in the slip and
/// Settings, told by one flower (`GardenView` draws it).
public enum GardenStage: Int, Comparable, Sendable {
    /// Waiting for permission, or the model resting: a closed bud sways.
    case bud
    /// Step 1 done: it opens, the first butterfly flutters in.
    case allowed
    /// Downloading: a riso cloud rains on it.
    case watering
    /// Loading into memory: the cloud leaves, a sun rises behind.
    case warming
    /// The model's ready: a second butterfly arrives.
    case ready
    /// The practice box was fixed: two more, and a shower of petals.
    case tried

    public static func < (a: GardenStage, b: GardenStage) -> Bool { a.rawValue < b.rawValue }
}

/// What setup says, wherever it says it: setup's window, the menu bar slip and VoiceOver.
extension SetupProgress.Step {
    /// The step's name in setup's list.
    public var title: String {
        switch self {
        case .allow: "Allow"
        case .model: "Get the model"
        case .tryIt: "Try it"
        }
    }

    /// "Step 2 of 3: get the model."
    public var line: String { numbered(what) }

    func numbered(_ what: String) -> String { "Step \(rawValue) of \(Self.allCases.count): \(what)." }

    private var what: String {
        switch self {
        case .allow: "allow Iris to read the box you type in"
        case .model: "get the model"
        case .tryIt: "try it once"
        }
    }

    /// The word beside a stuck step: permission isn't there yet; a download is stuck.
    public var stuckWord: String { self == .allow ? "not yet" : "stuck" }
}

extension SetupProgress {
    /// The line under the slip's SETTING UP band: which step, in the reader's words, or what
    /// was lost for someone who'd finished setup before. Nil once setup is complete.
    public var slipLine: String? {
        guard let step = current else { return nil }
        switch step {
        case .allow:
            return isReturning ? "Iris lost its permission to read text. Allow it again." : step.line
        case .model:
            switch facts.activity {
            case .failed(let failure): return failure.sentence
            case .downloading: return step.numbered("getting the model")
            default: return isReturning ? "The model isn't on this Mac any more. Download it again." : step.line
            }
        case .tryIt:
            return step.line
        }
    }

    /// What VoiceOver says when the page turns to the current step by itself.
    public var announcement: String { current?.line ?? "Setup is done." }

    /// The garden follows setup, never ahead of it: without permission it's still the bud,
    /// whatever the model is doing.
    public var garden: GardenStage {
        if facts.tried || (isComplete && facts.finishedBefore) { return .tried }
        guard facts.trusted else { return .bud }
        switch facts.activity {
        case .downloading: return .watering
        case .loading: return .warming
        case .ready: return .ready
        case .none, .failed: return facts.modelOnDisk ? .ready : .allowed
        }
    }
}
