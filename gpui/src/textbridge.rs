//! Reads the text you're writing in another app, and writes the answer back
//! (`TextBridge.swift`, ported).
//!
//! Read: Accessibility (the selected text, else the whole box). Electron apps (Slack) build
//! their tree only once asked (`AXManualAccessibility`); Chromium browsers only for a screen
//! reader (`AXEnhancedUserInterface`). Fallback: copy the selection with ⌘C.
//!
//! Write: paste, the one way that works the same in native boxes, Electron and browsers. The
//! clipboard is saved first and put back after, unless something else wrote to it meanwhile.
//!
//! Under test (`IRIS_CONTROL`), as the Swift app's `OnDisplay`: only the app the test named is
//! read (never whatever has focus on someone's screen), the clipboard is never touched, no key
//! press is posted, and a paste is written into the box through Accessibility instead.

#![allow(clippy::missing_safety_doc)]

/// Where the text came from: the selection, or the whole box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Selection,
    WholeField,
}

/// Why nothing could be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    NotTrusted,
    SecureField,
    NothingToCheck,
}

/// Whether a test drives this Iris (see the module's comment).
pub fn under_test() -> bool {
    std::env::var("IRIS_CONTROL").is_ok() || crate::driven::on()
}

#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(windows)]
#[path = "textbridge_win.rs"]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(any(target_os = "macos", windows)))]
pub use other::*;

#[cfg(not(any(target_os = "macos", windows)))]
mod other {
    use super::*;
    /// What was read. (Windows reads through UI Automation: still to come.)
    #[derive(Clone)]
    pub struct Captured {
        pub pid: i32,
        pub app_name: Option<String>,
        pub bundle_id: Option<String>,
        pub text: String,
        pub scope: Scope,
        pub field_frame: Option<[f64; 4]>,
        pub can_replace: bool,
        pub own: bool,
    }
    pub fn is_trusted() -> bool { false }
    pub fn request_trust() {}
    pub fn focused_app() -> Option<i32> { None }
    pub fn capture(_pid: i32) -> Result<Captured, CaptureError> { Err(CaptureError::NothingToCheck) }
    pub fn still_holds(_c: &Captured) -> bool { false }
    impl Captured {
        pub fn element_known(&self) -> bool { false }
    }
    pub fn current_text(_c: &Captured) -> Option<String> { None }
    pub fn write_for_test(_c: &Captured, _text: &str) -> bool { false }
    pub struct Clipboard;
    impl Clipboard {
        pub fn take() -> Clipboard { Clipboard }
        pub fn restore(&self) {}
    }
    pub struct ReplaceBorrow;
    pub fn begin_replace_borrow() -> ReplaceBorrow { ReplaceBorrow }
    pub fn put_on_clipboard(_text: &str, _transient: bool) -> isize { 0 }
    pub fn clipboard_count() -> isize { 0 }
    pub fn activate(_pid: i32) {}
    pub fn has_keyboard(_pid: i32) -> bool { false }
    pub fn select_all_and_paste(_pid: i32, _whole: bool) -> bool { false }
    pub fn copy_shortcut() {}
    pub fn clipboard_string() -> Option<String> { None }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::c_void;

    use accessibility_sys::*;
    use core_foundation::base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use objc2::rc::{Retained, autoreleasepool};
    use objc2_app_kit::{NSApplicationActivationOptions, NSApplicationActivationPolicy, NSPasteboard, NSPasteboardItem, NSRunningApplication, NSWorkspace};
    use objc2_foundation::{NSArray, NSData, NSString};

    /// How long one Accessibility call may block before giving up on that app (a hung app
    /// would otherwise stall Iris for seconds).
    const AX_TIMEOUT: f32 = 0.25;

    /// An Accessibility element, retained.
    pub struct Element(AXUIElementRef);

    impl Element {
        unsafe fn retain(r: AXUIElementRef) -> Element {
            unsafe { CFRetain(r as CFTypeRef) };
            Element(r)
        }
        fn app(pid: i32) -> Element {
            let e = Element(unsafe { AXUIElementCreateApplication(pid) });
            unsafe { AXUIElementSetMessagingTimeout(e.0, AX_TIMEOUT) };
            e
        }
        fn system_wide() -> Element {
            let e = Element(unsafe { AXUIElementCreateSystemWide() });
            unsafe { AXUIElementSetMessagingTimeout(e.0, AX_TIMEOUT) };
            e
        }
        fn attr(&self, name: &str) -> Option<CFType> {
            let mut value: CFTypeRef = std::ptr::null();
            let key = CFString::new(name);
            let err = unsafe { AXUIElementCopyAttributeValue(self.0, key.as_concrete_TypeRef(), &mut value) };
            (err == kAXErrorSuccess && !value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
        }
        fn element(&self, name: &str) -> Option<Element> {
            let v = self.attr(name)?;
            (v.type_of() == unsafe { AXUIElementGetTypeID() }).then(|| unsafe { Element::retain(v.as_CFTypeRef() as AXUIElementRef) })
        }
        fn string(&self, name: &str) -> Option<String> {
            self.attr(name)?.downcast::<CFString>().map(|s| s.to_string())
        }
        fn set(&self, name: &str, value: &CFType) -> bool {
            let key = CFString::new(name);
            unsafe { AXUIElementSetAttributeValue(self.0, key.as_concrete_TypeRef(), value.as_CFTypeRef()) == kAXErrorSuccess }
        }
        fn settable(&self, name: &str) -> bool {
            let key = CFString::new(name);
            let mut ok: u8 = 0;
            unsafe { AXUIElementIsAttributeSettable(self.0, key.as_concrete_TypeRef(), &mut ok) == kAXErrorSuccess && ok != 0 }
        }
        fn pid(&self) -> Option<i32> {
            let mut pid = 0;
            (unsafe { AXUIElementGetPid(self.0, &mut pid) } == kAXErrorSuccess && pid > 0).then_some(pid)
        }
        fn role(&self) -> String {
            self.string("AXRole").unwrap_or_default()
        }
        fn children(&self) -> Vec<Element> {
            let Some(v) = self.attr("AXChildren") else { return vec![] };
            let Some(arr) = v.downcast::<core_foundation::array::CFArray>() else { return vec![] };
            arr.iter().filter_map(|item| {
                let r = *item as CFTypeRef;
                (unsafe { core_foundation::base::CFGetTypeID(r) } == unsafe { AXUIElementGetTypeID() }).then(|| unsafe { Element::retain(r as AXUIElementRef) })
            }).collect()
        }
        fn same(&self, other: &Element) -> bool {
            unsafe { core_foundation::base::CFEqual(self.0 as CFTypeRef, other.0 as CFTypeRef) != 0 }
        }
    }

    impl Clone for Element {
        fn clone(&self) -> Self {
            unsafe { Element::retain(self.0) }
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0 as CFTypeRef) };
        }
    }

    // Accessibility elements are Core Foundation objects, safe to use from any thread.
    unsafe impl Send for Element {}
    unsafe impl Sync for Element {}

    /// What was read.
    #[derive(Clone)]
    pub struct Captured {
        pub pid: i32,
        pub app_name: Option<String>,
        pub bundle_id: Option<String>,
        pub element: Option<Element>,
        pub text: String,
        pub scope: Scope,
        /// Where the bud stands and notes appear, in screen points from the top left: the box,
        /// or the selected words when the "box" is a whole page. None: at the pointer.
        pub field_frame: Option<[f64; 4]>,
        /// False when the text can't be edited (selected in a page): Copy instead of Replace.
        pub can_replace: bool,
        /// Iris's own box (setup's practice box): read and written directly, never through
        /// Accessibility or the clipboard.
        pub own: bool,
    }

    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    impl Captured {
        /// Whether the box can be read again (text copied out of an app that hides it can't).
        pub fn element_known(&self) -> bool {
            self.element.is_some()
        }
    }

    /// Asks macOS to show its Accessibility prompt for Iris.
    pub fn request_trust() {
        let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as _) };
    }

    fn running(pid: i32) -> Option<Retained<NSRunningApplication>> {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
    }

    /// An app's name as people know it, and its bundle ID.
    pub fn app_info(pid: i32) -> (Option<String>, Option<String>) {
        autoreleasepool(|_| {
            let Some(app) = running(pid) else { return (None, None) };
            let name = (app.activationPolicy() == NSApplicationActivationPolicy::Regular)
                .then(|| app.localizedName().map(|n| n.to_string())).flatten().filter(|n| !n.is_empty());
            (name, app.bundleIdentifier().map(|b| b.to_string()))
        })
    }

    fn can_own_text_box(pid: i32) -> bool {
        let Some(app) = running(pid) else { return false };
        app.bundleIdentifier().map(|b| b.to_string()).as_deref() != Some("com.apple.loginwindow")
            && app.activationPolicy() != NSApplicationActivationPolicy::Prohibited
    }

    /// The app with keyboard focus, as Accessibility sees it.
    fn focused_application_pid() -> Option<i32> {
        Element::system_wide().element("AXFocusedApplication")?.pid()
    }

    /// The app whose text box you're in: the one Accessibility says has the keyboard, else the
    /// menu bar's owner, else the frontmost app; never loginwindow or a background helper.
    pub fn focused_app() -> Option<i32> {
        autoreleasepool(|_| {
            let ws = NSWorkspace::sharedWorkspace();
            [focused_application_pid(),
             ws.menuBarOwningApplication().map(|a| a.processIdentifier()),
             ws.frontmostApplication().map(|a| a.processIdentifier())]
                .into_iter().flatten().find(|p| can_own_text_box(*p))
        })
    }

    /// Apps already asked to build their Electron tree.
    static ELECTRON: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

    /// Bumped by every new `copy_selection` or `Replace` so an earlier `lateCopy` watchdog stops.
    static LATE_COPY_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    /// Tracks active Replace clipboard borrows so `copy_selection` waits instead of snapshotting
    /// Iris's own transient pasteboard entry.
    static REPLACE_BORROW: (std::sync::Mutex<usize>, std::sync::Condvar) =
        (std::sync::Mutex::new(0), std::sync::Condvar::new());

    /// Guards the clipboard while Replace owns it; dropping releases waiting `copy_selection`s.
    pub struct ReplaceBorrow;

    impl Drop for ReplaceBorrow {
        fn drop(&mut self) {
            let (lock, cvar) = &REPLACE_BORROW;
            let mut count = lock.lock().unwrap();
            *count = count.saturating_sub(1);
            if *count == 0 {
                cvar.notify_all();
            }
        }
    }

    /// Cancels any pending `lateCopy` watchdog and marks the clipboard as borrowed by Replace.
    pub fn begin_replace_borrow() -> ReplaceBorrow {
        LATE_COPY_GEN.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let (lock, _) = &REPLACE_BORROW;
        *lock.lock().unwrap() += 1;
        ReplaceBorrow
    }

    fn wait_for_replace_borrow() {
        let (lock, cvar) = &REPLACE_BORROW;
        // Cover `paste::start_replace`'s worst-case hold (20ms + 600ms focus poll + 60ms + 450ms).
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1_500);
        let mut count = lock.lock().unwrap();
        while *count > 0 {
            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else { break };
            let (next, timeout) = cvar.wait_timeout(count, remaining).unwrap();
            count = next;
            if timeout.timed_out() {
                break;
            }
        }
    }

    #[derive(Default)]
    struct Read {
        element: Option<Element>,
        text: Option<String>,
        scope: Option<Scope>,
        frame: Option<[f64; 4]>,
        selection: Option<[f64; 4]>,
        editable: bool,
        secure: bool,
        owner: Option<i32>,
    }

    /// Reads the text box you're in, in the app `pid`. Accessibility calls are synchronous
    /// calls into the other app: call it off the main thread.
    pub fn capture(pid: i32) -> Result<Captured, CaptureError> {
        autoreleasepool(|_| capture_inner(pid))
    }

    fn capture_inner(pid: i32) -> Result<Captured, CaptureError> {
        if !is_trusted() {
            return Err(CaptureError::NotTrusted);
        }
        let first = {
            let mut seen = ELECTRON.lock().unwrap();
            // Prune terminated processes so a recycled PID is treated as a new app.
            seen.retain(|p| running(*p).is_some_and(|a| !a.isTerminated()));
            !seen.contains(&pid)
        };
        let (read, mark_seen) = read_focused(pid, first);
        if first && mark_seen {
            let mut seen = ELECTRON.lock().unwrap();
            if !seen.contains(&pid) {
                seen.push(pid);
            }
        }
        if read.secure {
            return Err(CaptureError::SecureField);
        }
        // The box itself says which app it lives in: the one a Replace must paste into.
        let mut pid = pid;
        if let Some(owner) = read.owner.filter(|o| *o != pid) {
            if under_test() {
                return Err(CaptureError::NothingToCheck);
            }
            if can_own_text_box(owner) {
                pid = owner;
            }
        }
        let frame = read.frame.or(read.selection);
        let (app_name, bundle_id) = app_info(pid);
        if let Some(text) = read.text {
            return Ok(Captured { pid, app_name, bundle_id, element: read.element, text, scope: read.scope.unwrap_or(Scope::WholeField), field_frame: frame, can_replace: read.editable, own: false });
        }
        // Copying borrows the clipboard and presses ⌘C: never under test.
        if under_test() {
            return Err(CaptureError::NothingToCheck);
        }
        if let Some(copied) = copy_selection(pid).filter(|c| !c.trim().is_empty()) {
            return Ok(Captured { pid, app_name, bundle_id, element: None, text: copied, scope: Scope::Selection, field_frame: frame, can_replace: read.editable, own: false });
        }
        Err(CaptureError::NothingToCheck)
    }

    fn read_focused(pid: i32, first_time: bool) -> (Read, bool) {
        let app = Element::app(pid);
        // Always try reading immediately first: native macOS apps (and Electron/Chromium apps
        // whose tree is already built) answer in ~1ms, avoiding a 200ms sleep and disk walk.
        let mut read = read_in(&app);
        let saw = read.text.is_some() || read.secure;
        if !first_time || saw {
            return (read, saw);
        }
        let mut enabled = false;
        if app.set("AXManualAccessibility", &CFBoolean::true_value().as_CFType()) {
            enabled = true;
            // Electron builds its tree only when asked; poll briefly instead of sleeping a flat 200ms.
            for _ in 0..8 {
                std::thread::sleep(std::time::Duration::from_millis(25));
                read = read_in(&app);
                if read.text.is_some() || read.secure {
                    return (read, true);
                }
            }
        }
        // Chromium builds a page's tree only for a screen reader: ask as VoiceOver does, once,
        // only when nothing was read, and give the page up to 3 seconds (polled at 50ms).
        if read.text.is_none() && !read.secure && draws_pages_like_chrome(pid) && ask_for_screen_reader_tree(&app) {
            enabled = true;
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                read = read_in(&app);
                if read.text.is_some() || read.secure {
                    break;
                }
            }
        }
        let mark_seen = enabled || read.text.is_some() || read.secure;
        (read, mark_seen)
    }

    fn ask_for_screen_reader_tree(app: &Element) -> bool {
        let is_on = || app.attr("AXEnhancedUserInterface").and_then(|v| v.downcast::<CFBoolean>()).map(bool::from);
        if is_on() != Some(false) {
            return false;
        }
        app.set("AXEnhancedUserInterface", &CFBoolean::true_value().as_CFType());
        is_on() == Some(true)
    }

    /// A Chromium browser, by its bundle ID or the page renderer it ships.
    fn draws_pages_like_chrome(pid: i32) -> bool {
        let Some(app) = running(pid) else { return false };
        let known = ["com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac", "company.thebrowser.Browser",
                     "com.vivaldi.Vivaldi", "com.operasoftware.Opera", "org.chromium.Chromium"];
        if let Some(id) = app.bundleIdentifier().map(|b| b.to_string())
            && known.iter().any(|k| id == *k || id.starts_with(&format!("{k}.")))
        {
            return true;
        }
        let Some(url) = app.bundleURL() else { return false };
        let Some(path) = url.path() else { return false };
        let frameworks = std::path::Path::new(&path.to_string()).join("Contents/Frameworks");
        fn walk(dir: &std::path::Path, depth: usize) -> bool {
            let Ok(entries) = std::fs::read_dir(dir) else { return false };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with("Helper (Renderer).app") {
                    return true;
                }
                if !name.ends_with(".app") && depth < 5 && e.path().is_dir() && walk(&e.path(), depth + 1) {
                    return true;
                }
            }
            false
        }
        walk(&frameworks, 0)
    }

    /// Editors keep private markers in their text (Slack starts each bullet line with U+E506);
    /// pasted back they show as stray glyphs (`EditorText.removingEditorMarkers`): the private
    /// use areas and U+FFFC go, except U+F8FF (the Apple logo, which people type).
    pub fn clean(s: String) -> String {
        s.chars().filter(|c| {
            let v = *c as u32;
            v == 0xF8FF || !((0xE000..=0xF8FE).contains(&v) || v >= 0xF0000 || v == 0xFFFC)
        }).collect()
    }

    fn read_in(app: &Element) -> Read {
        let mut read = Read::default();
        let Some(element) = focused_element(app) else { return read };
        unsafe { AXUIElementSetMessagingTimeout(element.0, AX_TIMEOUT) };
        read.owner = element.pid();
        if is_secure(&element) {
            read.secure = true;
            read.element = Some(element);
            return read;
        }
        read.frame = frame(&element);
        // `capture` only uses `read.selection` when `read.frame` is None; skipping the
        // selection/web-area walk when the element already gave its frame avoids up to 400+ AX IPCs.
        if read.frame.is_none() {
            read.selection = selection_bounds(&element)
                .or_else(|| web_area_containing(&element).and_then(|w| selection_bounds(&w)))
                .or_else(|| web_areas_under(&element).iter().find_map(selection_bounds));
        }
        read.editable = is_editable(&element);
        let cleaned = |attr: &str| element.string(attr).map(clean);
        if let Some(sel) = cleaned("AXSelectedText").filter(|s| !s.trim().is_empty()) {
            read.text = Some(sel);
            read.scope = Some(Scope::Selection);
        } else if let Some(value) = cleaned("AXValue").filter(|v| !v.trim().is_empty())
            && Some(&value) != cleaned("AXPlaceholderValue").as_ref()
        {
            read.text = Some(value);
            read.scope = Some(Scope::WholeField);
        }
        read.element = Some(element);
        read
    }

    fn focused_element(app: &Element) -> Option<Element> {
        if let Some(e) = app.element("AXFocusedUIElement") {
            return Some(e);
        }
        // What has focus system-wide is on someone's screen: never under test.
        if under_test() {
            return None;
        }
        Element::system_wide().element("AXFocusedUIElement")
    }

    fn is_secure(e: &Element) -> bool {
        let secure = "AXSecureTextField";
        e.role() == secure || e.string("AXSubrole").as_deref() == Some(secure)
    }

    fn value_of<T: Default>(v: &CFType, kind: u32) -> Option<T> {
        if v.type_of() != unsafe { AXValueGetTypeID() } {
            return None;
        }
        let mut out = T::default();
        unsafe { AXValueGetValue(v.as_CFTypeRef() as AXValueRef, kind, &mut out as *mut T as *mut c_void) }.then_some(out)
    }

    #[derive(Default, Clone, Copy)]
    #[repr(C)]
    struct Pt(f64, f64);
    #[derive(Default, Clone, Copy)]
    #[repr(C)]
    struct Rect(f64, f64, f64, f64);

    fn frame(e: &Element) -> Option<[f64; 4]> {
        let p: Pt = value_of(&e.attr("AXPosition")?, kAXValueTypeCGPoint)?;
        let s: Pt = value_of(&e.attr("AXSize")?, kAXValueTypeCGSize)?;
        (s.0 > 0.0 && s.1 > 0.0).then_some([p.0, p.1, s.0, s.1])
    }

    /// Whether a paste here would land; only no when it's sure.
    fn is_editable(e: &Element) -> bool {
        let role = e.role();
        if ["AXTextField", "AXTextArea", "AXComboBox"].contains(&role.as_str()) || e.settable("AXValue") || e.attr("AXEditableAncestor").is_some() {
            return true;
        }
        !["AXWebArea", "AXStaticText", "AXGroup", "AXScrollArea", "AXList", "AXRow", "AXCell"].contains(&role.as_str())
    }

    fn web_area_containing(e: &Element) -> Option<Element> {
        let mut current = e.clone();
        for _ in 0..12 {
            current = current.element("AXParent")?;
            if current.role() == "AXWebArea" {
                return Some(current);
            }
        }
        None
    }

    fn web_areas_under(e: &Element) -> Vec<Element> {
        let mut queue = std::collections::VecDeque::from([e.clone()]);
        let (mut found, mut seen) = (vec![], 0);
        while let Some(current) = queue.pop_front() {
            if seen >= 400 || found.len() >= 4 {
                break;
            }
            seen += 1;
            if current.role() == "AXWebArea" {
                found.push(current);
                continue;
            }
            queue.extend(current.children());
        }
        found
    }

    /// Where the selected words (or the caret) are: a character range in text views, text
    /// markers in web views.
    fn selection_bounds(e: &Element) -> Option<[f64; 4]> {
        for (range, bounds) in [("AXSelectedTextRange", "AXBoundsForRange"), ("AXSelectedTextMarkerRange", "AXBoundsForTextMarkerRange")] {
            let Some(r) = e.attr(range) else { continue };
            let mut out: CFTypeRef = std::ptr::null();
            let key = CFString::new(bounds);
            let err = unsafe { AXUIElementCopyParameterizedAttributeValue(e.0, key.as_concrete_TypeRef(), r.as_CFTypeRef(), &mut out) };
            if err != kAXErrorSuccess || out.is_null() {
                continue;
            }
            let v = unsafe { CFType::wrap_under_create_rule(out) };
            if let Some(rect) = value_of::<Rect>(&v, kAXValueTypeCGRect)
                && rect.3 > 0.0
            {
                return Some([rect.0, rect.1, rect.2.max(1.0), rect.3]);
            }
        }
        None
    }

    /// Whether the box Iris read still holds the same text (a Quick Fix that finished late
    /// must never paste over something else). Lightweight check: skips frame/selection/editable IPCs.
    pub fn still_holds(c: &Captured) -> bool {
        let Some(element) = &c.element else { return false };
        let app = Element::app(c.pid);
        let Some(now) = focused_element(&app) else { return false };
        if !now.same(element) {
            return false;
        }
        unsafe { AXUIElementSetMessagingTimeout(now.0, AX_TIMEOUT) };
        let cleaned = |attr: &str| now.string(attr).map(clean);
        let text = cleaned("AXSelectedText")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                cleaned("AXValue")
                    .filter(|v| !v.trim().is_empty())
                    .filter(|v| Some(v) != cleaned("AXPlaceholderValue").as_ref())
            });
        text.as_deref() == Some(c.text.as_str())
    }

    /// What the box holds now.
    pub fn current_text(c: &Captured) -> Option<String> {
        c.element.as_ref()?.string("AXValue")
    }

    /// Under test: the paste's last step done through Accessibility (no keyboard, no
    /// clipboard): the whole box replaced, or just the selection.
    pub fn write_for_test(c: &Captured, text: &str) -> bool {
        let Some(element) = &c.element else { return false };
        let attr = if c.scope == Scope::WholeField { "AXValue" } else { "AXSelectedText" };
        element.set(attr, &CFString::new(text).as_CFType())
    }

    // MARK: - The clipboard and the keyboard

    /// Every item and type on the clipboard, to put back exactly. Stored as plain Rust bytes so
    /// the snapshot can safely move onto the late-copy watchdog thread.
    pub struct Clipboard(Vec<Vec<(String, Vec<u8>)>>);

    impl Clipboard {
        pub fn take() -> Clipboard {
            autoreleasepool(|_| {
                let pb = NSPasteboard::generalPasteboard();
                let items = pb.pasteboardItems().map(|items| items.to_vec()).unwrap_or_default();
                Clipboard(items.iter().map(|item| {
                    item.types().to_vec().into_iter().filter_map(|t| {
                        item.dataForType(&t).map(|d| (t.to_string(), d.to_vec()))
                    }).collect()
                }).collect())
            })
        }
        pub fn restore(&self) {
            autoreleasepool(|_| {
                let pb = NSPasteboard::generalPasteboard();
                pb.clearContents();
                let items: Vec<Retained<NSPasteboardItem>> = self.0.iter().map(|types| {
                    let item = NSPasteboardItem::new();
                    for (t, d) in types {
                        item.setData_forType(&NSData::with_bytes(d), &NSString::from_str(t));
                    }
                    item
                }).collect();
                if !items.is_empty() {
                    let refs: Vec<&objc2::runtime::ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting>> =
                        items.iter().map(|i| objc2::runtime::ProtocolObject::from_ref(&**i)).collect();
                    pb.writeObjects(&NSArray::from_slice(&refs));
                }
            })
        }
    }

    /// Puts `text` on the clipboard (marked transient, so clipboard managers skip it, when
    /// `transient`); returns the clipboard's change count after.
    pub fn put_on_clipboard(text: &str, transient: bool) -> isize {
        autoreleasepool(|_| {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            let item = NSPasteboardItem::new();
            item.setString_forType(&NSString::from_str(text), &NSString::from_str("public.utf8-plain-text"));
            if transient {
                for t in ["org.nspasteboard.TransientType", "org.nspasteboard.AutoGeneratedType"] {
                    item.setData_forType(&NSData::new(), &NSString::from_str(t));
                }
            }
            let refs = [objc2::runtime::ProtocolObject::from_ref(&*item)];
            pb.writeObjects(&NSArray::from_slice(&refs));
            pb.changeCount()
        })
    }

    pub fn clipboard_count() -> isize {
        autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount())
    }

    pub fn clipboard_string() -> Option<String> {
        autoreleasepool(|_| NSPasteboard::generalPasteboard().stringForType(&NSString::from_str("public.utf8-plain-text")).map(|s| s.to_string()))
    }

    pub fn activate(pid: i32) {
        autoreleasepool(|_| {
            if let Some(app) = running(pid) {
                #[allow(deprecated)]
                app.activateWithOptions(NSApplicationActivationOptions::empty());
            }
        })
    }

    /// Whether `pid` has the keyboard (any one sign: active, owns the menu bar, or has focus).
    pub fn has_keyboard(pid: i32) -> bool {
        autoreleasepool(|_| {
            let Some(app) = running(pid) else { return false };
            if app.isTerminated() {
                return false;
            }
            app.isActive()
                || NSWorkspace::sharedWorkspace().menuBarOwningApplication().map(|a| a.processIdentifier()) == Some(pid)
                || focused_application_pid() == Some(pid)
        })
    }

    fn post_shortcut(key: u16) {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) else { return };
        for down in [true, false] {
            if let Ok(e) = CGEvent::new_keyboard_event(source.clone(), key, down) {
                e.set_flags(CGEventFlags::CGEventFlagCommand);
                e.post(CGEventTapLocation::HID);
            }
        }
    }

    /// ⌘A (for the whole box) then ⌘V, to the app in front — re-checking keyboard ownership
    /// before ⌘V so a focus switch during the 60 ms gap never pastes into another app.
    pub fn select_all_and_paste(pid: i32, whole: bool) -> bool {
        if whole {
            post_shortcut(0x00); // A
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        if !has_keyboard(pid) {
            return false;
        }
        post_shortcut(0x09); // V
        true
    }

    /// ⌘C to the app in front.
    pub fn copy_shortcut() {
        post_shortcut(0x08); // C
    }

    /// Copies the selection with ⌘C, the clipboard put back as it was. Waits up to 0.3 s, and
    /// keeps watching for a further second if the app answers late (Swift's `lateCopy`).
    fn copy_selection(expected_pid: i32) -> Option<String> {
        // Wait if a Replace is still borrowing the clipboard so we never snapshot Iris's own entry.
        wait_for_replace_borrow();
        let generation = LATE_COPY_GEN.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
        if !has_keyboard(expected_pid) {
            return None;
        }
        let saved = Clipboard::take();
        let before = clipboard_count();
        copy_shortcut();
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            if clipboard_count() != before {
                let copied = clipboard_string().map(clean);
                saved.restore();
                // Abort if another app took focus during ⌘C so we never misattribute its text.
                return has_keyboard(expected_pid).then_some(copied).flatten();
            }
        }
        // An app that answers ⌘C after Iris stopped waiting would leave the selection on the
        // clipboard: keep looking for a second more, and put the clipboard back if it does.
        std::thread::spawn(move || {
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                if LATE_COPY_GEN.load(std::sync::atomic::Ordering::Acquire) != generation {
                    return;
                }
                if clipboard_count() != before {
                    saved.restore();
                    return;
                }
            }
        });
        None
    }

    #[allow(dead_code)]
    fn unused(_: CGPoint, _: CGRect, _: CGSize) {}
}
