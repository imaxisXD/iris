import AppKit
import IrisCore
import IrisEngine
import SwiftUI

// Design check: every message Iris shows by your text (`ToastView`), straight from `Message`,
// where the app shows them from. Nothing is read or pasted.
//   Iris --toast-states <dir>          writes every state as a PNG, over a dark and a light app,
//                                      and prints each one's width and how long it stays
//   Iris --toast-workbench [state]     a window with a switcher (↑ ↓ to step, H hides the list)

@MainActor
enum ToastStates {
    struct State: Identifiable {
        let id: String
        let group: String
        let message: Message
        /// When it shows, for the switcher's caption.
        let from: String
    }

    static var states: [State] {
        let iconPath = FileManager.default.fileExists(atPath: "/Applications/1Password.app")
            ? "/Applications/1Password.app" : "/System/Applications/Notes.app"
        let appName = iconPath.contains("1Password") ? "1Password" : "Notes"
        let icon = NSWorkspace.shared.icon(forFile: iconPath)
        func problem(_ id: String, _ error: Error, _ from: String) -> State {
            State(id: id, group: "Problems", message: .problem(error), from: from)
        }
        return [
            State(id: "replaced", group: "Done", message: .replaced(undoKeys: true), from: "Replace in the panel"),
            State(id: "replaced-no-undo", group: "Done", message: .replaced(undoKeys: false),
                  from: "Replace, with the undo keys turned off in Settings"),
            State(id: "fixed-1", group: "Done", message: .fixed(1, undoKeys: true), from: "Quick Fix pasted a one-word fix"),
            State(id: "fixed-4", group: "Done", message: .fixed(4, undoKeys: true), from: "Quick Fix pasted its fix"),
            State(id: "looks-good", group: "Done", message: .looksGood, from: "Fix found nothing, in the panel or Quick Fix"),
            State(id: "copied", group: "Done", message: .copied, from: "Copy on text that can't be edited"),
            State(id: "kept", group: "Done", message: .kept("Northwind"), from: "You put a word back after a replace"),

            State(id: "checking", group: "Quiet notes", message: .checking, from: "Quick Fix starts; stays until its answer replaces it"),
            State(id: "turned-off-app", group: "Quiet notes", message: .turnedOff(name: appName, icon: icon),
                  from: "Shortcut in a turned-off app"),
            State(id: "turned-off-unknown", group: "Quiet notes", message: .turnedOff(name: nil, icon: nil),
                  from: "Turned-off app with no name"),
            State(id: "left-alone", group: "Quiet notes", message: .leftAlone,
                  from: "Quick Fix finished after the box changed (QuickFixLanding.leftAlone)"),
            State(id: "password", group: "Quiet notes", message: .problem(TextBridge.CaptureError.secureField),
                  from: "Shortcut in a password field"),

            problem("nothing-to-read", TextBridge.CaptureError.nothingToCheck, "No text box found"),
            problem("empty", RewriteError.emptyInput, "Quick Fix on an empty box"),
            problem("too-long", RewriteError.tooLong(limit: Rewriter.maxInputCharacters), "Quick Fix on a long text"),
            problem("no-model", EngineError.notDownloaded, "Quick Fix with no model on this Mac"),
            problem("no-disk", EngineError.notEnoughDisk(needed: "6.2 GB", free: "3.1 GB"), "Getting the model with a full disk"),
            problem("offline", URLError(.notConnectedToInternet), "Getting the model while offline"),
            problem("unknown", CocoaError(.fileReadUnknown), "Anything else: no system words on screen"),
        ]
    }
}

/// The workbench window: the switcher (deliberately not Iris's design) beside the real note.
struct ToastStatesView: View {
    let states = ToastStates.states
    @State var index: Int
    @State var listHidden = false

    var body: some View {
        HStack(spacing: 0) {
            if !listHidden { switcher }
            stage
        }
        .background(KeyCatcher { key in
            switch key {
            case .up: index = max(0, index - 1)
            case .down: index = min(states.count - 1, index + 1)
            case .toggle: listHidden.toggle()
            }
        })
    }

    private var current: ToastStates.State { states[index] }

    private var switcher: some View {
        let groups = Array(NSOrderedSet(array: states.map(\.group))) as! [String]
        return ScrollViewReader { scroller in
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    ForEach(groups, id: \.self) { group in
                        VStack(alignment: .leading, spacing: 2) {
                            Label(group, systemImage: "arrow.turn.down.right")
                                .font(.system(size: 11, weight: .semibold)).foregroundStyle(.secondary)
                                .padding(.bottom, 4)
                                .accessibilityAddTraits(.isHeader)
                            ForEach(Array(states.enumerated()).filter { $0.element.group == group }, id: \.element.id) { i, state in
                                Button { index = i } label: {
                                    Text(state.id).font(.system(size: 12, weight: i == index ? .semibold : .regular))
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .padding(.vertical, 4).padding(.horizontal, 8)
                                        .background(i == index ? Color.white.opacity(0.14) : .clear, in: RoundedRectangle(cornerRadius: 5))
                                }
                                .buttonStyle(.plain)
                                .accessibilityAddTraits(i == index ? .isSelected : [])
                                .id(i)
                            }
                        }
                    }
                }
                .padding(14)
            }
            .onChange(of: index) { _, i in withAnimation(nil) { scroller.scrollTo(i) } }
        }
        .frame(width: 210)
        .background(Color(white: 0.11))
        .foregroundStyle(.white)
        .environment(\.colorScheme, .dark)
        .accessibilityLabel("States")
    }

    private var stage: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach([DS.rgb(0x1F1C27), DS.rgb(0xF0EBE6)], id: \.self) { ground in
                ZStack(alignment: .topLeading) {
                    Color(nsColor: ground)
                    ToastStates.note(current).padding(30)
                }
            }
            // The facts the note can't show: where it comes from, how wide, how long it stays.
            VStack(alignment: .leading, spacing: 3) {
                Text("\(current.from)").font(.system(size: 11))
                Text("\(Int(ToastStates.width(current))) pt wide · \(current.message.kind == .warning ? "stays until a click or key" : "fades after \(current.message.stays.formatted())") · ↑ ↓ to step, H hides the list")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(white: 0.11)).foregroundStyle(.white)
        }
    }
}

extension ToastStates {
    static func note(_ s: State) -> some View {
        ToastView(message: s.message)
            .fixedSize()
    }

    /// The width the note takes on screen, as `Toast.show` measures it (its padding included).
    static func width(_ s: State) -> CGFloat {
        NSHostingView(rootView: note(s).padding(14)).fittingSize.width
    }

    static func openWindow(start: String?) {
        let first = states.firstIndex { $0.id == start } ?? 0
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1240, height: 420),
                              styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
        window.title = "States · the message by your text"
        window.contentView = NSHostingView(rootView: ToastStatesView(index: first))
        window.center()
        window.isReleasedWhenClosed = false
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
        keep = window
    }
    private static var keep: NSWindow?

    static func render(into dir: String) {
        FontRegistry.registerAll()
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        for s in states {
            let w = width(s)
            ComponentSnapshot.pure(s.id, width: max(w + 60, 520), dir: dir, prefix: "toast") {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach([DS.rgb(0x1F1C27), DS.rgb(0xF0EBE6)], id: \.self) { ground in
                        note(s).padding(16).frame(maxWidth: .infinity, alignment: .leading).background(Color(nsColor: ground))
                    }
                }
            }
            print("\(s.id)\t\(Int(w))pt\t\(s.message.stays)")
        }
    }
}

/// Arrow keys and H for the switcher, ignoring anything with a modifier.
private struct KeyCatcher: NSViewRepresentable {
    enum Key { case up, down, toggle }
    let onKey: (Key) -> Void
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty else { return event }
            switch event.keyCode {
            case 126, 123: onKey(.up); return nil
            case 125, 124: onKey(.down); return nil
            case 4: onKey(.toggle); return nil
            default: return event
            }
        }
        return view
    }
    func updateNSView(_ nsView: NSView, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator { var monitor: Any? }
}
