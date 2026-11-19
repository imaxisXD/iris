import AppKit
import Carbon.HIToolbox
import IrisCore
import SwiftUI

// MARK: - Dropdown

struct MenuItem: Identifiable, Equatable {
    let id: String
    let label: String
    /// An app's bundle ID: the row shows that app's icon before its name.
    var appIcon: String? = nil
    /// Or one of Iris's own icons before the name.
    var risoIcon: RisoIcon? = nil
}

/// The menu surface: a slip of paper with a pen edge (grained, the paper shadow), 32px rows
/// 2px apart. The row under the pointer (or arrow keys) gets a grey ink wash; the chosen row
/// is lettered in brush over a swipe of pink marker, like the chosen tone. Pink means chosen,
/// never hover. Long lists scroll, and follow the highlighted row.
struct MenuList: View {
    let items: [MenuItem]
    let chosen: String?
    @Binding var highlighted: String?
    let pick: (MenuItem) -> Void
    var width: CGFloat = 200
    @State private var shown = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// Taller lists scroll inside this height (about 11 rows).
    static let maxHeight: CGFloat = 372
    /// Paper round the rows.
    static let inset: CGFloat = 5

    var body: some View {
        let rows = VStack(spacing: 2) {
            ForEach(items) { item in
                let isChosen = item.id == chosen
                let isHighlighted = item.id == highlighted
                // A real button, so VoiceOver and other assistive tools can choose it.
                Button { pick(item) } label: {
                    HStack(spacing: 9) {
                        if let app = item.appIcon {
                            AppIconView(bundleID: app, size: 18)
                        } else if let icon = item.risoIcon {
                            RisoIconView(icon: icon, size: 18)
                        }
                        if isChosen {
                            Text(item.label).font(DS.Font.brush(20)).foregroundStyle(DS.Ink.i12)
                                .background(alignment: .bottom) {
                                    MarkerSwipe(height: 6, seed: item.id.count * 5 + 1).offset(y: -1)
                                }
                        } else {
                            Text(item.label).font(DS.Font.body)
                                .foregroundStyle(isHighlighted ? DS.Ink.i12 : DS.Ink.i11)
                        }
                    }
                    .lineLimit(1)
                    .accessibilityElement(children: .combine)
                    .padding(.horizontal, 10)
                    .frame(maxWidth: .infinity, minHeight: DS.Size.control, alignment: .leading)
                    .penWash(hover: isHighlighted, seed: item.id.count * 7 + 3)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .id(item.id)
                .accessibilityAddTraits(isChosen ? .isSelected : [])
                .onHover { if $0 { highlighted = item.id } }
                .animation(DS.easeOut, value: isHighlighted)
            }
        }
        .padding(Self.inset)

        Group {
            if CGFloat(items.count) * (DS.Size.control + 2) + Self.inset * 2 > Self.maxHeight {
                ScrollViewReader { proxy in
                    ScrollView { rows }
                        .frame(height: Self.maxHeight)
                        .onChange(of: highlighted) { _, id in if let id { proxy.scrollTo(id) } }
                }
            } else {
                rows
            }
        }
        .frame(width: width)
        .background(DS.Surface.overlay)
        .grain(0.22)
        // A pen edge, like the trigger above it, so the list stands off the page under it.
        .overlay(HandRect(seed: 91, wobble: 0.45, overshoot: 2)
            .stroke(DS.Ink.i12.opacity(0.6), style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round))
            .allowsHitTesting(false))
        .paperShadow(spread: 6, drop: 6)
        .scaleEffect(shown || reduceMotion ? 1 : 0.97, anchor: .top)
        .opacity(shown ? 1 : 0)
        .onAppear { withAnimation(DS.Motion.fast.enterAnimation) { shown = true } }
    }
}

/// A borderless child window that hosts a MenuList under its trigger.
@MainActor
final class MenuPanel: NSObject, NSWindowDelegate {
    /// Transparent room round the menu for its drop shadow.
    static let shadowRoom: CGFloat = 12
    static let shared = MenuPanel()
    private var panel: NSPanel?
    private var monitor: Any?
    private var onClose: (() -> Void)?
    private var model = MenuModel()

    var isOpen: Bool { panel != nil }

    final class MenuModel: ObservableObject {
        @Published var items: [MenuItem] = []
        @Published var chosen: String?
        @Published var highlighted: String?
        var pick: (MenuItem) -> Void = { _ in }
        var width: CGFloat = 200
    }

    private struct Host: View {
        @ObservedObject var model: MenuModel
        var body: some View {
            MenuList(items: model.items, chosen: model.chosen, highlighted: $model.highlighted,
                     pick: model.pick, width: model.width)
                .padding(MenuPanel.shadowRoom)
        }
    }

    /// The trigger the menu last hung from, and when the menu closed. Clicking a trigger while
    /// its menu is open closes the menu on the press (it loses focus) before the trigger's
    /// action fires; this tells that click apart, so a second click closes, like any menu
    /// (see `ClosingPress`).
    private weak var lastAnchor: NSView?
    private var closing = ClosingPress()

    /// Opens the menu under `anchor`, or closes it if it's already open there. Returns
    /// whether it's open now.
    @discardableResult
    func show(items: [MenuItem], chosen: String?, below anchor: NSView, width: CGFloat,
              pick: @escaping (MenuItem) -> Void, onClose: @escaping () -> Void) -> Bool {
        let sameTrigger = anchor === lastAnchor
        let closedByThisClick = closing.isClosingClick(pressBegan: MousePresses.pressOfThisClick)
        if sameTrigger, panel != nil || closedByThisClick {
            close()
            onClose()
            return false
        }
        close()
        lastAnchor = anchor
        guard let window = anchor.window else { return false }
        let anchorRect = window.convertToScreen(anchor.convert(anchor.bounds, to: nil))
        let model = MenuModel()
        model.items = items
        model.chosen = chosen
        model.highlighted = chosen
        model.width = max(width, anchorRect.width)
        model.pick = { [weak self] item in
            pick(item)
            self?.close()
        }
        self.model = model
        self.onClose = onClose

        let host = NSHostingView(rootView: Host(model: model))
        let size = host.fittingSize
        let panel = KeyPanel(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless],
                             backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .popUpMenu
        panel.contentView = host
        panel.delegate = self
        // 6px under the trigger, left edges aligned (the room for the drop sits outside the menu).
        let room = Self.shadowRoom
        var origin = CGPoint(x: anchorRect.minX - room, y: anchorRect.minY - 6 - size.height + room)
        if let screen = window.screen?.visibleFrame, origin.y < screen.minY {
            origin.y = anchorRect.maxY + 6 - room  // no room below: open upward
        }
        panel.setFrameOrigin(origin)
        window.addChildWindow(panel, ordered: .above)
        panel.makeKey()
        self.panel = panel
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel?.isKeyWindow == true else { return event }
            return self.handle(event) ? nil : event
        }
        return true
    }

    /// Letters typed while the menu is open, to jump to "Mail" by typing "ma".
    private var typed = ""
    private var typedAt = Date.distantPast

    private func handle(_ event: NSEvent) -> Bool {
        let ids = model.items.map(\.id)
        let flags = event.modifierFlags.intersection([.command, .control, .option])
        if flags.isEmpty, let chars = event.characters, chars.count == 1, let c = chars.first, c.isLetter || c.isNumber {
            if Date().timeIntervalSince(typedAt) > 1 { typed = "" }
            typed += chars.lowercased()
            typedAt = Date()
            if let match = model.items.first(where: { $0.label.lowercased().hasPrefix(typed) }) { model.highlighted = match.id }
            return true
        }
        let current = model.highlighted.flatMap { ids.firstIndex(of: $0) }
        switch Int(event.keyCode) {
        case kVK_Escape: close(); return true
        case kVK_DownArrow, kVK_UpArrow:
            model.highlighted = ListKeys.step(from: current, count: ids.count, down: Int(event.keyCode) == kVK_DownArrow).map { ids[$0] }
            return true
        case kVK_Return, kVK_ANSI_KeypadEnter, kVK_Space:
            if let id = model.highlighted, let item = model.items.first(where: { $0.id == id }) { model.pick(item) }
            return true
        default: return false
        }
    }

    func close() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        guard let panel else { return }
        self.panel = nil
        closing.closed(at: MousePresses.now)
        panel.parent?.removeChildWindow(panel)
        NSAnimationContext.runAnimationGroup({ $0.duration = DS.Motion.fast.exit; panel.animator().alphaValue = 0 },
                                             completionHandler: { panel.orderOut(nil) })
        onClose?()
        onClose = nil
    }

    func windowDidResignKey(_ notification: Notification) { close() }
}

/// Holds the trigger's NSView so the menu can open under it.
private final class AnchorBox { weak var view: NSView? }
private struct AnchorView: NSViewRepresentable {
    let box: AnchorBox
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        box.view = view
        return view
    }
    func updateNSView(_ nsView: NSView, context: Context) { box.view = nsView }
}

/// The select trigger: the secondary button with a chevron that turns over while open.
struct Dropdown<Value: Hashable>: View {
    let options: [(value: Value, label: String)]
    @Binding var selection: Value
    var width: CGFloat = 180
    /// A riso icon before the value, e.g. the clock on a duration.
    var icon: RisoIcon? = nil
    /// Snapshots only: draw the menu open, in place.
    var forcedOpen = false

    @State private var open = false
    @State private var hovering = false
    @State private var box = AnchorBox()
    @Environment(\.forcedInteraction) private var forced
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private func key(_ value: Value) -> String { String(describing: value) }
    private var label: String { options.first { $0.value == selection }?.label ?? "" }

    var body: some View {
        let isOpen = open || forcedOpen
        let state = forced ?? Interaction(hover: hovering)
        Button {
            guard let anchor = box.view else { return }
            open = MenuPanel.shared.show(
                items: options.map { MenuItem(id: key($0.value), label: $0.label) }, chosen: key(selection),
                below: anchor, width: width,
                pick: { item in if let match = options.first(where: { key($0.value) == item.id }) { selection = match.value } },
                onClose: { open = false })
        } label: {
            HStack(spacing: 8) {
                if let icon { RisoIconView(icon: icon, size: 20).padding(.leading, -2) }
                Text(label).font(DS.Font.body).tracking(-0.12).foregroundStyle(DS.Ink.i12).lineLimit(1)
                Spacer(minLength: 4)
                HandChevron().stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
                    .frame(width: 9, height: 5)
                    .rotationEffect(.degrees(isOpen ? 180 : 0))
                    .animation(reduceMotion ? nil : DS.Motion.fast.enterAnimation, value: isOpen)
            }
            .padding(.horizontal, 10)
            .frame(width: width, height: DS.Size.control)
            .background {
                // Drawn like the secondary button: a pen outline on a paper face with a crisp drop.
                ZStack {
                    HandRect(seed: 71, wobble: 0.3, closed: true).fill(DS.drop(0.18)).offset(y: 1)
                    HandRect(seed: 72, wobble: 0.4, closed: true)
                        .fill(state.hover || isOpen ? DS.Surface.controlHover : DS.Surface.control)
                    HandRect(seed: 73, wobble: 0.45, overshoot: 2)
                        .stroke(DS.Ink.i12.opacity(0.75), style: StrokeStyle(lineWidth: 1.2, lineCap: .round, lineJoin: .round))
                }
            }
            .background(AnchorView(box: box))
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.97))
        .onHover { hovering = $0 }
        .animation(DS.easeOut, value: hovering)
        .overlay(alignment: .topLeading) {
            if forcedOpen {
                MenuList(items: options.map { MenuItem(id: key($0.value), label: $0.label) }, chosen: key(selection),
                         highlighted: .constant(options.dropFirst().first.map { key($0.value) }), pick: { _ in }, width: width)
                    .offset(y: DS.Size.control + 6)
            }
        }
        .zIndex(forcedOpen ? 1 : 0)
        .accessibilityLabel(label)
    }
}

/// Action menu (no chosen item), e.g. "Add app": the same surface and trigger.
struct ActionMenuButton: View {
    let title: String
    let items: () -> [MenuItem]
    let pick: (MenuItem) -> Void
    @State private var box = AnchorBox()

    var body: some View {
        TactileButton(title: title, role: .secondary, icon: "plus") {
            guard let anchor = box.view else { return }
            MenuPanel.shared.show(items: items(), chosen: nil, below: anchor, width: 240, pick: pick, onClose: {})
        }
        .background(AnchorView(box: box))
    }
}

// MARK: - Confirm sheet

struct ConfirmRequest: Identifiable {
    let id = UUID()
    let title: String
    let message: String
    let confirmTitle: String
    var destructive = true
    let action: () -> Void
}

/// The confirm a window is showing, if any. Each `SheetHost` (each window) has its own.
@MainActor
final class SheetCenter: ObservableObject {
    @Published var request: ConfirmRequest?
}

extension EnvironmentValues {
    /// Asks the window this view is in to confirm something (`SheetHost`). Nothing outside one.
    @Entry var confirm: @MainActor (ConfirmRequest) -> Void = { _ in }
}

/// Two-part confirm: a band with the question and its consequence, then the actions row.
/// Cancel left (secondary), the named confirm right; Return confirms, Esc cancels.
struct ConfirmCard: View {
    let request: ConfirmRequest
    let dismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The question in brush (red pen when it destroys something), the consequence plain.
            VStack(alignment: .leading, spacing: 6) {
                Text(request.title).font(DS.Font.brush(25)).foregroundStyle(request.destructive ? DS.Hue.redText : DS.Ink.i12)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                Text(request.message).font(DS.Font.body).foregroundStyle(DS.Ink.i11).fixedSize(horizontal: false, vertical: true)
            }
            .padding(EdgeInsets(top: 20, leading: 20, bottom: 16, trailing: 20))
            PenRule()
            HStack(spacing: 12) {
                TactileButton(title: "Cancel", role: .secondary) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                TactileButton(title: request.confirmTitle, role: request.destructive ? .destructiveConfirm : .primary) {
                    request.action()
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
            .padding(12)
        }
        .frame(width: 420)
        // A slip of paper laid on the page: square, grained, a pen edge and the paper shadow,
        // a hair off straight. Never a rounded card.
        .background(DS.Surface.overlay)
        .grain(0.22)
        .overlay(HandRect(seed: 57, wobble: 0.45, overshoot: 2)
            .stroke(DS.Ink.i12.opacity(0.55), style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round))
            .allowsHitTesting(false))
        .paperShadow(spread: 10, drop: 12)
        .rotationEffect(.degrees(-0.4))
    }
}

/// Dims the window and shows the confirm card. In 240ms (16px rise, fade), out 200ms.
struct SheetHost<Content: View>: View {
    @StateObject private var center = SheetCenter()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ViewBuilder let content: Content

    init(@ViewBuilder content: () -> Content) { self.content = content() }

    var body: some View {
        ZStack {
            content.blur(radius: center.request == nil ? 0 : 1)
                .environment(\.confirm, { [center] in center.request = $0 })
            if let request = center.request {
                Color.black.opacity(0.3)
                    .ignoresSafeArea()
                    .onTapGesture { center.request = nil }
                    .transition(.opacity)
                ConfirmCard(request: request) { center.request = nil }
                    // Rises 16px on the way in; under Reduce Motion it only fades.
                    .transition(.asymmetric(
                        insertion: (reduceMotion ? AnyTransition.opacity : .opacity.combined(with: .offset(y: 16)))
                            .animation(DS.Motion.slow.enterAnimation),
                        removal: .opacity.animation(DS.Motion.slow.exitAnimation)))
            }
        }
        .animation(center.request == nil ? DS.Motion.slow.exitAnimation : DS.Motion.slow.enterAnimation, value: center.request?.id)
    }
}
