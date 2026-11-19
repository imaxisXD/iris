import AppKit
import SwiftUI

/// The small notes Iris floats over every app without taking focus: messages by your text
/// (`Toast`), the keep-a-word question (`KeepWordNote`) and the note under the menu bar flower
/// (`MenuBarHint`). One place makes, places, fades and announces them, so they behave alike.
@MainActor
enum FloatingNote {
    /// A borderless panel that never takes focus, on every Space, invisible until `fadeIn`.
    /// `clickable`: takes clicks (it has buttons) without becoming key.
    static func panel<V: View>(_ view: V, level: NSWindow.Level, clickable: Bool = false) -> NSPanel {
        let host = NSHostingView(rootView: view)
        let panel = NSPanel(contentRect: NSRect(origin: .zero, size: host.fittingSize),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isFloatingPanel = true
        panel.level = level
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false  // each note draws its own paper shadow
        panel.ignoresMouseEvents = !clickable
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        panel.contentView = host
        panel.alphaValue = 0
        return panel
    }

    /// Where a note goes by the text box `box`: right-aligned on its top edge, since buttons near
    /// a text box (Send, formatting) usually sit at its left or bottom and the note mustn't cover
    /// them. With no box, just above the pointer. Always on screen.
    static func origin(for size: CGSize, by box: CGRect?) -> CGPoint {
        let point = box.map { CGPoint(x: $0.midX, y: $0.midY) } ?? NSEvent.mouseLocation
        let visible = NSScreen.containing(point).visibleFrame
        var origin = box.map { CGPoint(x: $0.maxX - size.width + 14, y: $0.maxY - 4) }
            ?? CGPoint(x: point.x - size.width / 2, y: point.y + 8)
        origin.x = min(max(origin.x, visible.minX), visible.maxX - size.width)
        origin.y = min(max(origin.y, visible.minY), visible.maxY - size.height)
        return origin
    }

    static func fadeIn(_ panel: NSPanel) {
        panel.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { $0.duration = DS.Motion.fast.enter; panel.animator().alphaValue = 1 }
    }

    /// Leaves one step quicker than it arrived.
    static func fadeOut(_ panel: NSPanel) {
        NSAnimationContext.runAnimationGroup({ $0.duration = DS.Motion.fast.exit; panel.animator().alphaValue = 0 },
                                             completionHandler: { panel.orderOut(nil) })
    }

    /// Says the note to VoiceOver, since it never takes focus.
    static func announce(_ words: String, priority: NSAccessibilityPriorityLevel = .medium) {
        NSAccessibility.post(element: NSApp as Any, notification: .announcementRequested,
                             userInfo: [.announcement: words, .priority: priority.rawValue])
    }
}

extension View {
    /// The slip of paper a note by your text is written on (a message, the keep-a-word
    /// question): room round the words, grained paper, the paper shadow.
    func notePaper() -> some View {
        padding(.horizontal, 16).padding(.vertical, 10)
            .background(DS.Surface.overlay)
            .grain(0.22)
            .paperShadow(spread: 6, drop: 7)
    }
}

/// A borderless panel that takes the keyboard, for the ones with rows to arrow through (the
/// menu bar slip, a dropdown's menu). Borderless panels refuse it by default.
final class KeyPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

extension NSScreen {
    /// The screen `point` is on, else the main one.
    static func containing(_ point: CGPoint) -> NSScreen {
        screens.first { $0.frame.contains(point) } ?? main ?? screens[0]
    }
}
