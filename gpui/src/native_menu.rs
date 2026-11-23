//! A native menu popped up under a control (`Menu` in SwiftUI, which Settings' dropdowns use):
//! the choices, the current one ticked, the pick handed back. The menu runs its own tracking
//! loop, so `pop` returns once a choice is made or the menu is dismissed. The Mac's NSMenu;
//! Windows' popup menu (without the apps' icons).

use gpui::{Pixels, Point, Window};

/// A menu item's picture: none, an app's own icon (the app at this path), or a PNG. (Windows'
/// menus show the words only.)
#[derive(Clone, Debug)]
#[cfg_attr(windows, allow(dead_code))]
pub enum Picture {
    None,
    App(String),
    Png(Vec<u8>),
}

/// One item: its words and its picture.
#[derive(Clone, Debug)]
pub struct Item {
    pub title: String,
    pub picture: Picture,
}

impl Item {
    pub fn plain(title: impl Into<String>) -> Item {
        Item { title: title.into(), picture: Picture::None }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{NSImage, NSMenu, NSMenuItem, NSWorkspace};
    use objc2_foundation::{NSData, NSPoint, NSSize, NSString};

    use super::{Item, Picture};

    pub static PICKED: AtomicIsize = AtomicIsize::new(-1);

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "IrisMenuTarget"]
        pub struct Target;

        impl Target {
            #[unsafe(method(picked:))]
            fn picked(&self, sender: &NSMenuItem) {
                PICKED.store(sender.tag(), Ordering::SeqCst);
            }
        }
    );

    /// Pops a menu of `items` at `at` (points from the window's top left) in the window whose
    /// content view is `view`, its top left there (under the control, as Swift's menus hang);
    /// `selected` is ticked. The index picked, if any.
    pub fn pop(view: *mut AnyObject, items: &[Item], selected: Option<usize>, at: (f64, f64)) -> Option<usize> {
        let mtm = MainThreadMarker::new()?;
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let target: Retained<Target> = unsafe { msg_send![Target::alloc(mtm), init] };
        for (i, it) in items.iter().enumerate() {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), &NSString::from_str(&it.title), Some(sel!(picked:)), &NSString::from_str(""))
            };
            // Each app with its own icon, so it's found by sight as well as by name.
            let image: Option<Retained<NSImage>> = match &it.picture {
                Picture::None => None,
                Picture::App(path) => Some(NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path))),
                Picture::Png(bytes) => NSImage::initWithData(mtm.alloc::<NSImage>(), &NSData::with_bytes(bytes)),
            };
            if let Some(img) = image {
                img.setSize(NSSize::new(16.0, 16.0));
                item.setImage(Some(&img));
            }
            unsafe { item.setTarget(Some(&target)) };
            item.setTag(i as isize);
            if selected == Some(i) {
                item.setState(1);
            }
            menu.addItem(&item);
        }
        PICKED.store(-1, Ordering::SeqCst);
        unsafe {
            let flipped: bool = msg_send![view, isFlipped];
            let height: f64 = {
                let frame: objc2_foundation::NSRect = msg_send![view, frame];
                frame.size.height
            };
            let point = NSPoint::new(at.0, if flipped { at.1 } else { height - at.1 });
            let _: bool = msg_send![&menu, popUpMenuPositioningItem: None::<&NSMenuItem>, atLocation: point, inView: view];
        }
        let picked = PICKED.load(Ordering::SeqCst);
        (picked >= 0).then_some(picked as usize)
    }
}

/// The window's content view, to pop a menu in later (`pop_in`), once the event that asked for
/// it has let go of the app: the menu runs its own loop, and GPUI's tasks keep running in it.
pub fn view_of(window: &Window) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let h = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(a) = h.as_raw() else { return None };
        Some(a.ns_view.as_ptr() as usize)
    }
    // Windows: the window itself (HWND).
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let h = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Win32(w) = h.as_raw() else { return None };
        Some(w.hwnd.get() as usize)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = window;
        None
    }
}

/// Pops a native menu of `items` at `at` in the content view `view` (`view_of`), `selected`
/// ticked; the index picked. Never call it while the app is borrowed (from inside a handler).
pub fn pop_in(view: usize, items: &[Item], selected: Option<usize>, at: Point<Pixels>) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        mac::pop(view as *mut _, items, selected, (f32::from(at.x) as f64, f32::from(at.y) as f64))
    }
    #[cfg(windows)]
    {
        win::pop(view, items, selected, (f32::from(at.x), f32::from(at.y)))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (view, items, selected, at);
        None
    }
}

#[cfg(windows)]
mod win {
    use super::Item;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, PostMessageW, SetForegroundWindow, TrackPopupMenu, MF_CHECKED, MF_STRING,
        TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, WM_NULL,
    };

    /// `items` in a popup menu at `at` (points from the window's top left), `selected` ticked.
    pub fn pop(hwnd: usize, items: &[Item], selected: Option<usize>, at: (f32, f32)) -> Option<usize> {
        unsafe {
            let hwnd = HWND(hwnd as _);
            let menu = CreatePopupMenu().ok()?;
            for (i, it) in items.iter().enumerate() {
                let flags = if selected == Some(i) { MF_STRING | MF_CHECKED } else { MF_STRING };
                let _ = AppendMenuW(menu, flags, i + 1, &HSTRING::from(it.title.as_str()));
            }
            let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
            let mut p = POINT { x: (at.0 * scale).round() as i32, y: (at.1 * scale).round() as i32 };
            let _ = ClientToScreen(hwnd, &mut p);
            // A popup menu closes when its window isn't in front, and needs a message after it
            // (Microsoft's own advice for TrackPopupMenu).
            let _ = SetForegroundWindow(hwnd);
            let picked = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON, p.x, p.y, None, hwnd, None);
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            (picked.0 > 0).then(|| picked.0 as usize - 1)
        }
    }

    /// Windows' Open dialog on Program Files, for a program: (file name, its name).
    pub fn choose_program() -> Option<(String, String)> {
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW};
        let filter: Vec<u16> = "Programs (*.exe)\0*.exe\0\0".encode_utf16().collect();
        let dir: Vec<u16> = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into()).encode_utf16().chain([0]).collect();
        let title: Vec<u16> = "Choose an app".encode_utf16().chain([0]).collect();
        let mut buf = [0u16; 1024];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(buf.as_mut_ptr()),
            nMaxFile: buf.len() as u32,
            lpstrInitialDir: PCWSTR(dir.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
            ..Default::default()
        };
        if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]);
        let id = std::path::Path::new(&path).file_name()?.to_string_lossy().to_lowercase();
        Some((id, crate::textbridge::program_name(&path)))
    }
}

/// `NSOpenPanel` on /Applications (Windows: its Open dialog for a program), for an app that isn't listed: (bundle ID, name) of the app
/// chosen. Runs its own loop, like a menu (never call it from inside a handler). Under test it
/// opens nothing and answers none: a panel would come up on someone's screen.
pub fn choose_app() -> Option<(String, String)> {
    if crate::textbridge::under_test() {
        eprintln!("apps: Choose another app… would open the Mac's file panel here");
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
        use objc2_foundation::{NSArray, NSBundle, NSString, NSURL};
        let mtm = MainThreadMarker::new()?;
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str("/Applications"))));
        #[allow(deprecated)]
        panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&[NSString::from_str("app")])));
        if panel.runModal() != NSModalResponseOK {
            return None;
        }
        let path = panel.URL()?.path()?.to_string();
        let id = NSBundle::bundleWithPath(&NSString::from_str(&path))?.bundleIdentifier()?.to_string();
        Some((id, crate::appicon::display_name(&path)))
    }
    #[cfg(windows)]
    {
        win::choose_program()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}
