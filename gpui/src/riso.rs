//! Riso printing: the inks, the seeded hand, and ink printed through the screen.
//!
//! Ported from the Swift app (`DesignSystem.swift`, `Zine.swift`, `Components.swift`) line for
//! line where it makes a mark, so the same seed puts the same speck, wobble and bristle in the
//! same place. Printed areas are baked on the CPU into RGBA at the window's device scale and
//! drawn by GPUI as images (`Img`); pen lines are GPUI vector paths (`pen.rs`).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

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
    /// The window currently building its frame (`let_go_of_last_frame` sets it).
    static CURRENT_WINDOW: Cell<Option<gpui::WindowId>> = const { Cell::new(None) };
    /// Pictures made for one frame per window (a garden caught at this moment), and the last frame's.
    static FRAME_PICTURES: RefCell<HashMap<Option<gpui::WindowId>, (Vec<Img>, Vec<Img>)>> = RefCell::new(HashMap::new());
    /// Evicted `PRINTS` queued per window that previously rendered them, drained only when that
    /// window itself starts its next frame (never dropping textures out from under an idle window).
    static WINDOW_EVICTED: RefCell<HashMap<gpui::WindowId, Vec<Img>>> = RefCell::new(HashMap::new());
}

/// A picture for this frame only: GPUI keeps every picture it has drawn in its texture atlas
/// until it's told to let go, so a picture made fresh each frame must be let go of once the
/// next frame has replaced it (`let_go_of_last_frame`).
pub fn frame_image(w: u32, h: u32, rgba: Vec<u8>) -> Img {
    let img = image(w, h, rgba);
    let win = CURRENT_WINDOW.get();
    FRAME_PICTURES.with(|f| f.borrow_mut().entry(win).or_default().0.push(img.clone()));
    img
}

/// At the start of a frame: drops this window's pictures from the frame before last, and drops
/// any `PRINTS` entries evicted since this window's last frame from this window's atlas only.
pub fn let_go_of_last_frame(window: &mut gpui::Window) {
    let id = window.window_handle().window_id();
    CURRENT_WINDOW.set(Some(id));
    let old = FRAME_PICTURES.with(|f| {
        let mut f = f.borrow_mut();
        let entry = f.entry(Some(id)).or_default();
        let made = std::mem::take(&mut entry.0);
        std::mem::replace(&mut entry.1, made)
    });
    for img in old {
        let _ = window.drop_image(img);
    }
    let evicted = WINDOW_EVICTED.with(|e| e.borrow_mut().remove(&id)).unwrap_or_default();
    for img in evicted {
        let _ = window.drop_image(img);
    }
}

/// Releases any retained per-frame pictures and queued evictions for a window that just closed.
pub fn forget_window(id: gpui::WindowId) {
    if CURRENT_WINDOW.get() == Some(id) {
        CURRENT_WINDOW.set(None);
    }
    FRAME_PICTURES.with(|f| {
        f.borrow_mut().remove(&Some(id));
    });
    WINDOW_EVICTED.with(|e| {
        e.borrow_mut().remove(&id);
    });
    PRINTS.with(|m| m.borrow_mut().forget_window(id));
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

/// Soft ceiling on cached `RenderImage` pixel bytes kept in `PRINTS` (~64 MiB).
const PRINTS_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Soft ceiling on cached crisp source RGBA buffers kept in `RAW` (~8 MiB).
const RAW_MAX_BYTES: usize = 8 * 1024 * 1024;

struct ByteLru<V> {
    map: HashMap<String, (V, usize, u64)>,
    bytes: usize,
    tick: u64,
    limit: usize,
}

impl<V: Clone> ByteLru<V> {
    fn new(limit: usize) -> Self {
        Self { map: HashMap::new(), bytes: 0, tick: 0, limit }
    }

    fn get(&mut self, key: &str) -> Option<V> {
        let tick = self.tick.wrapping_add(1);
        self.tick = tick;
        let entry = self.map.get_mut(key)?;
        entry.2 = tick;
        Some(entry.0.clone())
    }

    fn insert(&mut self, key: String, val: V, size: usize) -> Vec<V> {
        let tick = self.tick.wrapping_add(1);
        self.tick = tick;
        let mut evicted = Vec::new();
        if let Some((old_val, old_size, _)) = self.map.insert(key, (val, size, tick)) {
            self.bytes = self.bytes.saturating_sub(old_size);
            evicted.push(old_val);
        }
        self.bytes = self.bytes.saturating_add(size);
        while self.bytes > self.limit && self.map.len() > 1 {
            let Some(oldest) = self.map.iter().min_by_key(|(_, (_, _, t))| *t).map(|(k, _)| k.clone()) else {
                break;
            };
            if let Some((old_val, old_size, _)) = self.map.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(old_size);
                evicted.push(old_val);
            }
        }
        evicted
    }
}

struct PrintEntry<V, W> {
    val: V,
    bytes: usize,
    tick: u64,
    windows: HashSet<W>,
}

struct PrintLru<V, W> {
    map: HashMap<String, PrintEntry<V, W>>,
    bytes: usize,
    tick: u64,
    limit: usize,
}

impl<V: Clone, W: Eq + std::hash::Hash + Copy> PrintLru<V, W> {
    fn new(limit: usize) -> Self {
        Self { map: HashMap::new(), bytes: 0, tick: 0, limit }
    }

    fn get(&mut self, key: &str, win: Option<W>) -> Option<V> {
        let tick = self.tick.wrapping_add(1);
        self.tick = tick;
        let entry = self.map.get_mut(key)?;
        entry.tick = tick;
        if let Some(w) = win {
            entry.windows.insert(w);
        }
        Some(entry.val.clone())
    }

    fn insert(&mut self, key: String, val: V, size: usize, win: Option<W>) -> Vec<(V, HashSet<W>)> {
        let tick = self.tick.wrapping_add(1);
        self.tick = tick;
        let mut windows = HashSet::new();
        if let Some(w) = win {
            windows.insert(w);
        }
        let mut evicted = Vec::new();
        if let Some(old) = self.map.insert(key, PrintEntry { val, bytes: size, tick, windows }) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
            evicted.push((old.val, old.windows));
        }
        self.bytes = self.bytes.saturating_add(size);
        while self.bytes > self.limit && self.map.len() > 1 {
            let Some(oldest) = self.map.iter().min_by_key(|(_, e)| e.tick).map(|(k, _)| k.clone()) else {
                break;
            };
            if let Some(old) = self.map.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(old.bytes);
                evicted.push((old.val, old.windows));
            }
        }
        evicted
    }

    fn forget_window(&mut self, win: W) {
        for entry in self.map.values_mut() {
            entry.windows.remove(&win);
        }
    }
}

thread_local! {
    static PRINTS: RefCell<PrintLru<Img, gpui::WindowId>> = RefCell::new(PrintLru::new(PRINTS_MAX_BYTES));
}

fn cache_get(key: &str) -> Option<Img> {
    let win = CURRENT_WINDOW.get();
    PRINTS.with(|m| m.borrow_mut().get(key, win))
}

fn cache_insert(key: String, img: Img, bytes: usize) {
    let win = CURRENT_WINDOW.get();
    let evicted = PRINTS.with(|m| m.borrow_mut().insert(key, img, bytes, win));
    if !evicted.is_empty() {
        WINDOW_EVICTED.with(|e| {
            let mut queues = e.borrow_mut();
            for (img, windows) in evicted {
                for w in windows {
                    queues.entry(w).or_default().push(img.clone());
                }
            }
        });
    }
}

/// Prints once per key and keeps it (bounded by an LRU byte budget), so a redraw never re-dithers.
pub fn cached(key: String, plate: &Plate, make: impl FnOnce() -> Vec<u8>) -> Img {
    try_cached(key, plate, || Some(make())).expect("`make` always gives a picture")
}

/// `cached` for a picture that may not exist (an app without an icon): `make` runs only on a
/// miss, and a `None` is not kept.
pub fn try_cached(key: String, plate: &Plate, make: impl FnOnce() -> Option<Vec<u8>>) -> Option<Img> {
    let key = format!("{key}@{}x{}x{}", plate.w, plate.h, plate.scale);
    if let Some(p) = cache_get(&key) {
        return Some(p);
    }
    let (w, h) = plate.px();
    let began = std::time::Instant::now();
    let bytes = (w as usize).saturating_mul(h as usize).saturating_mul(4);
    let p = image(w, h, make()?);
    if std::env::var("IRIS_TRACE").is_ok() {
        eprintln!("print ms={:.2} {key}", began.elapsed().as_secs_f64() * 1000.0);
    }
    cache_insert(key, p.clone(), bytes);
    Some(p)
}

/// A small picture made at `src` pixels square and shown `points` square without smoothing
/// (SwiftUI's `.interpolation(.none)`): scaled up a whole source pixel at a time, as Swift
/// draws the riso icons, so every printed dot stays crisp instead of blurring into the paper.
pub fn cached_crisp(key: String, src: u32, make: impl FnOnce() -> Vec<u8>, points: f32, scale: f32) -> Img {
    thread_local! { static RAW: RefCell<ByteLru<std::rc::Rc<Vec<u8>>>> = RefCell::new(ByteLru::new(RAW_MAX_BYTES)); }
    let n = ((points * scale).round() as u32).max(1);
    let out_key = format!("crisp-{key}-{n}");
    if let Some(p) = cache_get(&out_key) {
        return p;
    }
    let raw = RAW.with(|r| r.borrow_mut().get(&key)).unwrap_or_else(|| {
        let data = make();
        let size = data.len();
        let v = std::rc::Rc::new(data);
        RAW.with(|r| {
            let _ = r.borrow_mut().insert(key.clone(), v.clone(), size);
        });
        v
    });
    let bytes = (n as usize).saturating_mul(n as usize).saturating_mul(4);
    // Windows at 100–150 %: fewer pixels than the print, and a nearest-pixel shrink drops whole
    // rows and columns of its dots (Settings' ⌘ broke apart, the Tones squiggle went). Blended
    // down instead, each screen pixel the dots it covers: the print as a Mac's 2× reads from
    // arm's length. Windows only: the Mac draws exactly as it did.
    if cfg!(windows) && n < src {
        let p = image(n, n, blend_down(&raw, src, n));
        cache_insert(out_key, p.clone(), bytes);
        return p;
    }
    let mut out = vec![0u8; bytes];
    for y in 0..n {
        let sy = (((y as f32 + 0.5) * src as f32 / n as f32) as u32).min(src - 1);
        for x in 0..n {
            let sx = (((x as f32 + 0.5) * src as f32 / n as f32) as u32).min(src - 1);
            let (si, di) = (((sy * src + sx) * 4) as usize, ((y * n + x) * 4) as usize);
            out[di..di + 4].copy_from_slice(&raw[si..si + 4]);
        }
    }
    let p = image(n, n, out);
    cache_insert(out_key, p.clone(), bytes);
    p
}

/// A `src`-pixel RGBA print shrunk to `n` pixels by area: each pixel the average of the dots it
/// covers (premultiplied, so the paper between dots stays see-through).
#[cfg_attr(not(windows), allow(dead_code))]
fn blend_down(raw: &[u8], src: u32, n: u32) -> Vec<u8> {
    let r = src as f32 / n as f32;
    let span = |i: u32| -> Vec<(u32, f32)> {
        let (a, b) = (i as f32 * r, (i + 1) as f32 * r);
        (a.floor() as u32..(b.ceil() as u32).min(src)).map(|s| (s, (b.min(s as f32 + 1.0) - a.max(s as f32)).max(0.0))).collect()
    };
    let mut out = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        let ys = span(y);
        for x in 0..n {
            let mut acc = [0f32; 4];
            for &(sx, wx) in &span(x) {
                for &(sy, wy) in &ys {
                    let i = ((sy * src + sx) * 4) as usize;
                    let a = raw[i + 3] as f32 / 255.0 * wx * wy;
                    for c in 0..3 {
                        acc[c] += raw[i + c] as f32 * a;
                    }
                    acc[3] += a;
                }
            }
            let o = ((y * n + x) * 4) as usize;
            let alpha = acc[3] / (r * r);
            if acc[3] > 0.0 {
                for c in 0..3 {
                    out[o + c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
            }
            out[o + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::{ByteLru, PrintLru, blend_down};

    /// Shrinking 44 to 24 keeps every dot's share: solid ink stays solid, the paper stays clear,
    /// and a checkerboard of dots comes out as that ink at half strength, not as holes or blots.
    #[test]
    fn blend_down_keeps_the_ink() {
        let (src, n) = (44u32, 24u32);
        let px = |f: &dyn Fn(u32, u32) -> bool| -> Vec<u8> {
            (0..src * src).flat_map(|i| if f(i % src, i / src) { [0x58, 0x2A, 0x20, 255] } else { [0, 0, 0, 0] }).collect()
        };
        let solid = blend_down(&px(&|_, _| true), src, n);
        assert!(solid.chunks(4).all(|p| p == [0x58, 0x2A, 0x20, 255]));
        let clear = blend_down(&px(&|_, _| false), src, n);
        assert!(clear.chunks(4).all(|p| p[3] == 0));
        let checks = blend_down(&px(&|x, y| (x + y) % 2 == 0), src, n);
        for p in checks.chunks(4) {
            assert_eq!(&p[..3], &[0x58, 0x2A, 0x20]);
            assert!((100..=155).contains(&p[3]), "alpha {}", p[3]);
        }
    }

    #[test]
    fn byte_lru_evicts_least_recently_used() {
        let mut lru = ByteLru::new(100);
        assert!(lru.insert("a".into(), 1, 40).is_empty());
        assert!(lru.insert("b".into(), 2, 40).is_empty());
        // Touch "a" so "b" becomes least recently used.
        assert_eq!(lru.get("a"), Some(1));
        let evicted = lru.insert("c".into(), 3, 40);
        assert_eq!(evicted, vec![2]);
        assert_eq!(lru.get("b"), None);
        assert_eq!(lru.get("a"), Some(1));
        assert_eq!(lru.get("c"), Some(3));
        assert_eq!(lru.bytes, 80);
    }

    #[test]
    fn print_lru_tracks_per_window_evictions_and_forget() {
        let mut lru: PrintLru<u32, u32> = PrintLru::new(100);
        assert!(lru.insert("a".into(), 1, 40, Some(10)).is_empty());
        assert!(lru.insert("b".into(), 2, 40, Some(20)).is_empty());
        // Window 20 also uses "a", moving "a" ahead of "b" and recording both windows.
        assert_eq!(lru.get("a", Some(20)), Some(1));
        // Evicting "b" only reports window 20.
        let evicted = lru.insert("c".into(), 3, 40, Some(30));
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0].0, 2);
        assert!(evicted[0].1.contains(&20) && !evicted[0].1.contains(&10));
        // Forgetting window 10 strips it from "a"'s usage set before "a" is evicted.
        lru.forget_window(10);
        let evicted2 = lru.insert("d".into(), 4, 40, Some(30));
        assert_eq!(evicted2.len(), 1);
        assert_eq!(evicted2[0].0, 1);
        assert!(evicted2[0].1.contains(&20) && !evicted2[0].1.contains(&10));
    }
}
