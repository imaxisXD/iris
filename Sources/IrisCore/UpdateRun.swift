import Foundation

/// The rules for following one run of scripts/update.sh from the app: where it keeps its lock
/// and log, how far along it probably is, and what its log says went wrong. The script and
/// `scripts/lib.sh` hold the same paths and write the lines read here; `UpdateRunTests` reads
/// the scripts to keep the two in step.
public enum UpdateRun {
    /// The folder update.sh holds while it builds, inside the checkout (`IRIS_UPDATE_LOCK`).
    public static let lockPath = ".build/update.lock"
    /// A lock older than this was left by a run that died (`IRIS_UPDATE_LOCK_STALE_MINUTES`).
    public static let lockStaleMinutes = 15
    /// How long an update may run before Iris stops waiting for it: the age at which
    /// update.sh treats a lock as left by a run that died.
    public static let givesUpAfter = Double(lockStaleMinutes * 60)
    /// The log's path under the home folder (`IRIS_UPDATE_LOG`).
    public static let logPathInHome = "Library/Logs/Iris-update.log"

    /// How long an update takes until one has been timed on this Mac.
    public static let usualSeconds = 75.0

    /// How far along an update is after `elapsed` seconds, when it usually takes `expected`.
    /// The script doesn't report progress, so this is an honest guess: it reaches about 85%
    /// at the usual time and then creeps, never claiming done before Iris reopens.
    public static func progress(after elapsed: Double, expected: Double) -> Double {
        0.97 * (1 - exp(-2 * max(elapsed, 0) / (expected > 0 ? expected : usualSeconds)))
    }

    /// Whether `seconds` from Update & reopen to the new Iris starting is worth remembering as
    /// how long an update takes: not a launch that had nothing to do with it (too quick, or
    /// long after an update that never finished).
    public static func isBelievable(seconds: Double) -> Bool { (10...900).contains(seconds) }

    /// Why a run stopped, as its log tells it.
    public enum Failure: Equatable, Sendable {
        /// A different update was already building.
        case lockedOut
        case buildFailed
        /// The script's own line about the missing GitHub remote.
        case noRemote(String)
        case stopped

        /// What to say. The system's and git's own words stay in the log.
        public var message: String {
            switch self {
            case .lockedOut: "Another update is already running."
            case .buildFailed: "The new version didn't build."
            case .noRemote(let line): String(line.prefix(80))
            case .stopped: "The update stopped part way."
            }
        }
    }

    /// What scripts/update.sh and build-app.sh write when they stop.
    public enum Marker {
        public static let buildFailed = "Build failed"
        public static let lockedOut = "Another update is running"
        public static let noRemote = "No GitHub remote"
        /// update.sh's line for any step without a message of its own.
        public static let updateFailed = "Update failed"
        /// git's and the compiler's.
        static let others = ["fatal:", "error:"]
        static let all = [buildFailed, lockedOut, noRemote] + others + [updateFailed]
    }

    /// This run's failure, if `log` (the lines written since it began) has one.
    public static func failure(in log: String) -> Failure? {
        guard let line = log.split(separator: "\n").first(where: { line in Marker.all.contains { line.contains($0) } })
        else { return nil }
        if line.contains(Marker.lockedOut) { return .lockedOut }
        // build-app.sh lists the compiler's "error:" lines, then says "Build failed".
        if log.contains(Marker.buildFailed) { return .buildFailed }
        if line.contains(Marker.noRemote) { return .noRemote(String(line)) }
        return .stopped
    }
}

extension ReleaseVersion {
    /// Whether two versions name the same issue of the changelog: "v0.2.0" and "0.2.0" do,
    /// "0.2" and "0.2.0" don't (an issue is found by its full number). Unreadable ones never
    /// match anything.
    public static func same(_ a: String, _ b: String) -> Bool {
        guard let a = parts(a), let b = parts(b) else { return false }
        return a == b
    }

    /// The page GitHub names for a release, only if it really is a page on github.com: it's
    /// opened in the browser, so nothing else an answer might carry gets through.
    public static func releasePage(_ string: String?) -> URL? {
        guard let string, let url = URL(string: string), url.scheme == "https",
            let host = url.host?.lowercased(), host == "github.com" || host == "www.github.com"
        else { return nil }
        return url
    }
}
