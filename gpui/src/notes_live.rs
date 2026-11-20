//! The messages Iris leaves by your text, live (`Toast.show`): one at a time, on a slip of
//! paper above the text box's top right corner (`FloatingNote.origin`), for as long as the
//! message stays, never taking the keyboard. A message whose switch is off in Settings
//! (`hidden_notes`) still clears the one before it.

use std::time::Duration;

use gpui::*;

use crate::live::news;
use crate::toast::{self, Message};

thread_local! {
    static SHOWN: std::cell::RefCell<Option<(AnyWindowHandle, u64)>> = const { std::cell::RefCell::new(None) };
    static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The room round the paper (`.padding(14)`), for its shadow.
const PAD: f32 = 14.0;

struct NoteView {
    message: Message,
    born: std::time::Instant,
    /// Going: when it started fading out.
    leaving: Option<std::time::Instant>,
}

impl Render for NoteView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // `FloatingNote.fadeIn` / `fadeOut`: in over `DS.Motion.fast.enter`, out one step quicker.
        let alpha = match self.leaving {
            Some(l) => 1.0 - crate::parts::ds_curve(l.elapsed().as_secs_f32() / crate::parts::slow() / 0.12),
            None => crate::parts::ds_curve(self.born.elapsed().as_secs_f32() / crate::parts::slow() / 0.15),
        };
        if self.leaving.is_some() || self.born.elapsed().as_secs_f32() < 0.2 * crate::parts::slow() {
            window.request_animation_frame();
        }
        div().size_full().opacity(alpha).child(div().p(px(PAD)).child(toast::toast(&self.message, window.scale_factor())))
    }
}

thread_local! {
    /// The watch for the next click or key, while a problem message is up.
    static WATCH: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Puts away the message showing, if any: it fades out, then goes.
pub fn dismiss(cx: &mut App) {
    if let Some(id) = WATCH.with(|w| w.take()) {
        crate::monitor::stop(id);
    }
    if let Some((h, _)) = SHOWN.with(|s| s.borrow_mut().take()) {
        let Some(view) = h.downcast::<NoteView>() else {
            let _ = h.update(cx, |_, window, _| window.remove_window());
            return;
        };
        let _ = view.update(cx, |v, _, cx| {
            v.leaving = Some(std::time::Instant::now());
            cx.notify();
        });
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs_f32(0.13 * crate::parts::slow())).await;
            let _ = cx.update(|cx| h.update(cx, |_, window, _| window.remove_window()));
        })
        .detach();
    }
}

/// Shows `message` by the text box at `near` (screen points, top-left origin), or by the
/// pointer. `note` names the switch that can turn it off ("undo", "undoKeys", "checking",
/// "clean", "excluded").
pub fn show(message: Message, note: Option<&str>, near: Option<[f64; 4]>, cx: &mut App) {
    dismiss(cx);
    news(&format!("note {}", toast::spoken(&message)));
    if let Some(n) = note
        && !crate::app::settings().shows(n)
    {
        return;
    }
    let (w, h) = toast::size(&message);
    let (ww, wh) = (w + 2.0 * PAD, h + 2.0 * PAD);
    // The point the message belongs to: the box, or the pointer. Its screen keeps it in view.
    let pointer = if near.is_none() && !crate::textbridge::under_test() { Some(crate::menubar::pointer()) } else { None };
    let origin = match near {
        // Its right edge 14 past the box's, its bottom 4 into the box's top.
        Some([x, y, bw, _]) => (x + bw - ww as f64 + PAD as f64, y + 4.0 - wh as f64),
        // Under test, never by the pointer (it's on someone's screen): the test display's middle.
        None if crate::textbridge::under_test() => {
            let (x, y) = crate::test_display().map(|(x, y)| (x + 960.0, y + 540.0)).unwrap_or((200.0, 200.0));
            (x - ww as f64 / 2.0, y - wh as f64 / 2.0)
        }
        None => {
            let (px_, py) = pointer.unwrap_or_default();
            (px_ - ww as f64 / 2.0, py - 8.0 - wh as f64)
        }
    };
    let origin = (origin.0, origin.1.max(25.0));
    // Kept on its screen sideways too: beside a narrow box near the screen's edge, or by the
    // pointer there, most of it fell off the screen (seen on a Mac and on Windows). Its shadow's
    // margin may hang over the edge, never the paper.
    let anchor = match (near, pointer) {
        (Some([x, y, bw, bh]), _) => Some((x + bw / 2.0, y + bh / 2.0)),
        (None, Some(p)) => Some(p),
        _ => None,
    };
    let origin = match anchor {
        Some((ax, ay)) => {
            let [vx, _, vw, _] = crate::visible_frame_at(ax, ay);
            let (lo, hi) = (vx - PAD as f64, vx + vw - ww as f64 + PAD as f64);
            (if hi > lo { origin.0.clamp(lo, hi) } else { origin.0 }, origin.1)
        }
        None => origin,
    };
    // Under test nothing opens on someone's screen: a place off the test display becomes its middle.
    let origin = match crate::test_display() {
        Some((dx, dy)) if crate::textbridge::under_test() && !crate::on_test_display(origin.0, origin.1) => {
            (dx + 960.0 - ww as f64 / 2.0, dy + 540.0 - wh as f64 / 2.0)
        }
        _ => origin,
    };
    let stays = message.stays();
    let warning = message.kind == toast::Kind::Warning;
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(origin.0 as f32), px(origin.1 as f32)), size: size(px(ww), px(wh)) })),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let Ok(handle) = cx.open_window(options, |window, cx| {
        crate::no_system_shadow(window);
        cx.new(|_| NoteView { message, born: std::time::Instant::now(), leaving: None })
    }) else { return };
    let id = NEXT.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    let any: AnyWindowHandle = handle.into();
    SHOWN.with(|s| *s.borrow_mut() = Some((any, id)));
    // A problem stays until the next click or key (watched, never swallowed); never under test,
    // where those clicks and keys are someone's work.
    if warning && !crate::textbridge::under_test() {
        let hit = std::rc::Rc::new(std::cell::Cell::new(false));
        let h2 = hit.clone();
        WATCH.with(|w| w.set(Some(crate::monitor::watch(crate::monitor::Watch::ClicksAndKeys, move || h2.set(true)))));
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let done = cx.update(|cx| {
                    let current = SHOWN.with(|s| s.borrow().as_ref().map(|x| x.1)) == Some(id);
                    if current && hit.get() {
                        dismiss(cx);
                    }
                    !current || hit.get()
                });
                if done {
                    return;
                }
            }
        })
        .detach();
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs_f32(stays)).await;
        let _ = cx.update(|cx| {
            if SHOWN.with(|s| s.borrow().as_ref().map(|x| x.1)) == Some(id) {
                dismiss(cx);
            }
        });
    })
    .detach();
}
