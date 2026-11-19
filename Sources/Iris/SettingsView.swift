import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// Settings: one printed page. A rail of pages and the chosen page share the paper, with a
/// faint pen line between them (no sheet edge, no cards: see DESIGN.md). Each section: a
/// heading with its actions at the right, then its rows or a table.
struct SettingsView: View {
    enum Page: String, CaseIterable, Identifiable {
        case general, shortcuts, tones, words, apps, model, privacy, updates, about
        var id: String { rawValue }
        /// Pages share names with their riso icons.
        var icon: RisoIcon { RisoIcon(rawValue: rawValue)! }
        var title: String {
            switch self {
            case .general: "General"
            case .shortcuts: "Shortcuts"
            case .tones: "Tones"
            case .words: "Words"
            case .apps: "Apps"
            case .model: "Model & memory"
            case .privacy: "Privacy"
            case .updates: "Updates"
            case .about: "About"
            }
        }
        /// Page titles step right, like the poster's headline.
        var heading: [String] {
            switch self {
            case .general: ["How Iris", "behaves"]
            case .shortcuts: ["Keys for", "your hands"]
            case .tones: ["Tones based", "on your words"]
            case .words: ["Words to", "never change"]
            case .apps: ["Where Iris", "works"]
            case .model: ["The model", "on this Mac"]
            case .privacy: ["Nothing", "leaves here"]
            case .updates: ["What's new", "in Iris"]
            case .about: ["Who made", "Iris"]
            }
        }
        /// The running head over the title: which page, what's on it, one plain fact.
        var runningHead: [String] {
            let number = String(format: "%02d of %02d", (Page.allCases.firstIndex(of: self) ?? 0) + 1, Page.allCases.count)
            switch self {
            case .general: return [number, "When and where Iris shows up", "Changes save right away"]
            case .shortcuts: return [number, "The keys that call Iris", "They work in any app"]
            case .tones: return [number, "How each rewrite sounds", "Names and links stay as written"]
            case .words: return [number, "Names and terms to keep", "Iris never changes them"]
            case .apps: return [number, "Where Iris reads text", "Turn it off in any app"]
            case .model: return [number, "What runs on this Mac", "Downloaded once"]
            case .privacy: return [number, "What Iris can see", "Your text stays here"]
            case .updates: return [number, "New versions and what changed", "Iris looks only when you ask"]
            case .about: return [number, "Who made Iris and why", "Free and open source"]
            }
        }
        /// The page's lettering, top right: a ghost word behind in one of the pale inks, a
        /// hand-lettered word in front, both centred on `letteringInset`. Pick a ghost by its
        /// width as well as its meaning: it must print within `ghostWidths`, like the others
        /// (KEYS, VOICE and APPS printed far narrower, so the corner jumped from page to page).
        /// The self-check measures them.
        var lettering: (ghost: String, word: String, ink: GhostWord.Ink) {
            switch self {
            case .general: ("Habits", "Basics", .khaki)
            case .shortcuts: ("Touch", "Press", .sky)
            case .tones: ("Sound", "Tones", .pink)
            case .words: ("Words", "Keep", .tan)
            case .apps: ("Places", "Where", .sky)
            case .model: ("Local", "Model", .khaki)
            case .privacy: ("Yours", "Private", .pink)
            case .updates: ("New", "Latest", .sky)
            case .about: ("Story", "Thanks", .tan)
            }
        }
    }

    /// The page's size under the window's title bar. Its height is fixed and short enough to
    /// leave the desktop showing around it on a 13-inch screen: a page with more on it scrolls
    /// inside, the window never grows. On a screen too small even for that, it's as tall as
    /// the screen has room for.
    static let size = CGSize(width: 940, height: min(740, max(440, (NSScreen.main?.visibleFrame.height ?? 740) - 60)))

    /// A page's lettering stands on one centre line, this far in from the page's right edge:
    /// the ghost word and the hand word are both centred on it, whatever their lengths.
    /// Pinning either by an edge lets a short word drift off the other's middle.
    static let letteringInset: CGFloat = 87
    /// The ghost word's size, and how wide every page's ghost must print at it (see `lettering`).
    static let ghostSize: CGFloat = 124
    static let ghostWidths: ClosedRange<CGFloat> = 168...192

    @State var page: Page = .general
    /// Off only for design snapshots (they can't draw scroll views).
    var scrolls = true
    /// What's new, on the Updates page: made-up issues in snapshots; nil reads the bundled one
    /// each time the page is drawn.
    var changelog: Changelog? = nil

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            rail
                // Opened to a page from elsewhere (setup's Change shortcut, the slip).
                .onReceive(NotificationCenter.default.publisher(for: .irisTurnSettingsPage)) { note in
                    if let id = note.object as? String, let target = Page(rawValue: id) { page = target }
                }
            // One printed page: the rail and the content share the paper, with a faint pen
            // line between them. No sheet edge, no shadow (see DESIGN.md).
            HandLine(seed: 3, wobble: 0.6, vertical: true)
                .stroke(DS.Ink.i12.opacity(0.3), style: StrokeStyle(lineWidth: 1, lineCap: .round))
                .frame(width: 3)
                .padding(.vertical, 24)
                // A line of small caps running up the rule, like the zine's text up a photo's edge:
                // on the rule itself (its paper strip breaks the line around the words), between
                // the list of pages (it ends 399 from the top) and the page's pink foot (114 tall),
                // ending a little above the foot so the foot never runs into it, and starting no
                // higher than the list's end. The words run about 100 each way. Report a bug, at the rail's foot, sits to their left. The page says
                // "your words stay on this Mac" at its foot; this says what Iris is. A window too
                // short for the words between the two leaves them out.
                .overlay {
                    GeometryReader { geo in
                        // Laid over the rule with its padding, so this is the window's full height.
                        let centre = geo.size.height - 114 - 24 - 101
                        if centre - 101 >= 399 {
                            EdgeText(text: "Iris ✻ grammar and tone")
                                .position(x: geo.size.width / 2, y: centre)
                        }
                    }
                }
                .accessibilityHidden(true)
            content
        }
        .frame(width: Self.size.width, height: scrolls ? Self.size.height : nil)
        .background(DS.Surface.overlay.grain(0.22).ignoresSafeArea())
    }

    private var content: some View {
        Group {
            // A scroll view per page, so a page you turn to opens at its top, never part way
            // down where the last page was left.
            // A short page stretches to the window's height, so its foot always sits at the
            // bottom edge; a long one runs past it and scrolls, the foot at its end.
            if scrolls {
                GeometryReader { window in
                    ScrollView { pageBody.frame(minHeight: window.size.height) }.id(page)
                }
            } else { pageBody }
        }
        .frame(maxWidth: .infinity, maxHeight: scrolls ? .infinity : nil, alignment: .top)
        .clipped()  // the page's lettering runs off this edge
    }

    private var rail: some View {
        VStack(alignment: .leading, spacing: 1) {
            // No flower on the wordmark: a page's own flower (About, the model resting, an empty
            // list) is the one in view.
            IrisLogo(size: 26).padding(.leading, 8).padding(.top, 40).padding(.bottom, 18)
            ForEach(Page.allCases) { item in
                RailRow(title: item.title, icon: item.icon, current: item == page) { page = item }
            }
            Spacer(minLength: DS.Space.md)
            // Not a page: it opens GitHub, so it stands apart at the foot, on every page.
            ReportBugRow().frame(width: 194, alignment: .leading).padding(.bottom, 26)
        }
        .padding(.horizontal, 10)
        .frame(width: 214)
    }

    private var pageBody: some View {
        VStack(spacing: 0) {
            page_
            Spacer(minLength: 0)
            // The page ends on a solid pink foot with its folio, like the zine's back page.
            PageFoot(folio: String(format: "/%02d", (Page.allCases.firstIndex(of: page) ?? 0) + 1),
                     signOff: ["your words", "— stay on", "this Mac"])
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { Self.footFrame = $0 }
        }
    }

    /// Where the page's foot was last drawn, in the window (top-left origin), so the
    /// self-check can see it reaches the bottom edge.
    @MainActor static var footFrame: CGRect = .zero
    /// Where the page's ghost word and hand word were last drawn, in the window, so the
    /// self-check can see they share a centre line and the ghost prints as wide as the others.
    @MainActor static var ghostFrame: CGRect = .zero
    @MainActor static var handWordFrame: CGRect = .zero

    private var page_: some View {
        VStack(alignment: .leading, spacing: 30) {
            VStack(alignment: .leading, spacing: 26) {
                RunningHead(items: page.runningHead).padding(.trailing, 150)
                PageTitle(lines: page.heading)
            }
            pageContent
        }
        .padding(.horizontal, 44)
        .padding(.top, 44)
        .padding(.bottom, 52)
        // The ghost word prints behind everything on the page. A zero-wide frame centres it on
        // the lettering's line however wide it prints.
        .background(alignment: .topTrailing) {
            GhostWord(word: page.lettering.ghost, size: Self.ghostSize, ink: page.lettering.ink)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { Self.ghostFrame = $0 }
                .frame(width: 0)
                // A ghost of three letters or fewer prints on one line (`GhostWord`): it stands
                // lower, so the hand word still crosses its middle.
                .offset(x: -Self.letteringInset, y: page.lettering.ghost.count < 4 ? 57 : 14)
                .allowsHitTesting(false)
        }
        // The hand-lettered word sits across the ghost's middle, on the same line, like the
        // zine's GRiND over CRUSH. Long words letter smaller, so they clear the title.
        .overlay(alignment: .topTrailing) {
            HandWord(word: page.lettering.word, size: page.lettering.word.count > 5 ? 41 : 50,
                     seed: (Page.allCases.firstIndex(of: page) ?? 0) * 7)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { Self.handWordFrame = $0 }
                .frame(width: 0)
                .offset(x: -Self.letteringInset, y: 102)
                .allowsHitTesting(false)
        }
        .frame(maxWidth: 700, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder private var pageContent: some View {
        switch page {
        case .general: GeneralPage()
        case .shortcuts: ShortcutsPage()
        case .tones: TonesPage()
        case .words: WordsPage()
        case .apps: AppsPage()
        case .model: ModelPage()
        case .privacy: PrivacyPage()
        case .updates: UpdatesPage(changelog: changelog)
        case .about: AboutPage()
        }
    }
}

// MARK: - Shell pieces

struct RailRow: View {
    let title: String
    let icon: RisoIcon
    let current: Bool
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                RisoIconView(icon: icon, size: 24)
                Text(title).font(DS.Font.body).foregroundStyle(current || hovering ? DS.Ink.i12 : DS.Ink.i11)
                Spacer()
            }
            .padding(.horizontal, 10)
            .frame(height: 32)
            // Current page two steps above the rail, hover one, each fading in place.
            .penWash(hover: hovering, current: current, seed: title.count * 5 + 1)
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.99))
        .onHover { hovering = $0 }
        .animation(DS.easeOut, value: hovering)
    }
}

/// Report a bug, at the rail's foot under a short pen line: a ladybird from the garden, and
/// the turned-up arrow every link that opens the browser has. It opens a new GitHub issue with
/// this Iris and macOS filled in (`ProblemReport`). Never shown as the current page.
struct ReportBugRow: View {
    @State private var hovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            PenRule(seed: 777, ink: DS.Ink.i12.opacity(0.3)).frame(width: 150).padding(.leading, 10)
            Button {
                NSWorkspace.shared.open(ProblemReport.url(repo: Updater.repo, iris: Updater.versionLabel, macOS: Updater.macOSLabel))
            } label: {
                HStack(spacing: 10) {
                    RisoIconView(icon: .bug, size: 24)
                    HStack(spacing: 4) {
                        Text("Report a bug").font(DS.Font.body).foregroundStyle(hovering ? DS.Ink.i12 : DS.Ink.i11)
                        PenArrow(seed: 31, weight: PenArrow.besideWordsWeight * 0.9, right: true)
                            .frame(width: PenArrow.besideWords.width * 0.8, height: PenArrow.besideWords.height * 0.8)
                            .rotationEffect(.degrees(-35))
                    }
                }
                .padding(.leading, 10).padding(.trailing, 8)
                .frame(height: 32)
                .penWash(hover: hovering, seed: 41)
                .contentShape(Rectangle())
            }
            .buttonStyle(SqueezeStyle(scale: 0.99))
            .onHover { hovering = $0 }
            .animation(DS.easeOut, value: hovering)
            .accessibilityLabel("Report a bug")
            .accessibilityHint("Opens GitHub in your browser, with your Iris and macOS versions filled in.")
        }
    }
}

/// The poster headline: condensed caps that step right. Sized to introduce the page, not
/// to outshout the settings under it.
struct PageTitle: View {
    let lines: [String]
    /// Settings' size. Setup's headline (`OnboardingView`) is this one set bigger.
    var size: CGFloat = 30
    /// How far each line steps right of the one above.
    var step: CGFloat = 36
    var leading: CGFloat = -8
    var tracking: CGFloat = -0.8
    var body: some View {
        // Tight leading, like the zine's stacked caps: the lines almost touch.
        VStack(alignment: .leading, spacing: leading) {
            ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                Text(line).font(DS.Font.display(size)).textCase(.uppercase).tracking(tracking)
                    .foregroundStyle(DS.Ink.i12)
                    .padding(.leading, CGFloat(i) * step)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// A section like the poster's spec list: a pink folio number, the heading in brush, its
/// description, actions at the right, then an ink rule and the settings printed under it.
struct Section<Actions: View, Content: View>: View {
    var number: Int? = nil
    let title: String
    var detail: String? = nil
    @ViewBuilder var actions: Actions
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: DS.Space.md) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                if let number {
                    Text(String(format: "%02d", number)).font(DS.Font.folio).foregroundStyle(DS.Hue.pinkText)
                        .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(DS.Font.brush(24)).foregroundStyle(DS.Ink.i12)
                        .accessibilityAddTraits(.isHeader)
                    if let detail {
                        Text(detail).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: DS.Space.lg)
                actions
            }
            // A note-only section (nothing printed under it) gets no rule to hang from.
            if Content.self != EmptyView.self {
                PenRule(seed: title.count * 7 + (number ?? 0), ink: DS.Ink.i12.opacity(0.85), width: 1.2)
                content.padding(.top, 2)
            }
        }
    }
}

/// Settings that belong together, printed straight on the sheet (no card).
struct SheetGroup<Content: View>: View {
    @ViewBuilder let content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: DS.Space.md) { content }
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

extension Section where Actions == EmptyView {
    init(number: Int? = nil, title: String, detail: String? = nil, @ViewBuilder content: () -> Content) {
        self.number = number
        self.title = title
        self.detail = detail
        self.actions = EmptyView()
        self.content = content()
    }
}

/// One setting: label and description on the left, its control at the right.
struct SettingRow<Control: View>: View {
    let title: String
    var detail: String? = nil
    /// The red pen, when the row is an answer that went wrong (Updates).
    var titleInk: Color = DS.Ink.i12
    /// Drawn in place of the title and the detail, which VoiceOver still reads (Updates writes
    /// its answer in as the letter from GitHub opens).
    var titleView: AnyView? = nil
    var detailView: AnyView? = nil
    @ViewBuilder let control: Control

    var body: some View {
        HStack(alignment: .center, spacing: DS.Space.lg) {
            VStack(alignment: .leading, spacing: 3) {
                if let titleView {
                    titleView.accessibilityElement(children: .ignore).accessibilityLabel(title)
                } else {
                    Text(title).font(DS.Font.body).foregroundStyle(titleInk)
                }
                if let detailView {
                    detailView.accessibilityElement(children: .ignore).accessibilityLabel(detail ?? "")
                } else if let detail {
                    Text(detail).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            control
        }
    }
}

private struct Hairline: View {
    var seed: Int
    init(seed: Int = #line) { self.seed = seed }
    var body: some View { PenRule(seed: seed) }
}

// MARK: - General

struct GeneralPage: View {
    @ObservedObject private var settings = AppSettings.shared

    var body: some View {
        Section(number: 1, title: "The panel", detail: "What you see when you press the shortcut.") {
            SheetGroup {
                SettingRow(title: "Where it opens", detail: "Next to your text box. It moves to the other side when there's no room.") {
                    InkChoice(options: [(value: AppSettings.PanelPlacement.aboveField, label: "Above"),
                                        (value: .belowField, label: "Below"),
                                        (value: .nearPointer, label: "By the mouse")],
                              selection: $settings.placement, name: "Where it opens")
                }
                Hairline()
                SettingRow(title: "Starts in", detail: "Fix keeps your words and only corrects mistakes.") {
                    Dropdown(options: settings.modes.map { (value: $0.id, label: $0.label) },
                             selection: $settings.tones.defaultID, width: 170)
                }
                Hairline()
                SettingRow(title: "Show what changed", detail: "New words in pink, removed ones crossed out.") {
                    TactileSwitch(isOn: $settings.showChanges, label: "Show what changed")
                }
                Hairline()
                SettingRow(title: "Nothing to fix", detail: "Show the panel, or a small message that fades.") {
                    InkChoice(options: [(value: true, label: "Panel"), (value: false, label: "Message")],
                              selection: $settings.showPanelWhenClean, name: "Nothing to fix")
                }
            }
        }
        NotesSection()
        Section(number: 3, title: "Your Mac") {
            SheetGroup {
                SettingRow(title: "Open at login", detail: "Start Iris in the menu bar when you log in.") {
                    TactileSwitch(isOn: Binding(get: { settings.launchAtLogin },
                                                set: { settings.setLaunchAtLogin($0) }),
                                  label: "Open at login")
                }
                if let problem = settings.launchAtLoginProblem {
                    Text(problem).font(DS.Font.caption).foregroundStyle(DS.Hue.warnText)
                }
                Hairline()
                SettingRow(title: "First-time setup", detail: "Permission, model, and a practice box.") {
                    TactileButton(title: "Open setup", role: .secondary) {
                        NotificationCenter.default.post(name: .irisShowOnboarding, object: nil)
                    }
                }
            }
        }
    }
}

// MARK: - Shortcuts

struct ShortcutsPage: View {
    @ObservedObject private var settings = AppSettings.shared
    var forcedRecording = false

    var body: some View {
        Section(number: 1, title: "Shortcuts", detail: "They work in any app. To use your own, click the keys, then press the combination you want.") {
            TactileButton(title: "Restore defaults", role: .secondary, compact: true) { settings.resetShortcuts() }
        } content: {
            // Words on the left, the keys on the right, like every other setting row.
            SheetGroup {
                shortcutRow("Open Iris", detail: {
                    Text("Checks the box you're typing in.").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    HStack(spacing: 4) {
                        Keycap(key: "⌘"); Keycap(key: "Z")
                        Text("undoes a Replace.").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Command Z undoes a Replace.")
                }) {
                    ShortcutRecorder(combo: settings.openShortcut, other: settings.quickFixShortcut, action: .open) { combo in
                        if let combo { settings.openShortcut = combo }
                    }
                }
                Hairline()
                shortcutRow("Quick Fix", detail: {
                    Text("Fixes and replaces at once. Big rewrites still open the panel.").font(DS.Font.caption)
                        .foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
                }) {
                    ShortcutRecorder(combo: settings.quickFixShortcut, other: settings.openShortcut, action: .quickFix,
                                     allowsClear: true, forcedRecording: forcedRecording) { combo in
                        settings.quickFixShortcut = combo
                    }
                }
            }
        }
        Section(number: 2, title: "In the panel", detail: "Only while the panel is open, so they never clash with other apps.") {
            VStack(spacing: 0) {
                let rows: [(String, String)] = [
                    ("↵", "Replace your text with the result"), ("⇥", "Next tone (⇧⇥ goes back)"),
                    ("1–\(ToneList.panelRoom)", "Jump to a tone"), ("⌘C", "Copy the result"), ("⌘R", "Try again: write a new version"),
                    ("esc", "Close without changing anything"),
                ]
                ForEach(Array(rows.enumerated()), id: \.offset) { i, row in
                    TableRow(last: i == rows.count - 1, seed: i) {
                        HStack(spacing: 3) { ForEach(Array(Keycap.split(row.0).enumerated()), id: \.offset) { Keycap(key: $0.element) } }
                            .frame(width: 56, alignment: .leading)
                        Text(row.1).font(DS.Font.body).foregroundStyle(DS.Ink.i11)
                        Spacer()
                    }
                }
            }
        }
    }

    private func shortcutRow<D: View, C: View>(_ title: String, @ViewBuilder detail: () -> D,
                                               @ViewBuilder _ recorder: () -> C) -> some View {
        HStack(alignment: .top, spacing: DS.Space.lg) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                detail()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            recorder()
        }
    }
}

// MARK: - Tones

struct TonesPage: View {
    @ObservedObject private var settings = AppSettings.shared
    @State private var name = ""
    @State private var instruction = ""
    @State private var newRule = ""

    var body: some View {
        Section(number: 1, title: "Tones", detail: "The panel has room for Fix and \(ToneList.panelRoom - 1) tones: the first ones checked here, in this order. Their number keys pick them.") {
            VStack(spacing: 0) {
                TableHeader(columns: [("Key", 28), ("Tone", 118), ("How it sounds", nil), ("", 72), ("Show", 34)])
                TableRow {
                    Text("1").font(DS.Font.caption).foregroundStyle(DS.Ink.i10).frame(width: 28, alignment: .leading)
                    InkLabel(text: "Fix", size: 12).frame(width: 118, alignment: .leading)
                    Text("Corrects mistakes, keeps your words").font(DS.Font.caption).foregroundStyle(DS.Ink.i10).lineLimit(1)
                    Spacer(minLength: 0)
                    Spacer().frame(width: 72)
                    TactileCheckbox(isOn: .constant(true), label: "Fix is always shown").disabled(true).opacity(0.6)
                        .help("Fix is always in the panel").frame(width: 34, alignment: .leading)
                }
                let tones = settings.tones.all
                ForEach(Array(tones.enumerated()), id: \.element.id) { index, tone in
                    toneRow(tone, index: index, count: tones.count)
                }
            }
        }
        Section(number: 2, title: "Add a tone", detail: "Say how it should sound, e.g. \"Customer update: polite, says what happens next\".") {
            SheetGroup {
                WellField(placeholder: "Name", text: $name)
                WellField(placeholder: "How should it sound?", text: $instruction)
                HStack {
                    Spacer()
                    TactileButton(title: "Add tone", role: .primary, icon: "plus") {
                        settings.tones.add(name: name.trimmingCharacters(in: .whitespaces),
                                         instruction: instruction.trimmingCharacters(in: .whitespacesAndNewlines))
                        name = ""
                        instruction = ""
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || instruction.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        Section(number: 3, title: "Your style", detail: "Say it the way you'd tell a friend. Every tone follows these; Fix only corrects mistakes.") {
            SheetGroup {
                ForEach(Array(settings.styleRules.enumerated()), id: \.offset) { i, rule in
                    if i > 0 { Hairline(seed: 60 + i) }
                    HStack(spacing: 10) {
                        HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 1.9, lineCap: .round, lineJoin: .round))
                            .frame(width: 13, height: 10)
                            .accessibilityHidden(true)
                        Text(rule).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                        Spacer()
                        TactileButton(title: "Remove", role: .ghost, compact: true) {
                            settings.styleRules.removeAll { $0 == rule }
                        }
                    }
                }
                HStack(spacing: DS.Space.sm) {
                    WellField(placeholder: "e.g. keep my lowercase, never \"kindly\"", text: $newRule, onSubmit: addRule)
                    TactileButton(title: "Add", role: .secondary, action: addRule)
                        .disabled(newRule.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .padding(.top, settings.styleRules.isEmpty ? 0 : 4)
            }
        }
    }

    private func addRule() {
        let rule = newRule.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rule.isEmpty, !settings.styleRules.contains(rule) else { return }
        settings.styleRules.append(rule)
        newRule = ""
    }

    private func toneRow(_ tone: Mode, index: Int, count: Int) -> some View {
        let visible = !settings.tones.hidden.contains(tone.id)
        let key = settings.tones.key(of: tone.id)
        return TableRow(last: index == count - 1, dimmed: !visible, seed: index + 1) {
            Text(key.map { "\($0)" } ?? "–").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                .frame(width: 28, alignment: .leading)
            InkLabel(text: tone.label, size: 12).frame(width: 118, alignment: .leading)
            Text(tone.instruction).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).lineLimit(1).truncationMode(.tail)
                .help(tone.instruction)
            Spacer(minLength: 0)
            HStack(spacing: 2) {
                IconButton(symbol: "chevron.up", label: "Move \(tone.label) up") { settings.tones.move(tone.id, by: -1) }
                    .disabled(index == 0)
                IconButton(symbol: "chevron.down", label: "Move \(tone.label) down") { settings.tones.move(tone.id, by: 1) }
                    .disabled(index == count - 1)
                if tone.isCustom {
                    IconButton(symbol: "trash", label: "Delete \(tone.label)") { settings.tones.remove(tone.id) }
                }
            }
            .frame(width: 72, alignment: .leading)
            TactileCheckbox(isOn: Binding(
                get: { visible },
                set: { on in if on { settings.tones.hidden.remove(tone.id) } else { settings.tones.hidden.insert(tone.id) } }),
                label: "Show \(tone.label)")
                .frame(width: 34, alignment: .leading)
        }
    }
}

// MARK: - Words

struct WordsPage: View {
    @ObservedObject private var settings = AppSettings.shared
    @State private var newWord = ""

    var body: some View {
        Section(number: 1, title: "Never change", detail: "Iris keeps these exactly as written: product names, SKUs, people's names.") {
            HStack(spacing: DS.Space.sm) {
                WellField(placeholder: "Add a word or phrase", text: $newWord, onSubmit: add)
                TactileButton(title: "Add", role: .primary, action: add)
                    .disabled(newWord.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            if settings.glossary.isEmpty {
                EmptyState(lines: ["No words yet.", "Add the names Iris should never touch."])
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(settings.glossary.enumerated()), id: \.element) { i, word in
                        TableRow(last: i == settings.glossary.count - 1, seed: i) {
                            Text(word).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                            Spacer()
                            TactileButton(title: "Remove", role: .ghost, compact: true) {
                                settings.glossary.removeAll { $0 == word }
                            }
                        }
                    }
                }
            }
        }
        Section(number: 2, title: "Always protected", detail: "No need to add these: @mentions, #channels, :emoji:, links, email addresses and `code` are never changed.") {
            EmptyView()
        }
    }

    private func add() {
        let word = newWord.trimmingCharacters(in: .whitespaces)
        guard !word.isEmpty, !settings.glossary.contains(word) else { return }
        settings.glossary.append(word)
        newWord = ""
    }
}

struct EmptyState: View {
    let lines: [String]
    var body: some View {
        VStack(spacing: 4) {
            // The bloom alone: at this size the whole flower is a thin sprig.
            Image(nsImage: RisoArt.flower(points: 28, crop: .bloom)).resizable().frame(width: 28, height: 28)
                .padding(.bottom, 4)
            ForEach(lines, id: \.self) { Text($0).font(DS.Font.body).foregroundStyle(DS.Ink.i11) }
        }
        .frame(maxWidth: .infinity)
        .padding(32)
        .background(HandRect(seed: 91, wobble: 0.6, closed: true).fill(DS.Ink.i2))
    }
}

// MARK: - Apps

struct AppsPage: View {
    @ObservedObject private var settings = AppSettings.shared

    var body: some View {
        Section(number: 1, title: "Turned off in", detail: "Iris won't read text in these apps. Password managers are on the list from the start.") {
            ActionMenuButton(title: "Turn off in an app", items: appItems) { item in
                if let app = appRef(item), !settings.excludedApps.contains(app) { settings.excludedApps.append(app) }
            }
        } content: {
            // Laid out like the zine's brewing methods: each app big, its name as an ink label,
            // a brush X over it because Iris is turned off there.
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 4), alignment: .leading, spacing: 22) {
                ForEach(Array(settings.excludedApps.enumerated()), id: \.element.id) { i, app in
                    VStack(spacing: 9) {
                        AppIconView(bundleID: app.bundleID, size: 46)
                            .overlay { BrushCross(seed: i * 5 + 2).frame(width: 56, height: 56) }
                            .rotationEffect(.degrees(i.isMultiple(of: 2) ? -4 : 3))
                            .frame(height: 60)
                        InkLabel(text: app.name, size: 10.5)
                        TactileButton(title: "Remove", role: .ghost, compact: true) { settings.excludedApps.removeAll { $0 == app } }
                            .accessibilityLabel("Remove \(app.name)")
                    }
                    .frame(maxWidth: .infinity)
                }
            }
            .padding(.top, 6)
        }
        Section(number: 2, title: "Start in a tone", detail: "For example Professional in Mail. Other apps open in the tone from General.") {
            ActionMenuButton(title: "Pick an app", items: appItems) { item in
                if let app = appRef(item), settings.tones.perApp[app.bundleID] == nil { settings.tones.perApp[app.bundleID] = Mode.fix.id }
            }
        } content: {
            if settings.tones.perApp.isEmpty {
                EmptyState(lines: ["Every app opens in the same tone.", "Pick an app to give it its own."])
            } else {
                VStack(spacing: 0) {
                    let ids = settings.tones.perApp.keys.sorted()
                    ForEach(Array(ids.enumerated()), id: \.element) { i, bundleID in
                        TableRow(last: i == ids.count - 1, seed: i) {
                            AppIconView(bundleID: bundleID)
                            Text(AppIconView.name(for: bundleID)).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                            Spacer()
                            Dropdown(options: settings.modes.map { (value: $0.id, label: $0.label) },
                                     selection: Binding(get: { settings.tones.perApp[bundleID] ?? Mode.fix.id },
                                                        set: { settings.tones.perApp[bundleID] = $0 }), width: 150)
                            TactileButton(title: "Remove", role: .ghost, compact: true) { settings.tones.perApp[bundleID] = nil }
                        }
                    }
                }
            }
        }
    }

    /// Every app in the Applications folders, open or not (Mail and Notes are often closed
    /// when you set this up). Type a letter in the menu to jump.
    private func appItems() -> [MenuItem] {
        // Each app with its own icon, so it's found by sight as well as by name.
        InstalledApps.all.map { MenuItem(id: $0.bundleID, label: $0.name, appIcon: $0.bundleID) }
            + [MenuItem(id: "::choose", label: "Choose another app…", risoIcon: .apps)]
    }

    private func appRef(_ item: MenuItem) -> AppSettings.AppRef? {
        if item.id == "::choose" {
            let panel = NSOpenPanel()
            panel.directoryURL = URL(fileURLWithPath: "/Applications")
            panel.allowedContentTypes = [.application]
            guard panel.runModal() == .OK, let url = panel.url, let id = Bundle(url: url)?.bundleIdentifier else { return nil }
            return AppSettings.AppRef(bundleID: id, name: InstalledApps.name(at: url.path))
        }
        return AppSettings.AppRef(bundleID: item.id, name: item.label)
    }
}

struct AppIconView: View {
    let bundleID: String
    var size: CGFloat = 18
    var body: some View {
        Group {
            if let icon = Self.icon(bundleID, size: size) {
                Image(nsImage: icon).resizable()
            } else {
                // An app that isn't on this Mac: Iris's own apps icon, faded.
                Image(nsImage: RisoIcon.apps.image()).interpolation(.none).resizable().opacity(0.45)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }

    /// Each app's icon drawn once per size and kept, so a menu of every installed app opens
    /// and scrolls without redrawing them.
    @MainActor private static var cache: [String: NSImage] = [:]
    @MainActor private static func icon(_ bundleID: String, size: CGFloat) -> NSImage? {
        let key = "\(bundleID)-\(Int(size))"
        if let cached = cache[key] { return cached }
        guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else { return nil }
        let icon = NSWorkspace.shared.icon(forFile: url.path).flattened(points: size)
        cache[key] = icon
        return icon
    }

    static func name(for bundleID: String) -> String {
        guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else { return bundleID }
        return InstalledApps.name(at: url.path)
    }
}

// MARK: - Model

/// The model page: the model's state printed on a sky band like the menu bar slip's, then two
/// sections. "Which model": the two models drawn side by side like the zine's brewers (the chosen
/// one ringed in pink pen, the other with Use this one, the one that suits this Mac
/// marked "recommended"). "Memory": two plain rows.
struct ModelPage: View {
    /// Snapshots: a made-up state, chosen model and disk, and the Delete the pointer is on.
    @Environment(\.designState) private var design
    @Environment(\.confirm) private var confirm
    private var frozen: (state: EngineState, chosen: ModelChoice, onDisk: Set<ModelChoice>)? { design?.model }

    @ObservedObject private var settings = AppSettings.shared
    @ObservedObject private var engine = EngineMonitor.shared
    /// The model whose Delete the pointer is on: its drawing is crossed out, like the zine's cup.
    @State private var crossing: ModelChoice?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var state: EngineState { frozen?.state ?? engine.state }
    private var chosenModel: ModelChoice { frozen?.chosen ?? settings.model }
    private func onDisk(_ choice: ModelChoice) -> Bool { frozen.map { $0.onDisk.contains(choice) } ?? choice.isDownloaded }

    var body: some View {
        VStack(alignment: .leading, spacing: 30) {
            band
            if case .downloading(let fraction) = state {
                VStack(alignment: .leading, spacing: 6) {
                    ProgressTrack(fraction: fraction)
                    Text(chosenModel.downloaded(fraction)).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                }
                .padding(.top, -14)
            } else if case .failed = state {
                Text(state.detail).font(DS.Font.caption).foregroundStyle(DS.Hue.redText)
                    .fixedSize(horizontal: false, vertical: true).padding(.top, -14)
            }
            Section(number: 1, title: "Which model",
                    detail: "Each runs on this Mac. Iris downloads a model the first time you use it.") { models }
            Section(number: 2, title: "Memory") { memory }
        }
    }

    private var models: some View {
        let offered = ModelChoice.offered(onDisk: onDisk)
        let earlier = offered.filter { !$0.isTrained }
        return VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top, spacing: 0) {
                ForEach(Array(offered.filter(\.isTrained).enumerated()), id: \.element) { i, choice in
                    model(choice, index: i)
                }
            }
            ModelPipsNote()
            // Google's models Iris used before, while they're still on this Mac: smaller, so the
            // choice stays the two above. Use one, or delete it to free the disk.
            if !earlier.isEmpty {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Earlier models on this Mac").font(DS.Font.brush(20)).foregroundStyle(DS.Ink.i12)
                    HStack(alignment: .top, spacing: 0) {
                        ForEach(Array(earlier.enumerated()), id: \.element) { i, choice in
                            model(choice, index: i + 2, place: .setup)
                        }
                    }
                }
                .padding(.top, 8)
            }
        }
    }

    private var memory: some View {
        SheetGroup {
            SettingRow(title: "Load when Iris starts") {
                TactileSwitch(isOn: $settings.loadAtLaunch, label: "Load when Iris starts")
            }
            PenRule()
            SettingRow(title: "Free memory after") {
                Dropdown(options: AppSettings.idleChoices.map { (value: $0, label: $0 < 60 ? "\($0) min" : $0 == 60 ? "1 hour" : "\($0 / 60) hours") },
                         selection: $settings.idleMinutes, width: 132, icon: .clock)
            }
        }
    }

    /// The state as a ghost word in paper on a sky band (it runs as a ticker when it's too long),
    /// the command that applies now at its top left, the garden standing on its bottom edge.
    private var band: some View {
        ZStack(alignment: .topLeading) {
            StatusBand(word: state.title, height: 112, size: 84, drop: 22, frozenAt: frozen == nil ? nil : 0)
            statusAction.padding(.leading, 14).padding(.top, 14)
            GardenView(stage: state.gardenStage, progress: downloadFraction)
                .scaleEffect(0.66)
                .frame(width: GardenView.size.width * 0.66, height: GardenView.size.height * 0.66)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                .offset(y: 22).padding(.trailing, 6)
        }
        .frame(height: 112)
        .padding(.bottom, 12)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(state.title). \(state.detail)")
    }

    @ViewBuilder private var statusAction: some View {
        switch state {
        case .ready:
            TactileButton(title: EngineState.freeTitle, role: .secondary, compact: true) { Task { await Engine.shared.unload() } }
                .fixedSize()
        case .idle, .failed:
            // Asked for here, so it may download: say so when the model isn't on disk yet.
            TactileButton(title: onDisk(chosenModel) ? EngineState.loadTitle : "Download (\(chosenModel.downloadSize))",
                          role: .secondary, compact: true) { Task { try? await Engine.shared.load(settings.model, allowDownload: true) } }
                .fixedSize()
        case .downloading:
            TactileButton(title: EngineState.stopTitle, role: .secondary, compact: true) { confirmStop() }
                .fixedSize()
        default:
            EmptyView()
        }
    }

    /// One model drawn like a brewer: the chip big with its pink off register, its name as an
    /// ink label, the size stacked, one line on what it's for.
    private func model(_ choice: ModelChoice, index i: Int, place: ModelOption.Place = .settings) -> some View {
        let chosen = chosenModel == choice
        let downloaded = onDisk(choice)
        return VStack(alignment: .leading, spacing: 10) {
            ModelOption(choice: choice, index: i, place: place, chosen: chosen, onDisk: downloaded,
                        crossed: (design?.crossing ?? crossing) == choice) { settings.model = choice }
            if let strengths = ModelStrengths.of(choice) { ModelPips(strengths: strengths, seed: i).padding(.vertical, 2) }
            // Two separate buttons: choosing is the common need, deleting the rare one. The
            // chosen one needs no button or note: the pink ring already says it.
            HStack(alignment: .center, spacing: 14) {
                if !chosen { UseModelButton(choice: choice) { settings.model = choice } }
                if downloaded && !(chosen && isDownloading) {
                    CrossOutButton(title: "Delete", onHover: { crossing = $0 ? choice : (crossing == choice ? nil : crossing) }) {
                        confirmDelete(choice)
                    }
                    .accessibilityLabel("Delete \(choice.plainName.lowercased()) from disk")
                    .padding(.leading, chosen ? -6 : 0)  // alone, its X lines up with the words above
                }
            }
            .frame(minHeight: 30)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .topLeading) {
            if choice == .recommended { RecommendedMark().offset(x: 104, y: 4) }
        }
    }

    private var isDownloading: Bool {
        if case .downloading = state { return true }
        return false
    }

    private func confirmStop() { confirm(ModelDialogs.stop()) }

    private func confirmDelete(_ choice: ModelChoice) { confirm(ModelDialogs.delete(choice)) }

    private var downloadFraction: Double {
        if case .downloading(let fraction) = state { return fraction }
        return 0
    }
}

// MARK: - Privacy

struct PrivacyPage: View {
    var body: some View {
        Section(number: 1, title: "Privacy") {
            SheetGroup {
                row(.model, "Runs on your Mac", "The model runs on this Mac. Your text is never sent to a server.")
                Hairline()
                row(.hand, "Reads only when you ask", "Only after a shortcut, only the box you're in, never password fields.")
                Hairline()
                row(.paperPlane, "Never sends for you", "Iris replaces the text; you still press send. Read it first, especially for customers.")
                Hairline()
                row(.tray, "Keeps no history", "Your text isn't saved. Settings stay on this Mac.")
            }
        }
    }

    private func row(_ icon: RisoIcon, _ title: String, _ detail: String) -> some View {
        HStack(alignment: .top, spacing: DS.Space.md) {
            RisoIconView(icon: icon, size: 26).padding(.top, -3)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                Text(detail).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

extension Notification.Name {
    static let irisShowOnboarding = Notification.Name("irisShowOnboarding")
    /// Asks the app to open Settings, at a page when `object` names one ("shortcuts").
    static let irisOpenSettings = Notification.Name("irisOpenSettings")
    /// Tells the open Settings window to turn to the page `object` names. Only the app posts
    /// it, once the window is up; asking for Settings is `irisOpenSettings`.
    static let irisTurnSettingsPage = Notification.Name("irisTurnSettingsPage")
}
