//! Settings › General › When Iris speaks up: the stage over the switches (`NoteDemo`). About four
//! seconds of a message in use: a pen-drawn app window in the middle of a printed sky desktop
//! (a few files lying on it, a Dock along the bottom), someone's text in its box, the keys they
//! press on a scrap of paper (drawn caps that sink one by one and print pink), and the message
//! landing on the box's top edge, where Iris really puts it. Drawn from `t`, seconds into the
//! clip, with the real message (`toast::toast`).
//!
//! The Rune copy's demo in GPUI. GPUI can't turn or scale a box: the message is laid out at 82%
//! (`with_zoom`) and the key faces sink by being laid out smaller; the tape, which leans 7°, is
//! printed whole; the message's 1° lean and the key card's 0.8° are left out.

use gpui::{Div, FontWeight, ParentElement, Styled, div};

use crate::parts::*;
use crate::pen::{self, Pen};
use crate::riso::{self, Img, Plate, Rgb8, col, hue, ink, over};
use crate::settings::{Ctx, Values};
use crate::settings_kit::martian_line;
use crate::toast::{self, Message, faint_line, paper_shadow};

type El = Div;

/// The switch each clip shows (`AppSettings.Note`), in the rows' order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Note {
    Replaced,
    UndoKeys,
    Checking,
    Clean,
    Excluded,
}

pub const NOTES: [Note; 5] = [Note::Replaced, Note::UndoKeys, Note::Checking, Note::Clean, Note::Excluded];

/// One key as the keyboard prints it (`KeyCombo.Cap`): its symbol, if it has one, and its word.
pub type Cap = (Option<&'static str>, &'static str);

/// Quick Fix's keys out of the box (`KeyCombo.quickFixDefault.keycaps`).
pub const QUICK_FIX: [Cap; 4] = [(Some("⌃"), "control"), (Some("⌥"), "option"), (Some("⇧"), "shift"), (None, "G")];

/// Shortcuts to check the keys with: the default, someone's own, a wide key, the longest.
pub const SAMPLE_KEYS: [&[Cap]; 4] = [
    &[(Some("⌃"), "control"), (Some("⌥"), "option"), (Some("⇧"), "shift"), (None, "G")],
    &[(Some("⇧"), "shift"), (Some("⌘"), "command"), (None, "E")],
    &[(Some("⌥"), "option"), (None, "Space")],
    &[(Some("⌃"), "control"), (Some("⌥"), "option"), (Some("⇧"), "shift"), (Some("⌘"), "command"), (None, "F5")],
];

/// The clip's length in seconds (the live stage loops on it).
pub const LENGTH: f64 = 4.4;
/// A moment when the message is on the box, for Reduce Motion.
pub fn landed(note: Note) -> f64 {
    if note == Note::Checking { 2.2 } else { 2.8 }
}

/// The snapshots hold every clip still here (`DesignState.snapshot.noteAt`).
const SNAPSHOT_AT: f64 = 2.6;

/// The app window's width on the desk; the desk fills the column round it.
const WINDOW_W: f32 = 468.0;
/// Room kept at the bottom of the desk for the Dock.
const DOCK_ROOM: f32 = 58.0;
const TOP: f32 = 44.0;
const KEY_H: f32 = 37.0;
const CARD_PAD: (f32, f32, f32) = (13.0, 21.0, 10.0);

/// The stage over the switches, `w` points wide: the first row's clip (After replacing) held
/// where the Swift snapshot holds it.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> El {
    // Live: the clip the stage is on and how far in (`NotesSection.playing`), the person's own keys.
    let (note, t) = v.demo.unwrap_or((Note::Replaced, SNAPSHOT_AT));
    // The undo keys' clip shows the replace message: it plays without it when that's turned off.
    let shows = |n: Note| v.notes[NOTES.iter().position(|x| *x == n).unwrap_or(0)];
    let shows_note = if v.demo.is_none() { v.notes[0] } else { shows(if note == Note::UndoKeys { Note::Replaced } else { note }) };
    let d = Demo { note, t, keys: &v.demo_keys, shows_note, shows_undo_keys: v.notes[1] };
    demo(&d, w, cx.scale)
}

/// One clip at one moment.
pub struct Demo<'a> {
    pub note: Note,
    pub t: f64,
    /// The keys the person presses to fix, shown under the window.
    pub keys: &'a [Cap],
    /// Off when the person turned this message off: the clip plays without it.
    pub shows_note: bool,
    /// Off when the undo keys are turned off: the replace message comes without them.
    pub shows_undo_keys: bool,
}

impl Demo<'_> {
    /// 0 → 1 between two moments, with the strong ease-out.
    fn ramp(&self, a: f64, b: f64) -> f64 {
        let x = ((self.t - a) / (b - a)).clamp(0.0, 1.0);
        1.0 - (1.0 - x).powi(3)
    }
    fn leaving(&self) -> f64 {
        1.0 - self.ramp(3.9, 4.2)
    }
    fn keys_in(&self) -> f64 {
        self.ramp(0.45, 0.7) * self.leaving()
    }
    fn tape_on(&self) -> f64 {
        self.ramp(0.55, 0.75)
    }
    /// The keys go down one after another in Mac order, and come up together before the fix lands.
    fn key_down(&self, i: usize) -> f64 {
        0.8 + i as f64 * 0.09f64.min(0.35 / (self.keys.len().max(2) - 1) as f64)
    }
    const KEYS_UP: f64 = 1.25;
    fn pressed(&self, i: usize) -> f64 {
        let x = ((self.t - self.key_down(i)) / 0.07).clamp(0.0, 1.0);
        let up = ((self.t - Self::KEYS_UP) / 0.12).clamp(0.0, 1.0);
        (1.0 - (1.0 - x).powi(3)) * (1.0 - up)
    }
    /// Three ink flicks over a key as it goes down: up quickly, then fading.
    fn flick(&self, i: usize) -> f64 {
        let x = (self.t - self.key_down(i)) / 0.26;
        if x <= 0.0 || x >= 1.0 {
            return 0.0;
        }
        if x < 0.33 { 1.0 - (1.0 - x / 0.33).powi(3) } else { 1.0 - ((x - 0.33) / 0.67).powi(3) }
    }
    /// The text turns into the fixed version (back to the draft as the clip loops).
    fn fixed_in(&self) -> f64 {
        match self.note {
            Note::Replaced | Note::UndoKeys => self.ramp(1.3, 1.6) * (1.0 - self.ramp(4.1, 4.4)),
            Note::Checking => self.ramp(2.75, 3.05) * (1.0 - self.ramp(4.1, 4.4)),
            Note::Clean | Note::Excluded => 0.0,
        }
    }
    fn slip_in(&self) -> f64 {
        match self.note {
            Note::Replaced | Note::UndoKeys => self.ramp(1.7, 1.9) * self.leaving(),
            Note::Checking => self.ramp(1.2, 1.4) * (1.0 - self.ramp(2.95, 3.15)),
            Note::Clean | Note::Excluded => self.ramp(1.25, 1.45) * self.leaving(),
        }
    }
    /// The pen ring round the undo keys draws itself once the message has landed.
    fn ring_in(&self) -> f64 {
        self.ramp(2.1, 2.6)
    }

    fn draft(&self) -> &'static str {
        match self.note {
            Note::Replaced | Note::UndoKeys | Note::Checking => "hey i checked the pr and its look fine",
            Note::Clean => "Thanks, see you at 3.",
            Note::Excluded => "••••••••••••",
        }
    }
    const FIXED: &'static str = "Hey, I checked the PR and it looks fine.";

    /// The message the app shows for this clip.
    fn message(&self) -> Message {
        match self.note {
            Note::Replaced | Note::UndoKeys => Message::fixed(4, self.shows_undo_keys),
            Note::Checking => Message::checking(),
            Note::Clean => Message::looks_good(),
            Note::Excluded => Message::turned_off(Some("1Password"), generic_app_icon()),
        }
    }
}

/// A stand-in app icon for the "turned off here" clip: the system's plain application icon.
fn generic_app_icon() -> Option<Img> {
    toast::app_icon("/System/Library/CoreServices/Iris Demo.app")
}

/// Every clip with its message landed, and the replace clip without the undo keys, as the
/// Swift snapshot `after-note-demos` lays them out (660 wide on paper).
#[allow(dead_code)]
pub fn demos_sheet(scale: f32) -> El {
    let mut c = div().flex().flex_col().gap(z(18.0));
    for note in NOTES {
        c = c.child(demo(&Demo { note, t: landed(note), keys: &QUICK_FIX, shows_note: true, shows_undo_keys: true }, SHEET_W, scale));
    }
    let d = Demo { note: Note::Replaced, t: landed(Note::Replaced), keys: &QUICK_FIX, shows_note: true, shows_undo_keys: false };
    sheet(c.child(demo(&d, SHEET_W, scale)))
}

/// The keys mid-press, whatever they are: the default, someone's own, a wide key and the
/// longest, as the Swift snapshot `after-note-demo-keys` lays them out.
#[allow(dead_code)]
pub fn keys_sheet(scale: f32) -> El {
    let mut c = div().flex().flex_col().gap(z(18.0));
    for (i, keys) in SAMPLE_KEYS.iter().enumerate() {
        c = c.child(demo(&Demo { note: Note::Replaced, t: [1.1, 1.0, 0.95, 1.2][i], keys, shows_note: true, shows_undo_keys: true }, SHEET_W, scale));
    }
    sheet(c)
}

const SHEET_W: f32 = 660.0 - 48.0;

fn sheet(stages: El) -> El {
    div().w(z(660.0)).p(z(24.0)).bg(col(ink::PAPER)).child(stages)
}

fn window_h() -> f32 {
    24.0 + 3.0 + 10.0 + bubble_h() + 8.0 + 35.0 + 8.0
}
fn bubble_h() -> f32 {
    // The caption's line on the pixel grid, as SwiftUI lays it.
    (11.0 * MONO_LINE * 2.0).floor() / 2.0 + 12.0
}
fn card_size(keys: &[Cap]) -> (f32, f32) {
    let plus = toast::shaped_brush_w("+", 16.0);
    let caps: f32 = keys.iter().map(|k| key_size(*k).0).sum();
    let n = keys.len() as f32;
    (2.0 * CARD_PAD.0 + caps + (n - 1.0) * (plus + 6.0), CARD_PAD.1 + KEY_H + CARD_PAD.2)
}

/// The stage's height: the window and the keys, the room above, and the Dock's below.
pub fn height() -> f32 {
    TOP + window_h() + 22.0 + card_size(&QUICK_FIX).1 + 30.0 + DOCK_ROOM
}

/// `NoteDemo` at `d.t`, `w` points wide.
pub fn demo(d: &Demo, w: f32, scale: f32) -> El {
    let h = height();
    let window_x = (w - WINDOW_W) / 2.0;
    let (cw, ch) = card_size(d.keys);
    let keys_in = d.keys_in() as f32;
    let mut stage = sized(w, h)
        .child(at(printed(w, h, &desk(w, h, scale)), 0.0, 0.0))
        .child(at(backdrop_words(w, h), 0.0, 0.0))
        .child(at(window(d, scale), window_x, TOP))
        .child(at(key_card(d, (w, h), ((w - cw) / 2.0, card_y(keys_in)), scale).opacity(keys_in), (w - cw) / 2.0, card_y(keys_in)));
    let _ = ch;
    // The message on the window's top edge at its right end, as Iris puts it on a text box.
    if d.shows_note {
        let m = d.message();
        let (sw, sh) = toast::size(&m);
        // At 82%, pinned at its top right (laid out smaller: GPUI can't scale a box). Swift also
        // leans it 1°, which GPUI can't do to text.
        let k = 0.82;
        let slip = with_zoom(k, || {
            let mut slip = sized(sw, sh).child(at(toast::toast(&m, scale), 0.0, 0.0));
            if matches!(d.note, Note::UndoKeys) && d.shows_undo_keys {
                // The pink pen rings what that switch adds.
                slip = slip.child(at(toast::brush_ring(ink::PINK_TEXT, 17, 60.0, 36.0, d.ring_in() as f32, scale), sw - 8.0 - 60.0, (sh - 36.0) / 2.0));
            }
            slip
        });
        let turned = sized(sw, sh).child(at(slip, sw * (1.0 - k), 0.0));
        stage = stage.child(at(turned.opacity(d.slip_in() as f32), window_x + WINDOW_W - sw + 10.0, TOP - 16.0));
    }
    stage.child(at(faint_line("desk-edge", &pen::hand_rect(64, 0.5, 2.0, false, 0.0, 0.0, w, h), w, h, 1.2, hue::I12, 0.7, scale), 0.0, 0.0))
}

// MARK: - The desk

/// The desk printed once: Iris's sky through the coarse screen (`ScreenPlate(.sky, seed: 19)`),
/// cut by hand, with the files and the Dock drawn on it in pale paper and pale pen.
fn desk(w: f32, h: f32, scale: f32) -> Img {
    let plate = Plate { w, h, scale };
    riso::cached(format!("note-desk-{w}-{h}"), &plate, || {
        let clip = plate.fill(&pen::hand_rect(63, 0.4, 0.0, true, 0.0, 0.0, w, h).skia().unwrap());
        let mut out = riso::print_with(&plate, SKY, 19, 1.0, 1.0, &clip, 1.0, |c| c);
        let mut layer = |fill: bool, path: &Pen, width: f32, rgb: Rgb8, alpha: f32| {
            let (pw, ph) = plate.px();
            let mut m = tiny_skia::Mask::new(pw, ph).unwrap();
            let Some(p) = path.skia() else { return };
            if fill {
                m.fill_path(&p, tiny_skia::FillRule::Winding, true, plate.transform());
            } else {
                let mut pm = tiny_skia::Pixmap::new(pw, ph).unwrap();
                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(0, 0, 0, 255);
                paint.anti_alias = true;
                let st = tiny_skia::Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
                pm.stroke_path(&p, &paint, &st, plate.transform(), None);
                m = riso::alpha_mask(&pm);
            }
            let m = riso::intersect(&m, &clip);
            let top = riso::flat(&plate, rgb, &m, alpha);
            riso::lay(&mut out, &top);
        };
        for item in backdrop(w, h) {
            match item {
                Mark::Fill(p, rgb, a) => layer(true, &p, 0.0, rgb, a),
                Mark::Line(p, width, rgb, a) => layer(false, &p, width, rgb, a),
            }
        }
        out
    })
}

const SKY: Rgb8 = (120, 162, 220);
/// The desktop is background, so it takes no ink of its own: paper faintly through the sky,
/// a pale pen line (`Backdrop`).
const PAPER_A: f32 = 0.5;
const LINE_A: f32 = 0.3;
const TEXT_A: f32 = 0.5;

enum Mark {
    Fill(Pen, Rgb8, f32),
    Line(Pen, f32, Rgb8, f32),
}

fn rect_pen(x: f32, y: f32, w: f32, h: f32) -> Pen {
    Pen::poly(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h]])
}

fn ellipse_pen(cx: f32, cy: f32, w: f32, h: f32) -> Pen {
    let pts: Vec<[f32; 2]> = (0..48).map(|i| {
        let a = i as f32 / 48.0 * std::f32::consts::TAU;
        [cx + w / 2.0 * a.cos(), cy + h / 2.0 * a.sin()]
    }).collect();
    Pen::poly(&pts)
}

/// `DogEar`: a sheet with its top right corner folded over, in the `w`×`h` box at `x`, `y`.
fn dog_ear(x: f32, y: f32, w: f32, h: f32) -> Pen {
    let fold = w.min(h) * 0.3;
    let mut p = Pen::new();
    p.m(x, y);
    p.l(x + w - fold, y);
    p.l(x + w, y + fold);
    p.l(x + w, y + h);
    p.l(x, y + h);
    p.z();
    p.m(x + w - fold, y);
    p.l(x + w - fold, y + fold);
    p.l(x + w, y + fold);
    p
}

fn mountain(x: f32, y: f32, w: f32, h: f32) -> Pen {
    Pen::poly(&[[x, y + h], [x + w * 0.38, y], [x + w * 0.62, y + h * 0.5], [x + w * 0.75, y + h * 0.3], [x + w, y + h]])
}

fn lines(x: f32, ys: &[(f32, f32)]) -> Pen {
    let mut p = Pen::new();
    for (y, to) in ys {
        p.m(x, *y);
        p.l(*to, *y);
    }
    p
}

#[derive(Clone, Copy)]
enum File {
    Folder,
    Page,
    Picture,
}

/// The files lying on the desk: (kind, name, seed) at each side.
const LEFT: [(File, &str, i64); 2] = [(File::Folder, "Drafts", 81), (File::Page, "notes.txt", 84)];
const RIGHT: [(File, &str, i64); 2] = [(File::Picture, "Screenshot", 87), (File::Folder, "Invoices", 90)];
const ICON_W: f32 = 62.0;
const ICON_H: f32 = 30.0 + 4.0 + 8.0 * MONO_LINE;
const DOCK: (f32, f32) = (299.0, 49.0);

/// Where each file's box starts (its 62-point column's top left).
fn file_origins(w: f32, h: f32) -> Vec<(File, &'static str, i64, f32, f32)> {
    let mut out = vec![];
    for (i, (k, n, s)) in LEFT.iter().enumerate() {
        out.push((*k, *n, *s, 14.0, 22.0 + i as f32 * (ICON_H + 14.0)));
    }
    let top = (h - (2.0 * ICON_H + 14.0 + 40.0)) / 2.0;
    for (i, (k, n, s)) in RIGHT.iter().enumerate() {
        out.push((*k, *n, *s, w - 12.0 - ICON_W, top + i as f32 * (ICON_H + 14.0)));
    }
    out
}

fn dock_origin(w: f32, h: f32) -> (f32, f32) {
    ((w - DOCK.0) / 2.0, h - 8.0 - DOCK.1)
}

/// Everything drawn on the desk under the window, in order: the files and the Dock.
fn backdrop(w: f32, h: f32) -> Vec<Mark> {
    let paper = |p: Pen, a: f32| Mark::Fill(p, ink::PAPER, a);
    let line = |p: Pen| Mark::Line(p, 1.0, hue::I12, LINE_A);
    let thin = |p: Pen, width: f32| Mark::Line(p, width, hue::I12, LINE_A);
    let mut m = vec![];
    for (kind, _, seed, x, y) in file_origins(w, h) {
        let gx = x + (ICON_W - 34.0) / 2.0;
        match kind {
            File::Folder => {
                m.push(paper(pen::hand_rect(seed, 0.4, 0.0, true, gx, y + 6.0, 34.0, 24.0), PAPER_A));
                m.push(paper(pen::hand_rect(seed + 1, 0.4, 0.0, true, gx, y + 12.0, 34.0, 18.0), PAPER_A));
                m.push(line(pen::hand_rect(seed + 3, 0.5, 1.5, false, gx, y + 6.0, 34.0, 24.0)));
                m.push(line(pen::hand_rect(seed + 4, 0.5, 1.5, false, gx, y + 12.0, 34.0, 18.0)));
                m.push(line(pen::hand_rect(seed + 5, 0.4, 1.0, false, gx + 1.0, y + 2.0, 13.0, 5.0)));
            }
            File::Page | File::Picture => {
                let px = gx + 5.0;
                m.push(paper(dog_ear(px, y, 24.0, 30.0), PAPER_A));
                if matches!(kind, File::Picture) {
                    m.push(line(mountain(px + 4.0, y + 14.0, 16.0, 11.0)));
                    m.push(line(ellipse_pen(px + 9.0, y + 10.0, 5.0, 5.0)));
                } else {
                    m.push(line(lines(px + 5.0, &[(y + 11.0, px + 19.0), (y + 16.0, px + 19.0), (y + 21.0, px + 13.0)])));
                }
                m.push(line(dog_ear(px, y, 24.0, 30.0)));
            }
        }
    }
    // The Dock: a faint strip, six app tiles, a rule, the bin.
    let (dx, dy) = dock_origin(w, h);
    m.push(paper(pen::hand_rect(115, 0.4, 0.0, true, dx, dy, DOCK.0, DOCK.1), 0.32));
    for i in 0..6 {
        let (tx, ty) = (dx + 12.0 + i as f32 * 39.0, dy + 7.0);
        let seed = 101 + 2 * i as i64;
        m.push(paper(pen::hand_rect(seed, 0.4, 0.0, true, tx, ty, 30.0, 30.0), PAPER_A));
        match i {
            0 => m.push(line(ellipse_pen(tx + 15.0, ty + 15.0, 17.0, 12.0))),
            1 => m.push(line(lines(tx + 8.0, &[(ty + 10.0, tx + 22.0), (ty + 15.0, tx + 22.0), (ty + 20.0, tx + 16.0)]))),
            2 => {
                let (ex, ey) = (tx + 6.0, ty + 9.0);
                let mut env = Pen::new();
                env.m(ex, ey);
                env.l(ex + 7.5, ey + 6.0);
                env.l(ex + 15.0, ey);
                m.push(line(env));
                m.push(line(rect_pen(ex, ey, 18.0, 12.0)));
            }
            3 => m.push(Mark::Fill(rect_pen(tx, ty + 8.0, 30.0, 1.0), hue::I12, LINE_A)),
            4 => {
                m.push(line(ellipse_pen(tx + 15.0, ty + 15.0, 18.0, 18.0)));
                m.push(thin(ellipse_pen(tx + 15.0, ty + 15.0, 8.0, 18.0), 0.9));
                m.push(Mark::Fill(rect_pen(tx + 6.0, ty + 15.0 - 0.45, 18.0, 0.9), hue::I12, LINE_A));
            }
            _ => m.push(line(mountain(tx + 5.5, ty + 12.0, 19.0, 12.0))),
        }
        m.push(line(pen::hand_rect(seed + 50, 0.5, 1.5, false, tx, ty, 30.0, 30.0)));
        if i == 0 {
            m.push(Mark::Fill(ellipse_pen(tx + 15.0, ty + 30.0 + 3.0 + 1.5, 3.0, 3.0), hue::I12, TEXT_A));
        }
    }
    let rx = dx + 12.0 + 6.0 * 39.0;
    m.push(line(pen::hand_line_vertical(113, 0.4, rx + 1.0, dy + 7.0, 30.0)));
    let bx = rx + 2.0 + 9.0;
    m.push(line(pen::hand_rect(114, 0.4, 1.0, false, bx + 6.5, dy + 7.0 + 7.5, 17.0, 21.0)));
    m.push(line(lines(bx + 4.0, &[(dy + 7.0 + 5.0, bx + 26.0)])));
    m
}

/// The files' names and the calendar's date, faint on the desk.
fn backdrop_words(w: f32, h: f32) -> El {
    let word = |s: &str, size: f32| div().opacity(TEXT_A).child(t(s, martian(size, 450), hue::I12));
    let mut e = sized(w, h);
    for (_, name, _, x, y) in file_origins(w, h) {
        let tw = mono_w(name, 8.0, 0.0);
        e = e.child(at(word(name, 8.0), x + (ICON_W - tw) / 2.0, y + 34.0));
    }
    let (dx, dy) = dock_origin(w, h);
    let (tx, ty) = (dx + 12.0 + 3.0 * 39.0, dy + 7.0);
    e.child(at(word("7", 11.0), tx + (30.0 - mono_w("7", 11.0, 0.0)) / 2.0, ty + 9.0 + (21.0 - 11.0 * MONO_LINE) / 2.0))
}

// MARK: - The app

/// A tiny app window drawn in pen: three window buttons and the app's name, their message, and
/// your reply in its box with a caret.
fn window(d: &Demo, scale: f32) -> El {
    let (w, h) = (WINDOW_W, window_h());
    let excluded = d.note == Note::Excluded;
    let mut win = paper_shadow(sized(w, h), w, h, 18.0, 10.0, 0.18)
        .child(at(hand_fill(51, 0.5, w, h, col(hue::I1)), 0.0, 0.0));
    // The window buttons and the name, by them: the message lands over the window's right end.
    for i in 0..3 {
        win = win.child(at(drawn(9.0, 9.0, pen::hand_ellipse(30 + i, 0.3, 9.0, 9.0), Some(1.1), over(hue::I12, 0.6, hue::I1)), 10.0 + i as f32 * 14.0, 7.5));
    }
    let name = if excluded { "1Password" } else { "Messages" };
    win = win.child(at(t(name, caption(), hue::I10), 60.0, (24.0 - 11.0 * MONO_LINE) / 2.0))
        .child(at(pen_rule(8, w, over(hue::I12, 0.3, hue::I1), 1.0), 0.0, 24.0));
    // Every clip keeps the same rows: the password manager puts a label where the message is.
    let by = 27.0 + 10.0;
    if excluded {
        win = win.child(at(t("Password", caption(), hue::I10), 10.0, by + (bubble_h() - 11.0 * MONO_LINE) / 2.0));
    } else {
        let bw = mono_w("Did you get to the PR?", 11.0, 0.0) + 20.0;
        win = win.child(at(hand_fill(9, 0.5, bw, bubble_h(), col(hue::GROUND)), 10.0, by))
            .child(at(t("Did you get to the PR?", caption(), hue::I11), 20.0, by + 6.0));
    }
    // The box, and the words in it with the caret after them, blinking.
    let (bx, boxy, bw, bh) = (8.0, by + bubble_h() + 8.0, w - 16.0, 35.0);
    win = win.child(at(hand_fill(41, 0.5, bw, bh, col(ink::PAPER)), bx, boxy))
        .child(at(faint_line("demo-box", &pen::hand_rect(43, 0.5, 2.0, false, 0.0, 0.0, bw, bh), bw, bh, 1.0, hue::I12, 0.6, scale), bx, boxy));
    let fixed = d.fixed_in() as f32;
    let caret_on = (d.t * 2.4) as i64 % 2 == 0;
    for (words, alpha) in [(d.draft(), 1.0 - fixed), (Demo::FIXED, fixed)] {
        if alpha <= 0.0 {
            continue;
        }
        let tw = mono_w(words, 13.5, 0.0);
        let mut typed = sized(tw + 1.0 + 1.6, 19.0)
            .child(at(t(words, subtitle(), hue::I11), 0.0, (19.0 - 13.5 * MONO_LINE) / 2.0));
        if caret_on {
            typed = typed.child(at(sized(1.6, 19.0).bg(col(hue::I12)), tw + 1.0, 0.0));
        }
        win = win.child(at(typed.opacity(alpha), bx + 10.0, boxy + 8.0));
    }
    win.child(at(faint_line("demo-window", &pen::hand_rect(52, 0.5, 2.0, false, 0.0, 0.0, w, h), w, h, 1.2, hue::I12, 0.7, scale), 0.0, 0.0))
}

// MARK: - The keys

/// The keys on a scrap of paper, in Mac order, joined by brush plus signs that turn pink as the
/// chord builds; "you press" on masking tape across the card's corner.
fn card_y(keys_in: f32) -> f32 {
    TOP + window_h() + 22.0 + 10.0 * (1.0 - keys_in)
}

/// `desk` is the stage's size and `at` where the card lies on it, for the tape to print onto.
fn key_card(d: &Demo, desk: (f32, f32), card_at: (f32, f32), scale: f32) -> El {
    let (w, h) = card_size(d.keys);
    let plus_w = toast::shaped_brush_w("+", 16.0);
    let plus_h = line_box(16.0, BRUSH_LINE);
    let mut keys = div().flex().flex_row().gap(z(3.0)).items_center();
    let mut flicks = vec![];
    let mut x = CARD_PAD.0;
    for (i, key) in d.keys.iter().enumerate() {
        if i > 0 {
            let lit = d.t >= d.key_down(i) && d.t < Demo::KEYS_UP;
            keys = keys.child(sized(plus_w, plus_h).child(at(t("+", toast::brush_line(16.0, plus_h), if lit { ink::PINK_TEXT } else { TAN }), 0.0, 0.0)));
            x += plus_w + 6.0;
        }
        keys = keys.child(drawn_keycap(*key, d.pressed(i) as f32, 70 + i as i64 * 9, scale));
        let (kw, _) = key_size(*key);
        let f = d.flick(i) as f32;
        if f > 0.0 {
            let (e, fx, fy) = key_flicks(kw, f);
            flicks.push(at(e, x + fx, CARD_PAD.1 + fy));
        }
        x += kw;
    }
    let k = d.tape_on() as f32;
    let under = Under { desk, card_at, card: (w, h), tape_at: (-13.0, -11.0) };
    // The tape lands: printed whole at its lean (GPUI can't turn it), grown 15% as it comes.
    let lean = -7.0 - 6.0 * (1.0 - k);
    let grow = 1.15 - 0.15 * k;
    let (tw, th) = tape_size();
    let tape = with_zoom(grow, || tape_label(&under, lean, scale)).opacity(k);
    let tape_at = (-13.0 - tw * (grow - 1.0) / 2.0, -11.0 - th * (grow - 1.0) / 2.0);
    // Swift leans the card 0.8°, which GPUI can't do to its keys.
    paper_shadow(sized(w, h), w, h, 10.0, 7.0, 0.18)
        .child(at(sized(w, h).bg(col(ink::PAPER)), 0.0, 0.0))
        .child(at(printed(w, h, &riso::grain(w, h, scale, 0.22)), 0.0, 0.0))
        .child(at(keys.h(z(KEY_H)), CARD_PAD.0, CARD_PAD.1))
        .child(at(tape, tape_at.0, tape_at.1))
        .children(flicks)
}

const TAN: Rgb8 = (196, 157, 122);

/// What lies under the tape: the desk (its size), the card (where it lies, its size) and where
/// the tape sits on the card.
struct Under {
    desk: (f32, f32),
    card_at: (f32, f32),
    card: (f32, f32),
    tape_at: (f32, f32),
}

/// A point turned `deg` degrees clockwise about `c`, as `rotationEffect` turns it.
fn turn(p: (f32, f32), c: (f32, f32), degrees: f32) -> (f32, f32) {
    let (s, k) = degrees.to_radians().sin_cos();
    let (x, y) = (p.0 - c.0, p.1 - c.1);
    (c.0 + x * k - y * s, c.1 + x * s + y * k)
}

/// "you press" in brush on a strip of masking tape, printed with multiply: the tape darkens
/// whatever is under it, the card's paper or the sky's dots (`TapeLabel.Tape().fill(tape)
/// .blendMode(.multiply)`). Rune has no blend modes, so the multiply is baked from what's under.
fn tape_label(u: &Under, lean: f32, scale: f32) -> El {
    let (tw, th) = tape_size();
    let plate = Plate { w: tw, h: th, scale };
    let key = format!("demo-tape-{tw}-{:?}-{:?}-{:?}", u.desk, u.card_at, u.card);
    let px = riso::cached(key, &plate, || {
        let mask = plate.fill(&pen::tape(tw, th).skia().unwrap());
        let (dw, dh) = (u.desk.0 as usize, u.desk.1 as usize);
        let dots = riso::screen(dw, dh, SKY, 19, 1.0, 1.0);
        let (pw, ph) = plate.px();
        let mut out = vec![0u8; (pw * ph * 4) as usize];
        for y in 0..ph {
            for x in 0..pw {
                let i = (y * pw + x) as usize;
                let a = mask.data()[i];
                if a == 0 {
                    continue;
                }
                // The tape turns -7° about its middle on the card, the card -0.8° about its own.
                let p = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                let on_card = turn(p, (tw / 2.0, th / 2.0), -7.0);
                let on_card = (on_card.0 + u.tape_at.0, on_card.1 + u.tape_at.1);
                let paper = on_card.0 >= 0.0 && on_card.1 >= 0.0 && on_card.0 < u.card.0 && on_card.1 < u.card.1;
                let under = if paper {
                    ink::PAPER
                } else {
                    let q = turn(on_card, (u.card.0 / 2.0, u.card.1 / 2.0), -0.8);
                    let (qx, qy) = (q.0 + u.card_at.0, q.1 + u.card_at.1);
                    let (ix, iy) = ((qx.max(0.0) as usize).min(dw - 1), (qy.max(0.0) as usize).min(dh - 1));
                    dots[iy * dw + ix]
                };
                let c = riso::multiply(hue::TAPE, under);
                out[i * 4..i * 4 + 4].copy_from_slice(&[c.0, c.1, c.2, a]);
            }
        }
        out
    });
    // The strip and its words, turned `lean` degrees about the middle and printed whole.
    let (pw, ph) = plate.px();
    let rgba = px_rgba(&px, pw, ph);
    let (lead, _) = brush_box("you press", 15.0);
    turned(format!("demo-tape-turned-{tw}-{lean:.2}-{:?}", u.card), tw, th, 10.0, lean, scale, move |s| {
        s.picture(&rgba, tw, th, 0.0, 0.0);
        word_into(s, brush(15.0), "you press", 12.0 + lead, 2.0 + 15.0 * BRUSH_ASCENT + (15.0 * BRUSH_LINE - 15.0 * 1.26) / 2.0, hue::I12);
    })
}

fn tape_size() -> (f32, f32) {
    (toast::shaped_brush_w("you press", 15.0) + 24.0, 15.0 * BRUSH_LINE + 5.0)
}

/// A printed picture's straight RGBA back (GPUI holds it as BGRA).
fn px_rgba(img: &Img, pw: u32, ph: u32) -> Vec<u8> {
    let mut out = img.as_bytes(0).map(|b| b.to_vec()).unwrap_or_else(|| vec![0; (pw * ph * 4) as usize]);
    for p in out.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    out
}



/// A key's size: a modifier's cap, a lone letter's square, or a named key's wider cap.
fn key_size(key: Cap) -> (f32, f32) {
    let (symbol, word) = key;
    if symbol.is_some() {
        return (46.0, KEY_H);
    }
    if lone(key) {
        return (KEY_H, KEY_H);
    }
    ((22.0 + word.chars().count() as f32 * 7.5).max(46.0), KEY_H)
}

fn lone(key: Cap) -> bool {
    key.0.is_none() && key.1.chars().count() <= 3 && key.1 != "esc"
}

/// `DS.Font.martian(size, weight: 600)`: there's no 600 cut, so it's printed from the outlines.
fn martian_600(s: &str, size: f32, c: Rgb8, scale: f32) -> (El, f32) {
    martian_line(s, size, 600.0, 87.5, 0.0, size * MONO_LINE, c, scale)
}

/// The system face, semibold: it draws ⌃ ⌥ ⇧ ⌘ cleanly; the mono face doesn't.
fn system_semibold(s: &str, size: f32, c: Rgb8) -> El {
    let family = if cfg!(target_os = "windows") { "Segoe UI Symbol" } else { ".SystemUIFont" };
    div().font_family(family).font_weight(FontWeight::SEMIBOLD).text_size(z(size)).line_height(z((size * 1.2).ceil()))
        .text_color(col(c)).whitespace_nowrap().child(s.to_string())
}

/// `DrawnKeycap`: one key drawn from above in a heavy pen line, a crema face with the pink plate
/// a few dots off it. `down` 0 up … 1 all the way down: the face sinks, the plate snaps under it
/// and the face prints pink (its ink flicks are `key_flicks`).
fn drawn_keycap(key: Cap, down: f32, seed: i64, scale: f32) -> El {
    let (w, h) = key_size(key);
    let (symbol, word) = key;
    // The face sinks 7% as it goes down: laid out smaller about its middle.
    let k = 1.0 - 0.07 * down;
    let face = with_zoom(k, || {
        let mut face = sized(w, h)
            .child(at(hand_printed(seed + 1, 0.5, w, h, ink::CREMA, seed + 1, scale), 0.0, 0.0));
        if down > 0.0 {
            face = face.child(at(div().opacity(down).child(hand_printed(seed + 1, 0.5, w, h, ink::PINK, seed + 5, scale)), 0.0, 0.0));
        }
        let under = if down > 0.5 { ink::PINK } else { ink::CREMA };
        face = face
            .child(at(drawn(w - 8.0, h - 10.0, pen::hand_rect(seed + 2, 0.4, 1.5, false, 0.0, 0.0, w - 8.0, h - 10.0), Some(1.0), over(hue::I12, 0.28, under)), 4.0, 4.0))
            .child(at(hand_outline(seed, 0.7, 2.5, w, h, col(hue::I12), 1.9), 0.0, 0.0));
        match symbol {
            Some(sym) => face
                .child(at(div().flex().flex_row().w(z(w - 6.0)).justify_end().child(system_semibold(sym, 11.5, hue::I12)), 0.0, 4.0 - 1.0))
                .child(at(t(word, martian(7.5, 560), hue::I12), 6.0, h - 6.0 - 7.5 * MONO_LINE)),
            None if lone(key) => {
                let size = if word.chars().count() == 1 { 15.5 } else { 11.5 };
                let (e, _) = martian_600(word, size, hue::I12, scale);
                face.child(at(div().w(z(w)).h(z(h)).flex().items_center().justify_center().child(e), 0.0, 0.0))
            }
            None => face.child(at(t(word.to_lowercase(), martian(8.0, 560), hue::I12), 6.0, h - 6.0 - 8.0 * MONO_LINE)),
        }
    });
    sized(w, h)
        .child(at(hand_printed(seed + 3, 0.5, w, h, ink::PINK, seed + 3, scale), -4.0 * (1.0 - down), 3.0 * (1.0 - down)))
        .child(at(face, w * (1.0 - k) / 2.0, 1.5 * down + h * (1.0 - k) / 2.0))
}

/// The three ink flicks over a key going down (`flick` 0 … 1), placed over a key `w` wide.
/// Drawn by the card after its tape: Swift's tape is multiplied over them, so they show through
/// it, and the baked tape here would hide them.
fn key_flicks(w: f32, flick: f32) -> (El, f32, f32) {
    let mut p = Pen::new();
    for (a, b) in [([6.0, 14.0], [1.0, 6.0]), ([18.0, 12.0], [18.0, 2.0]), ([30.0, 14.0], [35.0, 6.0])] {
        p.m(a[0], a[1]);
        p.l(b[0], b[1]);
    }
    // Grown from 60% about their middle, laid out at that size.
    let k = 0.6 + 0.3 * flick;
    let flicks = with_zoom(k, || sized(36.0, 16.0).child(drawn(36.0, 16.0, p, Some(1.7), col(ink::PINK_TEXT)))).opacity(flick);
    (flicks, (w - 36.0) / 2.0 + 36.0 * (1.0 - k) / 2.0, (KEY_H - 16.0) / 2.0 - KEY_H / 2.0 - 9.0 - 3.0 * flick + 16.0 * (1.0 - k) / 2.0)
}
