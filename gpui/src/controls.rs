//! Settings' controls as the pages use them: each one draws exactly as its snapshot picture in
//! `settings_kit` when there's no live window (`interact::get()` is none), and in the live
//! window works and moves as its Swift counterpart in Components.swift and Overlays.swift does.

use std::rc::Rc;

use gpui::{AnyElement, App, Div, Rgba, InteractiveElement, IntoElement, ParentElement, Pixels, Point, Role, StatefulInteractiveElement, Styled, Toggled, Window, div, point, px};

use crate::parts::*;
use crate::pen::Pen;
use crate::riso::{self, Img, Rgb8, col, hue, ink, over};
use crate::settings_kit::{self as kit, faded, grow, row};
use crate::interact::{self as live, Act, Curve, Interaction, Live, alpha, mix, pressable};

/// `DS.Surface.controlHover`.
pub const CONTROL_HOVER: Rgb8 = (0xF7, 0xF3, 0xF0);
/// `DS.Ink.i4`: the current rail row, a held row.
pub const I4: Rgb8 = (0xE1, 0xD9, 0xD2);

fn name(el: gpui::Stateful<Div>, role: Role, label: &str) -> gpui::Stateful<Div> {
    el.role(role).aria_label(label.to_string())
}

// MARK: - Switch

/// `TactileSwitch`: the snapshot's `kit::switch`, or live: a click flips it (`act`), the knob
/// slides on `DS.Motion.knob`, the slot takes the pink plate as it goes, squeezed to 0.96 while
/// held. `enabled` false: faded to 0.6 and does nothing (Swift's `.disabled`).
pub fn toggle(on: bool, title: &str, scale: f32, act: Act) -> AnyElement {
    toggle_if(on, title, scale, true, act)
}

pub fn toggle_if(on: bool, title: &str, scale: f32, enabled: bool, act: Act) -> AnyElement {
    let l = live::get().unwrap_or_else(Live::still);
    let id = format!("switch-{title}");
    let k = l.track(&format!("{id}-knob"), on as u8 as f32, Curve::KNOB);
    let (w, h) = (50.0, 26.0);
    let el = pressable(&l, &id, w, h, 0.96, enabled.then_some(act), move |_| {
        let fade = k.clamp(0.0, 1.0);
        let knob = sized(20.0, 16.0)
            .child(at(hand_fill(65, 0.3, 20.0, 16.0, drop(0.22)), 0.0, 1.0))
            .child(at(faded(hand_printed(66, 0.35, 20.0, 16.0, ink::CREMA, 66, scale), 1.0 - fade), 0.0, 0.0))
            .child(at(faded(hand_printed(66, 0.35, 20.0, 16.0, ink::INK, 66, scale), fade), 0.0, 0.0))
            .child(at(hand_outline(67, 0.35, 1.5, 20.0, 16.0, col(hue::I12), 1.2), 0.0, 0.0));
        sized(w, h)
            .child(at(faded(hand_fill(61, 0.4, w, h, col(hue::I3)), 1.0 - fade), -0.8, -0.5))
            .child(at(faded(hand_printed(61, 0.4, w, h, ink::PINK, 61, scale), fade), -0.8, -0.5))
            .child(at(hand_outline(63, 0.45, 2.0, w, h, col(hue::I12), 1.2), 0.0, 0.0))
            .child(at(knob, 4.0 + (w - 28.0) * k, 5.0))
            .into_any_element()
    });
    let el = name(el, Role::Switch, title).aria_toggled(if on { Toggled::True } else { Toggled::False });
    faded(el, if enabled { 1.0 } else { 0.6 }).into_any_element()
}

// MARK: - Ink choice

/// `InkChoice`: the snapshot's `kit::ink_choice`, or live: each tab a button (squeezed to 0.97),
/// the pink marker flying to the chosen one on `DS.Motion.pill`, its width stretching between the
/// two tabs' as SwiftUI's matched geometry does; a tab under the pointer darkens to i12.
pub fn choice(labels: &[&str], selected: usize, title: &str, scale: f32, pick: impl Fn(usize) -> Act) -> AnyElement {
    let l = live::get().unwrap_or_else(Live::still);
    let tab_h = line_box(23.0, BRUSH_LINE).max(line_box(12.5, MONO_LINE) + 4.0);
    let (tx, ty, tw, th, marker) = tone_marker(labels, selected, scale);
    // The marker's flight: how far from the tab it left to the one chosen.
    let id = format!("choice-{title}");
    let (from, to, k) = l.flight(&id, selected, Curve::PILL);
    let (fx, _, fw, _, _) = tone_marker(labels, from, scale);
    let (x, mw) = if from == to { (tx, tw) } else { (fx + (tx - fx) * k, fw + (tw - fw) * k) };
    let _ = to;
    let mut tabs = row().gap(z(18.0));
    for (i, label) in labels.iter().enumerate() {
        let tw_ = brush_w(label, 23.0).max(mono_w(label, 12.5, -0.12));
        let chosen = i == selected;
        let tab_id = format!("{title}-{i}");
        let label_s = label.to_string();
        let el = pressable(&l, &tab_id, tw_, tab_h, 0.97, Some(pick(i)), move |hover| {
            let mut tab = sized(tw_, tab_h);
            if chosen {
                tab = tab.child(at(t(label_s.clone(), brush(23.0), hue::I12), 0.0, tab_h - line_box(23.0, BRUSH_LINE)));
            } else {
                tab = tab.child(at(t(label_s.clone(), body().tracked(-0.12), mix(hue::I10, hue::I12, hover)), 0.0, tab_h - 4.0 - line_box(12.5, MONO_LINE)));
            }
            tab.into_any_element()
        });
        tabs = tabs.child(el.role(Role::Tab).aria_label(label.to_string()).aria_selected(chosen));
    }
    let c = sized(kit::ink_choice_w(labels), tab_h)
        .child(div().absolute().top(z(ty)).left(z(x)).w(z(mw)).h(z(th)).child(printed_fill(&marker)))
        .child(at(tabs, 0.0, 0.0));
    kit::named(c, title, Role::TabList, title).into_any_element()
}

// MARK: - Dropdown

/// The dropdown's hand chevron, turned `turn` of a half turn about its middle (it flips while
/// its menu is open, on `DS.Motion.fast`).
fn chevron(turn: f32) -> Pen {
    let (cx, cy) = (4.5, 2.5);
    let a = turn * std::f32::consts::PI;
    let (s, c) = a.sin_cos();
    let r = |x: f32, y: f32| (cx + (x - cx) * c - (y - cy) * s, cy + (x - cx) * s + (y - cy) * c);
    let mut p = Pen::new();
    let (x0, y0) = r(0.0, 0.0);
    let (qx, qy) = r(4.5 * 0.9, 2.5 * 1.2);
    let (x1, y1) = r(4.5, 5.0);
    let (x2, y2) = r(9.0, 0.5);
    p.m(x0, y0);
    p.q(qx, qy, x1, y1);
    p.l(x2, y2);
    p
}

/// `Dropdown`: the snapshot's `kit::dropdown`, or live: a click pops a menu of `options` under
/// it with the current one ticked; picking one calls `pick` with its index. Hover (or the menu
/// open) lightens the face to `controlHover`; squeezed to 0.97 while held.
pub fn menu(options: Vec<String>, selected: usize, icon: Option<&Img>, w: f32, pick: impl Fn(usize, &mut App) + 'static) -> AnyElement {
    let id = format!("dropdown-{}", options.join("|"));
    menu_id(&id, options, selected, icon, w, pick)
}

/// `menu` with its own id, for dropdowns that offer the same choices (one per app).
pub fn menu_id(id: &str, options: Vec<String>, selected: usize, icon: Option<&Img>, w: f32, pick: impl Fn(usize, &mut App) + 'static) -> AnyElement {
    let label = options.get(selected).cloned().unwrap_or_default();
    let l = live::get().unwrap_or_else(Live::still);
    let id = id.to_string();
    let open = l.is_open(&id);
    let turn = l.track(&format!("{id}-chev"), open as u8 as f32, Curve::FAST);
    let h = 32.0;
    let icon = icon.cloned();
    let lb = label.clone();
    let pick = Rc::new(pick);
    let act = {
        let (l, id, options) = (l.clone(), id.clone(), options.clone());
        live::act(move |window, cx| {
            let pick = pick.clone();
            let items = options.iter().map(crate::native_menu::Item::plain).collect();
            pop(&l, &id, items, Some(selected), window, cx, move |i, cx| pick(i, cx));
        })
    };
    let el = pressable(&l, &id, w, h, 0.97, Some(act), move |hover| {
        let face = mix(ink::PAPER, CONTROL_HOVER, if open { 1.0 } else { hover });
        let mut inner = row().w(z(w)).h(z(h)).px(z(10.0)).gap(z(8.0)).items_center();
        if let Some(i) = &icon {
            inner = inner.child(div().flex_none().ml(z(-2.0)).child(printed(20.0, 20.0, i)));
        }
        inner = inner.child(t(lb.clone(), body().tracked(-0.12), hue::I12)).child(grow())
            .child(drawn(9.0, 5.0, chevron(turn), Some(1.4), col(hue::I12)));
        sized(w, h)
            .child(at(hand_fill(71, 0.3, w, h, drop(0.18)), 0.0, 1.0))
            .child(at(hand_fill(72, 0.4, w, h, col(face)), 0.0, 0.0))
            .child(at(hand_outline(73, 0.45, 2.0, w, h, over(hue::I12, 0.75, ink::PAPER), 1.2), 0.0, 0.0))
            .child(at(inner, 0.0, 0.0))
            .into_any_element()
    });
    let el = el.child(live::bounds_marker(&l, &id));
    name(el, Role::ComboBox, &label).into_any_element()
}

/// Pops a native menu of `items` under the control `id` (where it was last drawn), `selected`
/// ticked, and hands the pick to `then`. The menu runs its own loop, so it's popped from a task
/// of its own, after the click that asked for it has let go of the app.
pub fn pop(l: &Live, id: &str, items: Vec<crate::native_menu::Item>, selected: Option<usize>, window: &mut Window, cx: &mut App, then: impl FnOnce(usize, &mut App) + 'static) {
    let Some(view) = crate::native_menu::view_of(window) else { return };
    let at_: Point<Pixels> = match l.bounds(id) {
        Some(b) => point(b.origin.x, b.origin.y + b.size.height + px(6.0)),
        None => window.mouse_position(),
    };
    l.set_open(id, true);
    let (l, id) = (l.clone(), id.to_string());
    cx.spawn(async move |cx| {
        let picked = crate::native_menu::pop_in(view, &items, selected, at_);
        cx.update(|cx| {
            l.set_open(&id, false);
            if let Some(i) = picked {
                then(i, cx);
            }
            l.notify(cx);
        });
    })
    .detach();
}

// MARK: - Washes

/// `penWash`: the rail's hover wash (i3), fading in place; `current` holds it two steps up (i4).
pub fn wash(seed: i64, w: f32, h: f32, hover: f32, current: bool) -> AnyElement {
    if current {
        hand_fill(seed, 0.5, w, h, col(I4))
    } else {
        hand_fill(seed, 0.5, w, h, alpha(col(hue::I3), hover))
    }
}

/// A row that takes the hover wash under the pointer (`TableRow`, a held note row): `content`
/// `w`×`h`, the wash `inset` wider each side.
pub fn hover_row(l: &Live, id: &str, seed: i64, w: f32, inset: f32, h: f32, content: impl IntoElement) -> gpui::Stateful<Div> {
    let hv = l.hover(id);
    div().flex_none().relative().w(z(w))
        .child(at(wash(seed, w + 2.0 * inset, h, hv, false), -inset, 0.0))
        .child(content)
        .id(gpui::SharedString::from(format!("row-{id}")))
        .on_hover(l.hover_handler(id, None))
}

/// `TableRow`: the snapshot's `kit::table_row`, or live: its own hover wash (`penWash`, seed
/// `211 + seed × 37`) fading in under the pointer on `DS.easeOut`, at `dim` (a hidden tone's
/// row is at half).
pub fn table_row(id: &str, content: Div, last: bool, seed: i64, w: f32, dim: f32) -> AnyElement {
    let r = kit::table_row(content, last, seed, w);
    let Some(l) = live::get() else { return r.into_any_element() };
    let hv = l.hover(id);
    div().flex_none().relative().w(z(w)).h(z(kit::ROW_H))
        .child(at(hand_fill(211 + seed * 37, 0.5, w, kit::ROW_H, alpha(col(hue::I3), hv * dim)), 0.0, 0.0))
        .child(at(r, 0.0, 0.0))
        .id(gpui::SharedString::from(format!("row-{id}")))
        .on_hover(l.hover_handler(id, None))
        .into_any_element()
}


// MARK: - Buttons

fn seed_of(title: &str) -> i64 {
    riso::str_seed(title, 17)
}

/// `TactileButton(role: .secondary)`, as Settings uses it (`plus`: led by a pen plus).
pub fn secondary(title: &str, plus: bool, h: f32, enabled: bool, act: Act) -> Option<(AnyElement, f32)> {
    let mut b = Button::secondary(title).enabled(enabled);
    if plus {
        b = b.icon("plus");
    }
    if h < 30.0 {
        b = b.compact();
    }
    Some(button(&b, 2.0, act))
}

/// `TactileButton(role: .primary)`, as Settings uses it.
pub fn primary(title: &str, plus: bool, enabled: bool, scale: f32, act: Act) -> Option<(AnyElement, f32)> {
    let mut b = Button::primary(title).enabled(enabled);
    if plus {
        b = b.icon("plus");
    }
    Some(button(&b, scale, act))
}

/// `TactileButton(role: .ghost)`, as Settings uses it.
pub fn ghost(title: &str, h: f32, label: &str, act: Act) -> Option<(AnyElement, f32)> {
    let mut b = Button::ghost(title).label(label).id(format!("ghost-{label}"));
    if h < 30.0 {
        b = b.compact();
    }
    Some(button(&b, 2.0, act))
}

/// `ActionMenuButton`: a button that pops a menu of `items()` under it (asked for when it's
/// pressed, so the list is fresh) and hands the pick to `pick`. Returns it with its width.
pub fn menu_button(b: &Button, scale: f32, items: impl Fn() -> Vec<crate::native_menu::Item> + 'static, pick: impl Fn(usize, &mut App) + 'static) -> (AnyElement, f32) {
    let l = live::get().unwrap_or_else(Live::still);
    let id = format!("menu-{}", b.key());
    let (items, pick) = (Rc::new(items), Rc::new(pick));
    let act = {
        let (l, id) = (l.clone(), id.clone());
        live::act(move |window, cx| {
            let pick = pick.clone();
            pop(&l, &id, items(), None, window, cx, move |i, cx| pick(i, cx));
        })
    };
    let (el, w) = button(b, scale, act);
    let el = div().flex_none().relative().child(el).child(live::bounds_marker(&l, &id)).into_any_element();
    (el, w)
}

/// `IconButton` live: a pen glyph, its wash (seed 23) and ink (i10 → i12) under the pointer,
/// squeezed to 0.92 while held; faded to 0.4 and inert when it can't act.
pub fn icon_button(symbol: &str, enabled: bool, label: &str, act: Act) -> AnyElement {
    let l = live::get().unwrap_or_else(Live::still);
    let id = format!("icon-{label}");
    let sym = symbol.to_string();
    let el = pressable(&l, &id, 22.0, 22.0, 0.92, enabled.then_some(act), move |hover| {
        let hover = if enabled { hover } else { 0.0 };
        sized(22.0, 22.0)
            .child(at(hand_fill(23, 0.5, 22.0, 22.0, alpha(col(hue::I3), hover)), 0.0, 0.0))
            .child(at(drawn(11.0, 11.0, kit::pen_glyph(&sym, 11.0), Some(1.6), col(mix(hue::I10, hue::I12, hover))), 5.5, 5.5))
            .into_any_element()
    });
    faded(name(el, Role::Button, label), if enabled { 1.0 } else { 0.4 }).into_any_element()
}

/// The tick drawn `frac` of its length (SwiftUI's `.trim(from: 0, to:)`).
fn tick_trim(w: f32, h: f32, frac: f32) -> Pen {
    let quad = |a: (f32, f32), c: (f32, f32), b: (f32, f32), t: f32| {
        let u = 1.0 - t;
        (u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1)
    };
    let (p0, c0, p1) = ((0.0, h * 0.55), (w * 0.2, h * 0.75), (w * 0.36, h));
    let (c1, p2) = ((w * 0.55, h * 0.35), (w, 0.0));
    let mut pts: Vec<(f32, f32)> = (0..=16).map(|i| quad(p0, c0, p1, i as f32 / 16.0)).collect();
    pts.extend((1..=16).map(|i| quad(p1, c1, p2, i as f32 / 16.0)));
    let lens: Vec<f32> = pts.windows(2).map(|s| (s[1].0 - s[0].0).hypot(s[1].1 - s[0].1)).collect();
    let total: f32 = lens.iter().sum();
    let mut left = total * frac.clamp(0.0, 1.0);
    let mut p = Pen::new();
    p.m(pts[0].0, pts[0].1);
    for (i, len) in lens.iter().enumerate() {
        if left <= 0.0 {
            break;
        }
        let k = (left / len).min(1.0);
        let (a, b) = (pts[i], pts[i + 1]);
        p.l(a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
        left -= len;
    }
    p
}

/// `TactileCheckbox` live: a click flips it; the tick draws itself in and pops from 0.9 on
/// `DS.easeOut`, the pink plate swapping with it; squeezed to 0.92 while held. `alpha` fades a
/// disabled or hidden row's box, each layer on its own, as the snapshot does.
pub fn checkbox(on: bool, a: f32, label: &str, scale: f32, act: Option<Act>) -> AnyElement {
    let l = live::get().unwrap_or_else(Live::still);
    let id = format!("check-{label}");
    let k = l.track(&format!("{id}-tick"), on as u8 as f32, Curve::FAST);
    let still = l.reduce_motion;
    let s = 18.0;
    let el = pressable(&l, &id, s, s, 0.92, act, move |_| {
        let face = if on { hand_printed(23, 0.35, s, s, ink::PINK, 23, scale) } else { hand_fill(23, 0.35, s, s, col(ink::PAPER)) };
        let mut b = sized(s, s)
            .child(at(faded(face, a), -0.8, -0.5))
            .child(at(faded(hand_outline(24, 0.4, 2.0, s, s, col(hue::I12), 1.2), a), 0.0, 0.0));
        if k > 0.001 {
            let trim = if still { 1.0 } else { k };
            let pop = if still { 1.0 } else { 0.9 + 0.1 * k };
            let (tw, th) = (15.0 * pop, 14.0 * pop);
            let tick = with_zoom(zoom() * pop, || drawn(15.0, 14.0, tick_trim(15.0, 14.0, trim), Some(2.2), col(hue::I12)));
            b = b.child(at(faded(tick, a * k), 4.0 + (15.0 - tw) / 2.0, -0.5 + (14.0 - th) / 2.0));
        }
        b.into_any_element()
    });
    name(el, Role::CheckBox, label).aria_toggled(if on { Toggled::True } else { Toggled::False }).into_any_element()
}

/// `CrossOutButton` live: the small red brush X and the word in the red pen, its wash fading in
/// under the pointer (and `on_hover` told, so the page can cross out what would go); squeezed
/// to 0.97 while held. Always asks first (`act` opens the confirm).
pub fn cross_out(title: &str, id: &str, scale: f32, on_hover: Rc<dyn Fn(bool, &mut App)>, act: Act) -> Option<AnyElement> {
    let l = live::get().unwrap_or_else(Live::still);
    let w = 6.0 + 15.0 + 7.0 + mono_w(title, 12.5, -0.12) + 6.0;
    let h = 25.0;
    let x = kit::dry_brush_x(ink::RED, 2, 0.17, 0.0, 15.0, 15.0, scale);
    let ts = title.to_string();
    let seed = title.chars().count() as i64 * 5 + 1;
    let el = live::pressable_hearing(&l, id, w, h, 0.97, Some(act), Some(on_hover), move |hover| {
        sized(w, h)
            .child(at(hand_fill(seed, 0.5, w, h, alpha(col(hue::I3), hover)), 0.0, 0.0))
            .child(at(row().h(z(h)).px(z(6.0)).gap(z(7.0)).items_center().child(printed(15.0, 15.0, &x))
                .child(t(ts.clone(), body().tracked(-0.12), hue::RED_TEXT)), 0.0, 0.0))
            .into_any_element()
    });
    Some(name(el, Role::Button, title).into_any_element())
}

/// Opens `url` in the browser (`NSWorkspace.open`). Under test (`IRIS_CONTROL`) nothing opens on
/// anyone's screen: the address is written to `<home>/opened.log` instead.
pub fn open_url(url: &str) {
    if crate::textbridge::under_test() {
        use std::io::Write;
        eprintln!("open: {url}");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(crate::store::home().join("opened.log")) {
            let _ = writeln!(f, "{url}");
        }
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
    }
}

// MARK: - TactileButton, one for every surface

/// Which button (`ButtonRole`): one primary per surface, everything else secondary, ghost for
/// tertiary actions inside a form, the red final yes only in a confirm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Ghost,
    DestructiveConfirm,
}

/// A `TactileButton`: its words and what it carries besides.
#[derive(Clone, Debug)]
pub struct Button {
    pub title: String,
    pub kind: Kind,
    /// A pen glyph before the words ("plus", "arrow.down").
    pub icon: Option<&'static str>,
    /// Keys drawn in the button's own ink ("↵", "⌘C").
    pub key_chip: Option<String>,
    /// 26 points tall instead of 32.
    pub compact: bool,
    pub enabled: bool,
    /// The label swaps to ink dots and an "-ing…" verb.
    pub loading: bool,
    /// The widest label it shows, so it keeps one width as its label swaps.
    pub reserve: Option<String>,
    /// What VoiceOver says, when it isn't the title ("Use lighter").
    pub label: Option<String>,
    /// Tells two buttons with the same words apart (their hover and press are their own).
    pub id: Option<String>,
}

impl Button {
    pub fn new(title: impl Into<String>, kind: Kind) -> Button {
        Button { title: title.into(), kind, icon: None, key_chip: None, compact: false, enabled: true, loading: false, reserve: None, label: None, id: None }
    }
    pub fn primary(title: impl Into<String>) -> Button {
        Button::new(title, Kind::Primary)
    }
    pub fn secondary(title: impl Into<String>) -> Button {
        Button::new(title, Kind::Secondary)
    }
    pub fn ghost(title: impl Into<String>) -> Button {
        Button::new(title, Kind::Ghost)
    }
    pub fn icon(mut self, name: &'static str) -> Button {
        self.icon = Some(name);
        self
    }
    pub fn key_chip(mut self, keys: impl Into<String>) -> Button {
        self.key_chip = Some(keys.into());
        self
    }
    pub fn compact(mut self) -> Button {
        self.compact = true;
        self
    }
    pub fn enabled(mut self, on: bool) -> Button {
        self.enabled = on;
        self
    }
    pub fn loading(mut self, on: bool) -> Button {
        self.loading = on;
        self
    }
    pub fn reserve(mut self, widest: impl Into<String>) -> Button {
        self.reserve = Some(widest.into());
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Button {
        self.label = Some(label.into());
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Button {
        self.id = Some(id.into());
        self
    }

    fn key(&self) -> String {
        self.id.clone().unwrap_or_else(|| format!("{:?}-{}", self.kind, self.title))
    }
    fn height(&self) -> f32 {
        if self.compact { 26.0 } else { 32.0 }
    }
    fn lead(&self) -> f32 {
        (if self.icon.is_some() { 10.0 } else { 12.0 }) + if self.kind == Kind::Primary { 6.0 } else { 0.0 }
    }
    fn trail(&self) -> f32 {
        12.0 + if self.kind == Kind::Primary { 6.0 } else { 0.0 }
    }
    /// SwiftUI gives fixed-size words whole pixels (at 2×, half points).
    fn title_w(&self) -> f32 {
        let w = mono_w(&self.title, 12.5, -0.12).max(self.reserve.as_deref().map_or(0.0, |r| mono_w(r, 12.5, -0.12)));
        (w * 2.0 - 0.01).ceil() / 2.0
    }
    fn chips_w(&self) -> f32 {
        self.key_chip.as_deref().map_or(0.0, |k| 6.0 + button_chips(k, 0, hue::I12, ink::PAPER).1)
    }
    /// Its width, in points.
    pub fn width(&self) -> f32 {
        let glyph = if self.loading { 4.0 * 3.0 + 3.2 * 2.0 + 6.0 } else if self.icon.is_some() { 10.0 + 6.0 } else { 0.0 };
        self.lead() + glyph + self.title_w() + self.chips_w() + self.trail()
    }

    fn text(&self, hover: f32) -> Rgb8 {
        match self.kind {
            Kind::Primary => hue::I1,
            Kind::Secondary => hue::I12,
            Kind::Ghost => mix(hue::I10, hue::I12, hover),
            Kind::DestructiveConfirm => (255, 255, 255),
        }
    }
}

/// The pen glyphs a button can lead with.
fn button_glyph(name: &str, size: f32) -> Pen {
    if name == "arrow.down" {
        let s = size;
        let mut p = Pen::new();
        p.m(0.5 * s, 0.04 * s);
        p.q(0.46 * s, 0.5 * s, 0.52 * s, 0.9 * s);
        p.m(0.16 * s, 0.58 * s);
        p.l(0.52 * s, 0.94 * s);
        p.l(0.86 * s, 0.56 * s);
        return p;
    }
    kit::pen_glyph(name, size)
}

/// `focusRing`: a pen ring 4 points outside the control, in i10.
pub fn focus_ring(w: f32, h: f32) -> AnyElement {
    at(hand_outline(61, 0.6, 4.0, w + 8.0, h + 8.0, col(hue::I10), 2.0), -4.0, -4.0).into_any_element()
}

/// The button's face in `st` (`TactileButton.Face`): the one drawing for every state, used by
/// live windows and the component sheet alike.
pub fn button_face(b: &Button, st: Interaction, reduce_motion: bool, scale: f32) -> Div {
    let (w, h) = (b.width(), b.height());
    let seed = seed_of(&b.title);
    let hover = if b.enabled { st.hover } else { 0.0 };
    let pressed = if b.enabled { st.pressed } else { 0.0 };
    let text = b.text(hover);
    let mut face = sized(w, h);
    match b.kind {
        Kind::Primary => {
            // Two passes of one brush: a rose-tinted espresso stroke printed up and left, then
            // the espresso stroke over it. Hover: the rose comes up through a drier stroke.
            // Reduce Motion: pressing brings the rose through fully instead of squeezing.
            let plate = riso::Plate { w, h, scale };
            let under_ink = riso::cached(format!("slab-under-ink-{w}-{h}"), &plate, || riso::print(&plate, ink::INK, BRUSH + 7, &slab_mask(&plate, BRUSH + 7, 0.0), 1.0));
            let under_rose = riso::cached(format!("slab-under-rose-{w}-{h}"), &plate, || riso::print(&plate, ink::ROSE, BRUSH + 7, &slab_mask(&plate, BRUSH + 7, 0.0), 1.0));
            let top = riso::cached(format!("slab-top-{w}-{h}"), &plate, || riso::print(&plate, ink::INK, BRUSH, &slab_mask(&plate, BRUSH, 1.9), 1.0));
            let drier = riso::cached(format!("slab-top-dry-{w}-{h}"), &plate, || riso::print(&plate, ink::INK, BRUSH, &slab_mask(&plate, BRUSH, 2.6), 1.0));
            let still = reduce_motion && pressed > 0.5;
            let rose = if still { 1.0 } else { 0.35 + 0.5 * hover };
            if b.enabled {
                face = face
                    .child(at(printed(w, h, &under_ink), -2.5, -2.0))
                    .child(at(faded(printed(w, h, &under_rose), rose), -2.5, -2.0));
            }
            face = face
                .child(at(faded(printed(w, h, &top), 1.0 - hover), 0.0, 0.0))
                .child(at(faded(printed(w, h, &drier), hover), 0.0, 0.0));
        }
        Kind::Secondary => {
            // The keycap: paper face, pen outline, a small drop that flattens on press.
            face = face
                .child(at(hand_fill(seed + 9, 0.3, w, h, drop(0.18)), 0.0, 1.2 - 0.7 * pressed))
                .child(at(hand_fill(seed + 3, 0.4, w, h, col(mix(ink::PAPER, CONTROL_HOVER, hover))), 0.0, 0.0))
                .child(at(hand_outline(seed, 0.45, 2.0, w, h, over(hue::I12, 0.75, ink::PAPER), 1.1), 0.0, 0.0));
        }
        Kind::Ghost => {
            // The one hover in Iris: a pen-drawn grey ink wash.
            face = face.child(at(hand_fill(seed, 0.5, w, h, alpha(col(hue::I3), hover)), 0.0, 0.0));
        }
        Kind::DestructiveConfirm => {
            face = face
                .child(at(hand_printed(seed, 0.45, w, h, ink::RED, seed, scale), 0.0, 0.0))
                .child(at(hand_fill(seed, 0.45, w, h, Rgba { a: 0.05 * hover, ..col((0, 0, 0)) }), 0.0, 0.0));
        }
    }
    let mut words = row().h(z(h)).pl(z(b.lead())).gap(z(6.0)).items_center();
    if b.loading {
        words = words.child(ink_dots(text, 4.0));
    } else if let Some(icon) = b.icon {
        let ink_c = if b.kind == Kind::Secondary { hue::I9 } else { text };
        words = words.child(drawn(10.0, 10.0, button_glyph(icon, 10.0), Some(1.5), col(ink_c)));
    }
    // Tracked mono is set a letter a cell, and GPUI rounds each cell to whole pixels, so a 3%
    // squeeze would leave the words their full size: squeezed, they're set untracked (the
    // 0.12 points a letter is less than the squeeze takes off).
    let ts = if crate::parts::zoom() < 0.999 { body() } else { body().tracked(-0.12) };
    words = words.child(div().flex_none().w(z(b.title_w())).child(t(b.title.clone(), ts, text)));
    if let Some(k) = &b.key_chip {
        let under = if b.kind == Kind::Primary { ink::INK } else { ink::PAPER };
        words = words.child(button_chips(k, seed, text, under).0);
    }
    face = face.child(at(words, 0.0, 0.0));
    if st.focused {
        face = face.child(focus_ring(w, h));
    }
    face
}

/// A `TactileButton` that does `act` when pressed: hovered, pressed (squeezed to 0.97) and
/// focused as the window's dispatcher says, or as a forced state says; faded to 0.6 and inert
/// when not enabled. Returns it with its width.
pub fn button(b: &Button, scale: f32, act: Act) -> (AnyElement, f32) {
    let l = live::get().unwrap_or_else(Live::still);
    let (w, h) = (b.width(), b.height());
    let id = b.key();
    let st = l.state(&id, b.enabled);
    let k = if l.reduce_motion || !b.enabled { 1.0 } else { 1.0 - 0.03 * st.pressed };
    let face = if (1.0 - k).abs() > 1e-4 {
        let outer = crate::parts::zoom();
        let inner = crate::parts::with_zoom(outer * k, || button_face(b, st, l.reduce_motion, scale));
        at(inner, (w - w * k) / 2.0, (h - h * k) / 2.0).into_any_element()
    } else {
        at(button_face(b, st, l.reduce_motion, scale), 0.0, 0.0).into_any_element()
    };
    let mut el = sized(w, h).child(face).child(live::bounds_marker(&l, &id)).id(gpui::SharedString::from(format!("button-{id}")));
    if l.view.is_some() {
        el = el.on_hover(l.hover_handler(&id, None));
        if b.enabled {
            let (l1, l2, l3, l4) = (l.clone(), l.clone(), l.clone(), l.clone());
            let (i1, i2, i3, i4) = (id.clone(), id.clone(), id.clone(), id.clone());
            el = el
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| l1.set_pressed(&i1, true, cx))
                .on_mouse_up(gpui::MouseButton::Left, move |_, _, cx| l2.set_pressed(&i2, false, cx))
                .on_mouse_up_out(gpui::MouseButton::Left, move |_, _, cx| l3.set_pressed(&i3, false, cx))
                .on_click(move |_, window, cx| {
                    l4.set_pressed(&i4, false, cx);
                    act(window, cx);
                    l4.notify(cx);
                });
        }
    }
    // Heard as SwiftUI hears `TactileButton`: its title and its key chip together ("Replace, ↵").
    let label = b.label.clone().unwrap_or_else(|| match &b.key_chip {
        Some(k) => format!("{}, {k}", b.title),
        None => b.title.clone(),
    });
    let el = name(el, Role::Button, &label);
    (faded(el, if b.enabled { 1.0 } else { 0.6 }).into_any_element(), w)
}

// MARK: - Shortcut recorder

/// What a `ShortcutRecorder` shows: its keys (none: off), which shortcut it is (seeds its caps),
/// whether it's listening, whether ⌫ turns it off, and what's wrong with the keys last pressed
/// (`pending`: they only drew a warning, and Use anyway would save them).
#[derive(Clone, Debug, PartialEq)]
pub struct Recorder {
    pub keys: Option<crate::store::KeyCombo>,
    pub action: i64,
    pub listening: bool,
    pub allows_clear: bool,
    pub problem: Option<crate::tones::Problem>,
    pub pending: bool,
}

/// What the recorder does: pressed (start or stop listening), a key while it listens, Use anyway.
pub struct RecorderActs {
    pub focus: Option<gpui::FocusHandle>,
    pub press: Act,
    pub key: Rc<dyn Fn(&gpui::KeyDownEvent, &mut Window, &mut App)>,
    pub use_anyway: Act,
}

/// `ShortcutRecorder`: the keys printed in the pink plate like a Mac's, and under them what a
/// click does; listening, the keys go back to bare paper waiting for new ones (the last a
/// "…"), with what to press and the way out as keycaps; a problem shows as a badge with its
/// words. Squeezed to 0.98 while held. Returns it and its width.
pub fn recorder(r: &Recorder, scale: f32, acts: Option<RecorderActs>) -> (AnyElement, f32) {
    use crate::pages::shortcuts::{labeled_keycap, labeled_w};
    let l = live::get().unwrap_or_else(Live::still);
    // The caps: (symbol, word, seed, printed).
    let mut caps: Vec<(Option<&'static str>, &'static str, i64, bool)> = vec![];
    if r.listening {
        let mut waiting: Vec<_> = r.keys.map(crate::tones::caps).unwrap_or_default();
        waiting.pop();
        if r.keys.is_none() {
            waiting = vec![(Some("⌃"), "control"), (Some("⌥"), "option")];
        }
        for (i, (sym, word)) in waiting.into_iter().enumerate() {
            caps.push((sym, word, 70 + i as i64 * 7, false));
        }
        caps.push((None, "…", 97, false));
    } else if let Some(k) = r.keys {
        for (i, (sym, word)) in crate::tones::caps(k).into_iter().enumerate() {
            caps.push((sym, word, 11 + i as i64 * 13 + r.action * 5, true));
        }
    }
    let (face_w, face) = if caps.is_empty() {
        (8.0 + mono_w("Off", 12.5, 0.0), row().px(z(4.0)).py(z(4.0)).child(row().h(z(32.0)).items_center().child(t("Off", body(), hue::I10))))
    } else {
        let mut f = row().gap(z(5.0)).px(z(4.0)).py(z(4.0));
        let mut fw = 8.0 + 5.0 * (caps.len() as f32 - 1.0);
        for (sym, word, seed, printed_) in &caps {
            f = f.child(labeled_keycap(word, *sym, *seed, *printed_, scale));
            fw += labeled_w(word, *sym);
        }
        (fw, f)
    };
    let id = format!("recorder-{}", r.action);
    let face_h = 40.0;
    let press = acts.as_ref().map(|a| a.press.clone());
    let face = std::cell::Cell::new(Some(face));
    let mut el = pressable(&l, &id, face_w, face_h, 0.98, press, move |_| face.take().unwrap_or_else(row).into_any_element());
    let spoken = r.keys.map(|k| format!("Shortcut {}. Click to change.", crate::tones::spoken(k).replace(" + ", " "))).unwrap_or_else(|| "No shortcut. Click to set.".into());
    el = el.role(Role::Button).aria_label(spoken);
    if let Some(a) = &acts {
        if let Some(f) = &a.focus {
            el = el.track_focus(f);
        }
        let key = a.key.clone();
        el = el.on_key_down(move |e, window, cx| key(e, window, cx));
    }
    let mut col_ = kit::column().items_end().gap(z(6.0)).child(el);
    let mut w = face_w;
    if r.listening {
        let line1 = row().gap(z(4.0)).items_center()
            .child(t("Now press your new keys, like", caption(), ink::PINK_TEXT))
            .child(keycap("⌃", scale)).child(keycap("⌥", scale)).child(keycap("K", scale));
        let w1 = mono_w("Now press your new keys, like", 11.0, 0.0) + 3.0 * 4.0 + 3.0 * 20.0;
        let mut line2 = row().gap(z(4.0)).items_center().child(keycap("esc", scale)).child(t("keeps the old ones", caption(), hue::I10));
        let mut w2 = 31.0 + 4.0 + mono_w("keeps the old ones", 11.0, 0.0);
        if r.allows_clear {
            line2 = line2.child(t("·", caption(), hue::I9)).child(keycap("⌫", scale)).child(t("turns it off", caption(), hue::I10));
            w2 += 3.0 * 4.0 + mono_w("·", 11.0, 0.0) + 20.0 + mono_w("turns it off", 11.0, 0.0);
        }
        w = w.max(w1).max(w2);
        let spoken = format!("Now press your new keys. Escape keeps the old ones.{}", if r.allows_clear { " Delete turns it off." } else { "" });
        col_ = col_.child(kit::column().items_end().gap(z(5.0)).child(line1).child(line2)
            .id(gpui::SharedString::from(format!("{id}-how"))).aria_label(spoken));
    } else {
        let hint = if r.keys.is_none() { "Click to choose keys" } else { "Click to set your own keys" };
        w = w.max(mono_w(hint, 11.0, 0.0));
        col_ = col_.child(t(hint, caption(), hue::I10));
    }
    if let Some(p) = &r.problem {
        let (badge, badge_w) = problem_badge(if p.is_blocking() { "Can't use" } else { "Heads up" }, p.is_blocking());
        let msg = p.message();
        let msg_w = mono_w(&msg, 11.0, 0.0).min(200.0);
        let mut line = row().gap(z(8.0)).items_center().child(badge).child(kit::para_out(&msg, caption(), hue::I11, msg_w));
        let mut pw = badge_w + 8.0 + msg_w;
        if r.pending && !p.is_blocking() {
            let ua = acts.as_ref().map(|a| a.use_anyway.clone()).unwrap_or_else(|| live::act(|_, _| {}));
            let (b, bw) = button(&Button::ghost("Use anyway").compact().id(format!("use-anyway-{}", r.action)), scale, ua);
            line = line.child(b);
            pw += 8.0 + bw;
        }
        w = w.max(pw);
        col_ = col_.child(line);
    }
    (col_.w(z(w)).into_any_element(), w)
}

/// `Badge`: a tinted label with a pen outline, red for what can't be used, warm for a warning.
fn problem_badge(words: &str, red: bool) -> (Div, f32) {
    let (text, tint) = if red { (hue::RED_TEXT, (0xFB, 0xE6, 0xE1)) } else { (hue::WARN_TEXT, hue::WARN_TINT) };
    let seed = riso::str_seed(words, 13);
    let (w, h) = (16.0 + mono_w(words, 11.0, 0.0), 20.0);
    let b = sized(w, h)
        .child(at(hand_fill(seed + 3, 0.35, w, h, col(tint)), 0.0, 0.0))
        .child(at(hand_outline(seed, 0.4, 2.0, w, h, over(text, 0.45, tint), 1.0), 0.0, 0.0))
        .child(at(row().w(z(w)).h(z(h)).px(z(8.0)).items_center().child(t(words, caption(), text)), 0.0, 0.0));
    (b, w)
}

// MARK: - Links

/// `PenLink`: the words with a quick pen stroke printed in pink under them (deepening to the
/// pink text ink under the pointer, on `DS.easeOut`) and a pen arrow turned up; squeezed to 0.98
/// while held; opens `url` in the browser.
pub fn pen_link(title: &str, url: &'static str, s: f32) -> AnyElement {
    use std::f32::consts::PI;
    let l = live::get().unwrap_or_else(Live::still);
    let n = title.chars().count() as i64;
    let tw = mono_w(title, 12.5, 0.0);
    let lh = line_box(12.5, MONO_LINE);
    let plate = riso::Plate { w: tw, h: 5.0, scale: s };
    let under = |ink_: Rgb8, key: &str| riso::cached(format!("settings-pen-link-{key}{title}"), &plate, || {
        let mut rng = riso::Rng::new(n);
        let h = 5.0;
        let bend = (rng.f() - 0.3) * h * 0.8;
        let pts: Vec<[f32; 2]> = (0..=24).map(|i| {
            let t = i as f32 / 24.0;
            let sag = (t * PI).sin() * bend;
            let rise = t * h * 0.25;
            let jitter = (rng.f() - 0.5) * 0.3;
            [tw * t, h / 2.0 + sag - rise + jitter]
        }).collect();
        let mask = plate.fill(&kit::ribbon(&pts, 2.6, 0.6, 0.2, false).skia().unwrap());
        riso::print(&plate, ink_, n, &mask, 1.0)
    });
    let (pink, deep) = (under(ink::PINK, ""), under(ink::PINK_TEXT, "deep-"));
    let arrow_w = 22.0 * 0.8;
    let (w, h) = (tw + 4.0 + arrow_w, lh);
    let ts = title.to_string();
    let id = format!("link-{title}");
    let el = pressable(&l, &id, w, h, 0.98, Some(live::act(move |_, _| open_url(url))), move |hover| {
        let mut words = sized(tw, lh).child(at(printed(tw, 5.0, &pink), 0.0, lh - 5.0 + 4.0));
        if hover > 0.001 {
            words = words.child(at(faded(printed(tw, 5.0, &deep), hover), 0.0, lh - 5.0 + 4.0));
        }
        let words = words.child(at(t(ts.clone(), body(), hue::I12), 0.0, 0.0));
        let arrow = kit::arrow_turned(22.0 * 0.8, 12.0 * 0.8, 2.3 * 0.9, n + 9, -35.0, s);
        row().gap(z(4.0)).items_center().child(words).child(arrow).into_any_element()
    });
    name(el, Role::Link, title).into_any_element()
}
