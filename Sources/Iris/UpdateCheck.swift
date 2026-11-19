import AppKit
import SwiftUI

/// Where the one update check stands, for everyone who shows it: the menu bar slip's Updates
/// page and Settings › Updates. There's one path: the app (`AppDelegate`) runs the check and
/// the install, and tells this what happened; Settings asks through `start` and `install`,
/// which the app points at its own. Nothing here talks to GitHub.
@MainActor
final class UpdateCheck: ObservableObject {
    static let shared = UpdateCheck()

    enum Phase {
        /// Not asked yet since Iris opened.
        case idle
        /// Looking on GitHub since `since` (the bud sways, the butterfly loops).
        case looking(since: Date)
        case answered(Answer)
        /// A new Iris is building, 0...1. Iris quits and reopens when it's ready.
        case updating(Double)
        /// Update & reopen stopped: what went wrong, in the reader's words.
        case installFailed(String)
    }

    struct Answer {
        let status: Updater.Status
        /// When the look began and when the answer landed, for the scene's ending.
        let started: Date
        let at: Date
        var words: Words { Words(status) }
    }

    /// What an answer says, the same in the slip and in Settings (one set of words).
    struct Words {
        let outcome: UpdateCheckScene.Outcome
        let title: String
        let lead: String
        let warning: Bool
        /// What the butterfly's letter says in Settings: Iris's line, then the facts. None
        /// when it came back empty-handed.
        let letter: UpdateCheckScene.Letter?

        init(_ status: Updater.Status) {
            typealias Row = UpdateCheckScene.Letter.Row
            let yours = Row(name: "yours", value: Updater.version)
            switch status {
            case .upToDate:
                letter = .init(line: "nothing newer yet", rows: [Row(name: "newest", value: Updater.version), yours], ticked: true)
            case .behind(let count, _):
                letter = .init(line: "something new for you", rows: [yours, Row(name: "new", value: count == 1 ? "1 change" : "\(count) changes")])
            case .newRelease(let tag, _):
                letter = .init(line: "fresh off the press", rows: [Row(name: "newest", value: tag.hasPrefix("v") ? String(tag.dropFirst()) : tag), yours])
            case .noCheckout, .noRemote, .failed:
                letter = nil
            }
            switch status {
            case .upToDate:
                (outcome, title, lead, warning) = (.upToDate, "Up to date", "You have the latest Iris.", false)
            case .behind(let count, _):
                (outcome, title, lead, warning) = (.changes(count), count == 1 ? "1 new change" : "\(count) new changes", "Ready to install.", false)
            case .newRelease(let tag, _):
                let version = tag.hasPrefix("v") ? String(tag.dropFirst()) : tag
                (outcome, title, lead, warning) = (.changes(1), "Iris \(version) is out",
                                                   "Download it, then replace Iris in Applications. Your settings stay.", false)
            case .noCheckout:
                (outcome, title, lead, warning) = (.failed, "Can't update",
                                                   "This copy of Iris wasn't installed from its project folder, so it can't update itself.", true)
            case .noRemote:
                (outcome, title, lead, warning) = (.failed, "Can't update",
                                                   "Iris doesn't know where to get updates yet: its project folder isn't linked to GitHub.", true)
            case .failed(let message):
                (outcome, title, lead, warning) = (.failed, "Couldn't check", String(message.prefix(80)), true)
            }
        }
    }

    @Published private(set) var phase: Phase = .idle
    /// What the heading said when the last look began, so it can be rubbed out instead of
    /// jumping to "Looking…".
    private(set) var wordsBeforeLook = (title: "", lead: "")

    /// Looks for updates, the way the slip's Check for Updates does. The app sets it.
    var start: () -> Void = {}
    /// Update & reopen. The app sets it.
    var install: () -> Void = {}

    // What the app reports as the check goes.
    func looking(since: Date) {
        let was = UpdateStatus(phase: phase, start: {}, install: {})
        wordsBeforeLook = (was.title, was.lead)
        withAnimation(DS.easeOut) { phase = .looking(since: since) }
    }
    func answered(_ status: Updater.Status, started: Date, at: Date) {
        withAnimation(DS.easeOut) { phase = .answered(Answer(status: status, started: started, at: at)) }
    }
    func updating(_ progress: Double) { phase = .updating(progress) }
    func installFailed(_ message: String) { withAnimation(DS.easeOut) { phase = .installFailed(message) } }

    /// Snapshots only: draw a made-up phase.
    func preview(_ phase: Phase) { self.phase = phase }

    /// For the design preview and the self-check, never GitHub: a Check for updates that
    /// answers with the next of `answers` each time it's asked, after `slow` seconds for the
    /// looks numbered in `slowLooks` (from 0) and a moment otherwise, once the loop comes round.
    static func madeUpChecks(_ answers: [Updater.Status], slowLooks: Set<Int> = [], slow: TimeInterval = 4) -> () -> Void {
        var looks = 0
        return {
            let n = looks % answers.count, status = answers[n]
            looks += 1
            let started = Date()
            UpdateCheck.shared.looking(since: started)
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(slowLooks.contains(n) ? slow : 0.3))
                try? await Task.sleep(for: .seconds(UpdateCheckScene.wait(after: Date().timeIntervalSince(started))))
                UpdateCheck.shared.answered(status, started: started, at: Date())
            }
        }
    }
}
