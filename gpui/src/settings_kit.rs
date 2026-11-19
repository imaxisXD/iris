//! What the Settings pages share: text broken into lines as Core Text breaks it, Martian Mono
//! printed at any weight and width (GPUI only has the three cuts), the sections and rows, the
//! switch, dropdown and ink choice, the table, the form controls and the buttons, the dry-brush
//! X. Ported from the Rune copy (`settings.rs`, `pages/tones.rs`, `pages/apps.rs`), whose pages
//! were each measured against the Swift drawing: where two Rune pages draw a part differently
//! (a section, an ink label), both are kept, so each page lands where it was measured.

use gpui::{Div, InteractiveElement, ParentElement, Role, StatefulInteractiveElement, Styled, Toggled, div};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, Mask, PathBuilder, Transform};

use crate::parts::*;
use crate::pen::{self, Pen};
use crate::riso::{self, Img, Plate, Rgb8, Rng, col, hue, ink, over};

/// `DS.Ink.i2`: the empty states' wash.
pub const I2: Rgb8 = (0xF0, 0xEB, 0xE6);
/// `DS.Size.row`: a table row's least height.
pub const ROW_H: f32 = 38.0;

// MARK: - Layout

/// A flex column.
pub fn column() -> Div {
    div().flex().flex_col().flex_none()
}

/// A flex row.
pub fn row() -> Div {
    div().flex().flex_row().flex_none()
}

/// Room that takes what's left of a row.
pub fn grow() -> Div {
    div().flex_grow_1()
}

/// An element at `a` opacity (GPUI fades images, paths and text with their box).
pub fn faded(e: impl gpui::IntoElement, a: f32) -> Div {
    div().flex_none().opacity(a).child(e)
}

// MARK: - Text in lines

/// Lines of text one under another, `pitch` apart (each line's box `pitch` tall).
pub fn lines(ls: &[String], ts: Ts, c: Rgb8, pitch: f32) -> Div {
    let mut d = column();
    for l in ls {
        d = d.child(t(l.clone(), ts.line(pitch), c));
    }
    d
}

/// Mono words broken into lines `w` wide as Core Text breaks them: greedily, then, when the last
/// line would be one word alone, the word before it comes down to keep it company.
pub fn push_out(s: &str, w: f32, size: f32) -> Vec<String> {
    let fits = |line: &[&str]| mono_w(&line.join(" "), size, 0.0) <= w + 0.01;
    let mut ls: Vec<Vec<&str>> = vec![];
    for word in s.split(' ') {
        match ls.last_mut() {
            Some(line) if fits(&[&line[..], &[word]].concat()) => line.push(word),
            _ => ls.push(vec![word]),
        }
    }
    let n = ls.len();
    if n >= 2 && ls[n - 1].len() == 1 && ls[n - 2].len() > 2 {
        let moved = ls[n - 2].pop().unwrap();
        if fits(&[&[moved][..], &ls[n - 1][..]].concat()) {
            ls[n - 1].insert(0, moved);
        } else {
            ls[n - 2].push(moved);
        }
    }
    ls.iter().map(|l| l.join(" ")).collect()
}

/// A paragraph broken with `push_out`, its lines the style's own height apart.
pub fn para_out(s: &str, ts: Ts, c: Rgb8, w: f32) -> Div {
    lines(&push_out(s, w, ts.size), ts, c, ts.line).w(z(w))
}

/// The same, its lines on whole points (SwiftUI sets 11-point captions 13 apart).
pub fn para_floor(s: &str, ts: Ts, c: Rgb8, w: f32) -> Div {
    lines(&push_out(s, w, ts.size), ts, c, ts.line.floor()).w(z(w))
}

/// Mono words wrapped `w` wide the way AppKit wraps them: at spaces and after hyphens, and
/// never leaving one word alone on the last line (it takes the word before it along).
pub fn wrap_mono(s: &str, advance: f32, w: f32) -> Vec<String> {
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        cur.push(c);
        if c == ' ' || c == '-' {
            pieces.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    let fits = |line: &str| line.trim_end().chars().count() as f32 * advance <= w + 0.01;
    let mut ls: Vec<Vec<String>> = vec![vec![]];
    for p in pieces {
        let last = ls.last_mut().unwrap();
        let trial: String = last.concat() + &p;
        if last.is_empty() || fits(&trial) {
            last.push(p);
        } else {
            ls.push(vec![p]);
        }
    }
    let n = ls.len();
    if n > 1 && ls[n - 1].len() == 1 && ls[n - 2].len() > 1 {
        let moved = ls[n - 2].pop().unwrap();
        ls[n - 1].insert(0, moved);
    }
    ls.into_iter().map(|l| l.concat().trim_end().to_string()).collect()
}

/// A paragraph of mono lines (`wrap_mono`), one under another `pitch` apart, the whole on the
/// half point as SwiftUI lays it out.
pub fn mono_paragraph(s: &str, ts: Ts, c: Rgb8, w: f32, pitch: f32) -> Div {
    let ls = wrap_mono(s, ts.size * MONO_ADVANCE + ts.tracking, w);
    let h = (ls.len() as f32 * pitch * 2.0).round() / 2.0;
    let mut p = sized(w, h);
    for (i, l) in ls.into_iter().enumerate() {
        p = p.child(at(t(l, ts, c), 0.0, i as f32 * pitch));
    }
    p
}

// MARK: - Martian Mono at any weight and width

struct Outline {
    b: PathBuilder,
    x: f32,
    y: f32,
}
impl OutlinePen for Outline {
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

/// Martian Mono's advance at a weight and width.
pub fn mono_adv(size: f32, wght: f32, wdth: f32) -> f32 {
    crate::letters::martian_advance(size, wght, wdth)
}

/// Martian Mono's ascent and descent at `size` (the same at every weight and width).
pub fn mono_metrics(size: f32) -> (f32, f32) {
    let m = FontRef::new(MARTIAN).unwrap().metrics(Size::new(size), LocationRef::default());
    (m.ascent, -m.descent)
}

/// The baseline of a line of Martian `size` in a box `line_h` tall, centred as text is set.
pub fn mono_base(size: f32, line_h: f32) -> f32 {
    let (a, d) = mono_metrics(size);
    (line_h - a - d) / 2.0 + a
}

/// A line of Martian Mono at `wght` and `wdth` as one path, its baseline at (`x`, `base`), each
/// glyph one advance plus `tracking` along.
pub fn martian_path(s: &str, size: f32, wght: f32, wdth: f32, tracking: f32, x: f32, base: f32) -> Option<tiny_skia::Path> {
    let font = FontRef::new(MARTIAN).ok()?;
    let loc = font.axes().location([("wght", wght), ("wdth", wdth)]);
    let adv = mono_adv(size, wght, wdth);
    let glyphs = font.outline_glyphs();
    let cm = font.charmap();
    let mut pen = Outline { b: PathBuilder::new(), x, y: base };
    for c in s.chars() {
        if let Some(g) = cm.map(c).and_then(|gid| glyphs.get(gid)) {
            let _ = g.draw(DrawSettings::unhinted(Size::new(size), LocationRef::from(&loc)), &mut pen);
        }
        pen.x += adv + tracking;
    }
    pen.b.finish()
}

/// The width of a line of Martian at `wght` and `wdth`.
pub fn martian_w(s: &str, size: f32, wght: f32, wdth: f32, tracking: f32) -> f32 {
    s.chars().count() as f32 * (mono_adv(size, wght, wdth) + tracking)
}

/// A line of Martian Mono at a weight and width GPUI has no cut for, printed flat in `c` from
/// the variable font's outlines, in a box `line_h` tall. Returns it with its width.
#[allow(clippy::too_many_arguments)]
pub fn martian_line(s: &str, size: f32, wght: f32, wdth: f32, tracking: f32, line_h: f32, c: Rgb8, scale: f32) -> (Div, f32) {
    let w = martian_w(s, size, wght, wdth, tracking).max(1.0);
    let plate = Plate { w, h: line_h, scale };
    let img = riso::cached(format!("settings-martian-{s}-{size}-{wght}-{wdth}-{tracking}-{line_h}-{c:?}"), &plate, || {
        let mut mask = plate.mask();
        if let Some(p) = martian_path(s, size, wght, wdth, tracking, 0.0, mono_base(size, line_h)) {
            mask.fill_path(&p, FillRule::Winding, true, plate.transform());
        }
        riso::flat(&plate, c, &mask, 1.0)
    });
    (sized(w, line_h).child(at(printed(w, line_h, &img), 0.0, 0.0)), w)
}

// MARK: - Turned pictures

/// `PenArrow`'s three strokes (shaft and two barbs) in a `w`×`h` box, pointing left (or right).
pub fn arrow_strokes(w: f32, h: f32, weight: f32, seed: i64, right: bool) -> Vec<tiny_skia::Path> {
    let strokes = [
        pen::pen_stroke(&[[0.97, 0.56], [0.6, 0.5], [0.3, 0.52], [0.05, 0.5]], w, h, weight, 0.7, 0.8, seed),
        pen::pen_stroke(&[[0.04, 0.5], [0.2, 0.28], [0.36, 0.08]], w, h, weight, 0.9, 0.35, seed + 1),
        pen::pen_stroke(&[[0.04, 0.5], [0.21, 0.72], [0.38, 0.93]], w, h, weight, 0.9, 0.35, seed + 2),
    ];
    let flip = if right { Transform::from_row(-1.0, 0.0, 0.0, 1.0, w, 0.0) } else { Transform::identity() };
    strokes.iter().filter_map(|s| s.skia()?.transform(flip)).collect()
}

/// `PenArrow(right: true)` turned `degrees` about its middle, printed whole (GPUI can't turn a
/// box): a `w`×`h` box, the picture spilling past it.
pub fn arrow_turned(w: f32, h: f32, weight: f32, seed: i64, degrees: f32, scale: f32) -> Div {
    let pad = (w.max(h) / 2.0).ceil();
    let plate = Plate { w: w + 2.0 * pad, h: h + 2.0 * pad, scale };
    let img = riso::cached(format!("settings-arrow-{w}-{h}-{weight}-{seed}-{degrees}"), &plate, || {
        let turn = Transform::from_translate(pad, pad).pre_concat(Transform::from_rotate_at(degrees, w / 2.0, h / 2.0));
        let mut mask = plate.mask();
        for p in arrow_strokes(w, h, weight, seed, true) {
            mask.fill_path(&p, FillRule::Winding, true, plate.transform().pre_concat(turn));
        }
        riso::print(&plate, ink::INK, seed, &mask, 1.0)
    });
    sized(w, h).child(at(printed(plate.w, plate.h, &img), -pad, -pad))
}

// MARK: - Sections

/// `Section` as Settings draws it (General, Shortcuts, Tones, Words, Updates, About): a pink
/// folio, the heading in brush, its description broken as Core Text breaks it, actions at the
/// right, an ink rule, the content padded 2 from it. The words get what's left of the row after
/// the folio, its 12-point gap and the spacer's least 16 (and with actions, 24 and the actions).
pub fn section(number: Option<usize>, title: &str, detail: Option<&str>, actions: Option<(gpui::AnyElement, f32)>, content: Option<Div>, w: f32, scale: f32) -> Div {
    // SwiftUI rounds the heading's line up to a whole pixel, not a whole point.
    let brush_box = (24.0 * BRUSH_LINE * scale - 0.01).ceil() / scale;
    let title_base = (brush_box - 24.0 * BRUSH_LINE) / 2.0 + 24.0 * BRUSH_ASCENT;
    let folio_box = line_box(13.5, MONO_LINE);
    let folio_base = (folio_box - 13.5 * MONO_LINE) / 2.0 + 13.5;
    let mut head = row().w(z(w)).gap(z(12.0)).items_start();
    let mut text_w = w - 16.0;
    if let Some(n) = number {
        head = head.child(folio(n, scale).mt(z(title_base - folio_base)));
        text_w -= 12.0 + 3.0 * 13.5 * MONO_ADVANCE;
    }
    if let Some((_, aw)) = &actions {
        text_w -= 24.0 + aw;
    }
    let mut words = column().gap(z(4.0))
        .child(div().id(gpui::SharedString::from(format!("heading-{title}"))).role(Role::Heading).aria_label(title.to_string())
            .child(t(title, brush(24.0).line(brush_box), hue::I12)));
    if let Some(d) = detail {
        words = words.child(para_out(d, body(), hue::I11, text_w));
    }
    head = head.child(words);
    match actions {
        Some((a, _)) => head = head.child(grow().min_w(z(16.0))).child(div().flex_none().child(a)),
        None => head = head.child(grow()),
    }
    let mut s = column().w(z(w)).gap(z(12.0)).child(head);
    if let Some(c) = content {
        let seed = title.chars().count() as i64 * 7 + number.unwrap_or(0) as i64;
        s = s.child(pen_rule(seed, w, over(hue::I12, 0.85, ink::PAPER), 1.2)).child(column().pt(z(2.0)).child(c));
    }
    s
}

/// The section's folio: "01" in Martian at weight 300, a cut GPUI doesn't have, so printed.
pub fn folio(n: usize, scale: f32) -> Div {
    martian_line(&format!("{n:02}"), 13.5, 300.0, 87.5, 0.0, line_box(13.5, MONO_LINE), ink::PINK_TEXT, scale).0
}

/// `Section` as Apps, Model & memory and Privacy draw it (measured against SwiftUI's drawing):
/// the heading's box 30.5 tall, the description as wide as SwiftUI leaves it (the row less the
/// folio, the gaps, the spacer's 16 and the actions), wrapped as AppKit wraps it, and the
/// actions on the heading's baseline.
pub fn section_wrapped(number: usize, title: &str, detail: Option<&str>, actions: Option<(gpui::AnyElement, f32)>, content: Div, w: f32, scale: f32) -> Div {
    const HEADING_H: f32 = 30.5;
    let title_base = (line_box(24.0, BRUSH_LINE) - 24.0 * BRUSH_LINE) / 2.0 + 24.0 * BRUSH_ASCENT;
    let folio_base = (line_box(13.5, MONO_LINE) - 13.5 * MONO_LINE) / 2.0 + 13.5;
    let label = format!("{number:02}");
    let actions_w = actions.as_ref().map_or(0.0, |a| 12.0 + a.1);
    let text_w = w - mono_w(&label, 13.5, 0.0) - 12.0 - 12.0 - 16.0 - actions_w;
    let mut words = column().gap(z(4.0))
        .child(div().id(gpui::SharedString::from(format!("heading-{title}"))).role(Role::Heading).aria_label(title.to_string())
            .h(z(HEADING_H)).child(t(title, brush(24.0), hue::I12)));
    if let Some(d) = detail {
        words = words.child(mono_paragraph(d, body(), hue::I11, text_w, 12.5 * MONO_LINE));
    }
    let mut head = row().w(z(w)).gap(z(12.0)).items_start()
        .child(folio(number, scale).mt(z(title_base - folio_base)))
        .child(words)
        .child(grow());
    if let Some((a, _)) = actions {
        // First baselines line up: the heading's 23.4 down its box, a button's words 21 down it.
        let words_base = (32.0 - 12.5 * MONO_LINE) / 2.0 + 12.5;
        head = head.child(div().flex_none().mt(z(title_base - words_base)).child(a));
    }
    let seed = title.chars().count() as i64 * 7 + number as i64;
    column().w(z(w)).gap(z(12.0)).child(head)
        .child(pen_rule(seed, w, over(hue::I12, 0.85, ink::PAPER), 1.2))
        .child(column().pt(z(2.0)).child(content))
}

// MARK: - Rows

/// `SettingRow`: words on the left, the control at the right.
pub fn setting_row(title: &str, detail: Option<&str>, control: impl gpui::IntoElement, control_w: f32, w: f32) -> Div {
    let words_w = w - control_w - 16.0;
    let mut words = column().gap(z(3.0)).w(z(words_w)).child(t(title, body(), hue::I12));
    if let Some(d) = detail {
        words = words.child(para_out(d, caption(), hue::I10, words_w));
    }
    row().w(z(w)).gap(z(16.0)).items_center().child(words).child(control)
}

/// The faint rule between rows.
pub fn hairline(seed: i64, w: f32) -> gpui::AnyElement {
    pen_rule(seed, w, over(hue::I12, 0.3, ink::PAPER), 1.0)
}

/// A control named for VoiceOver by its row's title.
pub fn named(control: Div, id: &str, role: Role, title: &str) -> gpui::Stateful<Div> {
    control.id(gpui::SharedString::from(format!("control-{id}"))).role(role).aria_label(title.to_string())
}

// MARK: - Controls

/// `TactileSwitch`: a key sliding in a hand-drawn slot, the slot pink when on.
pub fn switch(on: bool, title: &str, scale: f32) -> gpui::Stateful<Div> {
    let (w, h) = (50.0, 26.0);
    let slot = if on { hand_printed(61, 0.4, w, h, ink::PINK, 61, scale) } else { hand_fill(61, 0.4, w, h, col(hue::I3)) };
    let knob = sized(20.0, 16.0)
        .child(at(hand_fill(65, 0.3, 20.0, 16.0, drop(0.22)), 0.0, 1.0))
        .child(at(hand_printed(66, 0.35, 20.0, 16.0, if on { ink::INK } else { ink::CREMA }, 66, scale), 0.0, 0.0))
        .child(at(hand_outline(67, 0.35, 1.5, 20.0, 16.0, col(hue::I12), 1.2), 0.0, 0.0));
    let s = sized(w, h)
        .child(at(slot, -0.8, -0.5))
        .child(at(hand_outline(63, 0.45, 2.0, w, h, col(hue::I12), 1.2), 0.0, 0.0))
        .child(at(knob, if on { w - 24.0 } else { 4.0 }, 5.0));
    named(s, title, Role::Switch, title).aria_toggled(if on { Toggled::True } else { Toggled::False })
}

/// The dropdown's hand chevron.
fn chevron() -> Pen {
    let mut chev = Pen::new();
    chev.m(0.0, 0.0);
    chev.q(4.5 * 0.9, 2.5 * 1.2, 4.5, 5.0);
    chev.l(9.0, 0.5);
    chev
}

/// `Dropdown` at rest: a pen outline on a paper face, an icon if it has one, the value and a
/// hand chevron.
pub fn dropdown(label: &str, icon: Option<&Img>, w: f32) -> gpui::Stateful<Div> {
    let h = 32.0;
    let mut inner = row().w(z(w)).h(z(h)).px(z(10.0)).gap(z(8.0)).items_center();
    if let Some(i) = icon {
        inner = inner.child(div().flex_none().ml(z(-2.0)).child(printed(20.0, 20.0, i)));
    }
    inner = inner.child(t(label, body().tracked(-0.12), hue::I12)).child(grow())
        .child(drawn(9.0, 5.0, chevron(), Some(1.4), col(hue::I12)));
    let d = sized(w, h)
        .child(at(hand_fill(71, 0.3, w, h, drop(0.18)), 0.0, 1.0))
        .child(at(hand_fill(72, 0.4, w, h, col(ink::PAPER)), 0.0, 0.0))
        .child(at(hand_outline(73, 0.45, 2.0, w, h, over(hue::I12, 0.75, ink::PAPER), 1.2), 0.0, 0.0))
        .child(at(inner, 0.0, 0.0));
    named(d, &format!("dropdown-{label}"), Role::ComboBox, label)
}

/// `InkChoice`: the others in mono, the chosen one in brush over a pink marker.
pub fn ink_choice(labels: &[&str], selected: usize, title: &str, scale: f32) -> gpui::Stateful<Div> {
    let tab_h = line_box(23.0, BRUSH_LINE).max(line_box(12.5, MONO_LINE) + 4.0);
    let (mx, my, mw, mh, marker) = tone_marker(labels, selected, scale);
    let mut tabs = row().gap(z(18.0));
    for (i, tab) in tone_tabs(labels, selected) {
        tabs = tabs.child(tab.id(gpui::SharedString::from(format!("{title}-{i}"))).role(Role::Tab)
            .aria_label(labels[i].to_string()).aria_selected(i == selected));
    }
    let c = sized(ink_choice_w(labels), tab_h)
        .child(at(printed(mw, mh, &marker), mx, my))
        .child(at(tabs, 0.0, 0.0));
    named(c, title, Role::TabList, title)
}

pub fn ink_choice_w(labels: &[&str]) -> f32 {
    labels.iter().map(|l| brush_w(l, 23.0).max(mono_w(l, 12.5, -0.12))).sum::<f32>() + 18.0 * (labels.len() as f32 - 1.0)
}

// MARK: - Table

/// `PenRule()`'s seed in `TableHeader`: its line in Components.swift.
const HEADER_RULE_SEED: i64 = 761;

/// `TableHeader`: small tracked caps over a pen rule; a column without a width takes the rest.
pub fn table_header(columns: &[(&str, Option<f32>)], w: f32) -> Div {
    let style = caption().tracked(0.5);
    let mut r = row().w(z(w)).h(z(30.0)).px(z(12.0)).gap(z(12.0)).items_center();
    for (label, width) in columns {
        let cell = t(label.to_uppercase(), style, hue::I10);
        match width {
            Some(cw) => r = r.child(div().flex_none().w(z(*cw)).child(cell)),
            None => r = r.child(cell).child(grow()),
        }
    }
    sized(w, 30.0).child(at(r, 0.0, 0.0)).child(at(table_rule(HEADER_RULE_SEED, w), 0.0, 30.0 - 3.0 + 1.5))
}

/// `TableRow`: the row's items 12 apart, a row tall, and a pen rule under it unless it's the
/// last. (A dimmed row fades its items itself, each on its own; the rule never dims.)
pub fn table_row(content: Div, last: bool, seed: i64, w: f32) -> Div {
    let inner = row().w(z(w)).h(z(ROW_H)).px(z(12.0)).items_center().child(content.flex_grow_1());
    let mut r = sized(w, ROW_H).child(at(inner, 0.0, 0.0));
    if !last {
        r = r.child(at(table_rule(211 + seed * 37, w), 0.0, ROW_H - 3.0 + 1.5));
    }
    r
}

/// `PenRule(seed:)` at its default ink.
fn table_rule(seed: i64, w: f32) -> gpui::AnyElement {
    pen_rule(seed, w, over(hue::I12, 0.3, ink::PAPER), 1.0)
}

// MARK: - Labels

/// `InkLabel` as the Tones table draws it: a name knocked out of a strip of espresso printed
/// through the screen, the strip and the words each at `alpha` in a faded row.
pub fn ink_label_faded(label: &str, size: f32, alpha: f32, scale: f32) -> Div {
    let (px_, py_) = (size * 0.62, size * 0.26);
    let w = mono_w(label, size, 0.0) + 2.0 * px_;
    let h = size * MONO_LINE + 2.0 * py_;
    let seed = label.chars().count() as i64 + 3;
    let plate = Plate { w, h, scale };
    let strip = riso::cached(format!("settings-inklabel-{label}-{size}"), &plate, || {
        let mask = plate.fill(&PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, w, h).unwrap()));
        riso::print(&plate, ink::INK, seed, &mask, 1.0)
    });
    sized(w, h)
        .child(at(faded(printed(w, h, &strip), alpha), 0.0, 0.0))
        .child(at(faded(t(label, martian(size, 430), ink::PAPER), alpha), px_, py_))
}

/// `InkLabel` as Apps and Model & memory draw it: the strip's height to the nearest half point
/// (measured: 10.5 pt is 18.5 tall, 12 pt 20.5). Returns it and its width.
pub fn ink_label(words: &str, size: f32, scale: f32) -> (Div, f32) {
    let (px_, py_) = (size * 0.62, size * 0.26);
    let w = mono_w(words, size, 0.0) + 2.0 * px_;
    let line = (size * MONO_LINE * 2.0).ceil() / 2.0;
    let h = ((line + 2.0 * py_) * 2.0).round() / 2.0;
    let plate = Plate { w, h, scale };
    let seed = words.chars().count() as i64 + 3;
    let strip = riso::cached(format!("settings-pages-inklabel-{seed}-{w}-{h}"), &plate, || {
        let mut m = plate.mask();
        m.fill_path(&PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, w, h).unwrap()), FillRule::Winding, false, plate.transform());
        riso::print(&plate, ink::INK, seed, &m, 1.0)
    });
    let e = sized(w, h)
        .child(at(printed(w, h, &strip), 0.0, 0.0))
        .child(at(t(words, martian(size, 430), ink::PAPER), px_, (h - size * MONO_LINE) / 2.0));
    (e, w)
}

// MARK: - Form controls

/// `TactileCheckbox`: the pink plate off register under a pen edge, an ink tick overshooting it.
/// Faded (disabled, or in a hidden row), each layer at `alpha` on its own.
pub fn checkbox_faded(on: bool, alpha: f32, label: &str, scale: f32) -> gpui::Stateful<Div> {
    let s = 18.0;
    let face = if on { hand_printed(23, 0.35, s, s, ink::PINK, 23, scale) } else { hand_fill(23, 0.35, s, s, col(ink::PAPER)) };
    let mut b = sized(s, s)
        .child(at(faded(face, alpha), -0.8, -0.5))
        .child(at(faded(hand_outline(24, 0.4, 2.0, s, s, col(hue::I12), 1.2), alpha), 0.0, 0.0));
    if on {
        // A 15×14 tick centred in the box, nudged up and right.
        b = b.child(at(faded(drawn(15.0, 14.0, pen::hand_tick(15.0, 14.0), Some(2.2), col(hue::I12)), alpha), 1.5 + 2.5, 2.0 - 2.5));
    }
    named(b, &format!("check-{label}"), Role::CheckBox, label).aria_toggled(if on { Toggled::True } else { Toggled::False })
}

/// `WellField` at rest, empty: a sunk grey well with its placeholder.
pub fn well_field(placeholder: &str, w: f32) -> gpui::Stateful<Div> {
    let h = 32.0;
    let f = sized(w, h)
        .child(at(hand_fill(81, 0.4, w, h, col(hue::I3)), 0.0, 0.0))
        .child(at(row().w(z(w)).h(z(h)).px(z(10.0)).items_center().child(t(placeholder, body(), hue::I10)), 0.0, 0.0));
    named(f, &format!("field-{placeholder}"), Role::TextInput, placeholder)
}

/// `PenGlyph`: plus, chevrons, a bin, drawn in a `size` box.
pub fn pen_glyph(name: &str, size: f32) -> Pen {
    let pt = |x: f32, y: f32| (x * size, y * size);
    let mut p = Pen::new();
    let poly = |p: &mut Pen, pts: &[(f32, f32)]| {
        let (x, y) = pt(pts[0].0, pts[0].1);
        p.m(x, y);
        for &(a, b) in &pts[1..] {
            let (x, y) = pt(a, b);
            p.l(x, y);
        }
    };
    match name {
        "plus" => {
            let (a, b, c) = (pt(0.5, 0.06), pt(0.47, 0.5), pt(0.53, 0.95));
            p.m(a.0, a.1);
            p.q(b.0, b.1, c.0, c.1);
            let (a, b, c) = (pt(0.05, 0.52), pt(0.5, 0.55), pt(0.96, 0.47));
            p.m(a.0, a.1);
            p.q(b.0, b.1, c.0, c.1);
        }
        "chevron.up" => poly(&mut p, &[(0.1, 0.72), (0.5, 0.28), (0.92, 0.7)]),
        "chevron.down" => poly(&mut p, &[(0.1, 0.3), (0.5, 0.74), (0.92, 0.32)]),
        "trash" => {
            poly(&mut p, &[(0.06, 0.22), (0.95, 0.2)]);
            poly(&mut p, &[(0.36, 0.2), (0.38, 0.05), (0.63, 0.05), (0.64, 0.2)]);
            poly(&mut p, &[(0.17, 0.3), (0.25, 0.96), (0.76, 0.95), (0.84, 0.29)]);
            poly(&mut p, &[(0.42, 0.42), (0.43, 0.82)]);
            poly(&mut p, &[(0.6, 0.42), (0.59, 0.82)]);
        }
        _ => {
            let (a, b, c, d) = (pt(0.06, 0.55), pt(0.38, 0.88), pt(0.6, 0.4), pt(0.98, 0.08));
            p.m(a.0, a.1);
            p.l(b.0, b.1);
            p.q(c.0, c.1, d.0, d.1);
        }
    }
    p
}

/// `IconButton` at rest: a pen glyph, 11 points in a 22-point square; faded when it can't act.
pub fn icon_button(symbol: &str, enabled: bool, id: &str) -> gpui::Stateful<Div> {
    let ink_c = if enabled { col(hue::I10) } else { over(hue::I10, 0.4, ink::PAPER) };
    let b = sized(22.0, 22.0).flex().items_center().justify_center()
        .child(drawn(11.0, 11.0, pen_glyph(symbol, 11.0), Some(1.6), ink_c));
    named(b, id, Role::Button, symbol)
}

// MARK: - Buttons

/// `TactileButton(role: .secondary)`, `h` tall (32, or 26 compact), with the pen plus when
/// `plus`: paper face, pen outline, a small drop. Returns it and its width.
pub fn secondary(title: &str, plus: bool, h: f32) -> (gpui::Stateful<Div>, f32) {
    let seed = riso::str_seed(title, 17);
    let title_w = mono_w(title, 12.5, -0.12);
    let lead = if plus { 10.0 } else { 12.0 };
    let mut content = row().h(z(h)).pl(z(lead)).pr(z(12.0)).gap(z(6.0)).items_center();
    let mut w = lead + 12.0 + title_w;
    if plus {
        let mut p = Pen::new();
        // `PenGlyph("plus")`: a touch off square, like a quick pen cross. 10 points, i9.
        p.m(5.0, 0.6);
        p.q(4.7, 5.0, 5.3, 9.5);
        p.m(0.5, 5.2);
        p.q(5.0, 5.5, 9.6, 4.7);
        content = content.child(drawn(10.0, 10.0, p, Some(1.5), col(hue::I9)));
        w += 16.0;
    }
    content = content.child(t(title, body().tracked(-0.12), hue::I12));
    let b = sized(w, h)
        .child(at(hand_fill(seed + 9, 0.3, w, h, drop(0.18)), 0.0, 1.2))
        .child(at(hand_fill(seed + 3, 0.4, w, h, col(ink::PAPER)), 0.0, 0.0))
        .child(at(hand_outline(seed, 0.45, 2.0, w, h, over(hue::I12, 0.75, ink::PAPER), 1.1), 0.0, 0.0))
        .child(at(content, 0.0, 0.0));
    (named(b, &format!("button-{title}"), Role::Button, title), w)
}

/// `TactileButton(role: .ghost)` at rest: only its words, in i10, `h` tall.
pub fn ghost(title: &str, h: f32) -> (gpui::Stateful<Div>, f32) {
    let w = 24.0 + mono_w(title, 12.5, -0.12);
    let b = row().w(z(w)).h(z(h)).pl(z(12.0)).items_center().child(t(title, body().tracked(-0.12), hue::I10));
    (named(b, &format!("ghost-{title}"), Role::Button, title), w)
}

/// `TactileButton(role: .primary)`, with a pen plus before its words when `plus`: the primary's
/// one dry brush. Disabled, the rose pass under it isn't printed and each layer fades to 0.6 on
/// its own (SwiftUI's `.opacity` on a stack, no compositing group), so the slab shows through
/// the words and greys them.
pub fn primary(title: &str, plus: bool, enabled: bool, scale: f32) -> (gpui::Stateful<Div>, f32) {
    let h = 32.0;
    let title_w = mono_w(title, 12.5, -0.12);
    let lead = if plus { 16.0 + 10.0 + 6.0 } else { 18.0 };
    let w = lead + title_w + 18.0;
    let a = if enabled { 1.0 } else { 0.6 };
    let plate = Plate { w, h, scale };
    let top = riso::cached(format!("slab-top-{w}-{h}"), &plate, || {
        riso::print(&plate, ink::INK, BRUSH, &slab_mask(&plate, BRUSH, 1.9), 1.0)
    });
    let mut b = sized(w, h);
    if enabled {
        let under = riso::cached(format!("slab-under-{w}-{h}"), &plate, || {
            let mask = slab_mask(&plate, BRUSH + 7, 0.0);
            let mut base = riso::print(&plate, ink::INK, BRUSH + 7, &mask, 1.0);
            riso::lay(&mut base, &riso::print(&plate, ink::ROSE, BRUSH + 7, &mask, 0.35));
            base
        });
        b = b.child(at(printed(w, h, &under), -2.5, -2.0));
    }
    let mut words = row().h(z(h)).pl(z(if plus { 16.0 } else { 18.0 })).pr(z(18.0)).gap(z(6.0)).items_center();
    if plus {
        words = words.child(faded(drawn(10.0, 10.0, pen_glyph("plus", 10.0), Some(1.5), col(hue::I1)), a));
    }
    words = words.child(faded(t(title, body().tracked(-0.12), hue::I1), a));
    b = b.child(at(faded(printed(w, h, &top), a), 0.0, 0.0)).child(at(words, 0.0, 0.0));
    (named(b, &format!("primary-{title}"), Role::Button, title), w)
}

// MARK: - Empty state

/// `EmptyState`: the flower's bloom over its lines, centred on a pen-edged wash.
pub fn empty_state(lines_: &[&str], w: f32) -> Div {
    let h = 32.0 + 28.0 + 4.0 + lines_.len() as f32 * (4.0 + 12.5 * MONO_LINE) + 32.0;
    // `RisoArt.flower(points: 28, crop: .bloom)`, drawn at its own 3× and shown at 28 points.
    let plate = Plate { w: 28.0, h: 28.0, scale: 3.0 };
    let bloom = riso::cached("settings-empty-bloom".into(), &plate, || crate::rig::flower(28.0, crate::rig::Crop::Bloom, true, 3.0).2);
    let mut stack = column().w(z(w)).items_center().gap(z(4.0)).pt(z(32.0)).child(div().flex_none().pb(z(4.0)).child(printed(28.0, 28.0, &bloom)));
    for l in lines_ {
        stack = stack.child(t(*l, body(), hue::I11));
    }
    sized(w, h)
        .child(at(hand_fill(91, 0.6, w, h, col(I2)), 0.0, 0.0))
        .child(at(stack, 0.0, 0.0))
}

// MARK: - The dry-brush X

/// `DryBrushX`: two dry-brush strokes printed through the screen (`RisoArt.screen` at full grid
/// and specks). Its bristles are cut `.destinationOut` in Swift, but every Swift drawing shows
/// none: the X prints solid, so they're left out, as the Rune copy leaves them.
pub fn dry_brush_x(rgb: Rgb8, seed: i64, width: f32, dryness: f32, w: f32, h: f32, scale: f32) -> Img {
    let plate = Plate { w, h, scale };
    riso::cached(format!("settings-dryx-{rgb:?}-{seed}-{width}-{dryness}-{w}-{h}"), &plate, || dry_brush_x_rgba(rgb, seed, width, &plate, Transform::identity()))
}

/// The X's strokes as straight RGBA on `plate`, drawn through `t` (its `w`×`h` box in points).
pub fn dry_brush_x_rgba(rgb: Rgb8, seed: i64, width: f32, plate: &Plate, t: Transform) -> Vec<u8> {
    let mask = dry_brush_x_mask(seed, width, plate, t, plate.w, plate.h);
    riso::print_with(plate, rgb, seed + 40, 1.0, 1.0, &mask, 1.0, |c| c)
}

/// The X's coverage in a `w`×`h` box, through `t`.
pub fn dry_brush_x_mask(seed: i64, width: f32, plate: &Plate, t: Transform, w: f32, h: f32) -> Mask {
    const STROKES: [([f32; 2], [f32; 2]); 2] = [([0.03, 0.08], [0.97, 0.95]), ([0.96, 0.03], [0.05, 0.93])];
    let mut m = plate.mask();
    for (k, (f, to)) in STROKES.iter().enumerate() {
        let p = brush_stroke(seed + k as i64 * 7, [f[0] * w, f[1] * h], [to[0] * w, to[1] * h], width);
        m.fill_path(&p.skia().unwrap(), FillRule::Winding, true, plate.transform().pre_concat(t));
    }
    m
}

/// `DryBrushStroke` from `a` to `b`: flat and fat, heavier where the brush lands, rough edges,
/// square ends frayed into bristle tips. `width` is its width as a share of its length.
fn brush_stroke(seed: i64, a: [f32; 2], b: [f32; 2], width: f32) -> Pen {
    let mut rng = Rng::new(seed);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy).max(1.0);
    let dir = [dx / length, dy / length];
    let n = [-dir[1], dir[0]];
    let bw = length * width;
    let bend = (rng.f() - 0.5) * length * 0.07;
    let steps = 22;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let off = (t * std::f32::consts::PI).sin() * bend;
        let c = [a[0] + dx * t + n[0] * off, a[1] + dy * t + n[1] * off];
        let half = bw / 2.0 * (1.12 - 0.22 * t);
        let l = half * (1.0 + (rng.f() - 0.5) * 0.16);
        let r = half * (1.0 + (rng.f() - 0.5) * 0.16);
        left.push([c[0] + n[0] * l, c[1] + n[1] * l]);
        right.push([c[0] - n[0] * r, c[1] - n[1] * r]);
    }
    let mut fray = |from: [f32; 2], to: [f32; 2], out_dir: [f32; 2], reach: f32| -> Vec<[f32; 2]> {
        (0..=18)
            .map(|k| {
                let u = k as f32 / 18.0;
                let base = [from[0] + (to[0] - from[0]) * u, from[1] + (to[1] - from[1]) * u];
                let out = if k % 2 == 1 { reach * (0.35 + rng.f() * 0.65) } else { reach * rng.f() * 0.15 };
                [base[0] + out_dir[0] * out, base[1] + out_dir[1] * out]
            })
            .collect()
    };
    let mut pts = left.clone();
    pts.extend(fray(left[steps], right[steps], dir, bw * 0.32));
    pts.extend(right.iter().rev().skip(1).copied());
    pts.extend(fray(right[0], left[0], [-dir[0], -dir[1]], bw * 0.22));
    Pen::poly(&pts)
}

// MARK: - Ink ribbons

/// `InkRibbon.path`: the centre line widened, pressing in fast and lifting slowly, its end
/// frayed into bristle tips when `fray`.
pub fn ribbon(c: &[[f32; 2]], width: f32, start_taper: f32, end_taper: f32, fray: bool) -> Pen {
    let mut rng = Rng::new(c.len() as i64 * 7);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for i in 0..c.len() {
        let a = c[i.saturating_sub(1)];
        let b = c[(i + 1).min(c.len() - 1)];
        let len = ((b[0] - a[0]).hypot(b[1] - a[1])).max(0.001);
        let n = [-(b[1] - a[1]) / len, (b[0] - a[0]) / len];
        let t = i as f32 / (c.len() - 1) as f32;
        let press = (t / 0.12).min(1.0) * (1.0 - start_taper) + start_taper;
        let lift = ((1.0 - t) / 0.3).min(1.0) * (1.0 - end_taper) + end_taper;
        let half = width / 2.0 * press.min(lift) * (1.0 + (rng.f() - 0.5) * 0.14);
        left.push([c[i][0] + n[0] * half, c[i][1] + n[1] * half]);
        right.push([c[i][0] - n[0] * half, c[i][1] - n[1] * half]);
    }
    let mut p = Pen::new();
    p.m(left[0][0], left[0][1]);
    for q in &left[1..] {
        p.l(q[0], q[1]);
    }
    if fray && c.len() > 1 {
        let e = c.len() - 1;
        let (a, b) = (c[e - 1], c[e]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(0.001);
        let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        for k in 0..=8 {
            let u = k as f32 / 8.0;
            let base = [left[e][0] + (right[e][0] - left[e][0]) * u, left[e][1] + (right[e][1] - left[e][1]) * u];
            let out = if k % 2 == 1 { width * (0.3 + rng.f() * 0.7) } else { 0.0 };
            p.l(base[0] + d[0] * out, base[1] + d[1] * out);
        }
    }
    for q in right.iter().rev() {
        p.l(q[0], q[1]);
    }
    p.z();
    p
}

/// `InkRibbon(line: .loop(seed:))`'s centre line in a `w`×`h` frame: `HandEllipse`'s loop a
/// little past full, growing as it goes round.
pub fn loop_line(seed: i64, w: f32, h: f32, width: f32) -> Vec<[f32; 2]> {
    let (ix, iy) = (w * 0.05 + width, h * 0.05 + width);
    let (rw, rh) = (w - 2.0 * ix, h - 2.0 * iy);
    let mut rng = Rng::new(seed);
    let start = -2.5 + rng.f() * 0.3;
    let sweep = 2.0 * std::f32::consts::PI + 0.55;
    let phase = rng.f() * 6.3;
    (0..=60).map(|i| {
        let t = i as f32 / 60.0;
        let a = start + sweep * t;
        let grow = 1.0 + 0.1 * t;
        let o = (a * 3.0 + phase).sin();
        [ix + rw / 2.0 + (rw / 2.0 * grow + o) * a.cos(), iy + rh / 2.0 + (rh / 2.0 * grow + o) * a.sin()]
    }).collect()
}
