import AppKit
import IrisCore
import SwiftUI

/// What the bud on the text box is doing. Its face is the panel's status: closed and
/// swaying while Iris reads, open when the fix is ready, dropping its petals when you're done.
@MainActor
final class BudState: ObservableObject {
    enum Phase: Equatable { case hidden, working, open, wilting }
    @Published var phase: Phase = .hidden
    /// When each beat happened (reference-date seconds), for the rig to pose from.
    @Published var arrivedAt: Double?
    @Published var bloomedAt: Double?
    @Published var wiltedAt: Double?
    /// The full arrival (the drop and its splash) or the quick one, once it's familiar.
    @Published var full = true
    /// Whether it went because the fix was put in (it stamps a tick) or was dismissed.
    @Published var replaced = false
}

/// Counts the bud's arrivals on this Mac for `BudArrival`, which decides when it plays in full.
@MainActor
enum BudShow {
    private static let countKey = "budArrivals"
    private static let dayKey = "budLastDay"
    /// Design check: the preview alternates full and quick.
    static var forced: Bool?

    static func next() -> Bool {
        if let forced { return forced }
        let defaults = UserDefaults.standard
        let count = defaults.integer(forKey: countKey)
        let today = Calendar.current.startOfDay(for: Date()).timeIntervalSince1970
        let firstToday = defaults.double(forKey: dayKey) != today
        defaults.set(count + 1, forKey: countKey)
        defaults.set(today, forKey: dayKey)
        return BudArrival.isFull(arrivals: count, firstToday: firstToday, roll: Int.random(in: 0..<BudArrival.nowAndThen))
    }
}

/// The panel's unfold, shared by the controller and the view: folded into the bud until
/// there's something to read, then opening out of it.
@MainActor
final class PanelPresence: ObservableObject {
    @Published var unfolded = true
    /// Where the bud stands, in the panel window's unit space; the panel unfolds from here.
    @Published var anchor = UnitPoint.bottomLeading
}

/// A small window that stands the flower by the corner of the text box the moment the
/// shortcut is pressed, before the panel has anything to show.
@MainActor
final class BudWindow {
    private let window: NSPanel
    private let state = BudState()
    private var hideTask: Task<Void, Never>?

    init() {
        window = NSPanel(contentRect: NSRect(origin: .zero, size: BudView.box),
                         styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        window.isFloatingPanel = true
        window.level = .floating
        window.backgroundColor = .clear
        window.isOpaque = false
        window.hasShadow = false
        window.ignoresMouseEvents = true
        window.hidesOnDeactivate = false
        window.isReleasedWhenClosed = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        window.animationBehavior = .none
        window.contentView = NSHostingView(rootView: BudView(state: state))
    }

    /// Whether the last arrival was the full show (the panel waits a little longer for it).
    var arrivedFull: Bool { state.full }

    /// Pops the bud up with its foot at `point` (screen coordinates) and starts it swaying.
    func show(standingAt point: CGPoint) {
        hideTask?.cancel()
        window.setFrameOrigin(CGPoint(x: point.x - BudView.box.width / 2, y: point.y - BudView.foot))
        state.full = BudShow.next()
        state.bloomedAt = nil
        state.wiltedAt = nil
        state.arrivedAt = Date.timeIntervalSinceReferenceDate
        state.phase = .working
        window.orderFrontRegardless()
    }

    /// Keeps the bud above the panel it stands under.
    func raise() { if window.isVisible { window.orderFrontRegardless() } }

    /// Whether the flower has opened (the answer is ready).
    var isOpen: Bool { state.phase == .open }

    /// Once open it stays open until it wilts: switching to a tone that's still being written
    /// doesn't shut it, and that tone being ready doesn't play the bloom again.
    func set(_ phase: BudState.Phase) {
        guard window.isVisible, state.phase != .wilting, state.phase != .open, state.phase != phase else { return }
        if phase == .open { state.bloomedAt = Date.timeIntervalSinceReferenceDate }
        state.phase = phase
    }

    /// The flower folds and sinks into the ground; when the fix was put in (`replaced`) a petal
    /// stamps a tick by the text box first. The window leaves once it's all gone.
    func wilt(replaced: Bool) {
        guard window.isVisible, state.phase != .wilting else { return }
        state.replaced = replaced
        state.wiltedAt = Date.timeIntervalSinceReferenceDate
        state.phase = .wilting
        hideTask?.cancel()
        hideTask = Task { @MainActor [window, state] in
            try? await Task.sleep(for: .milliseconds(1400))
            guard !Task.isCancelled else { return }
            window.orderOut(nil)
            // Gone: back to hidden, which also stops the rig's clock until the next arrival.
            state.phase = .hidden
        }
    }
}

/// The iris on the text box, played from its rig (`FlowerRig`): every piece is posed from the
/// time since each beat, so nothing can be left half-played between arrivals.
/// Reduce Motion: it appears as a bud, opens and goes, without the motion between.
struct BudView: View {
    /// Room for the burst round the head and for the petals to fall below the foot.
    static let box = CGSize(width: 260, height: 260)
    /// How far the window reaches below where the bud stands.
    static let foot: CGFloat = 60
    /// Big enough to read as the flower beside a 600pt panel, and in a shared clip.
    static let size: CGFloat = 108
    /// How high the bloom's head stands above its foot: the panel unrolls from there.
    static let headroom: CGFloat = IrisRig.stem * size / 92 + 2
    /// How far the open flower reaches round its foot (measured from `Iris --rig-frames`), so
    /// `PanelLayout` stands it beside the paper, its petals clear of the edge.
    static let reach = PanelLayout.Flower(left: IrisRig.reach.left * size / 92, right: IrisRig.reach.right * size / 92,
                                          height: IrisRig.reach.height * size / 92, headroom: headroom)

    @ObservedObject var state: BudState
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: state.phase == .hidden)) { context in
            let now = context.date.timeIntervalSinceReferenceDate
            BudScene(clock: clock(now), size: Self.size)
        }
        .frame(width: Self.box.width, height: Self.box.height - Self.foot, alignment: .bottom)
        .padding(.bottom, Self.foot)
        .opacity(state.phase == .hidden ? 0 : 1)
        .accessibilityHidden(true)
    }

    private func clock(_ now: Double) -> RigClock {
        if reduceMotion {
            // Still poses: a settled bud, then open, then gone.
            return RigClock(now: now, arrived: now - 9, bloomed: state.bloomedAt.map { _ in now - 9 },
                            wilted: state.wiltedAt.map { _ in now - 9 }, full: false)
        }
        return RigClock(now: now, arrived: state.arrivedAt, bloomed: state.bloomedAt, wilted: state.wiltedAt, full: state.full,
                        replaced: state.replaced)
    }
}

/// The flower with its drop and its pop, its foot on the bottom edge of its frame.
struct BudScene: View {
    let clock: RigClock
    var size: CGFloat = 92

    var body: some View {
        FlowerRig(clock: clock, size: size)
            // The rig's base sits this far above its canvas's bottom; put it on the box's edge.
            .offset(y: (IrisRig.canvas.height - IrisRig.base.y) * size / 92)
    }
}
