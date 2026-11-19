//! Clicks and key presses anywhere on the Mac, watched (never swallowed: they still go where
//! they were meant to), as the Swift app's `NSEvent.addGlobalMonitorForEvents`: a click
//! elsewhere closes the slip like a menu, and the next click or key puts a problem message away.

use std::cell::RefCell;

thread_local! {
    #[cfg(target_os = "macos")]
    static MONITORS: RefCell<Vec<(u64, objc2::rc::Retained<objc2::runtime::AnyObject>)>> = const { RefCell::new(Vec::new()) };
    static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// What to watch.
#[derive(Clone, Copy)]
pub enum Watch {
    Clicks,
    ClicksAndKeys,
}

/// Starts watching; `on` runs (on the main thread) for each event. Returns a number for `stop`.
pub fn watch(what: Watch, on: impl Fn() + 'static) -> u64 {
    let id = NEXT.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    #[cfg(target_os = "macos")]
    {
        use block2::RcBlock;
        use objc2_app_kit::{NSEvent, NSEventMask};
        let mut mask = NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown;
        if matches!(what, Watch::ClicksAndKeys) {
            mask |= NSEventMask::KeyDown;
        }
        let block = RcBlock::new(move |_event: std::ptr::NonNull<NSEvent>| on());
        if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block) {
            MONITORS.with(|ms| ms.borrow_mut().push((id, m)));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (what, on);
    id
}

/// Stops watching.
pub fn stop(id: u64) {
    #[cfg(target_os = "macos")]
    MONITORS.with(|ms| {
        let mut ms = ms.borrow_mut();
        if let Some(i) = ms.iter().position(|(n, _)| *n == id) {
            let (_, m) = ms.remove(i);
            unsafe { objc2_app_kit::NSEvent::removeMonitor(&m) };
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = id;
}
