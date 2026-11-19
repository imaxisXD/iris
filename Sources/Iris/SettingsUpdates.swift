import AppKit
import IrisCore
import SwiftUI

// Settings › Updates: two sections, like the other pages. Your copy is one row: the version
// you have (or where the update check stands, written in as the letter from GitHub opens)
// with the letter and its button at the right. What's new prints one version's changes beside
// its zine cover, the newest first, with the other versions as small covers to pick. The
// GitHub links are on About; Report a bug is at the rail's foot.

struct UpdatesPage: View {
    @ObservedObject var updates = UpdateCheck.shared
    var changelog: Changelog? = nil
    /// The version this copy is, to mark its cover "yours".
    var installed = Updater.version
    /// Snapshots only: the version shown before anyone picks one (the newest if nil).
    var start: String? = nil
    /// Snapshots only: the check in each state, without What's new under it.
    var showsNotes = true

    var body: some View {
        let status = UpdateStatus(phase: updates.phase, start: updates.start, install: updates.install,
                                  wordsBeforeLook: updates.wordsBeforeLook)
        VStack(alignment: .leading, spacing: 34) {
            Section(number: 1, title: "Your copy") {
                SettingRow(title: status.title, detail: status.lead, titleInk: status.warning ? DS.Hue.redText : DS.Ink.i12,
                           titleView: status.titleView, detailView: status.leadView) {
                    status.actions
                }
                status.detail
            }
            if showsNotes {
                Section(number: 2, title: "What's new") {
                    VersionNotes(changelog: changelog ?? .bundled, installed: installed, start: start)
                }
            }
        }
    }
}

// MARK: - The check

/// Where the update check stands, in the slip's words (`UpdateCheck.Words`), as the parts of
/// the Your copy row: the answer (in red when it can't), one line under it, and at the right
/// the bud and the letter it brings back while Iris looks and answers, then one button (Check
/// for updates, Check again, Try again, Download; none when another look can't help). At rest
/// there's no scene: the row says which version you have.
struct UpdateStatus {
    let phase: UpdateCheck.Phase
    let start: () -> Void
    let install: () -> Void
    /// What the heading said when this look began, rubbed out as "Looking…" is written in.
    var wordsBeforeLook = (title: "", lead: "")

    var title: String {
        switch phase {
        case .idle: "Iris \(Updater.version)"
        case .looking: "Looking…"
        case .answered(let answer): answer.words.title
        case .updating: "Updating"
        case .installFailed: "Update failed"
        }
    }

    var lead: String {
        switch phase {
        case .idle: "The version on this Mac."
        case .looking: "Looking for a new Iris…"
        case .answered(let answer): answer.words.lead
        case .updating: "Iris quits and reopens when the new version is ready. About a minute."
        case .installFailed(let message): message + " Iris is unchanged."
        }
    }

    var warning: Bool {
        switch phase {
        case .answered(let answer): answer.words.warning
        case .installFailed: true
        default: false
        }
    }

    /// The title, written in as it changes: "Looking…" on the click, the answer as the letter
    /// opens (`UpdateCheckScene.Beat`). Nil when it just stands still.
    @MainActor var titleView: AnyView? {
        let ink = warning ? DS.Hue.redText : DS.Ink.i12
        switch phase {
        case .looking(let since):
            return AnyView(AnswerWords(before: wordsBeforeLook.title.isEmpty ? nil : wordsBeforeLook.title, after: title, at: since,
                                       font: DS.Font.body, goes: 0, comes: 0.12))
        case .answered(let answer):
            return AnyView(AnswerWords(before: "Looking…", after: title, at: answer.at, font: DS.Font.body, ink: ink))
        default: return nil
        }
    }

    /// The line under the title: it rises in after the title is written.
    @MainActor var leadView: AnyView? {
        switch phase {
        case .looking(let since):
            return AnyView(AnswerWords(before: wordsBeforeLook.lead.isEmpty ? nil : wordsBeforeLook.lead, after: lead, at: since,
                                       style: .rising, font: DS.Font.caption, ink: DS.Ink.i10, beforeInk: DS.Ink.i10, goes: 0, comes: 0.15))
        case .answered(let answer):
            return AnyView(AnswerWords(before: UpdateStatus(phase: .looking(since: answer.started), start: {}, install: {}).lead,
                                       after: lead, at: answer.at, style: .rising, font: DS.Font.caption,
                                       ink: DS.Ink.i10, beforeInk: DS.Ink.i10, comes: UpdateCheckScene.Beat.lead))
        default: return nil
        }
    }

    @MainActor @ViewBuilder var actions: some View {
        HStack(alignment: .center, spacing: DS.Space.md) {
            scene
            button
        }
    }

    @MainActor @ViewBuilder private var scene: some View {
        switch phase {
        case .idle: EmptyView()
        case .updating: UpdateCheckScene.resting
        case .looking(let since): UpdateCheckScene(started: since, layout: .wide)
        case .answered(let answer):
            UpdateCheckScene(started: answer.started, answer: .init(outcome: answer.words.outcome, at: answer.at, letter: answer.words.letter),
                             layout: .wide)
        case .installFailed: UpdateCheckScene(started: .distantPast, answer: .init(outcome: .failed, at: .distantPast), layout: .wide)
        }
    }

    @MainActor @ViewBuilder private var button: some View {
        switch phase {
        case .idle:
            TactileButton(title: "Check for updates", role: .primary, action: start)
        case .looking:
            TactileButton(title: "Check for updates", role: .primary) {}.disabled(true)
        case .answered(let answer):
            AfterAnswer(at: answer.at, delay: UpdateCheckScene.Beat.button) {
                TactileButton(title: "Check for updates", role: .primary) {}.disabled(true)
            } after: {
                answerButton(answer)
            }
        case .updating: EmptyView()
        case .installFailed:
            TactileButton(title: "Try again", role: .secondary, action: start)
        }
    }

    /// The answer's button, once the letter has opened. None when there's nothing to press
    /// (Update & reopen is under the changes; another look can't help): the waiting button
    /// fades and the art eases right into its room.
    @MainActor @ViewBuilder private func answerButton(_ answer: UpdateCheck.Answer) -> some View {
        switch answer.status {
        case .behind: EmptyView()
        case .newRelease(_, let page):
            TactileButton(title: "Download", role: .primary) { NSWorkspace.shared.open(page) }
        case .failed:
            TactileButton(title: "Try again", role: .secondary, action: start)
        case .upToDate:
            TactileButton(title: "Check again", role: .secondary, action: start)
        case .noCheckout, .noRemote: EmptyView()
        }
    }

    /// Under the row: what's in the update with Update & reopen, or how far along it is.
    @MainActor @ViewBuilder var detail: some View {
        switch phase {
        case .answered(let answer):
            if case .behind(_, let changes) = answer.status {
                // Comes in as the letter opens, pushing What's new down then, not before.
                AfterAnswer(at: answer.at, delay: UpdateCheckScene.Beat.lead) {
                    EmptyView()
                } after: {
                    VStack(alignment: .leading, spacing: DS.Space.md) {
                        PenRule(seed: 311)
                        UpdateChanges(changes: changes, update: install)
                    }
                }
            }
        case .updating(let progress):
            ProgressTrack(fraction: progress).frame(height: 22)
        default: EmptyView()
        }
    }
}

// MARK: - What's new

extension Changelog {
    /// The changelog Settings shows: the one that ships inside Iris (Resources/Changelog.md),
    /// or made-up versions with `--sample-changelog`, to see the page with many.
    @MainActor static var bundled: Changelog { useSample ? sample : shipped }
    @MainActor static var useSample = CommandLine.arguments.contains("--sample-changelog")

    private static let shipped: Changelog = {
        guard let url = Bundle.module.url(forResource: "Changelog", withExtension: "md"),
            let text = try? String(contentsOf: url, encoding: .utf8)
        else { return Changelog(issues: []) }
        return Changelog.parse(text)
    }()
    private static let sample = Changelog.parse(SampleChangelog.text)
}

// MARK: - What's new

/// One version's changes printed on the page beside its cover, and the other versions as
/// small covers under them: a click on one prints its changes instead (`Changelog.reading`).
struct VersionNotes: View {
    let changelog: Changelog
    let installed: String
    var start: String? = nil
    @State private var picked: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if let reading = changelog.reading(picked ?? start) {
            VStack(alignment: .leading, spacing: DS.Space.md) {
                HStack(alignment: .top, spacing: 28) {
                    // The cover is the picture; the words beside it say the same for VoiceOver.
                    IssueCover(issue: reading.shown, width: 118, yours: yours(reading.shown))
                        .rotationEffect(.degrees(-3))
                        .padding(.top, 14)
                        .accessibilityHidden(true)
                    IssueNotes(issue: reading.shown, yours: yours(reading.shown))
                }
                .id(reading.shown.version)
                .transition(.opacity)
                if !reading.others.isEmpty {
                    PenRule(seed: 901).padding(.top, 6)
                    OtherVersions(issues: reading.others, installed: installed) { picked = $0 }
                }
            }
            .animation(reduceMotion ? nil : DS.easeOut, value: reading.shown.version)
        } else {
            Text("No notes for this version yet.").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
        }
    }

    private func yours(_ issue: Changelog.Issue) -> Bool { ReleaseVersion.same(issue.version, installed) }
}

/// A version's changes: its name in brush and the day it came out, each headline change with
/// its line, then the small ones ticked off in pink.
private struct IssueNotes: View {
    let issue: Changelog.Issue
    let yours: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(issue.title).font(DS.Font.brush(26)).foregroundStyle(DS.Ink.i12)
                    .fixedSize(horizontal: false, vertical: true)
                if let day = Self.day(issue.date) {
                    Text(day).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityAddTraits(.isHeader)
            .accessibilityLabel(["Version \(issue.version)", issue.title, Self.day(issue.date), yours ? "the one you have" : nil]
                .compactMap { $0 }.joined(separator: ", "))
            ForEach(Array(issue.chapters.enumerated()), id: \.offset) { _, chapter in
                VStack(alignment: .leading, spacing: 3) {
                    Text(chapter.title).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                    if !chapter.detail.isEmpty {
                        Text(chapter.detail).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityElement(children: .combine)
            }
            ForEach(Array(issue.also.enumerated()), id: \.offset) { _, line in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
                        .frame(width: 12, height: 9)
                        .accessibilityHidden(true)
                    Text(line).font(DS.Font.caption).foregroundStyle(DS.Ink.i11)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// "2026-11-17" as "17 Nov 2026".
    static func day(_ iso: String?) -> String? {
        guard let iso, let date = reader.date(from: iso) else { return iso }
        return writer.string(from: date)
    }
    private static let reader: DateFormatter = {
        let parse = DateFormatter()
        parse.locale = Locale(identifier: "en_US_POSIX")
        parse.dateFormat = "yyyy-MM-dd"
        return parse
    }()
    private static let writer: DateFormatter = {
        let show = DateFormatter()
        show.dateFormat = "d MMM yyyy"
        return show
    }()
}

/// The other versions as small covers in a row (more rows when there are many), each with its
/// number under it; the one you have wears its tape.
private struct OtherVersions: View {
    let issues: [Changelog.Issue]
    let installed: String
    let pick: (String) -> Void
    private static let perRow = 8

    var body: some View {
        HStack(alignment: .top, spacing: 18) {
            Text("OTHER\nVERSIONS").font(DS.Font.micro).tracking(1).foregroundStyle(DS.Ink.i10)
                .padding(.top, 4)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 14) {
                ForEach(Array(stride(from: 0, to: issues.count, by: Self.perRow)), id: \.self) { at in
                    HStack(alignment: .top, spacing: 14) {
                        ForEach(issues[at..<min(at + Self.perRow, issues.count)]) { issue in
                            CoverButton(issue: issue, yours: ReleaseVersion.same(issue.version, installed)) { pick(issue.version) }
                        }
                    }
                }
            }
        }
    }
}

/// A small cover to pick: it lifts a little under the pointer, like taking it off the pile.
private struct CoverButton: View {
    let issue: Changelog.Issue
    let yours: Bool
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                IssueCover(issue: issue, width: 44)
                    // The one you have wears its tape across the top, as the big cover does.
                    .overlay(alignment: .top) {
                        if yours { TapeLabel(text: "yours", size: 8).rotationEffect(.degrees(-7)).offset(y: -9) }
                    }
                    .rotationEffect(.degrees(hovering ? -3 : 0))
                    .offset(y: hovering ? -3 : 0)
                Text(issue.version).font(DS.Font.micro).foregroundStyle(DS.Ink.i11)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.96))
        .pointerStyle(.link)
        .onHover { hovering = $0 }
        .animation(DS.easeOut, value: hovering)
        .accessibilityLabel("Version \(issue.version), \(issue.title)\(yours ? ", the one you have" : "")")
        .accessibilityHint("Shows what changed in it")
    }
}

/// A zine cover printed in one riso ink: the masthead, the issue number, the version big and
/// the issue's title in brush, under a pen edge. The one you have wears a strip of tape saying so.
struct IssueCover: View {
    let issue: Changelog.Issue
    var width: CGFloat = 118
    var yours = false

    /// A zine's proportions.
    static let aspect: CGFloat = 330 / 244
    /// Each issue in the next ink round, so neighbours never match.
    static let inks: [RGB8] = [Riso.platePink, Riso.sky, Riso.tan, Riso.crema]
    static func ink(of issue: Changelog.Issue) -> RGB8 { inks[(issue.number - 1 + inks.count * 100) % inks.count] }

    var body: some View {
        let s = width / 98
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("IRIS").tracking(1.6 * s)
                Spacer(minLength: 2)
                Text(String(format: "No.%02d", issue.number))
            }
            .font(DS.Font.martian(9 * s, weight: 600))
            PenRule(seed: issue.number * 5 + 1, ink: DS.Ink.i12.opacity(0.7), width: 1.2 * s).padding(.top, 2 * s)
            Spacer(minLength: 0)
            Text(issue.version).font(DS.Font.display(30 * s)).tracking(-0.8 * s).lineLimit(1).minimumScaleFactor(0.6)
            Text(issue.title).font(DS.Font.brush(17 * s)).lineLimit(2).lineSpacing(-4 * s)
                .fixedSize(horizontal: false, vertical: true)
        }
        .foregroundStyle(DS.Ink.i12)
        .padding(9 * s)
        .frame(width: width, height: width * Self.aspect, alignment: .topLeading)
        .background(Rectangle().printedInk(Self.ink(of: issue), seed: issue.number * 13))
        .overlay(HandRect(seed: issue.number * 7 + 2, wobble: 0.5 * s).stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.4 * s, lineCap: .round)))
        // The spine: a band of the same ink a shade deeper down the bound edge, then the hinge.
        .overlay(alignment: .leading) {
            ZStack(alignment: .leading) {
                Rectangle().printedInk(Riso.ink, seed: issue.number * 3).opacity(0.16).frame(width: 9 * s)
                HandLine(seed: issue.number * 11, wobble: 0.4 * s, vertical: true)
                    .stroke(DS.Ink.i12.opacity(0.55), lineWidth: 1 * s)
                    .frame(width: 1).offset(x: 9 * s)
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
        .overlay(alignment: .top) {
            if yours {
                TapeLabel(text: "yours", size: 9 * s).rotationEffect(.degrees(-7)).offset(x: 12 * s, y: -15 * s)
                    .accessibilityHidden(true)
            }
        }
    }
}
