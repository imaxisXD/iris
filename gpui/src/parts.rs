//! Iris's parts in GPUI: type, pen rules, keycaps, buttons, the tone row, tape, notes.
//! Each follows its Swift namesake (DESIGN.md, "Components"); sizes are the Swift ones.
//!
//! Layout and type are GPUI's (flex boxes, its text). Pen lines are GPUI vector paths. Printed
//! ink is baked on the CPU through the riso screen and shown as GPUI images. What Iris sets a
//! degree off straight is printed whole into one picture (`raster`): GPUI can't turn a box.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{AnyElement, Div, FontFallbacks, IntoElement, ParentElement, Pixels, Rgba, Styled, canvas, div, px};
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};


use crate::pen::{self, Pen};
use crate::raster::{self, Sheet};
use crate::riso::{self, Img, Plate, Rgb8, col, hue, ink, over};

pub const MONO_430: &[u8] = include_bytes!("../fonts/IrisMono-430.ttf");
pub const MONO_450: &[u8] = include_bytes!("../fonts/IrisMono-450.ttf");
pub const MONO_560: &[u8] = include_bytes!("../fonts/IrisMono-560.ttf");
pub const MARTIAN: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/MartianMono-Variable.ttf");
pub const CAVEAT: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/CaveatBrush-Regular.ttf");

// MARK: - Zoom

thread_local! {
    /// How big the panel is laid out: 1 normally; less while it unrolls out of the flower.
    static ZOOM: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// The current zoom.
pub fn zoom() -> f32 {
    ZOOM.with(|z| z.get())
}

/// `v` points at the current zoom. Every size in the panel goes through this, so the panel can
/// be laid out smaller while it unrolls: GPUI can't scale a box once it's laid out.
pub fn z(v: f32) -> Pixels {
    px(v * zoom())
}

/// Builds with everything `k` times its size (quantised, so text and pictures come in a few
/// sizes rather than a new one every frame).
pub fn with_zoom<R>(k: f32, f: impl FnOnce() -> R) -> R {
    let k = ((k * 256.0).round() / 256.0).max(1.0 / 256.0);
    let was = ZOOM.with(|z| z.replace(k));
    let out = f();
    ZOOM.with(|z| z.set(was));
    out
}

// MARK: - Type

/// Martian Mono's advance at width 87.5, as a share of the size: every glyph the same.
pub const MONO_ADVANCE: f32 = 0.650;
/// Line heights as a share of the size (ascent + descent, no leading).
pub const MONO_LINE: f32 = 1.2;
pub const BRUSH_LINE: f32 = 1.26;
pub const BRUSH_ASCENT: f32 = 0.96;

/// A line's box: SwiftUI lays a line out on whole points (13.5 pt Martian takes 17, not 16.2).
pub fn line_box(size: f32, share: f32) -> f32 {
    (size * share - 0.01).ceil()
}

/// A text style: GPUI has no letter spacing, so tracked mono is set a cell a letter.
#[derive(Clone, Copy)]
pub struct Ts {
    pub family: &'static str,
    pub bytes: &'static [u8],
    pub size: f32,
    pub line: f32,
    pub tracking: f32,
    pub mono: bool,
}

impl Ts {
    pub fn tracked(self, tracking: f32) -> Ts {
        Ts { tracking, ..self }
    }
    pub fn line(self, line: f32) -> Ts {
        Ts { line, ..self }
    }
}

/// `DS.Font.martian(size, weight:)`: one face at width 87.5. GPUI can't set a variable font's
/// axes, so each weight is a fixed cut of Martian Mono (`tools/mono_cuts.py`).
pub fn martian(size: f32, weight: u16) -> Ts {
    let (family, bytes) = match weight {
        430 => ("Iris Mono 430", MONO_430),
        560 => ("Iris Mono 560", MONO_560),
        _ => ("Iris Mono 450", MONO_450),
    };
    Ts { family, bytes, size, line: size * MONO_LINE, tracking: 0.0, mono: true }
}
pub fn caption() -> Ts { martian(11.0, 450) }
pub fn body() -> Ts { martian(12.5, 450) }
pub fn subtitle() -> Ts { martian(13.5, 450) }
pub fn key_font() -> Ts { martian(10.5, 560) }

/// A key's face: Martian Mono has no ⌘ ⇥ ↵ ⌥ ⌃ ⇧, and Core Text sets them in Menlo in the Swift
/// app, so they're set in Menlo here (Windows has them in Segoe UI Symbol).
pub fn key_face(key: &str) -> Ts {
    if key.chars().all(|c| "⌘⇥↵⌥⌃⇧⌫⎋".contains(c)) {
        let family = if cfg!(target_os = "windows") { "Segoe UI Symbol" } else { "Menlo" };
        Ts { family, ..key_font() }
    } else {
        key_font()
    }
}

/// `DS.Font.brush(size)`: Caveat Brush, for Iris's own words.
pub fn brush(size: f32) -> Ts {
    Ts { family: "Caveat Brush", bytes: CAVEAT, size, line: line_box(size, BRUSH_LINE), tracking: 0.0, mono: false }
}

/// The width of a line of mono text: every glyph is one advance, plus the tracking.
pub fn mono_w(s: &str, size: f32, tracking: f32) -> f32 {
    s.chars().count() as f32 * (size * MONO_ADVANCE + tracking)
}

/// The width of brush words as SwiftUI sizes them: their advances, widened to take in ink that
/// overhangs either end (Caveat's letters reach past their advance).
pub fn brush_w(s: &str, size: f32) -> f32 {
    brush_box(s, size).1
}

/// (lead, width): the ink overhanging the start, and the width with both overhangs.
pub fn brush_box(s: &str, size: f32) -> (f32, f32) {
    thread_local! { static W: RefCell<HashMap<(String, u32), (f32, f32)>> = RefCell::new(HashMap::new()); }
    let key = (s.to_string(), size.to_bits());
    if let Some(w) = W.with(|m| m.borrow().get(&key).copied()) {
        return w;
    }
    let font = FontRef::new(CAVEAT).unwrap();
    let gm = font.glyph_metrics(Size::new(size), LocationRef::default());
    let (mut pen, mut left, mut right) = (0.0f32, 0.0f32, 0.0f32);
    for (g, adv) in crate::raster::shape(CAVEAT, s, size) {
        if let Some(b) = gm.bounds(g) {
            left = left.min(pen + b.x_min);
            right = right.max(pen + b.x_max);
        }
        pen += adv;
    }
    let lead = (-left).max(0.0);
    let w = (lead, pen.max(right) + lead);
    W.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 512 {
            m.clear();
        }
        m.insert(key, w);
    });
    w
}

/// A text box in a style: one line, its line box `ts.line` tall.
pub fn styled(ts: Ts, c: Rgb8) -> Div {
    let mut d = div().flex_none().font_family(ts.family).text_size(z(ts.size)).line_height(z(ts.line))
        .text_color(col(c)).whitespace_nowrap();
    // The keycaps' ⌘ ⇥ ↵ aren't in Martian Mono: Core Text falls back to Menlo for them in the
    // Swift app; Windows has them in Segoe UI Symbol.
    d.text_style().font_fallbacks = Some(FontFallbacks::from_fonts(vec!["Menlo".into(), "Segoe UI Symbol".into()]));
    let _ = ts.mono;
    d
}

/// One line of text. Tracked mono is set a cell a letter, each cell one advance plus tracking.
pub fn t(s: impl Into<String>, ts: Ts, c: Rgb8) -> Div {
    let s: String = s.into();
    if ts.tracking == 0.0 || !ts.mono {
        return styled(ts, c).child(s);
    }
    let cell = ts.size * MONO_ADVANCE + ts.tracking;
    let mut r = styled(ts, c).flex().flex_row();
    for ch in s.chars() {
        r = r.child(div().flex_none().w(z(cell)).child(ch.to_string()));
    }
    r
}

// MARK: - Building blocks


/// A box of fixed size that holds children placed with `at`.
pub fn sized(w: f32, h: f32) -> Div {
    div().flex_none().relative().w(z(w)).h(z(h))
}

/// `e` at (`x`, `y`) inside its parent, out of flow.
pub fn at(e: impl IntoElement, x: f32, y: f32) -> Div {
    div().absolute().left(z(x)).top(z(y)).child(e)
}

/// A pen path in a `w`×`h` box: stroked `width` wide, or filled when `width` is `None`.
pub fn drawn(w: f32, h: f32, path: Pen, width: Option<f32>, color: Rgba) -> AnyElement {
    let k = zoom();
    canvas(|_, _, _| (), move |b, _, window, _| {
        if let Some(p) = path.gpui(b.origin, width, k) {
            window.paint_path(p, color);
        }
    })
    .flex_none().w(z(w)).h(z(h)).into_any_element()
}

/// A printed picture in a `w`×`h` box, one image pixel a device pixel.
pub fn printed(w: f32, h: f32, img: &Img) -> AnyElement {
    let img = img.clone();
    canvas(|_, _, _| (), move |b, _, window, _| {
        let _ = window.paint_image(b, b, Default::default(), img, 0, false);
    })
    .flex_none().w(z(w)).h(z(h)).into_any_element()
}

/// A printed picture filling its box (stretched if the box changes size).
pub fn printed_fill(img: &Img) -> AnyElement {
    let img = img.clone();
    canvas(|_, _, _| (), move |b, _, window, _| {
        let _ = window.paint_image(b, b, Default::default(), img, 0, false);
    })
    .size_full().into_any_element()
}

/// `PenRule`: a divider drawn with the pen, `length` long, 3 points tall.
pub fn pen_rule(seed: i64, length: f32, color: Rgba, width: f32) -> AnyElement {
    drawn(length, 3.0, pen::hand_line(seed, 0.5, 0.0, 1.5, length), Some(width), color)
}

/// `HandRect` filled flat (a key's drop, a wash).
pub fn hand_fill(seed: i64, wobble: f32, w: f32, h: f32, color: Rgba) -> AnyElement {
    drawn(w, h, pen::hand_rect(seed, wobble, 0.0, true, 0.0, 0.0, w, h), None, color)
}

/// `HandRect` stroked with the pen.
pub fn hand_outline(seed: i64, wobble: f32, overshoot: f32, w: f32, h: f32, color: Rgba, width: f32) -> AnyElement {
    drawn(w, h, pen::hand_rect(seed, wobble, overshoot, false, 0.0, 0.0, w, h), Some(width), color)
}

/// `HandRect(closed: true).printedInk(rgb, seed:)`.
pub fn hand_printed(seed: i64, wobble: f32, w: f32, h: f32, rgb: Rgb8, ink_seed: i64, scale: f32) -> AnyElement {
    let plate = Plate { w, h, scale };
    let px_ = riso::cached(format!("handrect-{seed}-{wobble}-{rgb:?}-{ink_seed}"), &plate, || {
        let mask = plate.fill(&pen::hand_rect(seed, wobble, 0.0, true, 0.0, 0.0, w, h).skia().unwrap());
        riso::print(&plate, rgb, ink_seed, &mask, 1.0)
    });
    printed(w, h, &px_)
}

/// Same as `hand_printed`, for widths that change continuously (progress bars) so intermediate
/// frames are released on the next frame instead of populating `riso::PRINTS`.
pub fn hand_printed_frame(seed: i64, wobble: f32, w: f32, h: f32, rgb: Rgb8, ink_seed: i64, scale: f32) -> AnyElement {
    let plate = Plate { w, h, scale };
    let (pw, ph) = plate.px();
    let mask = plate.fill(&pen::hand_rect(seed, wobble, 0.0, true, 0.0, 0.0, w, h).skia().unwrap());
    let px_ = riso::frame_image(pw, ph, riso::print(&plate, rgb, ink_seed, &mask, 1.0));
    printed(w, h, &px_)
}

/// Vertical padding needed above and below a `w`-wide marker rotated by `degrees`.
fn marker_pad(w: f32, degrees: f32) -> f32 {
    (w * degrees.to_radians().sin().abs() / 2.0).ceil() + 1.0
}

/// Coverage mask for `MarkerSwipe`: a rough paintbrush stroke inside `0..w × 0..bh` with subtle
/// dry-brush streaks cut near the trailing right bristles (matching `BrushSlab` DNA), rotated by
/// `degrees` about its centre.
fn marker_mask(plate: &Plate, w: f32, bh: f32, pad: f32, seed: i64, degrees: f32) -> tiny_skia::Mask {
    let path = pen::paintbrush_stroke(seed, w, bh);
    let turn = tiny_skia::Transform::from_rotate_at(degrees, w / 2.0, pad + bh / 2.0);
    let stroke = path.translated(0.0, pad).skia().unwrap().transform(turn).unwrap();
    let shape = plate.fill(&stroke);
    let mut pm = riso::opaque(plate, 10.0);
    let mut rng = riso::Rng::new(seed.wrapping_mul(31).wrapping_add(7));
    let inset_l = 2.8f32.min(w * 0.14);
    let inset_r = 4.8f32.min(w * 0.22);
    let body_w = (w - inset_l - inset_r).max(4.0);
    let xf = plate.transform().pre_concat(turn);
    for _ in 0..5 {
        let edge = rng.f() < 0.75;
        let sy = pad + if edge {
            if rng.f() < 0.5 { bh * (0.14 + rng.f() * 0.16) } else { bh * (0.72 + rng.f() * 0.16) }
        } else {
            bh * (0.38 + rng.f() * 0.24)
        };
        let slen = (body_w * 0.45).min(10.0 + rng.f() * 20.0);
        let sx = (w - inset_r * 0.7 - slen - rng.f() * (body_w * 0.25).min(16.0)).max(inset_l + 2.0);
        let mut p = Pen::new();
        p.m(sx, sy);
        p.l((sx + slen).min(w - 0.5), sy + (rng.f() - 0.5) * 0.5);
        if let Some(sp) = p.skia() {
            let mut paint = tiny_skia::Paint::default();
            let alpha = 0.42 + rng.f() * 0.42;
            paint.set_color(tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, alpha).unwrap());
            paint.blend_mode = tiny_skia::BlendMode::DestinationOut;
            paint.anti_alias = true;
            let st = tiny_skia::Stroke {
                width: 0.5 + rng.f() * 0.5,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            pm.stroke_path(&sp, &paint, &st, xf, None);
        }
    }
    riso::intersect(&shape, &riso::alpha_mask(&pm))
}

/// `MarkerSwipe` as straight RGBA: the pink marker under a brush word, a rough paintbrush swipe
/// with frayed bristles and dry-brush streaks printed through the screen and multiplied onto the
/// paper. `h` is the stroke's height; its box is 2 taller, turned `degrees` about its middle,
/// with `pad` points above and below for the turn.
pub fn marker_rgba(w: f32, h: f32, seed: i64, degrees: f32, scale: f32) -> (Plate, f32, Vec<u8>) {
    let bh = h + 2.0;
    let pad = marker_pad(w, degrees);
    let plate = Plate { w, h: bh + 2.0 * pad, scale };
    let mask = marker_mask(&plate, w, bh, pad, seed, degrees);
    let (grid, specks) = riso::ink_screen(ink::PINK);
    let rgba = riso::print_with(&plate, ink::PINK, seed + 20, grid, specks, &mask, 1.0, |c| riso::multiply(c, ink::PAPER));
    (plate, pad, rgba)
}

/// `MarkerSwipe` placed: the pink marker `w` wide, its stroke `h` tall, turned `degrees`; returns
/// it and the room it keeps above and below for the turn.
pub fn marker_swipe(w: f32, h: f32, seed: i64, degrees: f32, scale: f32) -> (Div, f32) {
    let pad = marker_pad(w, degrees);
    let plate = Plate { w, h: h + 2.0 + 2.0 * pad, scale };
    let img = riso::cached(format!("marker-{seed}-{h}-{w}-{degrees}"), &plate, || marker_rgba(w, h, seed, degrees, scale).2);
    (sized(w, plate.h).child(printed(w, plate.h, &img)), pad)
}

/// The drop under a key or button: `DS.drop(alpha)`.
pub fn drop(alpha: f32) -> Rgba {
    Rgba { a: alpha, ..col((8, 6, 5)) }
}

// MARK: - Keycaps

/// `Keycap.split`: one cap per key, like a real keyboard.
pub fn split_keys(combo: &str) -> Vec<String> {
    // Windows writes "Ctrl+R" (`keys::command_chip`): one cap per word.
    if combo.len() > 1 && combo.contains('+') && !combo.contains(['⌘', '⌥', '⌃', '⇧']) {
        return combo.split('+').map(str::to_string).collect();
    }
    let mut caps = Vec::new();
    let mut rest = String::new();
    for c in combo.chars() {
        if "⌘⌥⌃⇧".contains(c) && rest.is_empty() { caps.push(c.to_string()) } else { rest.push(c) }
    }
    if !rest.is_empty() {
        caps.push(rest);
    }
    caps
}

/// `Keycap` (chip): crema printed a touch off register under a pen edge, with a crisp drop.
pub fn keycap(key: &str, scale: f32) -> Div {
    let w = (10.0 + key.chars().count() as f32 * 7.0).max(20.0);
    let h = 19.0;
    let s = riso::str_seed(key, 7);
    sized(w, h)
        .child(at(hand_fill(s + 9, 0.3, w, h, drop(0.22)), 0.0, 1.0))
        .child(at(hand_printed(s + 3, 0.45, w, h, ink::CREMA, s + 3, scale), -0.8, -0.5))
        .child(at(hand_outline(s, 0.45, 2.0, w, h, col(hue::I12), 1.2), 0.0, 0.0))
        .child(at(div().w(z(w)).h(z(h)).flex().items_center().justify_center().child(t(key, key_face(key), hue::I12)), 0.0, -0.5))
}

pub fn keycaps_w(combo: &str) -> f32 {
    let caps = split_keys(combo);
    caps.iter().map(|k| (10.0 + k.chars().count() as f32 * 7.0).max(20.0)).sum::<f32>() + 3.0 * (caps.len() as f32 - 1.0)
}

/// `KeyHint`: keycaps and what they do, in small caps.
pub fn key_hint(key: &str, label: &str, scale: f32) -> Div {
    let caps = split_keys(key).into_iter().fold(div().flex().flex_row().gap(z(3.0)), |r, k| r.child(keycap(&k, scale)));
    div().flex().flex_row().flex_none().gap(z(6.0)).items_center().py(z(4.0))
        .child(caps)
        .child(t(label.to_uppercase(), caption().tracked(0.4), hue::I10))
}

pub fn key_hint_w(key: &str, label: &str) -> f32 {
    keycaps_w(key) + 6.0 + mono_w(label, 11.0, 0.4)
}

// MARK: - Buttons

/// `TactileButton`'s keys in its own ink: small pen-edged caps, one per key.
pub fn button_chips(combo: &str, seed: i64, ink_c: Rgb8, under: Rgb8) -> (Div, f32) {
    let mut r = div().flex().flex_row().gap(z(3.0));
    let mut w = 0.0;
    for (i, k) in split_keys(combo).iter().enumerate() {
        let cw = (mono_w(k, 10.5, 0.0) + 8.0).max(18.0);
        let ch = 18.0;
        r = r.child(sized(cw, ch).flex().items_center().justify_center()
            .child(at(hand_outline(seed + 5 + i as i64, 0.35, 1.5, cw, ch, over(ink_c, 0.55, under), 1.0), 0.0, 0.0))
            .child(t(k.clone(), key_face(k), ink_c)));
        w += cw + if i > 0 { 3.0 } else { 0.0 };
    }
    (r, w)
}

/// Mask for `BrushSlab`: its shape, with the dry streaks and pits cut out.
pub fn slab_mask(plate: &Plate, seed: i64, dry: f32) -> tiny_skia::Mask {
    let (w, h) = (plate.w, plate.h);
    let shape = plate.fill(&pen::slab(seed, w, h).skia().unwrap());
    if dry <= 0.0 {
        return shape;
    }
    let mut pm = riso::opaque(plate, 20.0);
    let mut rng = riso::Rng::new(seed.wrapping_mul(29).wrapping_add(3));
    let cut = |pm: &mut tiny_skia::Pixmap, path: &tiny_skia::Path, alpha: f32, width: Option<f32>| {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, alpha.clamp(0.0, 1.0)).unwrap());
        paint.blend_mode = tiny_skia::BlendMode::DestinationOut;
        paint.anti_alias = true;
        match width {
            Some(width) => {
                let st = tiny_skia::Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
                pm.stroke_path(path, &paint, &st, plate.transform(), None);
            }
            None => pm.fill_path(path, &paint, tiny_skia::FillRule::Winding, plate.transform(), None),
        }
    };
    let reach = w.min(130.0);
    for k in 0..(16.0 * dry) as i32 {
        let from_left = k % 5 == 4;
        let edge = rng.f() < 0.8;
        let y = if edge {
            if rng.f() < 0.5 { h * (0.05 + rng.f() * 0.15) } else { h * (0.8 + rng.f() * 0.15) }
        } else {
            h * (0.25 + rng.f() * 0.5)
        };
        let len = reach * (0.08 + rng.f() * 0.3);
        let start = if from_left { 4.0 + rng.f() * 30.0 } else { w + 4.0 - len - rng.f() * reach * 0.45 };
        let mut p = Pen::new();
        p.m(start.max(0.0), y);
        p.l((w + 4.0).min(start + len), y + (rng.f() - 0.5) * 1.2);
        let open = edge && rng.f() < 0.3 * dry.min(1.6);
        let alpha = if open { 0.9 } else if edge { 0.2 + rng.f() * 0.3 * dry } else { 0.1 + rng.f() * 0.12 };
        let width = (if open { 0.9 } else { 0.5 + rng.f() * 0.5 }) * if dry > 1.0 { 1.3 } else { 1.0 };
        if let Some(path) = p.skia() {
            cut(&mut pm, &path, alpha, Some(width));
        }
    }
    for k in 0..(22.0 * dry) as i32 {
        let along = rng.f() * 60.0;
        let x = if k % 3 == 0 { 4.0 + along * 0.6 } else { w - 4.0 - along };
        let cy = if rng.f() < 0.5 { h * rng.f() * 0.25 } else { h * (0.75 + rng.f() * 0.25) };
        let r = 0.3 + rng.f() * 0.6;
        let alpha = 0.4 + rng.f() * 0.5;
        if let Some(path) = tiny_skia::PathBuilder::from_circle(x, cy, r) {
            cut(&mut pm, &path, alpha, None);
        }
    }
    riso::intersect(&shape, &riso::alpha_mask(&pm))
}

/// The primary button's one brush (`TactileButton.Face.brush`): every primary is this stroke.
pub const BRUSH: i64 = 4242;

/// `TactileButton(role: .primary)`: one loaded stroke of a dry brush in espresso over a second
/// pass with rose in it, printed up and left off register.
pub fn primary_button(title: &str, key_chip: Option<&str>, loading: bool, reserve: Option<&str>, enabled: bool, scale: f32) -> (Div, f32) {
    primary_button_full(title, key_chip, loading, reserve, enabled, 32.0, scale)
}

/// The compact primary button (`compact: true`, 26 points tall).
pub fn primary_button_sized(title: &str, h: f32, scale: f32) -> (Div, f32) {
    primary_button_full(title, None, false, None, true, h, scale)
}

fn primary_button_full(title: &str, key_chip: Option<&str>, loading: bool, reserve: Option<&str>, enabled: bool, h: f32, scale: f32) -> (Div, f32) {
    let ink_c = hue::I1;
    let title_w = mono_w(title, 12.5, -0.12).max(reserve.map_or(0.0, |r| mono_w(r, 12.5, -0.12)));
    let seed = riso::str_seed(title, 17);
    let mut content = div().flex().flex_row().gap(z(6.0)).items_center();
    let mut w = 36.0 + title_w;
    if loading {
        content = content.child(ink_dots(ink_c, 4.0));
        w += 4.0 * 3.0 + 3.2 * 2.0 + 6.0;
    }
    content = content.child(div().flex_none().w(z(title_w)).child(t(title, body().tracked(-0.12), ink_c)));
    if let Some(k) = key_chip {
        let (chips, cw) = button_chips(k, seed, ink_c, ink::INK);
        content = content.child(chips);
        w += 6.0 + cw;
    }
    let plate = Plate { w, h, scale };
    let under = riso::cached(format!("slab-under-{w}-{h}"), &plate, || {
        let mask = slab_mask(&plate, BRUSH + 7, 0.0);
        let mut base = riso::print(&plate, ink::INK, BRUSH + 7, &mask, 1.0);
        riso::lay(&mut base, &riso::print(&plate, ink::ROSE, BRUSH + 7, &mask, 0.35));
        base
    });
    let top = riso::cached(format!("slab-top-{w}-{h}"), &plate, || {
        riso::print(&plate, ink::INK, BRUSH, &slab_mask(&plate, BRUSH, 1.9), 1.0)
    });
    let mut b = sized(w, h);
    if enabled {
        b = b.child(at(printed(w, h, &under), -2.5, -2.0));
    }
    b = b.child(at(printed(w, h, &top), 0.0, 0.0))
        .child(at(content.h(z(h)).pl(z(18.0)).pr(z(18.0)), 0.0, 0.0));
    (if enabled { b } else { b.opacity(0.6) }, w)
}

/// `TactileButton(role: .secondary)`: paper face, pen outline, a small drop.
pub fn secondary_button(title: &str, key_chip: Option<&str>) -> (Div, f32) {
    let seed = riso::str_seed(title, 17);
    let title_w = mono_w(title, 12.5, -0.12);
    let mut content = div().flex().flex_row().gap(z(6.0)).items_center().child(t(title, body().tracked(-0.12), hue::I12));
    let mut w = 24.0 + title_w;
    if let Some(k) = key_chip {
        let (chips, cw) = button_chips(k, seed, hue::I12, ink::PAPER);
        content = content.child(chips);
        w += 6.0 + cw;
    }
    let h = 32.0;
    let b = sized(w, h)
        .child(at(hand_fill(seed + 9, 0.3, w, h, drop(0.18)), 0.0, 1.2))
        .child(at(hand_fill(seed + 3, 0.4, w, h, col(ink::PAPER)), 0.0, 0.0))
        .child(at(hand_outline(seed, 0.45, 2.0, w, h, over(hue::I12, 0.75, ink::PAPER), 1.1), 0.0, 0.0))
        .child(at(content.h(z(h)).pl(z(12.0)).pr(z(12.0)), 0.0, 0.0));
    (b, w)
}

/// `InkDots`: three pen dots darkening in turn, in place of a spinner: `0.25 + 0.75 × wave`,
/// the wave `sin(5t − 0.9 i)`. Drawn at one moment (`t` seconds); the live panel animates it.
pub fn ink_dots_at(c: Rgb8, dot: f32, time: f32) -> Div {
    let mut r = div().flex().flex_row().flex_none().gap(z(dot * 0.8)).items_center();
    for i in 0..3 {
        let wave = 0.5 + 0.5 * (5.0 * time - 0.9 * i as f32).sin();
        let a = 0.25 + 0.75 * wave;
        r = r.child(drawn(dot, dot, pen::hand_ellipse(30 + i, 0.6, dot, dot), None, Rgba { a, ..col(c) }));
    }
    r
}

pub fn ink_dots(c: Rgb8, dot: f32) -> Div {
    let time = if crate::live_slip::reduce_motion() || crate::app::settings().light_mode_active() { 0.0 } else { crate::clock() };
    ink_dots_at(c, dot, time)
}

// MARK: - Tone tabs

/// `ToneTabs`: the others in mono, the chosen one lettered in brush. Each tab holds the width
/// of both faces, so choosing never shifts the row. Returns each tab with its index (so the
/// caller can make them clickable), and the pink marker separately, so it can fly between tabs.
pub fn tone_tabs(labels: &[&str], selected: usize) -> Vec<(usize, Div)> {
    labels.iter().enumerate().map(|(i, label)| (i, tone_tab(label, i == selected, 0.0))).collect()
}

/// One tone tab: the chosen one in brush, another in mono, darkening from i10 to i12 as the
/// pointer comes onto it (`hover`, 0 → 1).
pub fn tone_tab(label: &str, chosen: bool, hover: f32) -> Div {
    tone_tab_blend(label, if chosen { 1.0 } else { 0.0 }, hover)
}

/// One tone tab at blend `chosen` (0 = mono, 1 = Caveat Brush): cross-fades between faces while
/// switching tones, matching SwiftUI's `ToneTabs` transition, and renders only the resting face
/// once settled.
pub fn tone_tab_blend(label: &str, chosen: f32, hover: f32) -> Div {
    let tab_h = tab_height();
    let tab = sized(tab_w(label), tab_h);
    let c = chosen.clamp(0.0, 1.0);
    let ink_ = if hover > 0.0 { crate::interact::mix(hue::I10, hue::I12, hover) } else { hue::I10 };
    if c >= 0.996 {
        tab.child(at(t(label, brush(23.0), hue::I12), 0.0, tab_h - line_box(23.0, BRUSH_LINE)))
    } else if c <= 0.004 {
        tab.child(at(t(label, body().tracked(-0.12), ink_), 0.0, tab_h - 4.0 - line_box(12.5, MONO_LINE)))
    } else {
        tab.child(at(t(label, body().tracked(-0.12), ink_).opacity(1.0 - c), 0.0, tab_h - 4.0 - line_box(12.5, MONO_LINE)))
            .child(at(t(label, brush(23.0), hue::I12).opacity(c), 0.0, tab_h - line_box(23.0, BRUSH_LINE)))
    }
}

/// A tone tab's size: as wide as both its faces, as tall as the taller.
pub fn tone_tab_size(label: &str) -> (f32, f32) {
    (tab_w(label), tab_height())
}

fn tab_height() -> f32 {
    line_box(23.0, BRUSH_LINE).max(line_box(12.5, MONO_LINE) + 4.0)
}

fn tab_w(label: &str) -> f32 {
    brush_w(label, 23.0).max(mono_w(label, 12.5, -0.12))
}

/// Where the marker sits under tone `i` in the row: (x, y, width, height, picture).
pub fn tone_marker(labels: &[&str], i: usize, scale: f32) -> (f32, f32, f32, f32, Img) {
    let i = i.min(labels.len().saturating_sub(1));
    let label = labels.get(i).copied().unwrap_or("Fix");
    let x: f32 = labels[..i].iter().map(|l| tab_w(l) + 18.0).sum();
    let bw = brush_w(label, 23.0);
    let (w, h, seed, degrees) = (bw + 9.0, 6.5, i as i64 + 3, -1.1);
    let pad = marker_pad(w, degrees);
    let plate = Plate { w, h: h + 2.0 + 2.0 * pad, scale };
    let img = riso::cached(format!("marker-{seed}-{h}-{w}-{degrees}"), &plate, || marker_rgba(w, h, seed, degrees, scale).2);
    (x - 4.0, tab_height() - 8.0 + 0.75 - pad, w, plate.h, img)
}

/// The pink brushstroke under the chosen tab, animated via `Live`: the old mark lifts/fades out
/// quickly (`105 ms`) while a fresh loaded-to-dry brushstroke paints itself across the new tab
/// in the travel direction (`210 ms`, `DS.Motion` ease-out) using a pure GPU `overflow_hidden`
/// clip over the cached brush image (zero per-frame CPU rasterization). Under Reduce Motion it
/// cross-fades in place (`150 ms`) with no wipe travel.
pub fn tone_marker_live(l: &crate::interact::Live, id: &str, labels: &[&str], selected: usize, scale: f32) -> AnyElement {
    const BRUSH_IN: f32 = 0.21;
    const BRUSH_OUT: f32 = 0.105;
    const REDUCE_FADE: f32 = 0.15;
    let (tx, ty, tw, th, to_img) = tone_marker(labels, selected, scale);
    if l.view.is_none() {
        return div().absolute().top(z(ty)).left(z(tx)).w(z(tw)).h(z(th)).child(printed_fill(&to_img)).into_any_element();
    }
    if l.reduce_motion {
        let (from, to, elapsed) = l.flight_elapsed(id, selected, REDUCE_FADE);
        if from != to && elapsed < REDUCE_FADE {
            let a = ds_curve((elapsed / REDUCE_FADE).clamp(0.0, 1.0));
            let (fx, fy, fw, fh, from_img) = tone_marker(labels, from, scale);
            return div().absolute().top_0().left_0().size_full()
                .child(div().absolute().top(z(fy)).left(z(fx)).w(z(fw)).h(z(fh)).opacity(1.0 - a).child(printed_fill(&from_img)))
                .child(div().absolute().top(z(ty)).left(z(tx)).w(z(tw)).h(z(th)).opacity(a).child(printed_fill(&to_img)))
                .into_any_element();
        }
        return div().absolute().top(z(ty)).left(z(tx)).w(z(tw)).h(z(th)).child(printed_fill(&to_img)).into_any_element();
    }
    let (from, to, elapsed) = l.flight_elapsed(id, selected, BRUSH_IN);
    if from != to && elapsed < BRUSH_IN {
        let out_k = ds_curve((elapsed / BRUSH_OUT).clamp(0.0, 1.0));
        let in_k = ds_curve((elapsed / BRUSH_IN).clamp(0.0, 1.0));
        let forward = to >= from;
        let reveal = (0.06 + 0.94 * in_k).clamp(0.06, 1.0);
        let clip_w = tw * reveal;
        let (clip_x, img_x) = if forward { (tx, 0.0) } else { (tx + tw - clip_w, -(tw - clip_w)) };
        let mut layer = div().absolute().top_0().left_0().size_full();
        if out_k < 0.996 {
            let (fx, fy, fw, fh, from_img) = tone_marker(labels, from, scale);
            layer = layer.child(
                div().absolute().top(z(fy)).left(z(fx)).w(z(fw)).h(z(fh))
                    .opacity(1.0 - out_k)
                    .child(printed_fill(&from_img)),
            );
        }
        let paint_in = div().absolute().top(z(ty)).left(z(clip_x)).w(z(clip_w)).h(z(th)).overflow_hidden()
            .child(div().absolute().top_0().left(z(img_x)).w(z(tw)).h(z(th)).child(printed_fill(&to_img)));
        return layer.child(paint_in).into_any_element();
    }
    div().absolute().top(z(ty)).left(z(tx)).w(z(tw)).h(z(th)).child(printed_fill(&to_img)).into_any_element()
}

/// `IRIS_SLOW=n` plays the app's motion n times slower, to look at it frame by frame.
pub fn slow() -> f32 {
    static SLOW: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SLOW.get_or_init(|| std::env::var("IRIS_SLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

/// SwiftUI's `.spring(duration:bounce:)`: where it is `t` seconds in, from 0 to 1.
pub fn swift_spring(t: f32, duration: f32, bounce: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let omega = std::f32::consts::TAU / duration;
    let zeta: f32 = 1.0 - bounce;
    let wd = omega * (1.0 - zeta * zeta).max(1e-6).sqrt();
    1.0 - (-zeta * omega * t).exp() * ((wd * t).cos() + zeta * omega / wd * (wd * t).sin())
}

/// `DS.Motion`'s curve, `.timingCurve(0.23, 1, 0.32, 1)`: `x` of the way through, how far.
pub fn ds_curve(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let (x1, y1, x2, y2) = (0.23f32, 1.0f32, 0.32f32, 1.0f32);
    let bez = |t: f32, a: f32, b: f32| 3.0 * (1.0 - t) * (1.0 - t) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t;
    // Solve the curve's x for t by bisection, then read y.
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < x { lo = mid } else { hi = mid }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

/// SwiftUI's `.spring(duration: 0.4, bounce: 0.12)` (`DS.Motion.pill`): where it is after `t`
/// seconds, from 0 to 1, overshooting a touch.
pub fn pill_spring(t: f32) -> f32 {
    let omega = std::f32::consts::TAU / 0.4;
    let zeta: f32 = 1.0 - 0.12;
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    1.0 - (-zeta * omega * t).exp() * ((wd * t).cos() + zeta * omega / wd * (wd * t).sin())
}

// MARK: - Notes (set a degree or two off straight, so printed whole)

/// A word in a face, filled into a sheet at (`x`, `base`).
pub fn word_into(sheet: &mut Sheet, ts: Ts, s: &str, x: f32, base: f32, c: Rgb8) {
    if let Some(p) = raster::text_path(ts.bytes, ts.size, s, x, base, ts.tracking) {
        sheet.fill(&p, c, 1.0);
    }
}

/// A picture `w`×`h` points, with `pad` room all round, turned `degrees` about its middle and
/// drawn by `draw` (in the unpadded box's points). Placed so the box sits where it would unturned.
pub fn turned(key: String, w: f32, h: f32, pad: f32, degrees: f32, scale: f32, draw: impl FnOnce(&mut Sheet)) -> Div {
    let plate = Plate { w: w + 2.0 * pad, h: h + 2.0 * pad, scale };
    let img = riso::cached(key, &plate, || {
        let mut sheet = Sheet::new(Plate { w: plate.w, h: plate.h, scale }, degrees, plate.w / 2.0, plate.h / 2.0);
        sheet.turn = sheet.turn.pre_translate(pad, pad);
        draw(&mut sheet);
        sheet.rgba()
    });
    sized(w, h).child(at(printed(plate.w, plate.h, &img), -pad, -pad))
}

/// `DoneNote`: a pink pen tick and Iris's word in brush, a degree off straight.
pub fn done_note(word: &str, size: f32, scale: f32) -> (Div, f32) {
    let (tw, th) = (size * 0.72, size * 0.58);
    let ww = brush_w(word, size);
    let lh = line_box(size, BRUSH_LINE);
    let base = size * BRUSH_ASCENT;
    let w = tw + 6.0 + ww;
    let (lead, _) = brush_box(word, size);
    let word_s = word.to_string();
    let e = turned(format!("done-{word}-{size}"), w, lh, 8.0, -2.0, scale, move |s| {
        s.stroke(&pen::hand_tick(tw, th).translated(0.0, base - th).skia().unwrap(), 2.0, ink::PINK_TEXT);
        word_into(s, brush(size), &word_s, tw + 6.0 + lead, base + (lh - size * 1.26) / 2.0, ink::PINK_TEXT);
    });
    (e, w)
}

/// "looks good": a green pen tick, the one green in Iris, and the brush word over the marker.
pub fn looks_good(scale: f32) -> (Div, f32) {
    let size = 26.0;
    let ww = brush_w("looks good", size);
    let (lead, _) = brush_box("looks good", size);
    let lh = line_box(size, BRUSH_LINE);
    let base = size * BRUSH_ASCENT;
    let w = 19.0 + 7.0 + ww;
    let e = turned(format!("looks-good-{scale}"), w, lh, 10.0, -2.0, scale, move |s| {
        s.stroke(&pen::hand_tick(19.0, 15.0).translated(0.0, base - 15.0).skia().unwrap(), 2.4, hue::GREEN);
        let (mp, pad, rgba) = marker_rgba(ww + 8.0, 7.0, 8, -1.2, scale);
        s.picture(&rgba, mp.w, mp.h, 26.0 - 4.0, lh - 9.0 - 3.0 - pad);
        word_into(s, brush(size), "looks good", 26.0 + lead, base + (lh - size * 1.26) / 2.0, hue::I12);
    });
    (e, w)
}

/// The red pen's note in brush, two degrees off straight.
pub fn red_note(note: &str, size: f32, scale: f32) -> (Div, f32) {
    let w = brush_w(note, size);
    let (lead, _) = brush_box(note, size);
    let lh = line_box(size, BRUSH_LINE);
    let base = size * BRUSH_ASCENT;
    let n = note.to_string();
    let e = turned(format!("red-{note}-{size}"), w, lh, 12.0, -2.0, scale, move |s| {
        word_into(s, brush(size), &n, lead, base + (lh - size * 1.26) / 2.0, hue::RED_TEXT);
    });
    (e, w)
}

/// A failed check at the panel's bottom left, where "looks good" sits: the red ring round "?"
/// (the problem's pen mark, as on setup and the message by your text) and the note in red
/// brush, two degrees off straight.
pub fn problem_note(note: &str, scale: f32) -> (Div, f32) {
    let size = 24.0;
    let (words, ww) = red_note(note, size, scale);
    let lh = line_box(size, BRUSH_LINE);
    let qh = line_box(20.0, BRUSH_LINE);
    let qw = brush_w("?", 20.0);
    let mark = sized(24.0, 24.0)
        .child(at(t("?", brush(20.0).line(qh), hue::RED_TEXT), (24.0 - qw) / 2.0, (24.0 - qh) / 2.0))
        .child(at(crate::toast::brush_ring(ink::RED, 9, 24.0, 24.0, 1.0, scale), 0.0, 0.0));
    let h = lh.max(24.0);
    let w = 24.0 + 8.0 + ww;
    (sized(w, h).child(at(mark, 0.0, (h - 24.0) / 2.0)).child(at(words, 32.0, (h - lh) / 2.0)), w)
}

/// `Badge(tone: .warn)`: a tinted label with a pen outline (being phased out in Swift; drawn
/// as Swift draws it). `w` is the width it was given.
pub fn badge(words: &str, w: f32) -> Div {
    let seed = riso::str_seed(words, 13);
    let h = 20.0;
    sized(w, h)
        .child(at(hand_fill(seed + 3, 0.35, w, h, col(hue::WARN_TINT)), 0.0, 0.0))
        .child(at(hand_outline(seed, 0.4, 2.0, w, h, over(hue::WARN_TEXT, 0.45, hue::WARN_TINT), 1.0), 0.0, 0.0))
        .child(at(div().w(z(w - 16.0)).h(z(h)).flex().items_center().child(t(words, caption(), hue::WARN_TEXT)), 8.0, 0.0))
}

pub fn badge_w(words: &str) -> f32 {
    16.0 + mono_w(words, 11.0, 0.0)
}

// MARK: - Tape

/// `TapeLabel`: the app's name in mono on pale masking tape, its icon stuck on the corner.
/// `text_w` is the room the name was given (a long name is cut short before it gets here).
pub fn tape_label(name: &str, icon: Option<&(Vec<u8>, u32)>, faded_icon: bool, text_w: f32, scale: f32) -> Div {
    let pad_x = 10.5 * 0.62 + 4.0;
    let pad_y = 10.5 * 0.26;
    let tw = text_w + 2.0 * pad_x;
    let th = line_box(10.5, MONO_LINE) + 2.0 * pad_y;
    let lead = if icon.is_some() { 16.0 } else { 0.0 };
    let name_s = name.to_string();
    let strip = turned(format!("tape-{name}-{tw}"), tw, th, 4.0, -1.5, scale, move |s| {
        s.fill(&pen::tape(tw, th).skia().unwrap(), riso::multiply(hue::TAPE, ink::PAPER), 1.0);
        let ts = martian(10.5, 430);
        word_into(s, ts, &name_s, pad_x, pad_y + (th - 2.0 * pad_y - ts.size * 1.2) / 2.0 + ts.size, hue::I12);
    });
    let mut e = sized(lead + tw, th).child(at(strip, lead, 0.0));
    if let Some((rgba, n)) = icon {
        let (rgba, n) = (rgba.clone(), *n);
        let icon_pic = turned(format!("icon-{}-{faded_icon}", riso::str_seed(name, 3)), 22.0, 22.0, 4.0, -9.0, scale, move |s| {
            let mut px_ = rgba;
            if faded_icon {
                for a in px_.iter_mut().skip(3).step_by(4) {
                    *a = (*a as f32 * 0.45).round() as u8;
                }
            }
            // The icon arrives at `n` pixels square; laid at 22 points.
            s.picture(&scale_to(&px_, n, (22.0 * s.plate.scale).round() as u32), 22.0, 22.0, 0.0, 0.0);
        });
        e = e.child(at(icon_pic, 0.0, (th - 22.0) / 2.0 - 5.0));
    }
    e
}

/// RGBA `n`×`n` scaled to `m`×`m` (bilinear).
fn scale_to(rgba: &[u8], n: u32, m: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(n, n, rgba.to_vec()).unwrap();
    image::imageops::resize(&img, m, m, image::imageops::FilterType::Triangle).into_raw()
}

pub fn tape_w(text_w: f32, icon: bool) -> f32 {
    text_w + 2.0 * (10.5 * 0.62 + 4.0) + if icon { 16.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::marker_rgba;

    #[test]
    fn marker_paintbrush_stays_in_bounds_with_frayed_bristles() {
        // Every tone seed stays inside its 0..w × 0..bh strip at 0 degrees (no banana bulge)
        // while keeping ragged, unclipped bristle tips on the trailing right end.
        for (seed, w) in [(3, 35.0), (4, 118.0), (5, 83.0), (6, 78.0), (7, 74.0)] {
            let (plate, pad, rgba) = marker_rgba(w, 6.5, seed, 0.0, 2.0);
            let (pw, ph) = plate.px();
            let top = (pad * 2.0) as u32;
            let bot = ((pad + 8.5) * 2.0) as u32;
            for y in 0..ph {
                if y < top || y >= bot {
                    for x in 0..pw {
                        assert_eq!(rgba[((y * pw + x) * 4 + 3) as usize], 0, "seed {seed} leaked at y={y}");
                    }
                }
            }
            // Trailing right bristle region (last ~4 pt = 8 px at 2×) should contain both painted
            // bristle tips and paper gaps (ragged tips, not a flat vertical rectangle).
            let mut tip_hits = 0u32;
            let mut tip_gaps = 0u32;
            let x_start = pw.saturating_sub(8);
            for y in (top + 2)..bot.saturating_sub(2) {
                for x in x_start..(pw.saturating_sub(1)) {
                    let a = rgba[((y * pw + x) * 4 + 3) as usize];
                    if a > 100 { tip_hits += 1; } else if a == 0 { tip_gaps += 1; }
                }
            }
            assert!(tip_hits > 0 && tip_gaps > 0, "seed {seed} right end lacked frayed bristles: hits={tip_hits}, gaps={tip_gaps}");
        }
    }

    #[test]
    fn tone_marker_handles_six_tones_and_clamps_bounds() {
        let six = ["Fix", "Professional", "Friendly", "Shorter", "Clearer", "Funny"];
        let (x0, _, w0, _, _) = super::tone_marker(&six, 0, 2.0);
        let (x5, _, w5, _, _) = super::tone_marker(&six, 5, 2.0);
        assert!(x5 > x0 && w0 > 0.0 && w5 > 0.0);
        // Out-of-range indices clamp to the last tone instead of panicking.
        let (x_clamped, _, w_clamped, _, _) = super::tone_marker(&six, 99, 2.0);
        assert_eq!(x_clamped, x5);
        assert_eq!(w_clamped, w5);
        // Empty label slice also never panics.
        let (_, _, w_empty, _, _) = super::tone_marker(&[], 0, 2.0);
        assert!(w_empty > 0.0);
    }
}
