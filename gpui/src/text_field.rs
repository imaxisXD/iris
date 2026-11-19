//! A text field (`WellField`, Components.swift): one line of text you type into, in a sunk grey
//! well that darkens a step while it has the keyboard, a pen ring round it then. Typing, the
//! caret, a selection (shift and the arrows, or dragging), ⌘A ⌘C ⌘V ⌘X, ⌫ and ⌦, Home and End,
//! Return (`submit`) and Escape (lets go of the keyboard). Input methods work through GPUI's
//! input handler, so accents and other scripts type as they do anywhere on the Mac.
//!
//! Built on GPUI's own example (`crates/gpui/examples/input.rs` at the pinned rev). The text,
//! the selection and the editing rules (`TextField`) are kept apart from the drawing (`view`),
//! so a field of several lines (setup's practice box) can reuse them with a drawing of its own.
//!
//! Under test (`IRIS_CONTROL`) copy and paste use a clipboard of their own, never the Mac's.

use std::cell::RefCell;
use std::ops::Range;

use gpui::*;
use unicode_segmentation::UnicodeSegmentation;

use crate::interact::{self as live, Act, Curve, Live, alpha};
use crate::parts::*;
use crate::riso::{col, hue, ink};
use crate::settings_kit::row;

actions!(iris_text_field, [Backspace, Delete, Left, Right, SelectLeft, SelectRight, SelectAll, Home, End, SelectHome, SelectEnd,
    WordLeft, WordRight, DeleteWordLeft, DeleteToStart, Paste, Cut, Copy, Submit, Cancel, Up, Down, SelectUp, SelectDown, NextField, PreviousField]);

/// The caret's blink: on, then off, this many milliseconds each (the Mac's own pace).
const BLINK_MS: u64 = 530;

/// The key context the field's keys are bound in.
const CONTEXT: &str = "IrisTextField";

/// Binds the field's keys (once).
pub fn init(cx: &mut App) {
    thread_local! { static DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    let m = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    let word = if cfg!(target_os = "macos") { "alt" } else { "ctrl" };
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new(Box::leak(format!("{word}-left").into_boxed_str()), WordLeft, c),
        KeyBinding::new(Box::leak(format!("{word}-right").into_boxed_str()), WordRight, c),
        KeyBinding::new(Box::leak(format!("{word}-backspace").into_boxed_str()), DeleteWordLeft, c),
        KeyBinding::new(Box::leak(format!("{m}-a").into_boxed_str()), SelectAll, c),
        KeyBinding::new(Box::leak(format!("{m}-v").into_boxed_str()), Paste, c),
        KeyBinding::new(Box::leak(format!("{m}-c").into_boxed_str()), Copy, c),
        KeyBinding::new(Box::leak(format!("{m}-x").into_boxed_str()), Cut, c),
        KeyBinding::new("enter", Submit, c),
        KeyBinding::new("tab", NextField, c),
        KeyBinding::new("shift-tab", PreviousField, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("escape", Cancel, c),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-left", Home, c),
            KeyBinding::new("cmd-right", End, c),
            KeyBinding::new("cmd-shift-left", SelectHome, c),
            KeyBinding::new("cmd-shift-right", SelectEnd, c),
            KeyBinding::new("cmd-backspace", DeleteToStart, c),
        ]);
    }
}

// MARK: - The text and its rules

/// What's typed, where the caret and the selection are, and what was last drawn (for clicks).
pub struct TextField {
    focus: FocusHandle,
    pub content: String,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// How far the line is scrolled left so the caret stays in sight.
    scroll: Pixels,
    selecting: bool,
    /// Several lines (setup's practice box): Return starts a new line, the text wraps, ↑ ↓ move
    /// between lines.
    multiline: bool,
    /// Several lines: each paragraph's start in the text, its wrapped layout and its top, as
    /// last drawn, and how far the box is scrolled down.
    paragraphs: std::rc::Rc<Vec<(usize, WrappedLine, Pixels)>>,
    line_height: Pixels,
    scroll_y: Pixels,
    /// When the caret last moved (the caret shows solid after a move, then blinks), and whether
    /// the field had the keyboard when last drawn (only then does it blink).
    moved: std::time::Instant,
    had_keys: bool,
}

impl TextField {
    pub fn new(cx: &mut App) -> Entity<TextField> {
        cx.new(|cx| {
            // The caret blinks at the Mac's pace while the field has the keyboard.
            cx.spawn(async move |this: WeakEntity<TextField>, cx| {
                loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(BLINK_MS)).await;
                    if this.update(cx, |f, cx| if f.had_keys { cx.notify() }).is_err() {
                        return;
                    }
                }
            })
            .detach();
            TextField {
            // A tab stop: Tab and ⇧Tab move between a page's fields.
            focus: cx.focus_handle().tab_stop(true),
            content: String::new(),
            selected: 0..0,
            reversed: false,
            marked: None,
            last_layout: None,
            last_bounds: None,
            scroll: px(0.0),
            selecting: false,
            multiline: false,
            paragraphs: Default::default(),
            line_height: px(20.0),
            scroll_y: px(0.0),
            moved: std::time::Instant::now(),
            had_keys: false,
            }
        })
    }

    /// Whether the caret shows now: solid for a moment after it moves, then on and off.
    fn caret_shows(&self) -> bool {
        (self.moved.elapsed().as_millis() / BLINK_MS as u128) % 2 == 0
    }

    /// A field of several lines, holding `text`.
    pub fn new_multiline(text: &str, cx: &mut App) -> Entity<TextField> {
        let e = TextField::new(cx);
        e.update(cx, |f, cx| {
            f.multiline = true;
            f.set(text, cx);
        });
        e
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The text with surrounding spaces trimmed, as Swift's pages read a field.
    pub fn trimmed(&self) -> &str {
        self.content.trim()
    }

    /// Replaces everything typed (empties the field after Add).
    pub fn set(&mut self, text: &str, cx: &mut Context<Self>) {
        self.content = text.to_string();
        self.selected = text.len()..text.len();
        self.reversed = false;
        self.marked = None;
        self.scroll = px(0.0);
        cx.notify();
    }

    pub fn selection(&self) -> Range<usize> {
        self.selected.clone()
    }

    fn cursor(&self) -> usize {
        if self.reversed { self.selected.start } else { self.selected.end }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.moved = std::time::Instant::now();
        self.selected = offset..offset;
        self.reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.moved = std::time::Instant::now();
        if self.reversed { self.selected.start = offset } else { self.selected.end = offset }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content.grapheme_indices(true).rev().find_map(|(i, _)| (i < offset).then_some(i)).unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content.grapheme_indices(true).find_map(|(i, _)| (i > offset).then_some(i)).unwrap_or(self.content.len())
    }

    fn word_start(&self, offset: usize) -> usize {
        word_start(&self.content, offset)
    }

    fn word_end(&self, offset: usize) -> usize {
        word_end(&self.content, offset)
    }

    /// Puts `text` in place of the selection (typing, pasting, deleting).
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.moved = std::time::Instant::now();
        let r = self.marked.take().unwrap_or(self.selected.clone());
        // One line: a pasted line break becomes a space.
        let text = if self.multiline { text.replace("\r\n", "\n").replace('\r', "\n") } else { text.replace(['\n', '\r'], " ") };
        self.content.replace_range(r.clone(), &text);
        let end = r.start + text.len();
        self.selected = end..end;
        self.reversed = false;
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            let prev = self.previous_boundary(self.cursor());
            self.select_to(prev, cx);
        }
        self.insert("", cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            let next = self.next_boundary(self.cursor());
            self.select_to(next, cx);
        }
        self.insert("", cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            let to = self.word_start(self.cursor());
            self.select_to(to, cx);
        }
        self.insert("", cx);
    }

    /// ⌘⌫: everything before the caret goes.
    fn delete_to_start(&mut self, _: &DeleteToStart, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(0, cx);
        }
        self.insert("", cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selected.is_empty() { self.previous_boundary(self.cursor()) } else { self.selected.start };
        self.move_to(to, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selected.is_empty() { self.next_boundary(self.selected.end) } else { self.selected.end };
        self.move_to(to, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_start(self.cursor());
        self.move_to(to, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_end(self.cursor());
        self.move_to(to, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.previous_boundary(self.cursor());
        self.select_to(to, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.next_boundary(self.cursor());
        self.select_to(to, cx);
    }

    pub fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = 0..self.content.len();
        self.reversed = false;
        cx.notify();
    }

    /// Several lines: where index `i` is drawn, from the box's top left (scrolled), if laid out.
    fn area_position(&self, i: usize) -> Option<Point<Pixels>> {
        let (start, line, top) = self.paragraphs.iter().rev().find(|(start, _, _)| *start <= i)?;
        let p = line.position_for_index((i - start).min(line.len()), self.line_height)?;
        Some(point(p.x, p.y + *top - self.scroll_y))
    }

    /// Several lines: the index nearest a point given from the box's top left.
    fn area_index(&self, p: Point<Pixels>) -> usize {
        let y = p.y + self.scroll_y;
        let Some((start, line, top)) = self.paragraphs.iter().rev().find(|(_, _, top)| *top <= y).or(self.paragraphs.first()) else { return 0 };
        let local = point(p.x.max(px(0.0)), (y - *top).max(px(0.0)));
        let i = match line.closest_index_for_position(local, self.line_height) { Ok(i) | Err(i) => i };
        (start + i).min(self.content.len())
    }

    /// ↑ or ↓ a line (several lines only; one line: to the start or the end).
    fn vertical(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let to = if !self.multiline {
            if down { self.content.len() } else { 0 }
        } else {
            match self.area_position(self.cursor()) {
                Some(p) => {
                    let dy = if down { self.line_height } else { -self.line_height };
                    let target = point(p.x, p.y + dy + self.line_height / 2.0);
                    if !down && p.y + self.scroll_y < self.line_height / 2.0 { 0 } else { self.area_index(target) }
                }
                None => self.cursor(),
            }
        };
        if select { self.select_to(to, cx) } else { self.move_to(to, cx) }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, true, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.content.len();
        self.move_to(n, cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.content.len();
        self.select_to(n, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            write_clipboard(&self.content[self.selected.clone()], cx);
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            write_clipboard(&self.content[self.selected.clone()], cx);
            self.insert("", cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = read_clipboard(cx) {
            self.insert(&text, cx);
        }
    }

    fn index_for_point(&self, p: Point<Pixels>) -> usize {
        if self.multiline {
            let Some(bounds) = self.last_bounds.as_ref() else { return 0 };
            return self.area_index(point(p.x - bounds.left(), p.y - bounds.top()));
        }
        self.index_for_x(p.x)
    }

    fn index_for_x(&self, x: Pixels) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref()) else { return 0 };
        if self.content.is_empty() {
            return 0;
        }
        line.closest_index_for_x(x - bounds.left() + self.scroll)
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.selecting = true;
        let i = self.index_for_point(e.position);
        if e.click_count >= 2 {
            self.selected = self.word_start(i.min(self.content.len()))..self.word_end(i);
            self.reversed = false;
            cx.notify();
        } else if e.modifiers.shift {
            self.select_to(i, cx);
        } else {
            self.move_to(i, cx);
        }
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            let i = self.index_for_point(e.position);
            self.select_to(i, cx);
        }
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    // UTF-16 offsets, as the Mac's input methods count.
    fn from_utf16(&self, offset: usize) -> usize {
        let (mut u8_, mut u16_) = (0, 0);
        for ch in self.content.chars() {
            if u16_ >= offset {
                break;
            }
            u16_ += ch.len_utf16();
            u8_ += ch.len_utf8();
        }
        u8_
    }

    fn to_utf16(&self, offset: usize) -> usize {
        let (mut u16_, mut u8_) = (0, 0);
        for ch in self.content.chars() {
            if u8_ >= offset {
                break;
            }
            u8_ += ch.len_utf8();
            u16_ += ch.len_utf16();
        }
        u16_
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.to_utf16(r.start)..self.to_utf16(r.end)
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.from_utf16(r.start)..self.from_utf16(r.end)
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(&mut self, r: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let r = self.range_from_utf16(&r);
        actual.replace(self.range_to_utf16(&r));
        Some(self.content[r].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.range_to_utf16(&self.selected), reversed: self.reversed })
    }

    fn set_selected_text_range(&mut self, r: Range<usize>, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.range_from_utf16(&r);
        self.reversed = false;
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(r) = r.as_ref().map(|r| self.range_from_utf16(r)) {
            self.selected = r;
            self.marked = None;
        }
        self.insert(text, cx);
    }

    fn replace_and_mark_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, new_selected: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        let r = r.as_ref().map(|r| self.range_from_utf16(r)).or(self.marked.clone()).unwrap_or(self.selected.clone());
        self.content.replace_range(r.clone(), text);
        self.marked = (!text.is_empty()).then(|| r.start..r.start + text.len());
        let start16 = self.to_utf16(r.start);
        let len16 = text.encode_utf16().count();
        self.selected = new_selected
            .map(|s| self.range_from_utf16(&(start16 + s.start.min(len16)..start16 + s.end.min(len16))))
            .unwrap_or_else(|| r.start + text.len()..r.start + text.len());
        self.reversed = false;
        cx.notify();
    }

    fn bounds_for_range(&mut self, r: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        if self.multiline {
            let r = self.range_from_utf16(&r);
            let a = self.area_position(r.start)?;
            let b = self.area_position(r.end)?;
            return Some(Bounds::from_corners(point(bounds.left() + a.x, bounds.top() + a.y), point(bounds.left() + b.x.max(a.x), bounds.top() + b.y + self.line_height)));
        }
        let line = self.last_layout.as_ref()?;
        let r = self.range_from_utf16(&r);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(r.start) - self.scroll, bounds.top()),
            point(bounds.left() + line.x_for_index(r.end) - self.scroll, bounds.bottom()),
        ))
    }

    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        if self.multiline {
            let i = self.index_for_point(p);
            return Some(self.to_utf16(i));
        }
        let local = self.last_bounds?.localize(&p)?;
        let line = self.last_layout.as_ref()?;
        Some(self.to_utf16(line.closest_index_for_x(local.x + self.scroll)))
    }
}

/// The start of the word before `offset` in `s` (⌥←, ⌥⌫).
pub fn word_start(s: &str, offset: usize) -> usize {
    let mut last = 0;
    for (i, w) in s.split_word_bound_indices() {
        if i >= offset {
            break;
        }
        if w.chars().any(|c| c.is_alphanumeric()) {
            last = i;
        }
    }
    last
}

/// The end of the word after `offset` in `s` (⌥→).
pub fn word_end(s: &str, offset: usize) -> usize {
    for (i, w) in s.split_word_bound_indices() {
        if i + w.len() > offset && w.chars().any(|c| c.is_alphanumeric()) {
            return i + w.len();
        }
    }
    s.len()
}

// MARK: - The clipboard

thread_local! {
    /// Under test: the clipboard copy and paste use, never the Mac's.
    static TEST_CLIPBOARD: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn write_clipboard(s: &str, cx: &mut App) {
    if crate::textbridge::under_test() {
        TEST_CLIPBOARD.with(|c| *c.borrow_mut() = Some(s.to_string()));
    } else {
        cx.write_to_clipboard(ClipboardItem::new_string(s.to_string()));
    }
}

fn read_clipboard(cx: &mut App) -> Option<String> {
    if crate::textbridge::under_test() {
        TEST_CLIPBOARD.with(|c| c.borrow().clone())
    } else {
        cx.read_from_clipboard().and_then(|i| i.text())
    }
}

/// Puts `text` on the test clipboard (`IRIS_CONTROL` only).
pub fn set_test_clipboard(text: &str) {
    TEST_CLIPBOARD.with(|c| *c.borrow_mut() = Some(text.to_string()));
}

/// What the test clipboard holds (`IRIS_CONTROL` only).
pub fn test_clipboard() -> Option<String> {
    TEST_CLIPBOARD.with(|c| c.borrow().clone())
}

// MARK: - Drawing

/// A field as a page draws it: the field, what's in it, and whether it has the keyboard (read
/// by the window as it renders, since a page is built without the app).
#[derive(Clone)]
pub struct Field {
    pub entity: Entity<TextField>,
    focus: FocusHandle,
    pub text: String,
    pub focused: bool,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field").field("text", &self.text).field("focused", &self.focused).finish()
    }
}

impl Field {
    pub fn of(entity: &Entity<TextField>, window: &Window, cx: &App) -> Field {
        let f = entity.read(cx);
        Field { entity: entity.clone(), focus: f.focus.clone(), text: f.content.clone(), focused: f.focus.is_focused(window) }
    }

    /// What's typed, trimmed.
    pub fn trimmed(&self) -> String {
        self.text.trim().to_string()
    }
}

/// Empties `entity` (after its words were added).
pub fn clear(entity: &Entity<TextField>, cx: &mut App) {
    entity.update(cx, |f, cx| f.set("", cx));
}

/// `WellField`: the sunk well (i3, i4 while it has the keyboard, fading on `DS.easeOut`), the pen
/// ring 2 points outside it then, the text or its placeholder 10 points in. Return calls
/// `submit`; Escape lets go of the keyboard.
pub fn view(field: &Field, placeholder: &str, w: f32, submit: Act) -> AnyElement {
    let l = live::get().unwrap_or_else(Live::still);
    let h = 32.0;
    let id = format!("field-{placeholder}");
    let k = l.track(&format!("{id}-focus"), field.focused as u8 as f32, Curve::FAST);
    let entity = field.entity.clone();
    let text = TextElement { field: entity.clone(), placeholder: placeholder.into() };
    let inner = row().w(z(w)).h(z(h)).px(z(10.0)).items_center()
        .child(styled(body(), hue::I12).flex_1().overflow_hidden().child(text));
    let el = well(w, h, k)
        .child(at(inner, 0.0, 0.0))
        .id(SharedString::from(id))
        .key_context(CONTEXT)
        .track_focus(&field.focus)
        // A click (or VoiceOver's press) gives it the keyboard; the press itself places the caret.
        .on_click({
            let f = field.focus.clone();
            move |_, window, cx| window.focus(&f, cx)
        })
        .cursor(CursorStyle::IBeam)
        .role(Role::TextInput)
        .aria_label(placeholder.to_string());
    let el = with_actions(el, &entity, submit);
    el.into_any_element()
}

fn with_actions(el: Stateful<Div>, e: &Entity<TextField>, submit: Act) -> Stateful<Div> {
    macro_rules! on {
        ($el:expr, $a:ty, $m:ident) => {{
            let e = e.clone();
            $el.on_action(move |a: &$a, window: &mut Window, cx: &mut App| e.update(cx, |t, cx| t.$m(a, window, cx)))
        }};
    }
    let el = on!(el, Backspace, backspace);
    let el = on!(el, Delete, delete);
    let el = on!(el, DeleteWordLeft, delete_word_left);
    let el = on!(el, DeleteToStart, delete_to_start);
    let el = on!(el, Left, left);
    let el = on!(el, Right, right);
    let el = on!(el, WordLeft, word_left);
    let el = on!(el, WordRight, word_right);
    let el = on!(el, SelectLeft, select_left);
    let el = on!(el, SelectRight, select_right);
    let el = on!(el, SelectAll, select_all);
    let el = on!(el, Home, home);
    let el = on!(el, End, end);
    let el = on!(el, SelectHome, select_home);
    let el = on!(el, SelectEnd, select_end);
    let el = on!(el, Copy, copy);
    let el = on!(el, Cut, cut);
    let el = on!(el, Paste, paste);
    let el = on!(el, Up, up);
    let el = on!(el, Down, down);
    let el = on!(el, SelectUp, select_up);
    let el = on!(el, SelectDown, select_down);
    let (e2, e3, e4) = (e.clone(), e.clone(), e.clone());
    // Tab and ⇧Tab: the next or the previous field (`focus_next` over the tab stops).
    let el = el.on_action(|_: &NextField, window: &mut Window, cx: &mut App| window.focus_next(cx))
        .on_action(|_: &PreviousField, window: &mut Window, cx: &mut App| window.focus_prev(cx));
    let e5 = e.clone();
    // Several lines: Return starts a new one; one line: it submits.
    el.on_action(move |_: &Submit, window: &mut Window, cx: &mut App| {
        if e5.read(cx).multiline {
            e5.update(cx, |t, cx| t.insert("\n", cx));
        } else {
            submit(window, cx)
        }
    })
        // Escape: the keyboard goes back to the window (nothing typed is lost).
        .on_action(|_: &Cancel, window: &mut Window, _cx: &mut App| window.blur(_cx))
        .on_mouse_down(MouseButton::Left, move |ev, window, cx| e2.update(cx, |t, cx| t.mouse_down(ev, window, cx)))
        .on_mouse_move(move |ev, window, cx| e3.update(cx, |t, cx| t.mouse_move(ev, window, cx)))
        .on_mouse_up(MouseButton::Left, move |ev, window, cx| e4.update(cx, |t, cx| t.mouse_up(ev, window, cx)))
}

/// The well, `k` of the way to having the keyboard: i3 to i4, the pen ring fading in.
fn well(w: f32, h: f32, k: f32) -> Div {
    let fill_ = crate::interact::mix(hue::I3, crate::controls::I4, k);
    sized(w, h)
        .child(at(hand_fill(81, 0.4, w, h, col(fill_)), 0.0, 0.0))
        .child(at(hand_outline(83, 0.45, 2.0, w + 4.0, h + 4.0, alpha(col(hue::I10), k), 2.0), -2.0, -2.0))
}

/// The field as the component sheet draws it, with the keyboard: `text` typed, the caret after it.
pub fn sheet_face(text: &str, w: f32) -> Div {
    let h = 32.0;
    let caret = div().flex_none().w(z(1.5)).h(z(17.0)).bg(col(hue::I12));
    well(w, h, 1.0).child(at(row().w(z(w)).h(z(h)).px(z(10.0)).items_center().child(t(text, body(), hue::I12)).child(caret), 0.0, 0.0))
}

/// The line itself: the text (or the placeholder, in i10), the selection under it and the caret,
/// scrolled so the caret stays in sight; it takes the Mac's typing while its field is focused.
struct TextElement {
    field: Entity<TextField>,
    placeholder: SharedString,
}

struct Prepaint {
    line: ShapedLine,
    caret: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll: Pixels,
}


impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Prepaint {
        let f = self.field.read(cx);
        let style = window.text_style();
        let (shown, color) = if f.content.is_empty() {
            (self.placeholder.clone(), col(hue::I10).into())
        } else {
            (SharedString::from(f.content.clone()), style.color)
        };
        let run = TextRun { len: shown.len(), font: style.font(), color, background_color: None, underline: None, strikethrough: None };
        let runs = match f.marked.as_ref().filter(|_| !f.content.is_empty()) {
            Some(m) => vec![
                TextRun { len: m.start, ..run.clone() },
                TextRun { len: m.end - m.start, underline: Some(UnderlineStyle { color: Some(run.color), thickness: px(1.0), wavy: false }), ..run.clone() },
                TextRun { len: shown.len() - m.end, ..run },
            ].into_iter().filter(|r| r.len > 0).collect(),
            None => vec![run],
        };
        let size = style.font_size.to_pixels(window.rem_size());
        let line = window.text_system().shape_line(shown, size, &runs, None);
        let content_x = |i: usize| if f.content.is_empty() { px(0.0) } else { line.x_for_index(i) };
        // Keep the caret in sight: scroll just enough, never past the start.
        let caret_x = content_x(f.cursor());
        let w = bounds.size.width - px(2.0);
        let mut scroll = f.scroll;
        if caret_x - scroll > w {
            scroll = caret_x - w;
        }
        if caret_x < scroll {
            scroll = caret_x;
        }
        if line.width <= w {
            scroll = px(0.0);
        }
        let x0 = bounds.left() - scroll;
        let (selection, caret) = if f.selected.is_empty() {
            // The Mac's insertion point: a thin bar in the text's ink, a touch taller than the letters.
            (None, Some(fill(Bounds::new(point(x0 + caret_x, bounds.top() - px(1.0)), size_(px(1.5 * zoom()), bounds.size.height + px(2.0))), col(hue::I12))))
        } else {
            // The selection in the pink plate's tint, as the Mac shades a selection in its accent.
            let wash = alpha(col(ink::PINK), 0.35);
            (Some(fill(Bounds::from_corners(point(x0 + content_x(f.selected.start), bounds.top()), point(x0 + content_x(f.selected.end), bounds.bottom())), wash)), None)
        };
        Prepaint { line, caret, selection, scroll }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), p: &mut Prepaint, window: &mut Window, cx: &mut App) {
        let focus = self.field.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.field.clone()), cx);
        let clip = Bounds { origin: point(bounds.left() - px(2.0), bounds.top() - px(2.0)), size: size_(bounds.size.width + px(4.0), bounds.size.height + px(4.0)) };
        let line = p.line.clone();
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            if let Some(s) = p.selection.take() {
                window.paint_quad(s);
            }
            let _ = line.paint(point(bounds.left() - p.scroll, bounds.top()), window.line_height(), TextAlign::Left, None, window, cx);
            let focused = focus.is_focused(window);
            let shows = self.field.read(cx).caret_shows();
            if focused && shows && let Some(c) = p.caret.take() {
                window.paint_quad(c);
            }
        });
        let scroll = p.scroll;
        let had = focus.is_focused(window);
        self.field.update(cx, |f, _| {
            f.had_keys = had;
            f.last_layout = Some(line);
            f.last_bounds = Some(bounds);
            f.scroll = scroll;
        });
    }
}

fn size_(w: Pixels, h: Pixels) -> Size<Pixels> {
    gpui::size(w, h)
}

/// For a test: types `text` into the field that has the keyboard, or presses a key there
/// (`cmd-a`, `enter`, `escape`…), through the window's own key handling. True if a field took it.
pub fn key_into(window: &mut Window, keystroke: &str, cx: &mut App) -> bool {
    // Test commands only: the few words a test sends are kept for the run.
    let keystroke: &'static str = Box::leak(keystroke.to_string().into_boxed_str());
    let parsed = Keystroke::parse(keystroke).ok();
    match parsed {
        Some(k) => window.dispatch_keystroke(k, cx),
        None => false,
    }
}

/// Every field's action handlers in one place, for a caller that builds its own drawing.
pub fn actions(el: Stateful<Div>, e: &Entity<TextField>, submit: Act) -> Stateful<Div> {
    with_actions(el, e, submit)
}

#[cfg(test)]
mod tests {
    use super::{word_end, word_start};

    #[test]
    fn word_moves() {
        let s = "Northwind  Tools, SKU-42";
        assert_eq!(word_start(s, s.len()), 22);
        assert_eq!(word_start(s, 11), 0);
        assert_eq!(word_start(s, 0), 0);
        assert_eq!(word_end(s, 0), 9);
        assert_eq!(word_end(s, 9), 16);
        assert_eq!(word_end(s, s.len()), s.len());
        // Letters beyond ASCII are words too.
        assert_eq!(word_end("café au lait", 0), "café".len());
    }
}

// MARK: - Several lines

/// The text of a field of several lines, wrapped to the box, `pitch` points a line, scrolled
/// down so the caret stays in sight; the selection under it, the caret over it. Put it in a box
/// of the height it should show (its parent's height is what it fills).
pub struct AreaElement {
    pub field: Entity<TextField>,
    pub pitch: f32,
}

pub struct AreaPrepaint {
    paragraphs: std::rc::Rc<Vec<(usize, WrappedLine, Pixels)>>,
    quads: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl Element for AreaElement {
    type RequestLayoutState = ();
    type PrepaintState = AreaPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> AreaPrepaint {
        let style = window.text_style();
        let size = style.font_size.to_pixels(window.rem_size());
        let lh = z(self.pitch);
        let f = self.field.read(cx);
        let content = f.content.clone();
        let (cursor, selected) = (f.cursor(), f.selected.clone());
        let mut scroll_y = f.scroll_y;
        // Shape paragraph by paragraph, so each knows where it starts in the text.
        let mut paragraphs = Vec::new();
        let mut top = px(0.0);
        let mut start = 0;
        for para in content.split('\n') {
            let text = SharedString::from(para.to_string());
            let run = TextRun { len: text.len(), font: style.font(), color: style.color, background_color: None, underline: None, strikethrough: None };
            let lines = window.text_system().shape_text(text, size, &[run], Some(bounds.size.width), None).unwrap_or_default();
            let line = lines.into_iter().next().unwrap_or_default();
            let h = line.size(lh).height.max(lh);
            paragraphs.push((start, line, top));
            top += h;
            start += para.len() + 1;
        }
        let total = top;
        // Where an index is drawn, unscrolled.
        let at = |i: usize| -> Point<Pixels> {
            let Some((s, line, t)) = paragraphs.iter().rev().find(|(s, _, _)| *s <= i) else { return point(px(0.0), px(0.0)) };
            let p = line.position_for_index((i - s).min(line.len()), lh).unwrap_or_default();
            point(p.x, p.y + *t)
        };
        // Keep the caret in sight.
        let c = at(cursor);
        if c.y + lh - scroll_y > bounds.size.height {
            scroll_y = c.y + lh - bounds.size.height;
        }
        if c.y < scroll_y {
            scroll_y = c.y;
        }
        scroll_y = scroll_y.min((total - bounds.size.height).max(px(0.0))).max(px(0.0));
        let origin = point(bounds.left(), bounds.top() - scroll_y);
        let mut quads = Vec::new();
        let mut caret = None;
        if selected.is_empty() {
            caret = Some(fill(Bounds::new(point(origin.x + c.x, origin.y + c.y + px(1.0)), size_(px(1.5 * zoom()), lh - px(2.0))), col(hue::I12)));
        } else {
            // The selection a visual line at a time, in the pink plate's tint.
            let wash = alpha(col(ink::PINK), 0.35);
            let (a, b) = (at(selected.start), at(selected.end));
            let right = bounds.size.width;
            let mut y = a.y;
            while y <= b.y + px(0.5) {
                let x0 = if (y - a.y).abs() < px(0.5) { a.x } else { px(0.0) };
                let x1 = if (y - b.y).abs() < px(0.5) { b.x } else { right };
                if x1 > x0 {
                    quads.push(fill(Bounds::from_corners(point(origin.x + x0, origin.y + y), point(origin.x + x1, origin.y + y + lh)), wash));
                }
                y += lh;
            }
        }
        let paragraphs = std::rc::Rc::new(paragraphs);
        let shown = paragraphs.clone();
        self.field.update(cx, |f, _| {
            f.paragraphs = shown;
            f.line_height = lh;
            f.scroll_y = scroll_y;
            f.last_bounds = Some(bounds);
        });
        AreaPrepaint { paragraphs, quads, caret }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), p: &mut AreaPrepaint, window: &mut Window, cx: &mut App) {
        let (focus, scroll_y) = {
            let f = self.field.read(cx);
            (f.focus.clone(), f.scroll_y)
        };
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.field.clone()), cx);
        let lh = z(self.pitch);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for q in p.quads.drain(..) {
                window.paint_quad(q);
            }
            for (_, line, top) in p.paragraphs.iter() {
                let _ = line.paint(point(bounds.left(), bounds.top() + *top - scroll_y), lh, TextAlign::Left, None, window, cx);
            }
            let focused = focus.is_focused(window);
            let shows = self.field.read(cx).caret_shows();
            if focused && shows && let Some(c) = p.caret.take() {
                window.paint_quad(c);
            }
        });
        let had = focus.is_focused(window);
        self.field.update(cx, |f, _| f.had_keys = had);
    }
}

/// A field of several lines in `w`×`h` of a parent's drawing: the text (in `ts`, `pitch` a line)
/// and the field's keys and clicks, without a well (the caller draws its own).
pub fn area(field: &Field, label: &str, w: f32, h: f32, ts: Ts, pitch: f32) -> AnyElement {
    let entity = field.entity.clone();
    let el = div().w(z(w)).h(z(h))
        .child(styled(ts, hue::I12).w(z(w)).h(z(h)).whitespace_normal().child(AreaElement { field: entity.clone(), pitch }))
        .id(SharedString::from(format!("area-{label}")))
        .key_context(CONTEXT)
        .track_focus(&field.focus)
        .on_click({
            let f = field.focus.clone();
            move |_, window, cx| window.focus(&f, cx)
        })
        .cursor(CursorStyle::IBeam)
        .role(Role::TextInput)
        .aria_label(label.to_string());
    with_actions(el, &entity, live::act(|_, _| {})).into_any_element()
}
