import AppKit
import SwiftUI

/// One short note at a time, for a result you can't otherwise see. Written like Iris
/// marking up a page: a pen mark for the kind of note, Iris's word in brush ("fixed 3 words"),
/// a plain detail, keys as keycaps. On a paper slip, so it reads over any app. Never takes
/// focus; never for a copy.
@MainActor
final class Toast {
    static let shared = Toast()
    private var panel: NSPanel?
    private var hideTask: Task<Void, Never>?
    /// While a problem is up: the next click or key press anywhere puts it away.
    private var dismissMonitors: [Any] = []

    enum Kind { case success, neutral, warning }

    /// Shows `message` by the text box at `anchor` (or by the pointer), for as long as the
    /// message says it stays.
    func show(_ message: Message, near anchor: CGRect? = nil) {
        // On the test display: said to the run, and never by the pointer (on the person's screen).
        var anchor = anchor
        if OnDisplay.isOn {
            OnDisplay.say("note \(message.spoken)")
            if anchor == nil, let screen = OnDisplay.screen {
                anchor = CGRect(x: screen.frame.midX - 100, y: screen.frame.midY, width: 200, height: 30)
            }
        }
        // A note that's turned off still clears the one before it, so "checking…" never
        // lingers after the fix it was about.
        if let note = message.note, !AppSettings.shared.shows(note) { return dismiss() }
        hideTask?.cancel()
        removeDismissMonitors()
        panel?.orderOut(nil)

        // One note by your text at a time: a pending "keep this word?" gives way to news.
        KeepWordNote.shared.close()
        let panel = FloatingNote.panel(ToastView(message: message).padding(14), level: .statusBar)
        panel.setAccessibilityLabel(message.spoken)
        panel.setFrameOrigin(FloatingNote.origin(for: panel.frame.size, by: anchor))
        FloatingNote.fadeIn(panel)
        self.panel = panel
        if !OnDisplay.isOn { FloatingNote.announce(message.spoken) }

        // On the test display the person's clicks and keys are theirs, not a dismissal.
        if message.kind == .warning, !OnDisplay.isOn {
            // Watch, never swallow: the click or key still goes where it was meant to.
            let put: (NSEvent) -> Void = { [weak self] _ in MainActor.assumeIsolated { self?.dismiss() } }
            if let global = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .keyDown], handler: put) {
                dismissMonitors.append(global)
            }
            if let local = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .keyDown], handler: { put($0); return $0 }) {
                dismissMonitors.append(local)
            }
        }
        hideTask = Task { [weak self] in
            try? await Task.sleep(for: message.stays)
            guard !Task.isCancelled else { return }
            FloatingNote.fadeOut(panel)
            if self?.panel === panel { self?.panel = nil }
        }
    }

    /// Fades out whatever note is showing.
    func dismiss() {
        hideTask?.cancel()
        removeDismissMonitors()
        guard let panel else { return }
        self.panel = nil
        FloatingNote.fadeOut(panel)
    }

    private func removeDismissMonitors() {
        dismissMonitors.forEach(NSEvent.removeMonitor)
        dismissMonitors = []
    }
}

extension Message {
    /// What VoiceOver reads: the whole note as one sentence or two.
    var spoken: String {
        [lead, [detail, label].compactMap { $0 }.joined(separator: " "), extra ?? ""].filter { !$0.isEmpty }.joined(separator: ". ")
    }
}

struct ToastView: View {
    let message: Message

    /// Past this the detail wraps instead of running off a small screen.
    static let detailWidth: CGFloat = 460

    private var lead: String { message.lead }
    private var detail: String? { message.detail }
    private var keys: [String] { message.keys }
    private var label: String? { message.label }
    private var crossed: NSImage? { message.crossed?.flattened() }
    private var extra: String? { message.extra }
    private var kind: Toast.Kind { message.kind }

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            if let crossed {
                // The app Iris is off in, crossed out with the brush (a choice, so ink, not red).
                Image(nsImage: crossed).resizable().frame(width: 26, height: 26)
                    .overlay(BrushCross(seed: 5).frame(width: 34, height: 32))
                    .frame(width: 30, height: 30)
            } else {
                ToastMark(kind: kind)
            }
            Text(lead).font(DS.Font.brush(24))
                .foregroundStyle(kind == .warning ? DS.Hue.redText : DS.Ink.i12)
                .fixedSize()
                .background(alignment: .bottom) {
                    // The pink marker under good news and quiet notes; a problem stays in red pen.
                    if kind != .warning {
                        MarkerSwipe(height: 6, seed: 6)
                            .rotationEffect(.degrees(-1.2)).padding(.horizontal, -3).offset(y: -2)
                    }
                }
            if detail != nil || !keys.isEmpty || label != nil || extra != nil {
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 6) {
                        if let detail {
                            Text(detail).font(DS.Font.body).foregroundStyle(DS.Ink.i11)
                                .fixedSize(horizontal: false, vertical: true)
                                .frame(maxWidth: Self.detailWidth, alignment: .leading)
                                .fixedSize(horizontal: true, vertical: false)
                        }
                        if let label { InkLabel(text: label, size: 11.5).fixedSize() }
                        ForEach(Array(keys.enumerated()), id: \.offset) { _, key in Keycap(key: key) }
                    }
                    if let extra { Text(extra).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize() }
                }
            }
        }
        .notePaper()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(message.spoken)
    }
}

/// Which kind of note, in pen: a pink tick for done, the iris's bloom for working or a quiet
/// note, a red circle round "?" for a problem.
struct ToastMark: View {
    let kind: Toast.Kind
    @MainActor private static let bud = RisoArt.flower(points: 30, crop: .bloom)

    var body: some View {
        switch kind {
        case .success:
            HandTick().stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 2.4, lineCap: .round, lineJoin: .round))
                .frame(width: 18, height: 14)
        case .neutral:
            Image(nsImage: Self.bud).resizable().frame(width: 30, height: 30)
                .padding(.vertical, -3)
        case .warning:
            Text("?").font(DS.Font.brush(20)).foregroundStyle(DS.Hue.redText)
                .frame(width: 24, height: 24)
                .overlay(BrushRing(rgb: Riso.red, seed: 9))
        }
    }
}
