//! The global shortcuts (`HotkeyCenter` in the Swift app): the open shortcut and Quick Fix,
//! registered with Carbon's `RegisterEventHotKey` on the Mac and `RegisterHotKey` on Windows,
//! from the key code and modifier mask the settings store (on Windows the ⌘ bit is the Windows
//! key: `keys.rs`). A shortcut another app already holds can't be registered: that's the
//! "taken" state the slip and setup tell people about.
//!
//! Never under test (`IRIS_CONTROL`): a test drives the shortcuts' handlers itself, so the
//! two never fight over a key with the installed Iris.

use std::cell::RefCell;

use gpui::*;

use crate::store::KeyCombo;

/// Which shortcut was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open = 1,
    QuickFix = 2,
}

thread_local! {
    static APP_CX: RefCell<Option<AsyncApp>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
mod carbon {
    use std::ffi::c_void;

    pub type OSStatus = i32;
    pub type EventTargetRef = *mut c_void;
    pub type EventHandlerRef = *mut c_void;
    pub type EventHandlerCallRef = *mut c_void;
    pub type EventRef = *mut c_void;
    pub type EventHotKeyRef = *mut c_void;

    #[repr(C)]
    pub struct EventTypeSpec {
        pub event_class: u32,
        pub event_kind: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct EventHotKeyID {
        pub signature: u32,
        pub id: u32,
    }

    pub const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
    pub const EVENT_HOTKEY_PRESSED: u32 = 5;
    pub const PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
    pub const TYPE_HOTKEY_ID: u32 = u32::from_be_bytes(*b"hkid");
    pub const SIGNATURE: u32 = u32::from_be_bytes(*b"IRIS");
    pub const HOTKEY_EXISTS: OSStatus = -9878;

    pub type Handler = extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        pub fn GetApplicationEventTarget() -> EventTargetRef;
        pub fn InstallEventHandler(target: EventTargetRef, handler: Handler, count: u32, types: *const EventTypeSpec,
                                   user: *mut c_void, out: *mut EventHandlerRef) -> OSStatus;
        pub fn RegisterEventHotKey(code: u32, modifiers: u32, id: EventHotKeyID, target: EventTargetRef, options: u32,
                                   out: *mut EventHotKeyRef) -> OSStatus;
        pub fn UnregisterEventHotKey(key: EventHotKeyRef) -> OSStatus;
        pub fn GetEventParameter(event: EventRef, name: u32, kind: u32, actual: *mut u32, size: usize, actual_size: *mut usize,
                                 data: *mut c_void) -> OSStatus;
    }
}

#[cfg(target_os = "macos")]
extern "C" fn on_hotkey(_call: carbon::EventHandlerCallRef, event: carbon::EventRef, _user: *mut std::ffi::c_void) -> carbon::OSStatus {
    let mut id = carbon::EventHotKeyID::default();
    let status = unsafe {
        carbon::GetEventParameter(event, carbon::PARAM_DIRECT_OBJECT, carbon::TYPE_HOTKEY_ID, std::ptr::null_mut(),
                                  std::mem::size_of::<carbon::EventHotKeyID>(), std::ptr::null_mut(), &mut id as *mut _ as *mut _)
    };
    if status == 0 && id.signature == carbon::SIGNATURE {
        let action = if id.id == Action::QuickFix as u32 { Action::QuickFix } else { Action::Open };
        APP_CX.with(|cell| {
            if let Some(acx) = cell.borrow().clone() {
                let acx_inner = acx.clone();
                acx.foreground_executor()
                    .spawn(async move {
                        let _ = acx_inner.update(|cx| match action {
                            Action::Open => crate::flow::open_panel(None, cx),
                            Action::QuickFix => crate::flow::quick_fix(None, cx),
                        });
                    })
                    .detach();
            }
        });
    }
    0
}

/// Windows: `RegisterHotKey` against a message-only window of Iris's own, whose messages GPUI's
/// loop hands on like any of its windows'. WM_HOTKEY's id is the action.
#[cfg(windows)]
mod win {
    use super::{Action, APP_CX};
    use crate::store::{carbon, KeyCombo};
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, FindWindowExW, PostMessageW, RegisterClassW, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
        WM_APP, WM_HOTKEY, WNDCLASSW,
    };

    /// Another app holds it (ERROR_HOTKEY_ALREADY_REGISTERED).
    const TAKEN: u32 = 1409;

    thread_local! {
        static WINDOW: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
    }

    /// Another copy of Iris was opened (the Start menu, say): this one shows Settings instead.
    const SHOW_SETTINGS: u32 = WM_APP + 7;

    /// Hands `f` to GPUI on the main thread's next turn, as the Mac's Carbon handler does (the
    /// window procedure runs inside Windows' message dispatch, not inside a GPUI update).
    fn run(f: impl FnOnce(&mut gpui::App) + 'static) {
        APP_CX.with(|cell| {
            if let Some(acx) = cell.borrow().clone() {
                let inner = acx.clone();
                acx.foreground_executor().spawn(async move { let _ = inner.update(f); }).detach();
            }
        });
    }

    /// The window class, one per copy of Iris (the installed one and a development one apart).
    fn class() -> Vec<u16> {
        format!("IrisHotkeys-{}", crate::store::bundle_id()).encode_utf16().chain([0]).collect()
    }

    /// Held for the lifetime of this process so a second launch while GPUI is still starting up
    /// (before `HWND_MESSAGE` exists) never races into running two copies and two llama-servers.
    static INSTANCE_MUTEX: std::sync::OnceLock<isize> = std::sync::OnceLock::new();

    /// Whether this copy of Iris is already running: if so it's asked to show Settings.
    pub fn already_running() -> bool {
        let mutex_name: Vec<u16> = format!("Local\\IrisInstance-{}", crate::store::bundle_id())
            .encode_utf16()
            .chain([0])
            .collect();
        let class = class();
        unsafe {
            if let Ok(handle) = CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr())) {
                let existed = GetLastError() == ERROR_ALREADY_EXISTS;
                if existed {
                    let _ = CloseHandle(handle);
                    // Wait briefly in case the first copy is still finishing GPUI startup before
                    // creating its message-only window, then ask it to open Settings.
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                    while std::time::Instant::now() < deadline {
                        if let Ok(h) = FindWindowExW(Some(HWND_MESSAGE), None, windows::core::PCWSTR(class.as_ptr()), None)
                            && !h.0.is_null()
                        {
                            let _ = PostMessageW(Some(h), SHOW_SETTINGS, WPARAM(0), LPARAM(0));
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    return true;
                }
                let _ = INSTANCE_MUTEX.set(handle.0 as isize);
                return false;
            }
            match FindWindowExW(Some(HWND_MESSAGE), None, windows::core::PCWSTR(class.as_ptr()), None) {
                Ok(h) if !h.0.is_null() => {
                    let _ = PostMessageW(Some(h), SHOW_SETTINGS, WPARAM(0), LPARAM(0));
                    true
                }
                _ => false,
            }
        }
    }

    unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == SHOW_SETTINGS {
            run(|cx| crate::open_settings(crate::settings::Page::General, cx));
            return LRESULT(0);
        }
        if msg == WM_HOTKEY {
            let action = if wparam.0 == Action::QuickFix as usize { Action::QuickFix } else { Action::Open };
            eprintln!("hotkey: {action:?} pressed");
            run(move |cx| match action {
                Action::Open => crate::flow::open_panel(None, cx),
                Action::QuickFix => crate::flow::quick_fix(None, cx),
            });
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn window() -> HWND {
        let made = WINDOW.with(|w| w.get());
        if made != 0 {
            return HWND(made as _);
        }
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let name = class();
            let class = WNDCLASSW { lpfnWndProc: Some(proc_), hInstance: instance.into(), lpszClassName: windows::core::PCWSTR(name.as_ptr()), ..Default::default() };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), windows::core::PCWSTR(name.as_ptr()), w!("Iris shortcuts"), WINDOW_STYLE(0), 0, 0, 0, 0,
                Some(HWND_MESSAGE), None, Some(instance.into()), None).unwrap_or_default();
            WINDOW.with(|w| w.set(hwnd.0 as isize));
            hwnd
        }
    }

    /// The virtual-key code of a stored key (`app::key_name`), none for keys a shortcut can't use.
    fn vk(k: KeyCombo) -> Option<u32> {
        let name = crate::app::key_name(k.key_code);
        let mut chars = name.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_uppercase() || c.is_ascii_digit() => Some(c as u32),
            _ => match name.as_str() {
                "Space" => Some(0x20),
                "↩" => Some(0x0D),
                "⇥" => Some(0x09),
                "esc" => Some(0x1B),
                "⌫" => Some(0x08),
                f if f.starts_with('F') => f[1..].parse::<u32>().ok().filter(|n| (1..=24).contains(n)).map(|n| 0x6F + n),
                _ => None,
            },
        }
    }

    fn mods(k: KeyCombo) -> HOT_KEY_MODIFIERS {
        let mut m = MOD_NOREPEAT;
        for (bit, flag) in [(carbon::CONTROL, MOD_CONTROL), (carbon::OPTION, MOD_ALT), (carbon::SHIFT, MOD_SHIFT), (carbon::CMD, MOD_WIN)] {
            if k.modifiers & bit != 0 {
                m |= flag;
            }
        }
        m
    }

    /// Registers `k` for `id`; Ok, or whether another app holds it.
    pub fn register(k: KeyCombo, id: i32) -> Result<(), bool> {
        let Some(code) = vk(k) else { return Err(false) };
        unsafe {
            RegisterHotKey(Some(window()), id, mods(k), code).map_err(|_| GetLastError().0 == TAKEN)
        }
    }

    /// Makes the window now, so a second copy can find this one before any shortcut is set.
    pub fn listen() {
        let _ = window();
    }

    pub fn unregister(id: i32) {
        unsafe {
            let _ = UnregisterHotKey(Some(window()), id);
        }
    }
}

thread_local! {
    #[cfg(target_os = "macos")]
    static REGISTERED: RefCell<Vec<carbon::EventHotKeyRef>> = const { RefCell::new(Vec::new()) };
    static INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Registers the shortcuts from the settings (dropping any registered before). Returns whether
/// the open shortcut is taken by another app.
pub fn apply(open: KeyCombo, quick_fix: Option<KeyCombo>) -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        if !INSTALLED.with(|i| i.get()) {
            let spec = carbon::EventTypeSpec { event_class: carbon::EVENT_CLASS_KEYBOARD, event_kind: carbon::EVENT_HOTKEY_PRESSED };
            let status = carbon::InstallEventHandler(carbon::GetApplicationEventTarget(), on_hotkey, 1, &spec, std::ptr::null_mut(), std::ptr::null_mut());
            if status == 0 {
                INSTALLED.with(|i| i.set(true));
            } else {
                eprintln!("hotkey: InstallEventHandler failed ({status})");
            }
        }
        REGISTERED.with(|r| {
            for k in r.borrow_mut().drain(..) {
                carbon::UnregisterEventHotKey(k);
            }
        });
        let register = |combo: KeyCombo, action: Action| -> carbon::OSStatus {
            let mut out: carbon::EventHotKeyRef = std::ptr::null_mut();
            let id = carbon::EventHotKeyID { signature: carbon::SIGNATURE, id: action as u32 };
            let status = carbon::RegisterEventHotKey(combo.key_code, combo.modifiers, id, carbon::GetApplicationEventTarget(), 0, &mut out);
            if status == 0 {
                REGISTERED.with(|r| r.borrow_mut().push(out));
            }
            status
        };
        let open_status = register(open, Action::Open);
        let taken = open_status != 0;
        if open_status != 0 && open_status != carbon::HOTKEY_EXISTS {
            eprintln!("hotkey: open shortcut registration failed ({open_status})");
        }
        if let Some(q) = quick_fix {
            let q_status = register(q, Action::QuickFix);
            if q_status != 0 {
                eprintln!("hotkey: quick-fix shortcut registration failed ({q_status})");
            }
        }
        taken
    }
    #[cfg(windows)]
    {
        let _ = &INSTALLED;
        win::unregister(Action::Open as i32);
        win::unregister(Action::QuickFix as i32);
        let taken = win::register(open, Action::Open as i32) == Err(true);
        if let Some(q) = quick_fix {
            let _ = win::register(q, Action::QuickFix as i32);
        }
        taken
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (open, quick_fix);
        false
    }
}

/// Registers the shortcuts and runs their handlers when they're pressed (Carbon calls back on
/// the main thread; the presses are handed to GPUI on the next tick).
pub fn start(cx: &mut App) {
    let s = crate::app::settings();
    crate::slip::set_keys(crate::app::shortcut_caps(s.open_shortcut), s.quick_fix_shortcut.map(crate::app::shortcut_caps));
    if crate::textbridge::under_test() {
        return;
    }
    let taken = apply(s.open_shortcut, s.quick_fix_shortcut);
    let keys = crate::keys::inline(s.open_shortcut);
    eprintln!("hotkey: {keys} {}", if taken { "is taken by another app" } else { "registered" });
    crate::app::set_shortcut_taken(taken);
    APP_CX.with(|cell| *cell.borrow_mut() = Some(cx.to_async()));
}

/// Lets go of the shortcuts while a recorder listens, so the current keys can be pressed again
/// (`HotkeyCenter.setPaused`), and takes them back after. Never under test.
pub fn set_paused(paused: bool) {
    if crate::textbridge::under_test() {
        return;
    }
    if paused {
        #[cfg(target_os = "macos")]
        REGISTERED.with(|r| {
            for k in r.borrow_mut().drain(..) {
                unsafe { carbon::UnregisterEventHotKey(k) };
            }
        });
        #[cfg(windows)]
        {
            win::unregister(Action::Open as i32);
            win::unregister(Action::QuickFix as i32);
        }
    } else {
        refresh();
    }
}

/// Registers the shortcuts as saved now (after a recorder saved new keys, or Restore
/// defaults), and tells the slip and setup which keys they are. Never registers under test.
pub fn refresh() {
    let s = crate::app::settings();
    crate::slip::set_keys(crate::app::shortcut_caps(s.open_shortcut), s.quick_fix_shortcut.map(crate::app::shortcut_caps));
    if crate::textbridge::under_test() {
        return;
    }
    let taken = apply(s.open_shortcut, s.quick_fix_shortcut);
    crate::app::set_shortcut_taken(taken);
}

/// Whether no other app holds `combo` (`HotkeyCenter.isAvailable`): registered for a moment and
/// let go. Under test, and off the Mac, every combo is free.
pub fn is_available(combo: KeyCombo) -> bool {
    if crate::textbridge::under_test() {
        return true;
    }
    #[cfg(target_os = "macos")]
    unsafe {
        let mut out: carbon::EventHotKeyRef = std::ptr::null_mut();
        let id = carbon::EventHotKeyID { signature: carbon::SIGNATURE, id: 99 };
        let status = carbon::RegisterEventHotKey(combo.key_code, combo.modifiers, id, carbon::GetApplicationEventTarget(), 0, &mut out);
        if status == 0 {
            carbon::UnregisterEventHotKey(out);
        }
        status == 0
    }
    #[cfg(windows)]
    {
        let free = win::register(combo, 99).is_ok();
        win::unregister(99);
        free
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = combo;
        true
    }
}

/// Windows: whether this copy of Iris already runs (it's asked to show Settings, and this one
/// should quit), and the window a later copy looks for, made at once.
#[cfg(windows)]
pub fn already_running() -> bool {
    win::already_running()
}

#[cfg(windows)]
pub fn listen() {
    win::listen()
}
