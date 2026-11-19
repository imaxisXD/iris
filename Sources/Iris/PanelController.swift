import AppKit
import Carbon.HIToolbox
import Combine
import IrisCore
import SwiftUI

/// A borderless panel that takes keyboard input without activating our app,
/// so the app the user was typing in stays frontmost.
final class FloatingPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class PanelController: NSObject, NSWindowDelegate {
    private let panel: FloatingPanel
    private let model = PanelModel()
    private let hosting: NSHostingView<PanelView>
    private var keyMonitor: Any?
    private var captured: TextBridge.Captured?
    /// What the panel sits by: the text box, or a point by the pointer.
    private var anchor = CGRect.zero
    /// The side of the text box people chose in Settings.
    private var prefer = PanelLayout.Side.above
    /// Where the paper and the flower went; its side holds while the panel grows.
    private var layout: PanelLayout?
    private var cancellables: Set<AnyCancellable> = []
    /// Where the pointer was when the shortcut was pressed. Without a text box to stand on,
    /// the bud and panel stay there: reading the live pointer on every redraw made the panel
    /// chase it (Tab to the next tone, and it jumped to wherever the pointer had moved).
    private var pointer = CGPoint.zero
    /// The flower that stands on the text box's corner; the panel unfolds out of it.
    private let bud = BudWindow()
    private let presence = PanelPresence()
    private var unfoldTask: Task<Void, Never>?
    /// Where the panel is in its life (its rules and tests are in IrisCore's `PanelLife`).
    private(set) var life = PanelLife() {
        didSet { presence.unfolded = life.stage == .unfolded }
    }

    override init() {
        hosting = NSHostingView(rootView: PanelView(model: model, presence: presence))
        panel = FloatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: PanelView.width, height: 160),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered, defer: true)
        super.init()

        panel.isFloatingPanel = true
        panel.level = .floating
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false  // the panel draws its own crisp drop
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        panel.animationBehavior = .none
        panel.contentView = hosting
        panel.delegate = self

        model.onReplace = { [weak self] text in self?.replace(with: text) }
        model.onClose = { [weak self] in self?.close() }
        model.onCopyOut = { [weak self] text in
            let frame = self?.captured?.fieldFrame
            TextBridge.copyToClipboard(text)
            self?.close()
            Toast.shared.show(.copied, near: frame)
        }
        model.onClean = { [weak self] in
            let frame = self?.captured?.fieldFrame
            self?.close()
            Toast.shared.show(.looksGood, near: frame)
        }

        // Resize to fit content whenever it changes (streaming text grows the panel).
        model.objectWillChange
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in self?.fitToContent() }
            .store(in: &cancellables)
        model.$phase
            .receive(on: RunLoop.main)
            .sink { [weak self] phase in self?.phaseChanged(phase) }
            .store(in: &cancellables)
        // For `scripts/verify`: says when the panel has come to rest after any change, so a capture
        // never catches it mid-spring.
        if OnDisplay.isOn {
            model.objectWillChange.receive(on: RunLoop.main).sink { [weak self] _ in self?.sayWhenSettled() }.store(in: &cancellables)
        }
    }

    /// Showing for this text box: the bud is up or the panel is open. Not while it folds away,
    /// so the shortcut pressed then opens a new one instead of being taken as a second close.
    var isShowing: Bool { life.isShowing }

    /// Takes a folding panel off screen now, so the keyboard goes back to the app underneath.
    /// Returns whether one was folding.
    func finishFolding() -> Bool {
        guard life.finishFold() else { return false }
        panel.orderOut(nil)
        return true
    }

    /// `answer`: one already in hand for this text (see `PanelModel.start`).
    func show(for captured: TextBridge.Captured, startingMode: String, answer: RewriteResult? = nil) {
        self.captured = captured
        pointer = NSEvent.mouseLocation
        switch (AppSettings.shared.placement, captured.fieldFrame) {
        case (.aboveField, let field?): stand(by: field, prefer: .above)
        case (.belowField, let field?): stand(by: field, prefer: .below)
        default:
            // By the pointer (or no text box to go by): the flower grows where the pointer is.
            stand(by: CGRect(x: pointer.x + BudView.reach.right + PanelLayout.flowerGap, y: pointer.y, width: 0, height: 0),
                  prefer: .above)
        }
        // On the test display: by the box it read, never at the pointer (on the person's screen).
        if OnDisplay.isOn, captured.fieldFrame == nil { return OnDisplay.trip("the panel would open at the pointer: no text box frame") }
        model.start(text: captured.text, modeID: startingMode, answer: answer, appName: captured.app.nameForPeople,
                    appBundleID: captured.app.bundleIdentifier, canReplace: captured.canReplace)
        // On the test display it never takes the keyboard: that's the person's (`press` instead).
        appear(takingKeys: !OnDisplay.isOn)
        installKeyMonitor()
        // A slow answer still unfolds, and streams in the panel as before.
        unfoldTask?.cancel()
        unfoldTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(PanelMotion.unfoldAnyway))
            guard !Task.isCancelled else { return }
            self?.unfold()
        }
        phaseChanged(model.phase)
    }

    /// Folded until there's something to read; the bud by `anchor` says Iris heard you. The
    /// model is started between this and `appear`, so the panel fits what it will show.
    private func stand(by anchor: CGRect, prefer: PanelLayout.Side) {
        self.anchor = anchor
        self.prefer = prefer
        layout = nil
        life.show()
    }

    /// Places the folded panel by its anchor and stands the bud there.
    private func appear(takingKeys: Bool) {
        fitToContent(choosingSide: true)
        let foot = layout?.foot ?? pointer
        presence.anchor = unitPoint(of: layout?.hinge ?? pointer)
        // Invisible until it unfolds, so the folded panel never flashes as a box before
        // SwiftUI has drawn it folded. It still takes the keys straight away.
        panel.alphaValue = 0
        if takingKeys { panel.makeKeyAndOrderFront(nil) } else { panel.orderFrontRegardless() }
        bud.show(standingAt: foot)
        budShownAt = Date()
    }

    /// The bud shows the model's state; a result, a failure or a model that's still waking
    /// up unfolds the panel out of it.
    private func phaseChanged(_ phase: PanelModel.Phase) {
        guard isShowing else { return }
        switch phase {
        case .done:
            bud.set(.open)
            unfold()
        case .failed:
            bud.set(.working)  // a failed check leaves the bud shut
            unfold()
        case .preparing:
            bud.set(.working)
            unfold()  // the panel explains the wait (downloading, waking up)
        case .generating:
            bud.set(.working)
        }
    }

    /// Opens the panel out of the bud when `PanelMotion` says: never before the bud has had
    /// its moment, and a beat after the flower blooms, so you see bud, bloom, panel.
    private func unfold() {
        unfoldTask?.cancel()
        guard life.stage == .budding else { return }
        let shown = life.generation
        let wait = PanelMotion.unrollDelay(waited: Date().timeIntervalSince(budShownAt), bloomed: bud.isOpen, full: bud.arrivedFull)
        guard wait > 0.01 else { return open() }
        unfoldTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(wait))
            guard let self, !Task.isCancelled, self.life.generation == shown else { return }
            self.open()
        }
    }

    private func open() {
        guard life.unfold() else { return }
        panel.alphaValue = 1
        bud.raise()
        if OnDisplay.isOn { sayWhenSettled() }
    }

    // MARK: Driven on the test display (`Driven`)

    private var settleTask: Task<Void, Never>?

    /// "panel settled <tone> <phase>" once nothing has changed for the longest of the panel's
    /// animations (the unroll's spring, the tab marker's slide) with room to spare.
    private func sayWhenSettled() {
        settleTask?.cancel()
        settleTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(0.8))
            guard let self, !Task.isCancelled, self.isShowing, self.panel.alphaValue > 0 else { return }
            let phase = switch self.model.phase {
            case .preparing: "preparing"
            case .generating: "writing"
            case .done: "done"
            case .failed: "failed"
            }
            OnDisplay.say("panel settled \(self.model.currentMode.id) \(phase) window \(self.panel.windowNumber)")
        }
    }

    /// A key pressed in the open panel, handed to its own key handler as the panel's key monitor
    /// would hand it a real press. Names: tab, shift-tab, escape, return, cmd-r, cmd-c, left, right, 1…9.
    func press(_ name: String) -> Bool {
        guard isShowing else { return false }
        let keys: [String: (Int, NSEvent.ModifierFlags, String)] = [
            "tab": (kVK_Tab, [], "\t"), "shift-tab": (kVK_Tab, .shift, "\t"), "escape": (kVK_Escape, [], "\u{1b}"),
            "return": (kVK_Return, [], "\r"), "cmd-r": (kVK_ANSI_R, .command, "r"), "cmd-c": (kVK_ANSI_C, .command, "c"),
            "left": (kVK_LeftArrow, [], ""), "right": (kVK_RightArrow, [], ""),
        ]
        let digits: [Int] = [kVK_ANSI_1, kVK_ANSI_2, kVK_ANSI_3, kVK_ANSI_4, kVK_ANSI_5, kVK_ANSI_6, kVK_ANSI_7, kVK_ANSI_8, kVK_ANSI_9]
        let key = keys[name] ?? Int(name).flatMap { (1...9).contains($0) ? (digits[$0 - 1], [], name) : nil }
        guard let (code, flags, chars) = key,
              let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                           windowNumber: panel.windowNumber, context: nil, characters: chars,
                                           charactersIgnoringModifiers: chars, isARepeat: false, keyCode: UInt16(code))
        else { return false }
        return handle(event)
    }

    /// The open panel drawn again by the snapshot renderer (ImageRenderer, as `--snapshot`
    /// draws it), at the live window's size, for `scripts/verify` to set beside its capture.
    func renderSnapshot(to path: String) {
        ComponentSnapshot.renderPure(PanelView(model: model), width: panel.frame.width, to: path)
    }

    private var budShownAt = Date.distantPast

    /// A screen point in the panel window's unit space (SwiftUI's, top-left origin).
    private func unitPoint(of point: CGPoint) -> UnitPoint {
        let frame = panel.frame
        guard frame.width > 0, frame.height > 0 else { return .bottomLeading }
        return UnitPoint(x: min(max((point.x - frame.minX) / frame.width, 0), 1),
                         y: min(max(1 - (point.y - frame.minY) / frame.height, 0), 1))
    }

    /// Design check (`Iris --preview-panel`): plays the bud and the unfold over a made-up
    /// text box with a made-up answer, again and again. Nothing is read, checked or pasted.
    func previewLoop(field: CGRect) {
        let original = OnboardingModel.practiceSample
        let fixed = OnboardingModel.practiceFixed
        let result = RewriteResult(mode: .fix, original: original, text: fixed, segments: WordDiff.diff(original, fixed),
                                   verdict: .minor, latencyMs: 900)
        // A stand-in chat app around a stand-in text box, so there's something for the bud to
        // stand on and nothing busy behind it in a recording.
        let room = CGSize(width: 140, height: 360)
        let previewText = PreviewText(original)
        let box = NSWindow(contentRect: CGRect(x: field.minX - room.width, y: field.minY - 40,
                                               width: field.width + room.width * 2, height: field.height + 40 + room.height),
                           styleMask: [.borderless], backing: .buffered, defer: false)
        box.backgroundColor = .clear
        box.isOpaque = false
        box.level = .normal
        box.contentView = NSHostingView(rootView:
            VStack(alignment: .leading, spacing: 14) {
                Spacer()
                HStack(alignment: .top, spacing: 10) {
                    RoundedRectangle(cornerRadius: 6).fill(Color(red: 0.79, green: 0.72, blue: 0.85)).frame(width: 30, height: 30)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("lead").font(.system(size: 13, weight: .bold))
                        Text("Did you get to the PR?").font(.system(size: 13))
                    }
                }
                .foregroundStyle(.black)
                .padding(.leading, room.width)
                PreviewField(text: previewText).font(.system(size: 13)).foregroundStyle(.black)
                    .frame(width: field.width - 24, height: field.height - 20, alignment: .topLeading)
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.white))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.5)))
                    .padding(.leading, room.width)
                    .padding(.bottom, 40)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(Color(red: 0.965, green: 0.962, blue: 0.97)))
        box.orderFrontRegardless()
        Task { @MainActor in
            var loop = 0
            while true {
                // The first loop plays the full arrival, the next the quick one, and so on.
                BudShow.forced = loop.isMultiple(of: 2)
                loop += 1
                previewText.text = original
                stand(by: field, prefer: .above)
                model.preview(original: original, result: nil, engine: .ready(AppSettings.shared.model), streaming: "")
                appear(takingKeys: false)
                try? await Task.sleep(for: .milliseconds(900))
                model.preview(original: original, result: result, engine: .ready(AppSettings.shared.model))
                try? await Task.sleep(for: .seconds(2.6))
                // As Replace would: the panel goes at once and the fix lands in the box.
                close(immediately: true)
                previewText.text = fixed
                try? await Task.sleep(for: .seconds(1.6))
            }
        }
    }

    /// Folds the panel back down into the flower, which folds and sinks. When Iris is about to
    /// paste (`immediately`), the panel leaves at once so the keystrokes reach the app, not the
    /// panel, and the flower stamps a tick by the text box as it goes.
    func close(immediately: Bool = false) {
        model.cancel()
        removeKeyMonitor()
        unfoldTask?.cancel()
        bud.wilt(replaced: immediately)
        captured = nil
        switch life.close(immediately: immediately, reduceMotion: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion) {
        case .nothing:
            return
        case .hideNow:
            panel.orderOut(nil)
        case .fold(let shown):
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(PanelMotion.foldDuration))
                guard let self, self.life.foldFinished(shown) else { return }
                self.panel.orderOut(nil)
            }
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        // Clicking anywhere else dismisses, like a popover.
        if isShowing { close() }
    }

    // MARK: Replace

    private func replace(with text: String) {
        guard let captured else { return close() }
        close(immediately: true)
        let original = model.original
        Task { await Paste.replace(captured, original: original, with: text, saying: .replaced()) }
    }

    // MARK: Keys

    private func installKeyMonitor() {
        removeKeyMonitor()
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel.isKeyWindow else { return event }
            return self.handle(event) ? nil : event
        }
    }

    private func removeKeyMonitor() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
    }

    private func handle(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let command = flags.contains(.command)
        switch Int(event.keyCode) {
        case kVK_Escape:
            close(); return true
        case kVK_Return, kVK_ANSI_KeypadEnter:
            model.accept(); return true
        case kVK_Tab:
            flags.contains(.shift) ? model.previous() : model.next(); return true
        case kVK_LeftArrow where !command:
            model.previous(); return true
        case kVK_RightArrow where !command:
            model.next(); return true
        case kVK_ANSI_C where command:
            // Let a text selection inside the panel copy normally; otherwise copy the result.
            if let textView = panel.firstResponder as? NSTextView, textView.selectedRange().length > 0 {
                return false
            }
            model.copy(); return true
        case kVK_ANSI_R where command:
            model.regenerate(); return true
        default:
            if !command, let chars = event.charactersIgnoringModifiers, let digit = Int(chars), (1...ToneList.panelRoom).contains(digit) {
                model.select(digit - 1); return true
            }
            return false
        }
    }

    // MARK: Layout

    /// `choosingSide`: a new panel picks above or below the text box once, as it's shown; later
    /// fits (streaming text growing the panel) keep that side. The rules are `PanelLayout`'s.
    private func fitToContent(choosingSide: Bool = false) {
        let size = hosting.fittingSize
        guard size.height > 0 else { return }
        // The window includes transparent room for the paper's shadow; place the paper, not the window.
        let m = PanelView.margin
        let paper = CGSize(width: size.width - m.horizontal * 2, height: size.height - m.top - m.bottom)
        let placed = PanelLayout.place(paper: paper, by: anchor, prefer: prefer,
                                       keep: choosingSide ? nil : layout?.side,
                                       in: screenForAnchor().visibleFrame, flower: BudView.reach)
        layout = placed
        let origin = CGPoint(x: placed.paper.minX - m.horizontal, y: placed.paper.minY - m.bottom)
        panel.setFrame(NSRect(origin: origin, size: size), display: true)
    }

    private func screenForAnchor() -> NSScreen {
        NSScreen.containing(CGPoint(x: anchor.midX, y: anchor.midY))
    }
}

/// The preview's stand-in text box contents, so the fix can land in it on Replace.
@MainActor
private final class PreviewText: ObservableObject {
    @Published var text: String
    init(_ text: String) { self.text = text }
}

private struct PreviewField: View {
    @ObservedObject var text: PreviewText
    var body: some View { Text(text.text) }
}
