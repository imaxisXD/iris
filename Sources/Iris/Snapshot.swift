import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// `Iris --snapshot <dir>` renders every UI part on its own, with forced
/// states, for design review. No model or Accessibility permission needed.
@MainActor
enum Snapshot {
    static func run(into dir: String) {
        FontRegistry.registerAll()
        let page: (AnyView) -> AnyView = { v in AnyView(v.padding(24).background(DS.Surface.panel)) }

        // /1 Buttons and controls, every state.
        ComponentSnapshot.hosted("buttons", width: 700, dir: dir, prefix: "after") {
            page(AnyView(VStack(alignment: .leading, spacing: 18) {
                stateRow("Primary") { TactileButton(title: "Replace", role: .primary, keyChip: "↵") {} }
                stateRow("Secondary") { TactileButton(title: "Open setup", role: .secondary) {} }
                HStack(spacing: 14) {
                    caption("Ghost", TactileButton(title: "Remove", role: .ghost) {})
                    caption("Final confirm", TactileButton(title: "Yes, delete", role: .destructiveConfirm) {})
                    caption("Icon", HStack(spacing: 2) { IconButton(symbol: "chevron.up", label: "Up") {}; IconButton(symbol: "trash", label: "Delete") {} })
                }
                HStack(spacing: 14) {
                    caption("Keycaps", HStack(spacing: 6) { ForEach(["↵", "⇥", "⌘C", "esc"], id: \.self) { Keycap(key: $0) } })
                }
                HStack(spacing: 26) {
                    caption("Switch, focused", TactileSwitch(isOn: .constant(true), label: "On"))
                    caption("Check, focused", TactileCheckbox(isOn: .constant(true), label: "On"))
                    caption("Choice, focused", InkChoice(options: [(value: true, label: "Panel"), (value: false, label: "Message")], selection: .constant(true), name: "x"))
                    caption("Picker, focused", Dropdown(options: [(value: "fix", label: "Fix")], selection: .constant("fix"), width: 120))
                }
                .environment(\.forcedInteraction, Interaction(focused: true))
                .padding(.vertical, 8)
                HStack(spacing: 14) {
                    caption("Switch", HStack(spacing: 8) { TactileSwitch(isOn: .constant(true), label: "On"); TactileSwitch(isOn: .constant(false), label: "Off") })
                    caption("Check", HStack(spacing: 8) { TactileCheckbox(isOn: .constant(true), label: "On"); TactileCheckbox(isOn: .constant(false), label: "Off") })
                }
                caption("Field · rest", WellField(placeholder: "Add a word or phrase", text: .constant("")).frame(width: 260))
            }))
        }

        // /2 Cards (settings groups).
        ComponentSnapshot.hosted("cards", width: 600, dir: dir, prefix: "after") {
            page(AnyView(VStack(alignment: .leading, spacing: 24) { GeneralPage() }))
        }
        // /3 Tables (lists).
        ComponentSnapshot.hosted("tables", width: 640, dir: dir, prefix: "after") {
            page(AnyView(VStack(alignment: .leading, spacing: 24) { TonesPage() }))
        }
        // /4 Dropdowns: closed and open.
        ComponentSnapshot.hosted("dropdowns", width: 460, dir: dir, prefix: "after") {
            let modes = ToneList.newMac.shown.map { (value: $0.id, label: $0.label) }
            return page(AnyView(HStack(alignment: .top, spacing: 24) {
                caption("Closed", Dropdown(options: modes, selection: .constant("fix"), width: 170))
                caption("Open", Dropdown(options: modes, selection: .constant("fix"), width: 170, forcedOpen: true))
                    .frame(height: 260, alignment: .top)
            }))
        }
        // The app menu (Apps › Turn off in an app, Start in a tone): each app with its icon.
        ComponentSnapshot.hosted("app-menu", width: 320, dir: dir, prefix: "after") {
            let apps = InstalledApps.all.prefix(9).map { MenuItem(id: $0.bundleID, label: $0.name, appIcon: $0.bundleID) }
            let items = apps + [MenuItem(id: "::choose", label: "Choose another app…", risoIcon: .apps)]
            return page(AnyView(MenuList(items: items, chosen: nil, highlighted: .constant(items[2].id), pick: { _ in }, width: 240)))
        }
        // /5 Sheets: the floating panel and the delete confirm.
        let original = "hey, i have checked the PR and its look fine but one thing need to change before merge @lead"
        let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge @lead"
        let result = RewriteResult(mode: .fix, original: original, text: fixed, segments: WordDiff.diff(original, fixed), verdict: .minor, latencyMs: 294)
        ComponentSnapshot.pure("sheets", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original, result: result, engine: .ready(.best))
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        // A long app name is cut short on its tape; the key hints never wrap.
        ComponentSnapshot.pure("sheets-long-app", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original, result: result, engine: .ready(.best),
                          appName: "T3 Code (Nightly) Preview Edition", appBundleID: "com.t3tools.t3code")
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        // Text that can't be edited (a chat history): Copy instead of Replace, no app.
        ComponentSnapshot.pure("sheets-copy-only", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original, result: result, engine: .ready(.best), canReplace: false)
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        // Height check: the panel mid-stream must be as tall as the finished one.
        ComponentSnapshot.pure("sheets-streaming", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original, result: nil, engine: .ready(.best), streaming: "hey, I")
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        let clean = "Can you review this when you get a chance? @lead"
        ComponentSnapshot.pure("sheets-clean", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: clean,
                          result: RewriteResult(mode: .fix, original: clean, text: clean, segments: WordDiff.diff(clean, clean),
                                                verdict: .unchanged, latencyMs: 200),
                          engine: .ready(.best))
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        let friendly = "Hey @lead! I checked the PR and it looks good. Just one small thing to change before we merge it."
        ComponentSnapshot.pure("sheets-tone", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            let mode = ToneList.newMac.shown[1]
            model.preview(original: original,
                          result: RewriteResult(mode: mode, original: original, text: friendly,
                                                segments: WordDiff.diff(original, friendly), verdict: nil, latencyMs: 700),
                          selected: 1, engine: .ready(.best))
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        // The menu bar slip: resting, and downloading with the progress line.
        ComponentSnapshot.pure("status-menu", width: 1060, dir: dir, prefix: "after") {
            func menu(_ engine: EngineState, ready: Bool, busy: Set<String> = [],
                      page: StatusMenu.SlipPage? = nil, updating: Double? = nil, problem: String? = nil,
                      setup: String? = nil) -> some View {
                let model = StatusMenu.Model()
                model.engine = engine
                model.problemLine = problem
                model.setupLine = setup
                // The rows the app shows (`SlipRows`), with the default shortcuts.
                model.groups = SlipRows.build(.init(openShortcut: KeyCombo.openDefault.symbols,
                                                    quickFixShortcut: KeyCombo.quickFixDefault.symbols,
                                                    shortcutTaken: problem != nil, setupComplete: setup == nil,
                                                    modelReady: ready))
                model.highlighted = page == nil && updating == nil && busy.isEmpty ? "check" : nil
                model.busy = busy
                model.showing = page.map { .page($0) } ?? updating.map { .updating($0) } ?? .commands
                return StatusMenuView(model: model, pick: { _ in }, frozenAt: 1000)
            }
            typealias Page = StatusMenu.SlipPage
            let start = Date(timeIntervalSince1970: 0)
            return AnyView(VStack(alignment: .leading, spacing: 40) {
                HStack(alignment: .top, spacing: 40) {
                    menu(.idle, ready: false)
                    // A download shows in the header and its garden; the model row only dims.
                    menu(.downloading(0.42), ready: false, busy: [StatusMenu.modelRow])
                    // Something besides the model is off: the shortcut is taken.
                    menu(.ready(.best), ready: true, problem: SlipRows.shortcutTakenLine(KeyCombo.openDefault.display))
                    // Setup isn't finished.
                    menu(.idle, ready: false, setup: "Step 2 of 3: get the model.")
                    // Check for Updates turns the page: looking, then what's new.
                    menu(.ready(.best), ready: true,
                         page: Page(id: UUID(), title: "Updates", lead: "Looking for a new Iris…",
                                    art: AnyView(UpdateCheckScene(started: start, frozenAt: start + 0.5))))
                }
                HStack(alignment: .top, spacing: 40) {
                    menu(.ready(.best), ready: true,
                         page: Page(id: UUID(), title: "Up to date", lead: "You have the latest Iris.",
                                    art: AnyView(UpdateCheckScene(started: start, answer: .init(outcome: .upToDate, at: start + 1.6),
                                                                  frozenAt: start + 3))))
                    menu(.ready(.best), ready: true,
                         page: Page(id: UUID(), title: "2 new changes", lead: "Ready to install.",
                                    art: AnyView(UpdateCheckScene(started: start, answer: .init(outcome: .changes(2), at: start + 1.6),
                                                                  frozenAt: start + 3)),
                                    content: AnyView(UpdateChanges(
                            changes: ["The menu bar flower in pen, with the pink plate", "Names each fix in the margin"], update: {}))))
                    // Update & reopen: the header says Updating and the flower grows in the rain.
                    menu(.ready(.best), ready: true, updating: 0.8)
                }
            }.padding(28).background(
                // A busy desktop, to judge how much shows through the paper.
                LinearGradient(colors: [Color(nsColor: DS.rgb(0x2B2A33)), Color(nsColor: DS.rgb(0x6A4F8C)),
                                        Color(nsColor: DS.rgb(0x1F6F78))], startPoint: .topLeading, endPoint: .bottomTrailing)))
        }
        // Every hover in Settings, drawn on: one pen wash everywhere.
        ComponentSnapshot.pure("hovers", width: 640, dir: dir, prefix: "after") {
            VStack(alignment: .leading, spacing: 18) {
                HStack(spacing: 10) { RisoIconView(icon: .tones, size: 24); Text("Tones").font(DS.Font.body); Spacer() }
                    .padding(.horizontal, 10).frame(width: 200, height: 32).penWash(hover: true, seed: 26)
                HStack(spacing: 10) { RisoIconView(icon: .general, size: 24); Text("General").font(DS.Font.body); Spacer() }
                    .padding(.horizontal, 10).frame(width: 200, height: 32).penWash(hover: false, current: true, seed: 36)
                HStack { Text("Kubernetes").font(DS.Font.body); Spacer(); Text("added today").font(DS.Font.caption).foregroundStyle(DS.Ink.i10) }
                    .padding(.horizontal, DS.Space.md).frame(height: DS.Size.row).penWash(hover: true, seed: 7)
                HStack(spacing: 18) {
                    Image(systemName: "chevron.up").font(.system(size: 12, weight: .medium)).frame(width: DS.Size.icon, height: DS.Size.icon)
                        .penWash(hover: true, seed: 23)
                    HStack(spacing: 6) { Keycap(key: "⌘"); Keycap(key: "R"); Text("TRY AGAIN").font(DS.Font.caption).tracking(0.4) }
                        .padding(.horizontal, 6).padding(.vertical, 4).penWash(hover: true, seed: 32)
                }
            }
            .padding(24).frame(width: 640, alignment: .leading).background(DS.Surface.overlay)
        }
        // A shortcut, set and while recording a new one.
        ComponentSnapshot.pure("shortcut-recorder", width: 520, dir: dir, prefix: "after") {
            VStack(alignment: .trailing, spacing: 28) {
                ShortcutRecorder(combo: .openDefault, other: nil, action: .open) { _ in }
                ShortcutRecorder(combo: .quickFixDefault, other: nil, action: .quickFix, allowsClear: true, forcedRecording: true) { _ in }
            }
            .padding(24).frame(maxWidth: .infinity, alignment: .trailing).background(DS.Surface.overlay)
        }
        // Learning words: the question beside the field.
        ComponentSnapshot.pure("keep-word", width: 760, dir: dir, prefix: "after") {
            KeepWordView(word: "Abhijyan", keep: {}, dismiss: {}).padding(24).background(DS.Surface.ground)
        }
        // The setup garden, one frame per beat of its story.
        ComponentSnapshot.pure("garden", width: 1240, dir: dir, prefix: "after") {
            let beats: [(GardenView.Stage, String)] = [(.bud, "1 · bud"), (.allowed, "2 · allowed"), (.watering, "3 · downloading"),
                                                       (.warming, "4 · loading"), (.ready, "5 · ready"), (.tried, "6 · tried it")]
            return AnyView(HStack(spacing: 18) {
                ForEach(beats, id: \.1) { stage, caption in
                    VStack(spacing: 6) {
                        GardenView(stage: stage, progress: 0.6, frozenAt: 1000)
                        Text(caption).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    }
                }
            }.padding(24).background(DS.Surface.overlay))
        }
        // Riso icons: actual rail size, then blown up to check the print.
        ComponentSnapshot.pure("icons", width: 1140, dir: dir, prefix: "after") {
            AnyView(VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 18) {
                    ForEach(RisoIcon.allCases, id: \.self) { icon in
                        VStack(spacing: 8) {
                            RisoIconView(icon: icon, size: 72)
                            Text(icon.rawValue).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                        }
                    }
                }
                HStack(spacing: 26) {
                    ForEach(RisoIcon.allCases, id: \.self) { RisoIconView(icon: $0, size: 24).frame(width: 46) }
                }
                // The menu-bar mark on a dark and a light bar, at size and zoomed.
                HStack(spacing: 18) {
                    ForEach([true, false], id: \.self) { dark in
                        HStack(spacing: 14) {
                            MenuBarMark(dark: dark).frame(width: 18, height: 18)
                            MenuBarMark(dark: dark).frame(width: 72, height: 72)
                        }
                        .padding(10)
                        .background(dark ? Color(red: 0.13, green: 0.13, blue: 0.19) : Color(white: 0.9))
                    }
                }
            }.padding(24).background(DS.Surface.panel))
        }
        // Failures: a stray letter (circled), a timeout (Try again), too long (no retry).
        ComponentSnapshot.pure("sheets-failed-short", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: "v", result: nil, selected: 4, engine: .ready(.best), error: RewriteError.offTask)
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        ComponentSnapshot.pure("sheets-failed-timeout", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original, result: nil, engine: .ready(.best), error: RewriteError.timedOut)
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        ComponentSnapshot.pure("sheets-failed-toolong", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: original + " " + original, result: nil, selected: 2, engine: .ready(.best),
                          error: RewriteError.tooLong(limit: Rewriter.maxInputCharacters))
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        // A romanized-Hindi draft: labeled as a translation, shown without a diff.
        let hinglish = "bhai order kal tak aa jayega, customer ko bol do"
        let english = "The order will arrive by tomorrow, tell the customer."
        ComponentSnapshot.pure("sheets-translated", width: 700, dir: dir, prefix: "after") {
            let model = PanelModel()
            model.preview(original: hinglish,
                          result: RewriteResult(mode: .fix, original: hinglish, text: english,
                                                segments: WordDiff.diff(hinglish, english), verdict: nil, translated: true, latencyMs: 400),
                          engine: .ready(.best))
            return AnyView(PanelView(model: model).padding(16).background(DS.Surface.ground))
        }
        ComponentSnapshot.hosted("confirm", width: 520, dir: dir, prefix: "after") {
            AnyView(ZStack {
                VStack(alignment: .leading, spacing: 12) { ModelPage() }.padding(24).background(DS.Surface.panel).blur(radius: 1)
                Color.black.opacity(0.3)
                ConfirmCard(request: ModelDialogs.delete(.e4b), dismiss: {})
            }
            .frame(height: 420).clipped())
        }
        // Whole screens for context.
        for p in SettingsView.Page.allCases {
            ComponentSnapshot.hosted("settings-\(p.rawValue)", width: SettingsView.size.width, dir: dir, prefix: "screen") {
                AnyView(SettingsView(page: p, scrolls: false))
            }
        }
        // Settings › Updates: each answer the update check gives (the slip's words), and a
        // longer run of made-up issues on the What's new shelf, an older one open.
        let made = Date()
        let answered = { (status: Updater.Status) in
            UpdateCheck.Phase.answered(.init(status: status, started: made.addingTimeInterval(-3), at: made.addingTimeInterval(-2)))
        }
        let checks: [(String, UpdateCheck.Phase)] = [
            ("idle", .idle), ("looking", .looking(since: made.addingTimeInterval(-0.5))), ("up-to-date", answered(.upToDate)),
            ("new", answered(.behind(2, ["The menu bar flower in pen", "Names each fix in the margin"]))),
            ("release", answered(.newRelease("v0.2.0", page: Updater.releases))),
            ("failed", answered(.failed("Couldn't reach GitHub. Check your connection and try again."))),
            ("cant", answered(.noCheckout)), ("updating", .updating(0.42)),
            ("install-failed", .installFailed("The new version didn't build.")),
        ]
        for (name, phase) in checks {
            UpdateCheck.shared.preview(phase)
            ComponentSnapshot.hosted("updates-check-\(name)", width: 660, dir: dir, prefix: "after") {
                VStack(alignment: .leading, spacing: 0) { UpdatesPage(updates: .shared, showsNotes: false) }
                    .padding(24).background(DS.Surface.overlay)
            }
        }
        // The letter from GitHub, beat by beat: growing, flying out, away, back with the letter,
        // dropped, opening, open; then each answer settled, in Settings and on the slip.
        do {
            let t0 = Date(timeIntervalSince1970: 0), at = t0.addingTimeInterval(2.8)
            let up = UpdateCheckScene.Answer(outcome: .upToDate, at: at, letter: UpdateCheck.Words(.upToDate).letter)
            let new = UpdateCheckScene.Answer(outcome: .changes(2), at: at, letter: UpdateCheck.Words(.behind(2, [])).letter)
            let failed = UpdateCheckScene.Answer(outcome: .failed, at: at)
            let frames: [(String, UpdateCheckScene.Answer?, Double)] = [
                ("0.15 growing", nil, 0.15), ("0.7 flying out", nil, 0.7), ("2.0 away", nil, 2.0),
                ("+0.35 back with it", up, 3.15), ("+0.8 dropped", up, 3.6), ("+1.05 opening", up, 3.85),
                ("+2 up to date", up, 4.8), ("+2 new changes", new, 4.8), ("+2 couldn't check", failed, 4.8),
            ]
            ComponentSnapshot.pure("updates-letter-storyboard", width: 760, dir: dir, prefix: "after") {
                VStack(alignment: .leading, spacing: 34) {
                    ForEach(Array(frames.enumerated()), id: \.offset) { _, f in
                        HStack(alignment: .bottom, spacing: 30) {
                            Text(f.0).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).frame(width: 130, alignment: .leading)
                            UpdateCheckScene(started: t0, answer: f.1, layout: .wide, frozenAt: t0.addingTimeInterval(f.2))
                            UpdateCheckScene(started: t0, answer: f.1.map { .init(outcome: $0.outcome, at: $0.at) },
                                             frozenAt: t0.addingTimeInterval(f.2))
                        }
                        .frame(height: 100)
                    }
                }
                .padding(.vertical, 40).padding(.leading, 24).frame(width: 760, alignment: .leading).background(DS.Surface.overlay)
            }
        }
        UpdateCheck.shared.preview(.idle)
        ComponentSnapshot.hosted("settings-updates-many", width: SettingsView.size.width, dir: dir, prefix: "screen") {
            AnyView(SettingsView(page: .updates, scrolls: false, changelog: Changelog.parse(SampleChangelog.text)))
        }
        // What's new with an older version picked: its changes printed, the newest among the
        // other covers. And many versions, the one you have not the newest.
        ComponentSnapshot.pure("updates-older-picked", width: 700, dir: dir, prefix: "after") {
            UpdatesPage(updates: .shared, changelog: .bundled, start: Changelog.bundled.issues.dropFirst().first?.version)
                .padding(44).background(DS.Surface.overlay)
        }
        ComponentSnapshot.pure("updates-many-older-yours", width: 700, dir: dir, prefix: "after") {
            UpdatesPage(updates: .shared, changelog: Changelog.parse(SampleChangelog.text), installed: "0.2.0")
                .padding(44).background(DS.Surface.overlay)
        }
        // Model & memory while a model downloads (Stop download on the band, the bar under it),
        // and the confirm Stop download asks.
        let downloading = DesignState(model: (.downloading(0.62), .lighter, [.best]), noteAt: 2.6)
        ComponentSnapshot.hosted("settings-model-downloading", width: SettingsView.size.width, dir: dir, prefix: "screen", design: downloading) {
            AnyView(SettingsView(page: .model, scrolls: false))
        }
        ComponentSnapshot.hosted("confirm-stop-download", width: 640, dir: dir, prefix: "after", design: downloading) {
            AnyView(ZStack {
                VStack(alignment: .leading, spacing: 12) { ModelPage() }.padding(24).background(DS.Surface.panel).blur(radius: 1)
                Color.black.opacity(0.3)
                ConfirmCard(request: ModelDialogs.stop(), dismiss: {})
            }
            .frame(height: 460).clipped())
        }
        // The pointer on Lighter's Delete: its drawing crossed out in red, like the zine's cup.
        ComponentSnapshot.hosted("settings-model-delete-hover", width: SettingsView.size.width, dir: dir, prefix: "screen",
                                 design: DesignState(model: (.ready(.best), .best, [.best, .lighter]), crossing: .lighter, noteAt: 2.6)) {
            AnyView(SettingsView(page: .model, scrolls: false))
        }
        // Someone who used Iris before the trained models: still on Google's model, with both of
        // Iris's own offered above it, and the earlier ones in their smaller row.
        ComponentSnapshot.hosted("settings-model-earlier", width: SettingsView.size.width, dir: dir, prefix: "screen",
                                 design: DesignState(model: (.ready(.e4b), .e4b, [.e4b, .e2b]), noteAt: 2.6)) {
            AnyView(SettingsView(page: .model, scrolls: false))
        }
        // Setup: every state, the whole page and what the window shows when it opens.
        SetupStates.render(into: dir)
        // Settings → General → When Iris speaks up: every clip with its note landed (the undo keys ringed).
        ComponentSnapshot.pure("note-demos", width: 660, dir: dir, prefix: "after") {
            VStack(alignment: .leading, spacing: 18) {
                ForEach(AppSettings.Note.allCases, id: \.self) { note in
                    NoteDemo(note: note, t: NoteDemo.landed(note), keys: KeyCombo.quickFixDefault.keycaps)
                }
                NoteDemo(note: .replaced, t: NoteDemo.landed(.replaced), keys: KeyCombo.quickFixDefault.keycaps,
                         showsUndoKeys: false)
            }
            .padding(24).background(DS.Surface.overlay)
        }
        // The keys are whatever the user recorded: mid-press, the default, someone's own, a wide
        // key and the longest there can be.
        ComponentSnapshot.pure("note-demo-keys", width: 660, dir: dir, prefix: "after") {
            VStack(alignment: .leading, spacing: 18) {
                ForEach(Array(NoteDemo.sampleKeys.enumerated()), id: \.offset) { i, keys in
                    NoteDemo(note: .replaced, t: [1.1, 1.0, 0.95, 1.2][i], keys: keys)
                }
            }
            .padding(24).background(DS.Surface.overlay)
        }
        // Where Iris lives, said when setup finishes: the note hanging from the menu bar flower.
        ComponentSnapshot.pure("menubar-hint", width: 520, dir: dir, prefix: "after") {
            VStack(spacing: 0) {
                HStack(spacing: 14) {
                    Spacer()
                    MenuBarMark(dark: true).frame(width: 18, height: 18)
                    Text("Wed 5 Nov  9:41").font(.system(size: 13)).foregroundStyle(.white)
                }
                .padding(.horizontal, 14).frame(height: 26)
                .background(Color(nsColor: DS.rgb(0x2B2A33)))
                MenuBarHintView(keys: ["⌃", "G"], tailOffset: 0)
                    .fixedSize()
                    .frame(width: 0)
                    .padding(.top, 2).padding(.leading, 382)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.bottom, 30)
            }
            .background(LinearGradient(colors: [Color(nsColor: DS.rgb(0x6A4F8C)), Color(nsColor: DS.rgb(0x1F6F78))],
                                       startPoint: .top, endPoint: .bottom))
        }
        // Every message by your text: `Iris --toast-states`, from the same `Message`s the app shows.
    }

    /// Writes the 1024px app icon: a paper tile with grain and the iris's bloom.
    static func icon(to path: String) {
        let view = ZStack {
            RoundedRectangle(cornerRadius: 185, style: .continuous).fill(Color(nsColor: DS.rgb(0xF7F3F0)))
                .grain(0.35)
                .clipShape(RoundedRectangle(cornerRadius: 185, style: .continuous))
            // The bloom in the middle, its stem running off the tile's bottom edge.
            Image(nsImage: RisoArt.flower(points: 824, crop: .icon, scale: 1.5)).resizable()
                .frame(width: 824, height: 824)
                .clipShape(RoundedRectangle(cornerRadius: 185, style: .continuous))
            RoundedRectangle(cornerRadius: 185, style: .continuous).strokeBorder(Color.black.opacity(0.08), lineWidth: 4)
        }
        .frame(width: 824, height: 824)
        .frame(width: 1024, height: 1024)
        let renderer = ImageRenderer(content: view)
        renderer.scale = 1
        if let tiff = renderer.nsImage?.tiffRepresentation,
            let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
            ComponentSnapshot.write(png, to: path)
        }
    }

    /// A control in rest, hover, pressed, focused and disabled, side by side.
    private static func stateRow<V: View>(_ title: String, @ViewBuilder _ make: @escaping () -> V) -> some View {
        HStack(spacing: 14) {
            caption("\(title) · rest", make().environment(\.forcedInteraction, .rest))
            caption("hover", make().environment(\.forcedInteraction, Interaction(hover: true)))
            caption("pressed", make().environment(\.forcedInteraction, Interaction(hover: true, pressed: true)))
            caption("keyboard focus", make().environment(\.forcedInteraction, Interaction(focused: true)))
            caption("disabled", make().disabled(true))
        }
    }

    private static func caption<V: View>(_ text: String, _ view: V) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            view
            Text(text).font(DS.Font.micro).textCase(.uppercase).tracking(0.4).foregroundStyle(DS.Ink.i10)
        }
    }
}

/// Made-up versions for the What's new shelf in snapshots: how it looks once Iris has a few.
/// Made-up versions for trying the shelf, on their own: never mixed with the real changelog,
/// whose versions would clash with these.
enum SampleChangelog {
    static let text = """
        # 0.5.0 · Night shift
        date: 2027-04-02
        ## Dark paper
        Iris prints on dark paper when your Mac is in dark mode.
        - Faster first fix after waking.

        # 0.4.0 · Many hands
        date: 2027-02-14
        ## Tones you share
        Send a tone to a friend as a link.

        # 0.3.0 · Small print
        date: 2027-01-09
        ## Quick Fix in more apps
        Works in Figma comments and Linear.
        ## Fewer surprises
        Big rewrites always ask first.
        - The flower sways less under Reduce Motion.

        # 0.2.0 · Second print
        date: 2026-12-01
        ## Faster fixes
        Fix comes back sooner on long messages.
        - Settings remembers its page.

        # 0.1.0 · First print
        date: 2026-11-07
        ## Fix and four tones
        Fix corrects grammar and spelling and keeps your words.
        ## Quick Fix
        Fixes the text box you're in and replaces it straight away.
        - The flower in the menu bar shows what Iris is doing.
        """
}
