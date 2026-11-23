//! The messages Iris leaves by your text (`ToastView`, `Message`): a pen mark for the kind (a
//! pink tick for done, the iris's bloom for working or a quiet note, a red ring round "?" for a
//! problem), Iris's word in brush over the pink marker (red, no marker, for a problem), a plain
//! detail, an app's name as an ink label, keys as keycaps, and a quieter line under it. On a
//! slip of paper with its tooth and shadow (`notePaper`). And the note hanging from the menu bar
//! flower when setup finishes (`MenuBarHintView`).
//!
//! `sheet` draws every message the design check draws (`Iris --toast-states`), each over a dark
//! and a light app as the Swift PNGs have it.
//!
//! The Rune copy's messages in GPUI: the marks, rings and crosses are printed pictures, the
//! paper's soft shadow is a GPUI box shadow, the words GPUI's text.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::PI;

use gpui::{BoxShadow, Div, InteractiveElement, ParentElement, Role, StatefulInteractiveElement, Styled, div, point};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Stroke};

use crate::parts::*;
use crate::pen::{self, Pen};
use crate::riso::{self, Img as Pixels, Plate, Rgb8, Rng, col, hue, ink};

/// `Toast.Kind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Success,
    Neutral,
    Warning,
}

/// `Message`: everything a message by your text says. The words are the Swift app's.
#[derive(Clone)]
pub struct Message {
    /// Iris's word, lowercase and short ("replaced", "nothing to read").
    pub lead: String,
    /// The plain sentence after it; `keys` are drawn as keycaps at its end.
    pub detail: Option<String>,
    pub keys: Vec<String>,
    /// A name printed as an ink label after the detail ("1Password").
    pub label: Option<String>,
    /// An app icon shown crossed out in brush instead of the usual mark (Iris is off there).
    pub crossed: Option<Pixels>,
    /// A second, quieter line under the rest.
    pub extra: Option<String>,
    pub kind: Kind,
    /// Seconds it stays up (a problem stays until the next click or key, up to a minute).
    pub hold: f32,
}

/// `DS.Motion.toastHold`.
const HOLD: f32 = 2.4;

impl Message {
    fn new(lead: &str, detail: Option<&str>, kind: Kind) -> Message {
        Message { lead: lead.into(), detail: detail.map(Into::into), keys: vec![], label: None, crossed: None, extra: None, kind, hold: HOLD }
    }

    /// News from Iris itself (Windows says the menu bar note's news this way, by the clock):
    /// `lead`, the sentence and its keys, held `hold` seconds.
    pub fn news(lead: &str, detail: &str, keys: Vec<String>, hold: f32) -> Message {
        Message { keys, hold, ..Message::new(lead, Some(detail), Kind::Neutral) }
    }

    /// How long it stays, in seconds.
    #[allow(dead_code)]
    pub fn stays(&self) -> f32 {
        if self.kind == Kind::Warning { 60.0 } else { self.hold }
    }

    /// After a paste the app's own ⌘Z takes it back: the message says so when `undo_keys`.
    pub fn replaced(undo_keys: bool) -> Message {
        Self::undoable("replaced", undo_keys)
    }

    /// Quick Fix pasted its fix.
    pub fn fixed(words: usize, undo_keys: bool) -> Message {
        Self::undoable(&if words == 1 { "fixed 1 word".into() } else { format!("fixed {words} words") }, undo_keys)
    }

    fn undoable(lead: &str, undo_keys: bool) -> Message {
        let mut m = Self::new(lead, undo_keys.then_some("Undo with"), Kind::Success);
        if undo_keys {
            m.keys = crate::keys::command("Z");
        }
        m
    }

    pub fn looks_good() -> Message {
        Self::new("looks good", Some("Nothing to fix."), Kind::Success)
    }

    /// Copy on text that can't be edited where it is.
    pub fn copied() -> Message {
        Message { keys: crate::keys::command("V"), ..Self::new("copied", Some("Paste it where you need it."), Kind::Success) }
    }

    /// You put a word back after a replace, so Iris learned it.
    pub fn kept(word: &str) -> Message {
        Self::new("kept", Some(&format!("Iris won't change \u{201C}{word}\u{201D} again.")), Kind::Success)
    }

    /// Quick Fix started; stays until its answer replaces it.
    pub fn checking() -> Message {
        Message { hold: 120.0, ..Self::new("checking…", Some("Reading the box you're in."), Kind::Neutral) }
    }

    /// Quick Fix finished after the box changed, so it pasted nothing.
    pub fn left_alone() -> Message {
        Message { hold: 4.0, ..Self::new("left it alone", Some("Your text changed while Iris was checking, so it pasted nothing."), Kind::Neutral) }
    }

    /// The shortcut in an app Iris is turned off in: its name and icon when known.
    pub fn turned_off(name: Option<&str>, icon: Option<Pixels>) -> Message {
        let extra = Some("Turn it back on in Settings › Apps.".to_string());
        match name {
            Some(name) => Message { label: Some(name.into()), crossed: icon, extra,
                ..Self::new("turned off here", Some("Iris doesn't read text in"), Kind::Neutral) },
            None => Message { extra, ..Self::new("turned off here", Some("Iris doesn't read text in this app."), Kind::Neutral) },
        }
    }

    /// The shortcut where there's no text box to read; `keys` open Iris.
    pub fn nothing_to_read(keys: &[&str]) -> Message {
        Message { keys: keys.iter().map(|k| k.to_string()).collect(),
            ..Self::new("nothing to read", Some("Click in a text box or select text, then press"), Kind::Warning) }
    }

    /// The shortcut in a password field.
    pub fn password() -> Message {
        Self::new("password field", Some("Iris never reads those."), Kind::Neutral)
    }

    /// A failed check or download, in the panel's words (`ProblemNote`): its lead and detail.
    pub fn problem(lead: &str, detail: &str) -> Message {
        Self::new(lead, Some(detail), Kind::Warning)
    }
}

/// Every state `Iris --toast-states` draws, by its name there.
pub fn states() -> Vec<(&'static str, Message)> {
    let (app, icon) = turned_off_app();
    vec![
        ("replaced", Message::replaced(true)),
        ("replaced-no-undo", Message::replaced(false)),
        ("fixed-1", Message::fixed(1, true)),
        ("fixed-4", Message::fixed(4, true)),
        ("looks-good", Message::looks_good()),
        ("copied", Message::copied()),
        ("kept", Message::kept("Northwind")),
        ("checking", Message::checking()),
        ("turned-off-app", Message::turned_off(Some(app), icon)),
        ("turned-off-unknown", Message::turned_off(None, None)),
        ("left-alone", Message::left_alone()),
        ("password", Message::password()),
        ("nothing-to-read", Message::nothing_to_read(&crate::keys::caps(crate::keys::open_default()).iter().map(String::as_str).collect::<Vec<_>>())),
        ("empty", Message::problem("nothing to fix", "The box is empty. Type something, then try again.")),
        ("too-long", Message::problem("that's a lot", "Iris checks about 1000 words at once. Select a part, then press the keys again.")),
        ("no-model", Message::problem("i need my model first", crate::words::w("Open the menu bar flower and finish setup to download it."))),
        ("no-disk", Message::problem("no room for my model", crate::words::w("The model needs 6.2 GB free, and this Mac has 3.1 GB. Free up some space."))),
        ("offline", Message::problem("no internet", "The model downloads once and needs the internet for that. It picks up where it stopped.")),
        ("unknown", Message::problem("couldn't fix", "Something got in the way and Iris left your text alone. Try again in a moment.")),
    ]
}

/// The design check's app: 1Password when it's installed, else Notes, with its real icon.
fn turned_off_app() -> (&'static str, Option<Pixels>) {
    if std::path::Path::new("/Applications/1Password.app").exists() {
        ("1Password", app_icon("/Applications/1Password.app"))
    } else {
        ("Notes", app_icon("/System/Applications/Notes.app"))
    }
}

/// An app's icon from its path, drawn once at 64 pixels (`NSImage.flattened`: 32 points at 2×).
/// Kept in `riso`'s print cache, so an icon it lets go of also leaves GPUI's texture atlas; the
/// icon is read from disk only on a miss (Settings' demo asks every frame).
pub fn app_icon(path: &str) -> Option<Pixels> {
    // Paths with no icon, so they aren't asked about again every frame.
    thread_local! { static MISSING: RefCell<std::collections::HashSet<String>> = RefCell::new(std::collections::HashSet::new()); }
    if MISSING.with(|m| m.borrow().contains(path)) {
        return None;
    }
    let plate = Plate { w: 64.0, h: 64.0, scale: 1.0 };
    let found = riso::try_cached(format!("app-icon-{path}"), &plate, || crate::icon_of_app(path, 64));
    if found.is_none() {
        MISSING.with(|m| {
            let mut m = m.borrow_mut();
            if m.len() >= 128 {
                m.clear();
            }
            m.insert(path.to_string());
        });
    }
    found
}

// MARK: - Measures

/// `DS.Font.brush(24)` as the message sets it: its line exactly as tall as the face says.
const LEAD_SIZE: f32 = 24.0;
fn lead_h() -> f32 {
    px_round(LEAD_SIZE * BRUSH_LINE)
}

/// A line's height on the pixel grid (at 2×), as SwiftUI lays a line of text out.
fn px_round(v: f32) -> f32 {
    (v * 2.0).round() / 2.0
}
fn lead_style() -> Ts {
    brush_line(LEAD_SIZE, lead_h())
}

/// `DS.Font.brush(size)` with its line `line_height` tall, kerned as Core Text kerns it (the
/// shared `brush` turns kerning off; in a message Swift's "that's" and "up here" are kerned).
pub fn brush_line(size: f32, line_height: f32) -> Ts {
    brush(size).line(line_height)
}

/// The width of brush words as Core Text sets them: their shaped advances (Caveat swaps in
/// contextual alternates, so the raw advances `brush_w` adds up are off by a point or two),
/// widened as `brush_w` widens them for ink overhanging the ends.
pub fn shaped_brush_w(s: &str, size: f32) -> f32 {
    thread_local! { static K: RefCell<HashMap<(String, u32), f32>> = RefCell::new(HashMap::new()); }
    let key = (s.to_string(), size.to_bits());
    if let Some(w) = K.with(|m| m.borrow().get(&key).copied()) {
        return w;
    }
    let face = rustybuzz::Face::from_slice(CAVEAT, 0).unwrap();
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(s);
    let out = rustybuzz::shape(&face, &[], buf);
    let shaped = out.glyph_positions().iter().map(|p| p.x_advance as f32).sum::<f32>() * size / face.units_per_em() as f32;
    let font = skrifa::FontRef::new(CAVEAT).unwrap();
    use skrifa::MetadataProvider;
    let gm = font.glyph_metrics(skrifa::instance::Size::new(size), skrifa::instance::LocationRef::default());
    let cm = font.charmap();
    let raw: f32 = s.chars().filter_map(|c| cm.map(c)).map(|g| gm.advance_width(g).unwrap_or(0.0)).sum();
    let w = brush_w(s, size) + shaped - raw;
    K.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 512 {
            m.clear();
        }
        m.insert(key, w);
    });
    w
}

/// Past this the detail wraps instead of running off a small screen.
const DETAIL_W: f32 = 460.0;
const BODY_LINE: f32 = 12.5 * MONO_LINE;
/// `caption`'s line, 13.2, on the pixel grid.
const CAPTION_LINE: f32 = 13.0;
const LABEL_SIZE: f32 = 11.5;
const PAD: (f32, f32) = (16.0, 10.0);

/// The detail's lines, broken at spaces as the monospaced face breaks them in `DETAIL_W`, and
/// as Core Text's standard strategy does, never leaving one word alone on the last line.
fn detail_lines(s: &str) -> Vec<String> {
    let per = (DETAIL_W / (12.5 * MONO_ADVANCE)).floor() as usize;
    let mut lines: Vec<Vec<&str>> = vec![vec![]];
    for word in s.split(' ') {
        let line = lines.last_mut().unwrap();
        let used: usize = line.iter().map(|w| w.chars().count() + 1).sum::<usize>();
        if line.is_empty() || used + word.chars().count() <= per {
            line.push(word);
        } else {
            lines.push(vec![word]);
        }
    }
    let n = lines.len();
    if n > 1 && lines[n - 1].len() == 1 && lines[n - 2].len() > 2 {
        let moved = lines[n - 2].pop().unwrap();
        lines[n - 1].insert(0, moved);
    }
    lines.into_iter().map(|l| l.join(" ")).collect()
}

fn detail_w(s: &str) -> f32 {
    px_ceil(mono_w(s, 12.5, 0.0)).min(DETAIL_W)
}

/// SwiftUI sizes a line of text up to the next pixel (at 2×, where the references are drawn).
fn px_ceil(v: f32) -> f32 {
    (v * 2.0 - 0.01).ceil() / 2.0
}

fn lead_w(s: &str) -> f32 {
    px_ceil(shaped_brush_w(s, LEAD_SIZE))
}

fn label_size(name: &str) -> (f32, f32) {
    (px_ceil(mono_w(name, LABEL_SIZE, 0.0)) + 2.0 * LABEL_SIZE * 0.62, px_round(LABEL_SIZE * MONO_LINE) + 2.0 * LABEL_SIZE * 0.26)
}

fn key_w(k: &str) -> f32 {
    (10.0 + k.chars().count() as f32 * 7.0).max(20.0)
}

fn mark_size(m: &Message) -> (f32, f32) {
    if m.crossed.is_some() {
        return (30.0, 30.0);
    }
    match m.kind {
        Kind::Success => (18.0, 14.0),
        Kind::Neutral => (30.0, 24.0),
        Kind::Warning => (24.0, 24.0),
    }
}

fn has_details(m: &Message) -> bool {
    m.detail.is_some() || !m.keys.is_empty() || m.label.is_some() || m.extra.is_some()
}

/// The detail row's (width, height): the sentence, the label, the keys, 6 apart.
fn detail_row(m: &Message) -> (f32, f32) {
    let mut parts: Vec<(f32, f32)> = vec![];
    if let Some(d) = &m.detail {
        parts.push((detail_w(d), detail_lines(d).len() as f32 * BODY_LINE));
    }
    if let Some(l) = &m.label {
        parts.push(label_size(l));
    }
    for k in &m.keys {
        parts.push((key_w(k), 19.0));
    }
    let w = parts.iter().map(|p| p.0).sum::<f32>() + 6.0 * (parts.len().max(1) - 1) as f32;
    (w, parts.iter().map(|p| p.1).fold(0.0, f32::max))
}

fn details_size(m: &Message) -> (f32, f32) {
    let (mut w, mut h) = detail_row(m);
    if let Some(x) = &m.extra {
        w = w.max(px_ceil(mono_w(x, 11.0, 0.0)));
        h += 4.0 + CAPTION_LINE;
    }
    (w, h)
}

/// The message's paper, without its shadow: (width, height) in points.
pub fn size(m: &Message) -> (f32, f32) {
    let (mw, mh) = mark_size(m);
    let mut w = mw + 12.0 + lead_w(&m.lead);
    let mut h = mh.max(lead_h());
    if has_details(m) {
        let (dw, dh) = details_size(m);
        w += 12.0 + dw;
        h = h.max(dh);
    }
    (w + 2.0 * PAD.0, h + 2.0 * PAD.1)
}

// MARK: - The message

/// `ToastView`: the message on its paper, `size(m)` big; its shadow reaches out of the box.
pub fn toast(m: &Message, scale: f32) -> Div {
    let (w, h) = size(m);
    let lw = lead_w(&m.lead);
    let lh = lead_h();
    let mut lead = sized(lw, lh);
    let colour = if m.kind == Kind::Warning { hue::RED_TEXT } else { hue::I12 };
    if m.kind != Kind::Warning {
        // The pink marker under good news and quiet notes, its bottom 2 points over the line's.
        let (marker, pad) = marker_swipe(lw + 6.0, 6.0, 6, -1.2, scale);
        lead = lead.child(at(marker, -3.0, lh - 8.0 - 2.0 - pad));
    }
    lead = lead.child(at(t(m.lead.clone(), lead_style(), colour), 0.0, 0.0));
    let mut r = div().flex().flex_row().gap(z(12.0)).items_center().child(mark(m, scale)).child(lead);
    if has_details(m) {
        let mut line = div().flex().flex_row().gap(z(6.0)).items_center();
        if let Some(d) = &m.detail {
            let lines = detail_lines(d);
            line = line.child(if lines.len() > 1 {
                lines.iter().fold(div().flex().flex_col().w(z(DETAIL_W)), |c, l| c.child(t(l.clone(), body(), hue::I11)))
            } else {
                div().w(z(detail_w(d))).child(t(d.clone(), body(), hue::I11))
            });
        }
        if let Some(l) = &m.label {
            line = line.child(ink_label(l, LABEL_SIZE, scale));
        }
        for k in &m.keys {
            line = line.child(keycap(k, scale));
        }
        let mut words = div().flex().flex_col().gap(z(4.0)).items_start().child(line);
        if let Some(x) = &m.extra {
            words = words.child(t(x.clone(), caption(), hue::I10));
        }
        r = r.child(words);
    }
    let e = note_paper(w, h, scale)
        .child(at(r.h(z(h - 2.0 * PAD.1)), PAD.0, PAD.1))
        .child(at(printed(w, h, &riso::grain(w, h, scale, 0.22)), 0.0, 0.0));
    div().child(div().id("message").role(Role::Label).aria_label(spoken(m)).child(e))
}

/// What VoiceOver reads: the whole message as a sentence or two.
pub fn spoken(m: &Message) -> String {
    let middle = [m.detail.clone(), m.label.clone()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    [m.lead.clone(), middle, m.extra.clone().unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(". ")
}

/// `notePaper` without the words: paper `w`×`h` with the paper shadow under it (`paperShadow(spread:
/// 6, drop: 7)`: a long soft shadow pulled in at the sides, and a hairline of contact).
fn note_paper(w: f32, h: f32, _scale: f32) -> Div {
    paper_shadow(sized(w, h), w, h, 6.0, 7.0, 0.18).child(at(sized(w, h).bg(col(ink::PAPER)), 0.0, 0.0))
}

/// `paperShadow`: a sheet lying on the table. Added to `e`, which is `w`×`h`. A blurred black
/// sheet is a GPUI box shadow of a clear box (its blur radius is the Gaussian's sigma, as SwiftUI's is).
pub fn paper_shadow(e: Div, w: f32, h: f32, spread: f32, drop: f32, opacity: f32) -> Div {
    let sheet = |w: f32, h: f32, a: f32, blur: f32| sized(w, h).shadow(vec![BoxShadow {
        color: gpui::black().opacity(a), offset: point(z(0.0), z(0.0)), blur_radius: z(blur), spread_radius: z(0.0), inset: false }]);
    e.child(at(sheet(w - 2.0 * spread, h, opacity, drop), spread, drop))
        .child(at(sheet(w, h, 0.10, 0.6), 0.0, 1.0))
}

/// `ToastMark`, or the crossed-out app.
fn mark(m: &Message, scale: f32) -> Div {
    if let Some(icon) = &m.crossed {
        // The app Iris is off in, crossed out with the brush (a choice, so ink, not red).
        return sized(30.0, 30.0)
            .child(at(printed(26.0, 26.0, icon), 2.0, 2.0))
            .child(at(brush_cross(5, 34.0, 32.0, scale), -2.0, -1.0));
    }
    match m.kind {
        Kind::Success => sized(18.0, 14.0).child(drawn(18.0, 14.0, pen::hand_tick(18.0, 14.0), Some(2.4), col(ink::PINK_TEXT))),
        Kind::Neutral => sized(30.0, 24.0).child(at(printed(30.0, 30.0, &bud(scale)), 0.0, -3.0)),
        Kind::Warning => {
            let q = brush(20.0);
            let qh = line_box(20.0, BRUSH_LINE);
            let qw = brush_w("?", 20.0);
            sized(24.0, 24.0)
                .child(at(t("?", q, hue::RED_TEXT), (24.0 - qw) / 2.0, (24.0 - qh) / 2.0))
                .child(at(brush_ring(ink::RED, 9, 24.0, 24.0, 1.0, scale), 0.0, 0.0))
        }
    }
}

/// The iris's bloom, 30 points (`RisoArt.flower(points: 30, crop: .bloom)`).
fn bud(scale: f32) -> Pixels {
    let plate = Plate { w: 30.0, h: 30.0, scale };
    riso::cached("bud-30".into(), &plate, || crate::rig::flower(30.0, crate::rig::Crop::Bloom, true, scale).2)
}

/// `InkLabel`: a name printed as a solid ink strip with paper-coloured type.
pub fn ink_label(name: &str, size: f32, scale: f32) -> Div {
    let (w, h) = (px_ceil(mono_w(name, size, 0.0)) + 2.0 * size * 0.62, px_round(size * MONO_LINE) + 2.0 * size * 0.26);
    let plate = Plate { w, h, scale };
    let seed = name.chars().count() as i64 + 3;
    let px = riso::cached(format!("ink-label-{w}-{h}-{seed}"), &plate, || {
        let mut m = plate.mask();
        m.fill_path(&PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, w, h).unwrap()), FillRule::Winding, false, plate.transform());
        riso::print(&plate, ink::INK, seed, &m, 1.0)
    });
    // Martian at 11.5 has no cut of its own: the 430 cut, set at 11.5.
    sized(w, h)
        .child(at(printed(w, h, &px), 0.0, 0.0))
        .child(at(t(name, martian(size, 430), ink::PAPER), size * 0.62, size * 0.26))
}

// MARK: - Brush marks

/// `InkRibbon(.loop)`: the brush loop's ribbon in a `w`×`h` box, `drawn` of it (0…1).
fn ribbon_loop(seed: i64, w: f32, h: f32, width: f32, start_taper: f32, end_taper: f32, fray: bool, drawn: f32) -> Option<Pen> {
    let (ix, iy) = (w * 0.05 + width, h * 0.05 + width);
    let (rw, rh) = (w - 2.0 * ix, h - 2.0 * iy);
    let (mx, my) = (ix + rw / 2.0, iy + rh / 2.0);
    let mut rng = Rng::new(seed);
    let start = -2.5 + rng.next() * 0.3;
    let sweep = 2.0 * std::f64::consts::PI + 0.55;
    let phase = rng.next() * 6.3;
    let all: Vec<[f32; 2]> = (0..=60).map(|i| {
        let t = i as f64 / 60.0;
        let a = start + sweep * t;
        let grow = 1.0 + 0.1 * t;
        let o = (a * 3.0 + phase).sin();
        let rx = rw as f64 / 2.0 * grow + o;
        let ry = rh as f64 / 2.0 * grow + o;
        [(mx as f64 + rx * a.cos()) as f32, (my as f64 + ry * a.sin()) as f32]
    }).collect();
    let keep = ((all.len() as f32 * drawn.clamp(0.0, 1.0)).round() as usize).max(0);
    let c = &all[..keep.min(all.len())];
    if c.len() <= 2 {
        return None;
    }
    let mut rng = Rng::new(c.len() as i64 * 7);
    let mut left = vec![];
    let mut right = vec![];
    for i in 0..c.len() {
        let a = c[i.saturating_sub(1)];
        let b = c[(i + 1).min(c.len() - 1)];
        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
        let n = [-(b[1] - a[1]) / len, (b[0] - a[0]) / len];
        let t = i as f32 / (all.len() - 1) as f32;
        let press = (t / 0.12).min(1.0) * (1.0 - start_taper) + start_taper;
        let lift = ((1.0 - t) / 0.3).min(1.0) * (1.0 - end_taper) + end_taper;
        let half = width / 2.0 * press.min(lift) * (1.0 + (rng.f() - 0.5) * 0.14);
        left.push([c[i][0] + n[0] * half, c[i][1] + n[1] * half]);
        right.push([c[i][0] - n[0] * half, c[i][1] - n[1] * half]);
    }
    let mut pts = left.clone();
    if fray && c.len() == all.len() {
        let e = c.len() - 1;
        let (a, b) = (c[e - 1], c[e]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
        let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        for k in 0..=8 {
            let u = k as f32 / 8.0;
            let base = [left[e][0] + (right[e][0] - left[e][0]) * u, left[e][1] + (right[e][1] - left[e][1]) * u];
            let out = if k % 2 == 1 { width * (0.3 + rng.f() * 0.7) } else { 0.0 };
            pts.push([base[0] + d[0] * out, base[1] + d[1] * out]);
        }
    }
    pts.extend(right.into_iter().rev());
    Some(Pen::poly(&pts))
}

/// `BrushRing`: a brush loop round a `w`×`h` frame, printed through the screen, drawn in to `drawn`.
pub fn brush_ring(rgb: Rgb8, seed: i64, w: f32, h: f32, drawn: f32, scale: f32) -> Div {
    let keep = ((61.0 * drawn.clamp(0.0, 1.0)).round() as usize).min(61);
    if keep <= 2 {
        return sized(w, h);
    }
    let weight = (w.min(h) * 0.062).max(2.2);
    let (bw, bh) = ((w + 2.0 * weight) / 0.9, (h + 2.0 * weight) / 0.9);
    let plate = Plate { w: bw, h: bh, scale };
    let make = || match ribbon_loop(seed, bw, bh, weight, 0.8, 0.5, true, drawn) {
        Some(p) => riso::print(&plate, rgb, seed + 3, &plate.fill(&p.skia().unwrap()), 1.0),
        None => vec![0; (plate.px().0 * plate.px().1 * 4) as usize],
    };
    let px = if keep == 61 {
        riso::cached(format!("ring-{seed}-{rgb:?}-full"), &plate, make)
    } else {
        let (pw, ph) = plate.px();
        riso::frame_image(pw, ph, make())
    };
    sized(w, h).child(at(printed(bw, bh, &px), (w - bw) / 2.0, (h - bh) / 2.0))
}

/// `DryBrushStroke` from `a` to `b` (points): one flat stroke with frayed ends.
fn dry_stroke(seed: i64, a: [f32; 2], b: [f32; 2], width: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy).max(1.0);
    let dir = [dx / length, dy / length];
    let n = [-dir[1], dir[0]];
    let w = length * width;
    let bend = (rng.f() - 0.5) * length * 0.07;
    let steps = 22;
    let mut left = vec![];
    let mut right = vec![];
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let off = (t * PI).sin() * bend;
        let c = [a[0] + dx * t + n[0] * off, a[1] + dy * t + n[1] * off];
        let half = w / 2.0 * (1.12 - 0.22 * t);
        let l = half * (1.0 + (rng.f() - 0.5) * 0.16);
        let r = half * (1.0 + (rng.f() - 0.5) * 0.16);
        left.push([c[0] + n[0] * l, c[1] + n[1] * l]);
        right.push([c[0] - n[0] * r, c[1] - n[1] * r]);
    }
    let mut fray = |from: [f32; 2], to: [f32; 2], out_dir: [f32; 2], reach: f32| -> Vec<[f32; 2]> {
        (0..=18).map(|k| {
            let u = k as f32 / 18.0;
            let base = [from[0] + (to[0] - from[0]) * u, from[1] + (to[1] - from[1]) * u];
            let out = if k % 2 == 1 { reach * (0.35 + rng.f() * 0.65) } else { reach * rng.f() * 0.15 };
            [base[0] + out_dir[0] * out, base[1] + out_dir[1] * out]
        }).collect()
    };
    let mut pts = left.clone();
    pts.extend(fray(left[steps], right[steps], dir, w * 0.32));
    pts.extend(right.iter().rev().skip(1).copied());
    pts.extend(fray(right[0], left[0], [-dir[0], -dir[1]], w * 0.22));
    Pen::poly(&pts)
}

/// `DryBrushX`: two dry-brush strokes printed through the screen, with bristle streaks and
/// pits cut out of them where the brush ran dry.
pub fn dry_brush_x(rgb: Rgb8, seed: i64, width: f32, dryness: f32, w: f32, h: f32, scale: f32) -> Div {
    const STROKES: [([f32; 2], [f32; 2]); 2] = [([0.03, 0.08], [0.97, 0.95]), ([0.96, 0.03], [0.05, 0.93])];
    let plate = Plate { w, h, scale };
    let px = riso::cached(format!("brush-x-{seed}-{rgb:?}-{width}-{dryness}"), &plate, || {
        let at = |u: [f32; 2]| [u[0] * w, u[1] * h];
        let mut shape = plate.mask();
        for (k, (f, to)) in STROKES.iter().enumerate() {
            let p = dry_stroke(seed + k as i64 * 7, at(*f), at(*to), width);
            shape.fill_path(&p.skia().unwrap(), FillRule::Winding, true, plate.transform());
        }
        let mut pm = riso::opaque(&plate, 4.0);
        if dryness > 0.0 {
            let mut rng = Rng::new(seed * 31 + 5);
            let cut = |pm: &mut Pixmap, path: &tiny_skia::Path, alpha: f32, width: Option<f32>| {
                let mut paint = Paint::default();
                paint.set_color(tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, alpha.clamp(0.0, 1.0)).unwrap());
                paint.blend_mode = tiny_skia::BlendMode::DestinationOut;
                paint.anti_alias = true;
                match width {
                    Some(width) => {
                        let st = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
                        pm.stroke_path(path, &paint, &st, plate.transform(), None);
                    }
                    None => pm.fill_path(path, &paint, FillRule::Winding, plate.transform(), None),
                }
            };
            for (f, to) in STROKES {
                let (a, b) = (at(f), at(to));
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
                let n = [-d[1], d[0]];
                let sw = len * width;
                for _ in 0..(6.0 + dryness * 10.0) as i32 {
                    let across = (rng.f() - 0.5) * sw * 0.9;
                    let start = if rng.f() < 0.7 { 0.45 + rng.f() * 0.4 } else { rng.f() * 0.7 };
                    let end = (start + 0.08 + rng.f() * (0.15 + dryness * 0.25)).min(1.03);
                    let mut p = Pen::new();
                    p.m(a[0] + d[0] * len * start + n[0] * across, a[1] + d[1] * len * start + n[1] * across);
                    p.l(a[0] + d[0] * len * end + n[0] * across, a[1] + d[1] * len * end + n[1] * across);
                    let open = rng.f() < 0.3;
                    let alpha = if open { 0.9 } else { 0.25 + rng.f() * 0.3 };
                    let lw = (sw * if open { 0.05 } else { 0.035 + rng.f() * 0.03 }).max(0.4);
                    if let Some(path) = p.skia() {
                        cut(&mut pm, &path, alpha, Some(lw));
                    }
                }
                for _ in 0..(10.0 + dryness * 40.0) as i32 {
                    let along = rng.f();
                    let across = (rng.f() - 0.5) * sw * 0.85;
                    let c = [a[0] + d[0] * len * along + n[0] * across, a[1] + d[1] * len * along + n[1] * across];
                    let r = (sw * (0.02 + rng.f() * 0.03)).max(0.35);
                    let alpha = 0.5 + rng.f() * 0.5;
                    if let Some(path) = PathBuilder::from_circle(c[0], c[1], r) {
                        cut(&mut pm, &path, alpha, None);
                    }
                }
            }
        }
        let mask = riso::intersect(&shape, &riso::alpha_mask(&pm));
        riso::print_with(&plate, rgb, seed + 40, 1.0, 1.0, &mask, 1.0, |c| c)
    });
    sized(w, h).child(printed(w, h, &px))
}

/// `BrushCross`: the brush X in ink, to say "not here".
pub fn brush_cross(seed: i64, w: f32, h: f32, scale: f32) -> Div {
    dry_brush_x(ink::INK, seed, 0.12, 0.4, w, h, scale)
}

// MARK: - Translucent pen lines

/// A pen line in `rgb` at `alpha`, laid once (stroked segment by segment, a translucent line
/// would darken where they meet). `w`×`h` is the box the path is drawn in; the
/// line can spill `pad` points past it.
#[allow(clippy::too_many_arguments)]
pub fn faint_line(key: &str, pen: &Pen, w: f32, h: f32, width: f32, rgb: Rgb8, alpha: f32, scale: f32) -> Div {
    let pad = 3.0 + width;
    let plate = Plate { w: w + 2.0 * pad, h: h + 2.0 * pad, scale };
    let px = riso::cached(format!("faint-{key}-{width}-{rgb:?}-{alpha}"), &plate, || {
        let (pw, ph) = plate.px();
        let mut pm = Pixmap::new(pw, ph).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(rgb.0, rgb.1, rgb.2, 255);
        paint.anti_alias = true;
        let st = Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        if let Some(p) = pen.translated(pad, pad).skia() {
            pm.stroke_path(&p, &paint, &st, plate.transform(), None);
        }
        let mut out = Vec::with_capacity((pw * ph * 4) as usize);
        for c in pm.pixels() {
            let a = c.alpha();
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                out.extend_from_slice(&[rgb.0, rgb.1, rgb.2, (a as f32 * alpha).round() as u8]);
            }
        }
        out
    });
    sized(w, h).child(at(printed(plate.w, plate.h, &px), -pad, -pad))
}

// MARK: - The sheet

/// Every message as `Iris --toast-states` draws it: each over a dark app and a light one, 16
/// points round it, at least 520 wide, one under another.
#[allow(dead_code)]
pub fn sheet(scale: f32) -> Div {
    let mut c = div().flex().flex_col().items_start();
    for (_, m) in states() {
        c = c.child(state_block(&m, scale));
    }
    c
}

/// One state as its Swift PNG has it.
pub fn state_block(m: &Message, scale: f32) -> Div {
    let (w, _) = size(m);
    let width = (w + 28.0 + 60.0).max(520.0);
    let mut c = div().flex().flex_col().w(z(width));
    for ground in [(0x1F, 0x1C, 0x27), (0xF0, 0xEB, 0xE6)] {
        c = c.child(div().w(z(width)).p(z(16.0)).bg(col(ground)).child(toast(m, scale)));
    }
    c
}

// MARK: - The menu bar note

/// The note under the menu bar flower as the Swift snapshot `after-menubar-hint` draws it: a
/// strip of menu bar over a desktop gradient, 520 wide, the note's tail under x = 382.
#[allow(dead_code)]
pub fn hint_sheet(scale: f32) -> Div {
    let (w, h) = hint_size(&HINT_KEYS, "find me up here", "In any text box, press");
    let ground = gpui::linear_gradient(180.0, gpui::linear_color_stop(col((0x6A, 0x4F, 0x8C)), 0.0), gpui::linear_color_stop(col((0x1F, 0x6F, 0x78)), 1.0));
    // The flower in a dark menu bar (white crayon), and the clock beside it.
    let png = crate::menubar::png(true, scale);
    let mark = image::load_from_memory(&png).map(|i| i.to_rgba8()).ok();
    let mut bar = sized(520.0, 26.0).bg(col((0x2B, 0x2A, 0x33)));
    if let Some(m) = mark {
        let (pw, ph) = (m.width(), m.height());
        bar = bar.child(at(printed(18.0, 18.0, &riso::cached("menubar-mark-dark".into(), &Plate { w: 18.0, h: 18.0, scale }, || m.into_raw())), 382.0 - 9.0, 4.0));
        let _ = (pw, ph);
    }
    let clock = div().font_family(".SystemUIFont").text_size(z(13.0)).line_height(z(16.0)).text_color(gpui::white()).whitespace_nowrap().child("Wed 5 Nov  9:41");
    bar = bar.child(at(clock, 406.5, 5.0));
    sized(520.0, 26.0 + 2.0 + h + 30.0).bg(ground)
        .child(at(bar, 0.0, 0.0))
        .child(at(hint(scale), 382.0 - w / 2.0, 28.0))
}

const TAIL: f32 = 11.0;

/// `NoteShape`: a hand-cut slip with a tail at the top pointing at the flower.
fn note_shape(w: f32, h: f32, tail_offset: f32, seed: i64) -> Pen {
    let mut rng = Rng::new(seed);
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

/// `MenuBarHintView`'s size: (width, height), its tail included.
pub fn hint_size(keys: &[&str], lead: &str, detail: &str) -> (f32, f32) {
    let lw = lead_w(lead);
    let row_w = mono_w(detail, 12.5, 0.0) + keys.iter().map(|k| 6.0 + key_w(k)).sum::<f32>();
    let row_h = BODY_LINE.max(if keys.is_empty() { 0.0 } else { 19.0 });
    (lw.max(row_w) + 36.0, 14.0 + TAIL + lead_h() + 7.0 + row_h + 14.0)
}

/// `MenuBarHintView` as setup's end shows it: "find me up here", and the keys that open Iris
/// (⌃ G, as the Swift snapshot has them), the tail in the middle.
pub fn hint(scale: f32) -> Div {
    hint_with(&HINT_KEYS, None, None, 0.0, scale)
}

const HINT_KEYS: [&str; 2] = ["⌃", "G"];

/// `MenuBarHintView`: where Iris lives, said once when setup finishes. `lead` and `detail` are
/// "find me up here" and "In any text box, press" unless other news comes from the menu bar
/// ("model's in"); `tail_offset` slides the tail to keep pointing at the flower near an edge.
pub fn hint_with(keys: &[&str], lead: Option<&str>, detail: Option<&str>, tail_offset: f32, scale: f32) -> Div {
    let lead = lead.unwrap_or("find me up here");
    let detail = detail.unwrap_or("In any text box, press");
    let (w, h) = hint_size(keys, lead, detail);
    let lw = lead_w(lead);
    let lh = lead_h();
    let (marker, pad) = marker_swipe(lw + 6.0, 7.0, 4, -1.2, scale);
    let title = sized(lw, lh)
        .child(at(marker, -3.0, lh - 9.0 - 3.0 - pad))
        .child(at(t(lead, lead_style(), hue::I12), 0.0, 0.0));
    let mut line = div().flex().flex_row().gap(z(6.0)).items_center().child(t(detail, body(), hue::I11));
    for k in keys {
        line = line.child(keycap(k, scale));
    }
    let words = div().flex().flex_col().gap(z(7.0)).items_start().child(title).child(line);
    let (paper, room) = hint_paper(w, h, tail_offset, scale);
    let spoken = format!("{lead}. {detail}") + &if keys.is_empty() { String::new() } else { format!(" {}", keys.join(" ")) };
    let e = sized(w, h)
        .child(at(printed(w + 2.0 * room, h + 2.0 * room, &paper), -room, -room))
        .child(at(words, 18.0, 14.0 + TAIL));
    div().child(div().id("hint").role(Role::Label).aria_label(spoken).child(e))
}

/// The hint's paper, `pad` points of room round it for the shadow: the shadow (pulled in 10 at
/// the sides, dropped 9, blurred 9), the paper, its cut edge at 18%, and the tooth over it.
fn hint_paper(w: f32, h: f32, tail_offset: f32, scale: f32) -> (Pixels, f32) {
    let pad = 40.0;
    let plate = Plate { w: w + 2.0 * pad, h: h + 2.0 * pad, scale };
    let px = riso::cached(format!("hint-{w}-{h}-{tail_offset}"), &plate, || {
        let (pw, ph) = plate.px();
        let n = (pw * ph) as usize;
        let shape = note_shape(w, h, tail_offset, 23);
        let cover = |path: &Pen, dx: f32, dy: f32| -> Vec<f32> {
            let mut m = Mask::new(pw, ph).unwrap();
            m.fill_path(&path.translated(pad + dx, pad + dy).skia().unwrap(), FillRule::Winding, true, plate.transform());
            m.data().iter().map(|v| *v as f32 / 255.0).collect()
        };
        let note = cover(&shape, 0.0, 0.0);
        let shadow = blur(&cover(&note_shape(w - 20.0, h, tail_offset, 23), 10.0, 9.0), pw as usize, ph as usize, 9.0 * scale);
        let mut line = Pixmap::new(pw, ph).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        line.stroke_path(&shape.translated(pad, pad).skia().unwrap(), &paint, &Stroke { width: 1.0, ..Default::default() }, plate.transform(), None);
        let grain = riso::grain_rgba(plate.w, plate.h, scale, 0.22);
        let mut out = vec![0u8; n * 4];
        for i in 0..n {
            let mut c = [0.0f32; 3];
            let mut a = shadow[i] * 0.2;
            let mut lay = |rgb: Rgb8, alpha: f32| {
                let oa = alpha + a * (1.0 - alpha);
                if oa > 0.0 {
                    for (k, v) in [rgb.0, rgb.1, rgb.2].iter().enumerate() {
                        c[k] = (*v as f32 * alpha + c[k] * a * (1.0 - alpha)) / oa;
                    }
                }
                a = oa;
            };
            lay(ink::PAPER, note[i]);
            lay(ink::INK, line.pixels()[i].alpha() as f32 / 255.0 * 0.18);
            // The tooth, multiplied over the paper and clipped to it.
            let g = grain[i * 4 + 3] as f32 / 255.0 * note[i];
            let dot = [grain[i * 4] as f32, grain[i * 4 + 1] as f32, grain[i * 4 + 2] as f32];
            for k in 0..3 {
                c[k] = c[k] * (1.0 - g) + c[k] * dot[k] / 255.0 * g;
            }
            out[i * 4..i * 4 + 4].copy_from_slice(&[c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, (a * 255.0).round() as u8]);
        }
        out
    });
    (px, pad)
}

/// Gaussian blur of coverage, `sigma` in pixels (the slip's three box blurs).
fn blur(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    crate::slip::blur(src, w, h, sigma)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paper's size against the Swift messages (`Iris --toast-states`, measured off the
    /// PNGs at 2×): within a point.
    #[test]
    fn sizes_match_swift() {
        let swift = [("replaced", 272.0, 50.0), ("replaced-no-undo", 134.5, 50.0), ("fixed-1", 301.0, 50.0), ("fixed-4", 312.5, 50.0),
            ("looks-good", 282.5, 50.0), ("copied", 398.5, 50.0), ("kept", 405.0, 50.0), ("checking", 384.0, 50.0),
            ("turned-off-app", 476.0, 57.0), ("turned-off-unknown", 499.0, 52.0), ("left-alone", 645.5, 50.0), ("password", 394.5, 50.0),
            ("nothing-to-read", 664.5, 50.0), ("empty", 594.5, 50.0), ("too-long", 633.5, 50.0), ("no-model", 716.5, 50.0),
            ("no-disk", 719.0, 50.0), ("offline", 635.0, 50.0), ("unknown", 636.0, 50.0)];
        for ((id, m), (sid, sw, sh)) in states().iter().zip(swift) {
            assert_eq!(*id, sid);
            let (w, h) = size(m);
            // The Notes app's name is what the Swift check printed; 1Password's label is wider.
            if *id == "turned-off-app" && m.label.as_deref() != Some("Notes") {
                continue;
            }
            assert!((w - sw).abs() <= 1.0 && (h - sh).abs() <= 0.5, "{id}: {w}×{h} against Swift's {sw}×{sh}");
        }
    }
}
