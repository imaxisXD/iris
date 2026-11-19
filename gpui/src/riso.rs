//! Riso printing: the inks, the seeded hand, and ink printed through the screen.
//!
//! Ported from the Swift app (`DesignSystem.swift`, `Zine.swift`, `Components.swift`) line for
//! line where it makes a mark, so the same seed puts the same speck, wobble and bristle in the
//! same place. Printed areas are baked on the CPU into RGBA at the window's device scale and
//! drawn by GPUI as images (`Img`); pen lines are GPUI vector paths (`pen.rs`).

use std::cell::RefCell;
use std::collections::HashMap;

use std::sync::Arc;

use gpui::{Rgba, RenderImage};
use tiny_skia::{FillRule, Mask, MaskType, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Bytes of one riso ink.
pub type Rgb8 = (u8, u8, u8);

/// Every riso ink Iris prints with (`Riso` in Swift).
pub mod ink {
    use super::Rgb8;
    pub const PINK: Rgb8 = (240, 134, 176);
    pub const PINK_TEXT: Rgb8 = (142, 36, 83);
    pub const ROSE: Rgb8 = (170, 102, 124);
    pub const ESPRESSO: Rgb8 = (100, 56, 46);
    pub const INK: Rgb8 = (42, 26, 22);
    pub const RED: Rgb8 = (217, 67, 46);
    pub const CREMA: Rgb8 = (226, 200, 185);
    pub const PAPER: Rgb8 = (255, 252, 249);
}

/// The neutral ladder and hues as bytes (`DS.Ink`, `DS.Hue`).
pub mod hue {
    use super::Rgb8;
    pub const I1: Rgb8 = (0xF7, 0xF3, 0xF0);
    pub const I3: Rgb8 = (0xE9, 0xE2, 0xDC);
    pub const I9: Rgb8 = (0x84, 0x76, 0x6D);
    pub const I10: Rgb8 = (0x6B, 0x5F, 0x58);
    pub const I11: Rgb8 = (0x4A, 0x40, 0x3B);
    pub const I12: Rgb8 = super::ink::INK;
    pub const GROUND: Rgb8 = (0xF0, 0xEB, 0xE6);
    pub const WARN_TINT: Rgb8 = (0xF4, 0xE7, 0xDC);
    pub const WARN_TEXT: Rgb8 = (0x6E, 0x3F, 0x22);
    pub const RED_TEXT: Rgb8 = (0xA5, 0x2A, 0x18);
    pub const GREEN: Rgb8 = (0x1E, 0x8C, 0x55);
    pub const TAPE: Rgb8 = (0xE0, 0xD1, 0xBB);
}

/// A printed picture, as GPUI draws it: BGRA, straight alpha, at the window's device scale.
pub type Img = Arc<RenderImage>;

/// Straight-alpha RGBA `w`×`h` pixels as a GPUI image.
pub fn image(w: u32, h: u32, mut rgba: Vec<u8>) -> Img {
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw(w, h, rgba).expect("w×h×4 bytes");
    Arc::new(RenderImage::new(vec![image::Frame::new(buf)]))
}

thread_local! {
    /// Pictures made for one frame (a garden caught at this moment), and the last frame's.
    static FRAME_PICTURES: RefCell<(Vec<Img>, Vec<Img>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// A picture for this frame only: GPUI keeps every picture it has drawn in its texture atlas
/// until it's told to let go, so a picture made fresh each frame must be let go of once the
/// next frame has replaced it (`let_go_of_last_frame`).
pub fn frame_image(w: u32, h: u32, rgba: Vec<u8>) -> Img {
    let img = image(w, h, rgba);
    FRAME_PICTURES.with(|f| f.borrow_mut().0.push(img.clone()));
    img
}

/// At the start of a frame: drops the pictures of the frame before last from GPUI's atlas.
pub fn let_go_of_last_frame(window: &mut gpui::Window) {
    let old = FRAME_PICTURES.with(|f| {
        let mut f = f.borrow_mut();
        let made = std::mem::take(&mut f.0);
        std::mem::replace(&mut f.1, made)
    });
    for img in old {
        let _ = window.drop_image(img);
    }
}

/// An sRGB byte colour.
pub fn col(c: Rgb8) -> Rgba {
    Rgba { r: c.0 as f32 / 255.0, g: c.1 as f32 / 255.0, b: c.2 as f32 / 255.0, a: 1.0 }
}

/// `c` at `alpha` over `under`, as one opaque colour (mixed in sRGB, as Core Graphics does).
pub fn over(c: Rgb8, alpha: f32, under: Rgb8) -> Rgba {
    let m = |a: u8, b: u8| (a as f32 * alpha + b as f32 * (1.0 - alpha)) / 255.0;
    Rgba { r: m(c.0, under.0), g: m(c.1, under.1), b: m(c.2, under.2), a: 1.0 }
}

/// Multiply blend of an ink onto the paper, as bytes: what `.blendMode(.multiply)` prints there.
pub fn multiply(c: Rgb8, under: Rgb8) -> Rgb8 {
    let m = |a: u8, b: u8| ((a as u32 * b as u32 + 127) / 255) as u8;
    (m(c.0, under.0), m(c.1, under.1), m(c.2, under.2))
}

// MARK: - The seeded hand

/// `SeededRandom`: the same LCG as Swift, so a seed draws the same wobble.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: i64) -> Rng {
        let s = seed.wrapping_mul(2_654_435_761).wrapping_add(1) as u64;
        Rng(s | 1)
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
    pub fn f(&mut self) -> f32 {
        self.next() as f32
    }
}

/// Swift's string seeds: `s.unicodeScalars.reduce(start) { $0 &* 31 &+ Int($1.value) }`.
pub fn str_seed(s: &str, start: i64) -> i64 {
    s.chars().fold(start, |acc, c| acc.wrapping_mul(31).wrapping_add(c as i64))
}

// MARK: - The screen

/// `RisoArt.screen`: a plate `w`×`h` points big, a point to a dot: nearly solid ink, a faint
/// printing grid, and sparse paper specks gathering where the drum ran dry. RGB bytes.
pub fn screen(w: usize, h: usize, rgb: Rgb8, seed: i64, grid: f64, specks: f64) -> Vec<Rgb8> {
    let (w, h) = (w.max(1), h.max(1));
    let mut rng = Rng::new(seed);
    struct Noise {
        cell: usize,
        gw: usize,
        lattice: Vec<f64>,
    }
    impl Noise {
        fn new(cell: usize, w: usize, h: usize, rng: &mut Rng) -> Noise {
            let gw = w / cell + 2;
            let lattice = (0..gw * (h / cell + 2)).map(|_| rng.next()).collect();
            Noise { cell, gw, lattice }
        }
        fn at(&self, x: usize, y: usize) -> f64 {
            let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
            let (gx, gy) = (x / self.cell, y / self.cell);
            let fx = smooth((x % self.cell) as f64 / self.cell as f64);
            let fy = smooth((y % self.cell) as f64 / self.cell as f64);
            let a = self.lattice[gy * self.gw + gx];
            let b = self.lattice[gy * self.gw + gx + 1];
            let c = self.lattice[(gy + 1) * self.gw + gx];
            let d = self.lattice[(gy + 1) * self.gw + gx + 1];
            let top = a + (b - a) * fx;
            top + ((c + (d - c) * fx) - top) * fy
        }
    }
    let dry = Noise::new(70, w, h, &mut rng);
    let tone = Noise::new(46, w, h, &mut rng);
    let paper = (ink::PAPER.0 as f64, ink::PAPER.1 as f64, ink::PAPER.2 as f64);
    let mut out = vec![(0u8, 0u8, 0u8); w * h];
    let mut speck = vec![false; w * h];
    let toward = |c: u8, p: f64, k: f64| (c as f64 + (p - c as f64) * k) as u8;
    for y in 0..h {
        for x in 0..w {
            let o = y * w + x;
            let chance = (0.004 + 0.085 * (dry.at(x, y) - 0.58).max(0.0)) * specks;
            if rng.next() < chance || (x > 0 && speck[o - 1] && rng.next() < 0.35 * specks) {
                let k = if specks < 1.0 { 0.45 } else { 1.0 };
                out[o] = (toward(rgb.0, paper.0, k), toward(rgb.1, paper.1, k), toward(rgb.2, paper.2, k));
                speck[o] = true;
                continue;
            }
            let mut lift = ((if y % 3 == 0 { 0.1 } else { 0.0 }) + (if x % 3 == 0 { 0.07 } else { 0.0 })) * grid;
            lift += (tone.at(x, y) - 0.5) * 0.1;
            let lift = lift.clamp(0.0, 0.3);
            out[o] = (toward(rgb.0, paper.0, lift), toward(rgb.1, paper.1, lift), toward(rgb.2, paper.2, lift));
        }
    }
    out
}

/// `PrintedInk`'s screen settings: dark ink shows the screen far more, so it gets a lighter touch.
pub fn ink_screen(rgb: Rgb8) -> (f64, f64) {
    let dark = (rgb.0 as u32 + rgb.1 as u32 + rgb.2 as u32) < 240;
    if dark { (0.22, 0.35) } else { (0.55, 0.6) }
}

// MARK: - Printing a shape

/// A shape's coverage at device pixels, drawn by `draw` in points.
#[derive(Clone, Copy)]
pub struct Plate {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

impl Plate {
    pub fn px(&self) -> (u32, u32) {
        (((self.w * self.scale).round() as u32).max(1), ((self.h * self.scale).round() as u32).max(1))
    }
    pub fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }
    /// An empty mask the plate's size.
    pub fn mask(&self) -> Mask {
        let (w, h) = self.px();
        Mask::new(w, h).unwrap()
    }
    /// The coverage of a filled path, antialiased.
    pub fn fill(&self, path: &tiny_skia::Path) -> Mask {
        let mut m = self.mask();
        m.fill_path(path, FillRule::Winding, true, self.transform());
        m
    }
}

/// Ink printed through the screen (`PrintedInk`), cut to `coverage`: the screen is made a dot a
/// point (`w`×`h` truncated, as Swift does), stretched to the plate without smoothing, the way
/// SwiftUI draws it with `.interpolation(.none)`. Straight-alpha RGBA, `alpha` times coverage.
pub fn print(plate: &Plate, rgb: Rgb8, seed: i64, coverage: &Mask, alpha: f32) -> Vec<u8> {
    let (grid, specks) = ink_screen(rgb);
    print_with(plate, rgb, seed, grid, specks, coverage, alpha, |c| c)
}

#[allow(clippy::too_many_arguments)]
pub fn print_with(plate: &Plate, rgb: Rgb8, seed: i64, grid: f64, specks: f64, coverage: &Mask, alpha: f32,
                  tint: impl Fn(Rgb8) -> Rgb8) -> Vec<u8> {
    let (sw, sh) = ((plate.w as usize).max(1), (plate.h as usize).max(1));
    let dots = screen(sw, sh, rgb, seed, grid, specks);
    let (pw, ph) = plate.px();
    let cov = coverage.data();
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        let sy = ((y as f32 + 0.5) / ph as f32 * sh as f32) as usize;
        for x in 0..pw {
            let i = (y * pw + x) as usize;
            let a = cov[i] as f32 / 255.0 * alpha;
            if a <= 0.0 {
                continue;
            }
            let sx = ((x as f32 + 0.5) / pw as f32 * sw as f32) as usize;
            let c = tint(dots[sy.min(sh - 1) * sw + sx.min(sw - 1)]);
            out[i * 4] = c.0;
            out[i * 4 + 1] = c.1;
            out[i * 4 + 2] = c.2;
            out[i * 4 + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// A flat colour cut to `coverage` (the few things Swift leaves flat: tape, badge tints).
pub fn flat(plate: &Plate, rgb: Rgb8, coverage: &Mask, alpha: f32) -> Vec<u8> {
    let (pw, ph) = plate.px();
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for (i, c) in coverage.data().iter().enumerate() {
        if *c == 0 {
            continue;
        }
        out[i * 4] = rgb.0;
        out[i * 4 + 1] = rgb.1;
        out[i * 4 + 2] = rgb.2;
        out[i * 4 + 3] = (*c as f32 * alpha).round() as u8;
    }
    out
}

/// Straight-alpha RGBA `top` laid over `under` (same size), source-over.
pub fn lay(under: &mut [u8], top: &[u8]) {
    for (u, t) in under.chunks_exact_mut(4).zip(top.chunks_exact(4)) {
        let ta = t[3] as f32 / 255.0;
        if ta <= 0.0 {
            continue;
        }
        let ua = u[3] as f32 / 255.0;
        let oa = ta + ua * (1.0 - ta);
        for k in 0..3 {
            let v = (t[k] as f32 * ta + u[k] as f32 * ua * (1.0 - ta)) / oa;
            u[k] = v.round().clamp(0.0, 255.0) as u8;
        }
        u[3] = (oa * 255.0).round() as u8;
    }
}

// MARK: - Cache

thread_local! {
    static PRINTS: RefCell<HashMap<String, Img>> = RefCell::new(HashMap::new());
}

/// Prints once per key and keeps it, so a redraw never re-dithers.
pub fn cached(key: String, plate: &Plate, make: impl FnOnce() -> Vec<u8>) -> Img {
    let key = format!("{key}@{}x{}x{}", plate.w, plate.h, plate.scale);
    if let Some(p) = PRINTS.with(|m| m.borrow().get(&key).cloned()) {
        return p;
    }
    let (w, h) = plate.px();
    let began = std::time::Instant::now();
    let p = image(w, h, make());
    if std::env::var("IRIS_TRACE").is_ok() {
        eprintln!("print ms={:.2} {key}", began.elapsed().as_secs_f64() * 1000.0);
    }
    PRINTS.with(|m| m.borrow_mut().insert(key, p.clone()));
    p
}

/// A small picture made at `src` pixels square and shown `points` square without smoothing
/// (SwiftUI's `.interpolation(.none)`): scaled up a whole source pixel at a time, as Swift
/// draws the riso icons, so every printed dot stays crisp instead of blurring into the paper.
pub fn cached_crisp(key: String, src: u32, make: impl FnOnce() -> Vec<u8>, points: f32, scale: f32) -> Img {
    thread_local! { static RAW: RefCell<HashMap<String, std::rc::Rc<Vec<u8>>>> = RefCell::new(HashMap::new()); }
    let n = ((points * scale).round() as u32).max(1);
    let out_key = format!("crisp-{key}-{n}");
    if let Some(p) = PRINTS.with(|m| m.borrow().get(&out_key).cloned()) {
        return p;
    }
    let raw = RAW.with(|r| r.borrow().get(&key).cloned()).unwrap_or_else(|| {
        let v = std::rc::Rc::new(make());
        RAW.with(|r| r.borrow_mut().insert(key.clone(), v.clone()));
        v
    });
    let mut out = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        let sy = (((y as f32 + 0.5) * src as f32 / n as f32) as u32).min(src - 1);
        for x in 0..n {
            let sx = (((x as f32 + 0.5) * src as f32 / n as f32) as u32).min(src - 1);
            let (si, di) = (((sy * src + sx) * 4) as usize, ((y * n + x) * 4) as usize);
            out[di..di + 4].copy_from_slice(&raw[si..si + 4]);
        }
    }
    let p = image(n, n, out);
    PRINTS.with(|m| m.borrow_mut().insert(out_key, p.clone()));
    p
}

// MARK: - Paper grain

/// `.grain(0.22)`: the paper's tooth, a 128-pixel noise tile shown at 64 points, multiplied
/// over the sheet. Baked as near-black dots at the multiply's strength over paper.
pub fn grain(w: f32, h: f32, scale: f32, opacity: f32) -> Img {
    let plate = Plate { w, h, scale };
    cached(format!("grain-{opacity}"), &plate, || grain_rgba(w, h, scale, opacity))
}

/// The grain's straight-alpha RGBA, `w`×`h` points at `scale`.
pub fn grain_rgba(w: f32, h: f32, scale: f32, opacity: f32) -> Vec<u8> {
    let plate = Plate { w, h, scale };
    let size = 128usize;
    let mut rng = Rng::new(42);
    let tile: Vec<u8> = (0..size * size)
        .map(|_| {
            let v = rng.next();
            ((v - 0.35) * 170.0).clamp(0.0, 255.0) as u8
        })
        .collect();
    let dot = multiply((26, 20, 16), ink::PAPER);
    let (pw, ph) = plate.px();
    // The tile is 64 pt: two of its pixels a point.
    let per_px = 2.0 / scale;
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        let ty = ((y as f32 * per_px) as usize) % size;
        for x in 0..pw {
            let tx = ((x as f32 * per_px) as usize) % size;
            let a = tile[ty * size + tx] as f32 / 255.0 * opacity;
            let i = ((y * pw + x) * 4) as usize;
            out[i] = dot.0;
            out[i + 1] = dot.1;
            out[i + 2] = dot.2;
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

// MARK: - Riso icons

/// A 4×4 Bayer screen: the threshold each dot of a plate is printed against.
const BAYER: [[f64; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// `RisoArt.print`: greyscale plates printed one over another, each ordered-dithered in its own
/// ink and nudged off register. Each plate draws in the 80-point canvas onto a white pixmap.
pub fn print_icon(pixels: u32, seed: i64, plates: &[(Rgb8, i32, i32, &dyn Fn(&mut Pixmap, Transform))]) -> Vec<u8> {
    let (w, h) = (pixels as i32, pixels as i32);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut rng = Rng::new(seed);
    let s = pixels as f32 / 80.0;
    for (rgb, dx, dy, draw) in plates {
        let mut pm = Pixmap::new(pixels, pixels).unwrap();
        pm.fill(tiny_skia::Color::WHITE);
        draw(&mut pm, Transform::from_scale(s, s));
        let data = pm.data();
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x - dx, y - dy);
                if sx < 0 || sy < 0 || sx >= w || sy >= h {
                    continue;
                }
                let gray = data[((sy * w + sx) * 4) as usize] as f64;
                let dark = 1.0 - gray / 255.0;
                if dark <= 0.02 {
                    continue;
                }
                let t = (BAYER[(y % 4) as usize][(x % 4) as usize] + 0.5) / 16.0 + (rng.next() - 0.5) * 0.35;
                if dark > t {
                    let o = ((y * w + x) * 4) as usize;
                    out[o] = rgb.0;
                    out[o + 1] = rgb.1;
                    out[o + 2] = rgb.2;
                    out[o + 3] = 255;
                }
            }
        }
    }
    out
}

fn gray_paint(g: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(g, g, g, 255);
    p.anti_alias = true;
    p
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let k = 0.552_284_8 * r;
    let mut b = PathBuilder::new();
    b.move_to(x + r, y);
    b.line_to(x + w - r, y);
    b.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    b.line_to(x + w, y + h - r);
    b.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    b.line_to(x + r, y + h);
    b.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    b.line_to(x, y + r);
    b.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    b.close();
    b.finish().unwrap()
}

/// `RisoIcon.apps`: four tiles, one pink, printed at 44 pixels (crema shade, pink, espresso ink).
pub fn apps_icon() -> (Vec<u8>, u32) {
    {
        let origins = [(12.0, 12.0), (44.0, 12.0), (12.0, 44.0), (44.0, 44.0)];
        let shade = |pm: &mut Pixmap, t: Transform| {
            for (x, y) in origins {
                pm.fill_path(&rounded(x + 1.0, y + 1.0, 24.0, 24.0, 6.0), &gray_paint(150), FillRule::Winding, t, None);
            }
        };
        let pink = |pm: &mut Pixmap, t: Transform| {
            for (i, (x, y)) in origins.iter().enumerate() {
                let g = if i == 1 { 25 } else { 180 };
                pm.fill_path(&rounded(*x, *y, 24.0, 24.0, 6.0), &gray_paint(g), FillRule::Winding, t, None);
            }
        };
        let inked = |pm: &mut Pixmap, t: Transform| {
            let stroke = Stroke { width: 4.8, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
            for (x, y) in origins {
                pm.stroke_path(&rounded(x, y, 24.0, 24.0, 6.0), &gray_paint(25), &stroke, t, None);
            }
        };
        (print_icon(44, str_seed("apps", 11), &[(ink::CREMA, 2, 2, &shade), (ink::PINK, -1, -1, &pink), (ink::ESPRESSO, 0, 0, &inked)]), 44)
    }
}

/// A mask's alpha as a paint target: a pixmap filled opaque, for cutting gaps out of.
pub fn opaque(plate: &Plate, pad: f32) -> Pixmap {
    let (w, h) = plate.px();
    let mut pm = Pixmap::new(w, h).unwrap();
    let mut p = Paint::default();
    p.set_color_rgba8(0, 0, 0, 255);
    let r = tiny_skia::Rect::from_xywh(-pad, -pad, plate.w + 2.0 * pad, plate.h + 2.0 * pad).unwrap();
    pm.fill_rect(r, &p, plate.transform(), None);
    pm
}

/// A pixmap's alpha as a mask.
pub fn alpha_mask(pm: &Pixmap) -> Mask {
    Mask::from_pixmap(pm.as_ref(), MaskType::Alpha)
}

/// Two masks multiplied.
pub fn intersect(a: &Mask, b: &Mask) -> Mask {
    let mut m = a.clone();
    for (x, y) in m.data_mut().iter_mut().zip(b.data()) {
        *x = ((*x as u32 * *y as u32 + 127) / 255) as u8;
    }
    m
}
