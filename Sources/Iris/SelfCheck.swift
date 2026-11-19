import AppKit
import Carbon.HIToolbox
import IrisCore
import SwiftUI

/// `Iris --self-check`: checks the panel and the menus in the real app, with real key presses
/// and clicks, so a change can be tested without anyone at the Mac. Run it through
/// `scripts/self-check.sh`, which waits until the Mac is idle, quits the installed Iris (they
/// would fight over the shortcut), runs this and opens the installed Iris again.
///
/// It needs Accessibility, like Iris itself: it posts keys and clicks, and reads the practice
/// box the way it reads any text box. It never loads the model (the panel just waits), never
/// pastes, and only touches its own windows and the menu bar flower. It moves the pointer.
///
/// Each check fails the way the bug it guards against would:
/// - fold: the shortcut pressed while the panel folds away opens a new panel, and the old
///   fold doesn't hide it.
/// - paste guard: a box Iris read counts as unchanged until something is typed in it; after
///   that, a Quick Fix that finished late would leave it alone instead of pasting over it.
/// - slip page: the slip closed on a page opens on its commands, not on that page.
/// - slip / picker: a click or a long click on the menu bar flower or a picker closes its
///   open menu without opening it again; the next click opens it.
/// - settings: asking for Settings at a page opens it and turns the page once, not forever;
///   it opens at its full size, on a screen, and doesn't move or grow after it shows.
/// - rail: a click on each rail row turns Settings to that page (Updates and About included),
///   and every row, the last too, is inside the window where a person can click it.
/// - rail foot: Report a bug sits at the rail's foot inside the window, under the last page
///   row and clear of it, so it can be clicked (it isn't: it would open the browser).
/// - page feet, lettering, other versions, the update check's answer: each says
///   above its check (`pageFeet`, `lettering`, `otherVersions`, `updateAnswer`)
///   what it guards.
@MainActor
enum SelfCheck {
    struct Parts {
        let panel: PanelController
        let statusMenu: StatusMenu
        let statusButton: NSStatusBarButton?
        let shortcut: KeyCombo
        let shortcutRegistered: Bool
    }

    /// A line about what Iris did, printed only while checking, to tell why a check failed.
    static func note(_ line: @autoclosure () -> String) {
        if LaunchMode.current == .selfCheck { print("  · \(line())") }
    }

    private static var failures = 0
    private static var practice: Process?

    static func run(_ parts: Parts) {
        BudShow.forced = false  // the quick arrival, and no change to the real arrival count
        Task {
            await checks(parts)
            practice?.terminate()
            print(failures == 0 ? "self-check: all passed" : "self-check: \(failures) failed")
            exit(failures == 0 ? 0 : 1)
        }
    }

    private static func checks(_ parts: Parts) async {
        guard TextBridge.isTrusted else {
            return fail("setup", "this copy of Iris isn't allowed in Privacy & Security → Accessibility, so it can't press keys or read the practice box")
        }
        let start = NSEvent.mouseLocation
        defer { move(to: start) }
        await fold(parts)
        await pasteGuard()
        await slip(parts)
        await picker()
        await settingsAtAPage()
        await railPages()
        await pageFeet()
        await lettering()
        await otherVersions()
        await updateAnswer()
    }

    // MARK: Checks

    /// The bug: asking for Settings at a page (setup's Change shortcut) posted the same
    /// notification the app listens for, so Settings opened itself again and again, forever.
    private static func settingsAtAPage() async {
        var turns = 0
        let watch = NotificationCenter.default.addObserver(forName: .irisTurnSettingsPage, object: nil, queue: .main) { _ in
            turns += 1
        }
        defer { NotificationCenter.default.removeObserver(watch) }
        // Where Settings should open: where it was left last time, or the middle of the screen.
        let saved = UserDefaults.standard.string(forKey: "windowTopLeft.settings").map(NSPointFromString)
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.shortcuts.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }) else { return fail("settings", "asking for Settings at a page didn't open it") }
        let opened = settings()?.frame ?? .zero
        try? await Task.sleep(for: .seconds(1))
        let settled = settings()?.frame ?? .zero
        let page = settings()?.contentLayoutRect.height ?? 0
        settings()?.close()
        note("Settings opened at \(opened), one second later \(settled), its page \(page) tall")
        // The bug: Settings stood 848 points tall, most of a laptop's screen. Its height is
        // fixed and a long page scrolls inside it; nothing on a page may push the window taller.
        guard abs(page - SettingsView.size.height) < 1 else {
            return fail("settings", "Settings' page is \(page) tall, not its fixed \(SettingsView.size.height) (a page scrolls inside the window, the window doesn't grow)")
        }
        // The bug: the window was placed before its page laid out, then grew from its top-left
        // corner, so it first opened low and off to the right of where it was put.
        guard opened == settled else {
            return fail("settings", "Settings moved or changed size after it opened (it was placed before it took its size)")
        }
        guard NSScreen.screens.contains(where: { $0.visibleFrame.insetBy(dx: -2, dy: -2).contains(opened) }) else {
            return fail("settings", "Settings opened partly off the screen at \(opened)")
        }
        if let saved {
            guard abs(opened.minX - saved.x) < 2, abs(opened.maxY - saved.y) < 2 else {
                return fail("settings", "Settings opened at \(opened.origin), not where it was left (top left \(saved))")
            }
        } else if let screen = NSScreen.main?.visibleFrame, abs(opened.midX - screen.midX) > 2 {
            return fail("settings", "Settings opened off centre (\(opened.midX) on a screen centred at \(screen.midX))")
        }
        guard turns == 1 else {
            return fail("settings", "asking for Settings once turned its page \(turns) times (it kept reopening itself)")
        }
        pass("settings", "asking for Settings at a page opens it on screen, settled, and turns to that page once")
    }

    /// The bug this guards: a page added to the rail that a click doesn't open (a row with no
    /// page, or one pushed out of the window as the rail grows). Clicks each row as a person
    /// would, then reads the window to see that page's first section.
    private static func railPages() async {
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.general.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }), let window = settings() else {
            return fail("rail", "Settings didn't open")
        }
        defer { window.close() }
        // Click its title bar first, as a person would, so the window has the keyboard.
        await click(CGPoint(x: window.frame.midX, y: window.frame.maxY - 12))
        try? await Task.sleep(for: .milliseconds(300))
        // Each page and a line only that page shows at its top (Updates: its heading's button).
        let pages: [(SettingsView.Page, String)] = [(.updates, "Check for updates"), (.about, "Where Iris comes from"), (.general, "The panel")]
        for (page, mark) in pages {
            guard let row = AXFind.element(in: window, role: kAXButtonRole, named: page.title),
                let frame = AXFind.frame(of: row)
            else { return fail("rail", "the rail has no \(page.title) row") }
            guard window.frame.insetBy(dx: 2, dy: 2).contains(frame) else {
                return fail("rail", "the \(page.title) row (\(frame)) is outside the window (\(window.frame)), so it can't be clicked")
            }
            // Scrolled down first, as after reading the page before: the next one should open at its top.
            scroll(at: CGPoint(x: window.frame.maxX - 200, y: window.frame.midY), lines: -40)
            try? await Task.sleep(for: .milliseconds(300))
            await click(CGPoint(x: frame.midX, y: frame.midY))
            let turned = await until(2) { AXFind.element(in: window, role: nil, named: mark) != nil }
            note("clicked \(page.title) at \(frame); found \"\(mark)\": \(turned)")
            guard turned else { return fail("rail", "a click on \(page.title) didn't turn Settings to it (no \"\(mark)\")") }
            let markFrame = AXFind.element(in: window, role: nil, named: mark).flatMap(AXFind.frame(of:))
            note("\"\(mark)\" at \(markFrame.map { "\($0)" } ?? "nowhere") in a window at \(window.frame)")
            guard let markFrame, window.frame.contains(markFrame) else {
                return fail("rail", "\(page.title) opened scrolled part way down: its first section is out of view")
            }
        }
        // Report a bug stands at the rail's foot: in the window, below the last page's row.
        guard let report = AXFind.element(in: window, role: kAXButtonRole, named: "Report a bug").flatMap(AXFind.frame(of:)),
            let last = AXFind.element(in: window, role: kAXButtonRole, named: SettingsView.Page.allCases.last!.title).flatMap(AXFind.frame(of:))
        else { return fail("rail", "the rail has no Report a bug row") }
        note("Report a bug at \(report), the last page row at \(last), in a window at \(window.frame)")
        guard window.frame.insetBy(dx: 2, dy: 2).contains(report) else {
            return fail("rail", "Report a bug (\(report)) is outside the window (\(window.frame)), so it can't be clicked")
        }
        // Screen coordinates count up: below means a smaller y.
        guard report.maxY <= last.minY else {
            return fail("rail", "Report a bug (\(report)) overlaps the \(SettingsView.Page.allCases.last!.title) row (\(last))")
        }
        pass("rail", "a click on Updates, About and General turns Settings to each, at its top, every row is in the window, and Report a bug sits under them")
    }

    /// The bug this guards: on a page shorter than the window (Words with one entry) the pink
    /// foot hung under the last section with bare paper below it. Turns to every page, scrolls
    /// to its end and checks the foot's bottom edge is the window's.
    private static func pageFeet() async {
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.general.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }), let window = settings(), let content = window.contentView else {
            return fail("foot", "Settings didn't open")
        }
        defer { window.close() }
        await click(CGPoint(x: window.frame.midX, y: window.frame.maxY - 12))
        try? await Task.sleep(for: .milliseconds(300))
        for page in SettingsView.Page.allCases {
            guard let row = AXFind.element(in: window, role: kAXButtonRole, named: page.title),
                let frame = AXFind.frame(of: row)
            else { return fail("foot", "the rail has no \(page.title) row") }
            await click(CGPoint(x: frame.midX, y: frame.midY))
            try? await Task.sleep(for: .milliseconds(400))
            for _ in 0..<4 {
                scroll(at: CGPoint(x: window.frame.maxX - 200, y: window.frame.midY), lines: -60)
                try? await Task.sleep(for: .milliseconds(150))
            }
            try? await Task.sleep(for: .milliseconds(500))
            let foot = SettingsView.footFrame, bottom = content.bounds.height
            note("\(page.title): foot \(foot), window content \(bottom) tall")
            guard abs(foot.maxY - bottom) < 2 else {
                return fail("foot", "on \(page.title) the page's foot ends at \(foot.maxY), not at the window's bottom (\(bottom))")
            }
        }
        pass("foot", "on every Settings page, scrolled to its end, the pink foot sits on the window's bottom edge")
    }

    /// The bug this guards: each Settings page's ghost word and hand word stood by their right
    /// edges, so a short ghost (KEYS, APPS) printed far narrower than the rest, the hand word
    /// hung off its middle by up to 73 points, and the corner jumped from page to page. Turns
    /// to every page and checks the ghost prints within `SettingsView.ghostWidths` and the two
    /// words share a centre line.
    private static func lettering() async {
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.general.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }), let window = settings() else {
            return fail("lettering", "Settings didn't open")
        }
        defer { window.close() }
        var centres: [CGFloat] = []
        for page in SettingsView.Page.allCases {
            NotificationCenter.default.post(name: .irisTurnSettingsPage, object: page.rawValue)
            try? await Task.sleep(for: .milliseconds(400))
            let ghost = SettingsView.ghostFrame, word = SettingsView.handWordFrame
            note("\(page.title): ghost \(page.lettering.ghost) \(ghost), word \(page.lettering.word) \(word)")
            guard SettingsView.ghostWidths.contains(ghost.width) else {
                return fail("lettering", "on \(page.title) the ghost word \(page.lettering.ghost) prints \(Int(ghost.width)) wide, outside \(SettingsView.ghostWidths)")
            }
            guard abs(ghost.midX - word.midX) < 2 else {
                return fail("lettering", "on \(page.title) the hand word's centre is \(Int(word.midX - ghost.midX)) points off the ghost's")
            }
            centres.append(ghost.midX)
        }
        guard let lo = centres.min(), let hi = centres.max(), hi - lo < 2 else {
            return fail("lettering", "the lettering's centre moves from page to page: \(centres)")
        }
        pass("lettering", "on every Settings page the ghost word prints as wide as the others and the hand word sits on its middle, on one line")
    }

    /// The bug this guards: after the letter from GitHub opened, the heading's button stayed a
    /// greyed Check for updates for good, so nobody could look again. Asks twice with made-up
    /// answers (never GitHub): up to date must bring Check again, pressable, and the heading
    /// must say so; a look that fails must then bring Try again.
    private static func updateAnswer() async {
        let real = UpdateCheck.shared.start
        defer { UpdateCheck.shared.start = real }
        UpdateCheck.shared.start = UpdateCheck.madeUpChecks([.upToDate, .failed("Couldn't reach GitHub. Check your connection and try again.")])
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.updates.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }), let window = settings() else {
            return fail("update answer", "Settings didn't open")
        }
        defer { window.close() }
        try? await Task.sleep(for: .milliseconds(600))
        for (asked, answer, heading) in [("Check for updates", "Check again", "Up to date"), ("Check again", "Try again", "Couldn't check")] {
            guard let button = AXFind.element(in: window, role: kAXButtonRole, named: asked), let frame = AXFind.frame(of: button) else {
                return fail("update answer", "Updates has no \(asked) button")
            }
            let clicked = Date()
            await click(CGPoint(x: frame.midX, y: frame.midY))
            // The loop plays at least 1.4 s, the letter opens 1.25 s after the answer.
            let ready = await until(5) {
                AXFind.element(in: window, role: kAXButtonRole, named: answer).map(AXFind.isEnabled) ?? false
            }
            let took = Date().timeIntervalSince(clicked)
            let said = AXFind.element(in: window, role: nil, named: heading) != nil
            note("pressed \(asked): \(answer) \(ready ? "pressable" : "missing") after \(String(format: "%.1f", took)) s, heading says \(heading): \(said)")
            guard ready else {
                return fail("update answer", "after the answer to \(asked) the heading's button never became \(answer) (it stayed a greyed Check for updates)")
            }
            guard said else { return fail("update answer", "the heading doesn't say \(heading) once the answer is in") }
            try? await Task.sleep(for: .milliseconds(500))
        }
        pass("update answer", "Check for updates brings Check again once the letter opens, and a failed look brings Try again")
    }

    /// What's new: a real click on another version's small cover prints its changes in place
    /// of the newest's, and the newest joins the other covers, to click back to. The covers sit
    /// under the newest version's changes: scrolled into view first, as a person would.
    private static func otherVersions() async {
        let issues = Changelog.bundled.issues
        guard issues.count >= 2 else { return fail("versions", "the changelog has fewer than two versions") }
        NotificationCenter.default.post(name: .irisOpenSettings, object: SettingsView.Page.general.rawValue)
        let settings = { NSApp.windows.first { $0.title == "Iris Settings" && $0.isVisible } }
        guard await until(2, { settings() != nil }), let window = settings() else {
            return fail("versions", "Settings didn't open")
        }
        defer { window.close() }
        await click(CGPoint(x: window.frame.midX, y: window.frame.maxY - 12))
        try? await Task.sleep(for: .milliseconds(300))
        // Turned to Updates from the rail, as a person would, and only then scrolled: a scroll
        // sent while the page was still turning went to the page before it.
        guard let row = AXFind.element(in: window, role: kAXButtonRole, named: SettingsView.Page.updates.title).flatMap(AXFind.frame(of:))
        else { return fail("versions", "the rail has no Updates row") }
        await click(CGPoint(x: row.midX, y: row.midY))
        guard await until(2, { AXFind.element(in: window, role: nil, named: "Your copy") != nil }) else {
            return fail("versions", "a click on Updates didn't turn Settings to it")
        }
        try? await Task.sleep(for: .milliseconds(400))
        // A cover's button reads its version and title, and says so when it's the one you have.
        let cover = { (issue: Changelog.Issue) -> CGRect? in
            let name = "Version \(issue.version), \(issue.title)"
            return (AXFind.element(in: window, role: kAXButtonRole, named: name)
                ?? AXFind.element(in: window, role: kAXButtonRole, named: name + ", the one you have")).flatMap(AXFind.frame(of:))
        }
        guard cover(issues[0]) == nil else { return fail("versions", "the newest version (\(issues[0].version)) is a small cover at rest, not the one printed") }
        // They sit at the page's end, just over its foot: scrolled all the way down, as pageFeet does.
        let before = cover(issues[1]), footBefore = SettingsView.footFrame
        for _ in 0..<4 {
            scroll(at: CGPoint(x: window.frame.maxX - 200, y: window.frame.midY), lines: -60)
            try? await Task.sleep(for: .milliseconds(150))
        }
        try? await Task.sleep(for: .milliseconds(500))
        let frame = cover(issues[1])
        note("cover of \(issues[1].version) at \(before.map { "\($0)" } ?? "nowhere"), scrolled to the end \(frame.map { "\($0)" } ?? "nowhere"), in a window at \(window.frame); the foot went from \(footBefore) to \(SettingsView.footFrame)")
        guard let frame, window.frame.insetBy(dx: 2, dy: 2).contains(frame) else {
            return fail("versions", "the cover of \(issues[1].version) isn't in the window, even scrolled down")
        }
        await click(CGPoint(x: frame.midX, y: frame.midY))
        guard await until(2, { cover(issues[1]) == nil && cover(issues[0]) != nil }) else {
            return fail("versions", "a click on \(issues[1].version) didn't print its changes (its cover is still there, or \(issues[0].version)'s didn't join the others)")
        }
        try? await Task.sleep(for: .milliseconds(500))
        guard let back = cover(issues[0]) else { return fail("versions", "the newest version's cover went away") }
        await click(CGPoint(x: back.midX, y: back.midY))
        guard await until(2, { cover(issues[0]) == nil && cover(issues[1]) != nil }) else {
            return fail("versions", "a click on the newest version's cover didn't bring it back")
        }
        pass("versions", "a click on another version's cover prints its changes, and a click on the newest's brings it back")
    }

    /// A trackpad-like scroll in quick small steps, as one gesture.
    /// Starts as a trackpad swipe does: fingers landing (no movement), a pixel of drift
    /// sideways, then the move, then fingers lifting. A shelf that decided on the first event
    /// gave all of this to the page.
    /// `sideways` positive swipes right (towards older books), `down` positive scrolls down.
    private static func swipe(at point: CGPoint, sideways: Int32 = 0, down: Int32 = 0) async {
        move(to: point)
        let steps: Int32 = 10
        // kCGScrollWheelEventScrollPhase: began 1, changed 2, ended 4.
        let phase = CGEventField(rawValue: 99)!
        func post(_ vertical: Int32, _ sideways: Int32, _ stage: Int64) {
            let event = CGEvent(scrollWheelEvent2Source: source, units: .pixel, wheelCount: 2, wheel1: vertical, wheel2: sideways, wheel3: 0)
            event?.setIntegerValueField(phase, value: stage)
            event?.location = flipped(point)
            event?.post(tap: .cghidEventTap)
        }
        post(0, 0, 1)
        try? await Task.sleep(for: .milliseconds(15))
        // A pixel of drift the other way first.
        post(sideways == 0 ? 0 : 1, sideways == 0 ? 1 : 0, 2)
        for _ in 0..<steps {
            try? await Task.sleep(for: .milliseconds(15))
            post(-down / steps, sideways / steps, 2)
        }
        try? await Task.sleep(for: .milliseconds(15))
        post(0, 0, 4)
        try? await Task.sleep(for: .milliseconds(700))
    }

    private static func fold(_ parts: Parts) async {
        guard parts.shortcutRegistered else {
            return fail("fold", "the shortcut \(parts.shortcut.display) is taken: quit the other Iris first")
        }
        guard await openPracticeBox() else { return fail("fold", "the practice box didn't open") }
        let panel = parts.panel
        press(parts.shortcut)
        guard await until(3, { panel.life.stage == .unfolded }) else {
            return fail("fold", "the shortcut didn't open the panel (stage \(panel.life.stage))")
        }
        press(CGKeyCode(kVK_Escape))
        guard await until(0.1, { panel.life.stage == .folding }) else {
            return fail("fold", "Esc didn't start folding the panel (stage \(panel.life.stage))")
        }
        press(parts.shortcut)
        guard await until(1, { panel.isShowing }) else {
            return fail("fold", "the shortcut pressed while the panel folded away didn't open a new one")
        }
        // Well past when the old fold would have ended: it must not take the new panel off screen.
        try? await Task.sleep(for: .seconds(PanelMotion.foldDuration * 2))
        guard panel.isShowing else { return fail("fold", "the new panel closed by itself (stage \(panel.life.stage))") }
        pass("fold", "the shortcut pressed while the panel folds away opens a new one")
        press(CGKeyCode(kVK_Escape))
        _ = await until(1, { !panel.isShowing })
    }

    /// The bug: a Quick Fix that waited minutes for the model pasted (select all, paste) into
    /// whatever box had focus by then. Iris now pastes only while the box still holds the text
    /// it read; this checks that reading on a real text box in another process.
    private static func pasteGuard() async {
        guard await openPracticeBox() else { return fail("paste guard", "the practice box didn't open") }
        let captured: TextBridge.Captured
        do { captured = try await TextBridge.capture(from: TextBridge.focusedApp()) } catch {
            return fail("paste guard", "couldn't read the practice box: \(error)")
        }
        note("read \(captured.text.count) characters from \(captured.app.localizedName ?? "?")")
        let pressed = Date()
        guard await TextBridge.stillHolds(captured, since: pressed) else {
            return fail("paste guard", "the box Iris had just read didn't count as unchanged, so Quick Fix would never paste")
        }
        press(CGKeyCode(kVK_ANSI_X))
        try? await Task.sleep(for: .milliseconds(250))
        let after = await TextBridge.stillHolds(captured, since: pressed)
        press(CGKeyCode(kVK_Delete))
        if after {
            return fail("paste guard", "after typing in the box, Iris still thought it held the text it read, so a late Quick Fix would paste over it")
        }
        pass("paste guard", "a box that changed after Iris read it is left alone")
    }

    private static func slip(_ parts: Parts) async {
        let menu = parts.statusMenu
        guard let button = parts.statusButton, let window = button.window,
            let point = onScreen(window.convertToScreen(button.convert(button.bounds, to: nil)))
        else {
            return fail("slip", "the menu bar flower isn't on screen (hidden by the notch or a menu bar manager?)")
        }
        note("flower at \(point) on a screen \(NSScreen.screens.first?.frame ?? .zero), menu bar shown \(NSMenu.menuBarVisible())")
        if await toggles("slip", "the flower", "the slip", at: point, isOpen: { menu.isOpen }) {
            press(CGKeyCode(kVK_Escape))
            _ = await until(1, { !menu.isOpen })
        }
        if menu.isOpen { menu.close() }
        await slipPage(menu, flower: point)
    }

    /// The bug: the slip turned to a page (an update check's answer) stayed on it after it was
    /// closed, so the next open showed that old answer instead of the commands. An update that
    /// is building isn't a page and must still be there.
    private static func slipPage(_ menu: StatusMenu, flower point: CGPoint) async {
        defer { menu.model.showing = .commands }
        for (what, showing) in [("a page", StatusMenu.Showing.page(.init(id: UUID(), title: "Updates", lead: "Self-check"))),
                                ("an update building", .updating(0.3))] {
            // Clear of the click that last closed it, which a new click straight after belongs to.
            try? await Task.sleep(for: .milliseconds(500))
            await click(point)
            guard await until(1.5, { menu.isOpen }) else { return fail("slip page", "a click on the flower didn't open the slip") }
            menu.model.showing = showing
            await click(point)
            guard await until(1.5, { !menu.isOpen }) else { return fail("slip page", "a click on the flower didn't close the slip on \(what)") }
            // Past the slip's fade, when it turns back.
            try? await Task.sleep(for: .milliseconds(500))
            await click(point)
            guard await until(1.5, { menu.isOpen }) else { return fail("slip page", "a click on the flower didn't open the slip again") }
            let page = menu.model.page, updating = menu.model.updating
            await click(point)
            _ = await until(1.5, { !menu.isOpen })
            note("reopened after \(what): page \(page?.title ?? "none"), updating \(updating.map { "\($0)" } ?? "no")")
            if what == "a page", page != nil {
                return fail("slip page", "closed on a page, the slip opened on that page again instead of its commands")
            }
            if what == "an update building", updating == nil {
                return fail("slip page", "closed while an update was building, the slip opened without it")
            }
        }
        pass("slip page", "a page closes back to the commands; an update building is still there when the slip opens")
    }

    /// A trigger and its menu, as people use them: a click opens it, a click closes it (and it
    /// stays closed), a long click closes it (and it stays closed), the next click opens it.
    private static func toggles(_ check: String, _ trigger: String, _ menu: String, at point: CGPoint,
                                isOpen: () -> Bool) async -> Bool {
        await click(point)
        guard await until(1.5, isOpen) else { fail(check, "a click on \(trigger) didn't open \(menu)"); return false }
        await click(point)
        try? await Task.sleep(for: .milliseconds(400))
        guard !isOpen() else { fail(check, "a click on \(trigger) closed \(menu) and opened it again"); return false }
        await click(point)
        guard await until(1.5, isOpen) else { fail(check, "a click on \(trigger) didn't open \(menu) again"); return false }
        await click(point, hold: 1.5)
        try? await Task.sleep(for: .milliseconds(400))
        guard !isOpen() else { fail(check, "a long click on \(trigger) closed \(menu) and opened it again"); return false }
        await click(point)
        guard await until(1.5, isOpen) else { fail(check, "the click after a long click didn't open \(menu)"); return false }
        pass(check, "a click or a long click on \(trigger) closes \(menu), and the next click opens it")
        return true
    }

    private static func picker() async {
        let window = NSWindow(contentRect: pickerFrame, styleMask: [.titled], backing: .buffered, defer: false)
        window.title = "Iris self-check"
        window.contentView = NSHostingView(rootView: PickerStand())
        window.isReleasedWhenClosed = false
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }
        // A first click on a window in the background only brings it forward, as it would for
        // a person: click its title bar, then the picker.
        await click(CGPoint(x: window.frame.midX, y: window.frame.maxY - 12))
        try? await Task.sleep(for: .milliseconds(300))
        let content = window.contentRect(forFrameRect: window.frame)
        let point = CGPoint(x: content.midX, y: content.midY)
        let menu = MenuPanel.shared
        _ = await toggles("picker", "a picker", "its menu", at: point, isOpen: { menu.isOpen })
        menu.close()
    }

    /// One picker on its own, centred in its window.
    private struct PickerStand: View {
        @State private var choice = 1
        var body: some View {
            Dropdown(options: [(value: 1, label: "One"), (value: 2, label: "Two"), (value: 3, label: "Three")], selection: $choice)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(DS.Surface.overlay)
        }
    }

    // MARK: The practice box

    /// Opens the practice box (a second Iris, `--practice-box`, standing in for the app you
    /// type in) and clicks into its text, so it has the keyboard like Slack would.
    private static func openPracticeBox() async -> Bool {
        guard let executable = Bundle.main.executableURL else { return false }
        let process = Process()
        process.executableURL = executable
        process.arguments = [LaunchMode.practiceBoxFlag]
        practice?.terminate()  // one practice box at a time; the checks each open their own
        do { try process.run() } catch { return false }
        practice = process
        let pid = process.processIdentifier
        guard await until(5, { NSRunningApplication(processIdentifier: pid)?.isFinishedLaunching == true }) else { return false }
        try? await Task.sleep(for: .milliseconds(500))
        await click(CGPoint(x: PracticeBox.frame.midX, y: PracticeBox.frame.midY - 10))
        return await until(2, { NSWorkspace.shared.frontmostApplication?.processIdentifier == pid })
    }

    /// Below the practice box, clear of it.
    static var pickerFrame: CGRect {
        let box = PracticeBox.frame
        return CGRect(x: box.minX, y: box.minY - 140, width: 260, height: 100)
    }

    // MARK: Input

    private static let source = CGEventSource(stateID: .hidSystemState)

    private static func press(_ combo: KeyCombo) {
        var flags: CGEventFlags = []
        if combo.modifiers & UInt32(cmdKey) != 0 { flags.insert(.maskCommand) }
        if combo.modifiers & UInt32(optionKey) != 0 { flags.insert(.maskAlternate) }
        if combo.modifiers & UInt32(controlKey) != 0 { flags.insert(.maskControl) }
        if combo.modifiers & UInt32(shiftKey) != 0 { flags.insert(.maskShift) }
        press(CGKeyCode(combo.keyCode), flags: flags)
    }

    private static func press(_ key: CGKeyCode, flags: CGEventFlags = []) {
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down)
            event?.flags = flags
            event?.post(tap: .cghidEventTap)
        }
    }

    /// A left click at `point` (screen coordinates, bottom-left origin), held for `hold`
    /// seconds. Iris handles the press while it's held, as it would a person's.
    private static func click(_ point: CGPoint, hold: Double = 0.05) async {
        let at = flipped(point)
        mouse(.mouseMoved, at)
        try? await Task.sleep(for: .milliseconds(30))
        mouse(.leftMouseDown, at)
        try? await Task.sleep(for: .seconds(hold))
        mouse(.leftMouseUp, at)
    }

    private static func move(to point: CGPoint) { mouse(.mouseMoved, flipped(point)) }

    /// A scroll wheel turned `lines` at `point` (negative scrolls the page down).
    private static func scroll(at point: CGPoint, lines: Int32) {
        move(to: point)
        let event = CGEvent(scrollWheelEvent2Source: source, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0)
        event?.location = flipped(point)
        event?.post(tap: .cghidEventTap)
    }

    private static func mouse(_ type: CGEventType, _ at: CGPoint) {
        let event = CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: at, mouseButton: .left)
        // A single click, as a real mouse says; the menu bar ignores a press without it.
        if type != .mouseMoved { event?.setIntegerValueField(.mouseEventClickState, value: 1) }
        event?.post(tap: .cghidEventTap)
    }

    /// Cocoa's bottom-left screen point to Quartz's top-left one.
    private static func flipped(_ point: CGPoint) -> CGPoint {
        CGPoint(x: point.x, y: (NSScreen.screens.first?.frame.maxY ?? 0) - point.y)
    }

    /// The middle of `rect`, if it's on a screen.
    private static func onScreen(_ rect: CGRect) -> CGPoint? {
        let middle = CGPoint(x: rect.midX, y: rect.midY)
        return NSScreen.screens.contains { $0.frame.contains(middle) } ? middle : nil
    }

    // MARK: Results

    private static func until(_ seconds: Double, _ condition: () -> Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while !condition() {
            if Date() > deadline { return false }
            try? await Task.sleep(for: .milliseconds(10))
        }
        return true
    }

    private static func pass(_ check: String, _ what: String) { print("PASS \(check): \(what)") }

    private static func fail(_ check: String, _ what: String) {
        failures += 1
        print("FAIL \(check): \(what)")
    }
}

/// Finds Iris's own views by what VoiceOver reads, in screen coordinates (bottom-left origin).
@MainActor
enum AXFind {
    static func element(in window: NSWindow, role: String?, named name: String) -> AXUIElement? {
        let app = AXUIElementCreateApplication(getpid())
        var windows: CFTypeRef?
        AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &windows)
        let target = (windows as? [AXUIElement])?.first { string($0, kAXTitleAttribute) == window.title }
        return target.flatMap { search($0, role: role, name: name, depth: 0) }
    }

    static func isEnabled(_ element: AXUIElement) -> Bool {
        var value: CFTypeRef?
        AXUIElementCopyAttributeValue(element, kAXEnabledAttribute as CFString, &value)
        return (value as? Bool) ?? false
    }

    static func frame(of element: AXUIElement) -> CGRect? {
        var position: CFTypeRef?, size: CFTypeRef?
        AXUIElementCopyAttributeValue(element, kAXPositionAttribute as CFString, &position)
        AXUIElementCopyAttributeValue(element, kAXSizeAttribute as CFString, &size)
        var p = CGPoint.zero, s = CGSize.zero
        guard let position, let size, AXValueGetValue(position as! AXValue, .cgPoint, &p),
            AXValueGetValue(size as! AXValue, .cgSize, &s)
        else { return nil }
        // AX is top-left; Cocoa is bottom-left.
        let top = NSScreen.screens.first?.frame.maxY ?? 0
        return CGRect(x: p.x, y: top - p.y - s.height, width: s.width, height: s.height)
    }

    private static func string(_ element: AXUIElement, _ attribute: String) -> String? {
        var value: CFTypeRef?
        AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
        return value as? String
    }

    private static func search(_ element: AXUIElement, role: String?, name: String, depth: Int) -> AXUIElement? {
        guard depth < 40 else { return nil }
        let names = [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute].compactMap { string(element, $0) }
        if names.contains(where: { $0 == name || $0.hasPrefix(name + ",") }), role == nil || string(element, kAXRoleAttribute) == role {
            return element
        }
        var children: CFTypeRef?
        AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children)
        for child in (children as? [AXUIElement]) ?? [] {
            if let found = search(child, role: role, name: name, depth: depth + 1) { return found }
        }
        return nil
    }
}

/// `Iris --practice-box`: a plain window with a sentence to fix, standing in for the app you
/// type in, for `--self-check`. A separate process, as Slack would be, so the panel floats
/// over another app as it does for real.
@MainActor
enum PracticeBox {
    /// Where it stands on the main screen; the self-check clicks into it.
    static var frame: CGRect {
        let visible = NSScreen.screens.first?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
        return CGRect(x: visible.midX - 300, y: visible.midY - 20, width: 600, height: 120)
    }

    static func run() -> Never {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        // `--on-display`: scripts/verify's text box, on the test display and never in front.
        var frame = frame
        if OnDisplay.isOn {
            // Never comes forward, not even at launch: a box on the test display must not take
            // the person's keyboard (measured: as an accessory app it came to the front on launch).
            app.setActivationPolicy(.prohibited)
            OnDisplay.start()
            guard let screen = OnDisplay.screen else { exit(3) }
            // Each journey's box in a spot of its own (IRIS_VERIFY_SLOT, 0…3), so journeys run side by side.
            let slot = ProcessInfo.processInfo.environment["IRIS_VERIFY_SLOT"].flatMap(Int.init) ?? 0
            let v = screen.visibleFrame
            frame = CGRect(x: v.minX + 40 + CGFloat(slot % 2) * 960, y: v.minY + 60 + CGFloat(slot / 2 % 2) * 540, width: 600, height: 120)
        }
        let window = NSWindow(contentRect: frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.title = "Iris practice box"
        let scroll = NSTextView.scrollableTextView()
        if let text = scroll.documentView as? NSTextView {
            text.string = OnboardingModel.practiceSample
            text.font = .systemFont(ofSize: 15)
            text.textContainerInset = NSSize(width: 10, height: 12)
        }
        window.contentView = scroll
        window.setFrame(window.frameRect(forContentRect: frame), display: true)
        window.orderFrontRegardless()
        if OnDisplay.isOn, let text = scroll.documentView {
            // The box has the app's focus (what Accessibility reads) without the app coming forward.
            window.makeFirstResponder(text)
            OnDisplay.say("ready practice-box window \(window.windowNumber)")
        }
        app.run()
        exit(0)
    }
}
