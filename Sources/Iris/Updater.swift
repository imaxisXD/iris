import AppKit
import IrisCore

/// "Check for Updates…": Iris is built from a git checkout on each Mac, so an
/// update means pulling from GitHub and rebuilding. A commit made in the checkout counts as
/// an update too, until this copy is rebuilt with it. scripts/build-app.sh records the
/// checkout's path in Info.plist (IrisSourcePath); scripts/update.sh does the rest.
@MainActor
enum Updater {
    enum Status {
        case upToDate
        /// How many commits this copy is missing, from GitHub or from the checkout itself, and
        /// their subject lines (newest first, at most 5).
        case behind(Int, [String])
        case noCheckout
        /// A downloaded copy: a newer release is on GitHub, to download from `page`.
        case newRelease(String, page: URL)
        case noRemote
        case failed(String)
    }

    static var sourcePath: String? {
        guard let path = Bundle.main.object(forInfoDictionaryKey: "IrisSourcePath") as? String,
            FileManager.default.fileExists(atPath: path + "/scripts/update.sh")
        else { return nil }
        return path
    }

    /// "0.1.0 (12 · a6099ea)" for a checkout build, the plain version otherwise.
    static var versionLabel: String {
        guard let build, let commit else { return version }
        return "\(version) (\(build) · \(commit))"
    }

    /// "0.1.0", or "dev" for a build without one.
    nonisolated static var version: String { Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev" }
    /// The build number (commits in the checkout when it was built), for a checkout build.
    nonisolated static var build: String? { Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String }
    /// The commit this copy was built from, short.
    nonisolated static var commit: String? { Bundle.main.object(forInfoDictionaryKey: "IrisCommit") as? String }

    /// "26.1 (25B78)": the macOS this Mac runs, for a problem report.
    nonisolated static var macOSLabel: String {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        let number = v.patchVersion > 0 ? "\(v.majorVersion).\(v.minorVersion).\(v.patchVersion)" : "\(v.majorVersion).\(v.minorVersion)"
        var size = 0
        sysctlbyname("kern.osversion", nil, &size, nil, 0)
        var chars = [CChar](repeating: 0, count: max(size, 1))
        guard size > 0, sysctlbyname("kern.osversion", &chars, &size, nil, 0) == 0 else { return number }
        return "\(number) (\(String(cString: chars)))"
    }

    /// Iris's code on GitHub: Settings › Updates links here (the code, a problem, a change).
    nonisolated static let repo = URL(string: "https://github.com/imaxisXD/iris")!

    static let logPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(UpdateRun.logPathInHome).path

    /// Fetches, then compares the commit this copy was built from with both GitHub and the
    /// checkout. Shown on the slip's Updates page and in Settings › Updates.
    static func check() async -> Status {
        guard let path = sourcePath else { return await checkReleases() }
        let builtFrom = commit
        return await Task.detached { Self.status(of: path, builtFrom: builtFrom) }.value
    }

    /// Where releases live. A downloaded copy asks it for the latest one, only when you choose
    /// Check for Updates: nothing about you or your text is sent.
    static let releases = URL(string: "https://github.com/imaxisXD/iris/releases/latest")!
    private static let latestAPI = URL(string: "https://api.github.com/repos/imaxisXD/iris/releases/latest")!

    /// A downloaded copy (no checkout to rebuild from): compares its version with GitHub's
    /// latest release.
    private static func checkReleases() async -> Status {
        guard Bundle.main.object(forInfoDictionaryKey: "IrisChannel") as? String == "release" else { return .noCheckout }
        let installed = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
        var request = URLRequest(url: latestAPI, timeoutInterval: 15)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            if (response as? HTTPURLResponse)?.statusCode == 404 { return .upToDate }  // no release yet
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                let tag = json["tag_name"] as? String
            else { return .failed("GitHub sent an answer Iris couldn't read. Try again later.") }
            // The page is opened in the browser: GitHub's own, or the releases page.
            let page = ReleaseVersion.releasePage(json["html_url"] as? String) ?? releases
            return ReleaseVersion.isNewer(tag, than: installed) ? .newRelease(tag, page: page) : .upToDate
        } catch {
            return .failed("Couldn't reach GitHub. Check your connection and try again.")
        }
    }

    /// Pulls, rebuilds and reopens Iris (scripts/update.sh, detached: it quits Iris). Returns
    /// where the log ended before it started, to read this run's lines from, or nil if the
    /// update couldn't start at all.
    static func install() -> UInt64? {
        guard let path = sourcePath else { return nil }
        let offset = (try? FileManager.default.attributesOfItem(atPath: logPath)[.size] as? UInt64) ?? 0
        guard install(from: path) else { return nil }
        UserDefaults.standard.set(Date().timeIntervalSince1970, forKey: startedKey)
        return offset
    }

    /// When the update that's building right now began, or nil if none is. It may have been
    /// started from this menu or from a terminal (scripts/ship.sh): update.sh holds
    /// .build/update.lock while it builds. A lock older than 15 minutes was left by a run that
    /// died; update.sh clears it, and it doesn't count here.
    static var runningUpdateStarted: Date? {
        guard let path = sourcePath,
            let touched = (try? FileManager.default.attributesOfItem(atPath: path + "/" + UpdateRun.lockPath))?[.modificationDate] as? Date,
            Date().timeIntervalSince(touched) < givesUpAfter  // as update.sh judges it (find -mmin): scripts/lib.sh
        else { return nil }
        return touched
    }

    // MARK: How far along

    private static let startedKey = "updateStartedAt"
    private static let lastSecondsKey = "lastUpdateSeconds"

    /// How long an update takes here: the last one's time, from Update & reopen to the new
    /// Iris starting, or about a minute and a quarter until there is one.
    static var expectedSeconds: Double {
        let last = UserDefaults.standard.double(forKey: lastSecondsKey)
        return last > 0 ? last : UpdateRun.usualSeconds
    }

    /// An update that failed never finishes: the next launch mustn't time it as one that did.
    static func forgetStart() { UserDefaults.standard.removeObject(forKey: startedKey) }

    /// How long an update may run before Iris stops waiting for it: the age at which
    /// update.sh treats a lock as left by a run that died (scripts/lib.sh).
    static let givesUpAfter = UpdateRun.givesUpAfter

    /// Call at launch: if this launch finished an update, remember how long it took.
    static func noteLaunch() {
        let defaults = UserDefaults.standard
        let started = defaults.double(forKey: startedKey)
        guard started > 0 else { return }
        defaults.removeObject(forKey: startedKey)
        let seconds = Date().timeIntervalSince1970 - started
        if UpdateRun.isBelievable(seconds: seconds) { defaults.set(seconds, forKey: lastSecondsKey) }
    }

    /// How far along an update is after `elapsed` seconds, from the time it usually takes
    /// here (the guess is `UpdateRun.progress`).
    static func progress(after elapsed: Double) -> Double {
        UpdateRun.progress(after: elapsed, expected: expectedSeconds)
    }

    /// This run's failure, if the update script logged one since `offset` (read by
    /// `UpdateRun.failure(in:)`).
    static func failure(since offset: UInt64) -> UpdateRun.Failure? {
        guard let handle = FileHandle(forReadingAtPath: logPath) else { return nil }
        defer { try? handle.close() }
        try? handle.seek(toOffset: offset)
        return UpdateRun.failure(in: String(decoding: handle.readDataToEndOfFile(), as: UTF8.self))
    }

    /// Runs the update script detached from Iris, since the script quits Iris to replace it.
    private static func install(from path: String) -> Bool {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/bin/zsh")
        task.arguments = ["-c", "nohup \"$0\" >> \"$1\" 2>&1 &", path + "/scripts/update.sh", logPath]
        // Not Iris's own environment as it is: whatever opened Iris may have left build settings
        // in it that would point the update at an old commit.
        task.environment = UpdateEnvironment.cleaned(ProcessInfo.processInfo.environment)
        do { try task.run() } catch { return false }
        return true
    }

    nonisolated private static func status(of path: String, builtFrom: String?) -> Status {
        guard run(["remote", "get-url", "origin"], in: path).status == 0 else { return .noRemote }
        // A stalled connection gives up instead of looking forever: git itself after 15 seconds
        // below 1 KB/s, and the process is stopped at 30 whatever it's doing.
        let fetch = run(["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=15", "fetch", "--quiet", "origin"],
                        in: path, timeout: 30)
        guard fetch.status == 0 else {
            return .failed("Couldn't reach GitHub. Check your connection and try again.")
        }
        // What this copy is missing: new commits on GitHub, and commits made in the checkout
        // since it was built (those never show up as "GitHub is ahead"). update.sh pulls,
        // which does nothing when GitHub has nothing new, then builds the checkout.
        var missing = ["HEAD..@{u}"]
        if let builtFrom, run(["cat-file", "-e", builtFrom + "^{commit}"], in: path).status == 0 {
            missing = ["^" + builtFrom, "HEAD", "@{u}"]
        }
        let count = run(["rev-list", "--count"] + missing, in: path)
        guard count.status == 0, let behind = Int(count.output.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            return .failed("Iris doesn't know which version to follow on GitHub yet.")
        }
        guard behind > 0 else { return .upToDate }
        let log = run(["log", "--format=%s", "--max-count=5"] + missing, in: path)
        let subjects = log.output.split(separator: "\n").map { line in
            // "Iris: menu bar slip as a hand-cut note" reads better without the app prefix.
            String(line.hasPrefix("Iris: ") ? line.dropFirst(6) : line[...])
        }
        return .behind(behind, log.status == 0 ? subjects : [])
    }

    nonisolated private static func run(_ arguments: [String], in path: String, timeout: Double? = nil) -> (status: Int32, output: String) {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        task.arguments = ["-C", path] + arguments
        // Never block on a password prompt from a background check, and never carry build
        // settings left in Iris's environment (as the install doesn't).
        task.environment = UpdateEnvironment.cleaned(ProcessInfo.processInfo.environment)
        if let timeout {
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout) { if task.isRunning { task.terminate() } }
        }
        let pipe = Pipe()
        task.standardOutput = pipe
        task.standardError = pipe
        do { try task.run() } catch { return (-1, error.localizedDescription) }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        task.waitUntilExit()
        return (task.terminationStatus, String(decoding: data, as: UTF8.self))
    }
}
