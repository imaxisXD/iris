//! How high Iris's windows by your text stand (`HostLevel`): above the window that holds the
//! text box, never under it.
//!
//! Iris's windows by your text (the panel and its flower, the notes, the keep-word question) are
//! GPUI pop-ups, which stand at the pop-up level (101): over ordinary windows and over a menu bar
//! popover. An app can raise its own windows higher still (a launcher, an overlay), and typing in
//! one of those, Iris opened behind it. So when the window under the text box stands at or above
//! Iris's level, Iris's window goes one above it (the same level isn't enough: the host comes
//! back on top the moment it's ordered front again), and never above the screen saver.

/// One window as the window server lists it, front to back: its owner's pid, its level, its
/// frame in points from the top left of the main screen (as the text box's frame is), and how
/// see-through it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostWindow {
    pub owner: i32,
    pub layer: isize,
    pub bounds: [f64; 4],
    pub alpha: f64,
}

/// Where Iris's windows by your text stand anyway: GPUI's pop-up level.
pub const POP_UP: isize = 101;
/// Under the screen saver (1000) and what the system keeps above everything.
pub const CEILING: isize = 999;

/// The window `owner` shows at `point`: the frontmost of its visible windows there.
pub fn host(owner: i32, point: (f64, f64), windows: &[HostWindow]) -> Option<HostWindow> {
    windows.iter().copied().find(|w| {
        let [x, y, wd, h] = w.bounds;
        w.owner == owner && w.alpha > 0.0 && w.layer < CEILING && point.0 >= x && point.0 < x + wd && point.1 >= y && point.1 < y + h
    })
}

/// The level for Iris's windows over the text at `point` in `owner`'s window: one above the
/// host, at most `CEILING`. None when Iris's own level already stands above it (or there's no
/// window of `owner` there), so nothing changes.
pub fn level(owner: i32, point: (f64, f64), windows: &[HostWindow]) -> Option<isize> {
    let layer = host(owner, point, windows)?.layer;
    (layer >= POP_UP).then(|| (layer + 1).min(CEILING))
}

thread_local! {
    /// The app whose text box Iris last read: the notes and the keep-word question that follow
    /// stand over its window too.
    static HOST: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// Iris read a text box in `pid`'s window.
pub fn set_host(pid: i32) {
    HOST.with(|h| h.set(Some(pid)));
}

/// Raises `window` above the window that holds the text box: `owner`'s (the app last read when
/// none), under `near` (the box's frame), or under the pointer.
pub fn raise(window: &gpui::Window, owner: Option<i32>, near: Option<[f64; 4]>) {
    let Some(owner) = owner.or_else(|| HOST.with(|h| h.get())) else { return };
    let point = near.map_or_else(crate::menubar::pointer, |[x, y, w, h]| (x + w / 2.0, y + h / 2.0));
    let Some(level) = level(owner, point, &on_screen()) else { return };
    crate::live::news(&format!("raised to level {level}, over the box's window"));
    set_level(window, level);
}

/// Visible windows, front to back. Their level, frame and owner need no Screen Recording.
#[cfg(target_os = "macos")]
pub fn on_screen() -> Vec<HostWindow> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::geometry::CGRect;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowAlpha, kCGWindowBounds, kCGWindowLayer,
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID,
    };
    let Some(list) = copy_window_info(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID) else {
        return vec![];
    };
    let key = |k| unsafe { CFString::wrap_under_get_rule(k) };
    let (owner_k, layer_k, bounds_k, alpha_k) =
        unsafe { (key(kCGWindowOwnerPID), key(kCGWindowLayer), key(kCGWindowBounds), key(kCGWindowAlpha)) };
    list.iter()
        .filter_map(|item| {
            let d: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
            let num = |k: &CFString| d.find(k).and_then(|v| v.downcast::<CFNumber>());
            let owner = num(&owner_k)?.to_i64()? as i32;
            let layer = num(&layer_k)?.to_i64()? as isize;
            let alpha = num(&alpha_k).and_then(|n| n.to_f64()).unwrap_or(1.0);
            let bounds = d.find(&bounds_k)?.downcast::<CFDictionary>()?;
            let r = CGRect::from_dict_representation(&bounds)?;
            Some(HostWindow { owner, layer, bounds: [r.origin.x, r.origin.y, r.size.width, r.size.height], alpha })
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn on_screen() -> Vec<HostWindow> {
    vec![]
}

#[cfg(target_os = "macos")]
fn set_level(window: &gpui::Window, level: isize) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(h) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::AppKit(a) = h.as_raw() else { return };
    unsafe {
        let view = a.ns_view.as_ptr() as *mut AnyObject;
        let w: *mut AnyObject = msg_send![view, window];
        if !w.is_null() {
            let _: () = msg_send![w, setLevel: level];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_level(_window: &gpui::Window, _level: isize) {}

#[cfg(test)]
mod tests {
    use super::*;

    const CALENDAR: i32 = 501;
    const OTHER: i32 = 77;
    const POPOVER: [f64; 4] = [400.0, 40.0, 640.0, 1200.0];
    const BOX: (f64, f64) = (700.0, 650.0);

    fn w(owner: i32, layer: isize, bounds: [f64; 4]) -> HostWindow {
        HostWindow { owner, layer, bounds, alpha: 1.0 }
    }

    /// The bug: typing in a window an app raised above the pop-up level, Iris opened behind it.
    /// The host's level decides, plus one.
    #[test]
    fn stands_above_a_raised_host_window() {
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 102, POPOVER)]), Some(103));
        // A menu bar popover, at Iris's own level: one above, so it can't come back on top.
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 101, POPOVER)]), Some(102));
    }

    /// Ordinary and floating windows: Iris already stands above them, nothing changes.
    #[test]
    fn ordinary_windows_change_nothing() {
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 0, POPOVER)]), None);
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 25, POPOVER)]), None);
    }

    /// The frontmost of the app's windows under the text box, not its menu bar item or a window
    /// somewhere else, and never another app's.
    #[test]
    fn picks_the_apps_window_under_the_box() {
        let windows = [
            w(OTHER, 500, POPOVER),
            w(CALENDAR, 25, [1400.0, 0.0, 30.0, 24.0]),
            w(CALENDAR, 102, POPOVER),
            w(CALENDAR, 0, [0.0, 0.0, 2000.0, 1800.0]),
        ];
        assert_eq!(level(CALENDAR, BOX, &windows), Some(103));
    }

    /// See-through windows (an app's invisible overlay) don't count.
    #[test]
    fn skips_invisible_windows() {
        let windows = [HostWindow { alpha: 0.0, ..w(CALENDAR, 900, POPOVER) }, w(CALENDAR, 0, POPOVER)];
        assert_eq!(level(CALENDAR, BOX, &windows), None);
    }

    #[test]
    fn no_host_window_changes_nothing() {
        assert_eq!(level(CALENDAR, BOX, &[]), None);
    }

    /// Never above the screen saver, and a window at the system's own levels isn't a host.
    #[test]
    fn stays_under_the_screen_saver() {
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 998, POPOVER)]), Some(CEILING));
        assert_eq!(level(CALENDAR, BOX, &[w(CALENDAR, 5000, POPOVER)]), None);
    }
}
