//! The answer panel (`PanelView`): a slip of paper over the text box. The tone row, the
//! answer with Iris's words printed on a pink marker and removals struck through, and the
//! bottom row: what you need to know, the app Replace types into, and the one button.

use gpui::{Role, AnyElement, BoxShadow, Div, HighlightStyle, InteractiveElement, IntoElement, ParentElement, ScrollHandle, StatefulInteractiveElement,
           StrikethroughStyle, Styled, StyledText, canvas, div, point};

use crate::model::{Action, AppLabel, Note, Panel, Phase, Seg, TONES};
use crate::parts::*;
use crate::riso::{self, Img, Plate, Rgb8, col, hue, ink, over};

pub const WIDTH: f32 = 620.0;
/// Room around the paper for its shadow (`PanelView.margin`).
pub const MARGIN: (f32, f32, f32) = (26.0, 12.0, 44.0);
const TEXT_W: f32 = WIDTH - 32.0;
const SUB: f32 = 13.5;
const LINE: f32 = 17.0;
const LINE_GAP: f32 = 5.0;
/// The paper without its answer: the tones, the rules and the bottom row (measured live as
/// `panel_chrome_h`; this is its usual height).
pub const CHROME: f32 = 137.0;
/// How far the words fade into the paper at a scrolling answer's edge.
const FADE: f32 = 26.0;
/// The paper under the answer, above the rule over the bottom row.
const BELOW_ANSWER: f32 = 12.0;

/// A scrolling answer's window onto its words: its room and the paper under it, so the words
/// run on to the rule over the bottom row and fade into the paper there, with no blank strip.
fn viewport() -> f32 {
    MAX_BODY.get() + BELOW_ANSWER
}

thread_local! {
    /// The most of the answer the paper shows before it scrolls (`set_max_body`): unlimited
    /// until a live panel says how much of the screen it may take.
    static MAX_BODY: std::cell::Cell<f32> = const { std::cell::Cell::new(f32::INFINITY) };
}

/// The answer's room on a screen `screen_h` points tall: the paper grows with the answer to
/// about half the screen (never less than the panel's own window holds), and a longer answer
/// scrolls inside it.
pub fn max_body(screen_h: f32) -> f32 {
    // Always ending halfway through a line, so the cut line says there's more on any screen.
    let room = (screen_h * 0.5).max(308.0) - CHROME;
    let lines = ((room - LINE / 2.0) / (LINE + LINE_GAP)).floor().max(1.0);
    lines * (LINE + LINE_GAP) + LINE / 2.0
}

/// The room the next panels' answers get (`max_body`).
pub fn set_max_body(h: f32) {
    MAX_BODY.set(h);
}

/// What a view needs from the app besides the panel: the scale, the app icons, and what a
/// click on a tone does (none in a snapshot).
pub struct Look<'a> {
    pub scale: f32,
    pub icon: &'a dyn Fn(&str) -> Option<(Vec<u8>, u32)>,
    pub pick: Option<&'a dyn Fn(usize) -> crate::interact::Act>,
    /// What a click on the button does (live only).
    pub act: Option<crate::interact::Act>,
}

pub fn panel(p: &Panel, look: &Look) -> Div {
    panel_with_body_h(p, look, None)
}

/// Builds the answer panel, optionally using a pre-evaluated animated `(body_h, height_moving)`
/// from `opening::scene` so the paper's height and bottom-anchored placement stay in lockstep on
/// the exact same frame with zero frame lag.
pub fn panel_with_body_h(p: &Panel, look: &Look, body_h_override: Option<(f32, bool)>) -> Div {
    let scale = look.scale;
    // Each tab a button of the shared kit (`ToneTabs`): it darkens under the pointer, squeezes to
    // 0.97 while held, cross-fades between mono and brush in lockstep with the fresh painted pink
    // brushstroke (`210 ms` ease-out, or `150 ms` cross-fade under Reduce Motion).
    let l = crate::interact::get().unwrap_or_else(crate::interact::Live::still);
    let labels: Vec<&str> = if p.tones.is_empty() {
        TONES.to_vec()
    } else {
        p.tones.iter().map(String::as_str).collect()
    };
    let selected = p.selected.min(labels.len().saturating_sub(1));
    let marker = tone_marker_live(&l, "panel-tones", &labels, selected, scale);
    let mut tabs = div().id("tones").role(Role::TabList).aria_label("Tone")
        .relative().flex().flex_row().gap(z(18.0)).child(marker);
    let label_curve = if l.reduce_motion { crate::interact::Curve::FAST } else { crate::interact::Curve::Ease(0.21) };
    for (i, label) in labels.iter().enumerate() {
        let chosen = i == selected;
        let (w, h) = crate::parts::tone_tab_size(label);
        let chosen_k = l.track(&format!("panel-tone-sel-{i}"), chosen as u8 as f32, label_curve);
        let label_s = (*label).to_string();
        let tab = crate::interact::pressable(&l, &format!("tone-{i}"), w, h, 0.97, look.pick.map(|pick| pick(i)),
            move |hover| crate::parts::tone_tab_blend(&label_s, chosen_k, hover).into_any_element());
        // Buttons, as Swift's `ToneTabs` are (each a `Button`, the chosen one selected).
        tabs = tabs.child(tab.role(Role::Button).aria_label(*label).aria_selected(chosen));
    }

    // Critically damped body height (`240 ms`, `0.0` bounce — never wobbles reading text) paired
    // with a calm in-place simultaneous dissolve (`95 ms` out / `160 ms` in, `0 ms` queue).
    let (cur_h, height_moving) = match body_h_override {
        Some(v) => v,
        None if l.view.is_some() => l.spring("panel-body-h", body_height(p), body_spring(p), 0.0),
        None => (body_height(p), false),
    };
    let swap_dur = if l.reduce_motion { 0.14 } else { 0.16 };
    let swap = if l.view.is_some() {
        l.body_swap(p, body_swap_key(p, selected), swap_dur)
    } else {
        None
    };
    let scrolls = !(height_moving || swap.is_some()) && natural_height(p) > MAX_BODY.get() + 0.5;
    let body = if height_moving || swap.is_some() {
        let mut stage = div().relative().w(z(TEXT_W)).h(z(cur_h.max(0.0))).overflow_hidden();
        let (in_alpha, in_dy) = match swap {
            Some((out_panel, elapsed)) => {
                if l.reduce_motion {
                    let a = ds_curve((elapsed / 0.14).clamp(0.0, 1.0));
                    if a < 0.996 {
                        stage = stage.child(at(content(&out_panel, scale).opacity(1.0 - a), 0.0, scrolled(&out_panel)));
                    }
                    (a, 0.0)
                } else {
                    let out_k = ds_curve((elapsed / 0.095).clamp(0.0, 1.0));
                    let in_k = ds_curve((elapsed / 0.16).clamp(0.0, 1.0));
                    if out_k < 0.996 {
                        stage = stage.child(at(content(&out_panel, scale).opacity(1.0 - out_k), 0.0, scrolled(&out_panel) - 1.5 * out_k));
                    }
                    (in_k, 2.5 * (1.0 - in_k))
                }
            }
            None => (1.0, 0.0),
        };
        let incoming = if in_alpha < 0.996 {
            content(p, scale).opacity(in_alpha)
        } else {
            content(p, scale)
        };
        stage.child(at(incoming, 0.0, in_dy)).into_any_element()
    } else if scrolls {
        let handle = answer_scroll(p, selected, &l);
        let room = viewport();
        // The words fade into the paper at an edge with more past it: the bottom while there's
        // more below, the top once some has scrolled up out of view; each fades in over its
        // first line of travel. Paper over the words, so the grain printed over the paper keeps
        // its tooth through it.
        let travel = (natural_height(p) - room).max(0.0);
        let y = (-f32::from(handle.offset().y) / zoom()).clamp(0.0, travel);
        let fade = |a: f32, down: bool| {
            let (clear, paper) = (gpui::Rgba { a: 0.0, ..col(ink::PAPER) }, gpui::Rgba { a: 0.95 * a, ..col(ink::PAPER) });
            let (from, to) = if down { (clear, paper) } else { (paper, clear) };
            div().absolute().left_0().w(z(TEXT_W)).h(z(FADE))
                .bg(gpui::linear_gradient(180.0, gpui::linear_color_stop(from, 0.0), gpui::linear_color_stop(to, 1.0)))
        };
        div().relative().w(z(TEXT_W)).h(z(room))
            .child(div().id("answer-scroll").w(z(TEXT_W)).h(z(room)).overflow_y_scroll().track_scroll(&handle).child(content(p, scale)))
            .child(fade((y / (LINE + LINE_GAP)).min(1.0), false).top_0())
            .child(fade(((travel - y) / (LINE + LINE_GAP)).min(1.0), true).bottom_0())
            .into_any_element()
    } else {
        content(p, scale).into_any_element()
    };

    let paper = div().relative().flex().flex_col().w(z(WIDTH)).bg(col(ink::PAPER))
        .shadow(vec![BoxShadow { color: gpui::black().opacity(0.10), offset: point(z(0.0), z(1.0)), blur_radius: z(1.2), spread_radius: z(0.0), inset: false }])
        .child(div().pl(z(16.0)).pt(z(14.0)).pb(z(10.0)).child(tabs))
        .child(div().pl(z(6.0)).child(pen_rule(11, WIDTH - 12.0, over(hue::I12, 0.85, ink::PAPER), 1.2)))
        .child(div().relative().pl(z(16.0)).pr(z(16.0)).pt(z(14.0)).pb(z(if scrolls { 0.0 } else { BELOW_ANSWER })).child(div().id("answer").role(Role::Label).aria_label(spoken(p)).child(body))
            .children(scrollbar(p, selected, &l, scale).map(|bar| at(bar, 16.0 + TEXT_W + 3.0, 14.0))))
        .child(div().pl(z(16.0)).child(pen_rule(29, TEXT_W, over(hue::I12, 0.3, ink::PAPER), 1.0)))
        .child(footer(p, look))
        // The paper's tooth over everything on it, sized at paint time to the paper.
        .child(div().absolute().top_0().left_0().size_full().child(grain_canvas(scale, height_moving)));
    // `paperShadow(spread: 16, drop: 18, opacity: 0.24)`: a black sheet pulled in 16 points at
    // the sides, dropped 18 and blurred (sigma 18, which is what GPUI's blur radius is), so the
    // shadow pools below. The sheet itself is clear: only its shadow is drawn.
    let sheet = div().absolute().top(z(18.0)).bottom(z(-18.0)).left(z(16.0)).right(z(16.0))
        .shadow(vec![BoxShadow { color: gpui::black().opacity(0.24), offset: point(z(0.0), z(0.0)), blur_radius: z(18.0), spread_radius: z(0.0), inset: false }]);
    div().relative().w(z(WIDTH)).child(sheet).child(paper)
}

/// `.grain(0.22)` over the paper, printed for whatever size the paper comes out.
fn grain_canvas(scale: f32, height_moving: bool) -> AnyElement {
    // Printed for the paper at full size; while it unrolls or smoothly resizes, reuse the last
    // full-size grain texture so intermediate heights never allocate and cache temporary plates.
    // Kept as its size and asked for again through the print cache, so this window still hears
    // when the cache lets go of it (a plate held here would be painted, and kept, after that).
    thread_local! { static LAST: std::cell::Cell<Option<(f32, f32, f32)>> = const { std::cell::Cell::new(None) }; }
    let k = zoom();
    canvas(|_, _, _| (), move |b, _, window, _| {
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        let img: Img = match LAST.with(|l| l.get()) {
            Some((lw, lh, ls)) if k != 1.0 || height_moving => riso::grain(lw, lh, ls, 0.22),
            _ => {
                let size = ((w / k).round(), (h / k).round(), scale);
                LAST.with(|l| l.set(Some(size)));
                riso::grain(size.0, size.1, size.2, 0.22)
            }
        };
        let _ = window.paint_image(b, b, Default::default(), img, 0, false);
    })
    .size_full().into_any_element()
}

/// How long the paper takes to reach a new height (critically damped, no bounce). While an
/// answer streams in it keeps up with the words (160 ms): the newest line is laid out at once and
/// only shows as the paper opens to it. A new tone or the finished answer eases over 240 ms.
pub fn body_spring(p: &Panel) -> f32 {
    if matches!(p.phase, Phase::Generating(_)) { 0.16 } else { 0.24 }
}

thread_local! {
    /// The answer's scroll position, and what it was showing (`answer_scroll`).
    static SCROLL: std::cell::RefCell<(ScrollHandle, String)> = std::cell::RefCell::new((ScrollHandle::new(), String::new()));
}

/// The answer's scrollbar, in the paper's right margin, when the answer scrolls: a faint pen
/// line for the track and a short printed ink stroke for the part in view, as long as that part
/// is of the whole, where it is. Always there while the answer scrolls (you read the panel once:
/// the sign of more can't wait to be found). The grey pen wash under the pointer; drag the stroke,
/// or press the line to jump there. Drawn the same on the Mac and on Windows.
fn scrollbar(p: &Panel, selected: usize, l: &crate::interact::Live, scale: f32) -> Option<AnyElement> {
    let room = viewport();
    let whole = natural_height(p);
    if !room.is_finite() || whole <= room + 0.5 {
        return None;
    }
    const W: f32 = 10.0;
    let travel = whole - room;
    // Where it is: the answer's own scroll while it shows this answer, the top otherwise (a
    // new answer coming in).
    let shown = SCROLL.with(|s| {
        let s = s.borrow();
        (s.1 == body_swap_key(p, selected)).then(|| (-f32::from(s.0.offset().y) / zoom()).clamp(0.0, travel)).unwrap_or(0.0)
    });
    let len = (room * room / whole).max(28.0).min(room);
    let top = shown / travel * (room - len);
    let hv = if l.view.is_some() { l.hover("answer-scrollbar") } else { 0.0 };
    let track = drawn(W, room, crate::pen::hand_line_vertical(53, 0.5, W / 2.0, 0.0, room), Some(1.0), over(hue::I12, 0.3, ink::PAPER));
    let stroke = scroll_stroke(len, scale);
    let mut bar = div().id("answer-scrollbar").relative().w(z(W)).h(z(room))
        .child(at(crate::controls::wash(59, W, room, hv, false), 0.0, 0.0))
        .child(at(track, 0.0, 0.0))
        .child(at(stroke, W / 2.0 - 3.0, top));
    if l.view.is_some() {
        bar = bar.on_hover(l.hover_handler("answer-scrollbar", None));
    }
    // Press, drag and let go, followed wherever the pointer goes once pressed.
    let (grab_len, grab_travel) = (len, travel);
    let drag = gpui::canvas(|_, _, _| (), move |b, _, window, _| {
        let k = zoom();
        let (y0, h) = (f32::from(b.origin.y) / k, f32::from(b.size.height) / k);
        let to = move |y: f32, grab: f32| {
            let t = ((y - y0 - grab) / (h - grab_len)).clamp(0.0, 1.0);
            SCROLL.with(|s| s.borrow().0.set_offset(point(z(0.0), z(-t * grab_travel))));
        };
        window.on_mouse_event(move |e: &gpui::MouseDownEvent, phase, window, _| {
            if phase != gpui::DispatchPhase::Bubble || e.button != gpui::MouseButton::Left || !b.contains(&e.position) {
                return;
            }
            let y = f32::from(e.position.y) / k;
            // On the stroke: hold it where it was taken; on the line: its middle jumps there.
            let grab = if y >= y0 + top && y <= y0 + top + grab_len { y - y0 - top } else { grab_len / 2.0 };
            DRAG.set(Some(grab));
            to(y, grab);
            window.refresh();
        });
        window.on_mouse_event(move |e: &gpui::MouseMoveEvent, _, window, _| {
            if let (Some(grab), true) = (DRAG.get(), e.dragging()) {
                to(f32::from(e.position.y) / k, grab);
                window.refresh();
            }
        });
        window.on_mouse_event(|_: &gpui::MouseUpEvent, _, _, _| DRAG.set(None));
    }).absolute().top_0().left_0().size_full();
    Some(bar.role(Role::ScrollBar).aria_label("Scroll the answer").child(drag).into_any_element())
}

thread_local! {
    /// Where the scrollbar's stroke was taken, from its top, while it's held.
    static DRAG: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// The scrollbar's stroke, `len` points: an ink ribbon pressed in at the top and lifted off,
/// frayed, at the bottom, printed through the riso screen.
fn scroll_stroke(len: f32, scale: f32) -> AnyElement {
    let (w, len) = (6.0, len.round());
    let plate = Plate { w, h: len, scale };
    let img = riso::cached(format!("scroll-stroke-{len}"), &plate, || {
        let line: Vec<[f32; 2]> = (0..=12).map(|i| {
            let t = i as f32 / 12.0;
            [w / 2.0 + 0.35 * (t * 5.0).sin(), 1.5 + t * (len - 5.0)]
        }).collect();
        let mask = plate.fill(&crate::settings_kit::ribbon(&line, 3.4, 0.55, 0.35, true).skia().unwrap());
        riso::print(&plate, ink::INK, 61, &mask, 0.85)
    });
    printed(w, len, &img)
}

/// Scrolls the answer `dy` points (down when positive), when it scrolls (↑ ↓ in the panel).
pub fn scroll_answer(p: &Panel, dy: f32) {
    let travel = natural_height(p) - viewport();
    if !(travel > 0.0) {
        return;
    }
    SCROLL.with(|s| {
        let s = s.borrow();
        let y = (-f32::from(s.0.offset().y) / zoom() + dy).clamp(0.0, travel);
        s.0.set_offset(point(z(0.0), z(-y)));
    });
}

/// How far the answer `p` was scrolled, in points (0 when it fit), so its copy fading out as the
/// next one comes in stays where it was being read.
fn scrolled(p: &Panel) -> f32 {
    if natural_height(p) <= MAX_BODY.get() + 0.5 {
        return 0.0;
    }
    SCROLL.with(|s| f32::from(s.borrow().0.offset().y) / zoom())
}

/// The answer's scroll position, kept while the panel shows the same thing: while it streams
/// (or the model wakes) the newest line stays in sight; a new tone or a finished answer starts
/// at the top.
fn answer_scroll(p: &Panel, selected: usize, l: &crate::interact::Live) -> ScrollHandle {
    SCROLL.with(|s| {
        let mut s = s.borrow_mut();
        let key = body_swap_key(p, selected);
        if s.1 != key {
            s.1 = key;
            s.0.set_offset(point(z(0.0), z(0.0)));
            // A new answer follows from the top, at rest.
            l.spring_reset("panel-scroll");
            if l.view.is_some() {
                l.spring("panel-scroll", 0.0, 0.16, 0.0);
            }
        }
        // Down to the last streamed line (not the bottom of the draft holding the height).
        let shown = match &p.phase {
            Phase::Generating(partial) => para_height(&plain(partial)),
            Phase::Preparing => natural_height(p),
            _ => 0.0,
        };
        let room = viewport();
        if shown > room {
            // Eased up a line at a time as the words arrive (snapped under Reduce Motion).
            let target = room - shown;
            let y = if l.view.is_some() {
                l.spring("panel-scroll", target, 0.16, 0.0).0
            } else {
                target
            };
            s.0.set_offset(point(z(0.0), z(y)));
        }
        s.0.clone()
    })
}

/// How tall the paper for a draft of `text` is likely to stand on a screen `screen_h` points
/// tall: as tall as the draft and a line (an answer often runs a line longer), within what the
/// answer may take (`max_body`), for choosing above or below its box.
pub fn paper_guess(text: &str, screen_h: f32) -> f32 {
    CHROME + (para_height(&plain(text)) + LINE + LINE_GAP).min(max_body(screen_h))
}

/// Deterministic height of the answer's room (`content`, up to where it scrolls) in points,
/// used to drive the critically damped body height spring synchronously before layout so
/// `opening::scene` keeps the paper's bottom edge anchored `10 pt` above the text box with zero
/// frame lag.
pub fn body_height(p: &Panel) -> f32 {
    natural_height(p).min(MAX_BODY.get())
}

/// How tall `content(p, ...)` lays out, all of it.
fn natural_height(p: &Panel) -> f32 {
    match &p.phase {
        Phase::Preparing => para_height(&plain(&p.original)) + 8.0 + 11.0 * MONO_LINE,
        // The draft holds the height while the answer streams in; an answer that runs past it
        // grows the paper a line at a time (Swift's `ZStack` is as tall as the taller of the two).
        Phase::Generating(partial) => para_height(&plain(&p.original)).min(MAX_BODY.get()).max(para_height(&plain(partial))),
        Phase::Failed(_) => para_height(&plain(&p.original)),
        Phase::Done(a) => {
            let mut h = para_height(&runs(p));
            if a.translated {
                let wrote = format!("You wrote: {}", p.original);
                let cols = (TEXT_W / (11.0 * MONO_ADVANCE)).floor().max(1.0) as usize;
                let chars: Vec<(char, Ink)> = wrote.chars().map(|c| (c, Ink::Plain)).collect();
                let lines = wrap(&chars, cols).len().max(1);
                h += 12.0 + lines as f32 * (11.0 * MONO_LINE);
            }
            h
        }
    }
}

fn para_height(runs: &[(String, Ink)]) -> f32 {
    let chars: Vec<(char, Ink)> = runs.iter().flat_map(|(s, k)| s.chars().map(move |c| (c, *k))).collect();
    let adv = SUB * MONO_ADVANCE;
    let columns = (TEXT_W / adv).floor() as usize;
    let n = wrap(&chars, columns).len();
    if n == 0 {
        0.0
    } else {
        n as f32 * LINE + (n - 1) as f32 * LINE_GAP
    }
}

/// Key for body cross-fade: changes on tone switch or phase boundary, never per streamed token.
fn body_swap_key(p: &Panel, selected: usize) -> String {
    match &p.phase {
        Phase::Preparing => format!("{selected}:prep"),
        Phase::Generating(_) => format!("{selected}:gen"),
        Phase::Done(a) => format!("{selected}:done:{}:{}", a.translated as u8, a.text),
        Phase::Failed(f) => format!("{selected}:fail:{}", f.note),
    }
}

// MARK: - The answer

#[derive(Clone, Copy, PartialEq)]
enum Ink {
    Plain,
    Struck,
    Added,
}

/// `resultText`: the diff inline, Iris's words marked, removed words struck, as runs.
fn runs(p: &Panel) -> Vec<(String, Ink)> {
    let Some(a) = p.answer() else { return vec![] };
    // `resultText`: plain when there's nothing to mark, for a translation, and when Settings ›
    // General › Show what changed is off.
    if a.verdict == Some(crate::model::Verdict::Unchanged) || a.translated || !crate::app::settings().show_changes {
        return vec![(a.text.clone(), Ink::Plain)];
    }
    let mut out = Vec::new();
    for (i, seg) in a.segments.iter().enumerate() {
        match seg {
            Seg::Same(s) => out.push((s.clone(), Ink::Plain)),
            Seg::Removed(s) => {
                if s.chars().all(char::is_whitespace) {
                    continue;
                }
                out.push((s.clone(), Ink::Struck));
                if matches!(a.segments.get(i + 1), Some(Seg::Added(_))) {
                    out.push((" ".into(), Ink::Plain));
                }
            }
            Seg::Added(s) => out.push((s.clone(), Ink::Added)),
        }
    }
    out
}

/// Lines as Core Text breaks a monospaced paragraph: greedily at spaces, trailing spaces
/// hanging past the edge. Each line is its characters with their ink.
fn wrap(chars: &[(char, Ink)], columns: usize) -> Vec<Vec<(char, Ink)>> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = start;
        let mut last_break = None;
        let mut i = start;
        while i < chars.len() {
            if chars[i].0 == '\n' {
                break;
            }
            if !chars[i].0.is_whitespace() && i - start + 1 > columns {
                break;
            }
            if chars[i].0.is_whitespace() && (i + 1 == chars.len() || !chars[i + 1].0.is_whitespace()) {
                last_break = Some(i + 1);
            }
            i += 1;
        }
        if i >= chars.len() {
            end = chars.len();
        } else if chars[i].0 == '\n' {
            end = i;
        } else if let Some(b) = last_break {
            end = b;
        } else {
            end = end.max(i).max(start + 1);
        }
        lines.push(chars[start..end].to_vec());
        start = if end < chars.len() && chars[end].0 == '\n' { end + 1 } else { end };
    }
    lines
}

/// A paragraph of the panel's text at 13.5 with 5 points between lines, the pink marker
/// printed behind Iris's words (`PrintedMarker`) and removals struck in espresso.
fn paragraph(runs: &[(String, Ink)], color: Rgb8, scale: f32) -> Div {
    let chars: Vec<(char, Ink)> = runs.iter().flat_map(|(s, k)| s.chars().map(move |c| (c, *k))).collect();
    let adv = SUB * MONO_ADVANCE;
    let columns = (TEXT_W / adv).floor() as usize;
    let lines = wrap(&chars, columns);
    let mut para = div().flex().flex_col().gap(z(LINE_GAP)).w(z(TEXT_W));
    let mut markers = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let s: String = line.iter().map(|c| c.0).collect();
        let mut highlights = Vec::new();
        let mut byte = 0;
        let mut i = 0;
        while i < line.len() {
            let kind = line[i].1;
            let (start_col, start_byte) = (i, byte);
            while i < line.len() && line[i].1 == kind {
                byte += line[i].0.len_utf8();
                i += 1;
            }
            match kind {
                Ink::Struck => highlights.push((start_byte..byte, HighlightStyle {
                    color: Some(col(ink::ESPRESSO).into()),
                    strikethrough: Some(StrikethroughStyle { thickness: z(1.0), color: Some(col(ink::ESPRESSO).into()) }),
                    ..Default::default()
                })),
                Ink::Added => markers.push((n, start_col, i - start_col)),
                Ink::Plain => {}
            }
        }
        // Lines spaced out (`lineSpacing(5)`) sit on whole-point lines in SwiftUI: 17, not 16.2.
        para = para.child(styled(subtitle().line(LINE), color).child(StyledText::new(s).with_highlights(highlights)));
    }
    // The markers go first, behind the words.
    let mut with_markers = div().relative().w(z(TEXT_W));
    for (n, c0, len) in markers {
        let x = c0 as f32 * adv - 1.5;
        let y = n as f32 * (LINE + LINE_GAP) + 1.0;
        let (w, h) = (len as f32 * adv + 3.0, LINE - 2.0);
        with_markers = with_markers.child(at(printed(w, h, &marker_tile(x, y, w, h, scale)), x, y));
    }
    with_markers.child(para)
}

/// The pink behind Iris's words: a 96-point tile of the pink screen, laid from the text's
/// origin, at 62%.
fn marker_tile(x: f32, y: f32, w: f32, h: f32, scale: f32) -> Img {
    thread_local! { static TILE: Vec<Rgb8> = riso::screen(96, 96, ink::PINK, 31, 0.55, 0.6); }
    let plate = Plate { w, h, scale };
    // The screen repeats every 96 points vertically (`y` is always an integer line offset), so
    // reducing `y` modulo 96 yields identical pixels while keeping cache keys bounded across lines.
    let ym = y.rem_euclid(96.0);
    riso::cached(format!("mark-{x}-{ym}-{w}"), &plate, || {
        let (pw, ph) = plate.px();
        let mut out = vec![0u8; (pw * ph * 4) as usize];
        TILE.with(|tile| {
            for py in 0..ph {
                let ty = ((ym + (py as f32 + 0.5) / scale).floor() as i64).rem_euclid(96) as usize;
                for px_ in 0..pw {
                    let tx = ((x + (px_ as f32 + 0.5) / scale).floor() as i64).rem_euclid(96) as usize;
                    let c = tile[ty * 96 + tx];
                    let o = ((py * pw + px_) * 4) as usize;
                    out[o..o + 4].copy_from_slice(&[c.0, c.1, c.2, (0.62f32 * 255.0).round() as u8]);
                }
            }
        });
        out
    })
}

fn plain(s: &str) -> Vec<(String, Ink)> {
    vec![(s.to_string(), Ink::Plain)]
}

fn content(p: &Panel, scale: f32) -> Div {
    match &p.phase {
        Phase::Preparing => div().flex().flex_col().gap(z(8.0))
            .child(paragraph(&plain(&p.original), hue::I9, scale))
            .child(div().flex().flex_row().gap(z(6.0)).items_center().child(ink_dots(hue::I11, 4.0)).child(t("Loading the model…", caption(), hue::I11))),
        Phase::Generating(partial) => {
            // The draft's height is held while the answer streams in (up to what the paper shows,
            // so a long draft leaves nothing blank to scroll into); once the answer runs past it,
            // the answer so far holds it (as `body_height` says).
            let held = para_height(&plain(&p.original)).min(MAX_BODY.get());
            let shown = if partial.is_empty() {
                // `WaitingText`: breathe gently (1.0 ↔ 0.45 over 0.8 s each way) while waiting
                // for the first word; still in snapshots and under Reduce Motion.
                let waiting = paragraph(&plain(&p.original), hue::I9, scale);
                if let Some(l) = crate::interact::get().filter(|l| l.view.is_some() && !l.reduce_motion && !crate::app::settings().light_mode_active()) {
                    let t = l.pulse_elapsed();
                    let wave = 0.5 + 0.5 * (std::f32::consts::PI * t / 0.8).cos();
                    waiting.opacity(0.45 + 0.55 * wave)
                } else {
                    waiting
                }
            } else {
                paragraph(&plain(partial), hue::I12, scale)
            };
            div().w(z(TEXT_W)).min_h(z(held)).child(shown)
        }
        Phase::Done(a) => {
            let mut c = div().flex().flex_col().gap(z(12.0)).child(paragraph(&runs(p), hue::I12, scale));
            if a.translated {
                let wrote = format!("You wrote: {}", p.original);
                c = c.child(div().id("you-wrote").role(Role::Label).aria_label(wrote.clone()).w(z(TEXT_W)).font_family(caption().family).text_size(z(11.0)).line_height(z(11.0 * MONO_LINE))
                    .text_color(col(hue::I10)).child(wrote));
            }
            c
        }
        // The draft in grey, untouched: what went wrong is the red pen at the bottom left.
        Phase::Failed(_) => paragraph(&plain(&p.original), hue::I9, scale),
    }
}

// MARK: - The bottom row

/// Cuts `s` to `room` points of mono text at `size`, ending in "…", as SwiftUI truncates.
fn cut(s: &str, size: f32, room: f32) -> String {
    let adv = size * MONO_ADVANCE;
    // Core Text lets a line overrun its room by a hair (widths land on half points).
    let fit = ((room + 0.5) / adv).floor().max(1.0) as usize;
    if s.chars().count() <= fit {
        return s.to_string();
    }
    let kept: String = s.chars().take(fit - 1).collect();
    format!("{}…", kept.trim_end())
}

fn footer(p: &Panel, look: &Look) -> Div {
    let scale = look.scale;
    let f = p.footer();
    let answered = p.answer().is_some();
    let working = matches!(p.phase, Phase::Preparing | Phase::Generating(_));

    // The button, fixed: the shared kit's `TactileButton`, pressing which does `look.act`.
    use crate::controls::{self, Button};
    let act = look.act.clone().unwrap_or_else(|| crate::interact::act(|_, _| {}));
    let button: Option<(gpui::AnyElement, f32)> = match f.action {
        Action::TryAgain => Some(controls::button(&Button::secondary("Try again").key_chip(&crate::keys::command_chip("R")).id("panel-action"), scale, act)),
        Action::None => None,
        a => {
            let title = match a {
                Action::Checking => "Checking…",
                Action::Writing => "Writing…",
                Action::Done => "Done",
                Action::Copy => "Copy",
                _ => "Replace",
            };
            let mut b = Button::primary(title).loading(working).reserve("Checking…").enabled(answered).id("panel-action");
            if answered {
                b = b.key_chip(&crate::keys::key("↵"));
            }
            Some(controls::button(&b, scale, act))
        }
    };

    // The note, fixed unless it's a badge.
    let badge_words = match &f.note {
        Note::Translated => Some("Translated to English, check it reads right"),
        Note::BigRewrite => Some("Big rewrite, read it before replacing"),
        _ => None,
    };
    let fixed_note: Option<(Div, f32)> = match &f.note.clone() {
        Note::Copied => Some(done_note("copied", 20.0, scale)),
        Note::Retry(r) => {
            let mut w = r.clone();
            if let Some(first) = w.get_mut(0..1) {
                first.make_ascii_lowercase();
            }
            Some(done_note(&w, 20.0, scale))
        }
        Note::LooksGood => Some(looks_good(scale)),
        Note::Problem(words) => Some(problem_note(words, scale)),
        Note::Keys { next_tone, try_again } => {
            let mut r = div().flex().flex_row().flex_none().gap(z(12.0));
            let mut w = 0.0;
            if *next_tone {
                r = r.child(key_hint(&crate::keys::key("⇥"), "Next tone", scale));
                w += key_hint_w(&crate::keys::key("⇥"), "Next tone");
            }
            if *try_again {
                r = r.child(key_hint(&crate::keys::command_chip("R"), "Try again", scale));
                w += key_hint_w(&crate::keys::command_chip("R"), "Try again") + if *next_tone { 12.0 } else { 0.0 };
            }
            Some((r, w))
        }
        _ => None,
    };

    // The app: "into" (or "nothing to change in") and its name on tape.
    let app = f.app.as_ref().map(|label| {
        let (lead, name) = match label {
            AppLabel::PastesInto(n) => ("into", n.as_str()),
            AppLabel::NothingToChange(n) => ("nothing to change in", n.as_str()),
        };
        // An app that isn't on this Mac gets Iris's own apps icon, faded (`AppIconView`).
        let icon = p.app_bundle.as_deref().map(|b| match (look.icon)(b) {
            Some(px_) => (px_, false),
            None => (riso::apps_icon(), true),
        });
        (lead, name, icon)
    });

    // SwiftUI's HStack: fixed things take their width; the two that can be cut short share
    // what's left, the less flexible first.
    let mut children = 0;
    let mut fixed = 0.0;
    if let Some((_, w)) = &fixed_note { fixed += w; children += 1; }
    if let Some((_, w)) = &button { fixed += w; children += 1; }
    // Tab's hint at the right when a failed check offers no Try again: with it, the row is the
    // note and the one button, whatever the note's length.
    let hint_w = key_hint_w(&crate::keys::key("⇥"), "Next tone");
    let hint = f.next_tone && button.is_none();
    if hint { fixed += hint_w; children += 1; }
    children += 1; // the spacer
    let mut flex: Vec<(usize, f32, f32)> = Vec::new(); // (which, natural, fixed part)
    if let Some(words) = badge_words {
        children += 1;
        flex.push((0, badge_w(words), 16.0));
    }
    if let Some((_, name, icon)) = &app {
        children += 1;
        // The tape alone beside the button (no "into" before it: the button says what happens).
        let fixed_part = tape_w(0.0, icon.is_some());
        flex.push((1, fixed_part + mono_w(name, 10.5, 0.0), fixed_part));
    }
    let gaps = 12.0 * (children as f32 - 1.0);
    // The spacer keeps its 8-point minimum.
    let mut room = TEXT_W - fixed - gaps - 8.0;
    let mut given = [0.0f32; 2];
    let total: f32 = flex.iter().map(|f| f.1).sum();
    if total <= room {
        for f in &flex {
            given[f.0] = f.1;
        }
    } else {
        flex.sort_by(|a, b| (a.1 - a.2).total_cmp(&(b.1 - b.2)));
        let mut left = flex.len();
        for f in &flex {
            let offer = room / left as f32;
            let take = offer.min(f.1);
            // A cut name reports the width it shows.
            let shown = match f.0 {
                0 => 16.0 + mono_w(&cut(badge_words.unwrap(), 11.0, take - 16.0), 11.0, 0.0),
                _ => f.2 + mono_w(&cut(app.as_ref().unwrap().1, 10.5, take - f.2), 10.5, 0.0),
            };
            given[f.0] = shown.min(take);
            room -= given[f.0];
            left -= 1;
        }
    }

    let mut r = div().flex().flex_row().items_center().gap(z(12.0)).px(z(16.0)).py(z(10.0)).w(z(WIDTH));
    if let Some((e, _)) = fixed_note {
        r = r.child(div().id("note").role(Role::Label).aria_label(note_words(&f.note)).child(e));
    }
    if let Some(words) = badge_words {
        let w = given[0];
        // Heard whole, as Swift's `Badge` text is, even when the line cuts it.
        r = r.child(div().id("badge").role(Role::Label).aria_label(words).child(badge(&cut(words, 11.0, w - 16.0), w)));
    }
    r = r.child(div().flex_grow_1());
    if let Some((lead, name, icon)) = app {
        let fixed_part = tape_w(0.0, icon.is_some());
        let shown = cut(name, 10.5, given[1] - fixed_part);
        let tape = match &icon {
            Some((px_, faded)) => tape_label(&shown, Some(px_), *faded, mono_w(&shown, 10.5, 0.0), scale),
            None => tape_label(&shown, None, false, mono_w(&shown, 10.5, 0.0), scale),
        };
        // VoiceOver says what the tape means.
        let spoken = if lead == "into" { format!("Replaces your text in {name}") } else { format!("Nothing to change in {name}") };
        r = r.child(div().flex().flex_row().flex_none().items_center().child(tape)
            .id("app").role(Role::Label).aria_label(spoken));
    }
    if hint {
        r = r.child(div().id("next-tone").role(Role::Label).aria_label("Tab for the next tone").child(key_hint(&crate::keys::key("⇥"), "Next tone", scale)));
    }
    if let Some((b, _)) = button {
        let (title, keys) = match f.action {
            Action::TryAgain => ("Try again", if crate::keys::is_mac() { "Command+R" } else { "Control+R" }),
            Action::Checking => ("Checking…", ""),
            Action::Writing => ("Writing…", ""),
            Action::Done => ("Done", "Return"),
            Action::Copy => ("Copy", "Return"),
            _ => ("Replace", "Return"),
        };
        let _ = title;
        r = r.child(div().id("action").aria_keyshortcuts(keys).child(b));
    }
    r
}


/// What VoiceOver reads for the answer: the text as it stands (or the draft and the note).
fn spoken(p: &Panel) -> String {
    match &p.phase {
        Phase::Done(a) => a.text.clone(),
        Phase::Generating(partial) if !partial.is_empty() => partial.clone(),
        _ => p.original.clone(),
    }
}

/// What VoiceOver reads for the note at the bottom left.
fn note_words(n: &Note) -> String {
    match n {
        Note::Copied => "copied".into(),
        Note::Retry(r) => r.clone(),
        Note::LooksGood => "Looks good, nothing to fix".into(),
        Note::Problem(words) => format!("{words}. Your text wasn't changed."),
        Note::Keys { next_tone, try_again } => {
            let mut v = Vec::new();
            if *next_tone { v.push("Tab for the next tone"); }
            if *try_again { v.push("Command R to try again"); }
            v.join(", ")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{LINE, LINE_GAP, body_height, body_swap_key};
    use crate::model::{Answer, Phase, Verdict};

    #[test]
    fn body_height_matches_line_count_and_phases() {
        let mut p = crate::opening::panel_state();
        // Short 1-line unchanged answer (no struck words) -> exact 1 line height.
        p.phase = Phase::Done(Answer::fix(&p.original, "PR looks good — one change needed.", Some(Verdict::Unchanged), false));
        assert!((body_height(&p) - LINE).abs() < 1e-3);

        // Diff against a longer original adds struck words, growing past 1 line as `content` renders.
        p.phase = Phase::Done(Answer::tone(&p.original, "PR looks good — one change needed.", false));
        assert!(body_height(&p) >= 2.0 * LINE + LINE_GAP - 1e-3);

        // Long 3+ line answer -> multiple lines with gaps.
        let three = "Hi, I've reviewed the PR and it looks great overall. There is just one thing that needs to be updated before we merge this change into main today.";
        p.phase = Phase::Done(Answer::tone(&p.original, three, false));
        let h3 = body_height(&p);
        assert!(h3 >= 3.0 * LINE + 2.0 * LINE_GAP - 1e-3, "expected 3+ line height, got {h3}");

        // Streaming: the draft's height until the answer runs past it, then the answer's.
        let short = crate::model::Panel { original: "can u send teh invoice for order 4471 before friday".into(), ..p.clone() };
        let mut s = short.clone();
        s.phase = Phase::Generating("Could you".into());
        assert!((body_height(&s) - LINE).abs() < 1e-3, "a short stream keeps the draft's one line");
        s.phase = Phase::Generating(three.into());
        assert!(body_height(&s) >= 3.0 * LINE + 2.0 * LINE_GAP - 1e-3, "a stream past the draft grows the paper");

        // A long answer grows the paper to about half the screen, then scrolls inside it.
        let long = "the two combi boilers left our warehouse this morning and i have checked the tracking. ".repeat(20);
        s.phase = Phase::Generating(long.clone());
        assert!(body_height(&s) > 8.0 * (LINE + LINE_GAP), "unlimited until a live panel says");
        let room = super::max_body(1080.0);
        assert!(room <= 540.0 - super::CHROME && room > 540.0 - super::CHROME - (LINE + LINE_GAP));
        // Ending halfway through a line on any screen, so the cut line says there's more.
        for screen in [700.0, 900.0, 1000.0, 1080.0, 1117.0, 1440.0] {
            let r = super::max_body(screen);
            assert!(((r - LINE / 2.0) / (LINE + LINE_GAP)).fract().abs() < 1e-3, "{screen}: {r}");
        }
        assert!(super::max_body(400.0) >= 7.0 * (LINE + LINE_GAP), "never less than seven lines");
        super::set_max_body(room);
        assert!((body_height(&s) - room).abs() < 1e-3);
        s.phase = Phase::Done(Answer::tone(&s.original, &long, false));
        assert!((body_height(&s) - room).abs() < 1e-3);
        assert!(super::paper_guess(&long, 1080.0) <= 540.0 + 1e-3);
        super::set_max_body(f32::INFINITY);

        // Preparing adds the "Loading the model…" row below the draft.
        p.phase = Phase::Failed(crate::model::Failure { note: "oops".into(), can_retry: true });
        let h_draft = body_height(&p);
        p.phase = Phase::Preparing;
        assert!(body_height(&p) > h_draft + 15.0);

        // Translated Done adds the "You wrote:" caption below the plain answer.
        p.phase = Phase::Done(Answer::fix(&p.original, "Looks good.", Some(Verdict::Unchanged), false));
        let h_plain = body_height(&p);
        p.phase = Phase::Done(Answer::fix(&p.original, "Looks good.", Some(Verdict::Minor), true));
        assert!(body_height(&p) > h_plain + 20.0);
    }

    #[test]
    fn body_swap_key_is_stable_across_streamed_tokens() {
        let mut p = crate::opening::panel_state();
        p.selected = 1;
        p.phase = Phase::Generating(String::new());
        let k0 = body_swap_key(&p, 1);
        p.phase = Phase::Generating("Hi, I've reviewed".into());
        let k1 = body_swap_key(&p, 1);
        assert_eq!(k0, k1, "streaming tokens in the same tone must not trigger body cross-fade");

        // Switching tone or finishing to Done changes the key.
        assert_ne!(k1, body_swap_key(&p, 2));
        p.phase = Phase::Done(Answer::tone(&p.original, "Hi, I've reviewed the PR.", false));
        assert_ne!(k1, body_swap_key(&p, 1));
    }
}
