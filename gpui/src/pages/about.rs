//! Settings › About (`AboutPage`, SettingsAbout.swift): where Iris comes from, the iris in its
//! meadow (`MakersJourney`, drawn still at the moment Reduce Motion and the snapshots draw it),
//! why Iris exists in the maker's words with its highlighted catches (`MakersNote`) and the
//! taped-in signature; then Behind Iris in Privacy's rows, each with its link.
//!
//! The meadow and the iris swaying in it are printed as one picture (GPUI can't turn the
//! iris); so are the taped signature and the links' arrows, which Iris sets off straight.

use std::f32::consts::PI;
use std::sync::OnceLock;

use gpui::{Div, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement, Styled, div};
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

use crate::icons::{self, Draw, Icon};
use crate::parts::*;
use crate::pen;
use crate::raster::Sheet;
use crate::riso::{self, Img as Pixels, Plate, Rgb8, hue, ink};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// The page's sections, `w` points wide.
pub fn content(_v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    let story = column().w(z(w)).gap(z(8.0))
        .child(div().flex_none().py(z(4.0)).child(match moving_live() {
            Some(live) => meadow_live(&live, s).into_any_element(),
            None => meadow(s).into_any_element(),
        }))
        .child(column().w(z(w)).gap(z(14.0)).pt(z(10.0)).pb(z(6.0)).child(makers_note(w, s)).child(signature(w, s)));
    let rows = column().w(z(w)).gap(z(12.0))
        .child(fact(Icon::Words, "Free and open source", "MIT licence. Anyone can read how Iris works or suggest a change.", Some("The code"), w, s))
        .child(hairline(941, w))
        .child(fact(Icon::Model, "Writes with Gemma 4", crate::words::w("Google's open model, running right on this Mac. Its terms and the fonts' licences are in the credits."), Some("Credits"), w, s))
        .child(hairline(942, w))
        .child(fact(Icon::About, "Other work", "Abhishek's other projects, all on GitHub.", Some("More by Abhishek"), w, s))
        .child(hairline(943, w))
        .child(fact(Icon::Updates, "This copy", &crate::words::ws(format!("Iris dev with Gemma 4, on macOS {}.", mac_os())), None, w, s));
    // Both sections in one stack: the page puts 30 between what it's given, About has 34.
    vec![column().w(z(w)).gap(z(34.0))
        .child(section(Some(1), "Where Iris comes from", None, None, Some(story), w, s))
        .child(section(Some(2), "Behind Iris", None, None, Some(rows), w, s))]
}

/// Where each link goes (`Updater.repo`, its credits, the maker's other work).
fn link_url(title: &str) -> &'static str {
    match title {
        "The code" => REPO,
        "Credits" => "https://github.com/imaxisXD/iris#credits",
        _ => "https://github.com/imaxisXD",
    }
}

/// `Updater.repo`.
pub const REPO: &str = "https://github.com/imaxisXD/iris";

/// `ProblemReport.url`: a new GitHub issue with the questions to answer and this copy's
/// versions filled in.
pub fn problem_report_url() -> String {
    let body = crate::words::ws(format!("**What happened?**\n\n\n**What did you expect?**\n\n\n**How can I make it happen again?**\n1.\n\n---\nIris dev · macOS {}", mac_os()));
    let mut q = String::new();
    for b in body.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { q.push(b as char) } else { q.push_str(&format!("%{b:02X}")) }
    }
    format!("{REPO}/issues/new?body={q}")
}

/// The macOS version as people know it ("26.1"), without the build code.
pub fn mac_os() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    // Windows: "11 25H2 (build 26300)" from the registry (its ProductName says 10 on 11 too).
    #[cfg(windows)]
    return V.get_or_init(|| {
        let key = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let read = |name: &str| std::process::Command::new("reg").args(["query", key, "/v", name]).output().ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.lines().find(|l| l.contains(name)).and_then(|l| l.split_whitespace().last().map(str::to_string)));
        let build = read("CurrentBuild").unwrap_or_default();
        let major = if build.parse::<u32>().unwrap_or(0) >= 22000 { "11" } else { "10" };
        format!("{major} {} (build {build})", read("DisplayVersion").unwrap_or_default())
    });
    #[cfg(not(windows))]
    V.get_or_init(|| {
        std::process::Command::new("sw_vers").arg("-productVersion").output().ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "26".into())
    })
}

// MARK: - Behind Iris

/// One fact as Privacy prints them: its riso icon, the fact and one line under it, and what
/// opens in the browser at the right.
fn fact(icon: Icon, title: &str, detail: &str, link: Option<&str>, w: f32, s: f32) -> Div {
    let link_w = link.map_or(0.0, |l| mono_w(l, 12.5, 0.0) + 4.0 + 22.0 * 0.8);
    let words_w = w - 26.0 - 12.0 - if link.is_some() { link_w + 12.0 } else { 0.0 };
    let mut r = row().w(z(w)).gap(z(12.0)).items_start()
        .child(sized(26.0, 23.0).child(at(printed(26.0, 26.0, &icon.image_at(26.0, s)), 0.0, -3.0)))
        .child(column().w(z(words_w)).gap(z(3.0))
            .child(t(title, body(), hue::I12))
            .child(para_floor(detail, caption(), hue::I10, words_w)));
    if let Some(l) = link {
        r = r.child(crate::controls::pen_link(l, link_url(l), s));
    }
    r
}

// MARK: - The signature

/// "made by" in caption and the name on a strip of masking tape (`TapeLabel(size: 11.5)`), the
/// strip a touch off straight (printed whole).
fn signature(w: f32, s: f32) -> gpui::Stateful<Div> {
    let size = 11.5;
    let tw = mono_w("Abhishek", size, 0.0);
    let pad_x = size * 0.62 + 4.0;
    let pad_y = size * 0.26;
    let (sw, sh) = (tw + 2.0 * pad_x, size * MONO_LINE + 2.0 * pad_y);
    let strip = turned(format!("settings-tape-sign-{sw}"), sw, sh, 4.0, -1.5, s, move |sheet| {
        sheet.fill(&pen::tape(sw, sh).skia().unwrap(), riso::multiply(hue::TAPE, ink::PAPER), 1.0);
        word_into(sheet, martian(size, 430), "Abhishek", pad_x, pad_y + mono_base(size, size * MONO_LINE), hue::I12);
    });
    let made = mono_w("made by", 11.0, 0.0);
    let total = made + 8.0 + sw;
    row().w(z(w)).child(row().ml(z((w - total) / 2.0)).gap(z(8.0)).items_center().child(t("made by", caption(), hue::I10)).child(strip))
        .id("settings-signature").aria_label("Iris is made by Abhishek.")
}

// MARK: - The maker's note (`MakersNote`)

/// A catch's critter (`RisoIcon`, as `ScribbleCritter` draws it).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Critter {
    PriceTag,
    Clock,
    Privacy,
    WatchingCloud,
    Model,
    Tray,
    Wifi,
    PaperPlane,
}

/// Each catch is one of these: grumpy, side-eyeing, sad or worn out (none of them happy).
#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Grumpy,
    SideEye,
    Sad,
    Tired,
}

impl Critter {
    fn raw(self) -> &'static str {
        match self {
            Critter::PriceTag => "priceTag",
            Critter::Clock => "clock",
            Critter::Privacy => "privacy",
            Critter::WatchingCloud => "watchingCloud",
            Critter::Model => "model",
            Critter::Tray => "tray",
            Critter::Wifi => "wifi",
            Critter::PaperPlane => "paperPlane",
        }
    }
    fn mood(self) -> Mood {
        match self {
            Critter::PriceTag | Critter::Privacy => Mood::Grumpy,
            Critter::WatchingCloud | Critter::Model => Mood::SideEye,
            Critter::Wifi | Critter::PaperPlane => Mood::Sad,
            Critter::Clock | Critter::Tray => Mood::Tired,
        }
    }
    fn seed(self) -> i64 {
        riso::str_seed(self.raw(), 5) & 0xffff
    }
}

enum Piece {
    Words(&'static str),
    /// The catches it takes turns showing, and what follows it.
    Token(&'static [(Critter, &'static str)], &'static str),
    Together(&'static str),
}

const PIECES: [Piece; 7] = [
    Piece::Words("I wanted help with my writing that didn’t"),
    Piece::Token(&[(Critter::PriceTag, "cost a monthly fee"), (Critter::Clock, "need a subscription"), (Critter::Privacy, "lock rewrites behind a plan")], ","),
    Piece::Words("didn’t send every sentence to"),
    Piece::Token(&[(Critter::WatchingCloud, "someone else’s server"), (Critter::Model, "a stranger’s computer"), (Critter::Tray, "a company’s data center")], ","),
    Piece::Words("and didn’t"),
    Piece::Token(&[(Critter::Wifi, "go quiet offline"), (Critter::PaperPlane, "give up on a plane"), (Critter::Clock, "stall on slow Wi-Fi")], "."),
    Piece::Together("So I made Iris."),
];

/// One phrase's catch as it changes (`Token`'s `@State`): which option shows, the one before,
/// when it changed, when its wait for the next turn began, and when it came on screen.
struct TokenState {
    index: usize,
    previous: usize,
    changed: Option<std::time::Instant>,
    wait_from: std::time::Instant,
    shown_at: std::time::Instant,
    hovered: bool,
}

thread_local! {
    static TOKENS: std::cell::RefCell<Vec<TokenState>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The morph: 200 ms on an expo-out curve; letters leave in 50 ms, arrive in 100 ms after 50.
const MOVE: f64 = 0.2;
/// The sticker grows in this long, then springs back to rest.
const GROW: f64 = 0.38;
/// Changing on its own: the first change after this long, the phrases this far apart, then
/// each changes again after `CYCLE`.
const FIRST_TURN: f64 = 2.0;
const TURN_GAP: f64 = 1.5;
const CYCLE: f64 = 4.5;
/// After this long a swap has settled (the sticker's spring has rung out).
const SETTLED: f64 = 1.2;
/// The critter blinks for this long after it appears or arrives, every `BLINK_EVERY`.
const LIVELY: f64 = 5.0;
const BLINK_EVERY: f64 = 2.4;
const BLINK_TAKES: f64 = 0.14;
const WINCE: f64 = 0.4;
const GRUMBLE: f64 = 1.2;

const TOKEN_H: f32 = 27.0;
const TOKEN_SIZE: f32 = 16.5;

fn token_w(chars: usize) -> f32 {
    2.5 + (TOKEN_H + 8.0) + chars as f32 * TOKEN_SIZE * MONO_ADVANCE + 6.0
}

/// Why Iris exists, set as a centred headline: the story in Settings' reading size, each catch
/// a size up on a soft pill with its critter poking up from the left end. Lines break as if
/// each catch were its longest (so a change never moves words to another line) and centre on
/// the width they have now. Live, each catch shows the next every few seconds (holding while
/// the pointer is on it, while paused and under Reduce Motion) and a click shows it at once.
fn makers_note(w: f32, s: f32) -> Div {
    let live = crate::interact::get().filter(|l| l.view.is_some());
    if let Some(l) = &live {
        tick_tokens(l);
    }
    let word = martian(13.5, 450);
    let adv = 13.5 * MONO_ADVANCE;
    // A line of 13.5-point words takes 17 points in SwiftUI, the glyphs centred in it.
    let word_h = line_box(13.5, MONO_LINE);
    let lift = (word_h - 13.5 * MONO_LINE) / 2.0;
    // (element, width, height, reserved width)
    let mut items: Vec<(Div, f32, f32, f32)> = Vec::new();
    let mut order = 0;
    for (i, piece) in PIECES.iter().enumerate() {
        match piece {
            Piece::Words(words) => {
                for wd in words.split(' ') {
                    let ww = mono_w(wd, 13.5, 0.0);
                    items.push((sized(ww, word_h).child(at(t(wd, word, hue::I11), 0.0, lift)), ww, word_h, ww));
                }
            }
            Piece::Together(words) => {
                let ww = mono_w(words, 13.5, 0.0);
                items.push((sized(ww, word_h).child(at(t(*words, word, hue::I11), 0.0, lift)), ww, word_h, ww));
            }
            Piece::Token(options, after) => {
                let longest = options.iter().map(|o| o.1.chars().count()).max().unwrap_or(0);
                let aw = adv * after.chars().count() as f32;
                let (e, tw) = match &live {
                    Some(l) => {
                        let (e, tw) = token_live(l, order, options, 500 + i as i64, s);
                        (row().items_center().child(e).child(t(*after, word, hue::I11).ml(z(-3.0))), tw)
                    }
                    None => {
                        let (critter, shown) = options[0];
                        let e = row().items_center()
                            .child(token(critter, shown, 500 + i as i64, s))
                            .child(t(*after, word, hue::I11).ml(z(-3.0)));
                        (div().child(e.id(gpui::SharedString::from(format!("settings-catch-{i}"))).role(Role::Button).aria_label(shown.to_string())), token_w(shown.chars().count()))
                    }
                };
                items.push((div().flex_none().child(e), tw - 3.0 + aw, TOKEN_H, token_w(longest) - 3.0 + aw));
                order += 1;
            }
        }
    }
    // `Flow`: spacing 7, lines 8 apart, centred.
    let mut rows: Vec<Vec<(Div, f32, f32)>> = vec![vec![]];
    let mut reserved = 0.0;
    for (e, iw, ih, room) in items {
        let first = rows.last().unwrap().is_empty();
        let extra = if first { room } else { room + 7.0 };
        if reserved + extra > w && !first {
            rows.push(vec![]);
            reserved = 0.0;
        }
        let first = rows.last().unwrap().is_empty();
        reserved += if first { room } else { room + 7.0 };
        rows.last_mut().unwrap().push((e, iw, ih));
    }
    let mut c = column().w(z(w)).gap(z(8.0));
    for r in rows {
        let rw: f32 = r.iter().map(|x| x.1).sum::<f32>() + 7.0 * (r.len() as f32 - 1.0);
        let rh = r.iter().map(|x| x.2).fold(0.0, f32::max);
        let mut line = sized(w, rh);
        let mut x = (w - rw) / 2.0;
        for (e, iw, ih) in r {
            line = line.child(at(e, x, (rh - ih) / 2.0));
            x += iw + 7.0;
        }
        c = c.child(line);
    }
    c
}

/// The pill a catch sits on, `w` wide: printed pale (a touch darker under the pointer).
fn pill(w: f32, seed: i64, hover: bool, transient: bool, s: f32) -> Pixels {
    let plate = Plate { w, h: TOKEN_H, scale: s };
    let ink_c = if hover { (230, 222, 216) } else { (236, 230, 225) };
    let make = || {
        let mask = plate.fill(&icons::rounded(0.0, 0.0, w, TOKEN_H, TOKEN_H / 2.0));
        riso::print(&plate, ink_c, seed, &mask, 1.0)
    };
    if transient {
        let (pw, ph) = plate.px();
        return riso::frame_image(pw, ph, make());
    }
    let key = if hover { format!("settings-token-pill-hover-{seed}-{w}") } else { format!("settings-token-pill-{seed}-{w}") };
    riso::cached(key, &plate, make)
}

/// One highlighted catch at rest: the pill printed pale, the critter tilted 4° on its left
/// end (turned in its own picture), the words a size up.
fn token(critter: Critter, words: &str, seed: i64, s: f32) -> Div {
    let n = words.chars().count();
    let w = token_w(n);
    let pill = pill(w, seed, false, false, s);
    // The critter's 40-point box centred on the sticker's room, lifted 4; its canvas is 1.7×.
    let room = (TOKEN_H + 8.0, TOKEN_H);
    let (cx, cy) = (2.5 + room.0 / 2.0, room.1 / 2.0 - 4.0);
    let c = 40.0 * 1.7;
    let style = martian(TOKEN_SIZE, 450);
    sized(w, TOKEN_H)
        .child(at(printed(w, TOKEN_H, &pill), 0.0, 0.0))
        .child(at(t(words, style, hue::I12), 2.5 + room.0, (TOKEN_H - TOKEN_SIZE * MONO_LINE) / 2.0))
        .child(at(printed(c, c, &critter_art(critter, s)), cx - c / 2.0, cy - c / 2.0))
}

/// The phrases' clocks, each frame: a phrase whose wait is up shows its next catch. The wait
/// starts again whenever the pointer comes or goes, the meadow is paused or played, or it's
/// clicked (`Token`'s `.task(id:)`).
fn tick_tokens(live: &crate::interact::Live) {
    let now = std::time::Instant::now();
    let slow = crate::parts::slow() as f64;
    TOKENS.with(|ts| {
        let mut ts = ts.borrow_mut();
        let count = PIECES.iter().filter(|p| matches!(p, Piece::Token(..))).count();
        if ts.len() != count {
            *ts = (0..count).map(|_| TokenState { index: 0, previous: 0, changed: None, wait_from: now, shown_at: now, hovered: false }).collect();
        }
        let mut lively = false;
        for (order, st) in ts.iter_mut().enumerate() {
            let hovered = live.is_hovered(&format!("about-catch-{order}"));
            if hovered != st.hovered {
                st.hovered = hovered;
                st.wait_from = now;
            }
            if !paused() && !hovered && !live.reduce_motion {
                let wait = if st.changed.is_none() { FIRST_TURN + order as f64 * TURN_GAP } else { CYCLE };
                if now.duration_since(st.wait_from).as_secs_f64() >= wait {
                    step(st, now);
                }
            }
            let since = st.changed.unwrap_or(st.shown_at);
            lively |= !live.reduce_motion && now.duration_since(since).as_secs_f64() < LIVELY.max(SETTLED) * slow;
        }
        // Playing, the meadow already asks for every frame; paused, a critter still blinks
        // for a while after it arrived, and a click still plays its swap.
        if lively {
            live.anim.borrow_mut().moving = true;
        }
    });
}

/// Shows the next catch.
fn step(st: &mut TokenState, now: std::time::Instant) {
    st.previous = st.index;
    st.index = (st.index + 1) % 3;
    st.changed = Some(now);
    st.wait_from = now;
}

/// expo-out, as cubic-bezier(0.19, 1, 0.22, 1) reads.
fn expo_out(x: f64) -> f64 {
    let c = x.clamp(0.0, 1.0);
    if c >= 1.0 { 1.0 } else { 1.0 - 2f64.powf(-10.0 * c) }
}

/// A letter of a swap: the letter, where it was and where it goes (in letters), and how far its
/// nearest shared neighbour moves, so letters that come or go travel with the crowd.
struct Letter {
    ch: char,
    from: Option<usize>,
    to: Option<usize>,
    shift: i64,
}

/// `Token.letters`: letters both phrases share slide to their places; the others fade.
fn letters(from: &[char], to: &[char]) -> Vec<Letter> {
    let mut used = vec![false; from.len()];
    let mut pairs: Vec<(usize, usize)> = vec![];
    for (j, &c) in to.iter().enumerate() {
        if c == ' ' {
            continue;
        }
        let m = (0..from.len()).filter(|&i| from[i] == c && !used[i]).min_by_key(|&i| (i as i64 - j as i64).abs());
        if let Some(m) = m {
            used[m] = true;
            pairs.push((m, j));
        }
    }
    let shift_near = |i: usize, in_from: bool| -> i64 {
        pairs.iter().min_by_key(|p| ((if in_from { p.0 } else { p.1 }) as i64 - i as i64).abs()).map_or(0, |p| p.1 as i64 - p.0 as i64)
    };
    let mut out: Vec<Letter> = pairs.iter().map(|&(m, j)| Letter { ch: from[m], from: Some(m), to: Some(j), shift: 0 }).collect();
    for (j, &c) in to.iter().enumerate() {
        if !pairs.iter().any(|p| p.1 == j) {
            out.push(Letter { ch: c, from: None, to: Some(j), shift: -shift_near(j, false) });
        }
    }
    for (i, &c) in from.iter().enumerate() {
        if !used[i] {
            out.push(Letter { ch: c, from: Some(i), to: None, shift: shift_near(i, true) });
        }
    }
    out
}

/// A live catch: the pill, its critter and its words, as `Token` draws them `t` seconds after
/// it last changed; a real button (Tab, Space and Return reach it in Swift; here the pointer
/// and the accessibility press). Returns it and how wide it is now.
fn token_live(live: &crate::interact::Live, order: usize, options: &'static [(Critter, &'static str)], seed: i64, s: f32) -> (gpui::Stateful<Div>, f32) {
    let now = std::time::Instant::now();
    let slow = crate::parts::slow() as f64;
    let (index, previous, changed, shown_at, hover) = TOKENS.with(|ts| {
        let ts = ts.borrow();
        let st = &ts[order];
        (st.index, st.previous, st.changed, st.shown_at, st.hovered)
    });
    let reduce = live.reduce_motion;
    let ts = match changed {
        _ if reduce => 9.0,
        Some(c) => now.duration_since(c).as_secs_f64() / slow,
        None => 9.0,
    };
    let from: Vec<char> = options[previous].1.chars().collect();
    let to: Vec<char> = options[index].1.chars().collect();
    let adv = TOKEN_SIZE * MONO_ADVANCE;
    let k = expo_out(ts / MOVE);
    let swapping = ts < SETTLED && previous != index;
    let (old_w, new_w) = (from.len() as f32 * adv, to.len() as f32 * adv);
    let width = if swapping { old_w + (new_w - old_w) * k as f32 } else { new_w };
    let w = 2.5 + (TOKEN_H + 8.0) + width + 6.0;
    let id = format!("about-catch-{order}");
    let act = crate::interact::act(move |_, _| {
        TOKENS.with(|ts| {
            if let Some(st) = ts.borrow_mut().get_mut(order) {
                step(st, std::time::Instant::now());
            }
        });
    });
    // The sticker's critter: how it poses now (blinking, wincing, grumbling).
    let since_on = changed.unwrap_or(shown_at);
    let lively = !reduce && now.duration_since(since_on).as_secs_f64() / slow < LIVELY;
    let arrived = if changed.is_some() && !reduce { ts } else { 99.0 };
    let critter = options[index].0;
    let face = Face::at(critter, wall_clock(), if lively { arrived } else { 99.0 }, lively);
    let pill_img = pill(w, seed, hover, swapping && (ts < MOVE * 3.0), s);
    let words_y = (TOKEN_H - TOKEN_SIZE * MONO_LINE) / 2.0;
    let style = martian(TOKEN_SIZE, 450);
    let el = crate::interact::pressable(live, &id, w, TOKEN_H, 0.96, Some(act), move |_| {
        let mut face_el = sized(w, TOKEN_H).child(at(printed(w, TOKEN_H, &pill_img), 0.0, 0.0));
        // The words: at rest, set whole; swapping, letter by letter.
        if swapping && ts < MOVE * 2.0 {
            for l in letters(&from, &to) {
                let (x, opacity) = match (l.from, l.to) {
                    (Some(f), Some(n)) => ((f as f64 + (n as f64 - f as f64) * k) as f32 * adv, 1.0),
                    (None, Some(n)) => ((n as f64 + l.shift as f64 * (1.0 - k)) as f32 * adv, ((ts - 0.05) / 0.1).clamp(0.0, 1.0)),
                    (Some(f), None) => ((f as f64 + l.shift as f64 * k) as f32 * adv, 1.0 - (ts / 0.05).clamp(0.0, 1.0)),
                    _ => (0.0, 0.0),
                };
                if opacity <= 0.0 || l.ch == ' ' {
                    continue;
                }
                face_el = face_el.child(at(t(l.ch.to_string(), style, hue::I12), 2.5 + TOKEN_H + 8.0 + x, words_y).opacity(opacity as f32));
            }
        } else {
            face_el = face_el.child(at(t(options[index].1, style, hue::I12), 2.5 + TOKEN_H + 8.0, words_y));
        }
        // The stickers, 40 points, centred in the sticker's room and lifted 4.
        let c = 40.0 * 1.7;
        let (cx, cy) = (2.5 + (TOKEN_H + 8.0) / 2.0, TOKEN_H / 2.0 - 4.0);
        let lift = if hover { 1.08 } else { 1.0 };
        if swapping {
            if ts < 0.3 {
                let e = crate::rig::curve::ease_out(ts / 0.3);
                let img = critter_frame(options[previous].0, s, 4.0 + (-14.0 - 4.0) * e, 1.0 - 0.5 * e, Face::REST);
                face_el = face_el.child(at(printed(c, c, &img), cx - c / 2.0, cy - c / 2.0).opacity((1.0 - (ts / 0.2).clamp(0.0, 1.0)) as f32));
            }
            let (scale, turn) = if ts < GROW {
                let e = crate::rig::curve::ease_out(ts / GROW);
                (0.65 + 0.55 * e, -5.0 + 14.0 * e)
            } else {
                let d = ts - GROW;
                let ring = (-d * 13.0).exp() * (d * 28.0).cos();
                (1.0 + 0.2 * ring, 4.0 + 5.0 * ring)
            };
            let img = critter_frame(critter, s, turn, scale * lift, face);
            face_el = face_el.child(at(printed(c, c, &img), cx - c / 2.0, cy - c / 2.0).opacity((ts / 0.18).clamp(0.0, 1.0) as f32));
        } else {
            let img = if face == Face::REST { critter_rest(critter, hover, s) } else { critter_frame(critter, s, 4.0, lift, face) };
            face_el = face_el.child(at(printed(c, c, &img), cx - c / 2.0, cy - c / 2.0));
        }
        face_el.into_any_element()
    });
    (el.role(Role::Button).aria_label(options[index].1.to_string()), w)
}

// MARK: - Scribble critters (`ScribbleCritter`)

const BLUE: Rgb8 = (120, 162, 220);
const DEEP_BLUE: Rgb8 = (78, 118, 190);
const YELLOW: Rgb8 = (250, 196, 60);
const CRITTER_INK: Rgb8 = (22, 16, 14);

fn paint(c: Rgb8, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, (a.clamp(0.0, 1.0) * 255.0).round() as u8);
    p.anti_alias = true;
    p
}

fn line_style(w: f32) -> Stroke {
    Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() }
}

fn oval(x: f32, y: f32, w: f32, h: f32) -> Path {
    PathBuilder::from_oval(Rect::from_xywh(x, y, w, h).unwrap()).unwrap()
}

/// `Scribble.scribble`: long looping strokes wandering across the box, seeded.
fn scribble(r: Rect, seed: i64, loops: usize) -> Path {
    let mut rng = riso::Rng::new(seed);
    let mut b = PathBuilder::new();
    let mut at_ = (r.x() + rng.f() * r.width(), r.y() + rng.f() * r.height());
    b.move_to(at_.0, at_.1);
    for _ in 0..loops {
        let next = (r.x() - 4.0 + rng.f() * (r.width() + 8.0), r.y() - 4.0 + rng.f() * (r.height() + 8.0));
        let ctl = ((at_.0 + next.0) / 2.0 + (rng.f() - 0.5) * r.width() * 0.9, (at_.1 + next.1) / 2.0 + (rng.f() - 0.5) * r.height() * 0.9);
        b.quad_to(ctl.0, ctl.1, next.0, next.1);
        at_ = next;
    }
    b.finish().unwrap()
}

fn polygon(points: &[(f32, f32)]) -> Path {
    let mut b = PathBuilder::new();
    b.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        b.line_to(x, y);
    }
    b.close();
    b.finish().unwrap()
}

/// An arc round (`cx`, `cy`) from `a0` to `a1` degrees (y down, as SwiftUI draws it).
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Path {
    let mut b = PathBuilder::new();
    for k in 0..=40 {
        let a = (a0 + (a1 - a0) * k as f32 / 40.0).to_radians();
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        if k == 0 { b.move_to(x, y) } else { b.line_to(x, y) }
    }
    b.finish().unwrap()
}

/// `ScribbleCritter.body`: its shape in a 64 box.
fn critter_body(c: Critter) -> Path {
    match c {
        Critter::PriceTag => polygon(&[(10.0, 33.0), (22.0, 16.0), (58.0, 16.0), (58.0, 50.0), (22.0, 50.0)]),
        Critter::Clock => oval(9.0, 12.0, 46.0, 44.0),
        Critter::Privacy => icons::rounded(12.0, 26.0, 40.0, 32.0, 9.0),
        Critter::WatchingCloud => {
            let mut b = PathBuilder::new();
            for (x, y, r) in [(20.0, 38.0, 13.0), (34.0, 28.0, 16.0), (47.0, 38.0, 12.0), (32.0, 42.0, 14.0)] {
                b.push_oval(Rect::from_xywh(x - r, y - r, r * 2.0, r * 2.0).unwrap());
            }
            b.finish().unwrap()
        }
        Critter::Model => icons::rounded(13.0, 13.0, 38.0, 38.0, 7.0),
        Critter::Tray => icons::rounded(11.0, 24.0, 42.0, 32.0, 5.0),
        Critter::Wifi => oval(16.0, 30.0, 32.0, 30.0),
        Critter::PaperPlane => polygon(&[(60.0, 10.0), (6.0, 34.0), (26.0, 40.0), (34.0, 58.0)]),
    }
}

/// `ScribbleCritter.eyes`: their middle, the gap between them, their radius.
fn critter_eyes(c: Critter) -> ((f32, f32), f32, f32) {
    match c {
        Critter::PriceTag => ((42.0, 33.0), 13.0, 4.6),
        Critter::Clock => ((32.0, 34.0), 15.0, 5.4),
        Critter::Privacy => ((32.0, 42.0), 14.0, 5.0),
        Critter::WatchingCloud => ((33.0, 35.0), 14.0, 5.6),
        Critter::Model => ((32.0, 32.0), 14.0, 5.0),
        Critter::Tray => ((32.0, 40.0), 14.0, 5.0),
        Critter::Wifi => ((32.0, 45.0), 12.0, 4.6),
        Critter::PaperPlane => ((32.0, 34.0), 11.0, 4.2),
    }
}

/// How a critter's face is just now: how open its eyes are (1 open), and how long since it
/// arrived (it winces, then a grumble puffs over it).
#[derive(Clone, Copy, PartialEq)]
struct Face {
    blink: f64,
    arrived: f64,
}

impl Face {
    const REST: Face = Face { blink: 1.0, arrived: 99.0 };

    /// Its face at Swift's clock `now`, `arrived` seconds after it came on, while it's lively.
    fn at(c: Critter, now: f64, arrived: f64, lively: bool) -> Face {
        if !lively {
            return Face::REST;
        }
        let phase = (now + (c.seed() % 97) as f64 * 0.13).rem_euclid(BLINK_EVERY);
        let blink = if phase < BLINK_TAKES { (phase / BLINK_TAKES * 2.0 - 1.0).abs().max(0.12) } else { 1.0 };
        Face { blink, arrived: if arrived < GRUMBLE { arrived } else { 99.0 } }
    }
}

/// The critter's 1.7× canvas (68 points for a 40-point critter), at rest: tilted 4°, eyes
/// open (`lift`: a touch bigger under the pointer).
fn critter_rest(c: Critter, lift: bool, s: f32) -> Pixels {
    if !lift {
        return critter_art(c, s);
    }
    let room = 40.0 * 1.7;
    let plate = Plate { w: room, h: room, scale: s };
    riso::cached(format!("critter-{}-lift", c.raw()), &plate, || critter_rgba(c, s, 4.0, 1.08, Face::REST))
}

/// The critter at rest, straight alpha (the still page's).
fn critter_art(c: Critter, s: f32) -> Pixels {
    let room = 40.0 * 1.7;
    let plate = Plate { w: room, h: room, scale: s };
    riso::cached(format!("critter-{}", c.raw()), &plate, || critter_rgba(c, s, 4.0, 1.0, Face::REST))
}

/// The critter for this frame only, turned `degrees` and scaled `scale` about its middle.
fn critter_frame(c: Critter, s: f32, degrees: f64, scale: f64, face: Face) -> Pixels {
    let room = 40.0 * 1.7;
    let (pw, ph) = Plate { w: room, h: room, scale: s }.px();
    riso::frame_image(pw, ph, critter_rgba(c, s, degrees as f32, scale as f32, face))
}

/// `ScribbleCritter.draw` into its 1.7× canvas: props behind, the body scribbled in blue
/// crayon, its edge gone over twice, props in front, the face, a grumble.
fn critter_rgba(c: Critter, s: f32, degrees: f32, scale: f32, face: Face) -> Vec<u8> {
    let size = 40.0;
    let room = size * 1.7;
    let plate = Plate { w: room, h: room, scale: s };
    let seed = c.seed();
    let (pw, ph) = plate.px();
    let mut pm = Pixmap::new(pw, ph).unwrap();
    let k = size / 64.0;
    let mut tf = Transform::from_scale(s, s).pre_concat(Transform::from_rotate_at(degrees, room / 2.0, room / 2.0));
    if scale != 1.0 {
        tf = tf.pre_translate(room / 2.0, room / 2.0).pre_scale(scale, scale).pre_translate(-room / 2.0, -room / 2.0);
    }
    let tf = tf.pre_translate((room - size) / 2.0, (room - size) / 2.0).pre_scale(k, k);
    let clip = |p: &Path| {
        let mut m = Mask::new(pw, ph).unwrap();
        m.fill_path(p, FillRule::Winding, true, tf);
        m
    };
    let shape = critter_body(c);
    let bounds = shape.bounds();
    let bounds = Rect::from_xywh(bounds.x(), bounds.y(), bounds.width(), bounds.height()).unwrap();
    // A prop in yellow crayon: scribbled inside, its edge gone over.
    let crayon_fill = |pm: &mut Pixmap, p: &Path| {
        let m = clip(p);
        let b = p.bounds();
        pm.stroke_path(&scribble(Rect::from_xywh(b.x(), b.y(), b.width(), b.height()).unwrap(), seed + 3, 14), &paint(YELLOW, 1.0), &line_style(3.8), tf, Some(&m));
        pm.stroke_path(p, &paint(YELLOW, 1.0), &line_style(2.6), tf, None);
    };
    let yellow_line = |pm: &mut Pixmap, p: &Path| pm.stroke_path(p, &paint(YELLOW, 1.0), &line_style(3.8), tf, None);
    // Props behind.
    match c {
        Critter::Wifi => {
            for r in [14.0f32, 22.0] {
                let mut b = PathBuilder::new();
                let rr = r + 8.0;
                for k in 0..=40 {
                    let a = (-140.0 + 100.0 * k as f32 / 40.0).to_radians();
                    let (x, y) = (32.0 + rr * a.cos(), 40.0 + rr * a.sin());
                    if k == 0 { b.move_to(x, y) } else { b.line_to(x, y) }
                }
                yellow_line(&mut pm, &b.finish().unwrap());
            }
        }
        // Two bells.
        Critter::Clock => {
            crayon_fill(&mut pm, &oval(6.0, 6.0, 18.0, 16.0));
            crayon_fill(&mut pm, &oval(40.0, 6.0, 18.0, 16.0));
        }
        // The shackle.
        Critter::Privacy => {
            pm.stroke_path(&arc(32.0, 28.0, 13.0, 180.0, 360.0), &paint(YELLOW, 1.0), &Stroke { width: 6.0, line_cap: LineCap::Round, ..Default::default() }, tf, None);
        }
        // Pins.
        Critter::Model => {
            let mut b = PathBuilder::new();
            for k in 0..3 {
                let v = 22.0 + k as f32 * 10.0;
                for (x0, y0, x1, y1) in [(v, 8.0, v, 14.0), (v, 50.0, v, 56.0), (8.0, v, 14.0, v), (50.0, v, 56.0, v)] {
                    b.move_to(x0, y0);
                    b.line_to(x1, y1);
                }
            }
            yellow_line(&mut pm, &b.finish().unwrap());
        }
        _ => {}
    }
    // The body, scribbled.
    let m = clip(&shape);
    pm.stroke_path(&scribble(bounds, seed, 26), &paint(BLUE, 1.0), &line_style(3.4), tf, Some(&m));
    pm.stroke_path(&scribble(bounds, seed + 7, 10), &paint(DEEP_BLUE, 0.55), &line_style(2.2), tf, Some(&m));
    // The edge gone over twice.
    for pass in 0..2 {
        let mut rng = riso::Rng::new(seed + pass * 11);
        let (dx, dy) = ((rng.f() - 0.5) * 1.6, (rng.f() - 0.5) * 1.6);
        pm.stroke_path(&shape, &paint(BLUE, 1.0), &line_style(3.6), tf.pre_translate(dx, dy), None);
    }
    // Props in front.
    match c {
        Critter::PriceTag => {
            let mut b = PathBuilder::new();
            b.move_to(22.0, 33.0);
            b.cubic_to(8.0, 34.0, 0.0, 20.0, 4.0, 8.0);
            yellow_line(&mut pm, &b.finish().unwrap());
            pm.fill_path(&oval(18.0, 29.0, 8.0, 8.0), &paint(ink::PAPER, 1.0), FillRule::Winding, tf, None);
        }
        Critter::WatchingCloud => crayon_fill(&mut pm, &oval(48.0, 50.0, 7.0, 10.0)),
        // Little feet.
        Critter::Clock => {
            let mut b = PathBuilder::new();
            b.move_to(18.0, 54.0);
            b.line_to(13.0, 61.0);
            b.move_to(46.0, 54.0);
            b.line_to(51.0, 61.0);
            yellow_line(&mut pm, &b.finish().unwrap());
        }
        // The lid.
        Critter::Tray => crayon_fill(&mut pm, &icons::rounded(8.0, 14.0, 48.0, 11.0, 4.0)),
        // Speed lines.
        Critter::PaperPlane => {
            let mut b = PathBuilder::new();
            b.move_to(4.0, 46.0);
            b.line_to(14.0, 42.0);
            b.move_to(8.0, 56.0);
            b.line_to(18.0, 52.0);
            yellow_line(&mut pm, &b.finish().unwrap());
        }
        _ => {}
    }
    // The face: big eyes, a mood (none of them happy), the mouth.
    let mood = c.mood();
    let (at_, gap, r) = critter_eyes(c);
    let (gap, r) = (gap * 1.45, r * 1.5);
    let lid = match mood { Mood::SideEye => 0.45, Mood::Tired => 0.55, _ => 0.0 };
    let look = match (mood, c) { (Mood::SideEye, Critter::Model) => -2.6, (Mood::SideEye, _) => 2.6, _ => 0.0 };
    let wince = face.arrived < WINCE;
    let ink_ = paint(CRITTER_INK, 1.0);
    for (side, dx) in [(-1.0f32, -gap / 2.0), (1.0, gap / 2.0)] {
        let p = (at_.0 + dx, at_.1);
        if wince {
            // > <: each eye squeezed shut toward the middle.
            let mut v = PathBuilder::new();
            v.move_to(p.0 - side * r, p.1 - r * 0.8);
            v.line_to(p.0 + side * r * 0.6, p.1);
            v.line_to(p.0 - side * r, p.1 + r * 0.8);
            pm.stroke_path(&v.finish().unwrap(), &ink_, &line_style(3.2), tf, None);
            continue;
        }
        let h = r * 1.15 * face.blink as f32;
        let eye = oval(p.0 - r, p.1 - h, r * 2.0, h * 2.0);
        pm.fill_path(&eye, &ink_, FillRule::Winding, tf, None);
        if face.blink > 0.6 {
            let hw = r * 0.5;
            pm.fill_path(&oval(p.0 - r * 0.45 + look, p.1 - r * 0.55 + if lid > 0.0 { h * lid } else { 0.0 }, hw, hw), &paint((255, 255, 255), 1.0), FillRule::Winding, tf, None);
        }
        // The upper lid, in the body's blue, with its ink edge.
        if lid > 0.0 {
            let (top, edge) = (p.1 - h, p.1 - h + h * 2.0 * lid);
            let m = clip(&eye);
            pm.fill_rect(Rect::from_xywh(p.0 - r - 1.0, top - 1.0, r * 2.0 + 2.0, edge - top + 1.0).unwrap(), &paint(BLUE, 1.0), tf, Some(&m));
            pm.stroke_path(&icons::line(p.0 - r, edge, p.0 + r, edge), &ink_, &Stroke { width: 2.4, line_cap: LineCap::Round, ..Default::default() }, tf, None);
        }
        // Brows: grumpy ones drop toward the middle, sad ones rise toward it.
        if mood == Mood::Grumpy || mood == Mood::Sad {
            let grumpy = mood == Mood::Grumpy;
            let inner = (p.0 - side * r * 0.9, p.1 - h - if grumpy { 2.0 } else { 8.0 });
            let outer = (p.0 + side * r * 0.9, p.1 - h - if grumpy { 8.0 } else { 3.0 });
            pm.stroke_path(&icons::line(inner.0, inner.1, outer.0, outer.1), &ink_, &line_style(3.2), tf, None);
        }
    }
    // The mouth: a frown, wobbly when sad, flat when side-eyeing.
    let m = (at_.0, at_.1 + r * 1.55 + 3.0);
    let mut b = PathBuilder::new();
    match mood {
        Mood::SideEye => {
            b.move_to(m.0 - 4.0 + look, m.1);
            b.line_to(m.0 + 4.0 + look, m.1 - 0.5);
        }
        Mood::Sad => {
            b.move_to(m.0 - 6.0, m.1 + 2.0);
            b.quad_to(m.0 - 3.0, m.1 - 2.0, m.0, m.1 - 1.0);
            b.quad_to(m.0 + 3.0, m.1 - 2.0, m.0 + 6.0, m.1 + 2.0);
        }
        _ => {
            b.move_to(m.0 - 5.0, m.1 + 2.0);
            b.quad_to(m.0, m.1 - 3.0, m.0 + 5.0, m.1 + 2.0);
        }
    }
    pm.stroke_path(&b.finish().unwrap(), &ink_, &line_style(2.6), tf, None);
    // After arriving: a small dark scribble puff above it, and a sweat drop by a sad one.
    if face.arrived < GRUMBLE {
        let a = face.arrived;
        let k = a / GRUMBLE;
        let pop = crate::rig::curve::spring(a, 0.3, 0.3) as f32;
        let fade = (1.0 - crate::rig::curve::ease_in(k)) as f32;
        if pop > 0.01 {
            let puff = Rect::from_xywh(44.0, 2.0 - 2.0 * k as f32, 17.0 * pop, 11.0 * pop).unwrap();
            pm.stroke_path(&scribble(puff, seed + 41, 8), &paint(CRITTER_INK, 0.7 * fade), &Stroke { width: 1.8, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() }, tf, None);
        }
        if mood == Mood::Sad {
            let y = 18.0 + 14.0 * k as f32;
            let mut d = PathBuilder::new();
            d.move_to(6.0, y - 7.0);
            d.quad_to(0.0, y + 2.0, 6.0, y + 4.0);
            d.quad_to(12.0, y + 2.0, 6.0, y - 7.0);
            let drop = d.finish().unwrap();
            pm.fill_path(&drop, &paint(BLUE, fade), FillRule::Winding, tf, None);
            pm.stroke_path(&drop, &paint(CRITTER_INK, 0.6 * fade), &Stroke { width: 1.4, ..Default::default() }, tf, None);
        }
    }
    straight(&pm)
}

// MARK: - The meadow (`MakersJourney`), still

/// Reduce Motion and the snapshots draw this moment.
const T: f64 = 3.3;
/// Whether the meadow wears SwiftUI's filters (`.blur` on the hill, clouds, leaves and the close
/// butterfly; the butterflies' `hueRotation`, `saturation` and `brightness`): in a live window,
/// as the live Swift app does, and not in `--snap`, as Swift's snapshots draw without them (its
/// leaves and the hill's edge print sharp, all three butterflies orange).
fn filters() -> bool {
    crate::interact::get().is_some_and(|l| l.view.is_some())
}
const W: f32 = 612.0;
const H: f32 = 210.0;
const SKY: Rgb8 = (120, 162, 220);
const HAZE: Rgb8 = (178, 216, 186);
const PLATE_GREEN: Rgb8 = (48, 146, 88);
const PALE_GREEN: Rgb8 = (86, 172, 116);
const DEEP_GREEN: Rgb8 = (28, 104, 60);
const SCENE_INK: Rgb8 = (42, 26, 22);
const SUNFLOWER: Rgb8 = (255, 190, 40);
const PLATE_PINK: Rgb8 = (236, 110, 176);
const HILL: [(f32, f32); 6] = [(-40.0, 108.0), (120.0, 82.0), (260.0, 94.0), (400.0, 74.0), (520.0, 88.0), (652.0, 76.0)];
const MEADOW: [(f32, f32); 5] = [(-40.0, 136.0), (160.0, 126.0), (330.0, 132.0), (480.0, 122.0), (652.0, 127.0)];
const SUN: (f32, f32) = (540.0, 52.0);
const IRIS_SIZE: f32 = 112.0;
const IRIS_BASE: (f32, f32) = (438.0, 156.0);
/// `IrisRig.canvas` and `IrisRig.base`: where the flower stands in its frame.
const RIG_CANVAS: (f32, f32) = (180.0, 200.0);
const RIG_BASE: (f32, f32) = (90.0, 166.0);
/// Parallax: how far each layer slides as the view drifts.
const DRIFT_PERIOD: f64 = 120.0;
const DRIFT_SKY: f32 = 0.5;
const DRIFT_HILL: f32 = 1.5;
const DRIFT_FAR: f32 = 2.5;
const DRIFT_NEAR: f32 = 6.0;

/// `Curve.noise`: a steady pseudo-random number in -1…1 for a seed.
fn noise(seed: i64) -> f64 {
    let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x % 20001) as f64 / 10000.0 - 1.0
}

/// The wind at x: a calm lean, a gust rolling across, a small quick quiver. Degrees.
fn wind(x: f32, t: f64) -> f64 {
    let travel = 12.0 * 70.0;
    let front = (t * 70.0) % travel - 200.0;
    let d = (x as f64 - front) / 160.0;
    3.0 + 5.0 * (-d * d).exp() + (t * 3.1 + x as f64 * 0.07).sin() * 0.7
}

/// How far the view has drifted at `t`, -1…1 (parallax: each layer slides by its share).
fn cam(t: f64) -> f32 {
    (t * 2.0 * std::f64::consts::PI / DRIFT_PERIOD).sin() as f32
}

/// Two inks mixed, a far thing printed a step toward the haze.
fn mix(a: Rgb8, b: Rgb8, k: f64) -> (f32, f32, f32) {
    let c = |x: u8, y: u8| ((x as f64 * (1.0 - k) + y as f64 * k) / 255.0) as f32;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn paint_f(c: (f32, f32, f32), a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(tiny_skia::Color::from_rgba(c.0, c.1, c.2, a.clamp(0.0, 1.0)).unwrap());
    p.anti_alias = true;
    p
}

/// The height of an edge at x, eased between its points.
fn edge_at(pts: &[(f32, f32)], x: f32) -> f32 {
    match pts.iter().position(|p| p.0 >= x) {
        Some(j) if j > 0 => {
            let (a, b) = (pts[j - 1], pts[j]);
            let k = (x - a.0) / (b.0 - a.0).max(1.0);
            let s = k * k * (3.0 - 2.0 * k);
            a.1 + (b.1 - a.1) * s
        }
        _ => pts[0].1,
    }
}

/// `EdgeShape`: a smooth edge through the points, closed down to the bottom or left open.
fn edge_shape(pts: &[(f32, f32)], closed: bool) -> Path {
    let mut b = PathBuilder::new();
    b.move_to(pts[0].0, pts[0].1);
    for j in 1..pts.len() {
        let (a, c) = (pts[j - 1], pts[j]);
        let mx = (a.0 + c.0) / 2.0;
        b.cubic_to(mx, a.1, mx, c.1, c.0, c.1);
    }
    if closed {
        b.line_to(W.max(pts[pts.len() - 1].0), H);
        b.line_to(0.0f32.min(pts[0].0), H);
        b.close();
    }
    b.finish().unwrap()
}

/// `Deckle`: a torn paper edge wandering in and out in small ragged steps.
fn deckle() -> Path {
    let mut rng = riso::Rng::new(7);
    let mut jag = || rng.f() * 4.5;
    let mut b = PathBuilder::new();
    let step = 3.0;
    b.move_to(jag(), jag());
    let mut x = 0.0f32;
    while x < W {
        x += step;
        let j = jag();
        b.line_to(x.min(W), j);
    }
    let mut y = 0.0f32;
    while y < H {
        y += step;
        let j = jag();
        b.line_to(W - j, y.min(H));
    }
    x = W;
    while x > 0.0 {
        x -= step;
        let j = jag();
        b.line_to(x.max(0.0), H - j);
    }
    y = H;
    while y > 0.0 {
        y -= step;
        let j = jag();
        b.line_to(j, y.max(0.0));
    }
    b.close();
    b.finish().unwrap()
}

/// Straight-alpha RGBA as a premultiplied pixmap.
fn pixmap_from(rgba: &[u8], w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new(w, h).unwrap();
    for (d, s) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let a = s[3] as u32;
        for k in 0..3 {
            d[k] = ((s[k] as u32 * a + 127) / 255) as u8;
        }
        d[3] = s[3];
    }
    pm
}

/// A premultiplied pixmap as straight-alpha RGBA.
fn straight(pm: &Pixmap) -> Vec<u8> {
    let mut out = pm.data().to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for k in 0..3 {
                px[k] = ((px[k] as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    out
}

/// SwiftUI's `.blur(radius:)`: a Gaussian with that sigma (in points), as three box blurs.
fn blur(pm: &mut Pixmap, sigma: f32) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let n = 3.0f32;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - n * (wl * wl) as f32 - 4.0 * n * wl as f32 - 3.0 * n) / (-4.0 * wl as f32 - 4.0)).round() as i32;
    let sizes: Vec<i32> = (0..3).map(|i| if i < m { wl } else { wu }).collect();
    let mut buf: Vec<f32> = pm.data().iter().map(|&v| v as f32).collect();
    let mut tmp = vec![0.0f32; buf.len()];
    for size in sizes {
        let r = ((size - 1) / 2).max(0) as i64;
        if r == 0 {
            continue;
        }
        let norm = 1.0 / (2 * r + 1) as f32;
        // Rows, then columns; outside the picture is clear.
        box_pass(&buf, &mut tmp, w, h, 1, w, r, norm);
        box_pass(&tmp, &mut buf, h, w, w, 1, r, norm);
    }
    for (d, v) in pm.data_mut().iter_mut().zip(&buf) {
        *d = v.round().clamp(0.0, 255.0) as u8;
    }
    // Keep it premultiplied: no colour above its alpha.
    for px in pm.data_mut().chunks_exact_mut(4) {
        for k in 0..3 {
            px[k] = px[k].min(px[3]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn box_pass(src: &[f32], dst: &mut [f32], len: usize, lines: usize, step: usize, line_step: usize, r: i64, norm: f32) {
    for l in 0..lines {
        let base = l * line_step;
        for ch in 0..4 {
            let at = |i: i64| -> f32 { if i < 0 || i >= len as i64 { 0.0 } else { src[(base + i as usize * step) * 4 + ch] } };
            let mut acc: f32 = (-r..=r).map(at).sum();
            for i in 0..len as i64 {
                dst[(base + i as usize * step) * 4 + ch] = acc * norm;
                acc += at(i + r + 1) - at(i - r);
            }
        }
    }
}

/// A layer drawn on its own (with room round it, so a blur has ink to spread from past the
/// frame), blurred, and laid on `onto`.
fn layer(onto: &mut Pixmap, s: f32, sigma: f32, filters: bool, draw: impl FnOnce(&mut Pixmap, Transform)) {
    let pad = 24.0;
    let mut l = Pixmap::new(((W + 2.0 * pad) * s).round() as u32, ((H + 2.0 * pad) * s).round() as u32).unwrap();
    draw(&mut l, Transform::from_scale(s, s).pre_translate(pad, pad));
    if filters && sigma > 0.0 {
        blur(&mut l, sigma * s);
    }
    onto.draw_pixmap((-pad * s).round() as i32, (-pad * s).round() as i32, l.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

/// A layer blurred by `sigma` points, made quickly: drawn at `q` pixels a point over `region`
/// (points, room for the blur included), blurred there and laid on `onto` (drawn at `s`)
/// smoothed back up. A blur this wide leaves nothing finer than a few points, so the coarse
/// pixels never show; it's what lets the live meadow blur its leaves every frame.
/// `s / q` must be a whole number (it is: 2 or 4); the layer starts on a whole device pixel.
fn soft(onto: &mut Pixmap, s: f32, q: f32, sigma: f32, region: Rect, draw: impl FnOnce(&mut Pixmap, Transform)) {
    let k = (s / q).round().max(1.0) as u32;
    let q = s / k as f32;
    let (ox, oy) = ((region.x() * s).round() as i32, (region.y() * s).round() as i32);
    let (w, h) = ((region.width() * q).ceil() as u32, (region.height() * q).ceil() as u32);
    let Some(mut l) = Pixmap::new(w.max(1), h.max(1)) else { return };
    draw(&mut l, Transform::from_scale(q, q).pre_translate(-(ox as f32) / s, -(oy as f32) / s));
    blur(&mut l, sigma * q);
    lay_on(onto, &l, ox, oy, k);
}

/// `src` (premultiplied) smoothed up `k` times, its top left at device pixel `ox, oy`, laid
/// over `dst`. Where `src` is clear it's skipped, so a layer that's mostly empty costs little
/// (tiny-skia's own scaled draw shades every pixel it covers: 7 ms for the leaves).
fn lay_on(dst: &mut Pixmap, src: &Pixmap, ox: i32, oy: i32, k: u32) {
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let k = k as i32;
    let sd = src.data();
    let dd = dst.data_mut();
    // Where each device pixel samples: the source pixel before it and how far toward the next
    // (0…256), centres aligned.
    let at = |d: i32, len: i32| -> (i32, i32, u32) {
        let u = ((2 * d + 1) * 128 / k) - 128;
        let i = u.div_euclid(256);
        let f = u.rem_euclid(256) as u32;
        (i.clamp(0, len - 1), (i + 1).clamp(0, len - 1), f)
    };
    for y in oy.max(0)..(oy + sh * k).min(dh) {
        let (y0, y1, fy) = at(y - oy, sh);
        for x in ox.max(0)..(ox + sw * k).min(dw) {
            let (x0, x1, fx) = at(x - ox, sw);
            let p = |xx: i32, yy: i32| ((yy * sw + xx) * 4) as usize;
            let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
            if sd[a + 3] == 0 && sd[b + 3] == 0 && sd[c + 3] == 0 && sd[d + 3] == 0 {
                continue;
            }
            let mut px = [0u32; 4];
            for (ch, v) in px.iter_mut().enumerate() {
                let top = sd[a + ch] as u32 * (256 - fx) + sd[b + ch] as u32 * fx;
                let bottom = sd[c + ch] as u32 * (256 - fx) + sd[d + ch] as u32 * fx;
                *v = (top * (256 - fy) + bottom * fy + 32768) >> 16;
            }
            let o = ((y * dw + x) * 4) as usize;
            let keep = 255 - px[3].min(255);
            for ch in 0..4 {
                dd[o + ch] = (px[ch] + (dd[o + ch] as u32 * keep + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// A sprite of straight RGBA, `px` pixels square, drawn `size` points square at `centre`,
/// turned `degrees`, without smoothing.
#[allow(clippy::too_many_arguments)]
fn sprite(pm: &mut Pixmap, tf: Transform, rgba: &[u8], px: u32, centre: (f32, f32), size: (f32, f32), degrees: f32, flip_y: bool, opacity: f32) {
    sprite_pm(pm, tf, &pixmap_from(rgba, px, px), px, centre, size, degrees, flip_y, opacity);
}

/// The same from a sprite already made a pixmap (the live meadow keeps them).
#[allow(clippy::too_many_arguments)]
fn sprite_pm(pm: &mut Pixmap, tf: Transform, src: &Pixmap, px: u32, centre: (f32, f32), size: (f32, f32), degrees: f32, flip_y: bool, opacity: f32) {
    let k = (size.0 / px as f32, size.1 / px as f32);
    let t = tf.pre_translate(centre.0, centre.1)
        .pre_scale(1.0, if flip_y { -1.0 } else { 1.0 })
        .pre_rotate(degrees)
        .pre_scale(k.0, k.1)
        .pre_translate(-(px as f32) / 2.0, -(px as f32) / 2.0);
    // `.interpolation(.none)` holds while it stands square; turned, the Swift snapshot smooths it.
    let square = (degrees.rem_euclid(180.0)).min(180.0 - degrees.rem_euclid(180.0)) < 1.0;
    let quality = if square { tiny_skia::FilterQuality::Nearest } else { tiny_skia::FilterQuality::Bilinear };
    let paint = PixmapPaint { opacity, quality, ..Default::default() };
    pm.draw_pixmap(0, 0, src.as_ref(), &paint, t, None);
}

/// The garden's butterfly (`Sprites.butterfly`), 36 pixels, straight alpha.
fn butterfly_rgba() -> Vec<u8> {
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
    fn shade(d: &mut Draw) {
        for (w, _) in wings() { d.fill(&w, 160) }
    }
    fn colour(d: &mut Draw) {
        for (w, g) in wings() { d.fill(&w, g) }
    }
    fn inked(d: &mut Draw) {
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
    }
    icons::print(36, 21, &[(ink::CREMA, 1, 1, shade), ((255, 116, 60), -1, -1, colour), (ink::ESPRESSO, 0, 0, inked)])
}

/// SwiftUI's `hueRotation`, `saturation` and `brightness`, on straight RGBA.
fn recolour(rgba: &mut [u8], hue_deg: f32, saturation: f32, brightness: f32) {
    let (c, s) = (hue_deg.to_radians().cos(), hue_deg.to_radians().sin());
    let hm = [
        [0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928],
        [0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283],
        [0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072],
    ];
    let z = saturation;
    let sm = [
        [0.213 + 0.787 * z, 0.715 - 0.715 * z, 0.072 - 0.072 * z],
        [0.213 - 0.213 * z, 0.715 + 0.285 * z, 0.072 - 0.072 * z],
        [0.213 - 0.213 * z, 0.715 - 0.715 * z, 0.072 + 0.928 * z],
    ];
    for px in rgba.chunks_exact_mut(4) {
        let v = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0];
        let a: Vec<f32> = (0..3).map(|i| hm[i][0] * v[0] + hm[i][1] * v[1] + hm[i][2] * v[2]).collect();
        for i in 0..3 {
            let b = sm[i][0] * a[0] + sm[i][1] * a[1] + sm[i][2] * a[2] + brightness;
            px[i] = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// The cumulus: puffs under a mound-shaped envelope (x across, y up from the flat base, r),
/// as shares of its width.
fn grow_cloud(seed: i64, tall: f32, count: i64) -> Vec<(f32, f32, f32)> {
    (0..count).map(|k| {
        let u = (noise(seed * 101 + k * 7) * 0.5 + 0.5) as f32;
        let v = (noise(seed * 37 + k * 13) * 0.5 + 0.5) as f32;
        let top = tall * (PI * u).sin().powf(0.7) * (1.0 - 0.25 * (u - 0.42).abs());
        let y = top * (1.0 - v.powf(2.2) * 0.85);
        let r = (0.03 + 0.06 * (1.0 - y / tall.max(0.01)) + 0.025 * noise(seed + k * 3) as f32) * (0.7 + tall);
        (u, y, r.max(0.02))
    }).collect()
}

struct Blade {
    root: (f32, f32),
    length: f32,
    width: f32,
    bend: f32,
    kink: f32,
    ink: f64,
    bloom: Option<u8>,
    near: f32,
}

fn blades() -> Vec<Blade> {
    let mut out: Vec<Blade> = (0..520i64).map(|i| {
        let (n1, n2, n3, n4) = (noise(i * 7 + 1), noise(i * 13 + 5), noise(i * 29 + 9), noise(i * 41 + 3));
        let x = (noise(i * 53 + 11) * 0.5 + 0.5) as f32 * 700.0 - 44.0;
        let top = edge_at(&MEADOW, x);
        let u = n2.abs() as f32;
        let near = u.powf(1.25);
        let depth = near * (H - top + 16.0);
        let tall = if i % 7 == 0 { 1.6 } else { 1.0 };
        let length = (13.0 + 9.0 * n3 as f32) * (0.6 + 2.5 * near) * tall;
        let bloom = if i % 17 == 0 { Some(0) } else if i % 25 == 0 { Some(1) } else if i % 29 == 0 { Some(2) } else { None };
        Blade { root: (x, top + depth + 2.0), length, width: (0.8 + n4.abs() as f32) * (0.55 + 2.2 * near), bend: n1 as f32 * 0.35,
                kink: n4 as f32 * 0.5, ink: n3.abs(), bloom, near }
    }).collect();
    // Back to front, so nearer grass draws over farther.
    out.sort_by(|a, b| a.root.1.partial_cmp(&b.root.1).unwrap());
    out
}

fn draw_grass(pm: &mut Pixmap, tf: Transform, all: &[Blade], behind: bool, t: f64) {
    let cam = cam(t);
    for (i, b) in all.iter().enumerate() {
        if (b.root.1 < IRIS_BASE.1 - 4.0) != behind {
            continue;
        }
        let slide = cam * (DRIFT_FAR + (DRIFT_NEAR - DRIFT_FAR) * b.near);
        let sway = 0.45 + 0.75 * b.near as f64;
        let steps = 6;
        let mut spine = vec![(b.root.0 + slide, b.root.1)];
        for k in 1..=steps {
            let f = k as f64 / steps as f64;
            let w = wind(b.root.0, t - 0.35 * f).to_radians();
            let angle = b.bend as f64 + (w * 1.4 * sway + b.kink as f64 * 0.6) * f.powf(1.5);
            let seg = b.length / steps as f32;
            let last = spine[spine.len() - 1];
            spine.push((last.0 + angle.sin() as f32 * seg, last.1 - angle.cos() as f32 * seg));
        }
        let (mut left, mut right) = (vec![], vec![]);
        for (k, q) in spine.iter().enumerate() {
            let half = b.width / 2.0 * (1.0 - k as f32 / steps as f32) + 0.15;
            left.push((q.0 - half, q.1));
            right.push((q.0 + half, q.1));
        }
        let mut p = PathBuilder::new();
        p.move_to(left[0].0, left[0].1);
        for k in 1..left.len() {
            p.quad_to(left[k - 1].0, left[k - 1].1, (left[k - 1].0 + left[k].0) / 2.0, (left[k - 1].1 + left[k].1) / 2.0);
        }
        p.line_to(spine[steps].0, spine[steps].1);
        for k in (1..right.len()).rev() {
            p.quad_to(right[k].0, right[k].1, (right[k - 1].0 + right[k].0) / 2.0, (right[k - 1].1 + right[k].1) / 2.0);
        }
        p.line_to(right[0].0, right[0].1);
        p.close();
        let path = p.finish().unwrap();
        let far = 1.0 - b.near as f64;
        if i % 13 == 0 && b.near > 0.35 {
            pm.fill_path(&path, &paint(ink::PINK, 1.0), FillRule::Winding, tf.pre_translate(1.6, 1.2), None);
            pm.stroke_path(&path, &paint(SCENE_INK, 0.75), &Stroke { width: 0.8, ..Default::default() }, tf, None);
        } else {
            let base = if b.ink > 0.45 { DEEP_GREEN } else { PALE_GREEN };
            pm.fill_path(&path, &paint_f(mix(base, HAZE, 0.62 * far * far), 1.0), FillRule::Winding, tf, None);
        }
        if let Some(kind) = b.bloom {
            draw_bloom(pm, tf, kind, spine[steps], i as f64, 0.45 + 0.9 * b.near, t);
        }
    }
}

/// A small flower on a blade's tip, nodding with it: a daisy, a pink cup or a buttercup.
fn draw_bloom(pm: &mut Pixmap, tf: Transform, kind: u8, c: (f32, f32), seed: f64, scale: f32, t: f64) {
    let l = tf.pre_translate(c.0, c.1).pre_scale(scale, scale).pre_rotate(((t * 1.4 + seed).sin() * 8.0) as f32);
    match kind {
        0 => {
            for k in 0..6 {
                let p = l.pre_rotate(k as f32 * 60.0);
                let petal = oval(-1.6, -6.0, 3.2, 5.4);
                pm.fill_path(&petal, &paint(ink::PAPER, 1.0), FillRule::Winding, p, None);
                pm.stroke_path(&petal, &paint(SCENE_INK, 0.8), &Stroke { width: 0.6, ..Default::default() }, p, None);
            }
            pm.fill_path(&oval(-2.0, -2.0, 4.0, 4.0), &paint(SUNFLOWER, 1.0), FillRule::Winding, l, None);
        }
        1 => {
            pm.fill_path(&oval(-3.6, -3.6, 7.2, 7.2), &paint(PLATE_PINK, 1.0), FillRule::Winding, l, None);
            pm.fill_path(&oval(-1.2, -1.2, 2.4, 2.4), &paint((255, 255, 255), 0.9), FillRule::Winding, l, None);
        }
        _ => pm.fill_path(&oval(-3.0, -3.0, 6.0, 6.0), &paint(SUNFLOWER, 1.0), FillRule::Winding, l, None),
    }
}

/// The brush loops spreading round the sun, in a paler sky: each a loop drawn by hand, its
/// radius straying as it goes round; the set breathes and turns very slowly.
fn draw_rings(pm: &mut Pixmap, tf: Transform, t: f64) {
    for k in 0..7 {
        let kf = k as f64;
        let r = 40.0 + kf * 32.0 + (t * 0.5 - kf * 0.6).sin() * 3.0;
        let turn = t * 0.04 * if k % 2 == 0 { 1.0 } else { -1.0 } + kf;
        let mut b = PathBuilder::new();
        for j in 0..=96i64 {
            let a = j as f64 / 90.0 * 2.0 * std::f64::consts::PI + turn;
            let stray = 7.0 * (noise(k * 97 + j % 90) * 0.6 + (a * 3.0 + kf).sin() * 0.5);
            let q = (SUN.0 as f64 + (r + stray) * a.cos(), SUN.1 as f64 + (r + stray) * a.sin() * 0.92);
            if j == 0 { b.move_to(q.0 as f32, q.1 as f32) } else { b.line_to(q.0 as f32, q.1 as f32) }
        }
        let fade = 1.0 - k as f32 * 0.05;
        pm.stroke_path(&b.finish().unwrap(), &paint((148, 186, 236), fade), &line_style(11.0 * fade), tf, None);
    }
}

type Puffs = Vec<(f32, f32, f32)>;

/// The painted cumulus: where each starts across, its flat base, its width, its puffs.
fn cumulus() -> [(f32, f32, f32, Puffs); 3] {
    [(110.0, 116.0, 290.0, grow_cloud(3, 0.48, 90)), (380.0, 62.0, 120.0, grow_cloud(9, 0.26, 46)), (-16.0, 52.0, 104.0, grow_cloud(14, 0.24, 34))]
}

/// Where cloud `i` (starting at `cx`) has drifted at `t`: a few points over minutes.
fn cloud_x(i: usize, cx: f32, t: f64) -> f32 {
    cx + ((t * 2.0 * std::f64::consts::PI / 400.0 + i as f64 * 2.0).sin() * 10.0) as f32
}

/// One cumulus at `x`: a blue-lavender shade, a soft white body, paper where the sun catches,
/// everything above its flat base.
fn draw_cloud(l: &mut Pixmap, tf: Transform, x: f32, cbase: f32, cw: f32, puffs: &Puffs) {
    let (pl, pt) = ((l.width() as f32), (l.height() as f32));
    let mut floor = Mask::new(pl as u32, pt as u32).unwrap();
    floor.fill_path(&PathBuilder::from_rect(Rect::from_xywh(x - cw, cbase - cw * 2.0, cw * 3.0, cw * 2.0).unwrap()), FillRule::Winding, false, tf);
    for (ink_c, dx, dy, k) in [((128u8, 144u8, 196u8), -0.1f32, 0.25f32, 1.0f32), ((150, 168, 212), 0.0, 0.05, 0.98), ((230, 236, 248), 0.18, -0.22, 0.84), ((255, 252, 249), 0.3, -0.38, 0.62)] {
        let mut b = PathBuilder::new();
        for (u, y, r) in puffs {
            let (px, py, pr) = (x + u * cw, cbase - y * cw, r * cw);
            let rr = pr * k;
            b.push_oval(Rect::from_xywh(px + dx * pr - rr, py + dy * pr - rr, rr * 2.0, rr * 2.0).unwrap());
        }
        l.fill_path(&b.finish().unwrap(), &paint(ink_c, 1.0), FillRule::Winding, tf, Some(&floor));
    }
}

/// The far hill's ink: the paler green a step toward the haze.
fn hill_ink() -> Rgb8 {
    let m = |x: u8, y: u8| (x as f64 * 0.7 + y as f64 * 0.3) as u8;
    (m(PALE_GREEN.0, HAZE.0), m(PALE_GREEN.1, HAZE.1), m(PALE_GREEN.2, HAZE.2))
}

/// The hill and the meadow: (edge, ink, seed, line alpha, line width, parallax, blur).
fn lands() -> [(&'static [(f32, f32)], Rgb8, i64, f32, f32, f32, f32); 2] {
    [(&HILL[..], hill_ink(), 21, 0.5, 1.4, DRIFT_HILL, 1.2), (&MEADOW[..], PLATE_GREEN, 22, 0.85, 1.8, DRIFT_FAR, 0.0)]
}

/// A few pink petals carried across on the breeze, turning slowly.
fn draw_petals(pm: &mut Pixmap, base: Transform, t: f64) {
    let span = W as f64 + 60.0;
    for i in 0..5 {
        let fi = i as f64;
        let x = (fi * span / 5.0 + t * 7.0 * (0.8 + 0.1 * fi)) % span - 30.0;
        let y = 70.0 + ((i * 23) % 70) as f64 + (t * 0.6 + fi * 1.7).sin() * 12.0;
        let l = base.pre_translate(x as f32, y as f32).pre_rotate((t * 40.0 + fi * 70.0) as f32).pre_scale((0.5 + 0.5 * (t * 0.9 + fi).sin().abs()) as f32, 1.0);
        pm.fill_path(&oval(-2.5, -4.0, 5.0, 8.0), &paint(ink::PINK, 0.9), FillRule::Winding, l, None);
    }
}

/// Butterflies wandering round the flower on slow figures of eight, wings slow: the garden's
/// orange, a rose and a pale one (`wing_set`).
fn draw_butterflies(pm: &mut Pixmap, base: Transform, t: f64, wings: &[Pixmap; 3]) {
    for (i, (centre, reach, speed, phase)) in [((438.0, 70.0), (70.0, 20.0), 0.16, 0.0), ((200.0, 92.0), (90.0, 18.0), 0.12, 2.2), ((330.0, 52.0), (60.0, 16.0), 0.19, 4.1)].into_iter().enumerate() {
        let a = t * speed + phase;
        let p = (centre.0 + reach.0 * a.sin(), centre.1 + reach.1 * (a * 2.0).sin() - 6.0 * (t * 1.3 + phase).sin());
        let size = 22.0 * if i == 0 { 1.0 } else { 0.8 };
        let flap = (0.4 + 0.6 * (t * 3.0 + phase).sin().abs()) as f32;
        let turn = (a.cos() * 20.0 + if a.cos() < 0.0 { 180.0 } else { 0.0 }) as f32;
        sprite_pm(pm, base, &wings[i], 36, (p.0 as f32, p.1 as f32), (size * flap, size), turn, a.cos() < 0.0, 1.0);
    }
}

/// The butterfly sprite (`butterfly_rgba`) three ways: the garden's orange, turned rose
/// (`hueRotation(-48°)`) and pale (`saturation(0.15)`, `brightness(0.25)`); all orange without
/// the filters.
fn wing_set(filters: bool) -> [Pixmap; 3] {
    let plain = butterfly_rgba();
    let tint = |hue: f32, sat: f32, bright: f32| {
        let mut rgba = plain.clone();
        if filters {
            recolour(&mut rgba, hue, sat, bright);
        }
        pixmap_from(&rgba, 36, 36)
    };
    [pixmap_from(&plain, 36, 36), tint(-48.0, 1.0, 0.0), tint(0.0, 0.15, 0.25)]
}

/// The light, as in a summer anime still: warm beams falling from the sun, each breathing
/// very slowly.
fn draw_beams(pm: &mut Pixmap, base: Transform, t: f64) {
    for (k, (angle, spread, alpha)) in [(2.28f64, 0.07f64, 0.22f64), (2.47, 0.045, 0.17), (2.64, 0.08, 0.2), (2.84, 0.04, 0.14)].into_iter().enumerate() {
        let breathe = 0.65 + 0.35 * (t * 0.18 + k as f64 * 1.7).sin();
        let mut b = PathBuilder::new();
        b.move_to(SUN.0, SUN.1);
        b.line_to(SUN.0 + ((angle - spread).cos() * 900.0) as f32, SUN.1 + ((angle - spread).sin() * 900.0) as f32);
        b.line_to(SUN.0 + ((angle + spread).cos() * 900.0) as f32, SUN.1 + ((angle + spread).sin() * 900.0) as f32);
        b.close();
        pm.fill_path(&b.finish().unwrap(), &paint_f((1.0, 0.95, 0.80), (alpha * breathe) as f32), FillRule::Winding, base, None);
    }
}

/// Dust in the light: soft specks rising and drifting very slowly, twinkling as they turn.
fn draw_dust(pm: &mut Pixmap, base: Transform, t: f64) {
    let dh = H as f64 + 20.0;
    for i in 0..34i64 {
        let (n1, n2, n3) = (noise(i * 17 + 3), noise(i * 31 + 7), noise(i * 43 + 1));
        let speed = 2.5 + 3.0 * n1.abs();
        let y = dh - (i as f64 * 37.0 + t * speed) % dh - 10.0;
        let x = n2.abs() * W as f64 + (t * 0.25 + i as f64).sin() * 14.0;
        let r = 0.8 + 2.2 * n3.abs();
        let twinkle = 0.35 + 0.65 * (t * (0.4 + n1.abs()) + i as f64).sin().abs();
        pm.fill_path(&oval((x - r) as f32, (y - r) as f32, (r * 2.0) as f32, (r * 2.0) as f32), &paint_f((1.0, 0.97, 0.88), (0.75 * twinkle) as f32), FillRule::Winding, base, None);
    }
}

/// Right in front of the lens: big leaves in the corners, swaying slowly and a little behind
/// the breeze. `tf` already slides with the view.
fn draw_leaves(l: &mut Pixmap, tf: Transform, t: f64) {
    let leaves: [((f32, f32), f32, f32, f64, Rgb8); 7] = [
        ((8.0, 250.0), 196.0, 38.0, 0.22, (22, 70, 46)), ((44.0, 256.0), 158.0, 34.0, 0.52, (236, 110, 176)),
        ((-20.0, 244.0), 150.0, 32.0, -0.08, (40, 112, 70)), ((86.0, 262.0), 118.0, 28.0, 0.8, (170, 102, 124)),
        ((602.0, 254.0), 176.0, 36.0, -0.3, (22, 70, 46)), ((634.0, 244.0), 132.0, 30.0, -0.08, (240, 134, 176)),
        ((566.0, 262.0), 112.0, 26.0, -0.62, (40, 112, 70)),
    ];
    for (i, (root, length, width, lean, ink_c)) in leaves.into_iter().enumerate() {
        let w = (wind(root.0, t - 0.6) - 3.0).to_radians() * 0.35 + (t * 0.5 + i as f64).sin() * 0.03;
        let a = lean + w;
        let tip = (root.0 + a.sin() as f32 * length, root.1 - a.cos() as f32 * length);
        let mid = ((root.0 + tip.0) / 2.0, (root.1 + tip.1) / 2.0);
        let (nx, ny) = (a.cos() as f32 * width, a.sin() as f32 * width);
        let mut b = PathBuilder::new();
        b.move_to(root.0, root.1);
        b.quad_to(mid.0 - nx, mid.1 - ny, tip.0, tip.1);
        b.quad_to(mid.0 + nx, mid.1 + ny, root.0, root.1);
        b.close();
        l.fill_path(&b.finish().unwrap(), &paint(ink_c, 1.0), FillRule::Winding, tf, None);
    }
}

/// The meadow at moment `t`, in two prints: behind the iris, and in front of it.
fn meadow_prints(s: f32, t: f64, filters: bool) -> (Vec<u8>, Vec<u8>) {
    let plate = Plate { w: W, h: H, scale: s };
    let (pw, ph) = plate.px();
    let base = Transform::from_scale(s, s);
    let cam = cam(t);
    // Torn from a sheet: both prints cut to the deckle (made only when a print is).
    let cut = || {
        let mut m = Mask::new(pw, ph).unwrap();
        m.fill_path(&deckle(), FillRule::Winding, true, base);
        m
    };

    let back = (|| -> Vec<u8> {
        let mut pm = sky_plate(s);
        let sky_tf = base.pre_translate(cam * DRIFT_SKY, 0.0);
        draw_rings(&mut pm, sky_tf, t);
        // The garden's sun, turning very slowly.
        sprite(&mut pm, sky_tf, &crate::garden::sun_rgba(), 44, SUN, (46.0, 46.0), (t * 3.0) as f32, false, 1.0);
        let clouds = cumulus();
        layer(&mut pm, s, 0.9, filters, |l, tf| {
            let tf = tf.pre_translate(cam * DRIFT_SKY, 0.0);
            for (i, (cx, cbase, cw, puffs)) in clouds.iter().enumerate() {
                draw_cloud(l, tf, cloud_x(i, *cx, t), *cbase, *cw, puffs);
            }
        });
        // The far hill, a touch out of focus; the meadow.
        for (pts, ink_c, seed, line_a, line_w, drift, sigma) in lands() {
            let slide = cam * drift;
            let mut mask = plate.mask();
            mask.fill_path(&edge_shape(pts, true), FillRule::Winding, true, base.pre_translate(slide, 0.0));
            let printed_ink = pixmap_from(&riso::print(&plate, ink_c, seed, &mask, 1.0), pw, ph);
            layer(&mut pm, s, sigma, filters, |l, tf| {
                let pad = (l.width() - pw) as i32 / 2;
                l.draw_pixmap(pad, pad, printed_ink.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
                l.stroke_path(&edge_shape(pts, false), &paint(SCENE_INK, line_a), &Stroke { width: line_w, line_cap: LineCap::Round, ..Default::default() }, tf.pre_translate(slide, 0.0), None);
            });
        }
        draw_grass(&mut pm, base, &blades(), true, t);
        pm.apply_mask(&cut());
        straight(&pm)
    })();

    let front = (|| -> Vec<u8> {
        let mut pm = Pixmap::new(pw, ph).unwrap();
        draw_grass(&mut pm, base, &blades(), false, t);
        draw_petals(&mut pm, base, t);
        draw_butterflies(&mut pm, base, t, &wing_set(filters));
        draw_beams(&mut pm, base, t);
        draw_dust(&mut pm, base, t);
        layer(&mut pm, s, 6.0, filters, |l, tf| draw_leaves(l, tf.pre_translate(cam * DRIFT_NEAR * 1.6, 0.0), t));
        // `.grain(0.45)` over the whole frame. The close butterfly is off the left edge now.
        let grain = pixmap_from(&riso::grain_rgba(W, H, s, 0.45), pw, ph);
        pm.draw_pixmap(0, 0, grain.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        pm.apply_mask(&cut());
        straight(&pm)
    })();
    (back, front)
}

/// The sky plate (`ScreenPlate(ink: .sky, seed: 11)`), opaque, one dot a point.
fn sky_plate(s: f32) -> Pixmap {
    let (pw, ph) = Plate { w: W, h: H, scale: s }.px();
    let dots = riso::screen(W as usize, H as usize, SKY, 11, 1.0, 1.0);
    let mut rgba = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        for x in 0..pw {
            let c = dots[((y as f32 / s) as usize).min(H as usize - 1) * W as usize + ((x as f32 / s) as usize).min(W as usize - 1)];
            let o = ((y * pw + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[c.0, c.1, c.2, 255]);
        }
    }
    pixmap_from(&rgba, pw, ph)
}

/// The iris in its meadow, quietly alive (drawn still): torn from a sheet, no drawn frame. The
/// iris (`FlowerRig`, open and at rest) stands between the grass behind it and the grass in
/// front, swaying `sway` degrees about its base: all one picture, as GPUI can't turn it.
fn meadow(s: f32) -> gpui::Stateful<Div> {
    let plate = Plate { w: W, h: H, scale: s };
    let filters = filters();
    let img = riso::cached(format!("settings-about-meadow-{filters}"), &plate, || {
        let (back, front) = meadow_prints(s, T, filters);
        let k = IRIS_SIZE / 92.0;
        let (fw, fh) = (RIG_CANVAS.0 * k, RIG_CANVAS.1 * k);
        let sway = ((wind(IRIS_BASE.0, T - 0.4) - 3.0) / 5.0 * 1.6 + (T * 0.8).sin() * 0.6) as f32;
        let slide = cam(T) * (DRIFT_FAR + (DRIFT_NEAR - DRIFT_FAR) * 0.3);
        let iris_plate = Plate { w: fw, h: fh, scale: s };
        let (iw, ih) = iris_plate.px();
        let clock = crate::rig::RigClock { now: 100.0, arrived: Some(0.0), bloomed: Some(1.0), wilted: None, full: false, replaced: true };
        let iris = crate::rig::draw_rig_at(&clock, IRIS_SIZE, s, iw, ih, 0.0, 0.0);
        let mut sh = Sheet::new(plate, 0.0, 0.0, 0.0);
        sh.picture(&back, W, H, 0.0, 0.0);
        let at_ = Transform::from_translate(IRIS_BASE.0 - RIG_BASE.0 * k + slide, IRIS_BASE.1 - RIG_BASE.1 * k)
            .pre_concat(Transform::from_rotate_at(sway, RIG_BASE.0 * k, RIG_BASE.1 * k));
        sh.picture_at(&iris, iw, ih, fw, fh, at_, 1.0);
        sh.picture(&front, W, H, 0.0, 0.0);
        sh.rgba()
    });
    sized(W, H).child(at(printed(W, H, &img), 0.0, 0.0))
        .id("settings-meadow").role(Role::Image).aria_label("The iris in a meadow, swaying in a breeze.")
}

// MARK: - The meadow, live

/// Swift's TimelineView paces the meadow at 30 frames a second (`Breeze.frameRate`).
const FPS: f64 = 30.0;

/// Swift's clock: seconds since its reference date, round an hour (`truncatingRemainder(3600)`).
fn wall_clock() -> f64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    (unix - 978_307_200.0).rem_euclid(3600.0)
}

/// A layer printed once, slid by whole device pixels as the view drifts: its pixmap, and the
/// device pixel where its left/top edge sits when it hasn't slid.
struct Slide {
    pm: Pixmap,
    at: (i32, i32),
}

/// How far past the frame a sliding layer is printed, in points (the most any slides is 2.5).
const SLIDE_ROOM: f32 = 6.0;

/// Paints the live meadow, on its own thread. What never moves (the sky plate, the iris at
/// rest, the sprites, the grain, the deckle) is made once; so are the clouds, the hill and the
/// meadow, printed with room round them and slid a whole device pixel at a time as the view
/// drifts (as Swift's `.offset` slides a printed layer, dots and all); the grass, the light
/// and everything alive are drawn each frame.
struct Painter {
    s: f32,
    sky: Pixmap,
    blades: Vec<Blade>,
    sun: Pixmap,
    /// The butterflies, orange, rose and pale (`wing_set`), and the close one's rose.
    wings: [Pixmap; 3],
    /// The iris open and at rest, cropped to its ink, where the crop sits in its full frame,
    /// and that frame's size in pixels and points.
    iris: Pixmap,
    iris_crop: (f32, f32),
    iris_px: (f32, f32),
    iris_pt: (f32, f32),
    /// Each cloud (where it starts across), then the hill, then the meadow.
    clouds: Vec<(f32, Slide)>,
    lands: Vec<(f32, Slide)>,
    /// The paper's grain: how strongly each pixel is dotted, and the dot's ink.
    grain: Vec<u8>,
    grain_ink: Rgb8,
    cut: Mask,
}

impl Painter {
    fn new(s: f32) -> Painter {
        let plate = Plate { w: W, h: H, scale: s };
        let (pw, ph) = plate.px();
        let base = Transform::from_scale(s, s);
        let k = IRIS_SIZE / 92.0;
        let (fw, fh) = (RIG_CANVAS.0 * k, RIG_CANVAS.1 * k);
        let (iw, ih) = Plate { w: fw, h: fh, scale: s }.px();
        let clock = crate::rig::RigClock { now: 100.0, arrived: Some(0.0), bloomed: Some(1.0), wilted: None, full: false, replaced: true };
        let iris = pixmap_from(&crate::rig::draw_rig_at(&clock, IRIS_SIZE, s, iw, ih, 0.0, 0.0), iw, ih);
        let (iris, crop) = crop_to_ink(&iris);
        let mut cut = Mask::new(pw, ph).unwrap();
        cut.fill_path(&deckle(), FillRule::Winding, true, base);
        // Each cloud on its own, at its starting place: x from its left, y up from its base.
        // Soft edges, painted (`.blur(radius: 0.9)`): room round each for the blur to spread into.
        let soft = 3.0;
        let clouds = cumulus().iter().map(|(cx, cbase, cw, puffs)| {
            let (left, top) = (cx - cw * 0.4 - soft, cbase - cw * 0.9 - soft);
            let (w, h) = (cw * 1.8 + 2.0 * soft, cw * 0.9 + 2.0 + 2.0 * soft);
            let (lw, lh) = ((w * s).ceil() as u32, (h * s).ceil() as u32);
            let mut pm = Pixmap::new(lw, lh).unwrap();
            let lx = (left * s).floor();
            let ly = (top * s).floor();
            draw_cloud(&mut pm, Transform::from_translate(-lx, -ly).pre_scale(s, s), *cx, *cbase, *cw, puffs);
            blur(&mut pm, 0.9 * s);
            (*cx, Slide { pm, at: (lx as i32, ly as i32) })
        }).collect();
        // The hill and the meadow, printed with room either side to slide into.
        let room = Plate { w: W + 2.0 * SLIDE_ROOM, h: H, scale: s };
        let (rw, rh) = room.px();
        let shift = (SLIDE_ROOM * s).round();
        // The far hill a touch out of focus (`.blur(radius: 1.2)`), the meadow sharp.
        let lands = lands().into_iter().map(|(pts, ink_c, seed, line_a, line_w, drift, sigma)| {
            let tf = Transform::from_translate(shift, 0.0).pre_scale(s, s);
            let mut mask = Mask::new(rw, rh).unwrap();
            mask.fill_path(&edge_shape(pts, true), FillRule::Winding, true, tf);
            let mut pm = pixmap_from(&riso::print(&room, ink_c, seed, &mask, 1.0), rw, rh);
            pm.stroke_path(&edge_shape(pts, false), &paint(SCENE_INK, line_a), &Stroke { width: line_w, line_cap: LineCap::Round, ..Default::default() }, tf, None);
            if sigma > 0.0 {
                blur(&mut pm, sigma * s);
            }
            (drift, Slide { pm, at: (-shift as i32, 0) })
        }).collect();
        let grain = riso::grain_rgba(W, H, s, 0.45).chunks_exact(4).map(|p| p[3]).collect();
        Painter {
            s,
            sky: sky_plate(s),
            blades: blades(),
            sun: pixmap_from(&crate::garden::sun_rgba(), 44, 44),
            wings: wing_set(true),
            iris,
            iris_crop: crop,
            iris_px: (iw as f32, ih as f32),
            iris_pt: (fw, fh),
            clouds,
            lands,
            grain,
            grain_ink: riso::multiply((26, 20, 16), ink::PAPER),
            cut,
        }
    }

    /// The meadow at `t`, torn to its deckle, as a picture GPUI can show.
    fn frame(&mut self, t: f64) -> Pixels {
        let s = self.s;
        let base = Transform::from_scale(s, s);
        let cam = cam(t);
        let px = |x: f32| (x * s).round() as i32;
        let mut pm = self.sky.clone();
        let sky_tf = base.pre_translate(cam * DRIFT_SKY, 0.0);
        draw_rings(&mut pm, sky_tf, t);
        sprite_pm(&mut pm, sky_tf, &self.sun, 44, SUN, (46.0, 46.0), (t * 3.0) as f32, false, 1.0);
        for (i, (cx, c)) in self.clouds.iter().enumerate() {
            over_at(&mut pm, &c.pm, c.at.0 + px(cloud_x(i, *cx, t) - cx + cam * DRIFT_SKY), c.at.1);
        }
        for (drift, l) in &self.lands {
            over_at(&mut pm, &l.pm, l.at.0 + px(cam * drift), l.at.1);
        }
        draw_grass(&mut pm, base, &self.blades, true, t);
        // The iris: the wind leans it, a beat behind the grass at its foot.
        let k = IRIS_SIZE / 92.0;
        let sway = ((wind(IRIS_BASE.0, t - 0.4) - 3.0) / 5.0 * 1.6 + (t * 0.8).sin() * 0.6) as f32;
        let slide = cam * (DRIFT_FAR + (DRIFT_NEAR - DRIFT_FAR) * 0.3);
        let at_ = Transform::from_translate(IRIS_BASE.0 - RIG_BASE.0 * k + slide, IRIS_BASE.1 - RIG_BASE.1 * k)
            .pre_concat(Transform::from_rotate_at(sway, RIG_BASE.0 * k, RIG_BASE.1 * k));
        let (fw, fh) = self.iris_pt;
        let iris_tf = base.pre_concat(at_).pre_scale(fw / self.iris_px.0, fh / self.iris_px.1).pre_translate(self.iris_crop.0, self.iris_crop.1);
        pm.draw_pixmap(0, 0, self.iris.as_ref(), &PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() }, iris_tf, None);
        draw_grass(&mut pm, base, &self.blades, false, t);
        draw_petals(&mut pm, base, t);
        draw_butterflies(&mut pm, base, t, &self.wings);
        draw_beams(&mut pm, base, t);
        draw_dust(&mut pm, base, t);
        // Right in front of the lens, out of focus (`.blur(radius: 6)`): drawn coarse, blurred,
        // smoothed back up.
        let near = cam * DRIFT_NEAR * 1.6;
        let q = (s / 4.0).max(0.5);
        soft(&mut pm, s, q, 6.0, Rect::from_xywh(-24.0, -24.0, W + 48.0, H + 48.0).unwrap(), |l, tf| draw_leaves(l, tf.pre_translate(near, 0.0), t));
        // One butterfly close to the lens, drifting slowly across (off the left edge at the
        // still moment, so the still meadow has none).
        let span = W as f64 + 120.0;
        let x = (t * 4.0) % span - 60.0;
        let y = 150.0 + (t * 0.7).sin() * 22.0;
        let flap = (0.5 + 0.5 * (t * 2.2).sin().abs()) as f32;
        let (bx, by) = (x as f32 + near, y as f32);
        if bx > -40.0 && bx < W + 40.0 {
            // Rose and out of focus (`.hueRotation(-48°)`, `.blur(radius: 3)`).
            let turn = ((t * 0.9).sin() * 14.0) as f32;
            soft(&mut pm, s, (s / 2.0).max(0.5), 3.0, Rect::from_xywh(bx - 34.0, by - 34.0, 68.0, 68.0).unwrap(), |l, tf| {
                sprite_pm(l, tf, &self.wings[1], 36, (bx, by), (40.0 * flap, 40.0), turn, false, 1.0);
            });
        }
        finish(&pm, &self.grain, self.grain_ink, &self.cut)
    }
}

/// A picture cropped to where it has ink, and where the crop sat (pixels).
fn crop_to_ink(pm: &Pixmap) -> (Pixmap, (f32, f32)) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, p) in pm.data().chunks_exact(4).enumerate() {
        if p[3] > 0 {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return (pm.clone(), (0.0, 0.0));
    }
    let mut out = Pixmap::new((x1 - x0) as u32, (y1 - y0) as u32).unwrap();
    out.draw_pixmap(-(x0 as i32), -(y0 as i32), pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    (out, (x0 as f32, y0 as f32))
}

/// `src` laid over `dst` with its top left at device pixel (`x`, `y`): premultiplied
/// source-over, a whole pixel at a time (much quicker than a transformed draw).
fn over_at(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let (x0, x1) = (x.max(0), (x + sw).min(dw));
    let (y0, y1) = (y.max(0), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let s = src.data();
    let d = dst.data_mut();
    for row in y0..y1 {
        let so = (((row - y) * sw + (x0 - x)) * 4) as usize;
        let doff = ((row * dw + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        for (o, p) in d[doff..doff + n].chunks_exact_mut(4).zip(s[so..so + n].chunks_exact(4)) {
            match p[3] {
                0 => {}
                255 => o.copy_from_slice(p),
                a => {
                    let k = 255 - a as u32;
                    for c in 0..4 {
                        o[c] = (p[c] as u32 + (o[c] as u32 * k + 127) / 255).min(255) as u8;
                    }
                }
            }
        }
    }
}

/// The frame finished in one pass: the paper's grain dotted over it (`.grain(0.45)`), cut to
/// the deckle, as a GPUI picture (BGRA, straight alpha).
fn finish(pm: &Pixmap, grain: &[u8], dot: Rgb8, cut: &Mask) -> Pixels {
    let mut out = vec![0u8; pm.data().len()];
    let dot = [dot.0 as u32, dot.1 as u32, dot.2 as u32];
    for (((o, p), &g), &m) in out.chunks_exact_mut(4).zip(pm.data().chunks_exact(4)).zip(grain).zip(cut.data()) {
        if m == 0 {
            continue;
        }
        // Premultiplied: the grain's dot over the frame.
        let (g, k) = (g as u32, 255 - g as u32);
        let a = g + (p[3] as u32 * k + 127) / 255;
        if a == 0 {
            continue;
        }
        let c = |i: usize| dot[i] * g / 255 + (p[i] as u32 * k + 127) / 255;
        let un = |v: u32| ((v * 255 + a / 2) / a).min(255) as u8;
        let alpha = ((a * m as u32 + 127) / 255) as u8;
        o.copy_from_slice(&[un(c(2)), un(c(1)), un(c(0)), alpha]);
    }
    let buf = image::RgbaImage::from_raw(pm.width(), pm.height(), out).expect("w×h×4 bytes");
    std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buf)]))
}

/// The live meadow's pictures: a painter thread draws each frame a little ahead; the main
/// thread only shows the newest, and lets go of the one it replaced (`let_go`) so GPUI's
/// texture atlas keeps one meadow, not every frame.
struct Feed {
    scale: f32,
    jobs: std::sync::mpsc::Sender<f64>,
    done: std::sync::Arc<std::sync::Mutex<Option<(f64, Pixels)>>>,
    shown: Option<(f64, Pixels)>,
    /// The last moment asked for.
    asked: f64,
}

impl Feed {
    fn new(scale: f32) -> Feed {
        let (jobs, rx) = std::sync::mpsc::channel::<f64>();
        let done = std::sync::Arc::new(std::sync::Mutex::new(None));
        let out = done.clone();
        let trace = std::env::var("IRIS_TRACE").is_ok();
        std::thread::Builder::new().name("iris-meadow".into()).spawn(move || {
            let mut painter = Painter::new(scale);
            while let Ok(mut t) = rx.recv() {
                // Behind: skip to the newest moment asked for.
                while let Ok(n) = rx.try_recv() {
                    t = n;
                }
                let began = std::time::Instant::now();
                let img = painter.frame(t);
                if trace {
                    eprintln!("meadow draw_ms={:.2} t={t:.3}", began.elapsed().as_secs_f64() * 1000.0);
                }
                *out.lock().unwrap() = Some((t, img));
            }
        }).expect("the meadow's thread");
        Feed { scale, jobs, done, shown: None, asked: -1.0 }
    }

    fn ask(&mut self, t: f64) {
        self.asked = t;
        let _ = self.jobs.send(t);
    }

    /// Takes the newest finished picture, if there is one.
    fn collect(&mut self) {
        if let Some(d) = self.done.lock().unwrap().take()
            && let Some(old) = self.shown.replace(d)
        {
            RETIRED.with(|r| r.borrow_mut().push(old.1));
        }
    }
}

thread_local! {
    static FEED: std::cell::RefCell<Option<Feed>> = const { std::cell::RefCell::new(None) };
    /// Meadow pictures replaced, for GPUI to let go of (`let_go`).
    static RETIRED: std::cell::RefCell<Vec<Pixels>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Stopped by the pause button, at this moment of Swift's clock (the phrases stop too).
    static PAUSED: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Whether the meadow and the phrases are stopped (the pause button).
fn paused() -> bool {
    PAUSED.get().is_some()
}

/// Settings' window, before it draws: lets go of the meadow pictures the last frames replaced.
pub fn let_go(window: &mut gpui::Window) {
    for img in RETIRED.with(|r| std::mem::take(&mut *r.borrow_mut())) {
        let _ = window.drop_image(img);
    }
}

/// A page turned: About starts again playing, its phrases at their first catch (Swift's
/// `@State` goes with the page). The painter stops until About is back.
pub fn reset() {
    PAUSED.set(None);
    FEED.with(|f| {
        if let Some(old) = f.borrow_mut().take()
            && let Some((_, img)) = old.shown
        {
            RETIRED.with(|r| r.borrow_mut().push(img));
        }
    });
    TOKENS.with(|t| t.borrow_mut().clear());
}

/// Settings window closed: stops the meadow thread and immediately drops any retained frames
/// (the closing window's GPU atlas is torn down by GPUI, so nothing needs to wait for `let_go`).
pub fn shutdown() {
    reset();
    RETIRED.with(|r| r.borrow_mut().clear());
}

/// The live meadow's picture for now, asking for the next frame. Waits for the very first
/// (about 20 ms, once) so About never opens on an empty frame; none if it hasn't come in
/// 400 ms (a busy Mac: the still meadow shows meanwhile).
fn live_meadow(s: f32) -> Option<Pixels> {
    FEED.with(|f| {
        let mut f = f.borrow_mut();
        if f.as_ref().is_none_or(|f| f.scale != s) {
            if let Some(old) = f.take()
                && let Some((_, img)) = old.shown
            {
                RETIRED.with(|r| r.borrow_mut().push(img));
            }
            *f = Some(Feed::new(s));
        }
        let feed = f.as_mut().unwrap();
        feed.collect();
        match held() {
            // Held: the frame it stopped on (or that moment, if none is shown yet).
            Some(at) => {
                if feed.shown.is_none() && feed.asked < 0.0 {
                    feed.ask(at);
                }
            }
            None => {
                let next = ((wall_clock() * FPS).floor() + 1.0) / FPS;
                if (next - feed.asked).abs() > 1e-6 {
                    feed.ask(next);
                }
            }
        }
        let began = std::time::Instant::now();
        while feed.shown.is_none() && began.elapsed() < std::time::Duration::from_millis(400) {
            std::thread::sleep(std::time::Duration::from_millis(2));
            feed.collect();
        }
        feed.shown.as_ref().map(|s| s.1.clone())
    })
}

/// The live dispatcher, when this is a live window that moves (none in a snapshot, none under
/// Reduce Motion: the still meadow then).
fn moving_live() -> Option<crate::interact::Live> {
    crate::interact::get().filter(|l| l.view.is_some() && !l.reduce_motion && !crate::app::settings().light_mode_active())
}

/// The moment the meadow holds at: where the pause button stopped it, or, sealed on the test
/// display, the still moment (Swift's `OnDisplay` holds it there, so a capture is of one frame,
/// the pause button still showing).
fn held() -> Option<f64> {
    PAUSED.get().or(crate::driven::on().then_some(T))
}

/// The meadow, live: the newest frame, and the pause button at its bottom right.
fn meadow_live(live: &crate::interact::Live, s: f32) -> Div {
    let img = live_meadow(s);
    if held().is_none() || img.is_none() {
        live.anim.borrow_mut().moving = true;
    }
    let picture = match img {
        Some(img) => sized(W, H).child(at(printed(W, H, &img), 0.0, 0.0))
            .id("settings-meadow").role(Role::Image).aria_label("The iris in a meadow, swaying in a breeze."),
        None => meadow(s),
    };
    sized(W, H).child(picture).child(at(pause_button(live, s), W - 10.0 - 26.0, H - 10.0 - 26.0))
}

/// Stops the meadow, or starts it again: a small paper disc with a pen pause or play mark
/// (WCAG 2.2.2: motion that runs on needs a way to stop it). The phrases stop with it.
fn pause_button(live: &crate::interact::Live, s: f32) -> gpui::Stateful<Div> {
    let held = paused();
    let label = if held { "Play the animation" } else { "Pause the animation" };
    let act = crate::interact::act(|_, _| {
        PAUSED.set(if paused() { None } else { Some(wall_clock()) });
        // The phrases wait their turn again from now.
        TOKENS.with(|t| t.borrow_mut().iter_mut().for_each(|x| x.wait_from = std::time::Instant::now()));
    });
    crate::interact::pressable(live, "about-pause", 26.0, 26.0, 0.96, Some(act), |_| {
        let plate = Plate { w: 26.0, h: 26.0, scale: s };
        let img = riso::cached(format!("about-pause-{held}"), &plate, || {
            let (pw, ph) = plate.px();
            let mut pm = Pixmap::new(pw, ph).unwrap();
            let tf = plate.transform();
            pm.fill_path(&oval(0.0, 0.0, 26.0, 26.0), &paint(ink::PAPER, 0.92), FillRule::Winding, tf, None);
            if let Some(ring) = pen::hand_ellipse(21, 0.4, 26.0, 26.0).skia() {
                pm.stroke_path(&ring, &paint(hue::I12, 0.55), &Stroke { width: 1.1, line_cap: LineCap::Round, ..Default::default() }, tf, None);
            }
            if held {
                let mut b = PathBuilder::new();
                b.move_to(0.0, 0.0);
                b.line_to(9.0, 5.5);
                b.line_to(0.0, 11.0);
                b.close();
                pm.fill_path(&b.finish().unwrap(), &paint(hue::I12, 1.0), FillRule::Winding, tf.pre_translate((26.0 - 9.0) / 2.0 + 1.0, (26.0 - 11.0) / 2.0), None);
            } else {
                for x in [13.0 - 1.5 - 2.6, 13.0 + 1.5] {
                    pm.fill_path(&icons::rounded(x, 8.0, 2.6, 10.0, 1.3), &paint(hue::I12, 1.0), FillRule::Winding, tf, None);
                }
            }
            straight(&pm)
        });
        printed(26.0, 26.0, &img)
    })
    .role(Role::Button).aria_label(label)
}

#[cfg(test)]
mod timing {
    /// The still meadow (snapshots, Reduce Motion) and the live painter's frames, at 2×.
    /// `cargo test --release meadow_frame_ms -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn meadow_frame_ms() {
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            let _ = super::meadow_prints(2.0, super::T, true);
            println!("still meadow_prints {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        let t0 = std::time::Instant::now();
        let mut p = super::Painter::new(2.0);
        println!("live painter made in {:.1} ms (once, on its thread)", t0.elapsed().as_secs_f64() * 1000.0);
        let mut times = vec![];
        for i in 0..90 {
            let t = 100.0 + i as f64 / super::FPS;
            let t0 = std::time::Instant::now();
            let _ = p.frame(t);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let first = times.remove(0);
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("live frame: first {first:.1} ms, then median {:.1} ms, p90 {:.1} ms, max {:.1} ms over 3 s",
                 times[times.len() / 2], times[times.len() * 9 / 10], times[times.len() - 1]);
        // What the leaves' blur costs, by part.
        let mut pm = tiny_skia::Pixmap::new(1224, 420).unwrap();
        let q = 0.5;
        let region = tiny_skia::Rect::from_xywh(-24.0, -24.0, super::W + 48.0, super::H + 48.0).unwrap();
        let (w, h) = ((region.width() * q).ceil() as u32, (region.height() * q).ceil() as u32);
        let t0 = std::time::Instant::now();
        let mut l = tiny_skia::Pixmap::new(w, h).unwrap();
        super::draw_leaves(&mut l, tiny_skia::Transform::from_scale(q, q).pre_translate(24.0, 24.0), 100.0);
        let t1 = t0.elapsed().as_secs_f64() * 1000.0;
        super::blur(&mut l, 6.0 * q);
        let t2 = t0.elapsed().as_secs_f64() * 1000.0;
        super::lay_on(&mut pm, &l, -48, -48, 4);
        let t3 = t0.elapsed().as_secs_f64() * 1000.0;
        println!("leaves: draw {t1:.2} ms, blur {:.2} ms, laid on {:.2} ms", t2 - t1, t3 - t2);
        // `MEADOW_PNG=<path>`: the live frame at the still moment, to set beside Swift's (which
        // holds still at that moment on the test display).
        if let Ok(path) = std::env::var("MEADOW_PNG") {
            let img = p.frame(super::T);
            let size = img.size(0);
            let mut rgba = img.as_bytes(0).unwrap().to_vec();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, rgba).unwrap().save(path).unwrap();
        }
    }
}
