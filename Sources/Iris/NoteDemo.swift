import AppKit
import SwiftUI

/// Settings → General → When Iris speaks up: a switch for each message Iris can leave by
/// your text (a `Toast`, a "note" in code), under a stage that plays what it looks like.
/// Words alone don't say much; seeing the message land on a text box does.
///
/// With nothing pointed at, the stage plays every note in turn and marks the row it's
/// showing. Pointing at a row, or tabbing to its switch, holds that row's clip.
struct NotesSection: View {
    @ObservedObject private var settings = AppSettings.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Snapshots hold every clip still at one moment.
    @Environment(\.designState) private var design
    @State private var pointed: AppSettings.Note?
    @FocusState private var focused: AppSettings.Note?
    /// When the current clip (or run of clips) started, and which note the run starts on.
    @State private var start = Date()
    @State private var first = 0
    private static let rows: [(note: AppSettings.Note, title: String, detail: String, label: String)] = [
        (.replaced, "After replacing", "Says “fixed 4 words” or “replaced”.", "Show a message after replacing"),
        (.undoKeys, "With the undo keys", "Adds “Undo with ⌘ Z” to that message.", "Show the undo keys after replacing"),
        (.checking, "While Quick Fix works", "Says “checking…” while Iris reads your text.", "Show a message while Quick Fix works"),
        (.clean, "When there's nothing to fix", "Says “looks good”.", "Show a message when there's nothing to fix"),
        (.excluded, "Where Iris is off", "Says “turned off here” if you press the shortcut there.", "Show a message where Iris is off"),
    ]

    var body: some View {
        Section(number: 2, title: "When Iris speaks up",
                detail: "After you press the keys, Iris tells you what happened in a small message by your text. It fades on its own. Errors always show.") {
            let still = reduceMotion || design?.noteAt != nil
            SheetGroup {
                // Only the stage is drawn every frame. The rows change when the stage moves on
                // to the next message, so they're drawn then: once a clip, not thirty times a second.
                TimelineView(.animation(minimumInterval: 1 / 30, paused: still)) { context in
                    let (note, t) = playing(at: context.date)
                    NoteDemo(note: note, t: t, keys: keys,
                             showsNote: settings.shows(note == .undoKeys ? .replaced : note),
                             showsUndoKeys: settings.shows(.undoKeys))
                        .padding(.bottom, 8)
                }
                TimelineView(.periodic(from: start, by: NoteDemo.length)) { context in
                    // A hair past the tick, so it never rounds down into the clip just ended.
                    let note = playing(at: still ? context.date : context.date.addingTimeInterval(0.05)).0
                    VStack(alignment: .leading, spacing: DS.Space.md) {
                        ForEach(Array(Self.rows.enumerated()), id: \.offset) { i, row in
                            noteRow(row, current: row.note == note)
                            if i < Self.rows.count - 1 { PenRule(seed: 700 + i) }
                        }
                    }
                }
            }
        }
        .onChange(of: focused) { _, note in hold(note) }
    }

    private func noteRow(_ row: (note: AppSettings.Note, title: String, detail: String, label: String),
                         current: Bool) -> some View {
        SettingRow(title: row.title, detail: row.detail) {
            TactileSwitch(isOn: settings.showsBinding(row.note), label: row.label)
                .focused($focused, equals: row.note)
        }
        // The keys ride on the replace note: with that off there's nothing to add them to.
        .disabled(row.note == .undoKeys && !settings.shows(.replaced))
        .opacity(row.note == .undoKeys && !settings.shows(.replaced) ? 0.45 : 1)
        .padding(.horizontal, 8).padding(.vertical, 6)
        // The row on the stage takes the rail's hover fill, so the two read as one.
        .penWash(hover: current, seed: row.title.count * 5 + 1)
        .padding(.horizontal, -8)
        .contentShape(Rectangle())
        .onHover { inside in
            if inside { hold(row.note) } else if pointed == row.note { hold(nil) }
        }
        .animation(DS.easeOut, value: current)
    }

    /// The keys the user really presses, as their keyboard prints them: Quick Fix, or Open
    /// Iris when Quick Fix is off. A shortcut they recorded shows as theirs.
    private var keys: [KeyCombo.Cap] { (settings.quickFixShortcut ?? settings.openShortcut).keycaps }

    private func hold(_ note: AppSettings.Note?) {
        guard note != pointed else { return }
        // Letting go carries on the run from the note that was held.
        if note == nil, let held = pointed, let index = Self.rows.firstIndex(where: { $0.note == held }) { first = index }
        pointed = note
        start = Date()
    }

    private func playing(at date: Date) -> (AppSettings.Note, Double) {
        let held = pointed ?? focused
        if reduceMotion || design?.noteAt != nil {
            let note = held ?? Self.rows[first].note
            return (note, design?.noteAt ?? NoteDemo.landed(note))
        }
        let elapsed = max(date.timeIntervalSince(start), 0)
        let local = elapsed.truncatingRemainder(dividingBy: NoteDemo.length)
        if let held { return (held, local) }
        let index = (first + Int(elapsed / NoteDemo.length)) % Self.rows.count
        return (Self.rows[index].note, local)
    }
}

/// About four seconds of a note in use: an app window in the middle of a quiet sky-blue
/// desk, someone's text in its message box, the keys they press (drawn caps that sink one by
/// one and print pink), and the note arriving on the box's top edge, where Iris really puts
/// it. Drawn from `t` (seconds into the clip) with the real `ToastView`, so it always matches
/// the app.
struct NoteDemo: View {
    let note: AppSettings.Note
    let t: Double
    /// The keys the user presses to fix, shown under the window.
    var keys: [KeyCombo.Cap]
    /// Off when the user turned this note off: the clip plays without it.
    var showsNote = true
    /// Off when the undo keys are turned off: the replace note comes without them.
    var showsUndoKeys = true

    static let length = 4.4
    /// A stand-in app icon for the "turned off here" clip, crossed out like the real note.
    @MainActor static let appIcon = NSWorkspace.shared.icon(for: .application).flattened()
    /// A moment when the note is on the box, for Reduce Motion.
    static func landed(_ note: AppSettings.Note) -> Double { note == .checking ? 2.2 : 2.8 }

    /// The app window's width on the desk; the desk fills the column around it.
    static let windowWidth: CGFloat = 468
    /// Room kept at the bottom of the desk for the Dock.
    static let dockRoom: CGFloat = 58

    var body: some View {
        VStack(spacing: 22) {
            window
            .frame(width: Self.windowWidth)
            .paperShadow(spread: 18, drop: 10)
            .overlay(alignment: .topTrailing) {
                slip.fixedSize()
                    .overlay(alignment: .trailing) {
                        // On the undo keys' clip, the pink pen rings what that switch adds.
                        if note == .undoKeys && showsUndoKeys {
                            BrushRing(rgb: Riso.pinkText, seed: 17, drawn: ringIn)
                                .frame(width: 60, height: 36).padding(.trailing, 8)
                        }
                    }
                    .scaleEffect(0.82, anchor: .topTrailing).rotationEffect(.degrees(-1))
                    .offset(x: 10, y: -16)
                    .opacity(showsNote ? slipIn : 0)
            }
            keyCard
            .opacity(keysIn)
            .offset(y: 10 * (1 - keysIn))
        }
        .padding(.top, 44)
        .padding(.bottom, 30 + Self.dockRoom)
        .frame(maxWidth: .infinity)
        // A desktop round the window, so it reads as a Mac: a few files lying on it and a Dock
        // along the bottom, small and quiet next to the window.
        .background(alignment: .topLeading) {
            VStack(spacing: 14) {
                DesktopIcon(kind: .folder, name: "Drafts", seed: 81)
                DesktopIcon(kind: .page, name: "notes.txt", seed: 84)
            }
            .padding(.top, 22).padding(.leading, 14)
        }
        .background(alignment: .trailing) {
            VStack(spacing: 14) {
                DesktopIcon(kind: .picture, name: "Screenshot", seed: 87)
                DesktopIcon(kind: .folder, name: "Invoices", seed: 90)
            }
            .padding(.trailing, 12).padding(.bottom, 40)
        }
        .overlay(alignment: .bottom) { DeskDock().padding(.bottom, 8) }
        // The desk: Iris's sky, printed.
        .background(ScreenPlate(ink: .sky, seed: 19))
        .clipShape(HandRect(seed: 63, wobble: 0.4, closed: true))
        .overlay(HandRect(seed: 64, wobble: 0.5, overshoot: 2)
            .stroke(DS.Ink.i12.opacity(0.7), style: StrokeStyle(lineWidth: 1.2, lineCap: .round)))
        // Decoration: each row's words say the same thing to VoiceOver.
        .accessibilityHidden(true)
    }

    /// The user's keys on a scrap of paper, in Mac order, joined by brush plus signs that turn
    /// pink as the chord builds. "you press" is written on a strip of masking tape stuck
    /// across the card's corner, so it stays with the keys whatever they are.
    private var keyCard: some View {
        HStack(spacing: 3) {
            ForEach(Array(keys.enumerated()), id: \.offset) { i, key in
                if i > 0 {
                    Text("+").font(DS.Font.brush(16))
                        .foregroundStyle(t >= keyDown(i) && t < keysUp ? DS.Hue.pinkText : DS.color(Riso.tan))
                }
                DrawnKeycap(symbol: key.symbol, word: key.word, down: pressed(i), flick: flick(i), seed: 70 + i * 9)
            }
        }
        .padding(.horizontal, 13).padding(.top, 21).padding(.bottom, 10)
        .background(DS.Surface.overlay.grain(0.22))
        .paperShadow(spread: 10, drop: 7)
        .overlay(alignment: .topLeading) {
            // Pressed down a beat after the card arrives: from a little bigger and more crooked.
            let k = tapeOn
            Text("you press").font(DS.Font.brush(15)).foregroundStyle(DS.Ink.i12)
                .padding(.horizontal, 12).padding(.top, 2).padding(.bottom, 3)
                .background(TapeLabel.Tape().fill(DS.Hue.tape).blendMode(.multiply))
                .scaleEffect(1.15 - 0.15 * k)
                .rotationEffect(.degrees(-7 - 6 * (1 - k)))
                .opacity(k)
                .offset(x: -13, y: -11)
        }
        .rotationEffect(.degrees(-0.8))
    }

    // MARK: The app

    /// A tiny app window drawn in pen, so the box reads as somewhere you type: three window
    /// buttons and the app's name, their message, and your reply in its box with a caret.
    private var window: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 5) {
                ForEach(0..<3, id: \.self) { i in
                    HandEllipse(seed: 30 + i, wobble: 0.3).stroke(DS.Ink.i12.opacity(0.6), lineWidth: 1.1)
                        .frame(width: 9, height: 9)
                }
                // The name sits by the buttons: the note lands over the window's right end.
                Text(note == .excluded ? "1Password" : "Messages").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    .padding(.leading, 8)
                Spacer()
            }
            .padding(.horizontal, 10).frame(height: 24)
            PenRule(seed: 8)
            // Every clip keeps the same rows, so the window (and the switches under it) never
            // jumps as the clips change: the password manager puts a label where the message is.
            ZStack(alignment: .leading) {
                Text("Did you get to the PR?").font(DS.Font.caption).foregroundStyle(DS.Ink.i11)
                    .padding(.horizontal, 10).padding(.vertical, 6)
                    .background(HandRect(seed: 9, wobble: 0.5, closed: true).fill(DS.Ink.i2))
                    .opacity(note == .excluded ? 0 : 1)
                Text("Password").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    .opacity(note == .excluded ? 1 : 0)
            }
            .padding([.horizontal, .top], 10)
            HStack(spacing: 0) {
                // Each version carries its own caret, so the caret sits at the end of the
                // words you can see and moves when they change.
                ZStack(alignment: .leading) {
                    typed(draft).opacity(1 - fixedIn)
                    typed(Self.fixed).opacity(fixedIn)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
            .background(HandRect(seed: 41, wobble: 0.5, closed: true).fill(DS.Surface.top))
            .overlay(HandRect(seed: 43, wobble: 0.5, overshoot: 2)
                .stroke(DS.Ink.i12.opacity(0.6), style: StrokeStyle(lineWidth: 1, lineCap: .round)))
            .padding(8)
        }
        .background(HandRect(seed: 51, wobble: 0.5, closed: true).fill(DS.Surface.panel))
        .overlay(HandRect(seed: 52, wobble: 0.5, overshoot: 2)
            .stroke(DS.Ink.i12.opacity(0.7), style: StrokeStyle(lineWidth: 1.2, lineCap: .round)))
    }

    /// The words in the box with the caret after them, blinking.
    private func typed(_ text: String) -> some View {
        HStack(spacing: 1) {
            Text(text).font(DS.Font.subtitle).foregroundStyle(DS.Ink.i11)
            Rectangle().fill(DS.Ink.i12).frame(width: 1.6, height: 19)
                .opacity(Int(t * 2.4) % 2 == 0 ? 1 : 0)
        }
    }

    // MARK: Timing

    /// 0 → 1 between two moments, with the strong ease-out.
    private func ramp(_ a: Double, _ b: Double) -> Double {
        let x = min(max((t - a) / (b - a), 0), 1)
        return 1 - pow(1 - x, 3)
    }
    private var leaving: Double { 1 - ramp(3.9, 4.2) }
    private var keysIn: Double { ramp(0.45, 0.7) * leaving }
    private var tapeOn: Double { ramp(0.55, 0.75) }
    /// The keys go down one after another in Mac order, however many there are, and come up
    /// together before the fix lands.
    private func keyDown(_ i: Int) -> Double { 0.8 + Double(i) * min(0.09, 0.35 / Double(max(keys.count - 1, 1))) }
    private var keysUp: Double { 1.25 }
    private func pressed(_ i: Int) -> Double {
        let x = min(max((t - keyDown(i)) / 0.07, 0), 1), up = min(max((t - keysUp) / 0.12, 0), 1)
        return (1 - pow(1 - x, 3)) * (1 - up)
    }
    /// Three ink flicks over a key as it goes down: up quickly, then fading.
    private func flick(_ i: Int) -> Double {
        let x = (t - keyDown(i)) / 0.26
        guard x > 0, x < 1 else { return 0 }
        return x < 0.33 ? 1 - pow(1 - x / 0.33, 3) : 1 - pow((x - 0.33) / 0.67, 3)
    }
    /// The text turns into the fixed version (back to the draft as the clip loops).
    private var fixedIn: Double {
        switch note {
        case .replaced, .undoKeys: ramp(1.3, 1.6) * (1 - ramp(4.1, 4.4))
        case .checking: ramp(2.75, 3.05) * (1 - ramp(4.1, 4.4))
        case .clean, .excluded: 0
        }
    }
    private var slipIn: Double {
        switch note {
        case .replaced, .undoKeys: ramp(1.7, 1.9) * leaving
        case .checking: ramp(1.2, 1.4) * (1 - ramp(2.95, 3.15))
        case .clean, .excluded: ramp(1.25, 1.45) * leaving
        }
    }

    /// The pen ring round the undo keys draws itself once the note has landed.
    private var ringIn: Double { ramp(2.1, 2.6) }

    // MARK: Words

    private static let fixed = "Hey, I checked the PR and it looks fine."
    private var draft: String {
        switch note {
        case .replaced, .undoKeys, .checking: "hey i checked the pr and its look fine"
        case .clean: "Thanks, see you at 3."
        case .excluded: "••••••••••••"
        }
    }

    /// The message the app shows for this note (`Message`).
    private var slip: ToastView {
        switch note {
        case .replaced, .undoKeys: ToastView(message: .fixed(4, undoKeys: showsUndoKeys))
        case .checking: ToastView(message: .checking)
        case .clean: ToastView(message: .looksGood)
        case .excluded: ToastView(message: .turnedOff(name: "1Password", icon: NoteDemo.appIcon))
        }
    }
}

/// One key, drawn from above in a heavy pen line: a crema face with the pink plate printed a
/// few dots off it. Pressed, the face sinks a little, the plate snaps under it and the face
/// prints pink, with three ink flicks over it. Printed like a Mac key: the symbol small at the
/// top right and the word along the bottom; a lone letter or F-key large in the middle; a
/// named key (Space, esc) as its word on a wider cap.
struct DrawnKeycap: View {
    let symbol: String?
    let word: String
    /// 0 up … 1 all the way down.
    var down: Double = 0
    /// The ink flicks over the key, 0 … 1.
    var flick: Double = 0
    var seed = 70

    private var lone: Bool { symbol == nil && word.count <= 3 && word != "esc" }
    private var size: CGSize {
        // About the height of the message box above, so the keys support the window
        // rather than outweigh it.
        if symbol != nil { return CGSize(width: 46, height: 37) }
        if lone { return CGSize(width: 37, height: 37) }
        return CGSize(width: max(46, 22 + CGFloat(word.count) * 7.5), height: 37)
    }

    var body: some View {
        let d = CGFloat(down)
        ZStack {
            HandRect(seed: seed + 3, wobble: 0.5, closed: true).printedInk(Riso.pink, seed: seed + 3)
                .offset(x: -4 * (1 - d), y: 3 * (1 - d))
            ZStack {
                HandRect(seed: seed + 1, wobble: 0.5, closed: true).printedInk(Riso.crema, seed: seed + 1)
                HandRect(seed: seed + 1, wobble: 0.5, closed: true).printedInk(Riso.pink, seed: seed + 5).opacity(down)
                HandRect(seed: seed + 2, wobble: 0.4, overshoot: 1.5)
                    .stroke(DS.Ink.i12.opacity(0.28), lineWidth: 1)
                    .padding(EdgeInsets(top: 4, leading: 4, bottom: 6, trailing: 4))
                HandRect(seed: seed, wobble: 0.7, overshoot: 2.5)
                    .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.9, lineCap: .round, lineJoin: .round))
                face
            }
            .scaleEffect(1 - 0.07 * d)
            .offset(y: 1.5 * d)
            flicks
        }
        .frame(width: size.width, height: size.height)
    }

    @ViewBuilder private var face: some View {
        if let symbol {
            ZStack {
                // The system face draws ⌃ ⌥ ⇧ ⌘ cleanly; the mono face doesn't.
                Text(symbol).font(.system(size: 11.5, weight: .semibold))
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                    .padding(.top, 4).padding(.trailing, 6)
                Text(word).font(DS.Font.martian(7.5, weight: 560))
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                    .padding(.bottom, 6).padding(.leading, 6)
            }
            .foregroundStyle(DS.Ink.i12)
        } else if lone {
            Text(word).font(word.count == 1 ? DS.Font.martian(15.5, weight: 600) : DS.Font.martian(11.5, weight: 600))
                .foregroundStyle(DS.Ink.i12)
        } else {
            Text(word.lowercased()).font(DS.Font.martian(8, weight: 560)).foregroundStyle(DS.Ink.i12)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                .padding(.bottom, 6).padding(.leading, 6)
        }
    }

    private var flicks: some View {
        Path { p in
            p.move(to: CGPoint(x: 6, y: 14)); p.addLine(to: CGPoint(x: 1, y: 6))
            p.move(to: CGPoint(x: 18, y: 12)); p.addLine(to: CGPoint(x: 18, y: 2))
            p.move(to: CGPoint(x: 30, y: 14)); p.addLine(to: CGPoint(x: 35, y: 6))
        }
        .stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 1.7, lineCap: .round))
        .frame(width: 36, height: 16)
        .scaleEffect(0.6 + 0.3 * flick)
        .offset(y: -size.height / 2 - 9 - 3 * flick)
        .opacity(flick)
    }
}

/// The desktop round the demo window is background, so it takes no ink of its own: paper
/// showing faintly through the sky and a pale pen line, never pink, tan or espresso. The
/// window, the keys and the note keep every colour, so the eye goes to them.
private enum Backdrop {
    static let paper = DS.Surface.overlay.opacity(0.5)
    static let line = DS.Ink.i12.opacity(0.3)
    static let text = DS.Ink.i12.opacity(0.5)
    static let pen = StrokeStyle(lineWidth: 1, lineCap: .round, lineJoin: .round)
}

/// A file lying on the demo's desktop: a small pale drawing with its name under it.
private struct DesktopIcon: View {
    enum Kind { case folder, page, picture }
    let kind: Kind
    let name: String
    var seed = 80

    var body: some View {
        VStack(spacing: 4) {
            glyph.frame(width: 34, height: 30)
            Text(name).font(DS.Font.martian(8, weight: 450)).foregroundStyle(Backdrop.text).lineLimit(1)
        }
        .frame(width: 62)
    }

    @ViewBuilder private var glyph: some View {
        switch kind {
        case .folder:
            ZStack(alignment: .topLeading) {
                HandRect(seed: seed, wobble: 0.4, closed: true).fill(Backdrop.paper)
                    .frame(width: 34, height: 24).offset(y: 6)
                HandRect(seed: seed + 1, wobble: 0.4, closed: true).fill(Backdrop.paper)
                    .frame(width: 34, height: 18).offset(y: 12)
                HandRect(seed: seed + 3, wobble: 0.5, overshoot: 1.5).stroke(Backdrop.line, style: Backdrop.pen)
                    .frame(width: 34, height: 24).offset(y: 6)
                HandRect(seed: seed + 4, wobble: 0.5, overshoot: 1.5).stroke(Backdrop.line, style: Backdrop.pen)
                    .frame(width: 34, height: 18).offset(y: 12)
                HandRect(seed: seed + 5, wobble: 0.4, overshoot: 1).stroke(Backdrop.line, style: Backdrop.pen)
                    .frame(width: 13, height: 5).offset(x: 1, y: 2)
            }
            .frame(width: 34, height: 30, alignment: .topLeading)
        case .page, .picture:
            ZStack {
                DogEar().fill(Backdrop.paper)
                if kind == .picture {
                    Mountain().stroke(Backdrop.line, style: Backdrop.pen).padding(EdgeInsets(top: 14, leading: 4, bottom: 5, trailing: 4))
                    Circle().stroke(Backdrop.line, lineWidth: 1).frame(width: 5, height: 5).offset(x: -3, y: -5)
                } else {
                    Path { p in
                        for y in [11.0, 16, 21] { p.move(to: CGPoint(x: 5, y: y)); p.addLine(to: CGPoint(x: y == 21 ? 13 : 19, y: y)) }
                    }
                    .stroke(Backdrop.line, lineWidth: 1)
                }
                DogEar().stroke(Backdrop.line, style: Backdrop.pen)
            }
            .frame(width: 24, height: 30)
        }
    }
}

/// A sheet with its top right corner folded over.
private struct DogEar: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        let fold = min(r.width, r.height) * 0.3
        p.move(to: CGPoint(x: r.minX, y: r.minY)); p.addLine(to: CGPoint(x: r.maxX - fold, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX, y: r.minY + fold)); p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX, y: r.maxY)); p.closeSubpath()
        p.move(to: CGPoint(x: r.maxX - fold, y: r.minY)); p.addLine(to: CGPoint(x: r.maxX - fold, y: r.minY + fold))
        p.addLine(to: CGPoint(x: r.maxX, y: r.minY + fold))
        return p
    }
}

private struct Mountain: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.maxY)); p.addLine(to: CGPoint(x: r.minX + r.width * 0.38, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + r.width * 0.62, y: r.minY + r.height * 0.5))
        p.addLine(to: CGPoint(x: r.minX + r.width * 0.75, y: r.minY + r.height * 0.3))
        p.addLine(to: CGPoint(x: r.maxX, y: r.maxY)); p.closeSubpath()
        return p
    }
}

/// The demo desktop's Dock: a faint strip along the bottom with a few app shapes drawn on it
/// in the same pale line, a dot under Messages, the one that's open. Only the shapes say
/// what they are; none of them is coloured.
private struct DeskDock: View {
    var body: some View {
        // Tiles a little smaller than the desktop's files, the way a Dock sits under a window.
        HStack(alignment: .top, spacing: 9) {
            tile(seed: 101, running: true) {
                Ellipse().stroke(Backdrop.line, lineWidth: 1).frame(width: 17, height: 12)
            }
            tile(seed: 103) {
                Path { p in for y in [10.0, 15, 20] { p.move(to: CGPoint(x: 8, y: y)); p.addLine(to: CGPoint(x: y == 20 ? 16 : 22, y: y)) } }
                    .stroke(Backdrop.line, lineWidth: 1)
            }
            tile(seed: 105) {
                ZStack {
                    Path { p in p.move(to: CGPoint(x: 0, y: 0)); p.addLine(to: CGPoint(x: 7.5, y: 6)); p.addLine(to: CGPoint(x: 15, y: 0)) }
                        .stroke(Backdrop.line, lineWidth: 1).frame(width: 18, height: 12)
                    Rectangle().stroke(Backdrop.line, lineWidth: 1).frame(width: 18, height: 12)
                }
            }
            tile(seed: 107) {
                VStack(spacing: 0) {
                    Rectangle().fill(Backdrop.line).frame(height: 1).padding(.top, 8)
                    Text("7").font(DS.Font.martian(11, weight: 450)).foregroundStyle(Backdrop.text).frame(maxHeight: .infinity)
                }
            }
            tile(seed: 109) {
                ZStack {
                    Circle().stroke(Backdrop.line, lineWidth: 1).frame(width: 18, height: 18)
                    Ellipse().stroke(Backdrop.line, lineWidth: 0.9).frame(width: 8, height: 18)
                    Rectangle().fill(Backdrop.line).frame(width: 18, height: 0.9)
                }
            }
            tile(seed: 111) {
                Mountain().stroke(Backdrop.line, style: Backdrop.pen).frame(width: 19, height: 12).offset(y: 3)
            }
            HandLine(seed: 113, wobble: 0.4, vertical: true).stroke(Backdrop.line, lineWidth: 1)
                .frame(width: 2, height: 30)
            // The bin, empty: just its outline.
            ZStack {
                HandRect(seed: 114, wobble: 0.4, overshoot: 1).stroke(Backdrop.line, lineWidth: 1)
                    .frame(width: 17, height: 21).offset(y: 3)
                Path { p in p.move(to: CGPoint(x: 4, y: 5)); p.addLine(to: CGPoint(x: 26, y: 5)) }
                    .stroke(Backdrop.line, style: Backdrop.pen)
            }
            .frame(width: 30, height: 30)
        }
        .padding(.horizontal, 12).padding(.top, 7).padding(.bottom, 6)
        .background(HandRect(seed: 115, wobble: 0.4, closed: true).fill(DS.Surface.overlay.opacity(0.32)))
    }

    private func tile<Glyph: View>(seed: Int, running: Bool = false, @ViewBuilder glyph: () -> Glyph) -> some View {
        VStack(spacing: 3) {
            ZStack {
                HandRect(seed: seed, wobble: 0.4, closed: true).fill(Backdrop.paper)
                glyph()
                HandRect(seed: seed + 50, wobble: 0.5, overshoot: 1.5).stroke(Backdrop.line, style: Backdrop.pen)
            }
            .frame(width: 30, height: 30)
            Circle().fill(Backdrop.text).frame(width: 3, height: 3).opacity(running ? 1 : 0)
        }
    }
}

extension NoteDemo {
    /// Shortcuts to check the keys with: the default, someone's own, a wide key, the longest.
    static let sampleKeys: [[KeyCombo.Cap]] = [
        [("⌃", "control"), ("⌥", "option"), ("⇧", "shift"), (nil, "G")],
        [("⇧", "shift"), ("⌘", "command"), (nil, "E")],
        [("⌥", "option"), (nil, "Space")],
        [("⌃", "control"), ("⌥", "option"), ("⇧", "shift"), ("⌘", "command"), (nil, "F5")],
    ]
}
