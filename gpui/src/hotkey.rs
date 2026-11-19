//! The global shortcuts (`HotkeyCenter` in the Swift app): the open shortcut and Quick Fix,
//! registered with Carbon's `RegisterEventHotKey`, the same key code and modifier mask the
//! settings store. A shortcut another app already holds can't be registered: that's the
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
    static PRESSED: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
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
        PRESSED.with(|p| p.borrow_mut().push(action));
    }
    0
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
        if !INSTALLED.with(|i| i.replace(true)) {
            let spec = carbon::EventTypeSpec { event_class: carbon::EVENT_CLASS_KEYBOARD, event_kind: carbon::EVENT_HOTKEY_PRESSED };
            carbon::InstallEventHandler(carbon::GetApplicationEventTarget(), on_hotkey, 1, &spec, std::ptr::null_mut(), std::ptr::null_mut());
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
        let taken = register(open, Action::Open) == carbon::HOTKEY_EXISTS;
        if let Some(q) = quick_fix {
            register(q, Action::QuickFix);
        }
        taken
    }
    #[cfg(not(target_os = "macos"))]
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
    let keys = crate::app::shortcut_caps(s.open_shortcut).concat();
    eprintln!("hotkey: {keys} {}", if taken { "is taken by another app" } else { "registered" });
    crate::app::set_shortcut_taken(taken);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(15)).await;
            let pressed: Vec<Action> = PRESSED.with(|p| p.borrow_mut().drain(..).collect());
            for action in pressed {
                let _ = cx.update(|cx| match action {
                    Action::Open => crate::flow::open_panel(None, cx),
                    Action::QuickFix => crate::flow::quick_fix(None, cx),
                });
            }
        }
    })
    .detach();
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
        status != carbon::HOTKEY_EXISTS
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = combo;
        true
    }
}
