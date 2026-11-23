//! Windows: the apps the Apps page offers (`appicon::installed` on the Mac lists the
//! Applications folders). Windows has no one folder of apps, so: every program the Start menu
//! starts (its shortcuts' targets), and every app with a window open now (Store apps such as
//! Notepad have no shortcut file). An app is known by its program file's name ("notepad.exe"),
//! as `textbridge::app_info` names the app a box is in.

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};
use windows::core::BOOL;

/// (program file name, name, path), by name. Iris itself and uninstallers are left out. Read
/// again at most every 30 seconds (resolving the Start menu's shortcuts takes a moment).
pub fn installed() -> Vec<(String, String, String)> {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(std::time::Instant, Vec<(String, String, String)>)>> = const { std::cell::RefCell::new(None) };
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((at, list)) = c.as_ref() && at.elapsed().as_secs() < 30 {
            return list.clone();
        }
        let list = read();
        *c = Some((std::time::Instant::now(), list.clone()));
        list
    })
}

/// Where the program `id` ("notepad.exe") is, if it's one of `installed`.
pub fn path_of(id: &str) -> Option<String> {
    installed().into_iter().find(|a| a.0 == id).map(|a| a.2)
}

fn read() -> Vec<(String, String, String)> {
    let mut apps: Vec<(String, String, String)> = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut add = |path: String, name: String| {
        let Some(id) = Path::new(&path).file_name().map(|f| f.to_string_lossy().to_lowercase()) else { return };
        if !id.ends_with(".exe") || id == "iris.exe" || id.starts_with("unins") || name.to_lowercase().contains("uninstall") {
            return;
        }
        if seen.insert(id.clone()) {
            apps.push((id, name, path));
        }
    };
    for (path, name) in start_menu() {
        add(path, name);
    }
    for path in open_apps() {
        let name = crate::textbridge::program_name(&path);
        add(path, name);
    }
    apps.sort_by_key(|a| a.1.to_lowercase());
    apps
}

/// The Start menu's shortcuts (yours and everyone's): each target program, by the shortcut's name.
fn start_menu() -> Vec<(String, String)> {
    let roots: Vec<PathBuf> = [("APPDATA", ""), ("ProgramData", "")].iter()
        .filter_map(|(var, _)| std::env::var(var).ok())
        .map(|r| PathBuf::from(r).join("Microsoft\\Windows\\Start Menu\\Programs"))
        .collect();
    let mut links = vec![];
    for root in roots {
        collect(&root, &mut links, 0);
    }
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
    unsafe {
        let _com = ComGuard(CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok());
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) else { return vec![] };
        let Ok(file) = link.cast::<IPersistFile>() else { return vec![] };
        links.into_iter().filter_map(|lnk| {
            file.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).ok()?;
            let mut buf = [0u16; 1024];
            link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
            let target = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]);
            let name = lnk.file_stem()?.to_string_lossy().to_string();
            (!target.is_empty()).then_some((target, name))
        }).collect()
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 3 {
            collect(&p, out, depth + 1);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
            out.push(p);
        }
    }
}

/// The programs of the windows open now (a visible window of their own with a title).
fn open_apps() -> Vec<String> {
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
        unsafe {
            let pids = &mut *(data.0 as *mut Vec<u32>);
            let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null());
            if IsWindowVisible(hwnd).as_bool() && !owned && GetWindowTextLengthW(hwnd) > 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid != 0 && pid != std::process::id() && !pids.contains(&pid) {
                    pids.push(pid);
                }
            }
        }
        BOOL(1)
    }
    let mut pids: Vec<u32> = vec![];
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut pids as *mut _ as isize));
    }
    pids.into_iter().filter_map(|p| crate::textbridge::exe_path(p as i32)).collect()
}
