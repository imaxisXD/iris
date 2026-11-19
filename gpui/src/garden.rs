//! The garden (`GardenView`): the flower telling the model's story, its sun, its cloud and
//! rain, its butterflies. Drawn for one moment `t`, every arrival long settled (as the Swift
//! snapshots draw it), in a 190-point square.
//!
//! The sprites and clouds are the Rune copy's. The scene turns things (the sun, the flower's
//! sway, each butterfly to its heading, the rain's slant), which GPUI can't do to a box, so the
//! whole garden is printed as one picture a frame on the CPU (about a millisecond), and kept
//! when it holds still.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tiny_skia::{FillRule, Mask, Paint, Path, PathBuilder, Pixmap, Transform};

use crate::icons::{self, Draw, ellipse};
use crate::raster::Sheet;
use crate::riso::{self, Img as Pixels, Plate, Rgb8, ink, print_icon};

pub const SIZE: f32 = 190.0;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Stage {
    Bud,
    Allowed,
    Watering,
    Warming,
    Ready,
}

impl Stage {
    fn butterflies(self) -> usize {
        match self {
            Stage::Bud => 0,
            Stage::Allowed | Stage::Watering | Stage::Warming => 1,
            Stage::Ready => 2,
        }
    }
}

// MARK: - Sprites

const ORANGE: Rgb8 = (255, 116, 60);
const SUNFLOWER: Rgb8 = (255, 190, 40);

/// Raw pictures kept by name: (pixels square, straight RGBA).
pub type Raw = Rc<(u32, Vec<u8>)>;

pub fn raw(key: &str, pixels: u32, make: impl FnOnce() -> Vec<u8>) -> Raw {
    thread_local! { static RAW: RefCell<HashMap<String, Raw>> = RefCell::new(HashMap::new()); }
    if let Some(r) = RAW.with(|m| m.borrow().get(key).cloned()) {
        return r;
    }
    let r = Rc::new((pixels, make()));
    RAW.with(|m| m.borrow_mut().insert(key.to_string(), r.clone()));
    r
}

fn sprite_raw(key: &str, pixels: u32, seed: i64, colour: Rgb8, shade: fn(&mut Draw), pink: fn(&mut Draw), inked: fn(&mut Draw)) -> Raw {
    raw(&format!("sprite-{key}"), pixels, || raw_sprite(pixels, seed, colour, shade, pink, inked))
}

fn shown(r: &Raw, key: &str) -> Pixels {
    let plate = Plate { w: r.0 as f32, h: r.0 as f32, scale: 1.0 };
    riso::cached(format!("shown-{key}"), &plate, || r.1.clone())
}

fn raw_sprite(pixels: u32, seed: i64, colour: Rgb8, shade: fn(&mut Draw), pink: fn(&mut Draw), inked: fn(&mut Draw)) -> Vec<u8> {
    icons::print(pixels, seed, &[(ink::CREMA, 1, 1, shade), (colour, -1, -1, pink), (ink::ESPRESSO, 0, 0, inked)])
}

fn wing(points: &[(f32, f32); 5], mirror: bool) -> Path {
    let p: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (if mirror { 80.0 - x } else { x }, y)).collect();
    let mut b = PathBuilder::new();
    b.move_to(p[0].0, p[0].1);
    b.quad_to(p[1].0, p[1].1, p[2].0, p[2].1);
    b.quad_to(p[3].0, p[3].1, p[4].0, p[4].1);
    b.close();
    b.finish().unwrap()
}
const FORE: [(f32, f32); 5] = [(38.0, 30.0), (14.0, 4.0), (4.0, 20.0), (10.0, 42.0), (38.0, 40.0)];
const HIND: [(f32, f32); 5] = [(38.0, 42.0), (12.0, 46.0), (18.0, 68.0), (30.0, 70.0), (38.0, 52.0)];
fn wings() -> [(Path, u8); 4] {
    [(wing(&FORE, false), 45), (wing(&FORE, true), 45), (wing(&HIND, false), 175), (wing(&HIND, true), 175)]
}

/// Seen from above, wings open: pointed orange forewings, pale hindwings, an ink body.
pub fn butterfly() -> Pixels {
    shown(&butterfly_raw(), "butterfly")
}

pub fn butterfly_raw() -> Raw {
    sprite_raw("butterfly", 36, 21, ORANGE,
        |d| for (w, _) in wings() { d.fill(&w, 160) },
        |d| for (w, g) in wings() { d.fill(&w, g) },
        |d| {
            d.pen(3.2);
            for (w, _) in wings() { d.stroke(&w) }
            d.pen(6.5);
            d.stroke(&icons::line(40.0, 26.0, 40.0, 60.0));
            d.pen(2.6);
            let mut b = PathBuilder::new();
            b.move_to(39.0, 24.0);
            b.quad_to(37.0, 12.0, 31.0, 9.0);
            d.stroke(&b.finish().unwrap());
            let mut b = PathBuilder::new();
            b.move_to(41.0, 24.0);
            b.quad_to(43.0, 12.0, 49.0, 9.0);
            d.stroke(&b.finish().unwrap());
        })
}

/// A sun with short rays, sunflower yellow so it never reads as another bloom.
pub fn sun_raw() -> Raw {
    raw("sprite-sun", 44, sun_rgba)
}

/// The sun's 44-pixel print, straight alpha.
pub fn sun_rgba() -> Vec<u8> {
    raw_sprite(44, 25, SUNFLOWER,
        |d| d.fill(&ellipse(41.0, 41.0, 34.0, 34.0), 160),
        |d| d.fill(&ellipse(40.0, 40.0, 32.0, 32.0), 45),
        |d| {
            d.pen(3.4);
            d.stroke(&ellipse(40.0, 40.0, 32.0, 32.0));
            for k in 0..8 {
                let a = k as f32 / 8.0 * 2.0 * std::f32::consts::PI;
                d.stroke(&icons::line(40.0 + 22.0 * a.cos(), 40.0 + 22.0 * a.sin(), 40.0 + 32.0 * a.cos(), 40.0 + 32.0 * a.sin()));
            }
        })
}

#[derive(Clone, Copy)]
enum CloudShape {
    Puffy,
    Long,
    Round,
    Wisp,
}

fn rounded_base(x0: f32, x1: f32, y: f32, h: f32) -> Path {
    icons::rounded(x0, y, x1 - x0, h, h / 2.0)
}

/// The parts of a cloud, whose union is its shape.
fn cloud_parts(kind: CloudShape) -> Vec<Path> {
    match kind {
        CloudShape::Puffy => vec![ellipse(24.0, 46.0, 28.0, 26.0), ellipse(42.0, 36.0, 36.0, 34.0), ellipse(58.0, 46.0, 26.0, 24.0), rounded_base(12.0, 68.0, 44.0, 14.0)],
        CloudShape::Long => vec![ellipse(14.0, 50.0, 18.0, 16.0), ellipse(30.0, 44.0, 24.0, 24.0), ellipse(48.0, 41.0, 26.0, 26.0), ellipse(66.0, 48.0, 20.0, 18.0), rounded_base(5.0, 76.0, 47.0, 11.0)],
        CloudShape::Round => vec![ellipse(34.0, 42.0, 40.0, 38.0), ellipse(56.0, 50.0, 24.0, 20.0), rounded_base(16.0, 66.0, 48.0, 12.0)],
        CloudShape::Wisp => vec![ellipse(40.0, 50.0, 70.0, 14.0), ellipse(30.0, 45.0, 26.0, 14.0), ellipse(52.0, 44.0, 22.0, 13.0)],
    }
}

/// Grey plates for a cloud at `pixels`: (fill of the union at `gray`, the union's outline in the pen).
/// Core Graphics unions the parts; here the parts are filled together, and the pen draws each
/// part's edge only where it's on the union's edge.
fn cloud_plates(kind: CloudShape, pixels: u32, fill_gray: u8, pen: f32) -> (Pixmap, Pixmap) {
    let s = pixels as f32 / 80.0;
    let t = Transform::from_scale(s, s);
    let parts = cloud_parts(kind);
    let mut fill = Pixmap::new(pixels, pixels).unwrap();
    fill.fill(tiny_skia::Color::WHITE);
    let mut inside = Mask::new(pixels, pixels).unwrap();
    for p in &parts {
        let mut paint = Paint::default();
        paint.set_color_rgba8(fill_gray, fill_gray, fill_gray, 255);
        paint.anti_alias = true;
        fill.fill_path(p, &paint, FillRule::Winding, t, None);
        inside.fill_path(p, FillRule::Winding, true, t);
    }
    // The union shrunk by half the pen: the pen's line is kept only outside it.
    let mut inner = Mask::new(pixels, pixels).unwrap();
    let shrink = pen * s / 2.0;
    for p in &parts {
        if let Some(stroked) = p.stroke(&tiny_skia::Stroke { width: 0.01, ..Default::default() }, 1.0) {
            let _ = stroked;
        }
        inner.fill_path(p, FillRule::Winding, true, t);
    }
    let (w, h) = (pixels as i32, pixels as i32);
    let src = inside.data().to_vec();
    let r = shrink.round().max(1.0) as i32;
    let eroded: Vec<u8> = (0..h).flat_map(|y| {
        let src = &src;
        (0..w).map(move |x| {
            let mut m = 255u8;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy > r * r { continue }
                    let (xx, yy) = (x + dx, y + dy);
                    let v = if xx < 0 || yy < 0 || xx >= w || yy >= h { 0 } else { src[(yy * w + xx) as usize] };
                    m = m.min(v);
                }
            }
            m
        })
    }).collect();
    let mut line = Pixmap::new(pixels, pixels).unwrap();
    line.fill(tiny_skia::Color::WHITE);
    let mut lines = Pixmap::new(pixels, pixels).unwrap();
    {
        let mut d = PenOnly(&mut lines, t, pen);
        for p in &parts { d.stroke(p) }
    }
    for (i, (dst, src)) in line.pixels_mut().iter_mut().zip(lines.pixels()).enumerate() {
        let a = src.alpha() as f32 / 255.0 * (1.0 - eroded[i] as f32 / 255.0);
        let g = (255.0 - (255.0 - 25.0) * a).round() as u8;
        *dst = tiny_skia::ColorU8::from_rgba(g, g, g, 255).premultiply();
    }
    let _ = inner;
    (fill, line)
}

struct PenOnly<'a>(&'a mut Pixmap, Transform, f32);
impl PenOnly<'_> {
    fn stroke(&mut self, p: &Path) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 25, 255);
        paint.anti_alias = true;
        let st = tiny_skia::Stroke { width: self.2, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        self.0.stroke_path(p, &paint, &st, self.1, None);
    }
}

fn copy(src: &Pixmap) -> impl Fn(&mut Pixmap, Transform) + '_ {
    move |pm: &mut Pixmap, _| pm.draw_pixmap(0, 0, src.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None)
}

/// The download's cloud: puffy, crema with an ink edge.
pub fn cloud() -> Raw {
    raw("sprite-cloud", 44, || {
        let (shade, _) = cloud_plates(CloudShape::Puffy, 44, 150, 3.4);
        let (pink, line) = cloud_plates(CloudShape::Puffy, 44, 215, 3.4);
        let (a, b, c) = (copy(&shade), copy(&pink), copy(&line));
        print_icon(44, 23, &[(ink::CREMA, 1, 1, &a), (ink::PINK, -1, -1, &b), (ink::ESPRESSO, 0, 0, &c)])
    })
}

/// An update's sky cloud: its blue a pixel up-left of the pen edge (`skyCloud`).
fn sky_cloud(kind: CloudShape, fill: Rgb8, density: u8, edge: Option<Rgb8>, seed: i64) -> Raw {
    raw(&format!("sprite-sky-{seed}"), 44, || sky_cloud_rgba(kind, fill, density, edge, seed))
}

fn sky_cloud_rgba(kind: CloudShape, fill: Rgb8, density: u8, edge: Option<Rgb8>, seed: i64) -> Vec<u8> {
    let (f, line) = cloud_plates(kind, 44, density, 3.4);
    let (a, b) = (copy(&f), copy(&line));
    print_icon(44, seed, &[(fill, -1, -1, &a), (edge.unwrap_or(ink::ESPRESSO), 0, 0, &b)])
}

/// The update letter's pale cloud (`UpdateCheckScene.cloud`), straight alpha.
pub fn mist_cloud_rgba() -> Vec<u8> {
    sky_cloud_rgba(CloudShape::Puffy, (204, 222, 244), 80, None, 57)
}

// MARK: - The scene

struct Cloud {
    image: fn() -> Raw,
    x0: f64,
    y: f64,
    width: f64,
    drift: f64,
    speed: f64,
    bob: f64,
    phase: f64,
    rains: bool,
}

impl Cloud {
    fn x(&self, t: f64) -> f64 {
        self.x0 + (t * self.speed).sin() * self.drift + (t * 0.9 + self.phase).sin() * self.bob
    }
}

const SKY_BLUE: Rgb8 = (120, 162, 220);
fn single() -> Cloud {
    Cloud { image: cloud, x0: 98.0, y: 24.0, width: 50.0, drift: 6.0, speed: 0.8, bob: 0.0, phase: 0.0, rains: true }
}
fn sky() -> Vec<Cloud> {
    let c = |image: fn() -> Raw, x0, y, width, phase, rains| Cloud { image, x0, y, width, drift: 8.0, speed: 0.22, bob: 1.5, phase, rains };
    vec![
        c(|| sky_cloud(CloudShape::Wisp, (204, 222, 244), 80, Some((150, 186, 232)), 51), 140.0, 14.0, 34.0, 0.4, false),
        c(|| sky_cloud(CloudShape::Long, (176, 204, 236), 75, None, 52), 100.0, 26.0, 76.0, 1.7, true),
        c(|| sky_cloud(CloudShape::Puffy, SKY_BLUE, 80, None, 53), 74.0, 36.0, 44.0, 3.1, true),
        c(|| sky_cloud(CloudShape::Round, (98, 138, 204), 80, None, 54), 120.0, 38.0, 40.0, 4.6, true),
    ]
}

fn hash(i: i64, n: i64) -> f64 {
    let v = (i as f64 * 12.9898 + n as f64 * 78.233).sin() * 43758.5453;
    v - v.floor()
}
fn ease(x: f64) -> f64 {
    1.0 - (1.0 - x).powi(3)
}

/// How far round the garden its picture reaches (clouds and butterflies stray past the square).
pub const PAD: f32 = 24.0;

/// A raw picture shown `w`×`h` points centred on (x, y), turned `degrees` about its middle.
fn put(sh: &mut Sheet, base: Transform, r: &Raw, x: f64, y: f64, w: f64, h: f64, degrees: f32, opacity: f32) {
    let local = base.pre_translate(x as f32, y as f32).pre_rotate(degrees).pre_translate(-(w as f32) / 2.0, -(h as f32) / 2.0);
    sh.sprite_at(&r.1, r.0, r.0, w as f32, h as f32, local, opacity);
}

/// The garden at moment `t`: `stage`, `progress` (how hard it rains), `growth` (an update building),
/// printed `zoom` times its size, `PAD` round it (in garden points). Straight RGBA and its size
/// in pixels.
pub fn garden_rgba(stage: Stage, progress: f64, growth: Option<f64>, t: f64, zoom: f32, scale: f32) -> (u32, u32, Vec<u8>) {
    let side = (SIZE + 2.0 * PAD) * zoom;
    let plate = Plate { w: side, h: side, scale };
    let mut sh = Sheet::new(plate, 0.0, 0.0, 0.0);
    let base = Transform::from_scale(zoom, zoom).pre_translate(PAD, PAD);
    let stage = if growth.is_some() { Stage::Watering } else { stage };

    // The sun, behind everything, once the model is loading.
    if stage >= Stage::Warming {
        put(&mut sh, base, &sun_raw(), 40.0, 66.0, 58.0, 58.0, ((t * 9.0) % 360.0) as f32, 1.0);
    }

    // The flower: open from `allowed` on, swaying from its root.
    let mut size = 1.0;
    let mut open = stage >= Stage::Allowed;
    if let Some(gr) = growth {
        let gr = gr.clamp(0.0, 1.0);
        size = 0.36 + 0.64 * ease(gr);
        open = (gr - 0.35) / 0.4 >= 0.5;
    }
    let sway = (t * 1.3).sin() * 2.2;
    let art = flower_raw(open, scale * zoom);
    // The 128-point box centred on (95, 124): swayed about its foot, grown about (0.5, 0.92).
    let local = base.pre_translate(31.0, 60.0)
        .pre_concat(Transform::from_rotate_at(sway as f32, 64.0, 128.0))
        .pre_translate(64.0, 117.76).pre_scale(size as f32, size as f32).pre_translate(-64.0, -117.76);
    sh.picture_at(&art.1, art.0, art.0, 128.0, 128.0, local, 1.0);

    // Watering: a cloud (or an update's rainy sky) and its rain.
    if stage == Stage::Watering {
        let clouds = if growth.is_none() { vec![single()] } else { sky() };
        let flower_top = 178.0 - 99.0 * size;
        for (index, cloud) in clouds.iter().enumerate() {
            if cloud.rains {
                rain(&mut sh, base, t, cloud, index as i64 * 50, flower_top, progress);
            }
            put(&mut sh, base, &(cloud.image)(), cloud.x(t), cloud.y, cloud.width, cloud.width, 0.0, 1.0);
        }
    }

    // Butterflies, each on its own loop round the flower head.
    if growth.is_none() {
        for i in 0..stage.butterflies() {
            let l = LOOPS[i % LOOPS.len()];
            let here = loop_point(l, t);
            let next = loop_point(l, t + 0.05);
            let heading = (next.1 - here.1).atan2(next.0 - here.0) + std::f64::consts::FRAC_PI_2;
            let flap = 0.28 + 0.72 * (t * 15.0 + l.3).sin().abs();
            wing_sprite(&mut sh, base, 32.0, flap, heading, here);
        }
    } else if let Some(gr) = growth {
        // Growing: small ones arrive in a bunch, each on a loop of its own.
        let swarm = ((gr - 0.3) / 0.7 * 7.0).floor().clamp(0.0, 6.0) as i64;
        let head_y = 186.0 - 108.0 * size;
        for i in 0..swarm {
            let (rx, ry) = (26.0 + hash(i, 101) * 44.0, 10.0 + hash(i, 102) * 18.0);
            let speed = (0.7 + hash(i, 103) * 0.7) * if hash(i, 104) < 0.5 { -1.0 } else { 1.0 };
            let phase = hash(i, 105) * 6.3;
            let s = 15.0 + hash(i, 106) * 6.0;
            let point = |t: f64| {
                let a = t * speed + phase;
                (95.0 + rx * a.sin(), head_y - 6.0 + ry * (2.0 * a).sin() * 0.6 - 6.0 * (a * 1.7).sin())
            };
            let (here, next) = (point(t), point(t + 0.05));
            let heading = (next.1 - here.1).atan2(next.0 - here.0) + std::f64::consts::FRAC_PI_2;
            let flap = 0.28 + 0.72 * (t * 17.0 + phase).sin().abs();
            wing_sprite(&mut sh, base, s as f32, flap, heading, here);
        }
    }
    let (pw, ph) = plate.px();
    (pw, ph, sh.rgba())
}

/// The garden as a picture (`side` points square, `PAD` round it at `zoom`). A garden held at
/// one moment (snapshots, or reduced motion) is printed once and kept.
pub fn garden(stage: Stage, progress: f64, growth: Option<f64>, t: f64, zoom: f32, scale: f32) -> (Pixels, f32) {
    let side = (SIZE + 2.0 * PAD) * zoom;
    let (pw, ph, rgba) = if t == 1000.0 {
        let key = format!("garden-{stage:?}-{progress}-{growth:?}-{zoom}");
        let plate = Plate { w: side, h: side, scale };
        return (riso::cached(key, &plate, || garden_rgba(stage, progress, growth, t, zoom, scale).2), side);
    } else {
        garden_rgba(stage, progress, growth, t, zoom, scale)
    };
    (riso::frame_image(pw, ph, rgba), side)
}

/// A butterfly `s` points, flapping (squeezed across), turned to its heading, centred on `at`.
fn wing_sprite(sh: &mut Sheet, base: Transform, s: f32, flap: f64, heading: f64, at: (f64, f64)) {
    let b = butterfly_raw();
    let local = base.pre_translate(at.0 as f32, at.1 as f32).pre_rotate(heading.to_degrees() as f32)
        .pre_translate(-s / 2.0, -s / 2.0).pre_translate(s * (1.0 - flap as f32) / 2.0, 0.0);
    sh.sprite_at(&b.1, b.0, b.0, s * flap as f32, s, local, 1.0);
}

#[derive(Clone, Copy)]
struct Loop(f64, f64, f64, f64);
const LOOPS: [Loop; 4] = [Loop(66.0, 24.0, 0.55, 0.0), Loop(58.0, 30.0, 0.72, 2.2), Loop(72.0, 20.0, 0.46, 4.1), Loop(50.0, 26.0, 0.84, 1.1)];
fn loop_point(l: Loop, t: f64) -> (f64, f64) {
    let a = t * l.2 + l.3;
    (95.0 + l.0 * a.sin(), 58.0 - 8.0 + l.1 * (2.0 * a).sin() * 0.6 - 8.0 * (a * 1.3).sin())
}

/// Rain from under a cloud: each drop its own pace, a new spot under the cloud each fall,
/// slanting a touch, splashing where it lands.
fn rain(sh: &mut Sheet, base: Transform, t: f64, cloud: &Cloud, salt: i64, flower_top: f64, progress: f64) {
    let drops = ((5 + (progress * 7.0) as i64) as f64 * cloud.width / 50.0) as i64;
    let top = cloud.y + cloud.width * 0.2;
    for j in 0..drops {
        let i = j + salt;
        let speed = 0.75 + 0.6 * hash(i, -1);
        let cycle = t * speed + hash(i, -2) * 7.0;
        let n = cycle.floor();
        let fall = cycle - n;
        let start = cloud.x(t - fall / speed);
        let x0 = start + (hash(i, n as i64) - 0.5) * cloud.width * 0.6;
        let landing = if (x0 - 95.0).abs() < 26.0 * ((178.0 - flower_top) / 99.0).max(0.5) { flower_top } else { 182.0 };
        let length = (landing - top).max(10.0);
        let x = x0 + fall * 3.0;
        let tall = 4.0 + hash(i, n as i64 + 9) * 3.0;
        if fall < 0.9 {
            let drop = icons::rounded(-0.8, -tall as f32 / 2.0, 1.6, tall as f32, 0.8);
            let local = base.pre_translate(x as f32, (top + fall * length) as f32).pre_rotate(-6.0);
            sh.fill_at(&drop, local, ink::ESPRESSO, (0.75 * (fall * 8.0).min(1.0)) as f32);
        } else {
            let k = (fall - 0.9) / 0.1;
            for side in [-1.0, 1.0] {
                let speck = icons::rounded(-1.0, -0.7, 2.0, 1.4, 0.7);
                let local = base.pre_translate((x + side * (1.0 + k * 4.0)) as f32, (top + 0.9 * length - k * 2.0) as f32);
                sh.fill_at(&speck, local, ink::ESPRESSO, (0.6 * (1.0 - k)) as f32);
            }
        }
    }
}

/// The flower and bud for the gardens (`Sprites.flower`, `Sprites.bud`): the rig at rest,
/// 128 points, standing on its ground, at `scale` pixels a point.
pub fn flower_raw(open: bool, scale: f32) -> Raw {
    raw(&format!("flower-{open}-{scale}"), (128.0 * scale).round() as u32, || crate::rig::flower(128.0, crate::rig::Crop::Whole, open, scale).2)
}

/// The same, as a picture.
pub fn flower_art(open: bool, scale: f32) -> Pixels {
    let r = flower_raw(open, scale);
    shown(&r, &format!("flower-{open}-{scale}"))
}
