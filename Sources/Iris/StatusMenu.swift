import AppKit
import Carbon.HIToolbox
import IrisCore
import IrisEngine
import SwiftUI

/// The menu bar menu as a slip of Iris paper instead of the system menu: the model's state
/// at the top, commands with their keys printed as keycaps, a hand-drawn highlight.
/// It opens without activating Iris, so "Check text" still acts on the app you were in.
///
/// Before changing it, read "The menu bar slip" in DESIGN.md: the header is the status, rows
/// are commands (they dim while busy, never turn into status notes), a task with its own
/// story turns the page, one flower in view, every state plays both ways.
@MainActor
final class StatusMenu: NSObject, NSWindowDelegate {
    /// The model's row, Load or Free: dimmed (`Model.busy`) while the model loads or frees.
    nonisolated static let modelRow = "model"

    struct Command: Identifiable, Equatable {
        let id: String
        let title: String
        var keys: [String] = []
        /// Runs inside the slip instead of closing it; the header shows what happens.
        var keepsOpen = false
        /// Printed before the title, like the Settings rail's icons.
        var icon: RisoIcon? = nil
        let run: () -> Void
        static func == (a: Command, b: Command) -> Bool { a.id == b.id && a.title == b.title && a.keys == b.keys }
    }

    final class Model: ObservableObject {
        @Published var engine: EngineState = .idle
        @Published var groups: [[Command]] = []
        @Published var highlighted: String?
        /// How far the tail sits from the slip's centre, when the slip is pushed in from a
        /// screen edge, so it still points at the flower.
        @Published var tailOffset: CGFloat = 0
        /// Rows whose action is under way: dimmed and not clickable. What's happening shows in
        /// the header (the model's state and the garden), never in the row.
        @Published var busy: Set<String> = []
        /// Setup isn't finished: the band says SETTING UP and this line says which step.
        @Published var setupLine: String?
        /// Something besides the model stops Iris (the shortcut is taken): the band says
        /// Something's off and this line says what and what to do.
        @Published var problemLine: String?
        /// The garden while setting up, when the model's state would run ahead of setup (a
        /// model on disk before permission): the closed bud.
        @Published var setupGarden: GardenView.Stage?
        /// What the slip shows: its commands, a page it has turned to, or an update building.
        @Published var showing: Showing = .commands

        /// A second page the slip has turned to (Updates), with a way back.
        var page: SlipPage? {
            if case .page(let page) = showing { return page }
            return nil
        }
        /// 0...1 while an update builds. The header says so and the garden grows in the rain.
        var updating: Double? {
            if case .updating(let progress) = showing { return progress }
            return nil
        }
    }

    /// One at a time: turning to a page leaves the commands, and an update building leaves
    /// any page.
    enum Showing {
        case commands
        case page(SlipPage)
        case updating(Double)
    }

    /// The slip turned to another page: no garden, a brush title and a line under it, art
    /// beside them if the page has some, then the page's own content.
    struct SlipPage {
        let id: UUID
        let title: String
        var lead: String? = nil
        var warning = false
        var art: AnyView? = nil
        var content: AnyView? = nil
        /// The answer to a look: when it came and what the page said while looking, so the
        /// old words rub out and the answer is written in as the letter opens.
        var answeredAt: Date? = nil
        var wordsWhileLooking: (title: String, lead: String?)? = nil
    }

    let model = Model()
    /// Rebuilds the commands each time the menu opens (shortcuts and model state change).
    var commands: () -> [[Command]] = { [] }
    private var panel: NSPanel?
    private var keyMonitor: Any?
    private var clickMonitor: Any?

    var isOpen: Bool { panel != nil }
    /// When it closed. A click on the menu bar icon closes the slip on the press (the slip
    /// loses focus) before the icon's action fires on release; the icon checks this so that
    /// same click doesn't open it again (see `ClosingPress`).
    private var closing = ClosingPress()

    /// Whether this click's release belongs to the press that just closed the slip. Asked
    /// once per click: the answer is used up, so the next click always counts.
    func takeClosedByThisClick() -> Bool {
        closing.isClosingClick(pressBegan: MousePresses.pressOfThisClick)
    }

    /// Space between the menu bar and the tip of the slip's tail.
    static let gap: CGFloat = 2
    /// Room around the slip for its shadow (none on top: the tail goes up to the bar).
    static let shadowRoom = (side: CGFloat(18), bottom: CGFloat(26))

    /// Where a paper of `size` (its shadow room included) hangs from the menu bar flower at
    /// `anchor`: centred under it with its tail pointing up at it, never covering it. Near a
    /// screen edge it's pushed in, and the tail slides to keep pointing at the flower. The slip
    /// and the note under the flower both hang this way.
    static func hang(_ size: CGSize, under anchor: CGRect, on screen: NSScreen?) -> (origin: CGPoint, tailOffset: CGFloat) {
        var origin = CGPoint(x: anchor.midX - size.width / 2, y: anchor.minY - gap - size.height)
        var tailOffset: CGFloat = 0
        if let visible = screen?.visibleFrame {
            let clamped = min(max(origin.x, visible.minX - shadowRoom.side + 6), visible.maxX - size.width + shadowRoom.side - 6)
            tailOffset = origin.x - clamped
            origin.x = clamped
        }
        return (origin, tailOffset)
    }

    func toggle(below button: NSStatusBarButton) {
        if isOpen { close() } else { show(below: button) }
    }

    /// Rebuilds the commands while open, when the model's state changes what applies.
    func refresh() {
        if isOpen { model.groups = commands() }
    }

    private func show(below button: NSStatusBarButton) {
        guard let window = button.window else { return }
        show(under: window.convertToScreen(button.convert(button.bounds, to: nil)), on: window.screen)
    }

    /// Opens under `anchor` (the flower's frame on screen).
    func show(under anchor: CGRect, on screen: NSScreen?) {
        model.groups = commands()
        model.highlighted = nil

        model.tailOffset = 0
        let host = NSHostingView(rootView: content(pick: { [weak self] in self?.pick($0) }))
        let size = host.fittingSize
        let panel = KeyPanel(contentRect: NSRect(origin: .zero, size: size),
                             styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .popUpMenu
        // Over full-screen apps too, on whichever Space is showing.
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        panel.contentView = host
        panel.delegate = self
        panel.isReleasedWhenClosed = false

        let hung = Self.hang(size, under: anchor, on: screen)
        model.tailOffset = hung.tailOffset
        panel.setFrameOrigin(hung.origin)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        // On the test display (`OnDisplay`) it never takes the keyboard or closes on someone's
        // click elsewhere: it's there to be looked at, while the person keeps working.
        if OnDisplay.isOn {
            panel.alphaValue = 1
            self.panel = panel
            return
        }
        panel.makeKey()
        NSAnimationContext.runAnimationGroup { $0.duration = DS.Motion.fast.enter; panel.animator().alphaValue = 1 }
        self.panel = panel

        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel?.isKeyWindow == true else { return event }
            return self.handle(event) ? nil : event
        }
        // A click anywhere else closes it, like a menu.
        clickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            MainActor.assumeIsolated { self?.close() }
        }
    }

    /// The slip with its shadow room, as its window holds it. On the test display its garden
    /// and band hold still at the moment the snapshots draw (`frozenAt`), so the two compare.
    private func content(pick: @escaping (Command) -> Void) -> some View {
        StatusMenuView(model: model, pick: pick, frozenAt: OnDisplay.isOn ? Self.frozenMoment : nil)
            .padding(.horizontal, Self.shadowRoom.side).padding(.bottom, Self.shadowRoom.bottom)
    }
    static let frozenMoment: Double = 1000

    /// The open slip, drawn again by the snapshot renderer from the same model, for
    /// `scripts/verify` to set beside the live window.
    func renderSnapshot(to path: String) {
        let view = content(pick: { _ in })
        ComponentSnapshot.renderPure(view, width: panel?.frame.width ?? NSHostingView(rootView: view).fittingSize.width, to: path)
    }

    private var flat: [Command] { model.groups.flatMap { $0 } }

    /// The open slip's window, for `scripts/verify` to capture.
    var windowNumber: Int? { panel?.windowNumber }

    private func pick(_ command: Command) {
        guard !model.busy.contains(command.id) else { return }
        if !command.keepsOpen { close() }
        command.run()
    }

    private func handle(_ event: NSEvent) -> Bool {
        let ids = flat.map(\.id)
        let current = model.highlighted.flatMap { ids.firstIndex(of: $0) }
        if event.modifierFlags.contains(.command), let chars = event.charactersIgnoringModifiers {
            if let match = flat.first(where: { $0.keys == ["⌘", chars.uppercased()] || $0.keys == ["⌘", chars] }) {
                pick(match)
                return true
            }
        }
        // A turned page has its own buttons; Escape turns back.
        if model.page != nil {
            if Int(event.keyCode) == kVK_Escape { withAnimation(DS.easeOut) { model.showing = .commands }; return true }
            return false
        }
        switch Int(event.keyCode) {
        case kVK_Escape: close(); return true
        case kVK_DownArrow, kVK_UpArrow:
            model.highlighted = ListKeys.step(from: current, count: ids.count, down: Int(event.keyCode) == kVK_DownArrow).map { ids[$0] }
            return true
        case kVK_Return, kVK_ANSI_KeypadEnter, kVK_Space:
            if let id = model.highlighted, let command = flat.first(where: { $0.id == id }) { pick(command) }
            return true
        default: return false
        }
    }

    func close() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        if let clickMonitor { NSEvent.removeMonitor(clickMonitor) }
        keyMonitor = nil
        clickMonitor = nil
        guard let panel else { return }
        self.panel = nil
        closing.closed(at: MousePresses.now)
        SelfCheck.note("slip closed at \(MousePresses.now), event \(NSApp.currentEvent.map { "\($0.type.rawValue) @\($0.timestamp)" } ?? "none")")
        NSAnimationContext.runAnimationGroup({ $0.duration = DS.Motion.fast.exit; panel.animator().alphaValue = 0 },
                                             completionHandler: { [weak self] in
            panel.orderOut(nil)
            // A page that was read doesn't wait for the next open: the slip opens on its
            // commands, like any menu. (An update that's building stays; it isn't a page.)
            MainActor.assumeIsolated {
                guard let self, !self.isOpen, self.model.page != nil else { return }
                self.model.showing = .commands
            }
        })
    }

    func windowDidResignKey(_ notification: Notification) { close() }
}

/// The slip itself. Also drawn on its own for design snapshots.
struct StatusMenuView: View {
    @ObservedObject var model: StatusMenu.Model
    let pick: (StatusMenu.Command) -> Void
    /// Snapshots only: freeze the garden on one frame.
    var frozenAt: Double? = nil
    @State private var shown = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// A hand-cut note with a tail pointing up at the menu bar flower.
    private var note: NoteShape { NoteShape(tailOffset: model.tailOffset) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let page = model.page {
                pageHeader
                PenRule(seed: 5, ink: DS.Ink.i12.opacity(0.85), width: 1.2).padding(.horizontal, 10)
                pageView(page)
            } else {
                header
                PenRule(seed: 5, ink: DS.Ink.i12.opacity(0.85), width: 1.2).padding(.horizontal, 10)
                ForEach(Array(model.groups.enumerated()), id: \.offset) { index, group in
                    if index > 0 { PenRule(seed: 40 + index).padding(.horizontal, 14).padding(.vertical, 3) }
                    VStack(spacing: 0) {
                        ForEach(group) { command in
                            row(command)
                        }
                    }
                    .padding(.horizontal, 6)
                }
            }
        }
        .padding(.top, 12 + NoteShape.tail).padding(.bottom, 8)
        .frame(width: 286)
        // Frosted paper: the desktop blurs softly behind it, the paper tint keeps text crisp.
        // The shadow is cut away under the note so it doesn't darken it from behind.
        .background {
            ZStack {
                ZStack {
                    note.fill(Color.black.opacity(0.2)).padding(.horizontal, 10).offset(y: 9).blur(radius: 9)
                    note.fill(Color.black.opacity(0.12)).offset(y: 1).blur(radius: 0.8)
                }
                .mask {
                    ZStack {
                        Rectangle().padding(-40)
                        note.blendMode(.destinationOut)
                    }
                    .compositingGroup()
                }
                // Live only: the snapshot renderer can't draw AppKit views.
                if frozenAt == nil { BehindWindowBlur().clipShape(note) }
                note.fill(DS.Surface.overlay.opacity(Self.paperOpacity))
                // The cut edge, drawn with the pen: soft, never a hard frame.
                note.stroke(DS.Ink.i12.opacity(0.18), lineWidth: 1)
            }
            .allowsHitTesting(false)
        }
        .overlay { Image(nsImage: Grain.tile).resizable(resizingMode: .tile).opacity(0.22).blendMode(.multiply)
            .clipShape(note).allowsHitTesting(false) }
        // Opens from the tail: a small grow out of the flower, then settles.
        .scaleEffect(shown || reduceMotion ? 1 : 0.94, anchor: UnitPoint(x: 0.5 + model.tailOffset / 286, y: 0))
        .opacity(shown ? 1 : 0)
        .onAppear { withAnimation(.spring(duration: 0.28, bounce: 0.18)) { shown = true } }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Iris")
    }

    /// The wordmark (no flower of its own: the garden is the slip's one flower), where the
    /// model is in Iris's words, and the setup garden in miniature showing the same state: a
    /// bud resting, a cloud while downloading, a sun while loading, in bloom with butterflies
    /// when ready. While an update builds it says so instead, and the flower grows in the rain.
    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            // A printed band like setup's plate: the status as a big ghost word in paper behind
            // the wordmark, and the garden standing on its bottom edge, spilling over onto the
            // slip. The status is said there once; nothing repeats it below.
            ZStack(alignment: .bottomLeading) {
                // The ghost is an overlay so a long word ("Downloading") never widens the slip.
                StatusBand(word: statusTitle, height: Self.bandHeight, size: 72, drop: 20, frozenAt: frozenAt)
                IrisLogo(size: 22).padding(.leading, 12).padding(.bottom, 12)
                    .accessibilityHidden(true)
                GardenView(stage: model.updating == nil ? gardenStage : .watering, progress: model.updating ?? downloadFraction,
                           growth: model.updating, frozenAt: frozenAt)
                    .scaleEffect(Self.gardenScale, anchor: .bottomTrailing)
                    .frame(width: GardenView.size.width * Self.gardenScale,
                           height: GardenView.size.height * Self.gardenScale, alignment: .bottomTrailing)
                    .frame(maxWidth: .infinity, alignment: .trailing)
                    .offset(y: 16)
            }
            .frame(height: Self.bandHeight, alignment: .bottom)
            .padding(.horizontal, -6)
            .padding(.bottom, 12)  // room for the garden spilling over the band's edge
            // The band says the status in pictures only (a ghost word, the garden): VoiceOver
            // gets it in words, joined with the line under it.
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(statusTitle)
            // Under the band only what the band can't say: how far along, or what went wrong.
            if let statusDetail {
                // Never cut short: the line is what to do next.
                Text(statusDetail).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let updating = model.updating {
                ProgressTrack(fraction: updating)
            } else if case .downloading(let fraction) = model.engine {
                ProgressTrack(fraction: fraction)
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 12)
        .accessibilityElement(children: .combine)
    }

    /// A turned page's top: the wordmark and the way back. No garden, so the only flower in
    /// sight is the one in the menu bar.
    private var pageHeader: some View {
        HStack(alignment: .center) {
            // Back where people look for it, top left: a pen arrow, the word and its key.
            BackButton { withAnimation(DS.easeOut) { model.showing = .commands } }
                .accessibilityHint("Returns to the menu. Or press Escape.")
                .padding(.leading, -6)
                .offset(y: 3)
            Spacer()
            IrisLogo(size: 18)
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 10)
    }

    private static let gardenScale: CGFloat = 0.56
    private static let bandHeight: CGFloat = 76
    static let paperOpacity: Double = 0.9

    private var gardenStage: GardenView.Stage { model.setupGarden ?? model.engine.gardenStage }

    private var downloadFraction: Double {
        if case .downloading(let fraction) = model.engine { return fraction }
        return 0
    }

    private var statusTitle: String {
        if model.updating != nil { return "Updating" }
        if model.setupLine != nil { return "Setting up" }
        if model.problemLine != nil { return EngineState.troubleTitle }
        return model.engine.title
    }

    /// Only a problem gets a line under the title: what went wrong. Resting, Ready, Downloading
    /// and Updating say enough with the title, the garden and the progress track.
    private var statusDetail: String? {
        if model.updating != nil { return nil }
        if let line = model.setupLine { return line }
        if let line = model.problemLine { return line }
        if case .failed = model.engine { return model.engine.detail }
        return nil
    }

    /// The slip turned to another page: the title in brush, then the page.
    private func pageView(_ page: StatusMenu.SlipPage) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .bottom, spacing: 8) {
                VStack(alignment: .leading, spacing: 6) {
                    let ink = page.warning ? DS.Hue.redText : DS.Ink.i12
                    if let at = page.answeredAt, let was = page.wordsWhileLooking {
                        AnswerWords(before: was.title, after: page.title, at: at, font: DS.Font.brush(26), ink: ink)
                            .accessibilityElement(children: .ignore).accessibilityLabel(page.title)
                        if let lead = page.lead {
                            AnswerWords(before: was.lead, after: lead, at: at, style: .rising, font: DS.Font.body,
                                        ink: DS.Ink.i11, beforeInk: DS.Ink.i11, comes: UpdateCheckScene.Beat.lead)
                                .accessibilityElement(children: .ignore).accessibilityLabel(lead)
                        }
                    } else {
                        Text(page.title).font(DS.Font.brush(26)).foregroundStyle(ink)
                        if let lead = page.lead {
                            Text(lead).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if let art = page.art { art }
            }
            if let content = page.content { content }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16).padding(.top, 14).padding(.bottom, 10)
        .accessibilityElement(children: .contain)
    }

    private func row(_ command: StatusMenu.Command) -> some View {
        let busy = model.busy.contains(command.id)
        let lit = model.highlighted == command.id && !busy
        // Rows without an icon (Quit) keep its room, so every title starts on one line.
        let iconRoom = model.groups.joined().contains { $0.icon != nil }
        return Button { pick(command) } label: {
            HStack(spacing: 8) {
                if let icon = command.icon {
                    RisoIconView(icon: icon, size: 22).accessibilityHidden(true)
                } else if iconRoom {
                    Color.clear.frame(width: 22, height: 22)
                }
                Text(command.title).font(DS.Font.body).foregroundStyle(lit ? DS.Ink.i12 : DS.Ink.i11)
                Spacer(minLength: 8)
                if !command.keys.isEmpty {
                    HStack(spacing: 3) {
                        ForEach(Array(command.keys.enumerated()), id: \.offset) { _, key in Keycap(key: key) }
                    }
                    .accessibilityHidden(true)
                }
            }
            .padding(.horizontal, 10)
            .frame(height: DS.Size.control)
            .background {
                // The row under the pointer gets a grey ink wash, like the Settings rail and
                // dropdowns. Pink is for what's chosen, never for hover.
                HandRect(seed: command.id.count * 7, wobble: 0.6, closed: true)
                    .fill(DS.Ink.i3).opacity(lit ? 1 : 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.99))
        // Under way: dimmed while the header and the garden show what's happening.
        .disabled(busy)
        .opacity(busy ? 0.4 : 1)
        .onHover { if $0 { model.highlighted = command.id } }
        .animation(DS.easeOut, value: lit)
        .animation(DS.easeOut, value: busy)
        .accessibilityLabel(command.title)
    }
}

/// The menu bar note: a hand-cut slip with a tail at the top pointing at the flower. Edges
/// wobble a little, like the keycaps; seeded so it never re-jitters.
struct NoteShape: Shape {
    static let tail: CGFloat = 11
    var tailOffset: CGFloat = 0
    var seed = 17

    func path(in rect: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        let p1 = rng.next() * 6.3, p2 = rng.next() * 6.3
        let body = CGRect(x: rect.minX, y: rect.minY + Self.tail, width: rect.width, height: rect.height - Self.tail)
        let cx = min(max(body.midX + tailOffset, body.minX + 22), body.maxX - 22)
        func wob(_ d: CGFloat) -> CGFloat { 0.9 * (sin(d * 0.045 + p1) * 0.6 + sin(d * 0.13 + p2) * 0.4) }
        var pts: [CGPoint] = []
        var d: CGFloat = 0
        // Top edge, left to right, with the tail rising in the middle.
        var x = body.minX + 3
        while x < body.maxX - 3 {
            if x > cx - 12, x < cx + 12 {
                pts.append(CGPoint(x: cx - 11, y: body.minY))
                pts.append(CGPoint(x: cx - 2, y: rect.minY + 1.5))
                pts.append(CGPoint(x: cx + 1.5, y: rect.minY + 0.5))
                pts.append(CGPoint(x: cx + 11, y: body.minY))
                x = cx + 14
                continue
            }
            pts.append(CGPoint(x: x, y: body.minY + wob(d))); x += 7; d += 7
        }
        var y = body.minY + 3
        while y < body.maxY - 3 { pts.append(CGPoint(x: body.maxX + wob(d), y: y)); y += 7; d += 7 }
        x = body.maxX - 3
        while x > body.minX + 3 { pts.append(CGPoint(x: x, y: body.maxY + wob(d))); x -= 7; d += 7 }
        y = body.maxY - 3
        while y > body.minY + 3 { pts.append(CGPoint(x: body.minX + wob(d), y: y)); y -= 7; d += 7 }
        return Path.catmullRom(pts, closed: true)
    }
}

/// The desktop behind the window, blurred (the system's frosted glass), for the paper to sit on.
private struct BehindWindowBlur: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .popover
        view.blendingMode = .behindWindow
        view.state = .active
        return view
    }
    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}
}

/// What's in an update, each change ticked off in pink, what happens next, and the button.
/// "Back" at the top is the way to say later.
struct UpdateChanges: View {
    let changes: [String]
    let update: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(changes.prefix(5).enumerated()), id: \.offset) { _, line in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
                            .frame(width: 12, height: 9)
                        Text(line).font(DS.Font.body).foregroundStyle(DS.Ink.i11).lineLimit(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            PenRule(seed: 61).padding(.vertical, 2)
            Text("Iris installs it, then quits and reopens. About a minute.").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                .fixedSize(horizontal: false, vertical: true)
            TactileButton(title: "Update & reopen", role: .primary, compact: true, action: update)
                .padding(.top, 2)
        }
    }
}

/// A page that went wrong: what happened, and a way to try again when that can help.
struct UpdateTrouble: View {
    let message: String
    var retry: (() -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(message).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
            if let retry { TactileButton(title: "Try again", role: .secondary, compact: true, action: retry) }
        }
    }
}
