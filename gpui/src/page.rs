//! The slip turned to another page (Updates): Back at the top left, the wordmark at the right,
//! the title in brush, and the letter from GitHub beside it (`UpdateCheckScene`, compact),
//! drawn at one settled moment as the Swift snapshot draws it.
//!
//! The letter's scene leans its letter and sways its flower, so it's printed whole as one
//! picture (GPUI can't turn a box); the Rune copy's drawing, composed on the CPU.

use gpui::{Div, ParentElement, Styled, div};
use tiny_skia::Transform;

use crate::garden;
use crate::letters;
use crate::parts::*;
use crate::pen;
use crate::raster::{self, Sheet};
use crate::riso::{self, Img, Plate, Rgb8, col, hue, ink, over};

/// What the letter came back with.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    UpToDate,
    Changes(usize),
    /// Came back empty-handed: the letter's answer half stays blank.
    Failed,
    /// Still looking: the letter hasn't come yet (drawn blank, the flower closed).
    Looking,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub lead: String,
    pub outcome: Outcome,
    /// What's new, ticked off (`UpdateChanges`), with Update & reopen under them.
    pub changes: Vec<String>,
    /// The answer's own button, when it has one ("Download", "Try again").
    pub button: Option<&'static str>,
    /// A warning (red title).
    pub warning: bool,
}

/// `PenArrow`'s strokes as a mask's printed ink: (straight RGBA) at `scale`.
fn arrow_rgba(w: f32, h: f32, weight: f32, seed: i64, right: bool, scale: f32) -> Img {
    let plate = Plate { w, h, scale };
    riso::cached(format!("arrow-{w}-{h}-{weight}-{seed}-{right}"), &plate, || {
        let strokes = [
            pen::pen_stroke(&[[0.97, 0.56], [0.6, 0.5], [0.3, 0.52], [0.05, 0.5]], w, h, weight, 0.7, 0.8, seed),
            pen::pen_stroke(&[[0.04, 0.5], [0.2, 0.28], [0.36, 0.08]], w, h, weight, 0.9, 0.35, seed + 1),
            pen::pen_stroke(&[[0.04, 0.5], [0.21, 0.72], [0.38, 0.93]], w, h, weight, 0.9, 0.35, seed + 2),
        ];
        let mut mask = plate.mask();
        // Pointing right: the strokes mirrored across the box (the ink isn't).
        let flip = if right { Transform::from_row(-1.0, 0.0, 0.0, 1.0, w, 0.0) } else { Transform::identity() };
        for s in &strokes {
            mask.fill_path(&s.skia().unwrap(), tiny_skia::FillRule::Winding, true, plate.transform().pre_concat(flip));
        }
        riso::print(&plate, ink::INK, seed, &mask, 1.0)
    })
}

/// `PenArrow`: the shaft pressed in and lifted off, two barbs flicked from the tip, printed.
pub fn pen_arrow(w: f32, h: f32, weight: f32, seed: i64, scale: f32) -> Div {
    sized(w, h).child(printed(w, h, &arrow_rgba(w, h, weight, seed, false, scale)))
}

/// The arrow pointing right (`PenArrow(right: true)`), as a link out has it.
pub fn pen_arrow_right(w: f32, h: f32, weight: f32, seed: i64, scale: f32) -> Div {
    sized(w, h).child(printed(w, h, &arrow_rgba(w, h, weight, seed, true, scale)))
}

/// `BackButton`: a pen arrow, "back" in brush and the esc key, on one centre line.
fn back_button(scale: f32) -> Div {
    let back_h = line_box(24.0, BRUSH_LINE);
    // The esc key at 85%: laid out smaller, as GPUI can't scale a box.
    let esc = with_zoom(0.85, || keycap("esc", scale));
    div().flex().flex_row().gap(z(6.0)).items_center().pl(z(6.0)).pr(z(2.0)).py(z(2.0))
        .child(pen_arrow(30.0, 16.0, 2.6, 3, scale))
        .child(div().relative().top(z(-3.0)).h(z(back_h)).child(t("back", brush(24.0), hue::I12)))
        .child(esc)
}

pub fn header(scale: f32) -> Div {
    let (logo, _, _) = letters::iris_logo(18.0, scale);
    div().flex().flex_row().items_center().px(z(16.0)).pb(z(10.0))
        .child(div().relative().left(z(-6.0)).top(z(3.0)).child(back_button(scale)))
        .child(div().flex_grow_1())
        .child(logo)
}

/// Text that wraps in `w` points.
pub fn para(s: impl Into<String>, ts: Ts, c: Rgb8, w: f32) -> Div {
    styled(ts, c).whitespace_normal().w(z(w)).child(s.into())
}

/// What a press on the page's button does (`"page-action"`), when the slip is live.
pub type Press<'a> = Option<&'a dyn Fn(&'static str) -> Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>>;

pub fn view(p: &Page, scale: f32) -> Div {
    view_live(p, None, scale)
}

pub fn view_live(p: &Page, press: Press, scale: f32) -> Div {
    use gpui::{InteractiveElement, IntoElement, StatefulInteractiveElement};
    let inner = crate::slip::WIDTH - 32.0;
    let art_w = 116.0;
    let words = div().flex().flex_col().gap(z(6.0)).w(z(inner - art_w - 8.0))
        .child(t(p.title.clone(), brush(26.0), if p.warning { hue::RED_TEXT } else { hue::I12 }))
        .child(para(p.lead.clone(), body(), hue::I11, inner - art_w - 8.0));
    // Live, the letter plays from the update check's clocks; else its settled moment.
    let live = crate::interact::get().filter(|l| l.view.is_some());
    let moment = live.as_ref().and_then(|l| PHASE.with(|ph| ph.borrow().as_ref().and_then(|ph| Moment::of(ph, l.reduce_motion))));
    let art = match (&live, &moment) {
        (Some(l), Some(m)) => compact_scene(&p.outcome, l, m, scale),
        _ => div().child(printed(SIZE.0, SIZE.1, &scene(&p.outcome, scale))),
    };
    let top = div().flex().flex_row().items_end().gap(z(8.0)).child(words).child(art);
    let mut v = div().flex().flex_col().gap(z(10.0)).px(z(16.0)).pt(z(14.0)).pb(z(10.0)).child(top);
    // The answer's button: the shared kit's (hover, press), the slip hearing the press.
    let button = |title: &'static str, primary: bool| -> gpui::AnyElement {
        use crate::controls::{self, Button};
        let b = if primary { Button::primary(title) } else { Button::secondary(title) }.compact().id(format!("page-{title}"));
        let (el, _) = controls::button(&b, scale, crate::interact::act(|_, _| {}));
        let wrap = div().id("page-action").child(el);
        match press {
            Some(p) => wrap.on_click(p("page-action")).into_any_element(),
            None => wrap.into_any_element(),
        }
    };
    if !p.changes.is_empty() {
        v = v.child(changes(&p.changes, inner).child(div().pt(z(2.0)).child(button("Update & reopen", true))));
    } else if let Some(title) = p.button {
        v = v.child(div().flex().flex_row().child(button(title, title != "Try again")));
    }
    v
}

/// `UpdateChanges`: each change ticked off in pink, what happens next, and the button.
fn changes(lines: &[String], w: f32) -> Div {
    let mut list = div().flex().flex_col().gap(z(8.0));
    for line in lines.iter().take(5) {
        let tick = drawn(12.0, 9.0, pen::hand_tick(12.0, 9.0), Some(1.8), col(ink::PINK_TEXT));
        // The tick's foot on the first line's baseline.
        let base = 12.5 * 1.0 + (line_box(12.5, MONO_LINE) - 12.5 * MONO_LINE) / 2.0;
        list = list.child(div().flex().flex_row().gap(z(8.0)).items_start()
            .child(sized(12.0, 9.0).mt(z(base - 9.0)).child(at(tick, 0.0, 0.0)))
            .child(para(line.clone(), body(), hue::I11, w - 20.0).line_clamp(2)));
    }
    div().flex().flex_col().gap(z(10.0))
        .child(list)
        .child(div().py(z(2.0)).child(pen_rule(61, w, over(hue::I12, 0.3, ink::PAPER), 1.0)))
        .child(para("Iris installs it, then quits and reopens. About a minute.", caption(), hue::I10, w))
}

// MARK: - The letter from GitHub (compact, settled)

/// The scene's moment: 3 s after the click, the answer 1.4 s in (`frozenAt: start + 3`).
const A: f64 = 1.4;
/// `Date(timeIntervalSince1970: 3).timeIntervalSinceReferenceDate`, for the wings' slow beat.
const NOW_REF: f64 = 3.0 - 978_307_200.0;

fn seg(t: f64, start: f64, length: f64) -> f64 {
    if length <= 0.0 { if t >= start { 1.0 } else { 0.0 } } else { ((t - start) / length).clamp(0.0, 1.0) }
}
fn ease_out(x: f64) -> f64 {
    1.0 - (1.0 - x).powi(3)
}
fn ease_in_out(x: f64) -> f64 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}

const SIZE: (f32, f32) = (116.0, 76.0);
const ROOT: (f64, f64) = (94.0, 76.0);
const BUD: f64 = 46.0;
const LETTER_AT: (f32, f32) = (36.0, 46.0);
const LETTER: (f32, f32) = (58.0, 42.0);
const BACK: [(f64, f64); 4] = [(166.0, 54.0), (114.0, 60.0), (70.0, 26.0), (38.0, 14.0)];
const SKY: (f32, f32, f32, f32) = (48.0, 8.0, 90.0, 72.0);

fn along(pts: &[(f64, f64)], k: f64) -> (f64, f64) {
    let n = pts.len() - 1;
    let x = k.clamp(0.0, 1.0) * n as f64;
    let i = (x as usize).min(n - 1);
    let u = x - i as f64;
    let (p0, p1, p2, p3) = (pts[i.saturating_sub(1)], pts[i], pts[i + 1], pts[(i + 2).min(n)]);
    let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (-a + c) * u + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u + (-a + 3.0 * b - 3.0 * c + d) * u * u * u);
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
}

/// The whole scene, printed once.
fn scene(outcome: &Outcome, scale: f32) -> Img {
    let plate = Plate { w: SIZE.0, h: SIZE.1, scale };
    riso::cached(format!("letter-scene-{outcome:?}"), &plate, || {
        // Came back empty-handed, or still looking: the bud stays shut under a grey sky.
        let shut = matches!(outcome, Outcome::Failed | Outcome::Looking);
        let open = if shut { 0.0 } else { ease_out(seg(A, 1.15, 0.45)) };
        let sway = (seg(A, 1.15, 0.45) * std::f64::consts::PI).sin() * 2.0;
        let clears = if shut { 0.0 } else { ease_in_out(seg(A, 1.15 - 0.35, 0.9)) };
        let sun_up = if shut { 0.0 } else { ease_out(seg(A, 1.15, 0.8)) };
        let drift = (NOW_REF * 0.7).sin() * 3.0;
        let resting_wings = 0.45 + 0.2 * (NOW_REF * 2.4).sin().max(0.0);
        let perch = (ROOT.0 + 4.0, ROOT.1 - BUD - open * 4.0);
        let mut sh = Sheet::new(plate, 0.0, 0.0, 0.0);
        let id = Transform::identity();
        let (sw, shh) = (SKY.2, SKY.3);
        let sky_rgba = sky(clears, sun_up, drift, scale);
        let (spw, sph) = Plate { w: sw, h: shh, scale }.px();
        sh.picture_at(&sky_rgba, spw, sph, sw, shh, id.pre_translate(SKY.0, SKY.1), 1.0);

        // The way back, a faint dotted trail fading.
        let dots_opacity = 0.3 * (1.0 - seg(A, 1.2, 0.5));
        for i in 0..14 {
            let k = (i as f64 + 0.5) / 14.0;
            let q = along(&BACK, k);
            sh.fill_at(&crate::icons::circle(q.0 as f32, q.1 as f32, 1.0), id, hue::I12, dots_opacity as f32);
        }

        // The flower: the bud opening into bloom, swaying from its root.
        let bud = BUD as f32;
        let plant = id.pre_translate((ROOT.0 - BUD / 2.0) as f32, (ROOT.1 - BUD) as f32).pre_concat(Transform::from_rotate_at(sway as f32, bud / 2.0, bud));
        let closed = garden::flower_raw(false, scale);
        sh.picture_at(&closed.1, closed.0, closed.0, bud, bud, plant, (1.0 - open) as f32);
        let opened = garden::flower_raw(true, scale);
        let pop = (0.85 + 0.15 * (open + (open * std::f64::consts::PI).sin() * 0.12)) as f32;
        let popped = plant.pre_translate(bud / 2.0, bud).pre_scale(pop, pop).pre_translate(-bud / 2.0, -bud);
        sh.picture_at(&opened.1, opened.0, opened.0, bud, bud, popped, open as f32);

        // The letter, leaning.
        let (lw, lh) = LETTER;
        let lean = id.pre_translate(LETTER_AT.0 - lw / 2.0, LETTER_AT.1 - lh / 2.0).pre_concat(Transform::from_rotate_at(-3.0, lw / 2.0, lh / 2.0));
        letter(&mut sh, lean, outcome, scale);

        let b = 20.0f32;
        let wings = garden::butterfly_raw();
        let at = id.pre_translate(perch.0 as f32 - b / 2.0 + b * (1.0 - resting_wings as f32) / 2.0, perch.1 as f32 - b / 2.0);
        sh.sprite_at(&wings.1, wings.0, wings.0, b * resting_wings as f32, b, at, 1.0);
        sh.rgba()
    })
}

/// The patch of sky: the plate, the sun coming up, the cloud drifting off, thinning into the
/// paper from round the flower (an elliptical fade, no edge).
fn sky(clears: f64, sun_up: f64, drift: f64, scale: f32) -> Vec<u8> {
    let (w, h) = (SKY.2, SKY.3);
    let plate = Plate { w, h, scale };
    let (pw, ph) = plate.px();
    let dots = riso::screen(w as usize, h as usize, (120, 162, 220), 23, 1.0, 1.0);
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        for x in 0..pw {
            let c = dots[((y as f32 / scale) as usize).min(h as usize - 1) * w as usize + ((x as f32 / scale) as usize).min(w as usize - 1)];
            let o = ((y * pw + x) * 4) as usize;
            out[o..o + 4].copy_from_slice(&[c.0, c.1, c.2, 255]);
        }
    }
    let sun = (w.min(h) * 0.26) as f64;
    stamp(&mut out, pw, ph, scale, &garden::sun_rgba(), 44, (w as f64 * 0.42, h as f64 * 0.66 - sun_up * h as f64 * 0.32), sun, if sun_up > 0.0 { 1.0 } else { 0.0 });
    let cloud = (w.min(h) * 0.5) as f64;
    stamp(&mut out, pw, ph, scale, &garden::mist_cloud_rgba(), 44, (w as f64 * 0.47 + drift + clears * w as f64 * 0.35, h as f64 * 0.32), cloud, 1.0 - clears);
    // Fade out from (0.54, 0.5), reaching nothing at the frame's edge.
    let stops = [(0.0, 1.0), (0.3, 0.8), (0.62, 0.4), (0.85, 0.12), (1.0, 0.0)];
    for y in 0..ph {
        for x in 0..pw {
            let (px, py) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let d = (((px - 0.54 * w) / (0.5 * w)).powi(2) + ((py - 0.5 * h) / (0.5 * h)).powi(2)).sqrt();
            let mut a = 0.0f32;
            for k in 0..stops.len() - 1 {
                let (t0, a0) = stops[k];
                let (t1, a1) = stops[k + 1];
                if d >= t0 && d <= t1 {
                    a = a0 + (a1 - a0) * (d - t0) / (t1 - t0);
                }
            }
            let o = ((y * pw + x) * 4 + 3) as usize;
            out[o] = (out[o] as f32 * a).round() as u8;
        }
    }
    out
}

/// A sprite of `px` pixels laid at `centre` (points), `size` points square, without smoothing.
#[allow(clippy::too_many_arguments)]
fn stamp(out: &mut [u8], pw: u32, ph: u32, scale: f32, rgba: &[u8], px: u32, centre: (f64, f64), size: f64, opacity: f64) {
    if opacity <= 0.0 {
        return;
    }
    let x0 = ((centre.0 - size / 2.0) * scale as f64) as i64;
    let y0 = ((centre.1 - size / 2.0) * scale as f64) as i64;
    let n = (size * scale as f64) as i64;
    for dy in 0..n {
        for dx in 0..n {
            let (x, y) = (x0 + dx, y0 + dy);
            if x < 0 || y < 0 || x >= pw as i64 || y >= ph as i64 { continue }
            let sx = (dx * px as i64 / n) as usize;
            let sy = (dy * px as i64 / n) as usize;
            let s = &rgba[(sy * px as usize + sx) * 4..][..4];
            let a = s[3] as f64 / 255.0 * opacity;
            if a <= 0.0 { continue }
            let o = ((y as u32 * pw + x as u32) * 4) as usize;
            for k in 0..3 {
                out[o + k] = (s[k] as f64 * a + out[o + k] as f64 * (1.0 - a)).round() as u8;
            }
        }
    }
}

/// The opened letter (`LetterView`, compact): two halves of paper, a few pen lines on the top
/// one where writing would be, and the tick or the count on the bottom; drawn through `at`.
fn letter(sh: &mut Sheet, at: Transform, outcome: &Outcome, scale: f32) {
    let half = LETTER.1 / 2.0;
    letter_bottom(sh, at.pre_translate(0.0, half), outcome, scale, true);
    letter_top(sh, at.pre_translate(0.0, 0.0), scale);
}

/// A half of the letter's paper, `seed`'s, with its crema shadow printed off register.
fn half_sheet(sh: &mut Sheet, at: Transform, seed: i64, scale: f32) {
    let (w, h) = LETTER;
    let half = h / 2.0;
    let t = at.pre_translate(0.0, 0.0);
    let plate = Plate { w, h: half, scale };
    let mask = plate.fill(&pen::hand_rect(seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap());
    let crema = riso::print(&plate, ink::CREMA, seed, &mask, 1.0);
    let (pw, ph) = plate.px();
    sh.picture_at(&crema, pw, ph, w, half, t.pre_translate(1.5, 1.5), 1.0);
    sh.fill_at(&pen::hand_rect(seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap(), t, ink::PAPER, 1.0);
}

fn half_outline(sh: &mut Sheet, at: Transform, seed: i64) {
    let (w, h) = LETTER;
    sh.stroke_at(&pen::hand_rect(seed + 9, 0.35, 2.5, false, 0.0, 0.0, w, h / 2.0).skia().unwrap(), at.pre_translate(0.0, 0.0), 1.1, hue::I12);
}

/// The bottom half, drawn through `bottom` (its own top left): the answer, the tick when
/// `with_tick` (the live scene draws it in itself).
fn letter_bottom(sh: &mut Sheet, bottom: Transform, outcome: &Outcome, scale: f32, with_tick: bool) {
    let (w, h) = LETTER;
    let half = h / 2.0;
    half_sheet(sh, bottom, 61, scale);
    match outcome {
        Outcome::Changes(n) => {
            let s = n.to_string();
            let (lead, bw) = brush_box(&s, 17.0);
            let lh = line_box(17.0, BRUSH_LINE);
            if let Some(p) = raster::text_path(CAVEAT, 17.0, &s, (w - bw) / 2.0 + lead, (half - lh) / 2.0 + 17.0 * BRUSH_ASCENT + (lh - 17.0 * 1.26) / 2.0, 0.0) {
                sh.fill_at(&p, bottom, ink::PINK_TEXT, 1.0);
            }
        }
        Outcome::UpToDate if with_tick => {
            sh.stroke_at(&pen::hand_tick(18.0, 13.0).skia().unwrap(), bottom.pre_translate((w - 18.0) / 2.0, (half - 13.0) / 2.0), 2.4, ink::PINK_TEXT);
        }
        _ => {}
    }
    half_outline(sh, bottom, 61);
}

/// The top half, drawn through `at`: lines where the writing would be.
fn letter_top(sh: &mut Sheet, at: Transform, scale: f32) {
    let w = LETTER.0;
    half_sheet(sh, at, 62, scale);
    let i8: Rgb8 = (0xA3, 0x95, 0x8B);
    sh.stroke_at(&pen::hand_line(70, 0.5, 0.0, 0.5, w * 0.6).skia().unwrap(), at.pre_translate(8.0, 7.0), 1.0, i8);
    sh.stroke_at(&pen::hand_line(71, 0.5, 0.0, 0.5, w * 0.42).skia().unwrap(), at.pre_translate(8.0, 13.0), 1.0, i8);
    half_outline(sh, at, 62);
}

// MARK: - The letter from GitHub, live (`UpdateCheckScene`), for the slip and Settings

/// Where things go in the scene (`UpdateCheckScene.Stage`), in points.
pub struct Stage {
    pub size: (f32, f32),
    /// The bud's root, on the ground, and its size.
    pub root: (f64, f64),
    pub bud: f64,
    /// The letter where it lands (centre), how big it opens, how small it is while carried.
    pub letter_at: (f64, f64),
    pub letter: (f32, f32),
    pub carried: f64,
    /// How far past the frame's right edge the butterfly goes.
    pub reach: f32,
    /// Out to GitHub, then back with the answer; the last point is where the letter drops.
    pub out: &'static [(f64, f64)],
    pub back: &'static [(f64, f64)],
    pub out_dots: usize,
    pub back_dots: usize,
    /// The printed sky round the flower.
    pub sky: (f32, f32, f32, f32),
    pub butterfly: f32,
    pub wide: bool,
    /// Whether the sky may spread past the stage's right edge (Settings) or stops at it (the
    /// slip, whose paper ends there).
    pub sky_spills: bool,
}

pub const COMPACT: Stage = Stage {
    size: SIZE, root: ROOT, bud: BUD, letter_at: (36.0, 46.0), letter: LETTER, carried: 0.5, reach: 50.0,
    out: &[(98.0, 30.0), (70.0, 10.0), (38.0, 6.0), (52.0, -8.0), (104.0, -8.0), (166.0, 4.0)],
    back: &BACK, out_dots: 22, back_dots: 14, sky: SKY, butterfly: 20.0, wide: false, sky_spills: false,
};

pub const WIDE: Stage = Stage {
    size: (222.0, 100.0), root: (188.0, 100.0), bud: 50.0, letter_at: (78.0, 54.0), letter: (156.0, 80.0), carried: 0.3, reach: 230.0,
    out: &[(194.0, 50.0), (150.0, 24.0), (96.0, 10.0), (122.0, -10.0), (206.0, -6.0), (290.0, -16.0), (444.0, -2.0)],
    back: &[(444.0, 70.0), (340.0, 86.0), (250.0, 66.0), (146.0, 20.0), (80.0, 8.0)],
    out_dots: 46, back_dots: 30, sky: (116.0, 6.0, 134.0, 100.0), butterfly: 22.0, wide: true, sky_spills: true,
};

/// The looking loop, and how long the ending plays after the answer.
pub const LOOP: f64 = 1.4;
pub const ENDING: f64 = 1.7;
/// Every beat of the ending is over by then.
const SETTLED: f64 = 2.0;

/// Where the check stands, as the scene plays it: seconds since the click and since the
/// answer (if it has come), and what came back.
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    pub t: f64,
    pub a: Option<f64>,
    pub failed: bool,
    pub reduce: bool,
}

impl Moment {
    /// The moment `phase` is at now (none when the scene stands still: idle, updating).
    pub fn of(phase: &crate::updater::Phase, reduce: bool) -> Option<Moment> {
        use crate::updater::{Phase, Status};
        let slow = crate::parts::slow() as f64;
        match phase {
            Phase::Looking(since) => Some(Moment { t: since.elapsed().as_secs_f64() / slow, a: None, failed: false, reduce }),
            Phase::Answered(status, at) => {
                let a = at.elapsed().as_secs_f64() / slow;
                // The answer waits for at least one loop, so the click was more than a second ago.
                Some(Moment { t: 10.0 + a, a: Some(a), failed: !matches!(status, Status::UpToDate | Status::Behind(..) | Status::NewRelease(..)), reduce })
            }
            _ => None,
        }
    }

    /// Landed and at rest: only the wings' slow beat is left (the sun is the last up, 1.95 s
    /// after the answer).
    pub fn settled(&self) -> bool {
        self.a.is_some_and(|a| a > SETTLED) || (self.reduce && self.a.is_some_and(|a| a > 0.3))
    }
}

/// The parts each layout draws its own way.
pub struct Art<'a> {
    /// A name for what the letter says, for keeping its halves.
    pub key: String,
    /// The sky patch for (clears, sun up, drift): straight RGBA at the plate's scale.
    pub sky: &'a dyn Fn(f64, f64, f64) -> Vec<u8>,
    /// The letter's bottom half without its tick, and its top half, each drawn through a
    /// transform from the half's own points (`letter.0` × `letter.1 / 2`).
    pub bottom: &'a dyn Fn(&mut Sheet, Transform),
    pub top: &'a dyn Fn(&mut Sheet, Transform),
    /// The tick in the bottom half (x, y, w, h), when the letter has one.
    pub tick: Option<(f32, f32, f32, f32)>,
    /// Back empty-handed: the red ring (drawn 0 → 1) round "?" (its opacity), through a transform.
    pub ring: &'a dyn Fn(&mut Sheet, Transform, f64, f64),
}

fn ease_in(x: f64) -> f64 {
    x * x * x
}

/// `Ease.spring`: a damped spring from 0 to 1 over normalised time `x`.
fn ease_spring(x: f64, bounce: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let zeta = 1.0 - bounce;
    let w = 9.0;
    let wd = w * (1.0 - zeta * zeta).sqrt();
    let v = 1.0 - (-zeta * w * x).exp() * ((wd * x).cos() + zeta * w / wd * (wd * x).sin());
    v + (1.0 - (1.0 - (-zeta * w).exp() * (wd.cos() + zeta * w / wd * wd.sin()))) * x
}

fn lerp(a: f64, b: f64, k: f64) -> f64 {
    a + (b - a) * k
}

/// The butterfly's sprite faces up: turn it to face along its path (radians).
fn facing(from: (f64, f64), to: (f64, f64)) -> f64 {
    (to.1 - from.1).atan2(to.0 - from.0) + std::f64::consts::FRAC_PI_2
}

/// Swift's reference clock now, for the wings' beat.
fn now_ref() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64()) - 978_307_200.0
}

/// The picture's room round the stage: the flight goes up past its top and out past its right.
const ROOM: (f32, f32) = (10.0, 30.0);

/// A half of the letter, printed once and kept: `draw` through a transform from the half's own
/// points, with 4 points of room round it.
fn half_picture(key: String, w: f32, h: f32, scale: f32, draw: &dyn Fn(&mut Sheet, Transform)) -> std::rc::Rc<(u32, u32, Vec<u8>)> {
    thread_local! { static HALVES: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<(u32, u32, Vec<u8>)>>> = Default::default(); }
    let key = format!("{key}@{scale}");
    if let Some(h) = HALVES.with(|m| m.borrow().get(&key).cloned()) {
        return h;
    }
    let plate = Plate { w: w + 8.0, h: h + 8.0, scale };
    let (pw, ph) = plate.px();
    let mut sh = Sheet::new(plate, 0.0, 0.0, 0.0);
    draw(&mut sh, Transform::from_translate(4.0, 4.0));
    let out = std::rc::Rc::new((pw, ph, sh.rgba()));
    HALVES.with(|m| m.borrow_mut().insert(key, out.clone()));
    out
}

/// The tick drawn in `k` of the way (`HandTick().trim(to:)`), as a polyline along its curves.
fn tick_drawn(w: f32, h: f32, k: f64) -> Option<tiny_skia::Path> {
    if k >= 1.0 {
        return pen::hand_tick(w, h).skia();
    }
    let q = |p0: (f32, f32), c: (f32, f32), p1: (f32, f32), u: f32| {
        let v = 1.0 - u;
        (v * v * p0.0 + 2.0 * v * u * c.0 + u * u * p1.0, v * v * p0.1 + 2.0 * v * u * c.1 + u * u * p1.1)
    };
    let mut pts = vec![];
    let (a, b, c) = ((0.0, h * 0.55), (w * 0.36, h), (w, 0.0));
    for i in 0..=20 {
        pts.push(q(a, (w * 0.2, h * 0.75), b, i as f32 / 20.0));
    }
    for i in 1..=20 {
        pts.push(q(b, (w * 0.55, h * 0.35), c, i as f32 / 20.0));
    }
    let lens: Vec<f32> = pts.windows(2).map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1)).collect();
    let total: f32 = lens.iter().sum();
    let mut left = total * k as f32;
    if left <= 0.0 {
        return None;
    }
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for (i, l) in lens.iter().enumerate() {
        if left >= *l {
            pb.line_to(pts[i + 1].0, pts[i + 1].1);
            left -= l;
        } else {
            let u = left / l;
            pb.line_to(pts[i].0 + (pts[i + 1].0 - pts[i].0) * u, pts[i].1 + (pts[i + 1].1 - pts[i].1) * u);
            break;
        }
    }
    pb.finish()
}

/// The seal on the back of the letter's top half: a printed pink sticker with a ✻ holding the
/// fold shut, turned −4°, in the middle of the half (`w` × `h` points).
fn seal_into(sh: &mut Sheet, at_: Transform, w: f32, h: f32, wide: bool, scale: f32) {
    let (sw, shh) = if wide { (22.0, 17.0) } else { (13.0, 10.0) };
    let turn = at_.pre_translate((w - sw) / 2.0, (h - shh) / 2.0).pre_concat(Transform::from_rotate_at(-4.0, sw / 2.0, shh / 2.0));
    let plate = Plate { w: sw, h: shh, scale };
    let mask = plate.fill(&pen::hand_rect(65, 0.5, 0.0, true, 0.0, 0.0, sw, shh).skia().unwrap());
    let pink = riso::print(&plate, ink::PINK, 65, &mask, 1.0);
    let (pw, ph) = plate.px();
    sh.picture_at(&pink, pw, ph, sw, shh, turn, 1.0);
    // ✻: three strokes crossing.
    let r = if wide { 3.6 } else { 2.6 };
    let (cx, cy) = (sw / 2.0, shh / 2.0);
    for k in 0..3 {
        let a = std::f32::consts::PI * k as f32 / 3.0 + 0.3;
        let (dx, dy) = (a.cos() * r, a.sin() * r);
        sh.stroke_at(&crate::icons::line(cx - dx, cy - dy, cx + dx, cy + dy), turn, if wide { 1.4 } else { 1.0 }, ink::PAPER);
    }
}

/// The scene at moment `m`: a picture `ROOM` bigger round the stage (the flight goes past
/// it), and whether the butterfly is left out (settled: it beats its wings on its own).
pub fn scene_live(st: &Stage, m: &Moment, art: &Art, scale: f32) -> (Img, bool) {
    let settled = m.settled();
    let plate = Plate { w: st.size.0 + ROOM.0 * 2.0 + st.reach, h: st.size.1 + ROOM.1 + ROOM.0, scale };
    if settled {
        // Under Reduce Motion the butterfly sits still on the bud, part of the picture; else
        // it beats its wings on its own (`resting_butterfly`).
        let key = format!("letter-live-{}-{}-{}-{}", st.wide, m.failed, m.reduce, art.key);
        // Drawn at a moment well after every beat, whichever frame first gets here.
        let rest = Moment { t: 30.0, a: Some(20.0), ..*m };
        let img = riso::cached(key, &plate, || scene_rgba(st, &rest, art, &plate, m.reduce));
        return (img, true);
    }
    let (pw, ph) = plate.px();
    (riso::frame_image(pw, ph, scene_rgba(st, m, art, &plate, true)), false)
}

/// The resting wings over a settled scene: (picture, where in the stage, wide, tall).
pub fn resting_butterfly(st: &Stage, failed: bool, scale: f32) -> (Img, f32, f32, f32, f32) {
    let b = st.butterfly;
    let raw = garden::butterfly_raw();
    let img = riso::cached_crisp("letter-butterfly".into(), raw.0, || raw.1.clone(), b, scale);
    let wings = (0.45 + 0.2 * (now_ref() * 2.4).sin().max(0.0)) as f32;
    // On the bloom, lifted 4 as it opened; on the shut bud when the answer was a failure.
    let perch = (st.root.0 as f32 + 4.0, st.root.1 as f32 - st.bud as f32 - if failed { 0.0 } else { 4.0 });
    (img, perch.0 - b * wings / 2.0, perch.1 - b / 2.0, b * wings, b)
}

fn scene_rgba(st: &Stage, m: &Moment, art: &Art, plate: &Plate, with_butterfly: bool) -> Vec<u8> {
    let scale = plate.scale;
    let mut sh = Sheet::new(Plate { w: plate.w, h: plate.h, scale }, 0.0, 0.0, 0.0);
    sh.turn = Transform::from_translate(ROOM.0, ROOM.1);
    let id = Transform::identity();
    let (t, a, failed, reduce) = (m.t, m.a, m.failed, m.reduce);
    let still = reduce;
    let p = (t / LOOP).rem_euclid(1.0);
    let now = now_ref();

    // The bud grows up on the click, then opens on a good answer.
    let grow = if still || a.is_some() && t > 1.0 { 1.0 } else { ease_spring(seg(t, 0.0, 0.5), 0.2) };
    let open = a.map_or(0.0, |a| if failed { 0.0 } else if reduce { (a / 0.3).clamp(0.0, 1.0) } else { ease_out(seg(a, 1.15, 0.45)) });
    let sway = if still { 0.0 } else { a.map_or((p * 2.0 * std::f64::consts::PI).sin() * 3.0, |a| (seg(a, 1.15, 0.45) * std::f64::consts::PI).sin() * 2.0) };

    // The butterfly.
    let flapping = 0.3 + 0.7 * (now * 2.0 * std::f64::consts::PI * 7.0).sin().abs();
    let resting_wings = 0.45 + 0.2 * (now * 2.4).sin().max(0.0);
    let perch = (st.root.0 + 4.0, st.root.1 - st.bud - open * 4.0);
    let (mut fly, mut heading, mut flap, mut shown) = (perch, 0.0, resting_wings, 1.0);
    if !still {
        if let Some(a) = a {
            if a < 0.65 {
                let k = ease_out(seg(a, 0.0, 0.65));
                fly = along(st.back, k);
                heading = facing(fly, along(st.back, k + 0.01)) * (1.0 - k * 0.6);
                flap = flapping;
                shown = seg(k, 0.0, 0.08);
            } else {
                let k = ease_in_out(seg(a, 0.65, 0.5));
                let from = st.back[st.back.len() - 1];
                fly = (lerp(from.0, perch.0, k), lerp(from.1, perch.1, k) - (k * std::f64::consts::PI).sin() * 14.0);
                heading = if k < 1.0 { 20.0 * (1.0 - k) } else { 0.0 };
                flap = if k < 1.0 { flapping } else { resting_wings };
            }
        } else if t > 0.25 {
            let k = ease_in_out(seg(t, 0.25, 0.9));
            fly = along(st.out, k);
            heading = facing(fly, along(st.out, k + 0.01));
            flap = flapping;
            shown = 1.0 - seg(k, 0.86, 0.14);
        }
    }

    // The sky rises with the bud; a good answer sends the cloud off and brings the sun up.
    let good = a.is_some() && !failed;
    let clears = a.map_or(0.0, |a| if good { if reduce { 1.0 } else { ease_in_out(seg(a, 1.15 - 0.35, 0.9)) } } else { 0.0 });
    let sun_up = a.map_or(0.0, |a| if good { if reduce { 1.0 } else { ease_out(seg(a, 1.15, 0.8)) } } else { 0.0 });
    let drift = if still { 0.0 } else { (now * 0.7).sin() * 3.0 };
    let (sx, sy, sw, shh) = st.sky;
    let sky_rgba = (art.sky)(clears, sun_up, drift);
    let (spw, sph) = Plate { w: sw, h: shh, scale }.px();
    let rise = grow as f32;
    let sky_at = id.pre_translate(sx + sw / 2.0, sy + shh).pre_scale(rise, rise).pre_translate(-sw / 2.0, -shh);
    if st.sky_spills {
        sh.picture_at(&sky_rgba, spw, sph, sw, shh, sky_at, 1.0);
    } else {
        // Cut at the stage's right edge, where the slip's paper ends.
        let mut l = Sheet::new(Plate { w: plate.w, h: plate.h, scale }, 0.0, 0.0, 0.0);
        l.turn = sh.turn;
        l.picture_at(&sky_rgba, spw, sph, sw, shh, sky_at, 1.0);
        let edge = ((ROOM.0 + st.size.0) * scale).round() as usize;
        let pw = l.pm.width() as usize;
        for row in l.pm.data_mut().chunks_exact_mut(pw * 4) {
            row[edge.min(pw) * 4..].fill(0);
        }
        let (lw, lh) = plate.px();
        let rgba = l.rgba();
        sh.turn = Transform::identity();
        sh.picture_at(&rgba, lw, lh, plate.w, plate.h, id, 1.0);
        sh.turn = Transform::from_translate(ROOM.0, ROOM.1);
    }

    // The dotted path: drawn behind it on the way out, pulsing while it's away, gone on the answer.
    let out_drawn = if still { 0.0 } else if a.is_none() { ease_in_out(seg(t, 0.25, 0.9)) } else { 1.0 };
    let out_fade = a.map_or(1.0, |a| 1.0 - seg(a, 0.0, 0.5));
    let pulsing = a.is_none() && t > 0.25 + 0.9;
    let back_drawn = if still { 0.0 } else { a.map_or(0.0, |a| ease_out(seg(a, 0.0, 0.65)) * (1.0 - seg(a, ENDING, 0.0))) };
    for i in 0..st.out_dots {
        let k = (i as f64 + 0.5) / st.out_dots as f64;
        if k >= out_drawn {
            break;
        }
        let wave = if pulsing { 0.25 + 0.75 * ((1.0 + ((k - p) * 4.0 * std::f64::consts::PI).sin()) / 2.0).powi(2) } else { 0.75 };
        let q = along(st.out, k);
        sh.fill_at(&crate::icons::circle(q.0 as f32, q.1 as f32, 1.2), id, hue::I12, (wave * out_fade * 0.75) as f32);
    }
    for i in 0..st.back_dots {
        let k = (i as f64 + 0.5) / st.back_dots as f64;
        if k >= back_drawn {
            break;
        }
        let q = along(st.back, k);
        let o = 0.3 * a.map_or(1.0, |a| 1.0 - seg(a, 1.2, 0.5));
        sh.fill_at(&crate::icons::circle(q.0 as f32, q.1 as f32, 1.0), id, hue::I12, o as f32);
    }

    // The flower: a bud swaying from the root, opening when the answer is good.
    let bud = st.bud as f32;
    let plant = id.pre_translate((st.root.0 - st.bud / 2.0) as f32, (st.root.1 - st.bud) as f32)
        .pre_concat(Transform::from_rotate_at(sway as f32, bud / 2.0, bud))
        .pre_translate(bud / 2.0, bud).pre_scale(grow as f32, grow as f32).pre_translate(-bud / 2.0, -bud);
    if open < 1.0 {
        let closed = garden::flower_raw(false, scale);
        sh.picture_at(&closed.1, closed.0, closed.0, bud, bud, plant, (1.0 - open) as f32);
    }
    if open > 0.0 {
        let opened = garden::flower_raw(true, scale);
        let overshoot = if open >= 1.0 { 1.0 } else { open + (open * std::f64::consts::PI).sin() * 0.12 };
        let pop = (0.85 + 0.15 * overshoot) as f32;
        let popped = plant.pre_translate(bud / 2.0, bud).pre_scale(pop, pop).pre_translate(-bud / 2.0, -bud);
        sh.picture_at(&opened.1, opened.0, opened.0, bud, bud, popped, open as f32);
    }

    // Back empty-handed: the red ring round "?".
    if failed && let Some(a) = a {
        let drawn = if reduce { 1.0 } else { ease_out(seg(a, 0.9, 0.4)) };
        let q = if reduce { 1.0 } else { seg(a, 0.9 + 0.25, 0.2) };
        (art.ring)(&mut sh, id, drawn, q);
    }

    // The letter: hanging under the butterfly, folded; dropped by the heading; opened.
    if let (Some(a), false) = (a, failed) {
        let (w, h) = st.letter;
        let half = h / 2.0;
        let landed = st.back[st.back.len() - 1];
        let (centre, k_scale, tilt) = if reduce {
            (st.letter_at, 1.0, -3.0)
        } else if a < 0.65 {
            ((fly.0, fly.1 + 15.0), st.carried, (a * 11.0).sin() * 6.0)
        } else {
            let k = seg(a, 0.65, 0.3);
            ((lerp(landed.0, st.letter_at.0, k), lerp(landed.1 + 15.0, st.letter_at.1, ease_in(k))), lerp(st.carried, 1.0, ease_out(k)),
             lerp(10.0, -3.0, ease_spring(seg(a, 0.65, 0.6), 0.35)))
        };
        let unfold = if reduce { 1.0 } else if a < 0.95 { 0.0 } else { ease_spring(seg(a, 0.95, 0.52), 0.22) };
        let fold = 180.0 * (1.0 - unfold);
        let lift = (1.0 - unfold.min(1.0)) * -(h as f64) / 4.0 * k_scale;
        let fade = if reduce { (a / 0.3).clamp(0.0, 1.0) } else { 1.0 };
        let tick = if reduce { 1.0 } else { ease_out(seg(a, 0.95 + 0.3, 0.3)) };
        let at_ = id.pre_translate(centre.0 as f32, (centre.1 + lift) as f32).pre_rotate(tilt as f32)
            .pre_scale(k_scale as f32, k_scale as f32).pre_translate(-w / 2.0, -h / 2.0);
        // Drawn on its own when it fades in (Reduce Motion), else straight on.
        let mut layer = (fade < 1.0).then(|| {
            let mut l = Sheet::new(Plate { w: plate.w, h: plate.h, scale }, 0.0, 0.0, 0.0);
            l.turn = sh.turn;
            l
        });
        let target = layer.as_mut().unwrap_or(&mut sh);
        let bottom = half_picture(format!("{}-bottom-{}", art.key, st.wide), w, half, scale, art.bottom);
        target.picture_at(&bottom.2, bottom.0, bottom.1, w + 8.0, half + 8.0, at_.pre_translate(-4.0, half - 4.0), 1.0);
        if let Some((tx, ty, tw, th)) = art.tick
            && let Some(path) = tick_drawn(tw, th, tick)
        {
            target.stroke_at(&path, at_.pre_translate(tx, half + ty), 2.4, ink::PINK_TEXT);
        }
        // The top half swings on the crease (its bottom edge): its front while open past
        // halfway, the seal on its back while folded down.
        let c = (fold.to_radians().cos()) as f32;
        let swing = at_.pre_translate(0.0, half).pre_scale(1.0, c).pre_translate(0.0, -half);
        if fold < 90.0 {
            let top = half_picture(format!("{}-top-{}", art.key, st.wide), w, half, scale, art.top);
            target.picture_at(&top.2, top.0, top.1, w + 8.0, half + 8.0, swing.pre_translate(-4.0, -4.0), 1.0);
        } else {
            let wide = st.wide;
            let back = half_picture(format!("seal-{}", st.wide), w, half, scale, &|sh: &mut Sheet, tf: Transform| {
                let sheet_seed = 63;
                let plate = Plate { w, h: half, scale };
                let mask = plate.fill(&pen::hand_rect(sheet_seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap());
                let crema = riso::print(&plate, ink::CREMA, sheet_seed, &mask, 1.0);
                let (pw, ph) = plate.px();
                sh.picture_at(&crema, pw, ph, w, half, tf.pre_translate(1.5, 1.5), 1.0);
                sh.fill_at(&pen::hand_rect(sheet_seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap(), tf, ink::PAPER, 1.0);
                // Upside down, so it reads the right way up once folded over.
                seal_into(sh, tf.pre_translate(0.0, half).pre_scale(1.0, -1.0), w, half, wide, scale);
                sh.stroke_at(&pen::hand_rect(sheet_seed + 9, 0.35, 2.5, false, 0.0, 0.0, w, half).skia().unwrap(), tf, 1.1, hue::I12);
            });
            target.picture_at(&back.2, back.0, back.1, w + 8.0, half + 8.0, swing.pre_translate(-4.0, -4.0), 1.0);
        }
        if let Some(l) = layer {
            let (pw, ph) = plate.px();
            let rgba = l.rgba();
            sh.turn = Transform::identity();
            sh.picture_at(&rgba, pw, ph, plate.w, plate.h, id, fade as f32);
            sh.turn = Transform::from_translate(ROOM.0, ROOM.1);
        }
    }

    if with_butterfly && shown > 0.0 {
        let b = st.butterfly;
        let wings = garden::butterfly_raw();
        let grow_b = if still || a.is_some() { 1.0 } else if t < 0.25 { grow as f32 } else { 1.0 };
        let at_ = id.pre_translate(fly.0 as f32, fly.1 as f32).pre_rotate(heading.to_degrees() as f32)
            .pre_translate(0.0, b / 2.0).pre_scale(grow_b, grow_b).pre_translate(0.0, -b / 2.0)
            .pre_scale(flap as f32, 1.0).pre_translate(-b / 2.0, -b / 2.0);
        sh.sprite_at(&wings.1, wings.0, wings.0, b, b, at_, shown as f32);
    }
    sh.rgba()
}

/// The live scene as an element the size of the stage: the picture spills out round it and is
/// drawn over what's next to it (the flight crosses the button), the resting wings over that.
/// Asks for the next frame while anything moves.
pub fn scene_element(st: &Stage, m: &Moment, art: &Art, live: &crate::interact::Live, scale: f32) -> Div {
    let (img, settled) = scene_live(st, m, art, scale);
    let plate_w = st.size.0 + ROOM.0 * 2.0 + st.reach;
    let plate_h = st.size.1 + ROOM.1 + ROOM.0;
    let mut el = sized(st.size.0, st.size.1).child(gpui::deferred(at(printed(plate_w, plate_h, &img), -ROOM.0, -ROOM.1)).with_priority(1));
    if settled && !m.reduce {
        let (wings, x, y, w, h) = resting_butterfly(st, m.failed, scale);
        el = el.child(gpui::deferred(at(printed(w, h, &wings), x, y)).with_priority(2));
    }
    // Settled, the wings still beat (Swift draws them thirty times a second); under Reduce
    // Motion nothing moves once the letter is in and the words and button have changed
    // (1.25 s after the answer).
    if !(settled && m.reduce && m.a.is_some_and(|a| a > 1.3)) {
        live.anim.borrow_mut().moving = true;
    }
    el
}

thread_local! {
    /// The slip's update check, as its live window last saw it (`set_phase`).
    static PHASE: std::cell::RefCell<Option<crate::updater::Phase>> = const { std::cell::RefCell::new(None) };
}

/// The live slip, before it draws: where the update check stands, for its page's scene.
pub fn set_phase(phase: &crate::updater::Phase) {
    PHASE.with(|p| *p.borrow_mut() = Some(phase.clone()));
}

/// The compact letter's art: its sky, its halves (pen lines; the tick or the count).
fn compact_scene(outcome: &Outcome, live: &crate::interact::Live, m: &Moment, scale: f32) -> Div {
    let outcome = outcome.clone();
    let key = format!("compact-{outcome:?}");
    let sky_ = move |clears: f64, sun_up: f64, drift: f64| sky(clears, sun_up, drift, scale);
    let o1 = outcome.clone();
    let bottom = move |sh: &mut Sheet, at_: Transform| letter_bottom(sh, at_, &o1, scale, false);
    let top = move |sh: &mut Sheet, at_: Transform| letter_top(sh, at_, scale);
    let (w, half) = (LETTER.0, LETTER.1 / 2.0);
    let tick = matches!(outcome, Outcome::UpToDate).then_some(((w - 18.0) / 2.0, (half - 13.0) / 2.0, 18.0, 13.0));
    let ring = move |sh: &mut Sheet, at_: Transform, drawn: f64, q: f64| crate::pages::updates::ring_drawn(sh, at_.pre_translate(COMPACT.letter_at.0 as f32 + 6.0 - 14.0, COMPACT.letter_at.1 as f32 - 4.0 - 14.0), 28.0, drawn, q, scale);
    let art = Art { key, sky: &sky_, bottom: &bottom, top: &top, tick, ring: &ring };
    scene_element(&COMPACT, m, &art, live, scale)
}
