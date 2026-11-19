//! Open at login (`AppSettings.setLaunchAtLogin`): Iris as the Mac's login item, through
//! `SMAppService.mainApp`. It only works for an app in a bundle; outside one macOS says no, and
//! the row says what it said, in Iris's words.

use std::cell::RefCell;

thread_local! {
    /// What macOS still needs before Iris opens at login, if anything (`launchAtLoginProblem`).
    static PROBLEM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// `SMAppService.Status`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    NotRegistered,
    Enabled,
    RequiresApproval,
    NotFound,
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::msg_send;

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    fn main_app() -> Option<*mut AnyObject> {
        let class = AnyClass::get(c"SMAppService")?;
        let app: *mut AnyObject = unsafe { msg_send![class, mainAppService] };
        (!app.is_null()).then_some(app)
    }

    pub fn status() -> isize {
        match main_app() {
            Some(app) => unsafe { msg_send![app, status] },
            None => 3,
        }
    }

    /// Registers or unregisters; the error's words when macOS refused.
    pub fn set(on: bool) -> Result<(), String> {
        let Some(app) = main_app() else { return Err("no SMAppService".into()) };
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let ok: Bool = unsafe {
            if on { msg_send![app, registerAndReturnError: &mut error] } else { msg_send![app, unregisterAndReturnError: &mut error] }
        };
        if ok.as_bool() {
            return Ok(());
        }
        if error.is_null() {
            return Err("refused".into());
        }
        let words: *mut AnyObject = unsafe { msg_send![error, localizedDescription] };
        let s: *const std::ffi::c_char = unsafe { msg_send![words, UTF8String] };
        Err(if s.is_null() { "refused".into() } else { unsafe { std::ffi::CStr::from_ptr(s) }.to_string_lossy().into_owned() })
    }
}

pub fn status() -> Status {
    #[cfg(target_os = "macos")]
    {
        match mac::status() {
            0 => Status::NotRegistered,
            1 => Status::Enabled,
            2 => Status::RequiresApproval,
            _ => Status::NotFound,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Status::NotFound
    }
}

/// Whether Iris opens at login now. Asking macOS takes about 13 ms, so the answer is kept:
/// asked again after the switch is flipped, or `forget` (a window coming to the front: it may
/// have been changed in System Settings meanwhile).
pub fn enabled() -> bool {
    if let Some(on) = KNOWN.get() {
        return on;
    }
    let on = status() == Status::Enabled;
    KNOWN.set(Some(on));
    on
}

/// Asks macOS again next time (`enabled`).
pub fn forget() {
    KNOWN.set(None);
}

thread_local! {
    static KNOWN: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Turns it on or off, and remembers what macOS still needs, if anything.
pub fn set(on: bool) {
    #[cfg(target_os = "macos")]
    let result = mac::set(on);
    #[cfg(not(target_os = "macos"))]
    let result: Result<(), String> = Err("not on this system".into());
    let problem = match result {
        Ok(()) if on && status() == Status::RequiresApproval => Some("Approve Iris in System Settings → General → Login Items.".to_string()),
        Ok(()) => None,
        Err(e) => {
            eprintln!("login item: {e}");
            Some("macOS didn't let Iris open at login. Add it in System Settings → General → Login Items.".to_string())
        }
    };
    PROBLEM.with(|p| *p.borrow_mut() = problem);
    forget();
}

pub fn problem() -> Option<String> {
    PROBLEM.with(|p| p.borrow().clone())
}
