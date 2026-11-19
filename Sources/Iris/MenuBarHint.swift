import AppKit
import SwiftUI

/// Where Iris lives, said once when setup finishes: a small hand-cut note hanging from the
/// menu bar flower by its tail, like the menu bar slip, so it points at the icon it names.
/// It fades on its own, or on any click. Iris has no Dock icon, so this is how people find it.
@MainActor
final class MenuBarHint {
    static let shared = MenuBarHint()
    private var panel: NSPanel?
    private var clickMonitor: Any?
    private var hideTask: Task<Void, Never>?

    /// Hangs the note under `anchor` (the flower's frame on screen) for eight seconds.
    /// `lead` and `detail` replace "find me up here" and "In any text box, press" for other
    /// news from the menu bar ("model's in").
    func show(under anchor: CGRect, on screen: NSScreen?, keys: [String], lead: String? = nil, detail: String? = nil) {
        close()
        let room = StatusMenu.shadowRoom
        let measure = NSHostingView(rootView: MenuBarHintView(keys: keys, lead: lead, detail: detail, tailOffset: 0)
            .padding(.horizontal, room.side).padding(.bottom, room.bottom))
        let hung = StatusMenu.hang(measure.fittingSize, under: anchor, on: screen)
        let view = MenuBarHintView(keys: keys, lead: lead, detail: detail, tailOffset: hung.tailOffset)
        let panel = FloatingNote.panel(view.padding(.horizontal, room.side).padding(.bottom, room.bottom), level: .popUpMenu)
        panel.setFrameOrigin(hung.origin)
        FloatingNote.fadeIn(panel)
        self.panel = panel
        FloatingNote.announce(view.spoken, priority: .high)
        clickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            MainActor.assumeIsolated { self?.close() }
        }
        hideTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(8))
            guard !Task.isCancelled else { return }
            self?.close()
        }
    }

    func close() {
        hideTask?.cancel()
        hideTask = nil
        if let clickMonitor { NSEvent.removeMonitor(clickMonitor) }
        clickMonitor = nil
        guard let panel else { return }
        self.panel = nil
        FloatingNote.fadeOut(panel)
    }
}

/// The note itself: the slip's paper and tail, Iris's word in brush over the pink marker, and
/// the keys that open it.
struct MenuBarHintView: View {
    let keys: [String]
    var lead: String? = nil
    var detail: String? = nil
    var tailOffset: CGFloat = 0

    private var note: NoteShape { NoteShape(tailOffset: tailOffset, seed: 23) }
    private var title: String { lead ?? "find me up here" }
    private var line: String { detail ?? "In any text box, press" }

    /// What VoiceOver says: the words on the note, keys included.
    var spoken: String { "\(title). \(line)" + (keys.isEmpty ? "" : " \(keys.joined(separator: " "))") }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            // The tail points at the flower; the words say what it is.
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(title).font(DS.Font.brush(24)).foregroundStyle(DS.Ink.i12)
                    .fixedSize()
                    .background(alignment: .bottom) {
                        MarkerSwipe(height: 7, seed: 4)
                            .rotationEffect(.degrees(-1.2)).padding(.horizontal, -3).offset(y: -3)
                    }
            }
            HStack(spacing: 6) {
                Text(line).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize()
                ForEach(Array(keys.enumerated()), id: \.offset) { _, key in Keycap(key: key) }
            }
        }
        .padding(.horizontal, 18)
        .padding(.top, 14 + NoteShape.tail)
        .padding(.bottom, 14)
        .background {
            ZStack {
                note.fill(Color.black.opacity(0.2)).padding(.horizontal, 10).offset(y: 9).blur(radius: 9)
                note.fill(DS.Surface.overlay)
                note.stroke(DS.Ink.i12.opacity(0.18), lineWidth: 1)
            }
        }
        .overlay { Image(nsImage: Grain.tile).resizable(resizingMode: .tile).opacity(0.22).blendMode(.multiply).clipShape(note) }
        .accessibilityElement(children: .combine)
    }
}
