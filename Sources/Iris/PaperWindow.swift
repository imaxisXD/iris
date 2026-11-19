import AppKit
import SwiftUI

/// Setup and Settings: a window that is one sheet of paper, the paper under its transparent
/// title bar too. The setup preview makes it the same way, so it looks like the real thing.
@MainActor
enum PaperWindow {
    /// `remembers` names a window that opens where the person last left it (Settings);
    /// without one, or when that place is no longer on a screen, it opens in the middle.
    static func make<Content: View>(title: String, content: Content, remembers name: String? = nil) -> NSWindow {
        let host = NSHostingController(rootView: content.font(DS.Font.body))
        let window = NSWindow(contentViewController: host)
        window.title = title
        window.styleMask = [.titled, .closable, .miniaturizable, .fullSizeContentView]
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isReleasedWhenClosed = false
        window.backgroundColor = DS.nsColor(Riso.paper)
        // Take the size the page asks for before placing the window. Placed first, the window
        // was centred at its starting size, then grew from its top-left corner as the page
        // laid out, so it first opened low and off to the right.
        window.setContentSize(host.view.fittingSize)
        place(window, remembering: name)
        return window
    }

    private static var moveWatchers: [NSObjectProtocol] = []

    private static func place(_ window: NSWindow, remembering name: String?) {
        guard let name else { return window.center() }
        let key = "windowTopLeft.\(name)"
        let saved = UserDefaults.standard.string(forKey: key).map(NSPointFromString)
        // Back where it was, if its title bar is still on a screen (a display may be gone).
        if let saved, NSScreen.screens.contains(where: { $0.visibleFrame.contains(NSPoint(x: saved.x + 80, y: saved.y - 12)) }) {
            window.setFrameTopLeftPoint(saved)
        } else {
            window.center()
        }
        moveWatchers.append(NotificationCenter.default.addObserver(forName: NSWindow.didMoveNotification, object: window, queue: .main) { note in
            guard let frame = (note.object as? NSWindow)?.frame else { return }
            UserDefaults.standard.set(NSStringFromPoint(NSPoint(x: frame.minX, y: frame.maxY)), forKey: key)
        })
    }
}
