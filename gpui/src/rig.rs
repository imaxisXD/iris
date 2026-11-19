//! The iris as a rig of parts (`IrisRig.swift`), drawn with tiny-skia so it lands on the same
//! pixels as the Swift app's: a heavy ink line that boils a little, the colour printed off
//! register in a coarse riso screen of dots, each part posed from the time since each beat.
//!
//! Ported line for line: `RigClock`, `Curve`, `IrisPose`, `IrisRig.draw` and its parts, the dot
//! screens and the pen with its boil, plus `RisoArt.flower` (the still). Geometry stays in f64
//! (Swift's CGFloat) until it reaches tiny-skia, because the pen's boil seeds itself from a
//! path's bounds truncated to a third of a point, so the arithmetic runs in the same order and
//! every RNG call comes in the same sequence as Swift's.
//!
//! SwiftUI's GraphicsContext maps onto a `Ctx`: an f64 transform and an optional clip mask over
//! one shared pixmap. Tiled-image fills (the dot screens, the grain in the flat pink) are
//! composited by hand: each tile smoothed on its own (bilinear, clamped at its edges) as
//! CoreGraphics draws them, and `Grain.tile`'s colour above its alpha kept, as CoreGraphics
//! keeps it (it lightens the pink a little where the grain is clear).

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, Pixmap, Stroke, StrokeDash, Transform};

use crate::riso::Rng;

// MARK: - The clock

/// Where the flower is in its life, as times since each beat (`None`: hasn't happened).
#[derive(Clone, Copy, Debug)]
pub struct RigClock {
    pub now: f64,
    pub arrived: Option<f64>,
    pub bloomed: Option<f64>,
    pub wilted: Option<f64>,
    /// The full show (drop, splash, growth, the petal's flight) or the quick one.
    pub full: bool,
    /// Whether it went because the fix was put in (a stamp with a tick) or was dismissed.
    pub replaced: bool,
}

impl RigClock {
    pub fn new(now: f64) -> RigClock {
        RigClock { now, arrived: None, bloomed: None, wilted: None, full: true, replaced: true }
    }
    fn since(&self, t: Option<f64>) -> Option<f64> {
        t.map(|t| self.now - t)
    }
}

/// `RisoArt.Crop`: the whole plant on its ground, the head alone, or the head centred for a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crop {
    Whole,
    Bloom,
    Icon,
}

// MARK: - Curves

/// `Curve`: the rig's motion curves.
pub mod curve {
    use std::f64::consts::PI;

    pub fn clamp(x: f64) -> f64 {
        1f64.min(0f64.max(x))
    }
    pub fn ease_out(x: f64) -> f64 {
        let c = clamp(x);
        1.0 - (1.0 - c).powf(3.0)
    }
    pub fn ease_in(x: f64) -> f64 {
        let c = clamp(x);
        c * c * c
    }
    pub fn ease_in_out(x: f64) -> f64 {
        let c = clamp(x);
        if c < 0.5 { 4.0 * c * c * c } else { 1.0 - (-2.0 * c + 2.0).powf(3.0) / 2.0 }
    }
    /// 0 → 1 over about `duration`, overshooting by `bounce` (0 none … 1 lots).
    pub fn spring(t: f64, duration: f64, bounce: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let x = t / duration;
        let damping = 5.5 * (1.0 - bounce * 0.7);
        1.0 - (-x * damping).exp() * (x * PI * (1.4 + bounce * 1.2)).cos()
    }
    /// A bump 0 → 1 → 0 over `duration`, peaking a third of the way.
    pub fn bump(t: f64, duration: f64) -> f64 {
        if !(t > 0.0 && t < duration) {
            return 0.0;
        }
        let x = t / duration;
        if x < 0.33 { ease_out(x / 0.33) } else { 1.0 - ease_in((x - 0.33) / 0.67) }
    }
    /// A steady pseudo-random number in -1…1 for a seed (UInt64 wrapping maths, as Swift).
    pub fn noise(seed: i64) -> f64 {
        let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 32;
        (x % 20001) as f64 / 10000.0 - 1.0
    }
}

use curve::{bump, clamp, ease_in, ease_in_out, ease_out, noise, spring};

// MARK: - Constants

/// The drawing area, in points at size 92; the flower's foot stands at `BASE`.
pub const CANVAS: (f64, f64) = (180.0, 200.0);
pub const BASE: (f64, f64) = (90.0, 166.0);
/// The stem's height: the head sits this far above the base.
pub const STEM: f64 = 70.0;
/// How far the open bloom reaches left and right of the base, and up from it.
pub const REACH: (f64, f64, f64) = (37.0, 42.0, 100.0);
/// Where the flicked fall lands as a stamp.
const STAMP_AT: (f64, f64) = (REACH.1 + 22.0, -19.0);

type Rgb = (u8, u8, u8);

const INK: Rgb = (38, 24, 22);
const PINK: Rgb = (233, 107, 192);
const PAPER: Rgb = (252, 248, 240);

/// The flower's inks (`IrisRig.Inks`).
mod inks {
    use super::Rgb;
    use crate::riso::ink;
    pub const BLUSH: Rgb = (250, 196, 218);
    pub const PINK: Rgb = ink::PINK;
    pub const HOT: Rgb = (233, 107, 192);
    pub const MAGENTA: Rgb = (208, 64, 150);
    pub const RASPBERRY: Rgb = ink::PINK_TEXT;
    pub const LILAC: Rgb = (200, 160, 226);
    pub const TAN: Rgb = (196, 157, 122);
    pub const ESPRESSO: Rgb = ink::ESPRESSO;
}

/// A tone printed in dots: light at its tip, deeper toward its base, along a ramp of inks.
#[derive(Clone, Copy)]
struct Tone {
    tip: f64,
    base: f64,
    ramp: &'static [Rgb],
}

const SOFT: &[Rgb] = &[inks::BLUSH, inks::PINK, inks::HOT, inks::MAGENTA];
const DEEP: &[Rgb] = &[inks::PINK, inks::HOT, inks::MAGENTA, inks::RASPBERRY];
const OUTER: &[Rgb] = &[inks::LILAC, inks::BLUSH, inks::PINK, inks::HOT];
const HEART_INKS: &[Rgb] = &[inks::MAGENTA, inks::RASPBERRY];
const BUD_INKS: &[Rgb] = &[inks::HOT, inks::MAGENTA, inks::RASPBERRY];
/// How big a dot is, in points.
const DOT_SCALE: f64 = 1.6;
const SOIL: Tone = Tone { tip: 0.85, base: 1.0, ramp: &[inks::TAN, inks::TAN, inks::ESPRESSO] };

/// A fall behind the head that opens with the petal it follows.
struct Extra {
    follow: usize,
    start: f64,
    open: f64,
    length: f64,
    width: f64,
    tone: Tone,
}

const EXTRAS: [Extra; 2] = [
    Extra { follow: 3, start: -84.0, open: -96.0, length: 31.0, width: 10.0, tone: Tone { tip: 0.45, base: 0.85, ramp: SOFT } },
    Extra { follow: 4, start: 84.0, open: 96.0, length: 31.0, width: 10.0, tone: Tone { tip: 0.45, base: 0.85, ramp: SOFT } },
];

/// A 4×4 Bayer screen (`RisoArt.bayer`).
const BAYER: [[f64; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

// MARK: - Paths in f64

#[derive(Clone, Copy, Debug)]
enum Seg {
    M(f64, f64),
    L(f64, f64),
    Q(f64, f64, f64, f64),
    C(f64, f64, f64, f64, f64, f64),
    Z,
}

/// A SwiftUI `Path`, in f64 points.
#[derive(Clone, Debug, Default)]
struct P(Vec<Seg>);

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn inset(self, d: f64) -> Rect {
        Rect { x: self.x + d, y: self.y + d, w: self.w - 2.0 * d, h: self.h - 2.0 * d }
    }
}

impl P {
    fn new() -> P {
        P(Vec::new())
    }
    fn m(&mut self, x: f64, y: f64) {
        self.0.push(Seg::M(x, y));
    }
    fn l(&mut self, x: f64, y: f64) {
        self.0.push(Seg::L(x, y));
    }
    fn q(&mut self, x: f64, y: f64, cx: f64, cy: f64) {
        self.0.push(Seg::Q(cx, cy, x, y));
    }
    fn c(&mut self, x: f64, y: f64, c1x: f64, c1y: f64, c2x: f64, c2y: f64) {
        self.0.push(Seg::C(c1x, c1y, c2x, c2y, x, y));
    }
    fn z(&mut self) {
        self.0.push(Seg::Z);
    }
    fn current(&self) -> Option<(f64, f64)> {
        let mut start = None;
        let mut cur = None;
        for s in &self.0 {
            match *s {
                Seg::M(x, y) => {
                    start = Some((x, y));
                    cur = start;
                }
                Seg::L(x, y) | Seg::Q(_, _, x, y) | Seg::C(_, _, _, _, x, y) => cur = Some((x, y)),
                Seg::Z => cur = start,
            }
        }
        cur
    }
    fn rect(r: Rect) -> P {
        let mut p = P::new();
        p.m(r.x, r.y);
        p.l(r.x + r.w, r.y);
        p.l(r.x + r.w, r.y + r.h);
        p.l(r.x, r.y + r.h);
        p.z();
        p
    }
    fn ellipse(r: Rect) -> P {
        let mut p = P::new();
        p.add_ellipse(r);
        p
    }
    /// CoreGraphics' ellipse: four cubics from the right-hand point, clockwise on screen.
    fn add_ellipse(&mut self, r: Rect) {
        let k = 0.552_284_749_830_793_6;
        let (cx, cy, rx, ry) = (r.x + r.w / 2.0, r.y + r.h / 2.0, r.w / 2.0, r.h / 2.0);
        self.m(cx + rx, cy);
        self.c(cx, cy + ry, cx + rx, cy + ry * k, cx + rx * k, cy + ry);
        self.c(cx - rx, cy, cx - rx * k, cy + ry, cx - rx, cy + ry * k);
        self.c(cx, cy - ry, cx - rx, cy - ry * k, cx - rx * k, cy - ry);
        self.c(cx + rx, cy, cx + rx * k, cy - ry, cx + rx, cy - ry * k);
        self.z();
    }
    /// `addArc(center:radius:startAngle:endAngle:clockwise: false)`: angles increasing (clockwise
    /// on a flipped screen), a line to the arc's start first.
    fn add_arc(&mut self, cx: f64, cy: f64, r: f64, start: f64, end: f64) {
        let mut sweep = end - start;
        while sweep < 0.0 {
            sweep += 2.0 * PI;
        }
        let (sx, sy) = (cx + r * start.cos(), cy + r * start.sin());
        if self.current().is_some() { self.l(sx, sy) } else { self.m(sx, sy) }
        let n = (sweep / (PI / 2.0)).ceil().max(1.0) as usize;
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut a = start;
        for _ in 0..n {
            let b = a + step;
            let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
            self.c(cx + r * cb, cy + r * sb, cx + r * (ca - k * sa), cy + r * (sa + k * ca), cx + r * (cb + k * sb), cy + r * (sb - k * cb));
            a = b;
        }
    }
    fn map(&self, f: impl Fn(f64, f64) -> (f64, f64)) -> P {
        P(self
            .0
            .iter()
            .map(|s| match *s {
                Seg::M(x, y) => {
                    let p = f(x, y);
                    Seg::M(p.0, p.1)
                }
                Seg::L(x, y) => {
                    let p = f(x, y);
                    Seg::L(p.0, p.1)
                }
                Seg::Q(a, b, x, y) => {
                    let (c, p) = (f(a, b), f(x, y));
                    Seg::Q(c.0, c.1, p.0, p.1)
                }
                Seg::C(a, b, c, d, x, y) => {
                    let (c1, c2, p) = (f(a, b), f(c, d), f(x, y));
                    Seg::C(c1.0, c1.1, c2.0, c2.1, p.0, p.1)
                }
                Seg::Z => Seg::Z,
            })
            .collect())
    }
    fn offset(&self, dx: f64, dy: f64) -> P {
        self.map(|x, y| (x + dx, y + dy))
    }
    fn scaled(&self, s: f64) -> P {
        self.map(|x, y| (x * s, y * s))
    }
    /// `Path.boundingRect`: the tight bounds of the curve, control points left out.
    fn bounds(&self) -> Rect {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut add = |x: f64, y: f64| {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        };
        #[allow(clippy::too_many_arguments)]
        fn cubic(add: &mut impl FnMut(f64, f64), cur: (f64, f64), ax: f64, ay: f64, bx: f64, by: f64, x: f64, y: f64) {
            add(x, y);
            let at = |t: f64, p0: f64, p1: f64, p2: f64, p3: f64| {
                let u = 1.0 - t;
                u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
            };
            for (p0, p1, p2, p3) in [(cur.0, ax, bx, x), (cur.1, ay, by, y)] {
                // B'(t)/3 = a t² + b t + c
                let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
                let b = 2.0 * (p0 - 2.0 * p1 + p2);
                let c = p1 - p0;
                let mut roots = Vec::new();
                if a.abs() < 1e-12 {
                    if b.abs() > 1e-12 {
                        roots.push(-c / b);
                    }
                } else {
                    let disc = b * b - 4.0 * a * c;
                    if disc >= 0.0 {
                        let s = disc.sqrt();
                        roots.push((-b + s) / (2.0 * a));
                        roots.push((-b - s) / (2.0 * a));
                    }
                }
                for t in roots {
                    if t > 0.0 && t < 1.0 {
                        add(at(t, cur.0, ax, bx, x), at(t, cur.1, ay, by, y));
                    }
                }
            }
        }
        let mut cur = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        for s in &self.0 {
            match *s {
                Seg::M(x, y) => {
                    add(x, y);
                    cur = (x, y);
                    start = cur;
                }
                Seg::L(x, y) => {
                    add(x, y);
                    cur = (x, y);
                }
                Seg::Q(qx, qy, x, y) => {
                    // Raised to a cubic first. Measured, not known: the bud's sheath has its
                    // edge at exactly -25 (a third of a point) worked straight from the quad,
                    // -24.999… this way, and Swift's frames match the boil this way gives.
                    let (ax, ay) = (cur.0 + 2.0 / 3.0 * (qx - cur.0), cur.1 + 2.0 / 3.0 * (qy - cur.1));
                    let (bx, by) = (x + 2.0 / 3.0 * (qx - x), y + 2.0 / 3.0 * (qy - y));
                    cubic(&mut add, cur, ax, ay, bx, by, x, y);
                    cur = (x, y);
                }
                Seg::C(ax, ay, bx, by, x, y) => {
                    cubic(&mut add, cur, ax, ay, bx, by, x, y);
                    cur = (x, y);
                }
                Seg::Z => cur = start,
            }
        }
        if !x0.is_finite() {
            return Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };
        }
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
    fn skia(&self) -> Option<tiny_skia::Path> {
        let mut b = tiny_skia::PathBuilder::new();
        for s in &self.0 {
            match *s {
                Seg::M(x, y) => b.move_to(x as f32, y as f32),
                Seg::L(x, y) => b.line_to(x as f32, y as f32),
                Seg::Q(cx, cy, x, y) => b.quad_to(cx as f32, cy as f32, x as f32, y as f32),
                Seg::C(a, c, d, e, x, y) => b.cubic_to(a as f32, c as f32, d as f32, e as f32, x as f32, y as f32),
                Seg::Z => b.close(),
            }
        }
        b.finish()
    }
}

// MARK: - The context

/// An affine transform in f64: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Clone, Copy, Debug)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };
    fn translate(self, tx: f64, ty: f64) -> Affine {
        Affine { e: self.a * tx + self.c * ty + self.e, f: self.b * tx + self.d * ty + self.f, ..self }
    }
    fn scale(self, sx: f64, sy: f64) -> Affine {
        Affine { a: self.a * sx, b: self.b * sx, c: self.c * sy, d: self.d * sy, ..self }
    }
    fn rotate_deg(self, deg: f64) -> Affine {
        let r = deg * PI / 180.0;
        let (s, c) = (r.sin(), r.cos());
        Affine {
            a: self.a * c + self.c * s,
            b: self.b * c + self.d * s,
            c: -self.a * s + self.c * c,
            d: -self.b * s + self.d * c,
            ..self
        }
    }
    fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }
    fn skia(&self) -> Transform {
        Transform::from_row(self.a as f32, self.b as f32, self.c as f32, self.d as f32, self.e as f32, self.f as f32)
    }
    fn invert(&self) -> Option<Affine> {
        let det = self.det();
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) })
    }
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
}

/// A `GraphicsContext`: where it draws (its transform) and what it's cut to.
#[derive(Clone)]
struct Ctx {
    ts: Affine,
    clip: Option<Rc<Mask>>,
}

impl Ctx {
    fn translate(&self, x: f64, y: f64) -> Ctx {
        Ctx { ts: self.ts.translate(x, y), clip: self.clip.clone() }
    }
    fn rotate(&self, deg: f64) -> Ctx {
        Ctx { ts: self.ts.rotate_deg(deg), clip: self.clip.clone() }
    }
    fn scale(&self, x: f64, y: f64) -> Ctx {
        Ctx { ts: self.ts.scale(x, y), clip: self.clip.clone() }
    }
    fn ok(&self) -> bool {
        let d = self.ts.det();
        d.is_finite() && d.abs() > 1e-9
    }
}

/// A tile for `.tiledImage`: premultiplied RGBA as floats (a value may sit above its alpha, as
/// `Grain.tile` stores it), each pixel `pitch` points square in the context's space.
struct Tile {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
    pitch: f64,
}

/// How tiles are sampled: 0 nearest, 1 bilinear wrapping across tiles, 2 bilinear with each
/// tile clamped at its edges. 2 is what CoreGraphics does with `.tiledImage` (measured against
/// `Iris --rig-frames`: mean residual 0.316 against 0.330 and 0.426, and no specks along the
/// seams); the others stay for measuring.
pub static TILE_FILTER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

impl Tile {
    fn at(&self, x: i64, y: i64) -> [f32; 4] {
        let tx = x.rem_euclid(self.w as i64) as usize;
        let ty = y.rem_euclid(self.h as i64) as usize;
        self.px[ty * self.w + tx]
    }
    /// The tile at (`u`, `v`) in its own pixels.
    fn sample(&self, u: f64, v: f64) -> [f32; 4] {
        if TILE_FILTER.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            return self.at(u.floor() as i64, v.floor() as i64);
        }
        let (w, h) = (self.w as f64, self.h as f64);
        if TILE_FILTER.load(std::sync::atomic::Ordering::Relaxed) == 2 {
            // Each tile smoothed on its own, clamped at its edges: the seams don't blend.
            let (lu, lv) = (u - (u / w).floor() * w - 0.5, v - (v / h).floor() * h - 0.5);
            let (lu, lv) = (lu.clamp(0.0, w - 1.0), lv.clamp(0.0, h - 1.0));
            let (x0, y0) = (lu.floor(), lv.floor());
            let (fx, fy) = ((lu - x0) as f32, (lv - y0) as f32);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let (x1, y1) = ((x0 + 1).min(self.w as i64 - 1), (y0 + 1).min(self.h as i64 - 1));
            let (a, b, c, d) = (self.at(x0, y0), self.at(x1, y0), self.at(x0, y1), self.at(x1, y1));
            let mut out = [0f32; 4];
            for i in 0..4 {
                let top = a[i] + (b[i] - a[i]) * fx;
                let bottom = c[i] + (d[i] - c[i]) * fx;
                out[i] = top + (bottom - top) * fy;
            }
            return out;
        }
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (self.at(x0, y0), self.at(x0 + 1, y0), self.at(x0, y0 + 1), self.at(x0 + 1, y0 + 1));
        let mut out = [0f32; 4];
        for i in 0..4 {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            out[i] = top + (bottom - top) * fy;
        }
        out
    }
}

struct Canvas {
    pm: Pixmap,
}

fn color(c: Rgb, alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(tiny_skia::Color::from_rgba(c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, alpha).unwrap());
    p.anti_alias = true;
    p
}

impl Canvas {
    fn mask(&self, ctx: &Ctx, path: &tiny_skia::Path) -> Mask {
        let mut m = Mask::new(self.pm.width(), self.pm.height()).unwrap();
        m.fill_path(path, FillRule::Winding, true, ctx.ts.skia());
        if let Some(c) = &ctx.clip {
            for (x, y) in m.data_mut().iter_mut().zip(c.data()) {
                *x = ((*x as u32 * *y as u32 + 127) / 255) as u8;
            }
        }
        m
    }

    /// `clip(to:)`: the clip so far, cut to `path` as it lands under the current transform.
    fn clip(&self, ctx: &Ctx, path: &P) -> Ctx {
        let m = match path.skia() {
            Some(p) if ctx.ok() => self.mask(ctx, &p),
            _ => Mask::new(self.pm.width(), self.pm.height()).unwrap(),
        };
        Ctx { ts: ctx.ts, clip: Some(Rc::new(m)) }
    }

    fn fill(&mut self, ctx: &Ctx, path: &P, c: Rgb) {
        if !ctx.ok() {
            return;
        }
        if let Some(p) = path.skia() {
            self.pm.fill_path(&p, &color(c, 1.0), FillRule::Winding, ctx.ts.skia(), ctx.clip.as_deref());
        }
    }

    fn stroke_skia(&mut self, ctx: &Ctx, path: &P, c: Rgb, alpha: f32, stroke: &Stroke) {
        if !ctx.ok() {
            return;
        }
        let Some(p) = path.skia() else { return };
        if alpha >= 1.0 {
            self.pm.stroke_path(&p, &color(c, alpha), stroke, ctx.ts.skia(), ctx.clip.as_deref());
            return;
        }
        // Translucent: blended in float, as CoreGraphics does (tiny-skia's 8-bit pipeline puts
        // ink over ink a level off).
        let res = ctx.ts.det().abs().sqrt() as f32;
        let Some(outline) = p.stroke(stroke, res.max(0.01)) else { return };
        let m = self.mask(ctx, &outline);
        let data = self.pm.data_mut();
        let src = [c.0 as f32, c.1 as f32, c.2 as f32, 255.0];
        for (px, cov) in data.chunks_exact_mut(4).zip(m.data()) {
            if *cov == 0 {
                continue;
            }
            let k = *cov as f32 / 255.0 * alpha;
            for i in 0..4 {
                px[i] = (src[i] * k + px[i] as f32 * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    fn stroke_round(&mut self, ctx: &Ctx, path: &P, c: Rgb, alpha: f32, width: f64) {
        let s = Stroke { width: width as f32, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        self.stroke_skia(ctx, path, c, alpha, &s);
    }

    /// `fill(path, with: .tiledImage(tile))` at `opacity`, the tile's origin at the context's
    /// origin, sampled at each pixel's centre (no smoothing), composited source-over in
    /// premultiplied bytes as CoreGraphics does.
    fn fill_tiled(&mut self, ctx: &Ctx, path: &tiny_skia::Path, tile: &Tile, opacity: f32) {
        if !ctx.ok() {
            return;
        }
        let Some(inv) = ctx.ts.invert() else { return };
        let m = self.mask(ctx, path);
        let (w, h) = (self.pm.width() as usize, self.pm.height() as usize);
        // Only the rows and columns the path can reach.
        let b = path.bounds();
        let corners = [(b.left(), b.top()), (b.right(), b.top()), (b.left(), b.bottom()), (b.right(), b.bottom())]
            .map(|(x, y)| ctx.ts.apply(x as f64, y as f64));
        let x0 = corners.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
        let x1 = (corners.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max).ceil() + 1.0).min(w as f64).max(0.0) as usize;
        let y0 = corners.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
        let y1 = (corners.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max).ceil() + 1.0).min(h as f64).max(0.0) as usize;
        let md = m.data();
        let data = self.pm.data_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                let cov = md[y * w + x];
                if cov == 0 {
                    continue;
                }
                let (ux, uy) = inv.apply(x as f64 + 0.5, y as f64 + 0.5);
                let s = tile.sample(ux / tile.pitch, uy / tile.pitch);
                let k = cov as f32 / 255.0 * opacity;
                let sa = s[3] / 255.0 * k;
                if s[3] == 0.0 && s[0] == 0.0 && s[1] == 0.0 && s[2] == 0.0 {
                    continue;
                }
                let o = (y * w + x) * 4;
                for i in 0..4 {
                    let d = data[o + i] as f32;
                    let v = s[i] * k + d * (1.0 - sa);
                    data[o + i] = v.round().clamp(0.0, 255.0) as u8;
                }
                // Keep premultiplied colour within its alpha.
                let a = data[o + 3];
                for i in 0..3 {
                    data[o + i] = data[o + i].min(a);
                }
            }
        }
    }
}

// MARK: - Screens and grain

thread_local! {
    static DOT_TILES: RefCell<HashMap<u32, Rc<Tile>>> = RefCell::new(HashMap::new());
    static GRAIN: Rc<Tile> = Rc::new(grain_tile());
    static SALT: Rc<Tile> = Rc::new(salt_tile());
}

/// `IrisRig.dots`: an ink as ordered-dither dots, 0–16 sixteenths of the way to solid.
fn dots(c: Rgb, density: f64) -> Rc<Tile> {
    let level = (density.clamp(0.0, 1.0) * 16.0).round() as u32;
    let key = ((c.0 as u32) << 24) | ((c.1 as u32) << 16) | ((c.2 as u32) << 8) | level;
    DOT_TILES.with(|m| {
        if let Some(t) = m.borrow().get(&key) {
            return t.clone();
        }
        let t = Rc::new(screen(level as f64 / 16.0, c));
        m.borrow_mut().insert(key, t.clone());
        t
    })
}

/// `IrisRig.screen`: a 64-pixel tile, 4 pixels a cell, shown 16 points big and tiled at
/// `dotScale`, so a cell is 1.6 points.
fn screen(density: f64, c: Rgb) -> Tile {
    let (cells, px) = (16usize, 4usize);
    let w = cells * px;
    let mut rng = Rng::new(11);
    let mut out = vec![[0f32; 4]; w * w];
    for cy in 0..cells {
        for cx in 0..cells {
            let t = (BAYER[cy % 4][cx % 4] + 0.5) / 16.0 + (rng.next() - 0.5) * 0.3;
            if !(density > t) {
                continue;
            }
            for y in cy * px..(cy + 1) * px {
                for x in cx * px..(cx + 1) * px {
                    out[y * w + x] = [c.0 as f32, c.1 as f32, c.2 as f32, 255.0];
                }
            }
        }
    }
    Tile { w, h: w, px: out, pitch: cells as f64 * DOT_SCALE / w as f64 }
}

/// `Grain.tile`: 128 pixels shown at 64 points, (26, 20, 16) at a noisy alpha, written as is into
/// a premultiplied bitmap (so the colour can sit above its alpha); `scale: 0.9`.
fn grain_tile() -> Tile {
    let size = 128;
    let mut rng = Rng::new(42);
    let px = (0..size * size)
        .map(|_| {
            let v = rng.next();
            let alpha = ((v - 0.35) * 170.0).clamp(0.0, 255.0) as u8;
            [26.0, 20.0, 16.0, alpha as f32]
        })
        .collect();
    Tile { w: size, h: size, px, pitch: 0.5 * 0.9 }
}

/// `Grain.salt`: premultiplied paper specks, 128 pixels at 64 points; `scale: 0.5`.
fn salt_tile() -> Tile {
    let size = 128;
    let mut rng = Rng::new(17);
    let px = (0..size * size)
        .map(|_| {
            let alpha = ((rng.next() - 0.62) * 2.2).clamp(0.0, 1.0);
            [(252.0 * alpha) as u8 as f32, (249.0 * alpha) as u8 as f32, (246.0 * alpha) as u8 as f32, (255.0 * alpha) as u8 as f32]
        })
        .collect();
    Tile { w: size, h: size, px, pitch: 0.5 * 0.5 }
}

/// The ink `m` (0–1) of the way along a ramp.
fn along(ramp: &[Rgb], m: f64) -> Rgb {
    if ramp.len() < 2 {
        return ramp[0];
    }
    let x = m.clamp(0.0, 1.0) * (ramp.len() - 1) as f64;
    let i = (ramp.len() - 2).min(x as usize);
    let f = x - i as f64;
    let (a, b) = (ramp[i], ramp[i + 1]);
    let mix = |p: u8, q: u8| (p as f64 + (q as f64 - p as f64) * f).round() as u8;
    (mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// Dots landed `reg` off the line, in five bands from the tip to the base, each a step deeper and
/// a step along the ramp, printed over the last.
fn fill_screen(cv: &mut Canvas, ctx: &Ctx, path: &P, reg: (f64, f64), tone: Tone) {
    let moved = path.offset(reg.0, reg.1);
    let g = cv.clip(ctx, &moved);
    let bands = [(1.0, 0.0), (0.84, 0.25), (0.69, 0.5), (0.55, 0.75), (0.42, 1.0)];
    for (shrink, m) in bands {
        let area = if shrink == 1.0 { P::rect(moved.bounds().inset(-4.0)) } else { path.scaled(shrink).offset(reg.0, reg.1) };
        let ink = along(tone.ramp, m);
        let tile = dots(ink, tone.tip + (tone.base - tone.tip) * m);
        if let Some(a) = area.skia() {
            cv.fill_tiled(&g, &a, &tile, 1.0);
        }
    }
}

/// Flat pink landed `reg` off the line, with the print's coarse grain through it.
fn fill_pink(cv: &mut Canvas, ctx: &Ctx, path: &P, reg: (f64, f64)) {
    let moved = path.offset(reg.0, reg.1);
    if let Some(p) = moved.skia() {
        fill_pink_skia(cv, ctx, &p);
    }
}

fn fill_pink_skia(cv: &mut Canvas, ctx: &Ctx, moved: &tiny_skia::Path) {
    if !ctx.ok() {
        return;
    }
    cv.pm.fill_path(moved, &color(PINK, 1.0), FillRule::Winding, ctx.ts.skia(), ctx.clip.as_deref());
    // Clipped to the shape, the grained box round it is the shape itself.
    SALT.with(|t| cv.fill_tiled(ctx, moved, t, 0.18));
    GRAIN.with(|t| cv.fill_tiled(ctx, moved, t, 0.45));
}

// MARK: - The pen

/// The pen: a heavy round ink line that boils (shifts a hair, a few times a second), gone over
/// once more a hair off so it reads as pressed.
#[derive(Clone, Copy)]
struct Pen {
    seed: i64,
}

impl Pen {
    fn jitter(&self, path: &P) -> (f64, f64) {
        let r = path.bounds();
        let key = self
            .seed
            .wrapping_mul(7919)
            .wrapping_add(((r.x * 3.0) as i64).wrapping_mul(131))
            .wrapping_add(((r.y * 3.0) as i64).wrapping_mul(17))
            .wrapping_add((r.w * 3.0) as i64);
        (0.35 * noise(key), 0.35 * noise(key.wrapping_add(1)))
    }

    fn stroke(&self, cv: &mut Canvas, ctx: &Ctx, path: &P, width: f64) {
        let j = self.jitter(path);
        let line = path.offset(j.0, j.1);
        cv.stroke_round(ctx, &line, INK, 1.0, width);
        cv.stroke_round(ctx, &line.offset(0.4, 0.35), INK, 0.35, width * 0.7);
    }

    /// A cubic drawn in short pieces whose width runs from `widths.0` to `widths.1`.
    #[allow(clippy::too_many_arguments)]
    fn taper(&self, cv: &mut Canvas, ctx: &Ctx, p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), widths: (f64, f64), c: Rgb) {
        let at = |t: f64| {
            let u = 1.0 - t;
            (
                u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
                u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1,
            )
        };
        let n = 14;
        let j = 0.3 * noise(self.seed.wrapping_mul(3));
        for i in 0..n {
            let mut s = P::new();
            let a = at(i as f64 / n as f64);
            let b = at((i + 1) as f64 / n as f64);
            s.m(a.0, a.1);
            s.l(b.0, b.1);
            let w = widths.0 + (widths.1 - widths.0) * i as f64 / (n - 1) as f64;
            let st = Stroke { width: w as f32, line_cap: LineCap::Round, ..Stroke::default() };
            cv.stroke_skia(ctx, &s.offset(j, 0.0), c, 1.0, &st);
        }
    }
}

// MARK: - Shapes

/// A fall: a narrow haft flaring into a blade with a ruffled tip, curling by `curl`.
fn fall(length: f64, width: f64, curl: f64) -> P {
    let (l, w, c) = (length, width, curl);
    let mut p = P::new();
    p.m(0.0, 0.0);
    p.c(-0.85 * w + c * 0.6, -0.68 * l, -0.15 * w, -0.25 * l, -0.95 * w + c * 0.3, -0.42 * l);
    p.q(-0.3 * w + c, -0.98 * l, -0.85 * w + c, -0.97 * l);
    p.q(0.3 * w + c, -0.98 * l, c, -1.1 * l);
    p.q(0.85 * w + c * 0.6, -0.68 * l, 0.85 * w + c, -0.97 * l);
    p.c(0.0, 0.0, 0.95 * w + c * 0.3, -0.42 * l, 0.15 * w, -0.25 * l);
    p
}

/// A standard: pinched at the base, a cupped spoon at the top, its tip leaning in by `curl`.
fn standard(length: f64, width: f64, curl: f64) -> P {
    let mut p = P::new();
    p.m(0.0, 0.0);
    p.c(-width * 0.5 + curl, -length * 0.86, -width * 0.15, -length * 0.3, -width * 0.75 + curl * 0.6, -length * 0.6);
    p.q(width * 0.5 + curl, -length * 0.86, curl * 1.3, -length * 1.18);
    p.c(0.0, 0.0, width * 0.75 + curl * 0.6, -length * 0.6, width * 0.15, -length * 0.3);
    p
}

/// An irregular grained blot, like a fleck of spilled ink.
fn blob(c: (f64, f64), r: f64, seed: i64, squash: f64) -> P {
    let n = 7usize;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * 2.0 * PI;
            let rr = r * (1.0 + 0.28 * noise(seed * 31 + i as i64));
            (c.0 + a.cos() * rr, c.1 + a.sin() * rr * squash)
        })
        .collect();
    let mut p = P::new();
    p.m((pts[0].0 + pts[n - 1].0) / 2.0, (pts[0].1 + pts[n - 1].1) / 2.0);
    for i in 0..n {
        let next = pts[(i + 1) % n];
        p.q((pts[i].0 + next.0) / 2.0, (pts[i].1 + next.1) / 2.0, pts[i].0, pts[i].1);
    }
    p.z();
    p
}

/// Paper under the line (so parts in front hide the ones behind), pink off register, then ink.
#[allow(clippy::too_many_arguments)]
fn draw_petal(cv: &mut Canvas, ctx: &Ctx, shape: &P, beard: bool, tone: Tone, reg: (f64, f64), pen: Pen) {
    cv.fill(ctx, shape, PAPER);
    fill_screen(cv, ctx, shape, reg, tone);
    if beard {
        // The fall's pale throat and its beard: an ink line with short hairs down the haft.
        let b = shape.bounds();
        cv.fill(ctx, &P::ellipse(Rect { x: -b.w * 0.13, y: b.y * 0.5, w: b.w * 0.26, h: -b.y * 0.36 }), PAPER);
        for (k, f) in [0.16, 0.22, 0.28, 0.34, 0.4, 0.46].iter().enumerate() {
            let x = if k % 2 == 0 { -0.8 } else { 0.8 };
            cv.fill(ctx, &P::ellipse(Rect { x: x - 1.1, y: b.y * f - 1.1, w: 2.2, h: 2.2 }), INK);
        }
    }
    pen.stroke(cv, ctx, shape, 2.4);
}

// MARK: - The pose

#[derive(Clone, Copy, Debug, Default)]
struct PetalPose {
    angle: f64,
    width: f64,
    length: f64,
}

#[derive(Clone, Copy, Debug)]
struct Speck {
    at: (f64, f64),
    size: f64,
}

#[derive(Clone, Copy, Debug)]
struct Flick {
    at: (f64, f64),
    angle: f64,
    length: f64,
}

#[derive(Clone, Copy, Debug)]
struct Drop {
    y: f64,
    sx: f64,
    sy: f64,
    splat: bool,
}

#[derive(Clone, Debug)]
struct Flying {
    at: (f64, f64),
    spin: f64,
    scale: f64,
    trail: Vec<(f64, f64)>,
}

/// `IrisPose`: every part's state at one moment, worked out from the clock.
#[derive(Clone, Debug)]
struct Pose {
    puddle: f64,
    flecks: Vec<Speck>,
    growth: f64,
    stretch: f64,
    bend: f64,
    curl: f64,
    leaf: f64,
    leaf_turn: f64,
    bud: f64,
    head_shown: bool,
    head_x: f64,
    head_y: f64,
    head_turn: f64,
    lean: f64,
    sink: f64,
    squash_x: f64,
    squash_y: f64,
    register: (f64, f64),
    bud_body: f64,
    bud_split: f64,
    petals: [PetalPose; 8],
    dots: Vec<Speck>,
    flicks: Vec<Flick>,
    drop: Option<Drop>,
    flying: Option<Flying>,
    stamp_scale: f64,
    ring: f64,
    stamp_register: (f64, f64),
    boil: i64,
}

const OPEN_ANGLE: [f64; 8] = [0.0, -70.0, 70.0, -122.0, 122.0, 180.0, -35.0, 35.0];
const START_ANGLE: [f64; 8] = [0.0, -30.0, 30.0, -100.0, 100.0, 180.0, -10.0, 10.0];
const CROWN: [(f64, f64, f64, bool); 8] = [
    (-135.0, 150.0, 5.0, false),
    (-225.0, 110.0, 3.6, true),
    (-75.0, 190.0, 3.0, false),
    (-285.0, 80.0, 2.8, true),
    (120.0, 160.0, 4.2, false),
    (210.0, 115.0, 3.2, true),
    (60.0, 200.0, 2.6, false),
    (280.0, 70.0, 2.4, true),
];

/// `PanelMotion.afterBloom`.
fn after_bloom(full: bool) -> f64 {
    if full { 0.26 } else { 0.06 }
}

impl Pose {
    fn new(c: &RigClock) -> Pose {
        let mut p = Pose {
            puddle: 1.0,
            flecks: Vec::new(),
            growth: 1.0,
            stretch: 1.0,
            bend: 0.0,
            curl: 0.0,
            leaf: 1.0,
            leaf_turn: 0.0,
            bud: 1.0,
            head_shown: true,
            head_x: 1.0,
            head_y: 1.0,
            head_turn: 0.0,
            lean: 0.0,
            sink: 0.0,
            squash_x: 1.0,
            squash_y: 1.0,
            register: (3.0, 2.6),
            bud_body: 1.0,
            bud_split: 0.0,
            petals: [PetalPose::default(); 8],
            dots: Vec::new(),
            flicks: Vec::new(),
            drop: None,
            flying: None,
            stamp_scale: 0.0,
            ring: 0.0,
            stamp_register: (1.4, 1.4),
            boil: 0,
        };
        let a = c.since(c.arrived).unwrap_or(99.0);
        let b = c.since(c.bloomed);
        let w = c.since(c.wilted);
        let full = c.full;
        let t = c.now;
        p.boil = (t * 8.0).floor() as i64;

        // Arrival.
        let land = if full { 0.2 } else { 0.0 };
        if full && a < land + 0.08 {
            if a < land {
                let x = clamp((a - 0.06) / (land - 0.06));
                let f = x * x;
                let wobble = if a < 0.06 { 0.08 * (a / 0.06 * PI * 2.0).sin() } else { 0.0 };
                p.drop = Some(Drop {
                    y: -112.0 * (1.0 - f),
                    sx: 1.25 * (1.0 - wobble) * (1.0 - 0.3 * f),
                    sy: 1.25 * (1.0 + wobble) * (1.0 + 0.6 * f),
                    splat: false,
                });
            } else {
                p.drop = Some(Drop { y: 0.0, sx: 1.0, sy: 1.0, splat: true });
            }
        }
        if full {
            let after = a - land;
            p.flecks = CROWN
                .iter()
                .map(|&(vx, vy, size, stays)| {
                    if !(after > 0.0) {
                        return Speck { at: (0.0, 0.0), size: 0.0 };
                    }
                    let s = after;
                    let y = -vy * s + 900.0 * s * s;
                    if y >= 0.0 && s > 0.05 {
                        let land_t = vy / 900.0;
                        return Speck { at: (vx * land_t, 1.0), size: if stays { size * 0.75 } else { 0.0 } };
                    }
                    Speck { at: (vx * s, y), size }
                })
                .collect();
        }
        p.puddle = if full { spring(a - land - 0.04, 0.24, 0.3) } else { ease_out(a / 0.1) * 0.65 };
        let grow_at = if full { land + 0.07 } else { 0.0 };
        let grow = spring(a - grow_at, if full { 0.26 } else { 0.15 }, 0.4);
        p.growth = 1f64.min(grow);
        p.stretch = 1.0 + 0f64.max(grow - 1.0) * 1.1;
        p.bend = 1.0 - ease_out((a - grow_at - 0.05) / 0.3);
        p.bud = spring(a - grow_at, if full { 0.24 } else { 0.1 }, 0.3);
        p.head_shown = p.bud > 0.02;
        let leaf_at = if full { grow_at + 0.12 } else { 0.04 };
        let u = spring(a - leaf_at, if full { 0.34 } else { 0.14 }, 0.5);
        p.leaf = u;
        p.leaf_turn = (1.0 - u) * 40.0;
        let snap_at = if full { 0.6 } else { 0.1 };
        let r = if a < snap_at { 1.0 } else { 1.0 - spring(a - snap_at, 0.12, 0.5) };
        p.register = (3.0 + 3.6 * r, 2.6 + 3.0 * r);

        // Reading.
        let sway = |t: f64| (t * 2.0 * PI / 1.6).sin() * 5.0;
        let settled = if full { 0.6 } else { 0.16 };
        let busy = 1.0 + 0.4 * ease_in_out((a - settled - 1.2) / 0.6);
        let read_lean = |t: f64| 5.0 + sway(t) * 0.3 * busy;
        let read_curl = |t: f64| (12.0 + sway(t) * busy) * 2.4;
        let read_turn = |t: f64| {
            let beat = t % 0.8;
            let nod = 11.0 * busy * bump(beat - 0.1, 0.24);
            16.0 + (sway(t - 0.1) - sway(t)) * 1.5 + nod
        };
        let reading = if full { ease_out((a - settled) / 0.2) } else { ease_in_out((a - settled - 0.25) / 0.4) };
        if b.is_none() && w.is_none() {
            p.lean = read_lean(t) * reading;
            p.curl = read_curl(t) * reading;
            p.head_turn = read_turn(t) * reading;
            let beat = t % 0.8;
            p.squash_y = 1.0 - 0.05 * bump(beat, 0.12) * reading;
            let jolt = bump(beat - 0.1, 0.24) * reading;
            p.register.0 += 1.6 * jolt;
            p.register.1 += 1.2 * jolt;
            let breath = 1.0 + (t * 2.0 * PI / 0.8).sin() * 0.04 * busy * reading;
            p.head_x = 1.0 / breath;
            p.head_y = breath;
        }

        // Bloom.
        let gather = if full { 0.14 } else { 0.0 };
        let mut open = [0f64; 8];
        if let Some(b) = b {
            let stagger: [f64; 8] = if full {
                [0.16, 0.08, 0.11, 0.02, 0.09, 0.16, 0.12, 0.14]
            } else {
                [0.03, 0.02, 0.025, 0.0, 0.01, 0.02, 0.025, 0.03]
            };
            for i in 0..8 {
                open[i] = if i < 3 || i > 5 {
                    spring(b - gather - stagger[i], if full { 0.3 } else { 0.14 }, if full { 0.45 } else { 0.3 })
                } else {
                    spring(b - gather - stagger[i], if full { 0.32 } else { 0.14 }, if full { 0.45 } else { 0.3 })
                };
            }
            p.bud_split = ease_out((b - gather) / if full { 0.08 } else { 0.05 });
            p.bud_body = 1.0 - ease_out((b - gather) / if full { 0.08 } else { 0.05 });
            let after = after_bloom(full);
            let squash = if full { bump(b, gather * 1.6) } else { 0.0 };
            p.head_y = 1.0 - 0.16 * squash + (if full { 0.15 } else { 0.0 }) * bump(b - gather, 0.12);
            p.head_x = 1.0 + 0.12 * squash;
            let swell = 1.0 + (if full { 0.2 } else { 0.1 }) * bump(b - gather, if full { 0.34 } else { 0.22 })
                - (if full { 0.04 } else { 0.02 }) * bump(b - gather - (if full { 0.3 } else { 0.2 }), 0.24);
            p.head_x *= swell;
            p.head_y *= swell;
            let recoil = bump(b - after - 0.08, 0.26);
            let read_at_bloom = ease_out((a - b - settled) / 0.2);
            let from = c.bloomed.map(|bt| read_lean(bt) * read_at_bloom).unwrap_or(0.0);
            let hand_off = ease_in_out(b / if full { 0.2 } else { 0.1 });
            let idle = ease_out((b - 0.5) / 0.4) * (1.0 - ease_in_out((b - 1.6) / 0.5));
            p.lean = from * (1.0 - hand_off) + (4.0 - (if full { 12.0 } else { 0.0 }) * bump(b, gather * 1.8)) * hand_off - 9.0 * recoil
                + 2.5 * (t * 2.0 * PI / 2.4).sin() * idle;
            p.curl = c.bloomed.map(read_curl).unwrap_or(0.0) * read_at_bloom * (1.0 - hand_off) - 6.0 * recoil;
            p.head_turn = c.bloomed.map(read_turn).unwrap_or(0.0) * read_at_bloom * (1.0 - hand_off) - 10.0 * recoil;
            p.head_y *= 1.0 - 0.06 * recoil;
            for (i, o) in open.iter_mut().enumerate().take(6).skip(3) {
                *o += 0.03 * (t * 2.0 * PI / 1.8 + i as f64).sin() * idle;
            }
            if b > 2.1
                && let Some(bloomed) = c.bloomed
            {
                p.boil = ((bloomed + 2.1) * 8.0).floor() as i64;
            }
            p.stretch *= 1.0 + 0.08 * bump(b - gather, 0.3);
            p.leaf_turn -= 12.0 * bump(b - gather - 0.06, 0.34);
            // The pop.
            let count = if full { 9 } else { 2 };
            let pop_at = gather + if full { 0.1 } else { 0.05 };
            let pp = ease_out((b - pop_at) / 0.2);
            let dry = 1.0 - ease_in((b - pop_at - if full { 0.35 } else { 0.12 }) / if full { 0.2 } else { 0.12 });
            if b > pop_at && dry > 0.0 {
                let sizes = [2.6, 4.2, 2.0, 3.4, 4.8, 2.4, 3.8, 2.2, 3.0];
                p.dots = (0..count)
                    .map(|i| {
                        let angle = PI * (0.62 + 0.8 * i as f64 / 1.max(count - 1) as f64);
                        let reach = 30.0 * (1.0 + 0.6 * noise(i as i64 * 13 + 5) * 0.5 + 0.3) * pp;
                        Speck { at: (angle.cos() * reach, angle.sin() * reach), size: sizes[i] * dry }
                    })
                    .collect();
                if full {
                    p.flicks = (0..3)
                        .map(|i| {
                            let angle = PI * [1.05, 1.32, 0.8][i];
                            let reach = 42.0 * pp;
                            Flick { at: (angle.cos() * reach, angle.sin() * reach), angle, length: 5.0 * dry }
                        })
                        .collect();
                }
            }
        }

        // Done.
        let mut landed = -1.0;
        let mut shut_at = 0.04;
        if let Some(w) = w {
            if full && c.replaced {
                p.lean = 4.0 - 6.0 * bump(w, 0.07) + 8.0 * bump(w - 0.06, 0.3);
                p.curl = 4.0 * bump(w - 0.06, 0.3);
                p.head_turn = -12.0 * bump(w - 0.05, 0.1) + 10.0 * bump(w - 0.12, 0.3);
                open[5] += 0.12 * bump(w, 0.08);
                let (fly_from, fly_for) = (0.06, 0.34);
                let from = (0.0, -STEM + 16.0);
                let to = STAMP_AT;
                let arc = |f: f64| (from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f * f - 360.0 * f * (1.0 - f));
                let f = clamp((w - fly_from) / fly_for);
                let in_flight = w > fly_from && w < fly_from + fly_for;
                if w > fly_from && w < fly_from + fly_for + 0.14 {
                    let tail = if in_flight { 0f64.max(f - 0.5) } else { ease_in((w - fly_from - fly_for) / 0.14) };
                    // `stride(from: tail, through: f, by: 0.035)`.
                    let mut trail = Vec::new();
                    let mut i = 0;
                    loop {
                        let v = tail + i as f64 * 0.035;
                        if v > f {
                            break;
                        }
                        trail.push(arc(v));
                        i += 1;
                    }
                    p.flying = Some(Flying { at: arc(f), spin: f * 360.0, scale: if in_flight { 1.0 - 0.15 * f } else { 0.0 }, trail });
                }
                if w > fly_from {
                    open[5] = 0.0;
                }
                landed = w - fly_from - fly_for;
                shut_at = fly_from + fly_for;
            } else {
                p.lean = 6.0;
                if c.replaced {
                    if w > 0.04 {
                        open[5] = 0.0;
                    }
                    landed = w - 0.04;
                }
            }
            if landed > 0.0 {
                let hold = if full { 0.6 } else { 0.28 };
                p.stamp_scale = (0.55 + 0.45 * spring(landed, 0.18, 0.7)) * (1.0 - ease_in((landed - hold) / 0.14));
                p.ring = ease_out(landed / 0.22);
                let r = 1.0 - spring(landed - 0.05, 0.12, 0.5);
                p.stamp_register = (1.4 + 3.0 * r, 1.4 + 2.5 * r);
            }
            let up = w - shut_at;
            let shut = ease_in_out(up / if full { 0.18 } else { 0.12 });
            for o in open.iter_mut() {
                *o *= 1.0 - shut;
            }
            p.bud_split = 1.0 - shut;
            p.bud_body = shut;
            p.squash_y = 1.0 + 0.07 * bump(up, 0.14);
            p.sink = STEM * 1.7 * ease_in((up - if full { 0.1 } else { 0.06 }) / if full { 0.26 } else { 0.18 });
            if p.sink > STEM * 1.65 {
                p.head_shown = false;
            }
            let dry = 1.0 - ease_in((up - if full { 0.3 } else { 0.2 }) / 0.2);
            p.puddle *= dry;
            for f in p.flecks.iter_mut() {
                f.size *= dry;
            }
        }

        for i in 0..8 {
            let o = open[i];
            if !(o > 0.001) {
                p.petals[i] = PetalPose::default();
                continue;
            }
            p.petals[i].angle = START_ANGLE[i] + (OPEN_ANGLE[i] - START_ANGLE[i]) * o;
            p.petals[i].length = 0.15 + 0.85 * 1.06f64.min(o);
            p.petals[i].width = 0.3 + 0.7 * 1.04f64.min(o);
        }
        p
    }
}

// MARK: - Drawing

fn draw(cv: &mut Canvas, ctx: &Ctx, p: &Pose, k: f64) {
    let ctx = ctx.scale(k, k);
    let b = BASE;
    let pen = Pen { seed: p.boil };

    // The puddle the drop spilled, and the specks it threw.
    if p.puddle > 0.01 {
        let (w, h) = (34.0 * p.puddle, 6.0 * p.puddle);
        let mut puddle = P::ellipse(Rect { x: -w / 2.0, y: -h / 2.0, w, h });
        puddle.add_ellipse(Rect { x: w * 0.38, y: -h * 0.15, w: 5.0 * p.puddle, h: 3.0 * p.puddle });
        let g = ctx.translate(b.0, b.1);
        fill_screen(cv, &g, &puddle, (0.0, 0.0), SOIL);
    }
    for (i, fleck) in p.flecks.iter().enumerate() {
        if !(fleck.size > 0.2) {
            continue;
        }
        let g = ctx.translate(b.0 + fleck.at.0, b.1 + fleck.at.1);
        fill_screen(cv, &g, &blob((0.0, 0.0), fleck.size, i as i64, 0.85), (0.0, 0.0), SOIL);
    }

    // The plant, leaning and ducking about its base.
    let mut plant = ctx.clone();
    if p.sink > 0.0 {
        plant = cv.clip(&plant, &P::rect(Rect { x: 0.0, y: 0.0, w: CANVAS.0, h: b.1 + 1.0 }));
    }
    let plant = plant.translate(b.0, b.1 + p.sink).rotate(p.lean).scale(p.squash_x, p.squash_y);

    // The sword leaf.
    if p.leaf > 0.01 {
        let leaf = plant.translate(-3.0, 0.0).rotate(p.leaf_turn - 10.0).scale(1.0, p.leaf);
        let mut blade = P::new();
        blade.m(0.0, 0.0);
        blade.q(-13.0, -58.0, 1.0, -33.0);
        blade.q(-7.0, 0.0, -13.0, -22.0);
        blade.z();
        cv.fill(&leaf, &blade, PAPER);
        pen.stroke(cv, &leaf, &blade, 2.2);
        let mut vein = P::new();
        vein.m(-3.5, -3.0);
        vein.q(-11.5, -44.0, -4.0, -28.0);
        pen.stroke(cv, &leaf, &vein, 0.9);
    }

    // The stem.
    let top = (p.curl * 0.5, -STEM * p.growth * p.stretch);
    if p.growth > 0.01 {
        let c1 = (-7.0 * p.bend, top.1 * 0.4);
        let c2 = (6.0 * p.bend + p.curl * 0.2, top.1 * 0.75);
        pen.taper(cv, &plant, (0.0, 0.0), c1, c2, top, (5.6, 3.2), INK);
        pen.taper(cv, &plant, (0.0, 0.0), c1, c2, top, (2.4, 1.0), PINK);
    }

    // The head, riding the stem's tip.
    if p.head_shown {
        let head = plant.translate(top.0, top.1).rotate(p.head_turn).scale(p.head_x * p.bud, p.head_y * p.bud);
        draw_head(cv, &head, p, pen);
    }

    // The pop when it opens.
    let crown = (b.0, b.1 - STEM);
    for (i, dot) in p.dots.iter().enumerate() {
        if !(dot.size > 0.2) {
            continue;
        }
        fill_pink(cv, &ctx, &blob((crown.0 + dot.at.0, crown.1 + dot.at.1), dot.size, 40 + i as i64, 0.85), (0.0, 0.0));
    }
    for flick in &p.flicks {
        if !(flick.length > 0.5) {
            continue;
        }
        let mut f = P::new();
        let c = (crown.0 + flick.at.0, crown.1 + flick.at.1);
        let d = (flick.angle.cos() * flick.length, flick.angle.sin() * flick.length);
        f.m(c.0 - d.0, c.1 - d.1);
        f.q(c.0 + d.0, c.1 + d.1, c.0 - d.1 * 0.35, c.1 + d.0 * 0.35);
        pen.stroke(cv, &ctx, &f, 2.0);
    }

    // The ink drop.
    if let Some(drop) = p.drop {
        if drop.splat {
            let g = ctx.translate(b.0, b.1 - 1.0);
            fill_screen(cv, &g, &blob((0.0, 0.0), 13.0, 99, 0.3), (0.0, 0.0), SOIL);
        } else {
            let d = ctx.translate(b.0, b.1 + drop.y).scale(drop.sx, drop.sy);
            let mut tear = P::new();
            tear.m(0.0, -22.0);
            tear.q(8.0, -6.0, 6.0, -15.0);
            tear.add_arc(0.0, -7.0, 8.0, 5f64.to_radians(), 175f64.to_radians());
            tear.q(0.0, -22.0, -6.0, -15.0);
            cv.fill(&d, &tear, PAPER);
            fill_screen(cv, &d, &tear, (2.0, 2.0), SOIL);
            pen.stroke(cv, &d, &tear, 2.0);
        }
    }

    // The fall flicked onto the text box: a dotted pen trail behind it, then the stamp.
    if let Some(fly) = &p.flying {
        if fly.trail.len() > 1 {
            let mut trail = P::new();
            for (i, q) in fly.trail.iter().enumerate() {
                if i == 0 { trail.m(b.0 + q.0, b.1 + q.1) } else { trail.l(b.0 + q.0, b.1 + q.1) }
            }
            let s = Stroke {
                width: 1.5,
                line_cap: LineCap::Round,
                miter_limit: 10.0,
                dash: StrokeDash::new(vec![2.5, 4.0], 0.0),
                ..Stroke::default()
            };
            cv.stroke_skia(&ctx, &trail, INK, 1.0, &s);
        }
        if fly.scale > 0.01 {
            let f = ctx.translate(b.0 + fly.at.0, b.1 + fly.at.1).rotate(fly.spin).scale(fly.scale, fly.scale);
            draw_petal(cv, &f, &fall(26.0, 10.0, 0.0), true, Tone { tip: 0.6, base: 0.95, ramp: DEEP }, p.register, pen);
        }
    }
    if p.stamp_scale > 0.01 {
        let st = ctx.translate(b.0 + STAMP_AT.0, b.1 + STAMP_AT.1).rotate(-12.0).scale(p.stamp_scale, p.stamp_scale);
        if p.ring > 0.0 && p.ring < 1.0 {
            // The burst as it lands: a pink ring thrown out, thinning, with ink spokes.
            let r = 16.0 + 5.0 * p.ring;
            let thin = 1.0 - p.ring;
            let ring = P::ellipse(Rect { x: -r, y: -r * 0.8, w: 2.0 * r, h: 1.6 * r });
            let res = st.ts.det().abs().sqrt() as f32;
            if let Some(rp) = ring.skia()
                && let Some(stroked) = rp.stroke(&Stroke { width: (3.4 * thin) as f32, miter_limit: 10.0, ..Stroke::default() }, res.max(1.0))
            {
                fill_pink_skia(cv, &st, &stroked);
            }
            for k in 0..7 {
                let a = k as f64 / 7.0 * 2.0 * PI + 0.3;
                let mut spoke = P::new();
                spoke.m(a.cos() * (r + 2.0), a.sin() * (r + 2.0) * 0.8);
                spoke.l(a.cos() * (r + 2.0 + 4.0 * thin), a.sin() * (r + 2.0 + 4.0 * thin) * 0.8);
                pen.stroke(cv, &st, &spoke, 1.8);
            }
        }
        let st = st.scale(1.15, 1.15);
        let seal = P::ellipse(Rect { x: -13.0, y: -9.5, w: 26.0, h: 19.0 });
        let mut blot = seal.clone();
        blot.add_ellipse(Rect { x: 12.0, y: 5.0, w: 4.4, h: 3.6 });
        cv.fill(&st, &seal, PAPER);
        fill_pink(cv, &st, &blot, p.stamp_register);
        pen.stroke(cv, &st, &seal, 1.8);
        let mut tick = P::new();
        tick.m(-7.0, 0.0);
        tick.l(-1.5, 5.0);
        tick.q(9.5, -7.0, 2.5, -1.5);
        pen.stroke(cv, &st, &tick, 3.0);
    }
}

fn draw_head(cv: &mut Canvas, ctx: &Ctx, p: &Pose, pen: Pen) {
    // The page's misregistration, undone for this head's turn and stretch, then a part's.
    let plate = |angle: f64, sx: f64, sy: f64, slim: f64| -> (f64, f64) {
        let turn = (p.lean + p.head_turn + angle) * PI / 180.0;
        let sx = 0.15f64.max(p.squash_x * p.head_x * p.bud * sx);
        let sy = 0.15f64.max(p.squash_y * p.head_y * p.bud * sy);
        (
            slim * (p.register.0 * turn.cos() + p.register.1 * turn.sin()) / sx,
            slim * (-p.register.0 * turn.sin() + p.register.1 * turn.cos()) / sy,
        )
    };
    // Extra petals first, behind everything, then the heart.
    for e in &EXTRAS {
        let lead = p.petals[e.follow];
        if !(lead.length > 0.02) {
            continue;
        }
        let o = 1f64.min(0f64.max((lead.length - 0.15) / 0.85));
        let angle = e.start + (e.open - e.start) * o;
        let c = ctx.rotate(angle).scale(lead.width, lead.length);
        draw_petal(cv, &c, &fall(e.length, e.width, 0.0), false, e.tone, plate(angle, lead.width, lead.length, 1.0), pen);
    }
    // A dotted heart behind the petals.
    if p.petals[3].length > 0.02 {
        let r = 10.0 * p.petals[3].length;
        let disc = P::ellipse(Rect { x: -r, y: -r * 0.95, w: 2.0 * r, h: 1.9 * r });
        cv.fill(ctx, &disc, PAPER);
        fill_screen(cv, ctx, &disc, plate(0.0, 1.0, 1.0, 1.0), Tone { tip: 0.85, base: 1.0, ramp: HEART_INKS });
    }
    for i in [1usize, 2, 6, 7, 0, 3, 4, 5] {
        let petal = p.petals[i];
        if !(petal.length > 0.02) {
            continue;
        }
        let c = ctx.rotate(petal.angle).scale(petal.width, petal.length);
        let tone = match i {
            0 | 6 | 7 => Tone { tip: 0.4, base: 0.85, ramp: SOFT },
            1 | 2 => Tone { tip: 0.34, base: 0.75, ramp: OUTER },
            5 => Tone { tip: 0.6, base: 0.95, ramp: DEEP },
            _ => Tone { tip: 0.7, base: 1.0, ramp: DEEP },
        };
        let shape = match i {
            0 => standard(26.0, 11.0, 0.0),
            1 => standard(26.0, 11.0, 4.0 * petal.length),
            2 => standard(26.0, 11.0, -4.0 * petal.length),
            6 => standard(25.0, 9.5, 2.5 * petal.length),
            7 => standard(25.0, 9.5, -2.5 * petal.length),
            5 => fall(26.0, 8.5, 0.0),
            _ => fall(38.0, 8.5, (if i == 3 { 4.0 } else { -4.0 }) * petal.length),
        };
        draw_petal(cv, &c, &shape, (3..=5).contains(&i), tone, plate(petal.angle, petal.width, petal.length, 1.0), pen);
    }
    // The closed bud: one fat pink spindle with a twist round it, in a pointed sheath.
    if p.bud_body > 0.01 {
        let split = p.bud_split;
        let mut spindle = P::new();
        spindle.m(0.0, 2.0);
        spindle.c(0.5, -28.0, -12.5, -6.0, -7.0, -22.0);
        spindle.c(0.0, 2.0, 8.0, -22.0, 12.5, -6.0);
        for side in [-1.0f64, 1.0] {
            let mut half = ctx.rotate(side * 70.0 * split).scale(p.bud_body, p.bud_body);
            if split > 0.001 {
                half = cv.clip(&half, &P::rect(Rect { x: if side < 0.0 { -20.0 } else { 0.0 }, y: -40.0, w: 20.0, h: 50.0 }));
            }
            cv.fill(&half, &spindle, PAPER);
            fill_screen(
                cv,
                &half,
                &spindle,
                plate(side * 34.0 * split, p.bud_body, p.bud_body, 0.5),
                Tone { tip: 0.75, base: 1.0, ramp: BUD_INKS },
            );
            if split < 0.001 {
                let mut wrap = P::new();
                wrap.m(-6.0, -9.0);
                wrap.q(4.5, -22.0, 1.0, -11.0);
                pen.stroke(cv, &half, &wrap, 1.5);
            }
            pen.stroke(cv, &half, &spindle, 2.3);
            if split > 0.001 {
                let mut seam = P::new();
                seam.m(0.0, 2.0);
                seam.l(0.4, -29.0);
                pen.stroke(cv, &half, &seam, 2.0);
            }
        }
        if split < 0.5 {
            let sheath = ctx.scale(1.0 - split * 2.0, 1.0 - split * 2.0);
            for side in [-1.0f64, 1.0] {
                let mut leafy = P::new();
                leafy.m(side * -1.0, 4.0);
                leafy.q(side * 6.0, -13.0, side * 13.0, -1.0);
                leafy.q(side * 1.0, 4.0, side * 4.0, -4.0);
                cv.fill(&sheath, &leafy, PAPER);
                pen.stroke(cv, &sheath, &leafy, 1.6);
            }
        }
    }
    // The throat where the petals meet: a pinch of pink over the crossing lines.
    if p.bud_split > 0.5 {
        fill_pink(cv, ctx, &P::ellipse(Rect { x: -3.2, y: -3.6, w: 6.4, h: 6.0 }), (0.0, 0.0));
    }
}

// MARK: - Entry points

/// Premultiplied pixmap bytes as straight-alpha RGBA.
fn straight(pm: &Pixmap) -> Vec<u8> {
    let mut out = pm.data().to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        }
    }
    out
}

/// The rig posed for `clock` into a `w`×`h` pixel image, the canvas's top-left corner at pixel
/// (`dx`, `dy`), `size` the flower's size (92 is the rig's own) at `scale` pixels per point.
#[allow(clippy::too_many_arguments)]
pub fn draw_rig_at(clock: &RigClock, size: f32, scale: f32, w: u32, h: u32, dx: f64, dy: f64) -> Vec<u8> {
    let mut cv = Canvas { pm: Pixmap::new(w.max(1), h.max(1)).unwrap() };
    let ctx = Ctx { ts: Affine::IDENTITY.translate(dx, dy).scale(scale as f64, scale as f64), clip: None };
    draw(&mut cv, &ctx, &Pose::new(clock), size as f64 / 92.0);
    straight(&cv.pm)
}

/// RGBA (straight alpha, sRGB bytes) of `IrisRig.canvas` (180×200 pt) × size/92, at `scale`
/// pixels per point: the flower posed for `clock` (`FlowerRig`).
pub fn draw_rig(clock: &RigClock, size: f32, scale: f32) -> (u32, u32, Vec<u8>) {
    let k = size as f64 / 92.0;
    let w = (CANVAS.0 * k * scale as f64).round() as u32;
    let h = (CANVAS.1 * k * scale as f64).round() as u32;
    (w, h, draw_rig_at(clock, size, scale, w, h, 0.0, 0.0))
}

/// `RisoArt.flower`: the still flower (the quick show at rest, open or a closed bud), cropped
/// `.whole`, `.bloom` or `.icon`, `points` big at `scale` pixels per point.
pub fn flower(points: f32, crop: Crop, open: bool, scale: f32) -> (u32, u32, Vec<u8>) {
    let clock = RigClock { now: 1010.0, arrived: Some(1000.0), bloomed: if open { Some(1001.0) } else { None }, wilted: None, full: false, replaced: true };
    let (x, y, side) = match crop {
        Crop::Whole => (29.0, 53.0, 122.0),
        Crop::Bloom => (48.0, 50.0, 84.0),
        Crop::Icon => (36.0, 50.0, 108.0),
    };
    let s = scale as f64 * points as f64 / side;
    let px = (side * s).round() as u32;
    let mut cv = Canvas { pm: Pixmap::new(px.max(1), px.max(1)).unwrap() };
    let ctx = Ctx { ts: Affine::IDENTITY.scale(s, s).translate(-x, -y), clip: None };
    draw(&mut cv, &ctx, &Pose::new(&clock), 1.0);
    (px, px, straight(&cv.pm))
}
