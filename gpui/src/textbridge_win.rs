//! Windows: reads the text you're writing in another app through UI Automation, and writes
//! the answer back by pasting (`textbridge.rs` is the Mac's and says the why of each step).
//!
//! Read: the focused element's selection, else its whole text (TextPattern, else ValuePattern).
//! A password box is never read. An app that exposes neither: the selection copied with Ctrl+C,
//! once the shortcut's keys are let go (Ctrl+C sent while Win is still down is Win+Ctrl+C, which
//! turns on the colour filter), and never in a console, where Ctrl+C stops the program.
//!
//! Write: the clipboard kept, the answer put on it out of Windows' clipboard history, the app's
//! window brought back, Ctrl+A (for a whole box) and Ctrl+V, and the clipboard put back. GPUI's
//! own window activation is never used here: it taps Alt, which would reach the app.
//!
//! App identity: the program's file name ("notepad.exe") stands where the Mac has a bundle ID,
//! and its description ("Notepad") is its name.

use super::*;
use std::sync::Mutex;
use windows::core::{BSTR, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTextPattern2,
    IUIAutomationTextRange, IUIAutomationValuePattern, TextUnit_Line, TreeScope_Descendants, UIA_HasKeyboardFocusPropertyId,
    UIA_TextPattern2Id, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU,
    VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow, GA_ROOT,
    SW_RESTORE,
};

/// A UI Automation element (free-threaded: COM's multithreaded apartment).
#[derive(Clone)]
pub struct Element(IUIAutomationElement);
unsafe impl Send for Element {}
unsafe impl Sync for Element {}

impl Element {
    fn raw(&self) -> &IUIAutomationElement {
        &self.0
    }
}

/// What was read.
#[derive(Clone)]
pub struct Captured {
    pub pid: i32,
    pub app_name: Option<String>,
    /// The program's file name ("notepad.exe"): Windows' stand-in for a bundle ID.
    pub bundle_id: Option<String>,
    pub element: Option<Element>,
    /// The app's top-level window, to bring back for the paste.
    pub window: isize,
    pub text: String,
    pub scope: Scope,
    pub field_frame: Option<[f64; 4]>,
    pub can_replace: bool,
    pub own: bool,
}

impl Captured {
    pub fn element_known(&self) -> bool {
        self.element.is_some()
    }
}

/// Windows asks for no permission to read other apps' text.
pub fn is_trusted() -> bool {
    true
}

pub fn request_trust() {}

/// The window in front when the shortcut was pressed, by its program.
static FRONT: Mutex<isize> = Mutex::new(0);

/// The process that owns the window in front (remembered, for the paste).
pub fn focused_app() -> Option<i32> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    *FRONT.lock().unwrap() = hwnd.0 as isize;
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0 && pid != std::process::id()).then_some(pid as i32)
}

type UiaJob = Box<dyn FnOnce(&IUIAutomation) + Send>;

/// Dedicated MTA thread for all UI Automation COM calls: keeps UIA off the STA UI thread
/// (avoiding `RPC_E_CHANGED_MODE` and STA deadlocks) and avoids calling `CoInitializeEx` on
/// Windows Thread Pool workers without matching `CoUninitialize`.
fn on_uia<T: Send + 'static>(f: impl FnOnce(&IUIAutomation) -> T + Send + 'static) -> Option<T> {
    static TX: std::sync::OnceLock<std::sync::mpsc::Sender<UiaJob>> = std::sync::OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<UiaJob>();
        let _ = std::thread::Builder::new().name("iris-uia".into()).spawn(move || {
            struct ComGuard(bool);
            impl Drop for ComGuard {
                fn drop(&mut self) {
                    if self.0 {
                        unsafe { CoUninitialize() };
                    }
                }
            }
            let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
            // Made on the first job and asked again after a failure, as the old per-call
            // `CoCreateInstance` was, so one bad moment doesn't end every later read.
            let mut ua: Option<IUIAutomation> = None;
            while let Ok(job) = rx.recv() {
                if ua.is_none() {
                    ua = unsafe { CoCreateInstance::<_, IUIAutomation>(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }.ok();
                }
                // No automation: the job is dropped unrun and its caller gets `None`. A job that
                // panics also answers `None`, and the thread lives on for the next one.
                if let Some(ua) = &ua {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(ua)));
                }
            }
            drop(ua);
        });
        tx
    });
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
    tx.send(Box::new(move |ua| {
        let _ = reply_tx.send(f(ua));
    }))
    .ok()?;
    reply_rx.recv().ok()
}

fn bstr(b: windows::core::Result<BSTR>) -> Option<String> {
    b.ok().map(|s| s.to_string())
}

/// The program's file name and its description, for `pid`.
pub fn app_info(pid: i32) -> (Option<String>, Option<String>) {
    let Some(path) = exe_path(pid) else { return (None, None) };
    let file = std::path::Path::new(&path).file_name().map(|f| f.to_string_lossy().to_lowercase());
    (Some(program_name(&path)), file)
}

/// A program's name as people know it: its FileDescription ("Microsoft Edge"), else its file
/// name without ".exe", capitalized.
pub fn program_name(path: &str) -> String {
    description(path).unwrap_or_else(|| {
        let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut c = stem.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    })
}

/// The program file a process runs.
pub fn exe_path(pid: i32) -> Option<String> {
    unsafe {
        let h: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid as u32).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// The program's FileDescription ("Notepad", "Microsoft Edge"), from its version resource.
fn description(path: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), None, size, data.as_mut_ptr() as _).ok()?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let q: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain([0]).collect();
        if !VerQueryValueW(data.as_ptr() as _, PCWSTR(q.as_ptr()), &mut ptr, &mut len).as_bool() || len < 4 {
            return None;
        }
        let lang = *(ptr as *const u16);
        let page = *(ptr as *const u16).add(1);
        let key = format!("\\StringFileInfo\\{lang:04x}{page:04x}\\FileDescription");
        let k: Vec<u16> = key.encode_utf16().chain([0]).collect();
        if !VerQueryValueW(data.as_ptr() as _, PCWSTR(k.as_ptr()), &mut ptr, &mut len).as_bool() || len == 0 {
            return None;
        }
        let s = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let text = String::from_utf16_lossy(s).trim_end_matches('\0').trim().to_string();
        (!text.is_empty()).then_some(text)
    }
}

/// Screen pixels to the points GPUI places windows in.
fn to_points(r: RECT, hwnd: HWND) -> [f64; 4] {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64;
    let s = 96.0 / dpi;
    [r.left as f64 * s, r.top as f64 * s, (r.right - r.left) as f64 * s, (r.bottom - r.top) as f64 * s]
}

fn class_of(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// A console (Windows Terminal, conhost): Ctrl+C there stops the program instead of copying.
fn is_console(hwnd: HWND) -> bool {
    matches!(class_of(hwnd).as_str(), "ConsoleWindowClass" | "CASCADIA_HOSTING_WINDOW_CLASS" | "PseudoConsoleWindow")
}

/// Reads the text box you're in, in the app `pid`. UI Automation calls go into the other app
/// and can block: call it off the main thread.
pub fn capture(pid: i32) -> Result<Captured, CaptureError> {

    let window = HWND(*FRONT.lock().unwrap() as _);
    let window = if window.0.is_null() { unsafe { GetForegroundWindow() } } else { window };
    let window = unsafe { GetAncestor(window, GA_ROOT) };
    let (app_name, bundle_id) = app_info(pid);
    // Chromium (Edge, Chrome, Electron apps) builds its accessibility tree only once asked: the
    // first answers are a blank pane, then the whole page, before focus reaches the box (measured
    // in Edge, where the page's answer held a password box's dots). There, only a text box or a
    // selection is taken, asked again for up to 2 s; never the page.
    let chromium = class_of(window).starts_with("Chrome_WidgetWin");
    let settled = |r: &Read| r.secure || (r.text.is_some() && (!chromium || r.is_input || r.scope == Scope::Selection));
    // One read per job, the waits between them here: the UIA thread is never held for the 2 s,
    // so a check that `still_holds` or `current_text` asks for meanwhile isn't stuck behind it.
    let ask = || on_uia(move |ua| read_focused(ua, pid)).flatten();
    let mut read = ask();
    for _ in 0..10 {
        if read.as_ref().is_some_and(settled) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        read = ask();
    }
    let read = read.filter(|r| !chromium || settled(r));
    if let Some(r) = &read {
        // A password box, or text that is only a password box's dots (a second net, whatever
        // the app says about the box).
        let masked = r.text.as_deref().is_some_and(|t| {
            let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
            !t.is_empty() && t.chars().all(|c| matches!(c, '•' | '●' | '∙' | '*'))
        });
        if r.secure || masked {
            return Err(CaptureError::SecureField);
        }
    }
    let frame = read.as_ref().and_then(|r| r.frame).map(|f| to_points(f, window));
    let base = |text: String, scope: Scope, element: Option<Element>, editable: bool| Captured {
        pid, app_name: app_name.clone(), bundle_id: bundle_id.clone(), element, window: window.0 as isize, text, scope,
        field_frame: frame, can_replace: editable, own: false,
    };
    if let Some(r) = read.clone() {
        if let Some(text) = r.text.filter(|t| !t.trim().is_empty()) {
            return Ok(base(text, r.scope, r.element, r.editable));
        }
    }
    // Nothing exposed: copy the selection, never in a console and never under test.
    if under_test() || is_console(window) {
        return Err(CaptureError::NothingToCheck);
    }
    if let Some(copied) = copy_selection(pid).filter(|c| !c.trim().is_empty()) {
        return Ok(base(copied, Scope::Selection, None, read.map_or(true, |r| r.editable)));
    }
    Err(CaptureError::NothingToCheck)
}

#[derive(Clone)]
struct Read {
    element: Option<Element>,
    /// A text box (an edit control, or anything with a value to set), not a document or a page.
    is_input: bool,
    text: Option<String>,
    scope: Scope,
    frame: Option<RECT>,
    editable: bool,
    secure: bool,
}

fn read_focused(ua: &IUIAutomation, pid: i32) -> Option<Read> {
    unsafe {
        let mut el = ua.GetFocusedElement().ok()?;
        // A page (Chromium reports its document as focused while a box inside it has the
        // keyboard, a password box for as long as it's focused): the element in it that has
        // the keyboard instead.
        if el.CurrentControlType().map(|t| t.0) == Ok(50030 /* Document */) {
            let has_keys = ua.CreatePropertyCondition(UIA_HasKeyboardFocusPropertyId, &VARIANT::from(true)).ok();
            if let Some(inner) = has_keys.and_then(|c| el.FindFirst(TreeScope_Descendants, &c).ok()) {
                el = inner;
            }
        }
        if el.CurrentProcessId().ok()? != pid {
            return None;
        }
        let secure = el.CurrentIsPassword().map(|b| b.as_bool()).unwrap_or(false);
        let frame = el.CurrentBoundingRectangle().ok().filter(|r| r.right > r.left && r.bottom > r.top);
        // An edit control or a combo box. Not "anything with a value": Chromium gives the whole
        // page a value (its address), and took a page holding a password box for a text box.
        let kind = el.CurrentControlType().map(|t| t.0).unwrap_or(0);
        let is_input = kind == 50004 /* Edit */ || kind == 50003 /* ComboBox */;
        let mut read = Read { element: Some(Element(el.clone())), is_input, text: None, scope: Scope::WholeField, frame, editable: true, secure };
        if secure {
            return Some(read);
        }
        if let Ok(tp) = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
            // The selection, when there is one; else everything.
            if let Ok(sel) = tp.GetSelection() {
                if sel.Length().unwrap_or(0) > 0 {
                    if let Ok(range) = sel.GetElement(0) {
                        if let Some(t) = bstr(range.GetText(-1)).filter(|t| !t.is_empty()) {
                            read.text = Some(t);
                            read.scope = Scope::Selection;
                        }
                    }
                }
            }
            if read.text.is_none() {
                if let Ok(doc) = tp.DocumentRange() {
                    read.text = bstr(doc.GetText(-1));
                }
            }
        }
        if let Ok(vp) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
            read.editable = !vp.CurrentIsReadOnly().map(|b| b.as_bool()).unwrap_or(false);
            if read.text.as_deref().is_none_or(str::is_empty) {
                read.text = bstr(vp.CurrentValue());
                read.scope = Scope::WholeField;
            }
        }
        read.text = read.text.map(|t| t.replace("\r\n", "\n").replace('\r', "\n"));
        // A box as big as a document (Notepad's whole page): the panel goes by the line the
        // caret is on, or the selection, as the Mac goes by the selected words in a page.
        if read.frame.is_some_and(|r| r.bottom - r.top > 160) {
            let line = if read.scope == Scope::Selection {
                el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).ok()
                    .and_then(|tp| tp.GetSelection().ok()).and_then(|s| s.GetElement(0).ok()).and_then(|r| range_rect(&r))
            } else {
                el.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id).ok().and_then(|tp| {
                    let mut active = Default::default();
                    let caret = tp.GetCaretRange(&mut active).ok()?;
                    let _ = caret.ExpandToEnclosingUnit(TextUnit_Line);
                    range_rect(&caret)
                })
            };
            if line.is_some() {
                read.frame = line;
            }
        }
        Some(read)
    }
}

/// The first rectangle a text range covers on screen.
fn range_rect(range: &IUIAutomationTextRange) -> Option<RECT> {
    use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetUBound, SafeArrayUnaccessData};
    unsafe {
        let sa = range.GetBoundingRectangles().ok()?;
        if sa.is_null() {
            return None;
        }
        let upper = SafeArrayGetUBound(sa, 1).unwrap_or(-1);
        let mut out = None;
        if upper >= 3 {
            let mut data = std::ptr::null_mut();
            if SafeArrayAccessData(sa, &mut data).is_ok() {
                let v = std::slice::from_raw_parts(data as *const f64, 4);
                if v[2] > 0.0 && v[3] > 0.0 {
                    out = Some(RECT { left: v[0] as i32, top: v[1] as i32, right: (v[0] + v[2]) as i32, bottom: (v[1] + v[3]) as i32 });
                }
                let _ = SafeArrayUnaccessData(sa);
            }
        }
        let _ = SafeArrayDestroy(sa);
        out
    }
}

/// Whether the box Iris read still holds the same text.
pub fn still_holds(c: &Captured) -> bool {
    let Some(expected_el) = c.element.clone() else { return false };
    let pid = c.pid;
    let expected_text = c.text.clone();
    on_uia(move |ua| {
        let Some(now) = read_focused(ua, pid) else { return false };
        let same = match &now.element {
            Some(a) => unsafe { ua.CompareElements(a.raw(), expected_el.raw()).map(|x| x.as_bool()).unwrap_or(false) },
            None => false,
        };
        same && now.text.as_deref() == Some(expected_text.as_str())
    })
    .unwrap_or(false)
}

/// What the box holds now.
pub fn current_text(c: &Captured) -> Option<String> {
    let el = c.element.clone()?;
    on_uia(move |_| unsafe {
        if let Ok(tp) = el.raw().GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
            return bstr(tp.DocumentRange().ok()?.GetText(-1)).map(|t| t.replace("\r\n", "\n"));
        }
        bstr(el.raw().GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok()?.CurrentValue())
    })
    .flatten()
}

/// Under test: the whole box set through ValuePattern (no keyboard, no clipboard).
pub fn write_for_test(c: &Captured, text: &str) -> bool {
    let Some(el) = c.element.clone() else { return false };
    let text = text.to_string();
    on_uia(move |_| unsafe {
        el.raw().GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .and_then(|vp| vp.SetValue(&BSTR::from(text)))
            .is_ok()
    })
    .unwrap_or(false)
}

// MARK: - The clipboard

/// Every format on the clipboard that is plain memory, to put back exactly.
pub struct Clipboard(Vec<(u32, Vec<u8>)>);

fn open_clipboard() -> bool {
    for _ in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

impl Clipboard {
    pub fn take() -> Clipboard {
        // Held only until `restore`, so the ceiling is far above any real clipboard (an 8K
        // screenshot is about 130 MB) and only stops a runaway app-private blob.
        const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024 * 1024;
        let mut saved = vec![];
        let mut total = 0usize;
        if !open_clipboard() {
            return Clipboard(saved);
        }
        unsafe {
            let mut f = EnumClipboardFormats(0);
            while f != 0 {
                // Skip GDI handles, owner-display, display, and private/GDI-object formats
                // (none of which are valid HGLOBAL memory handles for GlobalLock).
                let non_hglobal = matches!(f, 2 | 3 | 9 | 14 | 0x0080..=0x008E | 0x0200..=0x03FF);
                if !non_hglobal {
                    if let Ok(h) = GetClipboardData(f) {
                        let g = HGLOBAL(h.0);
                        let n = GlobalSize(g);
                        if n > 0 && total.saturating_add(n) <= MAX_CLIPBOARD_BYTES {
                            let p = GlobalLock(g);
                            if !p.is_null() {
                                saved.push((f, std::slice::from_raw_parts(p as *const u8, n).to_vec()));
                                total += n;
                                let _ = GlobalUnlock(g);
                            }
                        }
                    }
                }
                f = EnumClipboardFormats(f);
            }
            let _ = CloseClipboard();
        }
        Clipboard(saved)
    }

    pub fn restore(&self) {
        if !open_clipboard() {
            return;
        }
        unsafe {
            let _ = EmptyClipboard();
            for (f, bytes) in &self.0 {
                put(*f, bytes);
            }
            let _ = CloseClipboard();
        }
    }
}

unsafe fn put(format: u32, bytes: &[u8]) {
    unsafe {
        let Ok(g) = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len().max(1)) else { return };
        let p = GlobalLock(g);
        if p.is_null() {
            let _ = GlobalFree(Some(g));
            return;
        }
        if !bytes.is_empty() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
        }
        let _ = GlobalUnlock(g);
        // SetClipboardData only takes ownership of `g` on success; free it on failure.
        if SetClipboardData(format, Some(HANDLE(g.0))).is_err() {
            let _ = GlobalFree(Some(g));
        }
    }
}

fn format(name: &str) -> u32 {
    let w: Vec<u16> = name.encode_utf16().chain([0]).collect();
    unsafe { RegisterClipboardFormatW(PCWSTR(w.as_ptr())) }
}

/// Puts `text` on the clipboard (kept out of Windows' clipboard history and cloud clipboard
/// when `transient`); returns the clipboard's sequence number after.
pub fn put_on_clipboard(text: &str, transient: bool) -> isize {
    if open_clipboard() {
        unsafe {
            let _ = EmptyClipboard();
            let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain([0]).collect();
            put(13, std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)); // CF_UNICODETEXT
            if transient {
                put(format("ExcludeClipboardContentFromMonitorProcessing"), &[0]);
                put(format("CanIncludeInClipboardHistory"), &0u32.to_le_bytes());
                put(format("CanUploadToCloudClipboard"), &0u32.to_le_bytes());
            }
            let _ = CloseClipboard();
        }
    }
    clipboard_count()
}

pub fn clipboard_count() -> isize {
    unsafe { GetClipboardSequenceNumber() as isize }
}

pub fn clipboard_string() -> Option<String> {
    if !open_clipboard() {
        return None;
    }
    unsafe {
        let out = GetClipboardData(13).ok().and_then(|h| {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u16;
            if p.is_null() {
                return None;
            }
            let n = GlobalSize(g) / 2;
            let s = std::slice::from_raw_parts(p, n);
            let end = s.iter().position(|c| *c == 0).unwrap_or(n);
            let text = String::from_utf16_lossy(&s[..end]).replace("\r\n", "\n");
            let _ = GlobalUnlock(g);
            Some(text)
        });
        let _ = CloseClipboard();
        out
    }
}

// MARK: - The keyboard

/// Brings the app's window back to the front (Iris is in front, so Windows lets it).
pub fn activate(_pid: i32) {
    let hwnd = HWND(*FRONT.lock().unwrap() as _);
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

pub fn has_keyboard(pid: i32) -> bool {
    let mut front = 0u32;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut front)) };
    front == pid as u32
}

/// Waits (up to a second) until Ctrl, Alt, Shift and Win are all up: keys sent while the
/// shortcut is still held would combine with it.
fn wait_for_keys_up() {
    for _ in 0..50 {
        let down = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN].iter().any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } as u16 & 0x8000 != 0);
        if !down {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() }, time: 0, dwExtraInfo: 0 } },
    }
}

fn ctrl(letter: u16) {
    let k = VIRTUAL_KEY(letter);
    let inputs = [key(VK_CONTROL, false), key(k, false), key(k, true), key(VK_CONTROL, true)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

/// Ctrl+A (for the whole box) then Ctrl+V, to the window in front, checking it's still the
/// app's before Ctrl+V so a switch during the 60 ms gap never pastes into another window.
pub fn select_all_and_paste(pid: i32, whole: bool) -> bool {
    wait_for_keys_up();
    if whole {
        ctrl(b'A' as u16);
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    if !has_keyboard(pid) {
        return false;
    }
    ctrl(b'V' as u16);
    true
}

/// Ctrl+C to the window in front.
pub fn copy_shortcut() {
    wait_for_keys_up();
    ctrl(b'C' as u16);
}

/// Bumped by every new `copy_selection` or Replace, so an earlier late-copy watch stops.
static LATE_COPY_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Replaces holding the clipboard: `copy_selection` waits for them rather than saving Iris's own
/// entry as the person's (the Mac's `lastReplace`).
static REPLACE_BORROW: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// The clipboard held by a Replace; dropping it lets a waiting copy go.
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

/// Stops any late-copy watch and marks the clipboard as held by a Replace.
pub fn begin_replace_borrow() -> ReplaceBorrow {
    LATE_COPY_GEN.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    *REPLACE_BORROW.0.lock().unwrap() += 1;
    ReplaceBorrow
}

fn wait_for_replace_borrow() {
    let (lock, cvar) = &REPLACE_BORROW;
    // Covers a Replace's longest hold (focus wait, Ctrl+A, the 450 ms before the clipboard goes back).
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

/// Copies the selection with Ctrl+C, the clipboard put back as it was. Waits up to 0.4 s, and
/// keeps watching a second more in case the app answers late (as the Mac does). Nothing when
/// another window took the keyboard, before or during the copy.
fn copy_selection(expected_pid: i32) -> Option<String> {
    wait_for_replace_borrow();
    let generation = LATE_COPY_GEN.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    if !has_keyboard(expected_pid) {
        return None;
    }
    let saved = Clipboard::take();
    let before = clipboard_count();
    copy_shortcut();
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if clipboard_count() != before {
            let copied = clipboard_string();
            saved.restore();
            return has_keyboard(expected_pid).then_some(copied).flatten();
        }
    }
    // An app answering Ctrl+C after Iris stopped waiting would leave the selection on the
    // clipboard: look a second more, and put the person's clipboard back if it does.
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
