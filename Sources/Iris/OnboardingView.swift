import AppKit
import IrisCore
import IrisEngine
import SwiftUI

struct OnboardingView: View {
    @StateObject private var model: OnboardingModel
    @ObservedObject private var settings = AppSettings.shared
    @Environment(\.confirm) private var confirm
    /// The window scrolls the page under the plate; snapshots render the whole page instead.
    var scrolls = true
    /// Done, at the end: close and point at the menu bar.
    var onFinish: () -> Void
    /// Set up later, on a stuck step: close, setup unfinished.
    var onLater: () -> Void = {}

    /// `frozen`: made-up facts to draw (the setup states and preview); nil reads the Mac.
    init(frozen: OnboardingModel.Frozen? = nil, scrolls: Bool = true, onFinish: @escaping () -> Void,
         onLater: @escaping () -> Void = {}) {
        _model = StateObject(wrappedValue: OnboardingModel(frozen: frozen))
        self.scrolls = scrolls
        self.onFinish = onFinish
        self.onLater = onLater
    }

    static let width: CGFloat = 720
    /// The window's height: a page that fits a 13-inch laptop's screen with room around it,
    /// tall enough that the model step's dots and its Download button are in view at once.
    static let height: CGFloat = 720
    static let plateHeight: CGFloat = 190

    private var progress: SetupProgress { model.progress }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The plate stays put while the page scrolls, so the garden is always in sight.
            plate.zIndex(1)  // the garden stands over the plate's edge onto the page
            if scrolls {
                ScrollViewReader { scroller in
                    ScrollView { page.id("top") }
                        .onChange(of: progress.current) { _, _ in
                            withAnimation(.easeInOut(duration: 0.35)) { scroller.scrollTo("top", anchor: .top) }
                        }
                }
            } else {
                page
            }
        }
        .frame(width: Self.width, height: scrolls ? Self.height : nil, alignment: .top)
        .background(DS.Surface.overlay)
        .grain(0.22)
        .onAppear { model.startWatching() }
        .onDisappear { model.stopWatching() }
        .onChange(of: model.tried) { _, _ in model.noteTried() }
        .onChange(of: progress.current) { _, _ in announce() }
    }

    // MARK: Plate

    /// The zine's cover move: a screen-printed plate across the top of the page, with the
    /// setup garden standing on its bottom edge, and a giant ghost word cut off by the page.
    private var plate: some View {
        ZStack(alignment: .bottomLeading) {
            ZStack(alignment: .bottomLeading) {
                ScreenPlate(ink: .sky, seed: 11)
                Text(progress.isReturning ? "AGAIN" : "HELLO").font(DS.Font.chunky(150)).foregroundStyle(plateGhost)
                    .fixedSize()
                    .offset(x: -14, y: 46)
                    .accessibilityHidden(true)
            }
            .frame(width: Self.width, height: Self.plateHeight)
            .clipped()
            IrisLogo(size: 30)
                .padding(.leading, 40).padding(.bottom, 26)
            GardenView(stage: gardenStage, progress: downloadFraction)
                .scaleEffect(1.16, anchor: .bottom)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .padding(.trailing, 24)
                .offset(y: 30)
        }
        .frame(width: Self.width, height: Self.plateHeight, alignment: .bottomLeading)
    }

    private var plateGhost: Color { DS.Surface.overlay.opacity(0.32) }

    private var gardenStage: GardenView.Stage { progress.garden }

    private var downloadFraction: Double {
        if case .downloading(let f) = progress.facts.activity { return f }
        return 0
    }

    // MARK: Page

    private var page: some View {
        VStack(alignment: .leading, spacing: 16) {
            RunningHead(items: ["Setup", progress.isReturning ? "One step" : "About five minutes", "Everything runs on this Mac"])
                .padding(.trailing, 120)
            HStack(alignment: .top, spacing: 26) {
                stepPage.frame(maxWidth: .infinity, alignment: .leading)
                HandLine(seed: 31, vertical: true).stroke(DS.Ink.i12.opacity(0.3), lineWidth: 1).frame(width: 1)
                StepList(progress: progress).frame(width: 196)
            }
            foot
        }
        .padding(.horizontal, 40)
        .padding(.top, 14)
        .padding(.bottom, 26)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder private var stepPage: some View {
        switch progress.current {
        case .allow: allowPage
        case .model: modelPage
        case .tryIt: tryPage
        case nil: donePage
        }
    }

    /// Big folio for the step, like the zine's page numbers; the way out only on a stuck step;
    /// Open at login once there's something to open.
    private var foot: some View {
        HStack(alignment: .lastTextBaseline) {
            Text("/0\(progress.current?.rawValue ?? 3)").font(DS.Font.display(46)).foregroundStyle(DS.Ink.i12)
                .accessibilityHidden(true)
            Spacer()
            if progress.offersSetUpLater {
                TactileButton(title: "Set up later", role: .ghost) { onLater() }
                    .accessibilityHint("Closes setup. The menu bar keeps where you stopped.")
            }
            if progress.current == .tryIt || progress.current == nil {
                VStack(alignment: .trailing, spacing: 4) {
                    HStack(spacing: 10) {
                        TactileSwitch(isOn: Binding(get: { settings.launchAtLogin }, set: { if model.frozen == nil { settings.setLaunchAtLogin($0) } }),
                                      label: "Open at login")
                        Text("Open at login").font(DS.Font.body).foregroundStyle(DS.Ink.i11)
                    }
                    // As in Settings: what macOS still needs (approving Iris in Login Items).
                    if let problem = settings.launchAtLoginProblem {
                        Text(problem).font(DS.Font.caption).foregroundStyle(DS.Hue.warnText)
                    }
                }
            }
        }
        .padding(.top, 6)
    }

    // MARK: Step 1

    @ViewBuilder private var allowPage: some View {
        let stuck = progress.status(of: .allow) == .stuck
        VStack(alignment: .leading, spacing: 14) {
            Headline(lines: progress.isReturning ? ["Allow", "✻ Iris again"] : ["Let Iris", "✻ in"], drawing: .hand)
            if stuck {
                ProblemLine(text: "the switch is on, but I can't see it")
                Text("macOS sometimes keeps an old copy of the permission. Clear it and add Iris again:")
                    .font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                NumberedSteps(items: ["In Accessibility, select Iris and press −.",
                                      "Press +, and choose Iris in Applications.",
                                      "Quit and reopen Iris."])
                HStack(spacing: 14) {
                    TactileButton(title: "Open System Settings", role: .primary) { model.openSystemSettings() }.fixedSize()
                    TactileButton(title: "Quit and reopen Iris", role: .secondary) { model.relaunch() }.fixedSize()
                }
            } else {
                Text(progress.isReturning
                     ? "macOS turned off Iris's permission, often after an update. Everything else is as you left it."
                     : "Iris reads the text box you're typing in and pastes the fix back. macOS asks you to allow that once.")
                    .font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 14) {
                    TactileButton(title: "Open System Settings", role: .primary) { model.openSystemSettings() }.fixedSize()
                    HandNote(text: "then turn Iris on", size: 19, curl: .left, seed: 5).accessibilityHidden(true)
                }
                Text("Privacy & Security › Accessibility › Iris. This page turns to step 2 by itself. On a company Mac, IT may need to allow it.")
                    .font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    // MARK: Step 2

    @ViewBuilder private var modelPage: some View {
        VStack(alignment: .leading, spacing: 10) {
            Headline(lines: ["Get", "✻ the model"], drawing: progress.facts.activity == .none ? nil : .model)
            switch progress.facts.activity {
            case .downloading(let fraction):
                Text("Downloading \(settings.model.plainName.lowercased()), one time.")
                    .font(DS.Font.body).foregroundStyle(DS.Ink.i11)
                ProgressTrack(fraction: fraction)
                Text(settings.model.downloaded(fraction)).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                Text("You can close this window. When the model's in, the menu bar flower will say so.")
                    .font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                TactileButton(title: EngineState.stopTitle, role: .secondary) { confirm(ModelDialogs.stop()) }.fixedSize()
            case .loading:
                HStack(spacing: 6) {
                    InkDots()
                    Text("Waking it up. A few seconds.").font(DS.Font.body).foregroundStyle(DS.Ink.i11)
                }
            case .failed(let failure):
                failed(failure)
            default:
                ModelChooser(settings: settings, frozen: model.frozen != nil)
                ModelPipsNote()
                HStack(spacing: 14) {
                    TactileButton(title: "Download \(settings.model.plainName.lowercased())", role: .primary, icon: "arrow.down") {
                        model.download()
                    }
                    .fixedSize()
                    Text("One time, \(settings.model.downloadSize).").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                }
            }
        }
    }

    /// The same words as everywhere else (`ProblemNote`, `ModelFailure.nextStep`). Here, on the
    /// download step itself, a model that isn't there is simply a download to try again.
    @ViewBuilder private func failed(_ reported: ModelFailure) -> some View {
        let failure = reported == .notDownloaded ? ModelFailure.other : reported
        ProblemLine(text: ProblemNote(failure).lead)
        if case .noRoom = failure {
            Text(failure.nextStep + " Iris tries again by itself once there's room.")
                .font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
            TactileButton(title: "Open Storage Settings", role: .primary) {
                if let url = URL(string: "x-apple.systempreferences:com.apple.settings.Storage") { NSWorkspace.shared.open(url) }
            }
            .fixedSize()
        } else {
            Text(failure.nextStep).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
            TactileButton(title: "Try again", role: .primary) { model.download() }.fixedSize()
        }
    }

    // MARK: Step 3

    private var tryPage: some View {
        VStack(alignment: .leading, spacing: DS.Space.md) {
            Headline(lines: ["Try", "✻ it once"], drawing: .tones)
            if model.frozen?.shortcutTaken ?? settings.openShortcutTaken {
                // This step is pressing the shortcut: if another app holds it, say so here.
                ProblemLine(text: "another app took these keys")
                Text("\(settings.openShortcut.display) is in use by another app, so it can't open Iris. Pick other keys, then come back.")
                    .font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                TactileButton(title: "Change shortcut", role: .primary) {
                    NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.shortcuts.rawValue)
                }
                .fixedSize()
            }
            keyLine("Click in the box and press", settings.openShortcut.keyRow)
            practiceBox
            keyLine("In the panel, press", KeyRow(keys: [(symbol: "↩", word: "return")], spoken: "Return"), "to replace it.")
            if let quick = settings.quickFixShortcut {
                keyLine("Skip the panel with Quick Fix:", quick.keyRow, meta: true)
            }
        }
    }

    private var practiceBox: some View {
        TextEditor(text: $model.practiceText)
            .font(DS.Font.body)
            .lineSpacing(5)
            .scrollContentBackground(.hidden)
            .padding(14)
            .frame(minHeight: 84, maxHeight: 110)
            .background(HandRect(seed: 85, wobble: 0.5, closed: true).fill(DS.Surface.well))
            .overlay(alignment: .topTrailing) {
                HandNote(text: "fix me", size: 20, curl: .down, reach: CGSize(width: 26, height: 24), flipped: true, seed: 9)
                    .offset(x: -6, y: -30)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
            .padding(.top, 18)
            .accessibilityLabel("Practice text")
    }

    // MARK: Done

    private var donePage: some View {
        VStack(alignment: .leading, spacing: DS.Space.md) {
            Headline(lines: ["All", "✻ set"], drawing: nil)
            if model.practiceText != OnboardingModel.practiceSample {
                Text(model.practiceText).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                    .padding(14).frame(maxWidth: .infinity, alignment: .leading)
                    .background(HandRect(seed: 85, wobble: 0.5, closed: true).fill(DS.Surface.well))
                DoneNote(text: "that's it", size: 26)
            }
            keyLine("Same keys in any app:", settings.openShortcut.keyRow)
            TactileButton(title: "Done", role: .primary) { onFinish() }.fixedSize()
                .padding(.top, DS.Space.xs)
        }
    }

    // MARK: Parts

    /// Words and keycaps on one line, or the keys on their own line when it doesn't fit.
    private func keyLine(_ before: String, _ keys: KeyRow, _ after: String? = nil, meta: Bool = false) -> some View {
        let font = meta ? DS.Font.caption : DS.Font.body
        let ink = meta ? DS.Ink.i10 : DS.Ink.i11
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                Text(before)
                keys
                if let after { Text(after) }
            }
            .fixedSize()
            VStack(alignment: .leading, spacing: 6) {
                Text(before).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    keys
                    if let after { Text(after).fixedSize(horizontal: false, vertical: true) }
                }
            }
        }
        .font(font)
        .foregroundStyle(ink)
        .accessibilityElement(children: .combine)
    }

    /// Says the new step when the page turns by itself, and moves VoiceOver to it.
    private func announce() {
        FloatingNote.announce(progress.announcement, priority: .high)
    }
}

/// A step's headline: the stair of condensed caps with ✻, and the step's drawing printed big
/// beside it, like the zine's brewers.
private struct Headline: View {
    let lines: [String]
    let drawing: RisoIcon?

    var body: some View {
        HStack(alignment: .top) {
            // Settings' stair headline, set bigger for a page with one thing on it.
            PageTitle(lines: lines, size: 38, step: 44, leading: -6, tracking: 0)
                .fixedSize()
            Spacer(minLength: 8)
            if let drawing {
                RisoDrawing(icon: drawing, size: 76).rotationEffect(.degrees(-6)).padding(.top, -4)
            }
        }
    }
}

/// What went wrong, in Iris's voice: the red ring round "?" and the note in red brush.
private struct ProblemLine: View {
    let text: String
    var body: some View {
        HStack(spacing: 8) {
            ToastMark(kind: .warning)
            Text(text).font(DS.Font.brush(24)).foregroundStyle(DS.Hue.redText).fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }
}

/// A fix as numbered steps, the numbers as small pink folios.
private struct NumberedSteps: View {
    let items: [String]
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(String(format: "%02d", i + 1)).font(DS.Font.caption).foregroundStyle(DS.Hue.pinkText)
                    Text(item).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }
}

/// The three steps as a numbered list on pen rules: where you are and what comes next. Not a
/// sidebar: nothing in it can be clicked, so no step can be skipped to.
private struct StepList: View {
    let progress: SetupProgress

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(SetupProgress.Step.allCases, id: \.self) { step in
                row(step)
                PenRule(seed: 60 + step.rawValue * 3)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Steps")
    }

    private func row(_ step: SetupProgress.Step) -> some View {
        let status = progress.status(of: step)
        return HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(String(format: "%02d", step.rawValue)).font(DS.Font.caption).foregroundStyle(DS.Hue.pinkText)
            VStack(alignment: .leading, spacing: 3) {
                Text(step.title).font(DS.Font.brush(21))
                    .foregroundStyle(status == .now || status == .stuck ? DS.Ink.i12 : DS.Ink.i10)
                    .background(alignment: .bottom) {
                        if status == .now || status == .stuck {
                            MarkerSwipe(height: 7, seed: 8 + step.rawValue).padding(.horizontal, -3).offset(y: -2)
                        }
                    }
                statusWord(status, step)
            }
        }
        .padding(.vertical, 10)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Step \(step.rawValue), \(step.title): \(spoken(status, step))")
    }

    @ViewBuilder private func statusWord(_ status: SetupProgress.Status, _ step: SetupProgress.Step) -> some View {
        switch status {
        case .done:
            HStack(spacing: 4) {
                HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                    .frame(width: 13, height: 10)
                Text("done").font(DS.Font.brush(18)).foregroundStyle(DS.Hue.pinkText)
            }
        case .now:
            Text("now").font(DS.Font.brush(18)).foregroundStyle(DS.Hue.pinkText)
        case .stuck:
            Text(step.stuckWord).font(DS.Font.brush(18)).foregroundStyle(DS.Hue.redText)
        case .later:
            Text("after \(step.rawValue - 1)".uppercased()).font(DS.Font.caption).tracking(1.3).foregroundStyle(DS.Ink.i10)
        }
    }

    private func spoken(_ status: SetupProgress.Status, _ step: SetupProgress.Step) -> String {
        switch status {
        case .done: "done"
        case .now: "current step"
        case .stuck: "needs attention"
        case .later: "after step \(step.rawValue - 1)"
        }
    }
}

/// The two sizes drawn side by side like Settings › Model & memory: the chosen one ringed in
/// pink, the one that suits this Mac marked recommended, Use this one under the other.
private struct ModelChooser: View {
    @ObservedObject var settings: AppSettings
    /// Made-up states draw nothing as on disk; the real window asks the disk.
    let frozen: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 18) {
            ForEach(Array(ModelChoice.offered(onDisk: onDisk).enumerated()), id: \.element) { i, choice in
                option(choice, index: i)
            }
        }
    }

    private func onDisk(_ choice: ModelChoice) -> Bool { !frozen && choice.isDownloaded }

    /// Made-up states are looked at, never acted on: they leave this Mac's settings alone.
    private func choose(_ choice: ModelChoice) { if !frozen { settings.model = choice } }

    private func option(_ choice: ModelChoice, index i: Int) -> some View {
        let chosen = settings.model == choice
        return VStack(alignment: .leading, spacing: 8) {
            ModelOption(choice: choice, index: i, place: .setup, chosen: chosen, onDisk: onDisk(choice)) { choose(choice) }
            if let strengths = ModelStrengths.of(choice) { ModelPips(strengths: strengths, seed: i).padding(.top, 2) }
            if !chosen { UseModelButton(choice: choice) { choose(choice) } }
        }
        .overlay(alignment: .topLeading) {
            if choice == .recommended { RecommendedMark().scaleEffect(0.85, anchor: .topLeading).offset(x: 84, y: 0) }
        }
        .frame(width: 190, alignment: .leading)
    }
}
