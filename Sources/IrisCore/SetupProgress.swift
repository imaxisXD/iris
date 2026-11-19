import Foundation

/// Where setup stands, worked out from facts the app reads: permission, the chosen model on
/// disk, what the model is doing, and whether the practice box was fixed.
///
/// Setup goes in one order, Allow, Get the model, Try it, and only the current step can be
/// acted on. A step is done only when the thing it asks for has really happened: macOS
/// reports the permission, the model's files are on disk, Iris replaced the practice text.
/// There's no Finish: setup is complete when all three hold (Try it only has to happen once).
/// The shortcut and Quick Fix ask `canCheck` first (Try it is pressing the shortcut); the
/// slip and the launch ask `isComplete`.
public struct SetupProgress: Equatable, Sendable {
    public enum Step: Int, CaseIterable, Comparable, Sendable {
        case allow = 1, model, tryIt
        public static func < (a: Step, b: Step) -> Bool { a.rawValue < b.rawValue }
    }

    /// How one step reads in setup's list.
    public enum Status: Equatable, Sendable {
        /// Already true ("✓ done").
        case done
        /// The step to act on.
        case now
        /// The step to act on, and what it asks for failed: say why and how to get past it.
        case stuck
        /// Comes after the current one; nothing to do yet.
        case later
    }

    /// What the chosen model is doing.
    public enum ModelActivity: Equatable, Sendable {
        case none
        case downloading(Double)
        case loading
        case ready
        case failed(ModelFailure)
    }

    public struct Facts: Equatable, Sendable {
        /// macOS reports Accessibility permission for Iris.
        public var trusted: Bool
        /// Every weight file of the chosen model is on disk.
        public var modelOnDisk: Bool
        public var activity: ModelActivity
        /// Iris replaced the practice text (typing in the box yourself doesn't count).
        public var tried: Bool
        /// Setup was completed once before. Try it isn't asked for again, and a missing
        /// permission or model means something was lost, not that this is someone new.
        public var finishedBefore: Bool
        /// Came back from System Settings and Iris still isn't allowed: the switch may be on
        /// for an older copy of Iris, or IT may have locked it.
        public var backFromSettingsUntrusted: Bool

        public init(trusted: Bool, modelOnDisk: Bool, activity: ModelActivity = .none, tried: Bool = false,
                    finishedBefore: Bool = false, backFromSettingsUntrusted: Bool = false) {
            self.trusted = trusted
            self.modelOnDisk = modelOnDisk
            self.activity = activity
            self.tried = tried
            self.finishedBefore = finishedBefore
            self.backFromSettingsUntrusted = backFromSettingsUntrusted
        }
    }

    public let facts: Facts
    public init(_ facts: Facts) { self.facts = facts }

    /// Setup is seen through: allowed, the model on disk, and the practice box fixed once.
    public var isComplete: Bool { current == nil }

    /// Iris can check text: allowed and the model on disk. True from Try it on, since that
    /// step is pressing the shortcut on the practice box; the shortcut and Quick Fix ask this,
    /// not `isComplete`. (The slip shows its check rows only once setup `isComplete`.)
    public var canCheck: Bool { facts.trusted && facts.modelOnDisk }

    /// The step to act on, or nil when setup is complete.
    public var current: Step? {
        if !facts.trusted { return .allow }
        if !facts.modelOnDisk { return .model }
        if !(facts.tried || facts.finishedBefore) { return .tryIt }
        return nil
    }

    /// Someone who finished setup before and lost the permission or the model since (an
    /// update re-signed Iris, the model was deleted): say what was lost, not "let's get you
    /// set up".
    public var isReturning: Bool { facts.finishedBefore && !isComplete }

    public func status(of step: Step) -> Status {
        if satisfied(step) && step != current { return .done }
        guard let current else { return .done }
        if step > current { return .later }
        switch step {
        case .allow: return facts.backFromSettingsUntrusted ? .stuck : .now
        case .model:
            if case .failed = facts.activity { return .stuck }
            return .now
        case .tryIt: return .now
        }
    }

    /// Whether what a step asks for is true, whatever the current step.
    public func satisfied(_ step: Step) -> Bool {
        switch step {
        case .allow: facts.trusted
        case .model: facts.modelOnDisk
        case .tryIt: facts.tried || facts.finishedBefore
        }
    }

    /// At Try it with the model on disk but not in memory, setup loads it without asking, so
    /// the practice check is quick. A model already on disk is never asked about again.
    public var preloads: Bool { current == .tryIt && facts.activity == .none }

    /// Offer a way to leave setup unfinished only where it can't move on by itself: a stuck
    /// step. Everywhere else the one way forward is the step's own button.
    public var offersSetUpLater: Bool { current.map { status(of: $0) == .stuck } ?? false }
}

