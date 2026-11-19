//! Learning words (`LearnWords`, `KeepWordNote`): after a Replace, Iris looks at the box for a
//! little while. If the writer puts back a word Iris changed (a name, or a spelling they meant),
//! Iris asks once, beside the box, whether to leave that word alone from now on. Keep it adds
//! it to Words (the glossary) and says so; No, or twelve seconds, lets the question go.

use std::time::{Duration, Instant};

use gpui::*;

use crate::controls::{self, Button};
use crate::interact::{self as live, Anim};
use crate::live::news;
use crate::parts::*;
use crate::riso::{hue, ink};
use crate::textbridge::{self, Captured};

/// How long to keep looking after a paste: long enough for ⌘Z or a quick retype.
const WATCH_SECONDS: u32 = 25;

thread_local! {
    /// The watch running (a new Replace starts another).
    static WATCHING: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static SHOWN: std::cell::RefCell<Option<WindowHandle<KeepView>>> = const { std::cell::RefCell::new(None) };
}

/// `WordWatch.contains`: `word` stands on its own in `text` (no letter or digit either side).
pub fn contains(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let at = from + i;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
            return true;
        }
        from = at + word.len();
    }
    false
}

/// Starts looking, once Iris's version has been pasted. `keep`: the words the brain says the
/// writer might put back (from, to).
pub fn watch(captured: Captured, keep: Vec<(String, String)>, cx: &mut App) {
    if keep.is_empty() || !captured.element_known() {
        return;
    }
    let id = WATCHING.with(|w| {
        w.set(w.get() + 1);
        w.get()
    });
    cx.spawn(async move |cx| {
        // First see Iris's version land, so a paste that failed isn't read as "put back".
        let mut landed = false;
        for _ in 0..WATCH_SECONDS {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            if WATCHING.with(|w| w.get()) != id {
                return;
            }
            let c = captured.clone();
            let Some(text) = cx.background_executor().spawn(async move { textbridge::current_text(&c) }).await else { continue };
            if !landed {
                landed = keep.iter().any(|(_, to)| contains(&text, to));
                continue;
            }
            if let Some((from, _)) = keep.iter().find(|(from, to)| contains(&text, from) && !contains(&text, to)) {
                let (word, frame) = (from.clone(), captured.field_frame);
                let _ = cx.update(|cx| ask(&word, frame, cx));
                return;
            }
        }
    })
    .detach();
}

/// The question: "keep “word”?", with Keep it and No.
struct KeepView {
    word: String,
    near: Option<[f64; 4]>,
    born: Instant,
    leaving: Option<Instant>,
    anim: std::rc::Rc<std::cell::RefCell<Anim>>,
}

/// Its paper's padding (`notePaper`: 16 across, 10 down) and the window's room for the shadow.
const PAD: (f32, f32) = (16.0, 10.0);
const ROOM: f32 = 14.0;
const CAPTION: &str = "You changed it back. Iris won't touch it again.";

fn lead(word: &str) -> String {
    format!("keep \u{201C}{word}\u{201D}?")
}

/// The note's size: the mark, the words, the two buttons.
fn size(word: &str) -> (f32, f32) {
    let words_w = brush_w(&lead(word), 22.0).max(mono_w(CAPTION, 11.0, 0.0));
    let w = 24.0 + 12.0 + words_w + 12.0 + Button::secondary("Keep it").compact().width() + 12.0 + Button::ghost("No").compact().width();
    let h = line_box(22.0, BRUSH_LINE) + 1.0 + 11.0 * MONO_LINE;
    (w + 2.0 * PAD.0, h.max(26.0) + 2.0 * PAD.1)
}

impl Render for KeepView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::riso::let_go_of_last_frame(window);
        let slow = slow();
        let alpha = match self.leaving {
            Some(l) => 1.0 - ds_curve(l.elapsed().as_secs_f32() / slow / 0.12),
            None => ds_curve(self.born.elapsed().as_secs_f32() / slow / 0.15),
        };
        if self.leaving.is_some() || self.born.elapsed().as_secs_f32() < 0.2 * slow {
            window.request_animation_frame();
        }
        let scale = window.scale_factor();
        let (w, h) = size(&self.word);
        let (word, near) = (self.word.clone(), self.near);
        let anim = self.anim.clone();
        let body = live::build(&anim, crate::live_slip::reduce_motion(), window, cx, || {
            let qh = line_box(20.0, BRUSH_LINE);
            let mark = sized(24.0, 24.0)
                .child(at(t("?", brush(20.0), ink::PINK_TEXT), (24.0 - brush_w("?", 20.0)) / 2.0, (24.0 - qh) / 2.0))
                .child(at(crate::toast::brush_ring(ink::PINK_TEXT, 4, 24.0, 24.0, 1.0, scale), 0.0, 0.0));
            let words = div().flex().flex_col().gap(z(1.0))
                .child(t(lead(&word), brush(22.0), hue::I12))
                .child(t(CAPTION, caption(), hue::I10));
            let w1 = word.clone();
            let keep = controls::button(&Button::secondary("Keep it").compact().id("keep-it"), scale, live::act(move |_, cx| {
                // `AppSettings.learn`: into Words, once.
                let w = w1.clone();
                crate::app::change(|s| if !s.glossary.contains(&w) { s.glossary.push(w.clone()) });
                crate::brain::configure(cx);
                news(&format!("kept {w1}"));
                close(cx);
                crate::notes_live::show(crate::toast::Message::kept(&w1), None, near, cx);
            })).0;
            let no = controls::button(&Button::ghost("No").compact().id("keep-no"), scale, live::act(|_, cx| {
                news("keep: no");
                close(cx);
            })).0;
            let row = div().flex().flex_row().gap(z(12.0)).items_center().h(z(h - 2.0 * PAD.1))
                .child(mark).child(words).child(keep).child(no);
            crate::toast::paper_shadow(sized(w, h), w, h, 6.0, 7.0, 0.18)
                .child(at(sized(w, h).bg(crate::riso::col(ink::PAPER)), 0.0, 0.0))
                .child(at(printed(w, h, &crate::riso::grain(w, h, scale, 0.22)), 0.0, 0.0))
                .child(at(row, PAD.0, PAD.1))
        });
        let spoken = format!("Keep {} as you wrote it? {CAPTION}", self.word);
        div().size_full().opacity(alpha)
            .child(div().p(px(ROOM)).child(div().id("keep-word").role(Role::Group).aria_label(spoken).child(body)))
    }
}

/// Lets the question go (fading out).
pub fn close(cx: &mut App) {
    let Some(h) = SHOWN.with(|s| s.borrow_mut().take()) else { return };
    let _ = h.update(cx, |v, _, cx| {
        v.leaving = Some(Instant::now());
        cx.notify();
    });
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs_f32(0.13 * slow())).await;
        let _ = cx.update(|cx| h.update(cx, |_, window, _| window.remove_window()));
    })
    .detach();
}

/// Asks whether to keep `word`, in the message's place by the box at `near`.
pub fn ask(word: &str, near: Option<[f64; 4]>, cx: &mut App) {
    close(cx);
    crate::notes_live::dismiss(cx);
    news(&format!("keep {word}?"));
    let (w, h) = size(word);
    let (ww, wh) = (w + 2.0 * ROOM, h + 2.0 * ROOM);
    // `FloatingNote.origin`: its right edge 14 past the box's, its bottom 4 into the box's top.
    let origin = match near {
        Some([x, y, bw, _]) => (x + bw - ww as f64 + ROOM as f64, y + 4.0 - wh as f64),
        None => {
            let (px_, py) = crate::test_display().filter(|_| textbridge::under_test()).map(|(x, y)| (x + 960.0, y + 540.0)).unwrap_or_else(crate::menubar::pointer);
            (px_ - ww as f64 / 2.0, py - 8.0 - wh as f64)
        }
    };
    let origin = match crate::test_display() {
        Some((dx, dy)) if textbridge::under_test() && !crate::on_test_display(origin.0, origin.1) => (dx + 960.0 - ww as f64 / 2.0, dy + 540.0),
        _ => (origin.0, origin.1.max(25.0)),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(origin.0 as f32), px(origin.1 as f32)), size: size_(ww, wh) })),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let word = word.to_string();
    let Ok(handle) = cx.open_window(options, |window, cx| {
        crate::no_system_shadow(window);
        cx.new(|_| KeepView { word, near, born: Instant::now(), leaving: None, anim: Default::default() })
    }) else { return };
    SHOWN.with(|s| *s.borrow_mut() = Some(handle));
    // Asked once; if it's ignored, it goes on its own.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs_f32(12.0 * slow())).await;
        let _ = cx.update(|cx| {
            if SHOWN.with(|s| s.borrow().as_ref() == Some(&handle)) {
                close(cx);
            }
        });
    })
    .detach();
}

fn size_(w: f32, h: f32) -> Size<Pixels> {
    gpui::size(px(w), px(h))
}

#[cfg(test)]
mod tests {
    use super::contains;

    #[test]
    fn whole_words_only() {
        assert!(contains("Hi Abhijyan, thanks", "Abhijyan"));
        assert!(!contains("Hi Abhijyans", "Abhijyan"));
        assert!(contains("colour.", "colour"));
        assert!(!contains("", "x"));
        assert!(!contains("anything", ""));
    }
}
