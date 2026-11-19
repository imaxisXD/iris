import IrisCore
import IrisEngine
import SwiftUI

/// The floating panel shown over the message box: a slip of paper lying on the app, like
/// the poster's sheet on its grey table. Sharp corners, no border, one long soft shadow.
/// Tone names in the poster's label style, a hand-drawn ink rule, a fainter one over the keys.
/// One primary action, Replace, with the pink plate printed slightly off, Iris's mark.
///
///  ┌──────────────────────────────────────────────────────────┐
///  │ (Fix)(Professional) Friendly  Shorter  Clearer            │  tone tabs
///  ├──────────────────────────────────────────────────────────┤
///  │ text, with Iris's edits in pink and removals struck through│
///  ├──────────────────────────────────────────────────────────┤
///  │ ⇥ NEXT TONE  ⌘R TRY AGAIN                     [Replace ↵] │
///  └──────────────────────────────────────────────────────────┘
struct PanelView: View {
    @ObservedObject var model: PanelModel
    /// Folded into the bud on the text box until there's something to read.
    @ObservedObject var presence = PanelPresence()
    @ObservedObject private var settings = AppSettings.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    static let width: CGFloat = 620
    /// Transparent room around the slip for its shadow (the window draws none). The
    /// controller subtracts it when placing the panel, so the paper itself sits `PanelLayout.gap` off the field.
    static let margin = (horizontal: CGFloat(26), top: CGFloat(12), bottom: CGFloat(44))

    var body: some View {
        VStack(spacing: 0) {
            header
            // Both rules drawn in the keycaps' hand, so every line on the slip is the same pen.
            PenRule(seed: 11, ink: DS.Ink.i12.opacity(0.85), width: 1.2).padding(.horizontal, 6)
            content.padding(.horizontal, DS.Space.lg).padding(.top, DS.Space.md + 2).padding(.bottom, DS.Space.md)
            PenRule(seed: 29).padding(.horizontal, DS.Space.lg)
            footer
        }
        .frame(width: Self.width)
        .background(DS.Surface.overlay)
        .grain(0.22)
        // The sheet's shadow (a blurred shape rather than .shadow, so it renders the same
        // everywhere), a little darker here: the panel lies on other apps, not on Iris's paper.
        .paperShadow(spread: 16, drop: 18, opacity: 0.24)
        .padding(.horizontal, Self.margin.horizontal)
        .padding(.top, Self.margin.top)
        .padding(.bottom, Self.margin.bottom)
        .fixedSize(horizontal: false, vertical: true)
        // Unrolls sideways out of the bloom beside it: from a sliver at the flower's head, a
        // quick spring with a little give, and folds back down into it. The paper is solid at
        // once (a fast fade), so it never shows as a pale ghost box while it grows.
        // Reduce Motion just fades.
        .scaleEffect(x: presence.unfolded || reduceMotion ? 1 : PanelMotion.folded.scaleX,
                     y: presence.unfolded || reduceMotion ? 1 : PanelMotion.folded.scaleY, anchor: presence.anchor)
        .rotationEffect(.degrees(presence.unfolded || reduceMotion ? 0 : PanelMotion.folded.degrees), anchor: presence.anchor)
        .animation(reduceMotion ? nil
                   : (presence.unfolded ? .spring(duration: PanelMotion.unroll.duration, bounce: PanelMotion.unroll.bounce)
                      : .easeIn(duration: PanelMotion.foldShrink)),
                   value: presence.unfolded)
        .opacity(presence.unfolded ? 1 : 0)
        .animation(reduceMotion ? .easeOut(duration: DS.Motion.fast.enter)
                   : presence.unfolded ? .easeOut(duration: PanelMotion.unrollFade)
                   : .easeOut(duration: PanelMotion.foldFade.duration).delay(PanelMotion.foldFade.delay),
                   value: presence.unfolded)
    }

    // MARK: Header

    private var header: some View {
        // The number keys jump to a tone; they're listed in Settings, not printed here.
        ToneTabs(labels: model.modes.map(\.label), selected: model.selected) { model.select($0) }
            .padding(.horizontal, DS.Space.lg)
            .padding(.top, 14)
            .padding(.bottom, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: Content

    @ViewBuilder private var content: some View {
        switch model.phase {
        case .preparing:
            preparing
        case .generating(let partial):
            VStack(alignment: .leading, spacing: DS.Space.md) {
                // The original text, invisible, holds the panel's height while the answer
                // streams in, so the panel doesn't collapse to one line and regrow.
                ZStack(alignment: .topLeading) {
                    Text(model.original).font(DS.Font.subtitle).lineSpacing(5).hidden()
                    if partial.isEmpty {
                        // Before the first word arrives: the draft, breathing, so it's clear Iris is on it.
                        WaitingText(text: model.original)
                    } else {
                        Text(partial).font(DS.Font.subtitle).lineSpacing(5).foregroundStyle(DS.Ink.i12)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(partial.isEmpty ? model.original : partial)
            }
        case .done(let result):
            VStack(alignment: .leading, spacing: DS.Space.md) {
                resultText(result).font(DS.Font.subtitle).lineSpacing(5)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    // The pink behind Iris's words is printed by a second copy of the same text,
                    // laid out the same and drawing only the markers, so the text people read
                    // stays plain text they can select and copy.
                    .background(alignment: .topLeading) {
                        resultText(result).font(DS.Font.subtitle).lineSpacing(5)
                            .textRenderer(PrintedMarker(screen: Image(nsImage: RisoArt.screen(
                                points: CGSize(width: 96, height: 96), rgb: Riso.pink, seed: 31, grid: 0.55, specks: 0.6))))
                            // Selectable text skips custom renderers, and the `.textSelection`
                            // below reaches this copy too: without this the live panel prints no
                            // pink at all (snapshots don't select, so they still showed it).
                            .textSelection(.disabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .allowsHitTesting(false)
                            .accessibilityHidden(true)
                    }
                    .textSelection(.enabled)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(result.text)
                    .accessibilityValue(changeSummary(result))
                if model.translated {
                    // No diff for a translation, so keep the original in view to compare against.
                    Text("You wrote: \(model.original)").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        case .failed(let failure):
            RedPenNote(failure: failure, draft: model.original)
        }
    }

    private var preparing: some View {
        VStack(alignment: .leading, spacing: DS.Space.sm) {
            Text(model.original).font(DS.Font.subtitle).lineSpacing(5).foregroundStyle(DS.Ink.i9)
                .frame(maxWidth: .infinity, alignment: .leading)
            switch model.engineState {
            case .downloading(let fraction):
                Text("Downloading the model, one time. You can close this; it keeps going.")
                    .font(DS.Font.caption).foregroundStyle(DS.Ink.i11)
                ProgressTrack(fraction: fraction)
            case .failed(let failure):
                Text(failure.sentence).font(DS.Font.caption).foregroundStyle(DS.Hue.redText)
            default:
                HStack(spacing: 6) {
                    InkDots()
                    Text("Loading the model…").font(DS.Font.caption).foregroundStyle(DS.Ink.i11)
                }
            }
        }
    }

    /// The diff inline, for Fix and every tone: Iris's words in a pink highlight, removed words
    /// struck in espresso. Settings › General › Show what changed turns it off for plain text.
    private func resultText(_ result: RewriteResult) -> Text {
        // A translation has nothing in common with the original, so a diff is just noise.
        guard settings.showChanges, result.verdict != .unchanged, !model.translated else {
            return Text(result.text).foregroundStyle(DS.Ink.i12)
        }
        var text = Text("")
        for (index, segment) in result.segments.enumerated() {
            switch segment {
            case .same(let s):
                text = text + Text(s).foregroundStyle(DS.Ink.i12)
            case .removed(let s):
                guard !s.allSatisfy(\.isWhitespace) else { continue }
                text = text + Text(s).foregroundStyle(DS.Hue.espresso).strikethrough()
                if index + 1 < result.segments.count, case .added = result.segments[index + 1] {
                    text = text + Text(" ")
                }
            case .added(let s):
                // Marked for `PrintedMarker`, which prints the pink behind it.
                text = text + Text(s).foregroundStyle(DS.Ink.i12).customAttribute(AddedWords())
            }
        }
        return text
    }


    /// What VoiceOver reads after the corrected text: "2 changes: its look to it looks; need to needs".
    private func changeSummary(_ result: RewriteResult) -> String {
        guard result.mode.isFix else { return "" }
        if model.translated { return "Translated to English" }
        var pairs: [String] = []
        var removed = ""
        for segment in result.segments {
            switch segment {
            case .removed(let s): removed += s
            case .added(let s):
                let from = removed.trimmingCharacters(in: .whitespaces), to = s.trimmingCharacters(in: .whitespaces)
                pairs.append(from.isEmpty ? "added \(to)" : "\(from) to \(to)")
                removed = ""
            case .same:
                let from = removed.trimmingCharacters(in: .whitespaces)
                if !from.isEmpty { pairs.append("removed \(from)") }
                removed = ""
            }
        }
        let tail = removed.trimmingCharacters(in: .whitespaces)
        if !tail.isEmpty { pairs.append("removed \(tail)") }
        guard !pairs.isEmpty else { return "No changes" }
        return "\(pairs.count == 1 ? "1 change" : "\(pairs.count) changes"): " + pairs.joined(separator: "; ")
    }

    // MARK: Footer

    /// Left: what you need to know right now (nothing to fix, a big rewrite, copied),
    /// otherwise the keys. Right: the one primary action, whose label carries the
    /// loading state so the panel never grows a status row. The rules are `PanelFooter`'s.
    private var footer: some View {
        let state = model.footer
        return HStack(spacing: DS.Space.md) {
            footerStatus(state.note)
            Spacer(minLength: DS.Space.sm)
            if let label = state.app {
                // The app your text is in, beside the button, with no words before it: the name on
                // a strip of masking tape with its icon stuck on the corner, not an ink label, which
                // would read as a second black button. VoiceOver says what it means.
                let unchanged = label == .nothingToChange(label.app)
                TapeLabel(text: label.app, appIcon: model.appBundleID)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(unchanged ? "Nothing to change in \(label.app)" : "Replaces your text in \(label.app)")
            }
            switch state.action {
            case .tryAgain:
                // Nothing to replace. When another go could work, that's the next step instead.
                TactileButton(title: "Try again", role: .secondary, keyChip: "⌘R") { model.regenerate() }
            case .none:
                EmptyView()
            case .checking, .writing, .replace, .copy, .done:
                TactileButton(title: primaryTitle(state.action), role: .primary,
                              keyChip: model.currentResult == nil ? nil : "↵",
                              loading: isWorking, reserveTitle: "Checking…") { model.accept() }
                    .disabled(model.currentResult == nil)
            }
        }
        .padding(.horizontal, DS.Space.lg)
        .padding(.vertical, 10)
    }

    @ViewBuilder private func footerStatus(_ note: PanelFooter.Note) -> some View {
        // Statuses in Iris's own word, a pink tick and brush, never a badge.
        switch note {
        case .copied:
            DoneNote(text: "copied")
        case .retry(let text):
            DoneNote(text: text.prefix(1).lowercased() + text.dropFirst())
        case .looksGood:
            // A human moment, so it gets the brush: Iris's word over the pink marker, with
            // a green pen tick, the one green in Iris. Nothing more to say.
            HStack(alignment: .firstTextBaseline, spacing: 7) {
                HandTick().stroke(DS.Hue.green, style: StrokeStyle(lineWidth: 2.4, lineCap: .round, lineJoin: .round))
                    .frame(width: 19, height: 15)
                Text("looks good").font(DS.Font.brush(26)).foregroundStyle(DS.Ink.i12)
                    .fixedSize()
                    .background(alignment: .bottom) {
                        MarkerSwipe(height: 7, seed: 8)
                            .rotationEffect(.degrees(-1.2)).padding(.horizontal, -4).offset(y: -3)
                    }
            }
            .rotationEffect(.degrees(-2))
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Looks good, nothing to fix")
        case .translated:
            Badge(text: "Translated to English, check it reads right", tone: .warn)
        case .bigRewrite:
            Badge(text: "Big rewrite, read it before replacing", tone: .warn)
        case .keys(let nextTone, let tryAgain):
            // Two keys you can't guess. Esc, ⌘C and the number keys work too; Settings → Shortcuts lists them.
            HStack(spacing: DS.Space.md) {
                if nextTone { KeyHint(key: "⇥", label: "Next tone") { model.next() } }
                if tryAgain { KeyHint(key: "⌘R", label: "Try again") { model.regenerate() } }
            }
            // Never squeezed onto two lines: a long app name gets cut short instead.
            .fixedSize()
        case .none:
            EmptyView()
        }
    }

    private var isWorking: Bool {
        switch model.phase {
        case .preparing, .generating: true
        default: false
        }
    }

    private func primaryTitle(_ action: PanelFooter.Action) -> String {
        switch action {
        case .checking: "Checking…"
        case .writing: "Writing…"
        case .done: "Done"
        // Text that can't be edited where it is (a chat history, a web page): copy it instead.
        case .copy: "Copy"
        case .replace, .tryAgain, .none: "Replace"
        }
    }
}

/// A failed check, marked up like an editor would: a short draft gets circled with an arrow
/// to the note; a longer one stays in grey with the note under it, curling down to Try again.
private struct RedPenNote: View {
    let failure: PanelModel.Failure
    let draft: String

    private var trimmed: String { draft.trimmingCharacters(in: .whitespacesAndNewlines) }
    private let pen = StrokeStyle(lineWidth: 1.7, lineCap: .round, lineJoin: .round)

    var body: some View {
        Group {
            if failure.circled {
                HStack(alignment: .center, spacing: 8) {
                    Text(trimmed).font(DS.Font.subtitle).foregroundStyle(DS.Ink.i12).lineLimit(1).fixedSize()
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .overlay(BrushRing(rgb: Riso.red, seed: 5))
                    HandArrow(curl: .left).stroke(DS.Hue.red, style: pen).frame(width: 44, height: 16).scaleEffect(x: -1)
                    note
                }
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    if !trimmed.isEmpty {
                        Text(draft).font(DS.Font.subtitle).lineSpacing(5).foregroundStyle(DS.Ink.i9).lineLimit(4)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    HStack(alignment: .bottom, spacing: 6) {
                        if failure.canRetry { Spacer(minLength: 0) }
                        note
                        if failure.canRetry {
                            HandArrow(curl: .down).stroke(DS.Hue.red, style: pen).frame(width: 26, height: 22).offset(y: 10)
                        }
                    }
                    // Lands the arrow over the Try again button.
                    .padding(.trailing, failure.canRetry ? 48 : 0)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(failure.detail)
    }

    private var note: some View {
        Text(failure.note).font(DS.Font.brush(25)).foregroundStyle(DS.Hue.redText)
            .rotationEffect(.degrees(-2))
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// The draft in grey, pulsing gently while Iris works. Under Reduce Motion it holds still.
private struct WaitingText: View {
    let text: String
    @State private var dim = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Text(text).font(DS.Font.subtitle).lineSpacing(5)
            .foregroundStyle(DS.Ink.i9)
            .opacity(dim && !reduceMotion ? 0.45 : 1)
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) { dim = true }
            }
    }
}

/// Marks the words Iris put in, for `PrintedMarker`.
struct AddedWords: TextAttribute {}

/// Prints the pink plate behind Iris's words through the screen, like every other pink in
/// Iris, instead of a flat highlight. It draws the markers only, never the words: it sits
/// behind the text itself (`PanelView`).
struct PrintedMarker: TextRenderer {
    let screen: Image

    func draw(layout: Text.Layout, in context: inout GraphicsContext) {
        var context = context
        for line in layout {
            for run in line where run[AddedWords.self] != nil {
                let box = run.typographicBounds.rect.insetBy(dx: -1.5, dy: 1)
                var marker = context
                marker.opacity = 0.62
                marker.fill(Path(box), with: .tiledImage(screen))
            }
        }
    }
}
