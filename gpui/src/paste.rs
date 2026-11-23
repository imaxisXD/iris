//! Replace: the answer pasted over the text Iris read (`TextBridge.paste`, `Paste.replace`).
//!
//! The person's clipboard is kept first; the answer goes on it marked transient (clipboard
//! managers skip it); the app the text came from comes back to the front; once it has the
//! keyboard, ⌘A (for a whole box) and ⌘V; and 450 ms later the clipboard is put back, unless
//! something else wrote to it meanwhile. If the app never takes the keyboard back (it quit, or
//! something else came forward), nothing is pasted and the answer stays on the clipboard, so it
//! never lands in another app. Under test, the last step is done through Accessibility instead,
//! never touching the clipboard or the keyboard (`TextBridge.writeForVerify`).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::time::Duration;

use gpui::*;

use crate::live::news;
use crate::textbridge::{self, Captured, Clipboard, Scope};

type QueuedReplace = Box<dyn FnOnce(&mut App)>;

thread_local! {
    /// Serialises pastes so two Replaces never fight over the clipboard (Swift's `lastReplace`).
    static REPLACING: Cell<bool> = const { Cell::new(false) };
    static QUEUED: RefCell<VecDeque<QueuedReplace>> = const { RefCell::new(VecDeque::new()) };
}

fn finish_replace(cx: &mut App) {
    if let Some(next) = QUEUED.with(|q| q.borrow_mut().pop_front()) {
        next(cx);
    } else {
        REPLACING.with(|r| r.set(false));
    }
}

/// Pastes `text` over what `captured` read. `then` hears whether it landed.
pub fn replace_then(captured: Captured, text: String, cx: &mut App, then: impl FnOnce(bool, &mut App) + 'static) {
    // Iris's own box (setup's practice box): written directly, an edit of its own.
    if captured.own {
        let ok = crate::live_setup::write_practice(&text, cx);
        news(if ok { "replaced practice box" } else { "replace failed" });
        then(ok, cx);
        return;
    }
    if textbridge::under_test() {
        let ok = textbridge::write_for_test(&captured, &text);
        news(&if ok { format!("replaced {}", if captured.scope == Scope::WholeField { "box" } else { "selection" }) } else { "replace failed".into() });
        then(ok, cx);
        return;
    }
    if REPLACING.with(|r| r.replace(true)) {
        QUEUED.with(|q| {
            q.borrow_mut().push_back(Box::new(move |cx| start_replace(captured, text, cx, then)));
        });
        return;
    }
    start_replace(captured, text, cx, then);
}

fn start_replace(captured: Captured, text: String, cx: &mut App, then: impl FnOnce(bool, &mut App) + 'static) {
    cx.spawn(async move |cx| {
        let _borrow = textbridge::begin_replace_borrow();
        let saved = Clipboard::take();
        let ours = textbridge::put_on_clipboard(&text, true);
        textbridge::activate(captured.pid);
        // Iris is a non-activating panel, so the target app usually already has focus once the
        // panel gives up keys; yield briefly instead of sleeping a flat 120ms, polling up to ~600ms.
        cx.background_executor().timer(Duration::from_millis(20)).await;
        let mut has = false;
        for attempt in 0..25 {
            if textbridge::has_keyboard(captured.pid) {
                has = true;
                break;
            }
            if attempt < 24 {
                cx.background_executor().timer(Duration::from_millis(25)).await;
            }
        }
        news(&format!("paste: {} has the keyboard: {has}", captured.pid));
        if !has {
            // Not transient any more: it's the person's to paste now.
            textbridge::put_on_clipboard(&text, false);
            drop(_borrow);
            let _ = cx.update(|cx| {
                then(false, cx);
                finish_replace(cx);
            });
            return;
        }
        let pid = captured.pid;
        let whole = captured.scope == Scope::WholeField;
        let pasted = cx.background_executor().spawn(async move { textbridge::select_all_and_paste(pid, whole) }).await;
        if !pasted {
            textbridge::put_on_clipboard(&text, false);
            drop(_borrow);
            let _ = cx.update(|cx| {
                then(false, cx);
                finish_replace(cx);
            });
            return;
        }
        let _ = cx.update(|cx| then(true, cx));
        // Give the app time to read the clipboard before putting the person's back.
        cx.background_executor().timer(Duration::from_millis(450)).await;
        if textbridge::clipboard_count() == ours {
            saved.restore();
        }
        drop(_borrow);
        let _ = cx.update(finish_replace);
    })
    .detach();
}
