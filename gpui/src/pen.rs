//! The pen: every line in Iris, slightly wobbly and seeded so it never re-jitters.
//!
//! Each shape is the Swift shape's `path(in:)` ported as is (`HandRect`, `HandLine`, `HandTick`,
//! `HandArrow`, `HandEllipse`, `TapeLabel.Tape`, `DryBrushStroke`, `BrushSlab.SlabShape`). A
//! `Pen` path can be stroked by Rune as vector strokes, or filled by tiny-skia into a mask
//! for printed ink.

use gpui::{LineCap, LineJoin, PathBuilder, PathStyle, StrokeOptions, point, px};
use std::f32::consts::PI;

use crate::riso::Rng;

#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    M([f32; 2]),
    L([f32; 2]),
    Q([f32; 2], [f32; 2]),
    C([f32; 2], [f32; 2], [f32; 2]),
    Z,
}

/// A path in points, top-left origin.
#[derive(Clone, Debug, Default)]
pub struct Pen(pub Vec<Cmd>);

impl Pen {
    pub fn new() -> Pen {
        Pen(Vec::new())
    }
    pub fn m(&mut self, x: f32, y: f32) {
        self.0.push(Cmd::M([x, y]));
    }
    pub fn l(&mut self, x: f32, y: f32) {
        self.0.push(Cmd::L([x, y]));
    }
    pub fn q(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.push(Cmd::Q([cx, cy], [x, y]));
    }
    pub fn z(&mut self) {
        self.0.push(Cmd::Z);
    }
    pub fn poly(points: &[[f32; 2]]) -> Pen {
        let mut p = Pen::new();
        for (i, q) in points.iter().enumerate() {
            if i == 0 { p.m(q[0], q[1]) } else { p.l(q[0], q[1]) }
        }
        p.z();
        p
    }

    /// `Path.catmullRom`: smooth through every point.
    pub fn catmull_rom(p: &[[f32; 2]], closed: bool) -> Pen {
        let mut path = Pen::new();
        let Some(first) = p.first() else { return path };
        path.m(first[0], first[1]);
        for i in 0..p.len().saturating_sub(1) {
            let a = if i > 0 { p[i - 1] } else { p[i] };
            let b = p[i];
            let c = p[i + 1];
            let e = if i + 2 < p.len() { p[i + 2] } else { c };
            path.0.push(Cmd::C(
                [b[0] + (c[0] - a[0]) / 6.0, b[1] + (c[1] - a[1]) / 6.0],
                [c[0] - (e[0] - b[0]) / 6.0, c[1] - (e[1] - b[1]) / 6.0],
                c,
            ));
        }
        if closed {
            path.z();
        }
        path
    }

    pub fn translated(&self, dx: f32, dy: f32) -> Pen {
        let t = |p: [f32; 2]| [p[0] + dx, p[1] + dy];
        Pen(self.0.iter().map(|c| match *c {
            Cmd::M(p) => Cmd::M(t(p)),
            Cmd::L(p) => Cmd::L(t(p)),
            Cmd::Q(a, p) => Cmd::Q(t(a), t(p)),
            Cmd::C(a, b, p) => Cmd::C(t(a), t(b), t(p)),
            Cmd::Z => Cmd::Z,
        }).collect())
    }

    /// The path as a GPUI path at `origin` (window points): stroked `width` wide with round
    /// caps and joins, like `lineCap: .round, lineJoin: .round`, or filled when `width` is `None`;
    /// everything `k` times its size.
    pub fn gpui(&self, origin: gpui::Point<gpui::Pixels>, width: Option<f32>, k: f32) -> Option<gpui::Path> {
        let mut b = match width {
            Some(w) => PathBuilder::fill().with_style(PathStyle::Stroke(StrokeOptions {
                line_width: w * k,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            })),
            None => PathBuilder::fill(),
        };
        let at = |q: [f32; 2]| point(origin.x + px(q[0] * k), origin.y + px(q[1] * k));
        for c in &self.0 {
            match *c {
                Cmd::M(q) => b.move_to(at(q)),
                Cmd::L(q) => b.line_to(at(q)),
                Cmd::Q(a, q) => b.curve_to(at(q), at(a)),
                Cmd::C(a, d, q) => b.cubic_bezier_to(at(q), at(a), at(d)),
                Cmd::Z => b.close(),
            }
        }
        b.build().ok()
    }

    pub fn skia(&self) -> Option<tiny_skia::Path> {
        let mut b = tiny_skia::PathBuilder::new();
        for c in &self.0 {
            match *c {
                Cmd::M(q) => b.move_to(q[0], q[1]),
                Cmd::L(q) => b.line_to(q[0], q[1]),
                Cmd::Q(a, q) => b.quad_to(a[0], a[1], q[0], q[1]),
                Cmd::C(a, d, q) => b.cubic_to(a[0], a[1], d[0], d[1], q[0], q[1]),
                Cmd::Z => b.close(),
            }
        }
        b.finish()
    }
}

/// `HandRect`: a rectangle drawn as one slightly wobbly stroke.
pub fn hand_rect(seed: i64, wobble: f32, overshoot: f32, closed: bool, x: f32, y: f32, w: f32, h: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let per = 2.0 * (w + h);
    let p1 = rng.f() * 6.3;
    let p2 = rng.f() * 6.3;
    let f1 = 0.05 + rng.f() * 0.04;
    let f2 = 0.14 + rng.f() * 0.06;
    let at = |d: f32| -> ([f32; 2], [f32; 2]) {
        let mut d = d % per;
        if d < 0.0 {
            d += per;
        }
        if d < w {
            return ([x + d, y], [0.0, -1.0]);
        }
        d -= w;
        if d < h {
            return ([x + w, y + d], [1.0, 0.0]);
        }
        d -= h;
        if d < w {
            return ([x + w - d, y + h], [0.0, 1.0]);
        }
        d -= w;
        ([x, y + h - d], [-1.0, 0.0])
    };
    let mut points = Vec::new();
    let start = -1.0 - rng.f() * 1.5;
    let end = per + if closed { 0.0 } else { overshoot };
    let mut d = start;
    while d <= end {
        let (p, n) = at(d);
        let offset = wobble * ((d * f1 + p1).sin() * 0.6 + (d * f2 + p2).sin() * 0.4);
        points.push([p[0] + n[0] * offset, p[1] + n[1] * offset]);
        d += 5.0;
    }
    Pen::catmull_rom(&points, closed)
}

/// `HandLine`: a horizontal rule, one slightly wobbly ink stroke through the rect's middle.
pub fn hand_line(seed: i64, wobble: f32, x: f32, mid_y: f32, length: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let p1 = rng.f() * 6.3;
    let p2 = rng.f() * 6.3;
    let f1 = 0.015 + rng.f() * 0.015;
    let f2 = 0.06 + rng.f() * 0.04;
    let mut points = Vec::new();
    let mut d = x + rng.f() * 2.0;
    let end = x + length - rng.f() * 2.0;
    while d <= end {
        let offset = wobble * ((d * f1 + p1).sin() * 0.6 + (d * f2 + p2).sin() * 0.4);
        points.push([d, mid_y + offset]);
        d += 6.0;
    }
    Pen::catmull_rom(&points, false)
}

/// `HandLine(vertical: true)`: the rule run top to bottom through `mid_x`.
pub fn hand_line_vertical(seed: i64, wobble: f32, mid_x: f32, y: f32, length: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let p1 = rng.f() * 6.3;
    let p2 = rng.f() * 6.3;
    let f1 = 0.015 + rng.f() * 0.015;
    let f2 = 0.06 + rng.f() * 0.04;
    let mut points = Vec::new();
    let mut d = y + rng.f() * 2.0;
    let end = y + length - rng.f() * 2.0;
    while d <= end {
        let offset = wobble * ((d * f1 + p1).sin() * 0.6 + (d * f2 + p2).sin() * 0.4);
        points.push([mid_x + offset, d]);
        d += 6.0;
    }
    Pen::catmull_rom(&points, false)
}

/// `HandTick`: a pen tick, the way a checklist gets marked off.
pub fn hand_tick(w: f32, h: f32) -> Pen {
    let mut p = Pen::new();
    p.m(0.0, h * 0.55);
    p.q(w * 0.2, h * 0.75, w * 0.36, h);
    p.q(w * 0.55, h * 0.35, w, 0.0);
    p
}

/// `HandArrow(curl: .down)`: curls over and points down, at a button below the note.
pub fn hand_arrow_down(w: f32, h: f32) -> Pen {
    let mut p = Pen::new();
    let (a, b) = ([0.0, 3.0], [w - 3.0, h]);
    p.m(a[0], a[1]);
    p.q(w, 0.0, b[0], b[1]);
    p.m(b[0] - 5.0, b[1] - 7.0);
    p.l(b[0], b[1]);
    p.l(b[0] + 5.0, b[1] - 6.0);
    p
}

/// `HandEllipse`: an editor's pen loop that overshoots its start.
pub fn hand_ellipse(seed: i64, wobble: f32, w: f32, h: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let start = -2.5 + rng.f() * 0.3;
    let sweep = 2.0 * PI + 0.55;
    let phase = rng.f() * 6.3;
    let mut points = Vec::new();
    let mut t = 0.0f32;
    while t <= 1.0 {
        let a = start + sweep * t;
        let grow = 1.0 + 0.1 * t;
        let o = wobble * (a * 3.0 + phase).sin();
        points.push([w / 2.0 + (w / 2.0 * grow + o) * a.cos(), h / 2.0 + (h / 2.0 * grow + o) * a.sin()]);
        t += 1.0 / 40.0;
    }
    Pen::catmull_rom(&points, false)
}

/// `TapeLabel.Tape`: a strip of tape, torn zigzag ends, edges a touch uneven.
pub fn tape(w: f32, h: f32) -> Pen {
    let mut rng = Rng::new(12);
    let mut p = Pen::new();
    let teeth = 5;
    p.m(2.0, 0.5);
    p.l(w - 2.0, 0.0);
    for k in 1..=teeth {
        let y = h * k as f32 / teeth as f32;
        p.l(w - if k % 2 == 0 { 0.0 } else { 2.5 } - rng.f(), y);
    }
    p.l(1.5, h - 0.5);
    for k in (0..teeth).rev() {
        let y = h * k as f32 / teeth as f32;
        p.l(if k % 2 == 0 { 0.0 } else { 2.5 } + rng.f(), y);
    }
    p.z();
    p
}

/// One flat stroke of a rough paintbrush inside `0..w × 0..h`: loaded where it lands on the
/// left, tapering slightly toward the right, with a tiny organic hand sway (capped at ±0.45 pt
/// so it never looks banana-curved) and ragged bristle tips on both ends that stay strictly
/// inside `0..w` so they are never chopped flat.
pub fn paintbrush_stroke(seed: i64, w: f32, h: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let inset_l = 2.8f32.min(w * 0.14);
    let inset_r = 4.8f32.min(w * 0.22);
    let body_w = (w - inset_l - inset_r).max(2.0);
    let steps = 20usize;
    // Tiny organic sway capped at ±0.45 pt — hand-drawn, never banana-curved.
    let bend = (rng.f() - 0.5) * (body_w * 0.012).min(0.9);
    let mut top = Vec::with_capacity(steps + 1);
    let mut bot = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = inset_l + body_w * t;
        let cy = h * 0.5 + (t * PI).sin() * bend;
        // Loaded press on the left (1.08×), gentle lift taper toward the right (0.84×).
        let half = (h * 0.42) * (1.08 - 0.24 * t);
        let top_off = half * (1.0 + (rng.f() - 0.5) * 0.22);
        let bot_off = half * (1.0 + (rng.f() - 0.5) * 0.22);
        top.push([cx, (cy - top_off).clamp(0.15, h - 0.15)]);
        bot.push([cx, (cy + bot_off).clamp(0.15, h - 0.15)]);
    }
    let mut fray = |from: [f32; 2], to: [f32; 2], dir: f32, reach: f32, tips: usize, long_chance: f32| -> Vec<[f32; 2]> {
        (0..=tips * 2)
            .map(|k| {
                let u = k as f32 / (tips * 2) as f32;
                let bx = from[0] + (to[0] - from[0]) * u;
                let by = from[1] + (to[1] - from[1]) * u;
                let out = if k % 2 == 1 {
                    let boost = if rng.f() < long_chance { 0.85 + rng.f() * 0.15 } else { 0.42 + rng.f() * 0.48 };
                    reach * boost
                } else {
                    reach * (rng.f() * 0.14)
                };
                [(bx + dir * out).clamp(0.1, w - 0.1), by]
            })
            .collect()
    };
    let r_reach = (inset_r - 0.15).max(0.5);
    let l_reach = (inset_l - 0.15).max(0.5);
    let mut pts = top.clone();
    pts.extend(fray(top[steps], bot[steps], 1.0, r_reach, 7, 0.28));
    pts.extend(bot.iter().rev().skip(1).copied());
    pts.extend(fray(bot[0], top[0], -1.0, l_reach, 5, 0.15));
    Pen::poly(&pts)
}

/// `BrushSlab.SlabShape`: one loaded brush stroke, blunt where it lands on the left, ragged
/// bristle tips where it lifts on the right, edges that wander. Measured in points from the ends.
pub fn slab(seed: i64, w: f32, h: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let p1 = rng.f() * 6.3;
    let p2 = rng.f() * 6.3;
    let edge = |x: f32, phase: f32| 0.9 * (x * 0.05 + phase).sin() + 0.5 * (x * 0.17 + phase * 2.0).sin();
    let mut pts = Vec::new();
    let mut x = 5.0f32;
    while x <= w - 7.0 {
        let lift = (x / w - 0.7).max(0.0) * 3.0;
        pts.push([x, 1.0 + lift + edge(x, p1)]);
        x += 4.0;
    }
    let mut y = 2.0f32;
    while y < h - 2.0 {
        let gap = 1.2 + rng.f() * 2.6;
        let long = rng.f() < 0.18;
        let reach = if long { 7.0 + rng.f() * 6.0 } else { 1.0 + rng.f() * 4.5 };
        let base = w - 8.0 + (rng.f() - 0.5) * 3.0;
        pts.push([base, y]);
        pts.push([base + reach, y + gap * (0.3 + rng.f() * 0.3)]);
        y += gap;
    }
    x = w - 7.0;
    while x >= 5.0 {
        let lift = (x / w - 0.7).max(0.0) * 3.0;
        pts.push([x, h - 1.0 - lift + edge(x, p2)]);
        x -= 4.0;
    }
    let mut yl = h - 2.0;
    while yl > 2.0 {
        let gap = 1.5 + rng.f() * 3.0;
        let bulge = (yl / h * PI).sin() * 2.0;
        let fray = if rng.f() < 0.35 { 1.5 + rng.f() * 3.0 } else { rng.f() * 0.8 };
        pts.push([5.0 - bulge, yl]);
        pts.push([5.0 - bulge - fray, yl - gap * 0.5]);
        yl -= gap;
    }
    Pen::poly(&pts)
}

/// `PenStroke`: one pen stroke through `points` (in a `w`×`h` box), smoothed, pressing in at
/// the start and lifting at the end; a closed ribbon to print through the screen.
pub fn pen_stroke(points: &[[f32; 2]], w: f32, h: f32, width: f32, start_taper: f32, end_taper: f32, seed: i64) -> Pen {
    let pts: Vec<[f32; 2]> = points.iter().map(|p| [p[0] * w, p[1] * h]).collect();
    let mut c = Vec::new();
    for i in 0..pts.len() - 1 {
        let a = pts[i.saturating_sub(1)];
        let b = pts[i];
        let d = pts[i + 1];
        let e = pts[(i + 2).min(pts.len() - 1)];
        for k in 0..10 {
            let t = k as f32 / 10.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, d: f32, e: f32| 0.5 * ((2.0 * b) + (-a + d) * t + (2.0 * a - 5.0 * b + 4.0 * d - e) * t2 + (-a + 3.0 * b - 3.0 * d + e) * t3);
            c.push([f(a[0], b[0], d[0], e[0]), f(a[1], b[1], d[1], e[1])]);
        }
    }
    c.push(pts[pts.len() - 1]);
    let mut rng = Rng::new(seed);
    let mut left = Vec::new();
    let mut right = Vec::new();
    for i in 0..c.len() {
        let a = c[i.saturating_sub(1)];
        let b = c[(i + 1).min(c.len() - 1)];
        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
        let n = [-(b[1] - a[1]) / len, (b[0] - a[0]) / len];
        let t = i as f32 / (c.len() - 1) as f32;
        let press = (t / 0.15).min(1.0) * (1.0 - start_taper) + start_taper;
        let lift = ((1.0 - t) / 0.35).min(1.0) * (1.0 - end_taper) + end_taper;
        let half = width / 2.0 * press.min(lift) * (1.0 + (rng.f() - 0.5) * 0.12);
        left.push([c[i][0] + n[0] * half, c[i][1] + n[1] * half]);
        right.push([c[i][0] - n[0] * half, c[i][1] - n[1] * half]);
    }
    let mut all = left;
    all.extend(right.into_iter().rev());
    Pen::poly(&all)
}
