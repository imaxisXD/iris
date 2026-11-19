import AppKit
import ApplicationServices
import Carbon.HIToolbox
import IrisCore

/// Reads the text the user is writing in another app and writes the result back.
///
/// Read: Accessibility API (selected text, else the whole field). Electron apps
/// like Slack only expose text after `AXManualAccessibility` is set on them.
/// Fallback: copy the current selection with ⌘C.
///
/// Write: paste. It is the one method that works the same in native fields,
/// Electron, and browsers. The user's clipboard is saved first and put back after.
enum TextBridge {
    struct Captured {
        enum Scope { case selection, wholeField }
        let app: NSRunningApplication
        let element: AXUIElement?
        let text: String
        let scope: Scope
        /// Where the bud stands and notes appear, in Cocoa screen coordinates (origin
        /// bottom-left): the text box, or the selected words when the "box" is a whole page
        /// (text selected in a chat history or a web page). Nil when neither is known, so
        /// Iris shows up at the pointer.
        let fieldFrame: CGRect?
        /// False when the text can't be edited (selected in a chat history or a web page):
        /// a paste would go nowhere, so the panel offers Copy instead of Replace.
        var canReplace = true
    }

    enum CaptureError: LocalizedError {
        case notTrusted
        case secureField
        case nothingToCheck

        var errorDescription: String? {
            switch self {
            case .notTrusted: "Iris needs Accessibility permission to read and replace text."
            case .secureField: "That's a password field. Iris never reads those."
            case .nothingToCheck: "No text found. Click in a text box, or select some text, then try again."
            }
        }
    }

    static var isTrusted: Bool { AXIsProcessTrusted() }

    static func requestTrust() {
        let key = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        _ = AXIsProcessTrustedWithOptions([key: true] as CFDictionary)
    }

    static func openAccessibilitySettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    /// Whether the box Iris read still holds the same text, so a Quick Fix that finished
    /// late can't paste over something else. Reads the same app's focus again; with no box to
    /// compare (text copied out of an app that hides it), only a quick answer counts.
    @MainActor
    static func stillHolds(_ captured: Captured, since pressed: Date) async -> Bool {
        guard let element = captured.element else { return QuickFixLanding.unreadableStillThere(after: Date().timeIntervalSince(pressed)) }
        let pid = captured.app.processIdentifier
        let read = await Task.detached(priority: .userInitiated) { readFocused(pid: pid, enableElectron: false, askForPages: false) }.value
        guard let now = read.element, CFEqual(now, element) else { return false }
        return read.text == captured.text
    }

    // MARK: Capture

    /// What the Accessibility read found, gathered off the main thread.
    private struct FocusRead: @unchecked Sendable {
        var element: AXUIElement?
        var text: String?
        var scope: Captured.Scope = .wholeField
        var axFrame: CGRect?  // top-left origin, converted on the main thread
        /// Where the selected words (or the caret) are, same origin.
        var selectionFrame: CGRect?
        var editable = true
        var secure = false
        /// The process that owns `element`.
        var ownerPid: pid_t?
    }

    /// How long a single Accessibility call may block before we give up on that app.
    /// A hung app otherwise stalls the caller for seconds.
    static let axTimeout: Float = 0.25

    /// The app whose text box you're in: the one that gets your keystrokes.
    ///
    /// `NSWorkspace.frontmostApplication` alone gets this wrong. When the app in front quits
    /// (Iris itself, relaunching after an update), Launch Services marks loginwindow as
    /// frontmost and keeps saying so after keyboard focus has gone back to the app you were
    /// in, until you switch apps. So ask Accessibility which app has keyboard focus first
    /// (it can't answer for an Electron app before its tree is built), then the app that owns
    /// the menu bar, and only then the frontmost app. loginwindow is never the answer.
    @MainActor
    static func focusedApp() async -> NSRunningApplication? {
        let focusedPid = await Task.detached(priority: .userInitiated) { focusedApplicationPid() }.value
        let candidates = [
            focusedPid.flatMap { NSRunningApplication(processIdentifier: $0) },
            NSWorkspace.shared.menuBarOwningApplication,
            NSWorkspace.shared.frontmostApplication,
        ]
        return candidates.lazy.compactMap { $0 }.first(where: canOwnTextBox)
    }

    /// Not loginwindow, and not a background helper (a browser's web content process, say).
    private static func canOwnTextBox(_ app: NSRunningApplication) -> Bool {
        app.bundleIdentifier != "com.apple.loginwindow" && app.activationPolicy != .prohibited
    }

    /// Reads the text box you're in, in `app` (from `focusedApp()`).
    @MainActor
    static func capture(from app: NSRunningApplication?) async throws -> Captured {
        guard isTrusted else { throw CaptureError.notTrusted }
        guard var app else { throw CaptureError.nothingToCheck }
        // On the test display, only the app the run named: never the person's.
        if OnDisplay.isOn, app.processIdentifier != Driven.target {
            OnDisplay.trip("asked to read \(app.localizedName ?? "an app") (pid \(app.processIdentifier)), not the app the run named")
            throw CaptureError.nothingToCheck
        }

        let pid = app.processIdentifier
        let firstTime = !electronEnabled.contains(pid)
        electronEnabled.insert(pid)
        let pagesNeedAsking = firstTime && drawsPagesLikeChrome(app)
        // Accessibility calls are synchronous IPC into the other app; never make them on main.
        let read = await Task.detached(priority: .userInitiated) {
            readFocused(pid: pid, enableElectron: firstTime, askForPages: pagesNeedAsking)
        }.value
        if read.secure { throw CaptureError.secureField }
        let frame = anchorFrame(box: read.axFrame.map(cocoaFrame), selection: read.selectionFrame.map(cocoaFrame))
        // The box itself says which app it lives in. It can differ from `app` when the read
        // fell back to the system-wide focus, and it's the app a Replace must paste into.
        if OnDisplay.isOn, let owner = read.ownerPid, owner != pid {
            OnDisplay.trip("the text box read belongs to another app (pid \(owner))")
            throw CaptureError.nothingToCheck
        }
        if let owner = read.ownerPid, owner != pid, let ownerApp = NSRunningApplication(processIdentifier: owner),
            canOwnTextBox(ownerApp)
        {
            app = ownerApp
        }

        if let text = read.text {
            return Captured(app: app, element: read.element, text: text, scope: read.scope, fieldFrame: frame,
                            canReplace: read.editable)
        }
        // Copying borrows the person's clipboard and posts ⌘C to the app in front: never on the test display.
        if OnDisplay.isOn { throw CaptureError.nothingToCheck }
        // Fallback for apps that don't expose text: copy whatever is selected.
        // No element: this box can't be read again, which is what `stillHolds` and the word
        // watch go by. With one, Quick Fix compared the copied text with nothing and never pasted.
        if let copied = await copySelection(), !copied.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return Captured(app: app, element: nil, text: copied, scope: .selection, fieldFrame: frame,
                            canReplace: read.editable)
        }
        throw CaptureError.nothingToCheck
    }

    private nonisolated static func readFocused(pid: pid_t, enableElectron: Bool, askForPages: Bool) -> FocusRead {
        let appElement = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(appElement, axTimeout)
        guard enableElectron else { return readFocused(in: appElement) }
        if AXUIElementSetAttributeValue(appElement, "AXManualAccessibility" as CFString, kCFBooleanTrue) == .success {
            // Electron (Slack, Teams, VS Code...) builds its tree only when asked; give it a moment.
            Thread.sleep(forTimeInterval: 0.2)
        }
        var read = readFocused(in: appElement)
        // Chrome, Brave, Edge and Arc refuse that, and build a web page's tree only for a screen
        // reader: without it a page's text boxes don't exist for us at all, so a click in X's
        // post box read as nothing. Ask the way VoiceOver does, once, only when nothing was
        // read, and give the page up to 3 seconds to appear: a busy browser took longer than
        // 1.5 on the first press. Later presses read it at once. Only in those browsers
        // (`drawsPagesLikeChrome`): any other app would be switched into its screen reader
        // mode for nothing, and the press would wait three seconds to say "nothing to read".
        if askForPages, read.text == nil, !read.secure, askForScreenReaderTree(appElement) {
            for _ in 0..<20 {
                Thread.sleep(forTimeInterval: 0.15)
                read = readFocused(in: appElement)
                if read.text != nil || read.secure { break }
            }
        }
        return read
    }

    /// Turns on an app's screen reader support (AXEnhancedUserInterface) if it has it and it's
    /// off. True if it was turned on. Chromium answers "not implemented" but does it anyway, so
    /// the setting is read back rather than trusting the answer.
    private nonisolated static func askForScreenReaderTree(_ app: AXUIElement) -> Bool {
        let attribute = "AXEnhancedUserInterface" as CFString
        func isOn() -> Bool? {
            var value: CFTypeRef?
            guard AXUIElementCopyAttributeValue(app, attribute, &value) == .success else { return nil }
            return (value as? Bool) ?? false
        }
        guard isOn() == false else { return false }
        AXUIElementSetAttributeValue(app, attribute, kCFBooleanTrue)
        return isOn() == true
    }

    /// A browser built on Chromium (Chrome, Brave, Edge, Arc, Vivaldi, Opera), by its name or by
    /// the page renderer Chromium ships inside the app ("… Helper (Renderer).app").
    @MainActor
    private static func drawsPagesLikeChrome(_ app: NSRunningApplication) -> Bool {
        let known = ["com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac", "company.thebrowser.Browser",
                     "com.vivaldi.Vivaldi", "com.operasoftware.Opera", "org.chromium.Chromium"]
        if let id = app.bundleIdentifier, known.contains(where: { id == $0 || id.hasPrefix($0 + ".") }) { return true }
        guard let frameworks = app.bundleURL?.appendingPathComponent("Contents/Frameworks"),
            let entries = FileManager.default.enumerator(at: frameworks, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])
        else { return false }
        // Frameworks/<Name>.framework/Versions/<v>/Helpers/<Name> Helper (Renderer).app: five deep.
        while let url = entries.nextObject() as? URL {
            if url.lastPathComponent.hasSuffix("Helper (Renderer).app") { return true }
            if url.pathExtension == "app" || entries.level >= 5 { entries.skipDescendants() }
        }
        return false
    }

    private nonisolated static func readFocused(in appElement: AXUIElement) -> FocusRead {
        var read = FocusRead()
        guard let element = focusedElement(in: appElement) else { return read }
        AXUIElementSetMessagingTimeout(element, axTimeout)
        read.element = element
        var owner: pid_t = 0
        if AXUIElementGetPid(element, &owner) == .success, owner > 0 { read.ownerPid = owner }
        if isSecure(element) {
            read.secure = true
            return read
        }
        read.axFrame = rawFrame(element)
        read.selectionFrame = selectionBounds(element)
            ?? webArea(containing: element).flatMap(selectionBounds)
            ?? webAreas(under: element).lazy.compactMap(selectionBounds).first
        read.editable = isEditable(element)
        // Editors keep private markers in their text (Slack: one per bullet line); pasted back
        // they show as stray glyphs, so they never reach the model.
        let cleaned = { (attribute: String) in string(element, attribute).map(EditorText.removingEditorMarkers) }
        if let selected = cleaned(kAXSelectedTextAttribute), !selected.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            read.text = selected
            read.scope = .selection
        } else if let value = cleaned(kAXValueAttribute), !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
            value != cleaned("AXPlaceholderValue")
        {
            read.text = value
            read.scope = .wholeField
        }
        return read
    }

    // MARK: Replace

    /// The last replacement asked for. Each waits for the one before it, so two pastes never
    /// fight over the clipboard.
    @MainActor private static var lastReplace: Task<Void, Never>?

    /// What the captured field holds now, for noticing a word put back after a Replace.
    static func currentText(of captured: Captured) async -> String? {
        guard let element = captured.element else { return nil }
        return await Task.detached { string(element, kAXValueAttribute as String) }.value
    }

    /// Pastes `newText` over the captured text, then gives the user their clipboard back.
    /// `onPasted` runs the moment the paste is sent, before the clipboard is restored, so a
    /// confirmation appears with the new text rather than half a second after it.
    ///
    /// The temporary clipboard entry is marked transient so clipboard managers skip it, and
    /// the old clipboard is restored only if nothing else wrote to it in the meantime (if the
    /// user copied something during the paste, theirs wins).
    ///
    /// Returns false, with `newText` left on the clipboard to paste by hand, when the app the
    /// text came from didn't take the keyboard back (it quit, or something else came forward):
    /// select-all and paste must never land in another app.
    @MainActor
    @discardableResult
    static func replace(_ captured: Captured, with newText: String, onPasted: (() -> Void)? = nil) async -> Bool {
        let previous = lastReplace
        let this = Task { @MainActor in
            await previous?.value
            lateCopy?.cancel()  // a read still watching the clipboard would take this paste's entry for a late copy
            return await paste(newText, into: captured, onPasted: onPasted)
        }
        lastReplace = Task { _ = await this.value }
        return await this.value
    }

    /// Whether `app` has the keyboard, waiting up to half a second for it to come forward.
    /// Any one sign will do (see `focusedApp` for why the frontmost app alone can be wrong).
    @MainActor
    private static func hasKeyboard(_ app: NSRunningApplication) async -> Bool {
        let pid = app.processIdentifier
        for attempt in 0..<9 {
            if app.isTerminated { return false }
            if app.isActive || NSWorkspace.shared.menuBarOwningApplication?.processIdentifier == pid { return true }
            if await Task.detached(priority: .userInitiated, operation: { focusedApplicationPid() }).value == pid { return true }
            if attempt < 8 { try? await Task.sleep(for: .milliseconds(60)) }
        }
        return false
    }

    @MainActor
    private static func paste(_ newText: String, into captured: Captured, onPasted: (() -> Void)?) async -> Bool {
        if OnDisplay.isOn { return writeForVerify(newText, into: captured, onPasted: onPasted) }
        let saved = PasteboardSnapshot.take()
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let item = NSPasteboardItem()
        item.setString(newText, forType: .string)
        item.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
        item.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.AutoGeneratedType"))
        pasteboard.writeObjects([item])
        let ours = pasteboard.changeCount

        captured.app.activate()
        if captured.app.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            // Iris pasting into its own window: setup's practice box. The panel that had the
            // keyboard has just closed, so give it back to the window the box is in. Iris has
            // no Edit menu (it lives in the menu bar), and ⌘A ⌘V reach a text view through
            // that menu, so send select-all and paste to the box itself: the same undoable paste.
            NSApp.windows.first { $0.isVisible && !($0 is NSPanel) && $0.canBecomeKey }?.makeKeyAndOrderFront(nil)
            NotificationCenter.default.post(name: .irisPastedIntoIris, object: nil)
            try? await Task.sleep(for: .milliseconds(60))
            if captured.scope == .wholeField { NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: nil) }
            NSApp.sendAction(#selector(NSText.paste(_:)), to: nil, from: nil)
        } else {
            try? await Task.sleep(for: .milliseconds(120))
            guard await hasKeyboard(captured.app) else {
                // Not transient any more: it's the person's to paste now.
                copyToClipboard(newText)
                return false
            }
            if captured.scope == .wholeField {
                postShortcut(keyCode: CGKeyCode(kVK_ANSI_A), flags: .maskCommand)
                try? await Task.sleep(for: .milliseconds(60))
            }
            postShortcut(keyCode: CGKeyCode(kVK_ANSI_V), flags: .maskCommand)
        }
        onPasted?()

        // Give the target app time to read the pasteboard before restoring it.
        try? await Task.sleep(for: .milliseconds(450))
        if pasteboard.changeCount == ours { saved.restore() }
        return true
    }

    @MainActor
    static func copyToClipboard(_ text: String) {
        // The clipboard is the person's: on the test display, say what would be copied instead.
        if OnDisplay.isOn { return OnDisplay.say("copied \(text)") }
        // A read still watching for a late copy would take this for one and put the old clipboard back.
        lateCopy?.cancel()
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// On the test display, the last step of a paste, done without the keyboard or clipboard:
    /// the app in front is the person's, so ⌘A ⌘V can't be sent there. Writes the words into
    /// the box Iris read, through Accessibility, as the paste would leave it: the whole box
    /// replaced, or just the selection. Everything before this (the panel's Replace, Quick Fix's
    /// landing, `Paste`'s message after) is the real path.
    @MainActor private static func writeForVerify(_ newText: String, into captured: Captured, onPasted: (() -> Void)?) -> Bool {
        guard let element = captured.element, captured.app.processIdentifier == Driven.target else {
            OnDisplay.trip("a paste with no text box to write into")
            return false
        }
        let attribute = captured.scope == .wholeField ? kAXValueAttribute : kAXSelectedTextAttribute
        let result = AXUIElementSetAttributeValue(element, attribute as CFString, newText as CFString)
        guard result == .success else {
            OnDisplay.say("replace failed: AX error \(result.rawValue)")
            return false
        }
        OnDisplay.say("replaced \(captured.scope == .wholeField ? "box" : "selection")")
        onPasted?()
        return true
    }

    // MARK: Helpers

    /// Apps we've already asked to build their Electron accessibility tree.
    @MainActor private static var electronEnabled: Set<pid_t> = []

    /// The app with keyboard focus, as Accessibility sees it.
    private nonisolated static func focusedApplicationPid() -> pid_t? {
        let systemWide = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(systemWide, axTimeout)
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(systemWide, kAXFocusedApplicationAttribute as CFString, &value) == .success,
            let value, CFGetTypeID(value) == AXUIElementGetTypeID()
        else { return nil }
        var pid: pid_t = 0
        guard AXUIElementGetPid(value as! AXUIElement, &pid) == .success, pid > 0 else { return nil }
        return pid
    }

    private nonisolated static func focusedElement(in app: AXUIElement) -> AXUIElement? {
        var value: CFTypeRef?
        if AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &value) == .success,
            let value, CFGetTypeID(value) == AXUIElementGetTypeID()
        {
            return (value as! AXUIElement)
        }
        // What has focus system-wide is on the person's screen: never read it on the test display.
        if OnDisplay.isOn { return nil }
        let systemWide = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(systemWide, axTimeout)
        if AXUIElementCopyAttributeValue(systemWide, kAXFocusedUIElementAttribute as CFString, &value) == .success,
            let value, CFGetTypeID(value) == AXUIElementGetTypeID()
        {
            return (value as! AXUIElement)
        }
        return nil
    }

    private nonisolated static func string(_ element: AXUIElement, _ attribute: String) -> String? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else { return nil }
        return value as? String
    }

    private nonisolated static func isSecure(_ element: AXUIElement) -> Bool {
        let secure = kAXSecureTextFieldSubrole as String
        return string(element, kAXRoleAttribute) == secure || string(element, kAXSubroleAttribute) == secure
    }

    private nonisolated static func rawFrame(_ element: AXUIElement) -> CGRect? {
        var positionRef: CFTypeRef?
        var sizeRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXPositionAttribute as CFString, &positionRef) == .success,
            AXUIElementCopyAttributeValue(element, kAXSizeAttribute as CFString, &sizeRef) == .success,
            let positionRef, let sizeRef
        else { return nil }
        var origin = CGPoint.zero
        var size = CGSize.zero
        AXValueGetValue(positionRef as! AXValue, .cgPoint, &origin)
        AXValueGetValue(sizeRef as! AXValue, .cgSize, &size)
        guard size.width > 0, size.height > 0 else { return nil }
        return CGRect(origin: origin, size: size)
    }

    /// Whether a paste here would land. Only says no when it's sure: a text field or area,
    /// a value the app lets us set, or anything inside an editable web box (Slack's composer,
    /// a contenteditable) all count as editable, and an app that says nothing keeps Replace.
    private nonisolated static func isEditable(_ element: AXUIElement) -> Bool {
        let role = string(element, kAXRoleAttribute) ?? ""
        if [kAXTextFieldRole as String, kAXTextAreaRole as String, kAXComboBoxRole as String].contains(role) { return true }
        var settable: DarwinBoolean = false
        if AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &settable) == .success, settable.boolValue {
            return true
        }
        var ancestor: CFTypeRef?
        if AXUIElementCopyAttributeValue(element, "AXEditableAncestor" as CFString, &ancestor) == .success, ancestor != nil {
            return true
        }
        // Read-only for certain: a web page or a run of plain text, with none of the above.
        return !["AXWebArea", kAXStaticTextRole as String, kAXGroupRole as String, kAXScrollAreaRole as String,
                 kAXListRole as String, kAXRowRole as String, kAXCellRole as String].contains(role)
    }

    /// The web page an element sits in. Chrome and Electron apps report a click in a page as a
    /// small group inside it, and only the page knows where the selection is.
    private nonisolated static func webArea(containing element: AXUIElement) -> AXUIElement? {
        var current = element
        for _ in 0..<12 {
            var parent: CFTypeRef?
            guard AXUIElementCopyAttributeValue(current, kAXParentAttribute as CFString, &parent) == .success,
                let parent, CFGetTypeID(parent) == AXUIElementGetTypeID()
            else { return nil }
            current = parent as! AXUIElement
            if string(current, kAXRoleAttribute) == "AXWebArea" { return current }
        }
        return nil
    }

    /// Web pages inside an element, nearest first, for apps that report focus on a wrapper
    /// around the page. A short, bounded search: it gives up after 400 elements.
    private nonisolated static func webAreas(under element: AXUIElement) -> [AXUIElement] {
        var queue = [element], found: [AXUIElement] = [], seen = 0
        while !queue.isEmpty, seen < 400, found.count < 4 {
            let current = queue.removeFirst()
            seen += 1
            if string(current, kAXRoleAttribute) == "AXWebArea" { found.append(current); continue }
            var children: CFTypeRef?
            if AXUIElementCopyAttributeValue(current, kAXChildrenAttribute as CFString, &children) == .success,
                let children = children as? [AXUIElement]
            {
                queue.append(contentsOf: children)
            }
        }
        return found
    }

    /// Where the selected words are (or the caret, if nothing is selected). Text views answer
    /// with a character range; web views (Chrome, Safari, Electron apps) with text markers.
    private nonisolated static func selectionBounds(_ element: AXUIElement) -> CGRect? {
        for (range, bounds) in [(kAXSelectedTextRangeAttribute as String, kAXBoundsForRangeParameterizedAttribute as String),
                                ("AXSelectedTextMarkerRange", "AXBoundsForTextMarkerRange")] {
            var rangeRef: CFTypeRef?
            var boundsRef: CFTypeRef?
            guard AXUIElementCopyAttributeValue(element, range as CFString, &rangeRef) == .success, let rangeRef,
                AXUIElementCopyParameterizedAttributeValue(element, bounds as CFString, rangeRef, &boundsRef) == .success,
                let boundsRef, CFGetTypeID(boundsRef) == AXValueGetTypeID()
            else { continue }
            var rect = CGRect.zero
            guard AXValueGetValue(boundsRef as! AXValue, .cgRect, &rect), rect.height > 0 else { continue }
            rect.size.width = max(rect.width, 1)  // a caret has no width
            return rect
        }
        return nil
    }

    /// What the bud stands on: the text box if it's a place to stand (`PanelLayout.canStand`),
    /// else the selected words. Nil if neither is.
    @MainActor
    private static func anchorFrame(box: CGRect?, selection: CGRect?) -> CGRect? {
        func fits(_ rect: CGRect) -> Bool {
            let screen = NSScreen.screens.first(where: { $0.frame.intersects(rect) }) ?? NSScreen.main
            guard let visible = screen?.visibleFrame else { return true }
            return PanelLayout.canStand(on: rect, visible: visible)
        }
        if let box, fits(box) { return box }
        if let selection, fits(selection) { return selection }
        return nil
    }

    /// AX uses a top-left origin on the primary screen; Cocoa uses bottom-left.
    @MainActor
    private static func cocoaFrame(_ ax: CGRect) -> CGRect {
        let primaryHeight = NSScreen.screens.first?.frame.height ?? 0
        return CGRect(x: ax.minX, y: primaryHeight - ax.minY - ax.height, width: ax.width, height: ax.height)
    }

    @MainActor
    private static func copySelection() async -> String? {
        // A Replace may still be borrowing the clipboard: wait until it has given it back.
        await lastReplace?.value
        // An earlier read still watching for a late copy stops: this read watches now.
        lateCopy?.cancel()
        // The clipboard is only touched if the app copies something: an empty box, or an app
        // that ignores ⌘C, leaves it exactly as it was.
        let saved = PasteboardSnapshot.take()
        let pasteboard = NSPasteboard.general
        let before = pasteboard.changeCount
        postShortcut(keyCode: CGKeyCode(kVK_ANSI_C), flags: .maskCommand)
        var copied: String?
        for _ in 0..<15 {
            try? await Task.sleep(for: .milliseconds(20))
            if pasteboard.changeCount != before {
                copied = pasteboard.string(forType: .string).map(EditorText.removingEditorMarkers)
                saved.restore()
                break
            }
        }
        if copied == nil {
            // An app that answers ⌘C after Iris stopped waiting would leave the selection on
            // the clipboard: keep looking for a second more, and put the clipboard back if it does.
            lateCopy = Task { @MainActor in
                for _ in 0..<50 {
                    try? await Task.sleep(for: .milliseconds(20))
                    // Stopped by the next read or a paste, which look after the clipboard themselves.
                    guard !Task.isCancelled else { return }
                    if pasteboard.changeCount != before { return saved.restore() }
                }
            }
        }
        return copied
    }

    /// A read that gave up on ⌘C but still watches, for a second, for the app to answer late
    /// (see `copySelection`). The next read or paste cancels it.
    @MainActor private static var lateCopy: Task<Void, Never>?

    private static func postShortcut(keyCode: CGKeyCode, flags: CGEventFlags) {
        let source = CGEventSource(stateID: .combinedSessionState)
        let down = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: true)
        let up = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: false)
        down?.flags = flags
        up?.flags = flags
        down?.post(tap: .cghidEventTap)
        up?.post(tap: .cghidEventTap)
    }
}

extension NSRunningApplication {
    /// The name to show people, or nil when it would mean nothing to them: system processes
    /// (loginwindow, Dock) and helpers without a Dock icon of their own. Where there's no
    /// name, say "this app" or leave the app out.
    var nameForPeople: String? {
        guard activationPolicy == .regular, let name = localizedName, !name.isEmpty else { return nil }
        return name
    }
}

/// Every item and type on the general pasteboard, so it can be put back exactly.
struct PasteboardSnapshot {
    private let items: [[NSPasteboard.PasteboardType: Data]]

    static func take() -> PasteboardSnapshot {
        let items = NSPasteboard.general.pasteboardItems ?? []
        return PasteboardSnapshot(items: items.map { item in
            var entry: [NSPasteboard.PasteboardType: Data] = [:]
            for type in item.types { if let data = item.data(forType: type) { entry[type] = data } }
            return entry
        })
    }

    func restore() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard !items.isEmpty else { return }
        let restored = items.map { entry -> NSPasteboardItem in
            let item = NSPasteboardItem()
            for (type, data) in entry { item.setData(data, forType: type) }
            return item
        }
        pasteboard.writeObjects(restored)
    }
}

extension Notification.Name {
    /// Iris replaced text in one of its own windows (setup's practice box).
    static let irisPastedIntoIris = Notification.Name("irisPastedIntoIris")
}
