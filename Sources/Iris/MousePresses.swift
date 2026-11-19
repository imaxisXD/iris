import AppKit

/// When the current click's press began, for telling the click that closed a menu from a new
/// one (see `ClosingPress` in IrisCore, which has the rules and their tests).
@MainActor
enum MousePresses {
    private static var lastPressAt: TimeInterval?
    private static var monitor: Any?

    /// Call once at launch, before any menu opens.
    static func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { event in
            lastPressAt = event.timestamp
            return event
        }
    }

    /// Now, on the clock event timestamps use, for a menu noting when it closed.
    static var now: TimeInterval { ProcessInfo.processInfo.systemUptime }

    /// When the press of the click being handled began; nil when it isn't a click (a key).
    static var pressOfThisClick: TimeInterval? {
        switch NSApp.currentEvent?.type {
        case .leftMouseUp, .rightMouseUp, .leftMouseDown, .rightMouseDown: lastPressAt
        default: nil
        }
    }
}
