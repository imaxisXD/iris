//! The menu bar slip (`StatusMenuView`): a hand-cut note hanging from the menu bar flower.
//! The header is the status: a printed sky band with the status as a ghost word in paper, the
//! wordmark, and the garden standing on its edge. Under it, only what the band can't say. Then
//! the commands, with their riso icons and keycaps.
//!
//! The Rune copy's slip in GPUI. The garden is printed whole each frame (`garden.rs`); the
//! paper, its shadow and its tooth are printed for whatever size the slip comes out, at paint
//! time.

use gpui::{AnyElement, Div, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement, Styled, canvas, div};
use skrifa::outline::DrawSettings;
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap};

use crate::garden::{self, Stage};
use crate::icons::Icon;
use crate::letters::{self, LONDRINA};
use crate::parts::*;
use crate::pen::{self, Pen};
use crate::riso::{self, Img as Pixels, Plate, Rgb8, Rng, col, hue, ink, over};

pub const WIDTH: f32 = 286.0;
const TAIL: f32 = 11.0;
const BAND_H: f32 = 76.0;
const GARDEN_SCALE: f32 = 0.56;
const PAPER_OPACITY: f32 = 0.9;

thread_local! {
    /// The live slip's paper is solid. Swift's live slip lays its 90% paper over the system's
    /// frosted glass (a blur of the desktop, clipped to the cut shape), which GPUI can't do: it
    /// blurs only a whole window's rectangle. Solid paper reads as the frosted one does, and
    /// never shows the words behind it. Snapshots keep the 90% the Swift snapshots draw.
    static SOLID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_solid_paper(solid: bool) {
    SOLID.with(|s| s.set(solid));
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Engine {
    Idle,
    Downloading(f64),
    /// Waking up: the model loading into memory.
    Loading,
    Ready,
}

impl Engine {
    fn title(self) -> &'static str {
        match self {
            Engine::Idle => "Resting",
            Engine::Downloading(_) => "Downloading",
            Engine::Loading => "Waking up",
            Engine::Ready => "Ready",
        }
    }
    fn stage(self) -> Stage {
        match self {
            Engine::Idle => Stage::Bud,
            Engine::Downloading(_) => Stage::Watering,
            Engine::Loading => Stage::Warming,
            Engine::Ready => Stage::Ready,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Command {
    pub id: &'static str,
    pub title: &'static str,
    pub keys: Vec<&'static str>,
    pub icon: Option<Icon>,
}

/// What the slip shows (`StatusMenu.Model`).
#[derive(Clone, Debug)]
pub struct Slip {
    pub engine: Engine,
    pub groups: Vec<Vec<Command>>,
    pub highlighted: Option<&'static str>,
    pub busy: Vec<&'static str>,
    pub setup_line: Option<String>,
    /// While setting up, the garden tells setup's story (`setupGarden`), never ahead of it.
    pub setup_garden: Option<Stage>,
    pub problem_line: Option<String>,
    pub updating: Option<f64>,
    /// A page the slip has turned to (Updates).
    pub page: Option<crate::page::Page>,
}

pub const OPEN_KEYS: [&str; 3] = ["⌃", "⌥", "G"];
pub const QUICK_KEYS: [&str; 4] = ["⌃", "⌥", "⇧", "G"];

thread_local! {
    /// The shortcuts as Settings has them, once the app has said (the snapshots keep ⌃⌥G).
    static KEYS: std::cell::RefCell<Option<(Vec<&'static str>, Option<Vec<&'static str>>)>> = const { std::cell::RefCell::new(None) };
}

/// Tells the slip which keys the shortcuts are (each change of shortcut leaks a few bytes of
/// key names, which is nothing).
pub fn set_keys(open: Vec<String>, quick: Option<Vec<String>>) {
    let leak = |v: Vec<String>| v.into_iter().map(|s| &*Box::leak(s.into_boxed_str())).collect::<Vec<&'static str>>();
    KEYS.with(|k| *k.borrow_mut() = Some((leak(open), quick.map(leak))));
}

/// The keys the app has told (none: never told, as in a snapshot).
pub fn told_keys() -> Option<(Vec<&'static str>, Option<Vec<&'static str>>)> {
    KEYS.with(|k| k.borrow().clone())
}

fn open_keys() -> Vec<&'static str> {
    KEYS.with(|k| k.borrow().as_ref().map(|k| k.0.clone())).unwrap_or_else(|| OPEN_KEYS.to_vec())
}

fn quick_keys() -> Option<Vec<&'static str>> {
    KEYS.with(|k| k.borrow().as_ref().map(|k| k.1.clone())).unwrap_or_else(|| Some(QUICK_KEYS.to_vec()))
}

/// `SlipRows.build`: the rows for where Iris stands.
pub fn rows(shortcut_taken: bool, setup_complete: bool, model_ready: bool) -> Vec<Vec<Command>> {
    let c = |id, title, keys: &[&'static str], icon| Command { id, title, keys: keys.to_vec(), icon };
    let settings = c("settings", "Settings…", &["⌘", ","], Some(Icon::General));
    let update = c("update", "Check for Updates…", &[], Some(Icon::Updates));
    let quit = vec![c("quit", "Quit Iris", &["⌘", "Q"], None)];
    if !setup_complete {
        return vec![vec![c("setup", "Finish setup…", &[], Some(Icon::Tray))], vec![settings, update], quit];
    }
    let mut check = vec![c("check", "Check text", &open_keys(), Some(Icon::Tones))];
    if shortcut_taken {
        check.insert(0, c("shortcut", "Change shortcut…", &[], Some(Icon::Shortcuts)));
    }
    check.push(c("quick", "Quick Fix", &quick_keys().unwrap_or_default(), Some(Icon::Wand)));
    let model = c("model", if model_ready { "Free model memory" } else { "Load model now" }, &[], Some(Icon::Model));
    vec![check, vec![model], vec![c("setup", "Setup…", &[], Some(Icon::Tray)), settings, update], quit]
}

pub fn shortcut_taken_line() -> String {
    format!("Another app uses {}, so it does nothing in Iris. Change it here.", open_keys().concat())
}

impl Slip {
    fn status_title(&self) -> &'static str {
        if self.updating.is_some() {
            "Updating"
        } else if self.setup_line.is_some() {
            "Setting up"
        } else if self.problem_line.is_some() {
            "Something's off"
        } else {
            self.engine.title()
        }
    }
    fn status_detail(&self) -> Option<&str> {
        if self.updating.is_some() {
            return None;
        }
        self.setup_line.as_deref().or(self.problem_line.as_deref())
    }
}

/// What the slip's rows do when it's live: the row under the pointer, and a row (or Back) pressed.
pub struct Acts<'a> {
    pub hover: &'a dyn Fn(&'static str) -> Box<dyn Fn(&bool, &mut gpui::Window, &mut gpui::App)>,
    pub press: &'a dyn Fn(&'static str) -> Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>,
}

/// The slip at moment `t` (the Swift snapshots draw t = 1000).
pub fn slip(s: &Slip, t: f64, scale: f32) -> Div {
    slip_live(s, t, None, scale)
}

/// The slip, its rows answering the pointer and clicks with `acts`.
pub fn slip_live(s: &Slip, t: f64, acts: Option<&Acts>, scale: f32) -> Div {
    let rule = || div().pl(z(10.0)).child(pen_rule(5, WIDTH - 20.0, over(hue::I12, 0.85, ink::PAPER), 1.2));
    let mut body = div().flex().flex_col().w(z(WIDTH)).pt(z(12.0 + TAIL)).pb(z(8.0));
    if let Some(page) = &s.page {
        let head = crate::page::header(scale);
        let head = match acts {
            Some(a) => div().id("back").role(Role::Button).aria_label("Back").on_click((a.press)("back")).child(head).into_any_element(),
            None => head.into_any_element(),
        };
        body = body.child(head).child(rule()).child(crate::page::view_live(page, acts.map(|a| a.press), scale));
        return paper(body, scale);
    }
    body = body.child(header(s, t, scale)).child(rule());
    let icon_room = s.groups.iter().flatten().any(|c| c.icon.is_some());
    for (index, group) in s.groups.iter().enumerate() {
        if index > 0 {
            body = body.child(div().pl(z(14.0)).py(z(3.0)).child(pen_rule(40 + index as i64, WIDTH - 28.0, over(hue::I12, 0.3, ink::PAPER), 1.0)));
        }
        let mut g = div().flex().flex_col().px(z(6.0));
        for command in group {
            let r = slip_row(s, command, icon_room, scale);
            let busy = s.busy.contains(&command.id);
            let r = div().id(command.id).role(Role::MenuItem).aria_label(command.title).child(r);
            g = g.child(match acts {
                Some(a) if !busy => r.on_hover((a.hover)(command.id)).on_click((a.press)(command.id)).into_any_element(),
                _ => r.into_any_element(),
            });
        }
        body = body.child(g);
    }
    paper(body, scale)
}

/// The paper, its cut edge and shadow behind; its tooth over everything, printed at paint time
/// for the size the slip came out.
fn paper(body: Div, scale: f32) -> Div {
    let k = zoom();
    let back = canvas(|_, _, _| (), move |b, _, window, _| {
        let (w, h) = ((f32::from(b.size.width) / k).round(), (f32::from(b.size.height) / k).round());
        let (img, pad) = note_paper(w, h, scale);
        let p = gpui::px(pad * k);
        let at = gpui::Bounds { origin: gpui::point(b.origin.x - p, b.origin.y - p), size: gpui::size(b.size.width + p * 2.0, b.size.height + p * 2.0) };
        let _ = window.paint_image(at, at, Default::default(), img, 0, false);
    }).absolute().top_0().left_0().size_full();
    let tooth = canvas(|_, _, _| (), move |b, _, window, _| {
        let (w, h) = ((f32::from(b.size.width) / k).round(), (f32::from(b.size.height) / k).round());
        let _ = window.paint_image(b, b, Default::default(), note_grain(w, h, scale), 0, false);
    }).absolute().top_0().left_0().size_full();
    div().relative().w(z(WIDTH)).child(back).child(body).child(tooth)
}

fn slip_row(s: &Slip, c: &Command, icon_room: bool, scale: f32) -> AnyElement {
    let busy = s.busy.contains(&c.id);
    let lit = s.highlighted == Some(c.id) && !busy;
    let w = WIDTH - 12.0;
    let mut r = div().flex().flex_row().w(z(w)).h(z(32.0)).px(z(10.0)).gap(z(8.0)).items_center();
    if let Some(icon) = c.icon {
        r = r.child(printed(22.0, 22.0, &icon.image_at(22.0, scale)));
    } else if icon_room {
        r = r.child(sized(22.0, 22.0));
    }
    r = r.child(t(c.title, body(), if lit { hue::I12 } else { hue::I11 })).child(div().flex_grow_1().min_w(z(8.0)));
    if !c.keys.is_empty() {
        r = r.child(c.keys.iter().fold(div().flex().flex_row().gap(z(3.0)), |k, key| k.child(keycap(key, scale))));
    }
    let mut e = sized(w, 32.0);
    if lit {
        e = e.child(at(hand_fill(c.id.len() as i64 * 7, 0.6, w, 32.0, col(hue::I3)), 0.0, 0.0));
    }
    e = e.child(at(r, 0.0, 0.0));
    if busy { e.opacity(0.4).into_any_element() } else { e.into_any_element() }
}

// MARK: - Header

fn header(s: &Slip, t: f64, scale: f32) -> Div {
    let band_w = WIDTH - 20.0;
    let (logo, _, logo_h) = letters::iris_logo(22.0, scale);
    let stage = if s.updating.is_some() { Stage::Watering } else { s.setup_garden.unwrap_or(s.engine.stage()) };
    let progress = s.updating.unwrap_or(match s.engine { Engine::Downloading(f) => f, _ => 0.0 });
    // The garden at 56%, its bottom right on the band's edge, spilling 16 over.
    let (g, side) = garden::garden(stage, progress, s.updating, t, GARDEN_SCALE * zoom(), scale);
    let side = side / zoom();
    let pad = garden::PAD * GARDEN_SCALE;
    let band = sized(band_w, BAND_H + 12.0)
        .child(at(div().id("status").role(Role::Label).aria_label(s.status_title()).child(printed(band_w, BAND_H, &status_band(s.status_title(), band_w, t, scale))), 0.0, 0.0))
        .child(at(logo, 12.0, BAND_H - 12.0 - logo_h))
        .child(at(printed(side, side, &g), band_w - garden::SIZE * GARDEN_SCALE - pad, BAND_H + 16.0 - garden::SIZE * GARDEN_SCALE - pad));
    let mut h = div().flex().flex_col().pl(z(10.0)).pr(z(10.0)).pb(z(12.0)).gap(z(8.0)).child(band);
    if let Some(detail) = s.status_detail() {
        h = h.child(div().pl(z(6.0)).pr(z(6.0)).child(crate::page::para(detail, caption(), hue::I10, band_w - 12.0)));
    }
    let fraction = s.updating.or(match s.engine { Engine::Downloading(f) => Some(f), _ => None });
    if let Some(f) = fraction {
        h = h.child(div().pl(z(6.0)).child(progress_track(f, band_w - 12.0, scale)));
    }
    h
}

/// `ProgressTrack`: a pink marker swipe across a faint pen line, the share in brush at its tip.
pub fn progress_track(f: f64, width: f32, scale: f32) -> Div {
    let f = f.clamp(0.0, 1.0) as f32;
    let w = ((width - 44.0) * f).max(16.0);
    // SwiftUI centres the line and the swipe in the box the brush figure makes (21.5 points,
    // its line rounded to a pixel), though the track keeps 18 of layout.
    let mid = (17.0 * BRUSH_LINE * scale).ceil() / scale / 2.0;
    sized(width, 18.0)
        .child(at(pen_rule(91, width, over(hue::I12, 0.18, ink::PAPER), 1.0), 0.0, mid - 1.5))
        .child(at(div().opacity(0.45).child(hand_printed(92, 0.7, w, 12.0, ink::PINK, 92, scale)), 0.0, mid - 6.0))
        .child(at(t(format!("{}%", (f * 100.0) as i32), brush(17.0), ink::PINK_TEXT), w + 6.0, mid - line_box(17.0, BRUSH_LINE) / 2.0))
}

/// `StatusBand`: the sky plate, the status as a ghost word in paper along its bottom edge (a
/// ticker when it's too long, caught at moment `t`), cut by hand.
fn status_band(word: &str, w: f32, t: f64, scale: f32) -> Pixels {
    let word = word.to_uppercase();
    let plate = Plate { w, h: BAND_H, scale };
    // Only a ticker moves with the clock; a word that fits is printed once.
    let ticker = advance(&FontRef::new(LONDRINA).unwrap(), &word, 72.0) > w - 8.0;
    let key = if ticker { format!("band-{word}-{w}-{t:.3}") } else { format!("band-{word}-{w}") };
    let make = || {
        let clip = plate.fill(&pen::hand_rect(21, 0.5, 0.0, true, 0.0, 0.0, w, BAND_H).skia().unwrap());
        // `ScreenPlate`: the full screen (grid 1, specks 1), not printed ink's lighter touch.
        let mut out = riso::print_with(&plate, (120, 162, 220), 13, 1.0, 1.0, &clip, 1.0, |c| c);
        // The ghost word: paper at 32%, its box's bottom 20 points below the band's.
        let size = 72.0;
        let font = FontRef::new(LONDRINA).unwrap();
        let m = font.metrics(Size::new(size), LocationRef::default());
        // The box's bottom 20 points under the band's, its baseline a descent above it; SwiftUI sets
        // this face 9 points lower than its metrics say (measured against the Swift drawing).
        let baseline = BAND_H + 20.0 + m.descent + 9.0; // descent is negative
        let word_w = advance(&font, &word, size);
        let mut glyphs = Mask::new(plate.px().0, plate.px().1).unwrap();
        let put = |glyphs: &mut Mask, s: &str, x: f32| {
            let mut pen = Outline { b: PathBuilder::new(), x, y: baseline };
            draw_text(&font, s, size, &mut pen);
            if let Some(p) = pen.b.finish() {
                glyphs.fill_path(&p, FillRule::Winding, true, plate.transform());
            }
        };
        if word_w <= w - 8.0 {
            put(&mut glyphs, &word, -4.0);
        } else {
            let gap = size * 0.55;
            let cycle = word_w + gap;
            let travelled = (t as f32 % (cycle / 26.0)) * 26.0;
            let mut x = travelled - cycle;
            for _ in 0..3 {
                put(&mut glyphs, &word, x);
                x += word_w;
                star(&mut glyphs, &plate, x + gap / 2.0, baseline - size * 0.33, size * 0.42 * 0.42);
                x += gap;
            }
        }
        let paper = ink::PAPER;
        for (i, (c, g)) in out.chunks_exact_mut(4).zip(glyphs.data()).enumerate() {
            let a = *g as f32 / 255.0 * 0.32 * (clip.data()[i] as f32 / 255.0);
            for k in 0..3 {
                c[k] = (c[k] as f32 * (1.0 - a) + [paper.0, paper.1, paper.2][k] as f32 * a).round() as u8;
            }
        }
        out
    };
    if ticker {
        let (pw, ph) = plate.px();
        riso::frame_image(pw, ph, make())
    } else {
        riso::cached(key, &plate, make)
    }
}

/// "✻", the heavy teardrop-spoked star between ticker repeats.
fn star(mask: &mut Mask, plate: &Plate, cx: f32, cy: f32, r: f32) {
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (dx, dy) = (a.cos(), a.sin());
        let (nx, ny) = (-dy, dx);
        let tip = (cx + dx * r, cy + dy * r);
        let mid = (cx + dx * r * 0.62, cy + dy * r * 0.62);
        let w = r * 0.11;
        let mut b = PathBuilder::new();
        b.move_to(cx + dx * r * 0.12, cy + dy * r * 0.12);
        b.quad_to(mid.0 + nx * w * 1.6, mid.1 + ny * w * 1.6, tip.0, tip.1);
        b.quad_to(mid.0 - nx * w * 1.6, mid.1 - ny * w * 1.6, cx + dx * r * 0.12, cy + dy * r * 0.12);
        b.close();
        if let Some(p) = b.finish() {
            mask.fill_path(&p, FillRule::Winding, true, plate.transform());
        }
    }
}

struct Outline {
    b: PathBuilder,
    x: f32,
    y: f32,
}
impl skrifa::outline::OutlinePen for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.b.move_to(self.x + x, self.y - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.b.line_to(self.x + x, self.y - y);
    }
    fn quad_to(&mut self, a: f32, b: f32, x: f32, y: f32) {
        self.b.quad_to(self.x + a, self.y - b, self.x + x, self.y - y);
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.b.cubic_to(self.x + a, self.y - b, self.x + c, self.y - d, self.x + x, self.y - y);
    }
    fn close(&mut self) {
        self.b.close();
    }
}

/// `s` shaped as Core Text sets it (kerning included): each glyph and where it starts, and the width.
fn shape(s: &str, size: f32) -> (Vec<(u32, f32, f32)>, f32) {
    let face = rustybuzz::Face::from_slice(LONDRINA, 0).unwrap();
    let k = size / face.units_per_em() as f32;
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(s);
    let out = rustybuzz::shape(&face, &[], buf);
    let mut x = 0.0;
    let mut glyphs = Vec::new();
    for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
        glyphs.push((info.glyph_id, x + pos.x_offset as f32 * k, pos.y_offset as f32 * k));
        x += pos.x_advance as f32 * k;
    }
    (glyphs, x)
}

fn advance(_font: &FontRef, s: &str, size: f32) -> f32 {
    shape(s, size).1
}

fn draw_text(font: &FontRef, s: &str, size: f32, pen: &mut Outline) {
    let outlines = font.outline_glyphs();
    let start = pen.x;
    let (glyphs, _) = shape(s, size);
    for (gid, x, y) in glyphs {
        if let Some(o) = outlines.get(skrifa::GlyphId::new(gid)) {
            pen.x = start + x;
            let base = pen.y;
            pen.y = base - y;
            let _ = o.draw(DrawSettings::unhinted(Size::new(size), LocationRef::default()), &mut *pen);
            pen.y = base;
        }
    }
}

// MARK: - The paper

/// `NoteShape`: a hand-cut slip with a tail at the top pointing at the flower.
fn note_shape(w: f32, h: f32, tail_offset: f32) -> Pen {
    let mut rng = Rng::new(17);
    let p1 = rng.f() * 6.3;
    let p2 = rng.f() * 6.3;
    let (bx, by, bw, bh) = (0.0, TAIL, w, h - TAIL);
    let cx = (bx + bw / 2.0 + tail_offset).clamp(bx + 22.0, bx + bw - 22.0);
    let wob = |d: f32| 0.9 * ((d * 0.045 + p1).sin() * 0.6 + (d * 0.13 + p2).sin() * 0.4);
    let mut pts = Vec::new();
    let mut d = 0.0f32;
    let mut x = bx + 3.0;
    while x < bx + bw - 3.0 {
        if x > cx - 12.0 && x < cx + 12.0 {
            pts.push([cx - 11.0, by]);
            pts.push([cx - 2.0, 1.5]);
            pts.push([cx + 1.5, 0.5]);
            pts.push([cx + 11.0, by]);
            x = cx + 14.0;
            continue;
        }
        pts.push([x, by + wob(d)]);
        x += 7.0;
        d += 7.0;
    }
    let mut y = by + 3.0;
    while y < by + bh - 3.0 {
        pts.push([bx + bw + wob(d), y]);
        y += 7.0;
        d += 7.0;
    }
    x = bx + bw - 3.0;
    while x > bx + 3.0 {
        pts.push([x, by + bh + wob(d)]);
        x -= 7.0;
        d += 7.0;
    }
    y = by + bh - 3.0;
    while y > by + 3.0 {
        pts.push([bx + wob(d), y]);
        y -= 7.0;
        d += 7.0;
    }
    Pen::catmull_rom(&pts, true)
}

/// Gaussian blur of a mask, `sigma` in pixels: three box blurs each way, which come within a
/// percent or two of a Gaussian at a fraction of the cost (the exact separable blur took 77 ms
/// for the slip's shadow; this takes a few).
pub fn blur(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    // Box widths whose three passes give this sigma (Kovesi's method).
    let n = 3.0f32;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - n * (wl * wl) as f32 - 4.0 * n * wl as f32 - 3.0 * n) / (-4.0 * wl as f32 - 4.0)).round() as i32;
    let mut a = src.to_vec();
    let mut b = vec![0.0f32; w * h];
    for pass in 0..3 {
        let r = ((if pass < m { wl } else { wu }) - 1) / 2;
        box_pass(&a, &mut b, w, h, r, true);
        box_pass(&b, &mut a, w, h, r, false);
    }
    a
}

/// One running-sum box blur of radius `r` along rows (or columns), zero past the edges.
fn box_pass(src: &[f32], out: &mut [f32], w: usize, h: usize, r: i32, rows: bool) {
    let (lines, len) = if rows { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| if rows { line * w + i } else { i * w + line };
    let k = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut sum = 0.0f32;
        for i in 0..=(r as usize).min(len - 1) {
            sum += src[at(line, i)];
        }
        for i in 0..len {
            out[at(line, i)] = sum * k;
            let add = i as i32 + r + 1;
            if (add as usize) < len {
                sum += src[at(line, add as usize)];
            }
            let sub = i as i32 - r;
            if sub >= 0 {
                sum -= src[at(line, sub as usize)];
            }
        }
    }
}

/// The slip's paper, `pad` points of room round it for the shadow: the shadow cut away under
/// the note, the paper at 90% (the desktop shows through a little), the cut edge in a soft pen.
fn note_paper(w: f32, h: f32, scale: f32) -> (Pixels, f32) {
    let pad = 40.0;
    let plate = Plate { w: w + 2.0 * pad, h: h + 2.0 * pad, scale };
    let opacity = if SOLID.with(|s| s.get()) { 1.0 } else { PAPER_OPACITY };
    let px = riso::cached(format!("note-{w}-{h}-{opacity}"), &plate, || {
        let (pw, ph) = plate.px();
        let (pwu, phu) = (pw as usize, ph as usize);
        let shape = note_shape(w, h, 0.0);
        let mask_at = |dx: f32, dy: f32, path: &Pen| {
            let mut m = Mask::new(pw, ph).unwrap();
            m.fill_path(&path.translated(pad + dx, pad + dy).skia().unwrap(), FillRule::Winding, true, plate.transform());
            m.data().iter().map(|v| *v as f32 / 255.0).collect::<Vec<f32>>()
        };
        let note = mask_at(0.0, 0.0, &shape);
        // The drop shadow: the note pulled in 10 at the sides, dropped 9, blurred; and a contact line.
        let narrow = note_shape(w - 20.0, h, 0.0);
        let far = blur(&mask_at(10.0, 9.0, &narrow), pwu, phu, 9.0 * scale);
        let near = blur(&mask_at(0.0, 1.0, &shape), pwu, phu, 0.8 * scale);
        let mut edge = Mask::new(pw, ph).unwrap();
        let path = shape.translated(pad, pad).skia().unwrap();
        edge.fill_path(&path, FillRule::Winding, true, plate.transform());
        let mut line = Pixmap::new(pw, ph).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        line.stroke_path(&path, &paint, &tiny_skia::Stroke { width: 1.0, ..Default::default() }, plate.transform(), None);
        let mut out = vec![0u8; pwu * phu * 4];
        for i in 0..pwu * phu {
            let shadow = (1.0 - (1.0 - far[i] * 0.2) * (1.0 - near[i] * 0.12)) * (1.0 - note[i]);
            // Shadow (black), then paper at 90%, then the edge in ink at 18%, source-over.
            let mut c = [0.0f32; 3];
            let mut a = shadow;
            let lay = |c: &mut [f32; 3], a: &mut f32, rgb: Rgb8, alpha: f32| {
                let oa = alpha + *a * (1.0 - alpha);
                if oa > 0.0 {
                    for (k, v) in [rgb.0, rgb.1, rgb.2].iter().enumerate() {
                        c[k] = (*v as f32 * alpha + c[k] * *a * (1.0 - alpha)) / oa;
                    }
                }
                *a = oa;
            };
            lay(&mut c, &mut a, ink::PAPER, note[i] * opacity);
            lay(&mut c, &mut a, ink::INK, line.pixels()[i].alpha() as f32 / 255.0 * 0.18);
            out[i * 4..i * 4 + 4].copy_from_slice(&[c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, (a * 255.0).round() as u8]);
        }
        out
    });
    (px, pad)
}

/// The paper's tooth over the note, clipped to it.
fn note_grain(w: f32, h: f32, scale: f32) -> Pixels {
    let plate = Plate { w, h, scale };
    riso::cached(format!("note-grain-{w}-{h}"), &plate, || {
        let grain = riso::grain_rgba(w, h, scale, 0.22);
        let clip = plate.fill(&note_shape(w, h, 0.0).skia().unwrap());
        let mut out = grain;
        for (c, m) in out.chunks_exact_mut(4).zip(clip.data()) {
            c[3] = ((c[3] as u32 * *m as u32 + 127) / 255) as u8;
        }
        out
    })
}


