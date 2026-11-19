import AppKit
import SwiftUI

// MARK: - Interaction state

/// Hover, press and keyboard focus for one control. Snapshots force a state through the
/// environment so every state can be reviewed side by side.
struct Interaction: Equatable {
    var hover = false
    var pressed = false
    var focused = false
    static let rest = Interaction()
}

private struct ForcedInteractionKey: EnvironmentKey { static let defaultValue: Interaction? = nil }
extension EnvironmentValues {
    var forcedInteraction: Interaction? {
        get { self[ForcedInteractionKey.self] }
        set { self[ForcedInteractionKey.self] = newValue }
    }
}

// MARK: - Buttons

/// Pick the role before the look: one primary per surface, everything else secondary,
/// ghost for tertiary actions inside a form. Something that destroys starts from a
/// `CrossOutButton`; `destructiveConfirm` is only its final yes.
enum ButtonRole {
    case primary, secondary, ghost, destructiveConfirm
}

struct TactileButton: View {
    let title: String
    var role: ButtonRole = .secondary
    var icon: String? = nil
    var keyChip: String? = nil
    var compact = false
    /// Loading: the label swaps in place to a spinner and an "-ing…" verb.
    var loading = false
    /// The widest label this button shows, so it keeps one width as its label swaps.
    var reserveTitle: String? = nil
    let action: () -> Void

    @State private var hovering = false
    @Environment(\.isEnabled) private var enabled

    var body: some View {
        Button(action: action) { EmptyView() }
            .buttonStyle(TactileButtonStyle(title: title, role: role, icon: icon, keyChip: keyChip, compact: compact,
                                            loading: loading, reserveTitle: reserveTitle, hovering: hovering))
            .onHover { hovering = $0 && enabled }
            .focusEffectDisabled()
    }
}

private struct TactileButtonStyle: ButtonStyle {
    let title: String
    let role: ButtonRole
    let icon: String?
    let keyChip: String?
    let compact: Bool
    let loading: Bool
    let reserveTitle: String?
    let hovering: Bool

    func makeBody(configuration: Configuration) -> some View {
        Face(title: title, role: role, icon: icon, keyChip: keyChip, compact: compact,
             loading: loading, reserveTitle: reserveTitle,
             live: Interaction(hover: hovering, pressed: configuration.isPressed))
    }

    struct Face: View {
        let title: String
        let role: ButtonRole
        let icon: String?
        let keyChip: String?
        let compact: Bool
        var loading = false
        var reserveTitle: String? = nil
        let live: Interaction
        @Environment(\.isFocused) private var focused
        @Environment(\.isEnabled) private var enabled
        @Environment(\.forcedInteraction) private var forced
        @Environment(\.accessibilityReduceMotion) private var reduceMotion

        var body: some View {
            let state = forced ?? Interaction(hover: live.hover, pressed: live.pressed, focused: focused)
            let shape = RoundedRectangle(cornerRadius: DS.Radius.control, style: .continuous)
            HStack(spacing: 6) {
                if loading {
                    InkDots(ink: text)
                } else if let icon {
                    PenGlyph(name: icon, size: 10, weight: 1.5).foregroundStyle(iconInk)
                }
                ZStack(alignment: .leading) {
                    if let reserveTitle { Text(reserveTitle).font(DS.Font.body).tracking(-0.12).lineLimit(1).fixedSize().hidden() }
                    Text(title).font(DS.Font.body).tracking(-0.12).lineLimit(1).fixedSize()
                }
                if let keyChip {
                    // Small keycaps drawn in the button's own ink, one per key, like the footer's.
                    HStack(spacing: 3) {
                        ForEach(Array(Keycap.split(keyChip).enumerated()), id: \.offset) { i, key in
                            Text(key).font(DS.Font.key)
                                .padding(.horizontal, 4).frame(minWidth: 18, minHeight: 18)
                                .background(HandRect(seed: seed + 5 + i, wobble: 0.35, overshoot: 1.5)
                                    .stroke(text.opacity(0.55), style: StrokeStyle(lineWidth: 1, lineCap: .round, lineJoin: .round)))
                        }
                    }
                }
            }
            .foregroundStyle(text)
            // The primary's brush ends are ragged, so give the words room inside them.
            .padding(.leading, (icon == nil ? 12 : 10) + (role == .primary ? 6 : 0))
            .padding(.trailing, 12 + (role == .primary ? 6 : 0))
            .frame(height: compact ? DS.Size.pill : DS.Size.control)
            .background { background(state) }
            .contentShape(shape)
            .scaleEffect(state.pressed && enabled && !reduceMotion ? 0.97 : 1)
            .animation(DS.easeOut, value: state)
            .focusRing(state.focused)
            .opacity(enabled ? 1 : 0.6)
        }

        /// The primary button's one brush: every primary button is the same stroke.
        static let brush = 4242

        /// Stable per label, so a button keeps its own hand-drawn edge between renders.
        private var seed: Int { title.unicodeScalars.reduce(17) { $0 &* 31 &+ Int($1.value) } }

        /// Drawn like the keycaps: a slightly wobbly edge, never a perfect rounded rectangle.
        @ViewBuilder private func background(_ state: Interaction) -> some View {
            let pressed = state.pressed && enabled
            switch role {
            case .primary:
                // Two passes of one brush, the same on every primary button whatever its words:
                // first a rose-tinted espresso stroke, printed up and left (the poster's
                // misregistration), then the espresso stroke, so where it ran dry the rose shows.
                // Hover: the rose comes up through a drier stroke, nothing moves. Pressed: the
                // whole button shrinks a little under the finger, like every button (see `body`).
                // Reduce Motion: nothing shrinks; the rose comes through fully instead.
                let hover = state.hover && enabled
                let still = pressed && reduceMotion
                ZStack {
                    ZStack {
                        BrushSlab(rgb: Riso.ink, seed: Self.brush + 7, dry: 0)
                        BrushSlab(rgb: Riso.rose, seed: Self.brush + 7, dry: 0)
                            .opacity(still ? 1 : hover ? 0.85 : 0.35)
                    }
                    .offset(x: -2.5, y: -2)
                    .opacity(enabled ? 1 : 0)
                    ZStack {
                        BrushSlab(rgb: Riso.ink, seed: Self.brush, dry: 1.9).opacity(hover ? 0 : 1)
                        BrushSlab(rgb: Riso.ink, seed: Self.brush, dry: 2.6).opacity(hover ? 1 : 0)
                    }
                }
            case .secondary:
                // The keycap treatment: paper face, pen outline, a small drop that flattens on press.
                ZStack {
                    HandRect(seed: seed + 9, wobble: 0.3, closed: true).fill(DS.drop(0.18))
                        .offset(y: pressed ? 0.5 : 1.2)
                    HandRect(seed: seed + 3, wobble: 0.4, closed: true)
                        .fill(state.hover ? DS.Surface.controlHover : DS.Surface.control)
                    HandRect(seed: seed, wobble: 0.45, overshoot: 2)
                        .stroke(DS.Ink.i12.opacity(0.75),
                                style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round))
                }
            case .ghost:
                // The one hover in Iris: a pen-drawn grey ink wash, never a rounded box.
                HandRect(seed: seed, wobble: 0.5, closed: true).fill(state.hover ? DS.Ink.i3 : .clear)
            case .destructiveConfirm:
                HandRect(seed: seed, wobble: 0.45, closed: true).printedInk(Riso.red, seed: seed).brightness(state.hover ? -0.05 : 0)
            }
        }

        private var text: Color {
            switch role {
            case .primary: DS.primaryText
            case .secondary: DS.Ink.i12
            case .ghost: live.hover ? DS.Ink.i12 : DS.Ink.i10
            case .destructiveConfirm: .white
            }
        }

        private var iconInk: Color { role == .secondary ? DS.Ink.i9 : text }
    }
}

/// Back on a turned page of the menu bar slip: a pen arrow, "back" in brush and the esc keycap
/// that does the same, all on one centre line. No box: a grey wash on hover says it's a button.
struct BackButton: View {
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) { EmptyView() }
            .buttonStyle(Style(hovering: hovering))
            .onHover { hovering = $0 }
            .focusEffectDisabled()
            .accessibilityLabel("Back")
    }

    private struct Style: ButtonStyle {
        let hovering: Bool
        func makeBody(configuration: Configuration) -> some View {
            Face(live: Interaction(hover: hovering, pressed: configuration.isPressed))
        }
    }

    struct Face: View {
        let live: Interaction
        @Environment(\.isFocused) private var focused
        @Environment(\.forcedInteraction) private var forced
        @Environment(\.accessibilityReduceMotion) private var reduceMotion

        var body: some View {
            let state = forced ?? Interaction(hover: live.hover, pressed: live.pressed, focused: focused)
            HStack(alignment: .center, spacing: 6) {
                PenArrow().frame(width: 30, height: 16)
                // Brush type sits high in its box; lift it so the arrow points at its middle.
                Text("back").font(DS.Font.brush(24)).foregroundStyle(DS.Ink.i12).offset(y: -3)
                Keycap(key: "esc").scaleEffect(0.85)
            }
            .padding(.leading, 6).padding(.trailing, 2).padding(.vertical, 2)
            .penWash(hover: state.hover, seed: 11)
            .contentShape(Rectangle())
            .scaleEffect(state.pressed && !reduceMotion ? 0.97 : 1)
            .animation(DS.easeOut, value: state)
            .focusRing(state.focused)
        }
    }
}

/// Delete, the zine's way: its crossed-out cup. A small red brush X and the word in the red pen,
/// no box, so it can never be taken for the boxed button beside it. Pointing at it calls
/// `onHover`, so the page can cross out the thing that would go (`DryBrushX` in red over its
/// drawing). Always opens a confirm; never deletes on its own.
struct CrossOutButton: View {
    let title: String
    var onHover: (Bool) -> Void = { _ in }
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) { EmptyView() }
            .buttonStyle(Style(title: title, hovering: hovering))
            .onHover { hovering = $0; onHover($0) }
            .focusEffectDisabled()
    }

    private struct Style: ButtonStyle {
        let title: String
        let hovering: Bool
        func makeBody(configuration: Configuration) -> some View {
            Face(title: title, live: Interaction(hover: hovering, pressed: configuration.isPressed))
        }
    }

    struct Face: View {
        let title: String
        let live: Interaction
        @Environment(\.isFocused) private var focused
        @Environment(\.forcedInteraction) private var forced
        @Environment(\.accessibilityReduceMotion) private var reduceMotion

        var body: some View {
            let state = forced ?? Interaction(hover: live.hover, pressed: live.pressed, focused: focused)
            HStack(spacing: 7) {
                // Solid at this size: the brush's texture would only be noise at 15 pt.
                DryBrushX(seed: 2, width: 0.17, dryness: 0).frame(width: 15, height: 15)
                Text(title).font(DS.Font.body).tracking(-0.12).foregroundStyle(DS.Hue.redText).lineLimit(1).fixedSize()
            }
            .padding(.horizontal, 6).padding(.vertical, 5)
            .penWash(hover: state.hover, seed: title.count * 5 + 1)
            .contentShape(Rectangle())
            .scaleEffect(state.pressed && !reduceMotion ? 0.97 : 1)
            .animation(DS.easeOut, value: state)
            .focusRing(state.focused)
        }
    }
}

/// A status in Iris's own word: a pink pen tick and the word in brush ("ready", "copied"),
/// a degree off straight, like setup's "done". Never a badge.
struct DoneNote: View {
    let text: String
    var size: CGFloat = 20

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                .frame(width: size * 0.72, height: size * 0.58)
            Text(text).font(DS.Font.brush(size)).foregroundStyle(DS.Hue.pinkText).lineLimit(1)
        }
        .rotationEffect(.degrees(-2))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(text)
    }
}

/// Waiting, in ink: three pen dots that darken one after another, in place of the system
/// spinner. Under Reduce Motion they sit still at half ink.
struct InkDots: View {
    var ink: Color = DS.Ink.i11
    var dot: CGFloat = 4
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        // On the test display (`OnDisplay`) it holds one moment, so a capture is of one frame.
        TimelineView(.animation(paused: reduceMotion || OnDisplay.isOn)) { timeline in
            let t = OnDisplay.isOn ? 1000 : timeline.date.timeIntervalSinceReferenceDate
            HStack(spacing: dot * 0.8) {
                ForEach(0..<3, id: \.self) { i in
                    let wave = reduceMotion ? 0.5 : 0.5 + 0.5 * sin(t * 5 - Double(i) * 0.9)
                    HandEllipse(seed: 30 + i, wobble: 0.6).fill(ink)
                        .frame(width: dot, height: dot)
                        .opacity(0.25 + 0.75 * wave)
                }
            }
        }
        .accessibilityLabel("Working")
    }
}

/// A small mark drawn in pen instead of an SF Symbol: plus, up, down, a bin, a tick. Named
/// like the symbols it replaces so callers read the same.
struct PenGlyph: View {
    let name: String
    var size: CGFloat = 11
    var weight: CGFloat = 1.6

    var body: some View {
        PenGlyphShape(name: name)
            .stroke(style: StrokeStyle(lineWidth: weight, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

struct PenGlyphShape: Shape {
    let name: String

    func path(in r: CGRect) -> Path {
        var p = Path()
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: r.minX + x * r.width, y: r.minY + y * r.height) }
        switch name {
        case "plus":
            // A touch off square, like a quick pen cross.
            p.move(to: pt(0.5, 0.06)); p.addQuadCurve(to: pt(0.53, 0.95), control: pt(0.47, 0.5))
            p.move(to: pt(0.05, 0.52)); p.addQuadCurve(to: pt(0.96, 0.47), control: pt(0.5, 0.55))
        case "chevron.up":
            p.move(to: pt(0.1, 0.72)); p.addLine(to: pt(0.5, 0.28)); p.addLine(to: pt(0.92, 0.7))
        case "chevron.down":
            p.move(to: pt(0.1, 0.3)); p.addLine(to: pt(0.5, 0.74)); p.addLine(to: pt(0.92, 0.32))
        case "trash":
            // A bin: lid, handle, a tapered body with two strokes down it.
            p.move(to: pt(0.06, 0.22)); p.addLine(to: pt(0.95, 0.2))
            p.move(to: pt(0.36, 0.2)); p.addLine(to: pt(0.38, 0.05)); p.addLine(to: pt(0.63, 0.05)); p.addLine(to: pt(0.64, 0.2))
            p.move(to: pt(0.17, 0.3)); p.addLine(to: pt(0.25, 0.96)); p.addLine(to: pt(0.76, 0.95)); p.addLine(to: pt(0.84, 0.29))
            p.move(to: pt(0.42, 0.42)); p.addLine(to: pt(0.43, 0.82))
            p.move(to: pt(0.6, 0.42)); p.addLine(to: pt(0.59, 0.82))
        case "arrow.down":
            // A pen arrow pointing down, a touch off straight: the shaft, then two barbs.
            p.move(to: pt(0.5, 0.04)); p.addQuadCurve(to: pt(0.52, 0.9), control: pt(0.46, 0.5))
            p.move(to: pt(0.16, 0.58)); p.addLine(to: pt(0.52, 0.94)); p.addLine(to: pt(0.86, 0.56))
        default:  // "checkmark"
            p.move(to: pt(0.06, 0.55)); p.addLine(to: pt(0.38, 0.88)); p.addQuadCurve(to: pt(0.98, 0.08), control: pt(0.6, 0.4))
        }
        return p
    }
}

/// Icon-only row tool (move up, delete). Squeezes deeper because it's small.
struct IconButton: View {
    let symbol: String
    let label: String
    let action: () -> Void
    @State private var hovering = false
    @Environment(\.isEnabled) private var enabled

    var body: some View {
        Button(action: action) {
            PenGlyph(name: symbol, size: 11)
        }
        .buttonStyle(IconStyle(hovering: hovering))
        .onHover { hovering = $0 && enabled }
        .help(label)
        .accessibilityLabel(label)
        .focusEffectDisabled()
    }

    private struct IconStyle: ButtonStyle {
        let hovering: Bool
        func makeBody(configuration: Configuration) -> some View { IconFace(configuration: configuration, hovering: hovering) }
        struct IconFace: View {
            let configuration: ButtonStyleConfiguration
            let hovering: Bool
            @Environment(\.isEnabled) private var enabled
            @Environment(\.isFocused) private var focused
            @Environment(\.accessibilityReduceMotion) private var reduceMotion
            var body: some View {
                configuration.label
                    .foregroundStyle(hovering ? DS.Ink.i12 : DS.Ink.i10)
                    .frame(width: DS.Size.icon, height: DS.Size.icon)
                    .penWash(hover: hovering, seed: 23)
                    .contentShape(Rectangle())
                    .scaleEffect(configuration.isPressed && enabled && !reduceMotion ? 0.92 : 1)
                    .animation(DS.easeOut, value: configuration.isPressed)
                    .animation(DS.easeOut, value: hovering)
                    .focusRing(focused)
                    .opacity(enabled ? 1 : 0.4)
            }
        }
    }
}

// MARK: - Keycaps (the one place the riso hand stays)

extension Keycap {
    /// One cap per key, like a real keyboard: "⌘R" becomes ⌘ and R. Words and ranges
    /// ("esc", "1–9") and lone keys stay whole.
    static func split(_ combo: String) -> [String] {
        let modifiers: Set<Character> = ["⌘", "⌥", "⌃", "⇧"]
        var caps: [String] = [], rest = ""
        for c in combo {
            if modifiers.contains(c) && rest.isEmpty { caps.append(String(c)) } else { rest.append(c) }
        }
        if !rest.isEmpty { caps.append(rest) }
        return caps
    }
}

struct Keycap: View {
    /// `.labeled` is printed like a Mac key: the symbol small at the top right, the word
    /// along the bottom ("⌃ / control"); a lone letter sits large in the middle.
    enum Size { case chip, labeled }
    let key: String
    var size: Size = .chip
    var seed: Int? = nil
    var symbol: String? = nil
    /// Printed in the pink plate, off register like the menu bar mark: a shortcut that's set.
    var printed = false

    private var loneLetter: Bool { symbol == nil && key.count <= 2 }

    var body: some View {
        let w: CGFloat = switch size {
        case .labeled: loneLetter ? 30 : max(40, 14 + CGFloat(key.count) * 6.6)
        case .chip: max(20, 10 + CGFloat(key.count) * 7)
        }
        let h: CGFloat = size == .labeled ? 32 : 19
        let s = seed ?? key.unicodeScalars.reduce(7) { $0 &* 31 &+ Int($1.value) }
        ZStack {
            // Crisp drop under the key: the tactile 1px drop.
            HandRect(seed: s + 9, wobble: 0.3, closed: true).fill(DS.drop(0.22))
                .offset(y: 1)
            HandRect(seed: s + 3, wobble: 0.45, closed: true)
                .printedInk(printed ? Riso.pink : Riso.crema, seed: s + 3)
                // The riso plate: almost in register, or clearly off it when printed.
                .offset(x: printed ? -1.8 : -0.8, y: printed ? -1.6 : -0.5)
            HandRect(seed: s, wobble: 0.45, overshoot: 2)
                .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.2, lineCap: .round, lineJoin: .round))
            if size == .labeled {
                ZStack {
                    if let symbol {
                        // The system face draws ⌃ ⌥ ⇧ ⌘ ↩ cleanly at this size; the mono face doesn't.
                        Text(symbol).font(.system(size: 12, weight: .semibold))
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                            .padding(.top, 3).padding(.trailing, 5)
                    }
                    Text(key).font(DS.Font.martian(loneLetter ? 13 : 9.5, weight: 560))
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: loneLetter ? .center : .bottomLeading)
                        .padding(.bottom, loneLetter ? 1 : 4).padding(.leading, loneLetter ? 0 : 5)
                }
                .foregroundStyle(DS.Ink.i12)
            } else {
                Text(key)
                    .font(DS.Font.key)
                    .foregroundStyle(DS.Ink.i12)
                    .offset(y: -0.5)
            }
        }
        .frame(width: w, height: h)
        .accessibilityLabel(Text(key))
    }
}

/// A shortcut drawn as the keys you press, printed like a Mac keyboard, side by side.
/// VoiceOver hears it in words.
struct KeyRow: View {
    let keys: [(symbol: String?, word: String)]
    let spoken: String

    var body: some View {
        HStack(spacing: 5) {
            ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
                Keycap(key: key.word, size: .labeled, symbol: key.symbol)
            }
        }
        .fixedSize()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(spoken)
    }
}

extension KeyCombo {
    var keyRow: KeyRow { KeyRow(keys: keycaps, spoken: spoken) }
}

/// Keycap plus what it does, for footers and help rows.
/// A key and what it does. With an action it's also a button, because people click
/// what looks like a command ("Try again") as often as they press the key.
struct KeyHint: View {
    let key: String
    let label: String
    var action: (() -> Void)? = nil
    @State private var hovering = false

    var body: some View {
        if let action {
            Button(action: action) { face(hover: hovering) }
                .buttonStyle(SqueezeStyle(scale: 0.96))
                .onHover { hovering = $0 }
                .animation(DS.easeOut, value: hovering)
                .accessibilityLabel(label)
                .accessibilityHint("Or press \(key)")
        } else {
            face(hover: false)
        }
    }

    private func face(hover: Bool) -> some View {
        HStack(spacing: 6) {
            HStack(spacing: 3) { ForEach(Array(Keycap.split(key).enumerated()), id: \.offset) { Keycap(key: $0.element) } }
            Text(label).font(DS.Font.caption).foregroundStyle(hover ? DS.Ink.i12 : DS.Ink.i10).textCase(.uppercase).tracking(0.4)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .penWash(hover: hover, seed: label.count * 3 + 5)
        .padding(.horizontal, -6)
        .contentShape(Rectangle())
    }
}

// MARK: - Tone tabs (the panel's poster row)

/// The poster's drink labels: small mono caps for the choices, the chosen one lettered in
/// brush with a pink marker stroke under it, the way "Cortado" and "Latte" sit over their
/// cups. Each tab reserves the width of both faces, so choosing never shifts the row.
struct ToneTabs: View {
    let labels: [String]
    let selected: Int
    var name = "Tone"
    let pick: (Int) -> Void
    @Namespace private var stroke
    @State private var hovered: Int?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 18) {
            ForEach(Array(labels.enumerated()), id: \.offset) { index, label in
                let chosen = index == selected
                Button { pick(index) } label: {
                    ZStack(alignment: .bottomLeading) {
                        // Both faces, hidden, hold the widest width and the tallest line.
                        brushFace(label).hidden()
                        monoFace(label).hidden()
                        if chosen {
                            brushFace(label)
                                .background(alignment: .bottom) {
                                    let mark = MarkerSwipe(height: 6, seed: index + 3)
                                        .rotationEffect(.degrees(-1.2))
                                        .padding(.horizontal, -3)
                                        .offset(y: 1)
                                    if reduceMotion { mark } else { mark.matchedGeometryEffect(id: "stroke", in: stroke) }
                                }
                        } else {
                            monoFace(label)
                                .foregroundStyle(hovered == index ? DS.Ink.i12 : DS.Ink.i10)
                                .padding(.bottom, 4)
                        }
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(SqueezeStyle(scale: 0.97))
                .onHover { hovered = $0 ? index : (hovered == index ? nil : hovered) }
                .animation(DS.easeOut, value: hovered)
                .accessibilityLabel(label)
                .accessibilityAddTraits(chosen ? .isSelected : [])
            }
        }
        .animation(reduceMotion ? DS.Motion.fast.enterAnimation : DS.Motion.pill, value: selected)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(name)
    }

    private func brushFace(_ label: String) -> some View {
        Text(label).font(DS.Font.brush(23)).foregroundStyle(DS.Ink.i12).lineLimit(1).fixedSize()
    }

    private func monoFace(_ label: String) -> some View {
        // Mixed case, like the poster's spec lines ("Cortado / 1:1"): quicker to skim than caps.
        Text(label).font(DS.Font.body).tracking(-0.12).lineLimit(1).fixedSize()
    }
}

// MARK: - Ink choice (settings choices)

/// A choice between a few options, set like the panel's tone tabs: plain mono for the
/// others, the chosen one lettered in brush over a pink marker stroke.
struct InkChoice<Value: Hashable>: View {
    let options: [(value: Value, label: String)]
    @Binding var selection: Value
    /// What's being chosen, for VoiceOver: the row's title.
    let name: String

    var body: some View {
        ToneTabs(labels: options.map(\.label),
                 selected: options.firstIndex { $0.value == selection } ?? 0,
                 name: name) { selection = options[$0].value }
            .fixedSize()
    }
}

struct SqueezeStyle: ButtonStyle {
    var scale: CGFloat = 0.97
    func makeBody(configuration: Configuration) -> some View { SqueezeFace(configuration: configuration, scale: scale) }

    private struct SqueezeFace: View {
        let configuration: ButtonStyleConfiguration
        let scale: CGFloat
        @Environment(\.accessibilityReduceMotion) private var reduceMotion
        @Environment(\.isFocused) private var focused
        @Environment(\.forcedInteraction) private var forced
        var body: some View {
            configuration.label
                .scaleEffect(configuration.isPressed && !reduceMotion ? scale : 1)
                .animation(DS.easeOut, value: configuration.isPressed)
                // Keyboard focus is the pen's, here as on the buttons: never the system's ring.
                .focusRing(forced?.focused ?? focused)
                .focusEffectDisabled()
        }
    }
}

// MARK: - Switch and checkbox

struct TactileSwitch: View {
    @Binding var isOn: Bool
    var label: String
    @Environment(\.isEnabled) private var enabled
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let pen = StrokeStyle(lineWidth: 1.2, lineCap: .round, lineJoin: .round)
        Button { isOn.toggle() } label: {
            // A key that slides in a hand-drawn slot; the slot takes the pink plate when on.
            ZStack(alignment: isOn ? .trailing : .leading) {
                Group {
                    if isOn { HandRect(seed: 61, wobble: 0.4, closed: true).printedInk(Riso.pink, seed: 61) }
                    else { HandRect(seed: 61, wobble: 0.4, closed: true).fill(DS.Ink.i3) }
                }
                .offset(x: -0.8, y: -0.5)
                HandRect(seed: 63, wobble: 0.45, overshoot: 2).stroke(DS.Ink.i12, style: pen)
                ZStack {
                    HandRect(seed: 65, wobble: 0.3, closed: true).fill(DS.drop(0.22)).offset(y: 1)
                    HandRect(seed: 66, wobble: 0.35, closed: true)
                        .printedInk(isOn ? Riso.ink : Riso.crema, seed: 66)
                    HandRect(seed: 67, wobble: 0.35, overshoot: 1.5).stroke(DS.Ink.i12, style: pen)
                }
                .frame(width: 20, height: 16)
                .padding(4)
            }
            .frame(width: 50, height: 26)
            .animation(reduceMotion ? nil : DS.Motion.knob, value: isOn)
            .animation(DS.easeOut, value: isOn)
        }
        .buttonStyle(SqueezeStyle(scale: 0.96))
        .opacity(enabled ? 1 : 0.6)
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "On" : "Off")
    }
}

struct TactileCheckbox: View {
    @Binding var isOn: Bool
    var label: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private var pop: CGFloat { isOn || reduceMotion ? 1 : 0.9 }

    var body: some View {
        Button { isOn.toggle() } label: {
            // The switch's sibling: the pink plate printed off register under a pen edge, and
            // an ink tick that overshoots the box, as if marked off in a hurry.
            ZStack {
                Group {
                    if isOn { HandRect(seed: 23, wobble: 0.35, closed: true).printedInk(Riso.pink, seed: 23) }
                    else { HandRect(seed: 23, wobble: 0.35, closed: true).fill(DS.Surface.top) }
                }
                .offset(x: -0.8, y: -0.5)
                HandRect(seed: 24, wobble: 0.4, overshoot: 2)
                    .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.2, lineCap: .round, lineJoin: .round))
                HandTick()
                    .trim(from: 0, to: isOn || reduceMotion ? 1 : 0)
                    .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 2.2, lineCap: .round, lineJoin: .round))
                    .frame(width: 15, height: 14).offset(x: 2.5, y: -2.5)
                    .scaleEffect(pop).opacity(isOn ? 1 : 0)
            }
            .frame(width: 18, height: 18)
            .animation(DS.easeOut, value: isOn)
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.92))
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "On" : "Off")
    }
}

// MARK: - Field (a well)

struct WellField: View {
    let placeholder: String
    @Binding var text: String
    var onSubmit: () -> Void = {}
    @FocusState private var focused: Bool

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.plain)
            .overlay(alignment: .leading) {
                if text.isEmpty {
                    Text(placeholder).font(DS.Font.body).foregroundStyle(DS.Ink.i10).allowsHitTesting(false)
                }
            }
            .font(DS.Font.body)
            .foregroundStyle(DS.Ink.i12)
            .focused($focused)
            .onSubmit(onSubmit)
            .padding(.horizontal, 10)
            .frame(height: DS.Size.control)
            .background(HandRect(seed: 81, wobble: 0.4, closed: true).fill(focused ? DS.Surface.wellFocus : DS.Surface.well))
            .overlay(HandRect(seed: 83, wobble: 0.45, overshoot: 2)
                .stroke(DS.focusRing, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                .padding(-2).opacity(focused ? 1 : 0))
            .animation(DS.easeOut, value: focused)
    }
}

// MARK: - Card and table

struct TableHeader: View {
    let columns: [(String, CGFloat?)]
    var body: some View {
        HStack(spacing: DS.Space.md) {
            ForEach(Array(columns.enumerated()), id: \.offset) { _, col in
                Text(col.0).font(DS.Font.caption).textCase(.uppercase).tracking(0.5).foregroundStyle(DS.Ink.i10)
                    .frame(width: col.1, alignment: .leading)
                if col.1 == nil { Spacer(minLength: 0) }
            }
        }
        .padding(.horizontal, DS.Space.md)
        .frame(height: 30)
        .overlay(alignment: .bottom) { PenRule().offset(y: 1.5) }
    }
}

/// A table row: its own hover fill fading in place, a rule below unless it's the last.
struct TableRow<Content: View>: View {
    var last = false
    var dimmed = false
    /// Each row's rule wobbles its own way: its place in the table, so it's the same rule
    /// every time the page is drawn.
    var seed = 0
    @ViewBuilder let content: Content
    @State private var hovering = false
    private var rowSeed: Int { 211 + seed * 37 }
    var body: some View {
        HStack(spacing: DS.Space.md) { content }
            .padding(.horizontal, DS.Space.md)
            .frame(minHeight: DS.Size.row)
            .penWash(hover: hovering, seed: rowSeed)
            .opacity(dimmed ? 0.5 : 1)
            .overlay(alignment: .bottom) { if !last { PenRule(seed: rowSeed).offset(y: 1.5) } }
            .onHover { hovering = $0 }
            .animation(DS.easeOut, value: hovering)
    }
}

// MARK: - Badge

struct Badge: View {
    enum Tone { case warn, red }
    let text: String
    let tone: Tone

    var body: some View {
        Text(text).font(DS.Font.caption)
            .foregroundStyle(ink)
            .padding(.horizontal, 8)
            .frame(height: 20)
            .background {
                // A tinted label with a pen outline, like the keycaps, never a machine capsule.
                let seed = text.unicodeScalars.reduce(13) { $0 &* 31 &+ Int($1.value) }
                ZStack {
                    HandRect(seed: seed + 3, wobble: 0.35, closed: true).fill(fill)
                    HandRect(seed: seed, wobble: 0.4, overshoot: 2)
                        .stroke(ink.opacity(0.45), style: StrokeStyle(lineWidth: 1, lineCap: .round, lineJoin: .round))
                }
            }
    }

    private var ink: Color {
        switch tone {
        case .warn: DS.Hue.warnText
        case .red: DS.Hue.redText
        }
    }
    private var fill: Color {
        switch tone {
        case .warn: DS.Hue.warnTint
        case .red: DS.Hue.redTint
        }
    }
}

// MARK: - Riso art

/// Riso printing: dithered plates for icons and sprites, and the still iris.
enum RisoArt {
    enum Crop { case whole, bloom, icon }

    /// The iris as a still: the ⌃G flower (`FlowerRig`) paused at rest, so every flower in Iris
    /// is the same drawing. `.whole` stands on its ground, framed like a sprite (foot near the
    /// bottom); `.bloom` is the head alone, for small sizes and the app icon. `open: false` is
    /// the closed bud on its stem. `.icon` centres the bloom with its stem running to the
    /// bottom edge, for a tile that clips it. `scale` is pixels per point of the image.
    @MainActor
    static func flower(points: CGFloat = 80, crop: Crop = .whole, open: Bool = true, scale: CGFloat = 3) -> NSImage {
        let key = "\(points)-\(crop)-\(open)-\(scale)"
        if let hit = stills[key] { return hit }
        let clock = RigClock(now: 1010, arrived: 1000, bloomed: open ? 1001 : nil, full: false)
        // In the rig's own points at size 92: the foot at (90, 166), the head 70 above it.
        let frame = switch crop {
        case .whole: CGRect(x: 29, y: 53, width: 122, height: 122)
        case .bloom: CGRect(x: 48, y: 50, width: 84, height: 84)
        case .icon: CGRect(x: 36, y: 50, width: 108, height: 108)
        }
        let view = FlowerRig(clock: clock, size: 92)
            .offset(x: -frame.minX, y: -frame.minY)
            .frame(width: frame.width, height: frame.height, alignment: .topLeading)
            .clipped()
            .environment(\.colorScheme, .light)
        let renderer = ImageRenderer(content: view)
        renderer.scale = scale * points / frame.width
        let image = renderer.nsImage ?? NSImage(size: NSSize(width: points, height: points))
        image.size = NSSize(width: points, height: points)
        stills[key] = image
        return image
    }
    @MainActor private static var stills: [String: NSImage] = [:]

    typealias Plate = (rgb: (UInt8, UInt8, UInt8), dx: Int, dy: Int, draw: (CGContext) -> Void)

    /// Prints greyscale plates one over another like a riso: each plate ordered-dithered
    /// (grey sets the dot density) in its own ink, nudged off register by (dx, dy).
    static func print(pixels: Int, seed: Int, plates: [Plate]) -> NSImage {
        let w = pixels, h = pixels
        var out = [UInt8](repeating: 0, count: w * h * 4)
        var rng = SeededRandom(seed: seed)
        for plate in plates {
            let gray = render(w, h, plate.draw)
            for y in 0..<h {
                for x in 0..<w {
                    let sx = x - plate.dx, sy = y - plate.dy
                    guard sx >= 0, sy >= 0, sx < w, sy < h else { continue }
                    let dark = 1 - CGFloat(gray[sy * w + sx]) / 255
                    guard dark > 0.02 else { continue }
                    let t = (Self.bayer[y % 4][x % 4] + 0.5) / 16 + (rng.next() - 0.5) * 0.35
                    if dark > t {
                        let o = (y * w + x) * 4
                        out[o] = plate.rgb.0; out[o + 1] = plate.rgb.1; out[o + 2] = plate.rgb.2; out[o + 3] = 255
                    }
                }
            }
        }
        return image(rgba: out, width: w, height: h)
    }

    /// A 4×4 Bayer screen: the threshold each dot of a plate is printed against.
    static let bayer: [[CGFloat]] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]

    /// RGBA bytes, row by row from the top, as an image of that many points.
    static func image(rgba: [UInt8], width w: Int, height h: Int, points: NSSize? = nil) -> NSImage {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8,
                                   samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                   bytesPerRow: w * 4, bitsPerPixel: 32)!
        _ = rgba.withUnsafeBufferPointer { memcpy(rep.bitmapData!, $0.baseAddress!, w * h * 4) }
        let size = points ?? NSSize(width: w, height: h)
        rep.size = size
        let image = NSImage(size: size)
        image.addRepresentation(rep)
        return image
    }

    private static func render(_ w: Int, _ h: Int, _ draw: (CGContext) -> Void) -> [UInt8] {
        var pixels = [UInt8](repeating: 255, count: w * h)
        pixels.withUnsafeMutableBytes { buffer in
            let ctx = CGContext(data: buffer.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w,
                                space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
            ctx.translateBy(x: 0, y: CGFloat(h))
            ctx.scaleBy(x: 1, y: -1)  // draw with a top-left origin, like the mockups
            draw(ctx)
        }
        return pixels
    }

}

/// "IRIS" in the doodle face. The flower is drawn beside it where there's room, never in it.
struct IrisLogo: View {
    /// The size the wordmark is asked for; it draws 30% bigger with a heavy pen, so the app's
    /// name stands out wherever it appears.
    var size: CGFloat = 30
    var body: some View {
        word
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Iris")
    }

    /// The doodle I, a little room, then a hollow R, a small lowercase i and a hollow S, the
    /// hollows outlined in a heavy pen with the pink plate printed off register inside.
    private var word: some View {
        let s = size * 1.3
        return HStack(alignment: .lastTextBaseline, spacing: s * 0.02) {
            Text("I").font(DS.Font.logo(s)).foregroundStyle(DS.Ink.i12)
            HandWord(word: "RIS", size: s * 0.9, pattern: [.hollow, .small, .hollow], weight: 1.6)
                .padding(-s * 0.1)
                .alignmentGuide(.lastTextBaseline) { $0[.bottom] - s * 0.2 }
        }
    }
}

/// A link drawn like the rest of Iris: pen underline, a small printed pen arrow turned up
/// (↗), as links that leave Iris for the browser are marked.
struct PenLink: View {
    let title: String
    let url: URL
    @State private var hovering = false

    var body: some View {
        Button { NSWorkspace.shared.open(url) } label: { face }
            .buttonStyle(SqueezeStyle(scale: 0.98))
            .onHover { hovering = $0 }
            .accessibilityLabel(title)
            .accessibilityHint("Opens in your browser")
    }

    private var face: some View {
        HStack(spacing: 4) {
            Text(title).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                .background(alignment: .bottom) {
                    // A quick pen stroke printed in pink, pressing in and lifting off.
                    InkRibbon(line: .under(seed: title.count), width: 2.6, startTaper: 0.6, endTaper: 0.2)
                        .printedInk(hovering ? Riso.pinkText : Riso.pink, seed: title.count)
                        .frame(height: 5).offset(y: 4)
                }
            // A touch smaller than beside a turn: turned up, it reaches higher.
            PenArrow(seed: title.count + 9, weight: PenArrow.besideWordsWeight * 0.9, right: true)
                .frame(width: PenArrow.besideWords.width * 0.8, height: PenArrow.besideWords.height * 0.8)
                .rotationEffect(.degrees(-35))
        }
    }
}

