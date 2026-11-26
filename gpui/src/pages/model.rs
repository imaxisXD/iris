//! Settings › Model & memory (`ModelPage` in SettingsView.swift): the model's state as a ghost
//! word on a sky band with the garden standing on its edge, then the two models drawn like the
//! zine's brewers (the chosen one ringed in pink brush, the one that suits this Mac marked
//! "recommended"), then two rows on memory.
//!
//! The snapshot draws the model resting with Best quality chosen and both models on disk, as
//! the Swift `--snapshot` finds them on this Mac. `IRIS_MODEL_STATE=downloading` or
//! `delete-hover` draws the other two states the Swift snapshot draws.
//!
//! What Rune turns (the chips, the sticker, the "recommended" words) is printed whole, turned.
//! The big figure (Martian at width 75) and its unit (weight 500) are printed from the variable
//! font's outlines.

use gpui::{Div, InteractiveElement, ParentElement, Role, StatefulInteractiveElement, Styled, div};
use skrifa::outline::DrawSettings;
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, Mask, PathBuilder, Transform};

use crate::garden::{self, Stage};
use crate::icons::{self, Draw, circle, ellipse, line, rounded};
use crate::letters::{self, LONDRINA};
use crate::parts::*;
use crate::pen::{self, Pen};
use crate::riso::{self, Img, Plate, Rgb8, Rng, hue, ink, over, str_seed};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

const BAND_H: f32 = 112.0;
const GHOST_SIZE: f32 = 84.0;
const GHOST_DROP: f32 = 22.0;
const GARDEN_SCALE: f32 = 0.66;
const SKY: Rgb8 = (120, 162, 220);

/// The models (`ModelChoice`): Iris's own two, then Google's earlier ones (offered only while
/// they're on this Mac). (id, name, one line on what it's for, model bytes, download bytes).
struct Choice {
    id: &'static str,
    name: &'static str,
    line: &'static str,
    model_bytes: f64,
    download_bytes: f64,
    trained: bool,
}

const CHOICES: [Choice; 4] = [
    Choice { id: "best", name: "Best quality", line: "Trained for Iris. The best in English and Hindi. For Macs with 16 GB or more.", model_bytes: 4.58e9, download_bytes: 4.78e9, trained: true },
    Choice { id: "lighter", name: "Lighter", line: "Trained for Iris. Nearly as good in English, weaker in Hindi. For Macs with 8 GB.", model_bytes: 3.77e9, download_bytes: 3.97e9, trained: true },
    Choice { id: "e4b", name: "Earlier best quality", line: "The model Iris used before. Not trained for Iris.", model_bytes: 5.18e9, download_bytes: 5.38e9, trained: false },
    Choice { id: "e2b", name: "Earlier faster model", line: "The model Iris used before. Not trained for Iris.", model_bytes: 4.36e9, download_bytes: 4.56e9, trained: false },
];

impl Choice {
    fn of(id: &str) -> &'static Choice {
        CHOICES.iter().find(|c| c.id == id).unwrap_or(&CHOICES[0])
    }
    /// "4.8": `downloadFigure`.
    fn figure(&self) -> String {
        format!("{:.1}", self.download_bytes / 1e9)
    }
    fn size(&self) -> String {
        format!("{} GB", self.figure())
    }
    /// "2.7 of 4.8 GB": how much is in, of the model alone, out of what the button promised.
    fn downloaded(&self, f: f64) -> String {
        format!("{:.1} of {:.1} GB", self.model_bytes * f / 1e9, self.download_bytes / 1e9)
    }
}

/// The model page live: the brain's state, the model chosen, which are on disk, and the
/// Delete the pointer is on.
#[derive(Clone, Debug)]
pub struct Live {
    pub engine: crate::brain::Engine,
    pub chosen: String,
    /// (id, size to download, on disk), as the brain lists them.
    pub models: Vec<(String, String, bool)>,
    pub recommended: String,
    pub crossing: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Engine {
    Resting,
    Downloading(f64),
    Loading,
    Ready,
    Failed(String),
}

/// What the page shows: the engine, the chosen model, the models offered and which are on
/// disk, which Delete the pointer is on, the one that suits this Mac.
struct Look {
    engine: Engine,
    chosen: String,
    offered: Vec<(&'static Choice, bool)>,
    crossing: Option<String>,
    previewing: Option<String>,
    recommended: String,
}

impl Look {
    /// As the Swift snapshot finds this Mac: both of Iris's models on disk, Best quality
    /// chosen. `IRIS_MODEL_STATE=downloading`, `delete-hover`, `use-hover`, `earlier` or
    /// `not-downloaded` draws the others.
    fn snapshot() -> Look {
        let both = |a: bool, b: bool| vec![(&CHOICES[0], a), (&CHOICES[1], b)];
        let (engine, chosen, offered, crossing, previewing) = match std::env::var("IRIS_MODEL_STATE").as_deref() {
            Ok("downloading") => (Engine::Downloading(0.62), "lighter", both(true, false), None, None),
            Ok("delete-hover") => (Engine::Ready, "best", both(true, true), Some("lighter".to_string()), None),
            Ok("use-hover") => (Engine::Ready, "best", both(true, true), None, Some("lighter".to_string())),
            // Someone who used Iris before the trained models: on Google's model still.
            Ok("earlier") => (Engine::Ready, "e4b", vec![(&CHOICES[0], false), (&CHOICES[1], false), (&CHOICES[2], true), (&CHOICES[3], true)], None, None),
            Ok("not-downloaded") => (Engine::Resting, "lighter", both(true, false), None, None),
            _ => (Engine::Resting, "best", both(true, true), None, None),
        };
        Look { engine, chosen: chosen.into(), offered, crossing, previewing, recommended: "best".into() }
    }

    fn live(l: &Live) -> Look {
        use crate::brain::Engine as E;
        // Iris's own two always; an earlier one only while it's on disk (`ModelChoice.offered`).
        let offered = CHOICES.iter().filter_map(|c| {
            let on_disk = l.models.iter().any(|(id, _, d)| id == c.id && *d);
            (c.trained || on_disk).then_some((c, on_disk))
        }).collect();
        let engine = match &l.engine {
            E::Idle => Engine::Resting,
            E::Downloading(f) => Engine::Downloading(*f),
            E::Loading => Engine::Loading,
            // Only show Ready when the loaded model is the one chosen in Settings; otherwise
            // reflect the chosen model's own state so switching to an undownloaded model offers
            // Download right away instead of claiming Ready / Free model memory.
            E::Ready(loaded) if loaded == &l.chosen => Engine::Ready,
            E::Ready(_) => Engine::Resting,
            E::Failed(f) => Engine::Failed(f.sentence.clone()),
        };
        Look { engine, chosen: l.chosen.clone(), offered, crossing: l.crossing.clone(), previewing: None, recommended: l.recommended.clone() }
    }

    fn on_disk(&self, id: &str) -> bool {
        self.offered.iter().any(|(c, d)| c.id == id && *d)
    }
    fn needs_download(&self) -> bool {
        matches!(self.engine, Engine::Resting) && !self.on_disk(&self.chosen)
    }
    fn not_downloaded_note(&self) -> String {
        crate::words::ws(format!(
            "{} isn't on this Mac yet. Download it here before using your shortcut.",
            Choice::of(&self.chosen).name
        ))
    }
    fn title(&self) -> &'static str {
        match self.engine {
            Engine::Resting if !self.on_disk(&self.chosen) => "Not downloaded",
            Engine::Resting => "Resting",
            Engine::Downloading(_) => "Downloading",
            Engine::Loading => "Waking up",
            Engine::Ready => "Ready",
            Engine::Failed(_) => "Something's off",
        }
    }
    fn detail(&self) -> String {
        match &self.engine {
            Engine::Resting if !self.on_disk(&self.chosen) => self.not_downloaded_note(),
            Engine::Resting => "The model loads on your first check.".into(),
            Engine::Downloading(_) => "Getting the model. One time only.".into(),
            Engine::Loading => "Getting ready. A few seconds.".into(),
            Engine::Ready => crate::words::w("Runs on this Mac. Nothing leaves it.").into(),
            Engine::Failed(s) => s.clone(),
        }
    }
    /// The command that applies now, at the band's top left (none while it wakes up).
    fn action(&self) -> Option<String> {
        match self.engine {
            Engine::Resting | Engine::Failed(_) => Some(if self.on_disk(&self.chosen) { "Load model now".into() } else { format!("Download ({})", Choice::of(&self.chosen).size()) }),
            Engine::Downloading(_) => Some("Stop download".into()),
            Engine::Ready => Some("Free model memory".into()),
            Engine::Loading => None,
        }
    }
    fn stage(&self) -> Stage {
        match self.engine {
            Engine::Resting | Engine::Failed(_) => Stage::Bud,
            Engine::Downloading(_) => Stage::Watering,
            Engine::Loading => Stage::Warming,
            Engine::Ready => Stage::Ready,
        }
    }
    fn progress(&self) -> f64 {
        if let Engine::Downloading(f) = self.engine { f } else { 0.0 }
    }
}

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    let look = v.model.as_ref().map(Look::live).unwrap_or_else(Look::snapshot);
    let mut out = vec![band(&look, w, s)];
    if let Engine::Downloading(f) = look.engine {
        out.push(column().w(z(w)).gap(z(6.0)).mt(z(-14.0))
            .child(progress_track(f, w, s))
            .child(t(Choice::of(&look.chosen).downloaded(f), caption(), hue::I10)));
    } else if let Engine::Failed(_) = look.engine {
        out.push(column().w(z(w)).mt(z(-14.0)).child(para_out(&look.detail(), caption(), hue::RED_TEXT, w)));
    } else if look.needs_download() {
        out.push(column().w(z(w)).mt(z(-14.0)).child(para_out(&look.not_downloaded_note(), caption(), hue::WARN_TEXT, w)));
    }
    let trained: Vec<_> = look.offered.iter().filter(|(c, _)| c.trained).collect();
    let earlier: Vec<_> = look.offered.iter().filter(|(c, _)| !c.trained).collect();
    let mut models = row().w(z(w)).items_start();
    for (i, (c, d)) in trained.iter().enumerate() {
        models = models.child(model(&look, c, *d, i, true, w / trained.len() as f32, s));
    }
    let mut which = column().w(z(w)).gap(z(18.0)).child(models).child(crate::model_compare::pips_note(w));
    if !earlier.is_empty() {
        let mut row_ = row().w(z(w)).items_start();
        for (i, (c, d)) in earlier.iter().enumerate() {
            row_ = row_.child(model(&look, c, *d, i + 2, false, w / earlier.len() as f32, s));
        }
        which = which.child(column().w(z(w)).gap(z(10.0)).pt(z(8.0)).child(t(crate::words::w("Earlier models on this Mac"), brush(20.0), hue::I12)).child(row_));
    }
    let idle = [5u32, 15, 30, 60, 240];
    let idle_label = |m: u32| if m < 60 { format!("{m} min") } else if m == 60 { "1 hour".into() } else { format!("{} hours", m / 60) };
    let idle_at = idle.iter().position(|m| *m == v.idle_minutes).unwrap_or(2);
    let free_after = crate::controls::menu(idle.iter().map(|m| idle_label(*m)).collect(), idle_at, Some(&clock(20.0, cx.scale)), 132.0, move |i, cx| {
        crate::app::change(|s| s.idle_minutes = idle[i]);
        crate::brain::configure(cx);
    });
    let load = crate::controls::toggle(v.load_at_launch, "Load when Iris starts", s, crate::interact::change(|s| s.load_at_launch = !s.load_at_launch));
    let light_title = crate::words::w("Go easy on this Mac");
    let light_detail = crate::words::w("Uses less memory and quieter motion, so this Mac stays responsive while Iris writes.");
    let light = crate::controls::toggle(v.light_mode, light_title, s, crate::interact::act(|window, cx| {
        crate::app::change(|s| s.light_mode = Some(!s.light_mode_active()));
        crate::brain::configure(cx);
        window.refresh();
    }));
    let memory = column().w(z(w)).gap(z(12.0))
        .child(setting_row("Load when Iris starts", None, load, 50.0, w))
        // `PenRule()` takes the line it's called on as its seed.
        .child(hairline(861, w))
        .child(setting_row("Free memory after", None, free_after, 132.0, w))
        .child(hairline(865, w))
        .child(setting_row(light_title, Some(light_detail), light, 50.0, w));
    out.push(section_wrapped(1, "Which model", Some(crate::words::w("Each runs on this Mac. Download a model here once before using it.")), None, which, w, s));
    out.push(section_wrapped(2, "Memory", None, None, memory, w, s));
    out
}

/// `ProgressTrack`: a pink marker swipe across a faint pen line, the share in brush at its tip.
/// Its frame is 18 tall, but the brush share is taller (21.5), so the ZStack inside is too and
/// centres the line and the swipe in it.
fn progress_track(f: f64, width: f32, scale: f32) -> Div {
    let f = f.clamp(0.0, 1.0) as f32;
    let w = ((width - 44.0) * f).max(16.0);
    let mid = (17.0 * BRUSH_LINE * 2.0).round() / 4.0;
    sized(width, 18.0)
        .child(at(pen_rule(91, width, over(hue::I12, 0.18, ink::PAPER), 1.0), 0.0, mid - 1.5))
        .child(at(faded(hand_printed_frame(92, 0.7, w, 12.0, ink::PINK, 92, scale), 0.45), 0.0, mid - 6.0))
        .child(at(t(format!("{}%", (f * 100.0) as i32), brush(17.0), ink::PINK_TEXT), w + 6.0, mid - line_box(17.0, BRUSH_LINE) / 2.0))
}

// MARK: - The band

/// The state as a ghost word in paper on the sky band, the command at its top left, the
/// garden standing on its bottom edge (`.offset(y: 22)`), 12 points of room under it.
fn band(look: &Look, w: f32, scale: f32) -> Div {
    let side = garden::SIZE * GARDEN_SCALE;
    let (g, g_side) = garden::garden(look.stage(), look.progress(), None, 1000.0, GARDEN_SCALE * zoom(), scale);
    let g_side = g_side / zoom();
    let pad = garden::PAD * GARDEN_SCALE;
    // The garden's frame is taller than the band, so the ZStack is too, and `.frame(height: 112)`
    // centres it: everything in it rides up by half the difference, on a whole point.
    let lift = ((side - BAND_H) / 2.0).floor();
    let mut b = sized(w, BAND_H + 12.0)
        .child(at(printed(w, BAND_H, &status_band(look.title(), w, scale)), 0.0, -lift));
    if let Some(title) = look.action() {
        let act = band_action(look);
        let (action, _) = crate::controls::button(&crate::controls::Button::secondary(title).compact().id("model-band-action"), scale, act);
        b = b.child(at(action, 14.0, 14.0 - lift));
    }
    let b = b
        .child(at(printed(g_side, g_side, &g), w - 6.0 - side - pad, 22.0 - lift - pad))
        .id("settings-model-state").role(Role::Image).aria_label(format!("{}. {}", look.title(), look.detail()));
    div().flex_none().child(b)
}

/// What the band's command does: frees the memory, loads (downloading when the model isn't on
/// disk, as Swift's button says it will), or asks before stopping a download.
fn band_action(look: &Look) -> crate::interact::Act {
    use crate::interact::act;
    match look.engine {
        Engine::Ready => act(|_, _| crate::brain::unload()),
        Engine::Downloading(_) => act(|window, _| crate::confirm::ask(window, "Stop downloading the model?",
            crate::words::w("What's downloaded so far stays on this Mac, and the download picks up where it stopped the next time."),
            "Stop download", |_, _| crate::brain::unload())),
        _ => act(|_, _| crate::brain::load(true)),
    }
}

/// `StatusBand(height: 112, size: 84, drop: 22)`: the sky plate, the state in paper at 32%
/// along its bottom edge (a ticker when too long, caught at its first moment), cut by hand.
fn status_band(word: &str, w: f32, scale: f32) -> Img {
    let word = word.to_uppercase();
    let plate = Plate { w, h: BAND_H, scale };
    riso::cached(format!("settings-model-band-{word}-{w}"), &plate, || {
        let clip = plate.fill(&pen::hand_rect(21, 0.5, 0.0, true, 0.0, 0.0, w, BAND_H).skia().unwrap());
        // `ScreenPlate`: the screen at full grid and specks (not `PrintedInk`'s lighter touch).
        let mut out = riso::print_with(&plate, SKY, 13, 1.0, 1.0, &clip, 1.0, |c| c);
        let font = FontRef::new(LONDRINA).unwrap();
        let m = font.metrics(Size::new(GHOST_SIZE), LocationRef::default());
        // The word's box hangs 22 points under the band, its baseline a descent above that.
        let baseline = BAND_H + GHOST_DROP + m.descent + GHOST_LOWER;
        let word_w = shape(&word, GHOST_SIZE).1;
        let mut glyphs = Mask::new(plate.px().0, plate.px().1).unwrap();
        let put = |glyphs: &mut Mask, s: &str, x: f32| {
            let mut pen = Outline { b: PathBuilder::new(), x, y: baseline };
            draw_text(&font, s, GHOST_SIZE, &mut pen);
            if let Some(p) = pen.b.finish() {
                glyphs.fill_path(&p, FillRule::Winding, true, plate.transform());
            }
        };
        if word_w <= w - 8.0 {
            put(&mut glyphs, &word, -4.0);
        } else {
            // Frozen at 0: the second of three copies starts at the band's left edge.
            let gap = GHOST_SIZE * 0.55;
            let mut x = -(word_w + gap);
            for _ in 0..3 {
                put(&mut glyphs, &word, x);
                x += word_w;
                letters::star_into(&mut glyphs, &plate, x + gap / 2.0, baseline - GHOST_SIZE * 0.33, GHOST_SIZE * 0.42 * 0.42);
                x += gap;
            }
        }
        let paper = [ink::PAPER.0, ink::PAPER.1, ink::PAPER.2];
        for (i, (c, g)) in out.chunks_exact_mut(4).zip(glyphs.data()).enumerate() {
            let a = *g as f32 / 255.0 * 0.32 * (clip.data()[i] as f32 / 255.0);
            for k in 0..3 {
                c[k] = (c[k] as f32 * (1.0 - a) + paper[k] as f32 * a).round() as u8;
            }
        }
        out
    })
}

/// How much lower SwiftUI sets Londrina than its metrics put it, at 84 points (measured).
const GHOST_LOWER: f32 = -0.5;

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

fn draw_text(font: &FontRef, s: &str, size: f32, pen: &mut Outline) {
    let outlines = font.outline_glyphs();
    let start = pen.x;
    for (gid, x, y) in shape(s, size).0 {
        if let Some(o) = outlines.get(skrifa::GlyphId::new(gid)) {
            pen.x = start + x;
            let base = pen.y;
            pen.y = base - y;
            let _ = o.draw(DrawSettings::unhinted(Size::new(size), LocationRef::default()), &mut *pen);
            pen.y = base;
        }
    }
}

// MARK: - Which model

/// One model drawn like a brewer (`ModelOption`; `big` in Settings' row, smaller under Earlier
/// models), its dots, then its buttons: Use this one unless it's chosen, Delete if it's on disk
/// (not while the chosen one downloads). A click on the drawing chooses it too.
fn model(look: &Look, c: &'static Choice, on_disk: bool, i: usize, big: bool, w: f32, scale: f32) -> Div {
    use crate::controls::{self as ctl, Button};
    use crate::interact::{self as live, Curve};
    let l = live::get().unwrap_or_else(live::Live::still);
    let chosen = look.chosen == c.id;
    let first = i == 0;

    // The drawing: the chip (turned, printed whole), its ring if chosen (or a soft preview ring
    // while pointing at "Use this one" or the unchosen drawing), the red X while Delete is under
    // the pointer, each fading in on `DS.easeOut`.
    let size = if big { if first { 92.0 } else { 74.0 } } else if first { 74.0 } else { 60.0 };
    let (fw, fh) = if big { (130.0, 116.0) } else { (104.0, 88.0) };
    let mut figure = sized(fw, fh)
        .child(at(chip(size as u32, if first { -6.0 } else { 5.0 }, scale), (fw - size) / 2.0, (fh - size) / 2.0));
    let use_id = format!("use-{}", c.id);
    let card_id = format!("model-card-{}", c.id);
    let crossed = look.crossing.as_deref() == Some(c.id);
    let previewed = !chosen && !crossed
        && (look.previewing.as_deref() == Some(c.id) || l.is_hovered(&use_id) || l.is_hovered(&card_id));
    let ring_k = l.track(&format!("model-ring-{}", c.id), chosen as u8 as f32, Curve::FAST);
    let preview_k = l.track(&format!("model-preview-{}", c.id), previewed as u8 as f32, Curve::FAST);
    let (rw, rh) = if big { (124.0, 112.0) } else { (98.0, 86.0) };
    if ring_k > 0.001 {
        figure = figure.child(crate::settings_kit::faded(ring(13 + i as i64, rw, rh, fw / 2.0, fh / 2.0, scale), ring_k));
    } else if preview_k > 0.001 {
        // Soft preview of the chosen ring while pointing at "Use this one" or the drawing itself,
        // lighter than the chosen state so hover never reads as already selected.
        let k = if l.reduce_motion { 1.0 } else { 0.90 + 0.10 * preview_k };
        let preview = with_zoom(zoom() * k, || ring(13 + i as i64, rw, rh, fw / (2.0 * k), fh / (2.0 * k), scale));
        figure = figure.child(crate::settings_kit::faded(preview, 0.45 * preview_k));
    }
    let x_k = l.track(&format!("model-x-{}", c.id), crossed as u8 as f32, Curve::FAST);
    if x_k > 0.001 {
        let (xw, xh) = if first { (132.0, 114.0) } else { (116.0, 100.0) };
        // In from 0.8 of its size and faded (`.scale(0.8).combined(with: .opacity)`).
        let k = if l.reduce_motion { 1.0 } else { 0.8 + 0.2 * x_k };
        let x = with_zoom(zoom() * k, || printed(xw, xh, &dry_brush_x(ink::RED, 4 + i as i64, 0.1, 0.65, xw, xh, scale)));
        figure = figure.child(at(crate::settings_kit::faded(x, x_k), (fw - xw * k) / 2.0, (fh - xh * k) / 2.0));
    }
    let name = if big { ink_label(c.name, 12.0, scale).0 } else { t(c.name, body(), hue::I12) };
    let mut option = column().gap(z(if big { 10.0 } else { 8.0 }))
        .child(figure)
        .child(name)
        .child(stacked_figure(&c.figure(), "GB", if on_disk { "on disk" } else { "to download" }, scale));
    if big {
        option = option.child(mono_paragraph(crate::words::w(c.line), caption(), hue::I10, 210.0, 11.0 * MONO_LINE));
    }
    let choose = {
        let id = c.id.to_string();
        live::act(move |window, cx| {
            crate::app::change(|s| s.model = Some(id.clone()));
            crate::brain::configure(cx);
            window.refresh();
        })
    };
    let mut option = option.relative().id(gpui::SharedString::from(format!("settings-model-{}", c.id))).role(Role::RadioButton)
        .aria_label(format!("{}, {}. {}", c.name, c.size(), crate::words::w(c.line))).aria_selected(chosen);
    if l.view.is_some() {
        let ch = choose.clone();
        option = option.cursor_pointer().on_click(move |_, window, cx| ch(window, cx));
        if !chosen {
            option = option.child(live::bounds_marker(&l, &card_id)).on_hover(l.hover_handler(&card_id, None));
        }
    }

    let downloading = matches!(look.engine, Engine::Downloading(_));
    let loading = matches!(look.engine, Engine::Loading);
    let mut stack = column().w(z(w)).gap(z(10.0)).child(option);
    if let Some(st) = crate::model_compare::Strengths::of(c.id) {
        stack = stack.child(div().flex_none().py(z(2.0)).child(crate::model_compare::pips(&st, i as i64, scale)));
    }
    let mut buttons = row().gap(z(14.0)).items_center().min_h(z(30.0));
    if !chosen {
        let b = Button::secondary("Use this one").compact().id(use_id).label(format!("Use {}", c.name.to_lowercase()));
        let btn = if look.previewing.as_deref() == Some(c.id) {
            live::forcing(live::Interaction::HOVER, || ctl::button(&b, scale, choose).0)
        } else {
            ctl::button(&b, scale, choose).0
        };
        buttons = buttons.child(btn);
    } else if !on_disk && !downloading && !loading {
        let b = Button::primary(format!("Download ({})", c.size())).compact().id(format!("download-{}", c.id))
            .label(format!("Download {} ({})", c.name.to_lowercase(), c.size()));
        buttons = buttons.child(ctl::button(&b, scale, live::act(|_, _| crate::brain::load(true))).0);
    }
    if on_disk && !(chosen && downloading) {
        let id = c.id.to_string();
        let hover = {
            let id = id.clone();
            std::rc::Rc::new(move |on: bool, _: &mut gpui::App| {
                crate::live_settings::MOMENT.with(|m| {
                    let mut m = m.borrow_mut();
                    if on { m.crossing = Some(id.clone()) } else if m.crossing.as_deref() == Some(&id) { m.crossing = None }
                });
            })
        };
        let (name_l, size_) = (c.name.to_lowercase(), c.size());
        let ask = live::act(move |window, _| {
            let id = id.clone();
            crate::confirm::ask(window, &format!("Delete the {name_l} model from disk?"),
                &format!("This frees {size_} of disk space, not memory (Iris frees memory by itself when you're not using it). To use it again, download it from this page."),
                "Delete from disk", move |_, _| crate::brain::delete(&id));
        });
        if let Some(x) = ctl::cross_out("Delete", &format!("delete-{}", c.id), scale, hover, ask) {
            // Alone, its X lines up with the words above.
            buttons = buttons.child(div().flex_none().ml(z(if chosen { -6.0 } else { 0.0 })).child(x)
                .id(gpui::SharedString::from(format!("settings-delete-{}", c.id))).aria_label(format!("Delete {} from disk", c.name.to_lowercase())));
        }
    }
    stack = stack.child(buttons);
    let mut m = div().flex_none().relative().w(z(w)).child(stack);
    if c.id == look.recommended {
        m = m.child(at(recommended_mark(scale), 104.0, 4.0));
    }
    m
}

/// `RisoDrawing(.model)`: the chip printed big, a dot a point, the pink plate landed a few dots
/// off the ink (`RisoIcon.poster`), upscaled without smoothing, then turned `degrees`.
pub fn chip(pixels: u32, degrees: f32, scale: f32) -> Div {
    let k = scale.round().max(1.0) as u32;
    let pt = pixels as f32;
    turned(format!("settings-model-chip-{pixels}-{degrees}"), pt, pt, (pt * 0.12).ceil(), degrees, scale, move |sh| {
        let off = (pixels as i32 / 28).max(2);
        let art: [fn(&mut Draw); 3] = [
            |d| d.fill(&rounded(21.0, 22.0, 40.0, 40.0, 6.0), 150),
            |d| {
                d.fill(&rounded(20.0, 20.0, 40.0, 40.0, 6.0), 170);
                d.fill(&rounded(30.0, 30.0, 20.0, 20.0, 3.0), 20);
            },
            |d| {
                d.pen(4.8);
                d.stroke(&rounded(20.0, 20.0, 40.0, 40.0, 6.0));
                d.pen(3.6);
                d.stroke(&rounded(30.0, 30.0, 20.0, 20.0, 3.0));
                d.pen(4.0);
                for i in 0..3 {
                    let c = 29.0 + i as f32 * 11.0;
                    d.stroke(&line(c, 11.0, c, 19.0));
                    d.stroke(&line(c, 61.0, c, 69.0));
                    d.stroke(&line(11.0, c, 19.0, c));
                    d.stroke(&line(61.0, c, 69.0, c));
                }
            },
        ];
        let small = icons::print(pixels, str_seed("model", 11),
            &[(ink::CREMA, off, off + 1, art[0]), (ink::PINK, off + 1, -off, art[1]), (ink::ESPRESSO, 0, 0, art[2])]);
        let big = nearest(&small, pixels, k);
        sh.picture_at(&big, pixels * k, pixels * k, pt, pt, Transform::identity(), 1.0);
    })
}

/// RGBA `n`×`n` blown up `k` times, each pixel a `k`×`k` block.
fn nearest(src: &[u8], n: u32, k: u32) -> Vec<u8> {
    let side = n * k;
    let mut out = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let s = (((y / k) * n + x / k) * 4) as usize;
            let d = ((y * side + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// `BrushRing` in a `w`×`h` frame centred on (`cx`, `cy`): one loop of a loaded brush a little
/// past full, heavy where it lands, frayed where it lifts, pink through the screen.
pub fn ring(seed: i64, w: f32, h: f32, cx: f32, cy: f32, scale: f32) -> Div {
    let weight = (w.min(h) * 0.062).max(2.2);
    let (bw, bh) = ((w + 2.0 * weight) / 0.9, (h + 2.0 * weight) / 0.9);
    let plate = Plate { w: bw, h: bh, scale };
    let px = riso::cached(format!("settings-model-ring-{seed}-{w}-{h}"), &plate, || {
        let mask = plate.fill(&ink_ribbon_loop(seed, weight, 0.8, 0.5, bw, bh).skia().unwrap());
        riso::print(&plate, ink::PINK, seed + 3, &mask, 1.0)
    });
    at(printed(bw, bh, &px), cx - bw / 2.0, cy - bh / 2.0)
}

/// `InkRibbon(line: .loop(seed:), fray: true)` in a `w`×`h` frame: a ribbon `width` wide round
/// `HandEllipse`'s loop, pressing in and lifting, its end frayed into bristle tips.
fn ink_ribbon_loop(seed: i64, width: f32, start_taper: f32, end_taper: f32, w: f32, h: f32) -> Pen {
    let (ix, iy) = (w * 0.05 + width, h * 0.05 + width);
    let (rw, rh) = (w - 2.0 * ix, h - 2.0 * iy);
    let mut rng = Rng::new(seed);
    let start = -2.5 + rng.f() * 0.3;
    let sweep = 2.0 * std::f32::consts::PI + 0.55;
    let phase = rng.f() * 6.3;
    let c: Vec<[f32; 2]> = (0..=60)
        .map(|i| {
            let t = i as f32 / 60.0;
            let a = start + sweep * t;
            let grow = 1.0 + 0.1 * t;
            let o = (a * 3.0 + phase).sin();
            let (rx, ry) = (rw / 2.0 * grow + o, rh / 2.0 * grow + o);
            [w / 2.0 + rx * a.cos(), h / 2.0 + ry * a.sin()]
        })
        .collect();
    let n = c.len();
    let mut rng = Rng::new(n as i64 * 7);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for i in 0..n {
        let a = c[i.saturating_sub(1)];
        let b = c[(i + 1).min(n - 1)];
        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
        let nn = [-(b[1] - a[1]) / len, (b[0] - a[0]) / len];
        let t = i as f32 / (n - 1) as f32;
        let press = (t / 0.12).min(1.0) * (1.0 - start_taper) + start_taper;
        let lift = ((1.0 - t) / 0.3).min(1.0) * (1.0 - end_taper) + end_taper;
        let half = width / 2.0 * press.min(lift) * (1.0 + (rng.f() - 0.5) * 0.14);
        left.push([c[i][0] + nn[0] * half, c[i][1] + nn[1] * half]);
        right.push([c[i][0] - nn[0] * half, c[i][1] - nn[1] * half]);
    }
    let mut pts = left.clone();
    let e = n - 1;
    let (a, b) = (c[e - 1], c[e]);
    let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
    let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
    for k in 0..=8 {
        let u = k as f32 / 8.0;
        let base = [left[e][0] + (right[e][0] - left[e][0]) * u, left[e][1] + (right[e][1] - left[e][1]) * u];
        let out = if k % 2 == 1 { width * (0.3 + rng.f() * 0.7) } else { 0.0 };
        pts.push([base[0] + d[0] * out, base[1] + d[1] * out]);
    }
    pts.extend(right.into_iter().rev());
    Pen::poly(&pts)
}

/// `StackedFigure`: the figure big in condensed mono (width 75, printed), its unit stacked
/// small beside it (weight 500, printed).
pub fn stacked_figure(value: &str, unit: &str, note: &str, scale: f32) -> Div {
    let (big, _) = martian_line(value, 26.0, 400.0, 75.0, -0.6, 26.0 * MONO_LINE, hue::I12, scale);
    let small = |s: &str| martian_line(&s.to_uppercase(), 8.5, 500.0, 87.5, 0.0, 8.5 * MONO_LINE, hue::I11, scale).0;
    row().gap(z(5.0)).items_center()
        .child(big)
        .child(column().child(small(unit)).child(small(note).mt(z(-1.0))))
}

/// `RecommendedMark`: a pink starburst sticker with a pen tick, slapped on at an angle, and
/// "recommended" in brush with "for this Mac" under it, a little off straight too.
pub fn recommended_mark(scale: f32) -> Div {
    let sticker = turned("settings-model-sticker".into(), 52.0, 52.0, 8.0, -12.0, scale, move |sh| {
        let plate = Plate { w: 52.0, h: 52.0, scale };
        let pink = riso::print(&plate, ink::PINK, 9, &plate.fill(&burst(52.0).skia().unwrap()), 1.0);
        sh.picture(&pink, 52.0, 52.0, 2.0, 2.0);
        sh.stroke_at(&burst(52.0).skia().unwrap(), Transform::identity(), 1.6, hue::I12);
        sh.stroke_at(&pen::hand_tick(22.0, 18.0).skia().unwrap(), Transform::from_translate(15.0, 17.0), 3.0, hue::I12);
    });
    let lh = line_box(19.0, BRUSH_LINE);
    let cap_h = 11.0 * MONO_LINE;
    let ww = brush_w("recommended", 19.0).max(mono_w(crate::words::w("for this Mac"), 11.0, 0.0));
    let wh = lh - 3.0 + cap_h;
    let (lead, _) = brush_box("recommended", 19.0);
    let words = turned("settings-model-recommended".into(), ww, wh, 6.0, -3.0, scale, move |sh| {
        word_into(sh, brush(19.0), "recommended", lead, (lh - 19.0 * BRUSH_LINE) / 2.0 + 19.0 * BRUSH_ASCENT, ink::PINK_TEXT);
        word_into(sh, caption(), crate::words::w("for this Mac"), 0.0, lh - 3.0 + mono_base(11.0, cap_h), hue::I10);
    });
    row().gap(z(6.0)).items_center().child(sticker).child(words)
}

/// `RecommendedMark.Burst(points: 14, inner: 0.78, seed: 3)` in a `side`-point square.
fn burst(side: f32) -> Pen {
    let mut rng = Rng::new(3);
    let (c, radius) = (side / 2.0, side / 2.0);
    let pts: Vec<[f32; 2]> = (0..28)
        .map(|k| {
            let a = k as f32 / 28.0 * 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
            let rad = (if k % 2 == 0 { radius } else { radius * 0.78 }) * (0.94 + rng.f() * 0.08);
            [c + a.cos() * rad, c + a.sin() * rad]
        })
        .collect();
    Pen::poly(&pts)
}

// MARK: - Memory

/// `RisoIcon.clock`: an alarm clock with two pink bells and little feet.
/// Shown 20 points square in its dropdown, crisp (`.interpolation(.none)`, as every riso icon).
pub fn clock(points: f32, scale: f32) -> Img {
    riso::cached_crisp("settings-model-icon-clock".into(), 44, || {
        let art: [fn(&mut Draw); 3] = [
            |d| d.fill(&circle(41.0, 47.0, 25.0), 150),
            |d| {
                d.fill(&circle(40.0, 45.0, 24.0), 120);
                d.fill(&ellipse(19.0, 17.5, 18.0, 15.0), 30);
                d.fill(&ellipse(61.0, 17.5, 18.0, 15.0), 30);
            },
            |d| {
                d.pen(4.8);
                d.stroke(&ellipse(19.0, 17.5, 18.0, 15.0));
                d.stroke(&ellipse(61.0, 17.5, 18.0, 15.0));
                d.fill(&circle(40.0, 45.0, 24.0), 255);
                d.stroke(&circle(40.0, 45.0, 24.0));
                d.stroke(&line(27.0, 65.0, 21.0, 73.0));
                d.stroke(&line(53.0, 65.0, 59.0, 73.0));
                d.stroke(&line(40.0, 45.0, 40.0, 30.0));
                d.stroke(&line(40.0, 45.0, 51.0, 51.0));
                d.fill(&circle(40.0, 45.0, 4.0), 0);
            },
        ];
        icons::print(44, str_seed("clock", 11), &[(ink::CREMA, 2, 2, art[0]), (ink::PINK, -1, -1, art[1]), (ink::ESPRESSO, 0, 0, art[2])])
    }, points, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Engine as BrainEngine;

    fn live_state(engine: BrainEngine, chosen: &str, on_disk: &[&str]) -> Look {
        let models = CHOICES
            .iter()
            .filter(|c| c.trained || on_disk.contains(&c.id))
            .map(|c| (c.id.to_string(), c.size(), on_disk.contains(&c.id)))
            .collect();
        Look::live(&Live {
            engine,
            chosen: chosen.into(),
            models,
            recommended: "best".into(),
            crossing: None,
        })
    }

    #[test]
    fn switching_from_loaded_model_to_undownloaded_shows_not_downloaded() {
        let look = live_state(BrainEngine::Ready("best".into()), "lighter", &["best"]);
        assert_eq!(look.engine, Engine::Resting);
        assert!(look.needs_download());
        assert_eq!(look.title(), "Not downloaded");
        assert_eq!(look.action().as_deref(), Some("Download (4.0 GB)"));
        assert!(look.detail().contains("Lighter"));
        assert!(look.detail().contains("Download it here"));
    }

    #[test]
    fn idle_undownloaded_model_shows_not_downloaded_instead_of_resting() {
        let look = live_state(BrainEngine::Idle, "lighter", &[]);
        assert!(look.needs_download());
        assert_eq!(look.title(), "Not downloaded");
        assert_eq!(look.action().as_deref(), Some("Download (4.0 GB)"));
        assert!(!look.detail().contains("loads on your first check"));
    }

    #[test]
    fn ready_matching_chosen_model_stays_ready() {
        let look = live_state(BrainEngine::Ready("best".into()), "best", &["best"]);
        assert_eq!(look.engine, Engine::Ready);
        assert!(!look.needs_download());
        assert_eq!(look.title(), "Ready");
        assert_eq!(look.action().as_deref(), Some("Free model memory"));
    }

    #[test]
    fn switching_from_loaded_model_to_another_on_disk_shows_resting_and_load() {
        let look = live_state(BrainEngine::Ready("best".into()), "lighter", &["best", "lighter"]);
        assert_eq!(look.engine, Engine::Resting);
        assert!(!look.needs_download());
        assert_eq!(look.title(), "Resting");
        assert_eq!(look.action().as_deref(), Some("Load model now"));
    }

    #[test]
    fn downloading_model_keeps_downloading_state_and_stop_action() {
        let look = live_state(BrainEngine::Downloading(0.42), "lighter", &["best"]);
        assert!(!look.needs_download());
        assert_eq!(look.title(), "Downloading");
        assert_eq!(look.action().as_deref(), Some("Stop download"));
    }
}
