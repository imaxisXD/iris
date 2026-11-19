//! A native Mac menu popped up under a control (`Menu` in SwiftUI, which Settings' dropdowns
//! use): the choices, the current one ticked, the pick handed back. The menu runs its own
//! tracking loop, so `pop` returns once a choice is made or the menu is dismissed.

use gpui::{Pixels, Point, Window};

/// A menu item's picture: none, an app's own icon (the app at this path), or a PNG.
#[derive(Clone, Debug)]
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
    #[cfg(not(target_os = "macos"))]
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (view, items, selected, at);
        None
    }
}

/// `NSOpenPanel` on /Applications, for an app that isn't listed: (bundle ID, name) of the app
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
    #[cfg(not(target_os = "macos"))]
    None
}
