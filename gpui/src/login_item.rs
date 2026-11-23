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
    #[cfg(windows)]
    {
        win::status()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Status::NotFound
    }
}

/// Windows: Iris in the person's Run key (HKCU, no administrator needed), as "Iris" for the
/// installed copy and "Iris (development)" for another, pointing at this iris.exe. Task Manager's
/// Startup apps can turn it off without removing it (StartupApproved): then it's off here too.
#[cfg(windows)]
mod win {
    use super::Status;
    use windows::core::{PCWSTR, w};
    use windows::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
    };

    const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const APPROVED: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");

    fn name() -> Vec<u16> {
        let n = if crate::store::is_installed_iris() { "Iris" } else { "Iris (development)" };
        n.encode_utf16().chain([0]).collect()
    }

    fn command() -> Option<String> {
        Some(format!("\"{}\"", std::env::current_exe().ok()?.display()))
    }

    fn read_string(key: PCWSTR, name: &[u16]) -> Option<String> {
        let mut buf = vec![0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        unsafe { RegGetValueW(HKEY_CURRENT_USER, key, PCWSTR(name.as_ptr()), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as _), Some(&mut size)) }.ok().ok()?;
        let n = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..n]))
    }

    /// Turned off in Task Manager: StartupApproved's first byte is odd (3, 7…) when it's off.
    fn turned_off_in_task_manager(name: &[u16]) -> bool {
        let mut buf = [0u8; 12];
        let mut size = buf.len() as u32;
        let ok = unsafe { RegGetValueW(HKEY_CURRENT_USER, APPROVED, PCWSTR(name.as_ptr()), RRF_RT_REG_BINARY, None, Some(buf.as_mut_ptr() as _), Some(&mut size)) }.is_ok();
        ok && buf[0] & 1 == 1
    }

    pub fn status() -> Status {
        let n = name();
        match read_string(RUN, &n) {
            Some(v) if Some(&v) == command().as_ref() && !turned_off_in_task_manager(&n) => Status::Enabled,
            _ => Status::NotRegistered,
        }
    }

    pub fn set(on: bool) -> Result<(), String> {
        let n = name();
        unsafe {
            if on {
                let cmd: Vec<u16> = command().ok_or("no path")?.encode_utf16().chain([0]).collect();
                RegSetKeyValueW(HKEY_CURRENT_USER, RUN, PCWSTR(n.as_ptr()), REG_SZ.0, Some(cmd.as_ptr() as _), (cmd.len() * 2) as u32)
                    .ok().map_err(|e| e.to_string())?;
                // On again after Task Manager turned it off: Task Manager's switch goes with it.
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, APPROVED, PCWSTR(n.as_ptr()));
            } else {
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, PCWSTR(n.as_ptr()));
            }
        }
        Ok(())
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
    #[cfg(windows)]
    let result = win::set(on);
    #[cfg(not(any(target_os = "macos", windows)))]
    let result: Result<(), String> = Err("not on this system".into());
    let problem = match result {
        Ok(()) if on && status() == Status::RequiresApproval => Some("Approve Iris in System Settings → General → Login Items.".to_string()),
        Ok(()) => None,
        Err(e) => {
            eprintln!("login item: {e}");
            Some(crate::words::w("macOS didn't let Iris open at login. Add it in System Settings → General → Login Items.").to_string())
        }
    };
    PROBLEM.with(|p| *p.borrow_mut() = problem);
    forget();
}

pub fn problem() -> Option<String> {
    PROBLEM.with(|p| p.borrow().clone())
}
