//! The confirm card any window can show (`SheetHost` and `ConfirmCard`, Overlays.swift): the
//! window dims, and a slip of paper with the question in brush (the red pen when it destroys
//! something), its consequence plain, a pen rule, then Cancel at the left and the named confirm
//! at the right. In on `DS.Motion.slow` (0.24 s, rising 16 points and fading; under Reduce Motion
//! it only fades), out in 0.2 s. A click on the dim cancels.
//!
//! A window shows it by asking (`ask`) and by putting `layer` last in what it renders.
//!
//! Left out, as GPUI can't do them: the page behind blurring by a point, and the card's 0.4°
//! lean. Return and Esc work when the window takes keys.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui::*;

use crate::parts::*;
use crate::riso::{self, col, hue, ink, over};
use crate::controls as controls;
use crate::settings_kit::{column, grow, row};
use crate::interact::{self as live, Act, Anim, Live};

const WIDTH: f32 = 420.0;
const ENTER: f32 = 0.24;
const EXIT: f32 = 0.20;

#[derive(Clone)]
pub struct Request {
    pub title: String,
    pub message: String,
    pub confirm_title: String,
    /// Red pen and the red confirm (`destructiveConfirm`); otherwise the primary button.
    pub destructive: bool,
    pub on_confirm: Act,
}

struct Shown {
    request: Request,
    /// The card's keyboard, taken when it comes up so Esc and Return reach it.
    focus: Option<FocusHandle>,
    since: Instant,
    /// Going away since then.
    leaving: Option<Instant>,
    anim: Rc<RefCell<Anim>>,
}

thread_local! {
    static SHOWN: RefCell<HashMap<WindowId, Shown>> = RefCell::new(HashMap::new());
}

/// Asks the person in `window` to confirm: `on_confirm` runs if they do.
pub fn ask(window: &mut Window, title: &str, message: &str, confirm_title: &str, on_confirm: impl Fn(&mut Window, &mut App) + 'static) {
    show(window, Request { title: title.into(), message: message.into(), confirm_title: confirm_title.into(), destructive: true, on_confirm: Rc::new(on_confirm) });
}

pub fn show(window: &mut Window, request: Request) {
    let id = window.window_handle().window_id();
    SHOWN.with(|s| s.borrow_mut().insert(id, Shown { request, focus: None, since: Instant::now(), leaving: None, anim: Rc::default() }));
    window.refresh();
}

/// Whether `window` is showing a confirm.
pub fn is_up(window: &Window) -> bool {
    let id = window.window_handle().window_id();
    SHOWN.with(|s| s.borrow().get(&id).is_some_and(|x| x.leaving.is_none()))
}

/// Puts the card away (Cancel, Esc, a click on the dim).
pub fn dismiss(window: &mut Window) {
    let id = window.window_handle().window_id();
    SHOWN.with(|s| {
        if let Some(x) = s.borrow_mut().get_mut(&id) && x.leaving.is_none() {
            x.leaving = Some(Instant::now());
        }
    });
    window.refresh();
}

/// Confirms (Return, or the confirm button): runs the action and puts the card away.
pub fn accept(window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let act = SHOWN.with(|s| s.borrow().get(&id).filter(|x| x.leaving.is_none()).map(|x| x.request.on_confirm.clone()));
    if let Some(a) = act {
        dismiss(window);
        a(window, cx);
    }
}

/// Esc and Return while a confirm is up; true if it took the key.
pub fn key(name: &str, window: &mut Window, cx: &mut App) -> bool {
    if !is_up(window) {
        return false;
    }
    match name {
        "escape" => dismiss(window),
        "enter" => accept(window, cx),
        _ => return false,
    }
    true
}

/// The dim and the card over the window, while one is up; put it last in the window's tree.
pub fn layer<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> AnyElement {
    let id = window.window_handle().window_id();
    let reduce = crate::live_slip::reduce_motion();
    let now = Instant::now();
    let slow = crate::parts::slow();
    let state = SHOWN.with(|s| {
        let mut s = s.borrow_mut();
        let x = s.get(&id)?;
        if let Some(l) = x.leaving && now.duration_since(l).as_secs_f32() > EXIT * slow {
            s.remove(&id);
            return None;
        }
        Some((x.request.clone(), x.since, x.leaving, x.anim.clone(), x.focus.clone()))
    });
    let Some((req, since, leaving, anim, focus)) = state else { return div().into_any_element() };
    // The card takes the keyboard as it comes up (`.keyboardShortcut(.cancelAction)` and
    // `.defaultAction` in Swift): Esc cancels, Return confirms.
    let focus = focus.unwrap_or_else(|| {
        let f = cx.focus_handle();
        SHOWN.with(|s| if let Some(x) = s.borrow_mut().get_mut(&id) { x.focus = Some(f.clone()) });
        window.focus(&f, cx);
        f
    });
    // How far in: 0 → 1 on the way in, back to 0 on the way out.
    let k = match leaving {
        None => live::ease_out(now.duration_since(since).as_secs_f32() / (ENTER * slow)),
        Some(l) => 1.0 - live::ease_out(now.duration_since(l).as_secs_f32() / (EXIT * slow)),
    };
    let rise = if leaving.is_none() && !reduce { 16.0 * (1.0 - k) } else { 0.0 };
    let settled = leaving.is_none() && now.duration_since(since).as_secs_f32() >= ENTER * slow;
    let l = Live { anim: anim.clone(), view: Some(cx.entity().downgrade().into()), now, reduce_motion: reduce, mouse: Some(window.mouse_position()) };
    anim.borrow_mut().moving = false;
    let card = live::with(l.clone(), || card(&req, window.scale_factor()));
    if !settled || anim.borrow().moving {
        window.request_animation_frame();
    }
    let size = window.viewport_size();
    let (vw, vh) = (f32::from(size.width), f32::from(size.height));
    let dim = div().absolute().top_0().left_0().size_full().bg(Rgba { a: 0.3 * k, ..col((0, 0, 0)) })
        .id("confirm-dim")
        .on_mouse_down(MouseButton::Left, |_, window, _| dismiss(window));
    // The card stands in the window's middle (the dim takes clicks round it).
    let card = div().absolute().left(px((vw - WIDTH) / 2.0)).top(px(vh / 2.0 - 90.0 + rise)).opacity(k).child(card)
        .id("confirm-card").on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    div().absolute().top_0().left_0().size_full()
        .track_focus(&focus)
        .on_key_down(|e, window, cx| {
            let name = match e.keystroke.key.as_str() { "escape" => "escape", "enter" => "enter", _ => return };
            if key(name, window, cx) {
                cx.stop_propagation();
            }
        })
        .child(dim).child(card).into_any_element()
}

/// A paragraph that wraps in `w`.
fn words(s: &str, ts: Ts, c: riso::Rgb8, w: f32) -> Div {
    styled(ts, c).w(z(w)).whitespace_normal().child(s.to_string())
}

fn card(req: &Request, scale: f32) -> Div {
    let inner = WIDTH - 40.0;
    let title_ink = if req.destructive { hue::RED_TEXT } else { hue::I12 };
    let band = column().w(z(WIDTH)).pt(z(20.0)).px(z(20.0)).pb(z(16.0)).gap(z(6.0))
        .child(words(&req.title, brush(25.0), title_ink, inner).id("confirm-title").role(Role::Heading).aria_label(req.title.clone()))
        .child(words(&req.message, body(), hue::I11, inner));
    let cancel = controls::secondary("Cancel", false, 32.0, true, live::act(|window, _| dismiss(window)))
        .map(|b| b.0).unwrap_or_else(|| div().into_any_element());
    let confirm = if req.destructive {
        destructive(&req.confirm_title, scale)
    } else {
        controls::primary(&req.confirm_title, false, true, scale, live::act(|window, cx| accept(window, cx))).map(|b| b.0).unwrap_or_else(|| div().into_any_element())
    };
    let actions = row().w(z(WIDTH)).p(z(12.0)).gap(z(12.0)).items_center().child(cancel).child(grow()).child(confirm);
    let body = column().w(z(WIDTH)).bg(col(ink::PAPER))
        .child(band)
        .child(pen_rule(761, WIDTH, over(hue::I12, 0.3, ink::PAPER), 1.0))
        .child(actions)
        .shadow(vec![BoxShadow { color: black().opacity(0.24), offset: point(px(0.0), z(12.0)), blur_radius: z(10.0), spread_radius: px(0.0), inset: false }]);
    // The pen edge round the slip, drawn over it once it's laid out.
    let edge = canvas(|_, _, _| (), move |b, _, window, _| {
        let (w, h) = (f32::from(b.size.width) / zoom(), f32::from(b.size.height) / zoom());
        if let Some(p) = crate::pen::hand_rect(57, 0.45, 2.0, false, 0.0, 0.0, w, h).gpui(b.origin, Some(1.1), zoom()) {
            window.paint_path(p, over(hue::I12, 0.55, ink::PAPER));
        }
    }).absolute().top_0().left_0().size_full();
    div().flex_none().relative().w(z(WIDTH)).child(body).child(edge)
}

/// `TactileButton(role: .destructiveConfirm)`: the red plate printed through the screen, white
/// words; a touch darker under the pointer.
fn destructive(title: &str, scale: f32) -> AnyElement {
    let Some(l) = live::get() else { return div().into_any_element() };
    let seed = riso::str_seed(title, 17);
    let (w, h) = (24.0 + mono_w(title, 12.5, -0.12), 32.0);
    let face = hand_printed(seed, 0.45, w, h, ink::RED, seed, scale);
    let ts = title.to_string();
    let el = live::pressable(&l, &format!("confirm-{title}"), w, h, 0.97, Some(live::act(|window, cx| accept(window, cx))), move |hover| {
        sized(w, h)
            .child(at(face, 0.0, 0.0))
            .child(at(hand_fill(seed, 0.45, w, h, Rgba { a: 0.05 * hover, ..col((0, 0, 0)) }), 0.0, 0.0))
            .child(at(row().w(z(w)).h(z(h)).pl(z(12.0)).items_center().child(t(ts.clone(), body().tracked(-0.12), (255, 255, 255))), 0.0, 0.0))
            .into_any_element()
    });
    el.role(Role::Button).aria_label(title.to_string()).into_any_element()
}
