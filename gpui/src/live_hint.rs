//! The note under the menu bar flower (`MenuBarHint`): "find me up here" and the keys that open
//! Iris, said once when setup finishes (or other news from the menu bar). It hangs from the
//! flower by its tail for eight seconds, fading in and out as the messages do, and goes at once
//! on a click anywhere. Never takes the keyboard. Windows has no menu bar: there the news is a
//! message standing on the taskbar by the clock (`show`).
#![cfg_attr(windows, allow(dead_code, unused_imports))]

use std::time::{Duration, Instant};

use gpui::*;

use crate::live::news;

/// Room round the note for its shadow (`StatusMenu.shadowRoom`).
const SIDE: f32 = 30.0;
const BOTTOM: f32 = 34.0;

struct HintView {
    keys: Vec<String>,
    lead: Option<String>,
    detail: Option<String>,
    born: Instant,
    leaving: Option<Instant>,
}

impl Render for HintView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::riso::let_go_of_last_frame(window);
        let slow = crate::parts::slow();
        let alpha = match self.leaving {
            Some(l) => 1.0 - crate::parts::ds_curve(l.elapsed().as_secs_f32() / slow / 0.12),
            None => crate::parts::ds_curve(self.born.elapsed().as_secs_f32() / slow / 0.15),
        };
        if self.leaving.is_some() || self.born.elapsed().as_secs_f32() < 0.2 * slow {
            window.request_animation_frame();
        }
        let keys: Vec<&str> = self.keys.iter().map(String::as_str).collect();
        let note = crate::toast::hint_with(&keys, self.lead.as_deref(), self.detail.as_deref(), 0.0, window.scale_factor());
        div().size_full().opacity(alpha).child(div().pl(px(SIDE)).child(note))
    }
}

thread_local! {
    static SHOWN: std::cell::RefCell<Option<(WindowHandle<HintView>, u64)>> = const { std::cell::RefCell::new(None) };
    static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static WATCH: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Lets the note go: it fades out, then its window goes.
pub fn close(cx: &mut App) {
    if let Some(id) = WATCH.with(|w| w.take()) {
        crate::monitor::stop(id);
    }
    let Some((h, _)) = SHOWN.with(|s| s.borrow_mut().take()) else { return };
    let _ = h.update(cx, |v, _, cx| {
        v.leaving = Some(Instant::now());
        cx.notify();
    });
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs_f32(0.13 * crate::parts::slow())).await;
        let _ = cx.update(|cx| h.update(cx, |_, window, _| window.remove_window()));
    })
    .detach();
}

/// "find me up here", with the keys that open Iris.
pub fn find_me(cx: &mut App) {
    let keys = crate::app::shortcut_caps(crate::app::settings().open_shortcut);
    show(keys, None, None, cx);
}

/// Hangs the note from the flower: `lead` and `detail` in place of "find me up here" and "In
/// any text box, press" for other news.
pub fn show(keys: Vec<String>, lead: Option<String>, detail: Option<String>, cx: &mut App) {
    // Windows: no menu bar to hang from; the news is a message standing on the taskbar at the
    // bottom right, by the clock where Iris's icon is.
    #[cfg(windows)]
    {
        let lead = lead.unwrap_or_else(|| "find me by the clock".into());
        let detail = detail.unwrap_or_else(|| "In any text box, press".into());
        let [vx, vy, vw, vh] = crate::visible_frame_at(1.0, 1.0);
        let corner = [vx + vw - 13.0, vy + vh - 8.0, 1.0, 1.0];
        crate::notes_live::show(crate::toast::Message::news(&lead, &detail, keys, 8.0), None, Some(corner), cx);
        return;
    }
    #[cfg(not(windows))]
    show_under_flower(keys, lead, detail, cx)
}

#[cfg(not(windows))]
fn show_under_flower(keys: Vec<String>, lead: Option<String>, detail: Option<String>, cx: &mut App) {
    close(cx);
    let [x, y, w, h] = crate::menubar::anchor();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let (nw, nh) = crate::toast::hint_size(&refs, lead.as_deref().unwrap_or("find me up here"), detail.as_deref().unwrap_or("In any text box, press"));
    let (ww, wh) = (nw + 2.0 * SIDE, nh + BOTTOM);
    // Centred under the flower, its tail's tip 2 points below the bar.
    let origin = point(px((x + w / 2.0) as f32 - ww / 2.0), px((y + h + 2.0) as f32));
    news(&format!("menu bar note: {}", lead.as_deref().unwrap_or("find me up here")));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(ww), px(wh)) })),
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
        cx.new(|_| HintView { keys, lead, detail, born: Instant::now(), leaving: None })
    }) else { return };
    let id = NEXT.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    SHOWN.with(|s| *s.borrow_mut() = Some((handle, id)));
    let current = move || SHOWN.with(|s| s.borrow().as_ref().map(|x| x.1)) == Some(id);
    // Any click goes (watched, never swallowed); never under test, where clicks are someone's.
    if !crate::textbridge::under_test() {
        let hit = std::rc::Rc::new(std::cell::Cell::new(false));
        let h2 = hit.clone();
        WATCH.with(|w| w.set(Some(crate::monitor::watch(crate::monitor::Watch::Clicks, move || h2.set(true)))));
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let done = cx.update(|cx| {
                    if current() && hit.get() {
                        close(cx);
                    }
                    !current() || hit.get()
                });
                if done {
                    return;
                }
            }
        })
        .detach();
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs_f32(8.0 * crate::parts::slow())).await;
        let _ = cx.update(|cx| {
            if current() {
                close(cx);
            }
        });
    })
    .detach();
}
