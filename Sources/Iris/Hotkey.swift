import AppKit
import Carbon.HIToolbox

/// A keyboard shortcut, stored as a Carbon key code plus Carbon modifier mask.
struct KeyCombo: Codable, Equatable, Hashable {
    var keyCode: UInt32
    var modifiers: UInt32

    static let openDefault = KeyCombo(keyCode: UInt32(kVK_ANSI_G), modifiers: UInt32(controlKey | optionKey))
    static let quickFixDefault = KeyCombo(keyCode: UInt32(kVK_ANSI_G), modifiers: UInt32(controlKey | optionKey | shiftKey))

    init(keyCode: UInt32, modifiers: UInt32) {
        self.keyCode = keyCode
        self.modifiers = modifiers
    }

    init?(event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        var carbon: UInt32 = 0
        if flags.contains(.control) { carbon |= UInt32(controlKey) }
        if flags.contains(.option) { carbon |= UInt32(optionKey) }
        if flags.contains(.shift) { carbon |= UInt32(shiftKey) }
        if flags.contains(.command) { carbon |= UInt32(cmdKey) }
        self.init(keyCode: UInt32(event.keyCode), modifiers: carbon)
    }

    var hasPrimaryModifier: Bool {
        modifiers & UInt32(controlKey | optionKey | cmdKey) != 0
    }

    /// Mac order: ⌃ ⌥ ⇧ ⌘ then the key.
    var symbols: [String] {
        var parts: [String] = []
        if modifiers & UInt32(controlKey) != 0 { parts.append("⌃") }
        if modifiers & UInt32(optionKey) != 0 { parts.append("⌥") }
        if modifiers & UInt32(shiftKey) != 0 { parts.append("⇧") }
        if modifiers & UInt32(cmdKey) != 0 { parts.append("⌘") }
        parts.append(Self.keyName(keyCode))
        return parts
    }

    var display: String { symbols.joined() }

    /// The same keys in words, for people who don't read ⌃⌥⇧⌘: "Control + Option + G".
    var spoken: String {
        let words = ["⌃": "Control", "⌥": "Option", "⇧": "Shift", "⌘": "Command", "↩": "Return", "⇥": "Tab",
                     "⌫": "Delete", "⌦": "Forward Delete", "esc": "Escape", "←": "Left Arrow", "→": "Right Arrow",
                     "↑": "Up Arrow", "↓": "Down Arrow", "↖": "Home", "↘": "End", "⇞": "Page Up", "⇟": "Page Down"]
        return symbols.map { words[$0] ?? $0 }.joined(separator: " + ")
    }

    /// One key as a Mac keyboard prints it: (⌃, control), or (nil, G) for the key itself.
    typealias Cap = (symbol: String?, word: String)

    /// The keys as a Mac keyboard prints them, symbol and word: (⌃, control), (⌥, option), (nil, G).
    var keycaps: [Cap] {
        let printed = ["⌃": "control", "⌥": "option", "⇧": "shift", "⌘": "command", "↩": "return", "⇥": "tab",
                       "⌫": "delete"]
        return symbols.map { key in printed[key].map { (symbol: key, word: $0) } ?? (symbol: nil, word: key) }
    }


    /// Label for a key on the user's current keyboard layout (so AZERTY shows "A" where it is).
    static func keyName(_ keyCode: UInt32) -> String {
        let special: [Int: String] = [
            kVK_Space: "Space", kVK_Return: "↩", kVK_Tab: "⇥", kVK_Delete: "⌫", kVK_ForwardDelete: "⌦",
            kVK_Escape: "esc", kVK_LeftArrow: "←", kVK_RightArrow: "→", kVK_UpArrow: "↑", kVK_DownArrow: "↓",
            kVK_Home: "↖", kVK_End: "↘", kVK_PageUp: "⇞", kVK_PageDown: "⇟",
            kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5", kVK_F6: "F6",
            kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        ]
        if let name = special[Int(keyCode)] { return name }

        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
            let layoutPointer = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else { return "?" }
        let layoutData = Unmanaged<CFData>.fromOpaque(layoutPointer).takeUnretainedValue() as Data
        var deadKeyState: UInt32 = 0
        var chars = [UniChar](repeating: 0, count: 4)
        var length = 0
        let status = layoutData.withUnsafeBytes { raw -> OSStatus in
            guard let layout = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
            return UCKeyTranslate(layout, UInt16(keyCode), UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                                  OptionBits(kUCKeyTranslateNoDeadKeysBit), &deadKeyState, chars.count, &length, &chars)
        }
        guard status == noErr, length > 0 else { return "?" }
        return String(utf16CodeUnits: chars, count: length).uppercased()
    }
}

/// Why a shortcut can't or shouldn't be used. Checked before saving.
enum ShortcutProblem: Equatable {
    case needsModifier
    case reservedBySystem(String)
    case commonConflict(String)
    case usedByOtherIrisShortcut
    case takenByAnotherApp

    var message: String {
        switch self {
        case .needsModifier: "Add ⌃, ⌥, or ⌘ so it doesn't fire while you type."
        case .reservedBySystem(let what): "macOS uses this for \(what). Pick another."
        case .commonConflict(let what): "Usually taken by \(what). It may not reach Iris."
        case .usedByOtherIrisShortcut: "Your other Iris shortcut already uses this."
        case .takenByAnotherApp: "Another app already uses this shortcut."
        }
    }

    /// Warnings can still be saved; errors can't.
    var isBlocking: Bool {
        if case .commonConflict = self { return false }
        return true
    }

    static func check(_ combo: KeyCombo, other: KeyCombo?) -> ShortcutProblem? {
        guard combo.hasPrimaryModifier else { return .needsModifier }
        if let other, other == combo { return .usedByOtherIrisShortcut }

        let cmd = UInt32(cmdKey), ctrl = UInt32(controlKey), opt = UInt32(optionKey)
        let reserved: [(Int, UInt32, String)] = [
            (kVK_Space, cmd, "Spotlight"),
            (kVK_Space, ctrl, "switching input languages"),
            (kVK_Space, ctrl | opt, "switching input languages"),
            (kVK_Tab, cmd, "switching apps"),
            (kVK_ANSI_Q, cmd, "Quit"), (kVK_ANSI_W, cmd, "Close window"),
            (kVK_ANSI_C, cmd, "Copy"), (kVK_ANSI_V, cmd, "Paste"), (kVK_ANSI_X, cmd, "Cut"),
            (kVK_ANSI_Z, cmd, "Undo"), (kVK_ANSI_A, cmd, "Select All"), (kVK_ANSI_S, cmd, "Save"),
            (kVK_ANSI_H, cmd, "Hide"), (kVK_ANSI_M, cmd, "Minimize"),
        ]
        for (key, mods, what) in reserved where UInt32(key) == combo.keyCode && mods == combo.modifiers {
            return .reservedBySystem(what)
        }
        let common: [(Int, UInt32, String)] = [
            (kVK_Space, opt, "ChatGPT and Raycast"),
            (kVK_Space, cmd | opt, "Finder search"),
            (kVK_ANSI_K, cmd, "Slack's quick switcher"),
            (kVK_ANSI_B, cmd, "Bold in Slack and most editors"),
            (kVK_ANSI_I, cmd, "Italic in Slack and most editors"),
        ]
        for (key, mods, what) in common where UInt32(key) == combo.keyCode && mods == combo.modifiers {
            return .commonConflict(what)
        }
        return nil
    }
}

/// Registers Iris's global shortcuts with Carbon. Carbon hotkeys need no
/// Accessibility permission and never see other keystrokes.
@MainActor
final class HotkeyCenter {
    enum Action: UInt32 { case open = 1, quickFix = 2 }

    static let shared = HotkeyCenter()

    var handler: ((Action) -> Void)?
    private var refs: [Action: EventHotKeyRef] = [:]
    private var eventHandler: EventHandlerRef?
    private var paused = false

    private init() {
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
            var id = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                              nil, MemoryLayout<EventHotKeyID>.size, nil, &id)
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    if let action = Action(rawValue: id.id) { HotkeyCenter.shared.fire(action) }
                }
            }
            return noErr
        }, 1, &eventType, nil, &eventHandler)
    }

    private func fire(_ action: Action) {
        guard !paused else { return }
        handler?(action)
    }

    /// Registers both shortcuts as Settings has them: at launch, whenever one changes, and
    /// after the recorder. Notes in Settings whether another app holds the one that opens the
    /// panel, which the slip and setup say with a way to change it.
    func apply(_ settings: AppSettings? = nil) {
        let settings = settings ?? .shared
        guard !paused else { return }
        let opens = register(.open, settings.openShortcut)
        if settings.openShortcutTaken == opens { settings.openShortcutTaken = !opens }
        register(.quickFix, settings.quickFixShortcut)
    }

    /// Returns false if the system refused (usually: another app owns it).
    @discardableResult
    private func register(_ action: Action, _ combo: KeyCombo?) -> Bool {
        unregister(action)
        guard let combo else { return true }
        var ref: EventHotKeyRef?
        let status = RegisterEventHotKey(combo.keyCode, combo.modifiers, Self.id(action.rawValue), GetApplicationEventTarget(), 0, &ref)
        guard status == noErr, let ref else { return false }
        refs[action] = ref
        return true
    }

    private func unregister(_ action: Action) {
        if let ref = refs.removeValue(forKey: action) { UnregisterEventHotKey(ref) }
    }

    /// Tries a combo without keeping it, to detect shortcuts owned by other apps.
    func isAvailable(_ combo: KeyCombo) -> Bool {
        var ref: EventHotKeyRef?
        let status = RegisterEventHotKey(combo.keyCode, combo.modifiers, Self.id(99), GetApplicationEventTarget(), 0, &ref)
        if status == noErr, let ref {
            UnregisterEventHotKey(ref)
            return true
        }
        return status != eventHotKeyExistsErr
    }

    private static func id(_ number: UInt32) -> EventHotKeyID {
        EventHotKeyID(signature: OSType(0x49524953), id: number)  // 'IRIS'
    }

    /// While the recorder is listening the shortcuts are let go, so pressing the current
    /// keys records them instead of firing; after, they're back as Settings has them.
    func setPaused(_ value: Bool) {
        paused = value
        if value {
            unregister(.open)
            unregister(.quickFix)
        } else {
            apply()
        }
    }
}
