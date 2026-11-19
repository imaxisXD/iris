//! First-time setup (`OnboardingView`): a zine page per step. The sky plate across the top with
//! a ghost HELLO (AGAIN for someone returning), the wordmark and the garden standing on its
//! edge; then the running head, the step's page (a stair headline with its drawing, two plain
//! lines and one button), the three steps listed on pen rules at the right, and the big folio.
//!
//! The Rune copy's setup (`rune/src/setup.rs` on `rune-ui`) in GPUI. The rules are IrisCore's
//! `SetupProgress` and `SetupWords`, ported as they are; the twelve made-up states are
//! `SetupStates`. Drawn whole (unscrolled), 720 wide, as `Iris --setup-states` draws them, which
//! includes what SwiftUI does with a page wider than the window: a step whose fixed parts need
//! more than the column's 391 points widens the page past 720, centred (so it runs off both
//! sides), and the page's height is the one it had at 720, the content centred in it.
//!
//! What GPUI can't do, done another way:
//! - The headline and the folio are Martian at width 75 (and ✻ from Zapf Dingbats), a cut GPUI
//!   has no font for: printed from the variable font's outlines (`settings_kit::martian_line`).
//! - What Rune turns (the step's drawing, the hand note's words, the chooser's chips) is printed
//!   whole, turned (`parts::turned`).
//! - What Rune scales (the garden by 1.16, the "recommended" mark by 0.85) is printed bigger
//!   (`garden::garden`'s zoom) or laid out smaller (`with_zoom`).

use gpui::{AnyElement, Div, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement, Styled, div};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, PathBuilder, Transform};

use crate::garden::{self, Stage};
use crate::icons::{self, Draw, rounded};
use crate::letters::{self, LONDRINA};
use crate::parts::*;
use crate::pen::{self, Pen};
use crate::raster::Sheet;
use crate::riso::{self, Img, Plate, Rgb8, col, hue, ink, over, str_seed};
use crate::settings_kit::{martian_line, push_out};

pub const WIDTH: f32 = 720.0;
const PLATE_H: f32 = 190.0;
const SKY: Rgb8 = (120, 162, 220);
/// The step list's column, and what the page keeps beside the step's own column.
const LIST_W: f32 = 196.0;
const BESIDE: f32 = 40.0 * 2.0 + 26.0 * 2.0 + 1.0 + LIST_W;
const I3: Rgb8 = hue::I3;
/// SwiftUI sets the page a point lower than its padding says (measured).
const PAGE_DROP: f32 = 1.0;

// MARK: - The rules (IrisCore `SetupProgress`, `SetupWords`, `ModelFailure`, `ProblemNote`)

/// Why getting the model failed (`ModelFailure`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Failure {
    NoRoom { needed: &'static str, free: &'static str },
    Offline,
    NotDownloaded,
    Other,
}

impl Failure {
    /// What to do, after a lead that already says what happened.
    pub fn next_step(self) -> String {
        match self {
            Failure::NoRoom { needed, free } => format!("The model needs {needed} free, and this Mac has {free}. Free up some space."),
            Failure::Offline => "The model downloads once and needs the internet for that. It picks up where it stopped.".into(),
            Failure::NotDownloaded => "Open the menu bar flower and finish setup to download it.".into(),
            Failure::Other => "Something got in the way. Try again in a moment; it picks up where it stopped.".into(),
        }
    }
    /// `ProblemNote(failure).lead`: the short brush lead.
    pub fn lead(self) -> &'static str {
        match self {
            Failure::NotDownloaded => "i need my model first",
            Failure::NoRoom { .. } => "no room for my model",
            Failure::Offline => "no internet",
            Failure::Other => "couldn't get the model",
        }
    }
}

/// What the chosen model is doing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Activity {
    None,
    Downloading(f64),
    Loading,
    Ready,
    Failed(Failure),
}

/// What setup reads from the Mac.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub trusted: bool,
    pub model_on_disk: bool,
    pub activity: Activity,
    pub tried: bool,
    pub finished_before: bool,
    pub back_from_settings_untrusted: bool,
}

impl Facts {
    const fn new(trusted: bool, model_on_disk: bool) -> Facts {
        Facts { trusted, model_on_disk, activity: Activity::None, tried: false, finished_before: false, back_from_settings_untrusted: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Step {
    Allow = 1,
    Model = 2,
    TryIt = 3,
}

pub const STEPS: [Step; 3] = [Step::Allow, Step::Model, Step::TryIt];

impl Step {
    pub fn title(self) -> &'static str {
        match self {
            Step::Allow => "Allow",
            Step::Model => "Get the model",
            Step::TryIt => "Try it",
        }
    }
    /// The word beside a stuck step.
    pub fn stuck_word(self) -> &'static str {
        if self == Step::Allow { "not yet" } else { "stuck" }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    Done,
    Now,
    Stuck,
    Later,
}

/// How far the garden has grown (`GardenStage`).
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum GardenStage {
    Bud,
    Allowed,
    Watering,
    Warming,
    Ready,
    Tried,
}

/// `SetupProgress`: where setup stands, from the facts.
#[derive(Clone, Copy, Debug)]
pub struct Progress(pub Facts);

impl Progress {
    pub fn current(&self) -> Option<Step> {
        let f = &self.0;
        if !f.trusted {
            return Some(Step::Allow);
        }
        if !f.model_on_disk {
            return Some(Step::Model);
        }
        if !(f.tried || f.finished_before) {
            return Some(Step::TryIt);
        }
        None
    }
    pub fn is_complete(&self) -> bool {
        self.current().is_none()
    }
    pub fn is_returning(&self) -> bool {
        self.0.finished_before && !self.is_complete()
    }
    pub fn satisfied(&self, step: Step) -> bool {
        let f = &self.0;
        match step {
            Step::Allow => f.trusted,
            Step::Model => f.model_on_disk,
            Step::TryIt => f.tried || f.finished_before,
        }
    }
    pub fn status(&self, step: Step) -> Status {
        if self.satisfied(step) && Some(step) != self.current() {
            return Status::Done;
        }
        let Some(current) = self.current() else { return Status::Done };
        if step > current {
            return Status::Later;
        }
        match step {
            Step::Allow => if self.0.back_from_settings_untrusted { Status::Stuck } else { Status::Now },
            Step::Model => if matches!(self.0.activity, Activity::Failed(_)) { Status::Stuck } else { Status::Now },
            Step::TryIt => Status::Now,
        }
    }
    /// A way to leave unfinished only on a stuck step.
    pub fn offers_set_up_later(&self) -> bool {
        self.current().is_some_and(|c| self.status(c) == Status::Stuck)
    }
    /// The garden follows setup, never ahead of it.
    pub fn garden(&self) -> GardenStage {
        let f = &self.0;
        if f.tried || (self.is_complete() && f.finished_before) {
            return GardenStage::Tried;
        }
        if !f.trusted {
            return GardenStage::Bud;
        }
        match f.activity {
            Activity::Downloading(_) => GardenStage::Watering,
            Activity::Loading => GardenStage::Warming,
            Activity::Ready => GardenStage::Ready,
            Activity::None | Activity::Failed(_) => if f.model_on_disk { GardenStage::Ready } else { GardenStage::Allowed },
        }
    }
}

// MARK: - The made-up states (`SetupStates`)

pub struct SetupState {
    pub id: &'static str,
    pub facts: Facts,
    pub shortcut_taken: bool,
    /// The practice box's text, when Iris has replaced it.
    pub practice: Option<String>,
    /// The live window's own news (nil in the made-up states).
    pub live: Option<Live>,
}

/// What the live window reads from the Mac and the settings, besides the facts.
#[derive(Clone)]
pub struct Live {
    /// Which of `MODELS` is chosen, and which suits this Mac.
    pub chosen: usize,
    pub recommended: usize,
    pub on_disk: [bool; 2],
    /// The shortcuts as keys: (symbol, word).
    pub open_keys: Vec<(Option<String>, String)>,
    pub quick_keys: Option<Vec<(Option<String>, String)>>,
    pub launch_at_login: bool,
    /// The practice box's field, to type in and for Iris to fix.
    pub practice: Option<crate::text_field::Field>,
}

/// What a button in the live window asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    OpenSystemSettings,
    Relaunch,
    Download,
    StopDownload,
    TryAgain,
    OpenStorage,
    /// Use this model (an index into `MODELS`).
    Use(usize),
    ChangeShortcut,
    Done,
    Later,
    LaunchAtLogin,
}

type OnAct = std::rc::Rc<dyn Fn(Act, &mut gpui::Window, &mut gpui::App)>;

thread_local! {
    static ON_ACT: std::cell::RefCell<Option<OnAct>> = const { std::cell::RefCell::new(None) };
}

/// Where the live window hears its buttons (none for snapshots: they're only looked at).
pub fn set_on_act(f: Option<OnAct>) {
    ON_ACT.with(|h| *h.borrow_mut() = f);
}

/// `e`, pressing which asks for `a`.
fn clickable(e: impl IntoElement, id: &str, a: Act) -> AnyElement {
    div().id(gpui::SharedString::from(format!("act-{id}"))).child(e)
        .on_click(move |_, window, cx| {
            if let Some(f) = ON_ACT.with(|h| h.borrow().clone()) {
                f(a, window, cx);
            }
        })
        .into_any_element()
}

/// The keys as `key_line` takes them.
fn key_refs(v: &[(Option<String>, String)]) -> Vec<(Option<&str>, &str)> {
    v.iter().map(|(s, w)| (s.as_deref(), w.as_str())).collect()
}

fn open_keys(state: &SetupState) -> Vec<(Option<&str>, &str)> {
    match &state.live {
        Some(l) => key_refs(&l.open_keys),
        // A preview shows the keys the settings have, as Swift's does; a snapshot keeps ⌃⌥G.
        None => crate::slip::told_keys().map_or(OPEN_KEYS.to_vec(), |(open, _)| open.into_iter().map(word_of).collect()),
    }
}

fn quick_keys(state: &SetupState) -> Option<Vec<(Option<&str>, &str)>> {
    match &state.live {
        Some(l) => l.quick_keys.as_deref().map(key_refs),
        None => match crate::slip::told_keys() {
            Some((_, quick)) => quick.map(|q| q.into_iter().map(word_of).collect()),
            None => Some(QUICK_KEYS.to_vec()),
        },
    }
}

/// A keycap as setup labels it: (symbol, word) for a modifier, (none, key) for the key.
fn word_of(cap: &'static str) -> (Option<&'static str>, &'static str) {
    match cap {
        "⌃" => (Some(cap), "control"),
        "⌥" => (Some(cap), "option"),
        "⇧" => (Some(cap), "shift"),
        "⌘" => (Some(cap), "command"),
        _ => (None, cap),
    }
}

fn chosen(state: &SetupState) -> usize {
    state.live.as_ref().map_or(CHOSEN, |l| l.chosen)
}

const FIXED: &str = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge";
/// `OnboardingModel.practiceSample`.
const PRACTICE: &str = "hey, i have checked the PR and its look fine but one thing need to change before merge";

pub fn states() -> Vec<SetupState> {
    let f = Facts::new;
    let s = |id, facts| SetupState { id, facts, shortcut_taken: false, practice: None, live: None };
    vec![
        s("1-allow", f(false, false)),
        s("1-allow-stuck", Facts { back_from_settings_untrusted: true, ..f(false, false) }),
        s("1-allow-returning", Facts { finished_before: true, ..f(false, true) }),
        s("2-model", f(true, false)),
        s("2-downloading", Facts { activity: Activity::Downloading(0.37), ..f(true, false) }),
        s("2-loading", Facts { activity: Activity::Loading, ..f(true, false) }),
        s("2-no-room", Facts { activity: Activity::Failed(Failure::NoRoom { needed: "6.2 GB", free: "3.1 GB" }), ..f(true, false) }),
        s("2-offline", Facts { activity: Activity::Failed(Failure::Offline), ..f(true, false) }),
        s("2-returning", Facts { finished_before: true, ..f(true, false) }),
        s("3-try", Facts { activity: Activity::Ready, ..f(true, true) }),
        SetupState { id: "3-try-shortcut-taken", facts: Facts { activity: Activity::Ready, ..f(true, true) }, shortcut_taken: true, practice: None, live: None },
        SetupState { id: "done", facts: Facts { activity: Activity::Ready, tried: true, ..f(true, true) }, shortcut_taken: false, practice: Some(FIXED.into()), live: None },
    ]
}

/// The headless scene for the state named `id` (`setup-<id>`), as tall as `--size` asks (its
/// own height when it doesn't say).
pub fn scene(id: &str, scale: f32) -> Option<Div> {
    let state = states().into_iter().find(|s| s.id == id)?;
    Some(window(&state, crate::settings::asked_height(), scale))
}

// MARK: - The settings setup reads (as the snapshots find them)

/// Iris's two models (`ModelChoice.offered`), Best quality then Lighter, with today's sizes.
struct Model {
    id: &'static str,
    name: &'static str,
    /// `modelBytes`, `downloadBytes`.
    model_bytes: f64,
    download_bytes: f64,
}

const MODELS: [Model; 2] = [
    Model { id: "best", name: "Best quality", model_bytes: 4.58e9, download_bytes: 4.78e9 },
    Model { id: "lighter", name: "Lighter", model_bytes: 3.77e9, download_bytes: 3.97e9 },
];
const CHOSEN: usize = 0;

/// Where the model named `id` is in `MODELS`.
pub fn model_index(id: &str) -> Option<usize> {
    MODELS.iter().position(|m| m.id == id)
}

/// The id of `MODELS[i]`.
pub fn model_id(i: usize) -> &'static str {
    MODELS[i.min(MODELS.len() - 1)].id
}
const RECOMMENDED: usize = 0;

impl Model {
    fn figure(&self) -> String {
        format!("{:.1}", self.download_bytes / 1e9)
    }
    fn download_size(&self) -> String {
        format!("{} GB", self.figure())
    }
    /// "1.9 of 5.4 GB".
    fn downloaded(&self, f: f64) -> String {
        format!("{:.1} of {:.1} GB", self.model_bytes * f / 1e9, self.download_bytes / 1e9)
    }
}

/// The shortcuts as keys you press: (symbol, word).
const OPEN_KEYS: [(Option<&str>, &str); 3] = [(Some("⌃"), "control"), (Some("⌥"), "option"), (None, "G")];
const QUICK_KEYS: [(Option<&str>, &str); 4] = [(Some("⌃"), "control"), (Some("⌥"), "option"), (Some("⇧"), "shift"), (None, "G")];
const RETURN_KEY: [(Option<&str>, &str); 1] = [(Some("↩"), "return")];

// MARK: - Type

fn micro() -> Ts {
    martian(9.5, 450)
}

/// `DS.Font.display(size)`: Martian at width 75, a cut GPUI has no font for, so printed from
/// the variable font's outlines, in a box the style's line tall. Returns it and its width.
fn display(s: &str, size: f32, scale: f32) -> (Div, f32) {
    martian_line(s, size, 400.0, 75.0, 0.0, size * MONO_LINE, hue::I12, scale)
}

/// A brush line's box on the pixel grid, as SwiftUI lays a line of Caveat out.
fn brush_line_h(size: f32, scale: f32) -> f32 {
    (size * BRUSH_LINE * scale - 0.01).ceil() / scale
}

fn brush_px(size: f32, scale: f32) -> Ts {
    brush(size).line(brush_line_h(size, scale))
}

/// Where a brush line's baseline sits in its box.
fn brush_base(size: f32, scale: f32) -> f32 {
    (brush_line_h(size, scale) - size * BRUSH_LINE) / 2.0 + size * BRUSH_ASCENT
}

/// Martian's baseline in a line of `size`: its ascent is the em.
fn mono_base(size: f32) -> f32 {
    size
}

/// Mono words broken `w` wide at spaces, as Core Text breaks them. `company`: when the last
/// line would be one word alone, the word before it comes down to keep it company (Core Text
/// does this for some paragraphs and not others; each is checked against the picture).
fn wrap(s: &str, advance: f32, w: f32, company: bool) -> Vec<String> {
    let fits = |n: usize| n as f32 * advance <= w + 0.01;
    let mut lines: Vec<Vec<&str>> = vec![];
    for word in s.split(' ') {
        match lines.last_mut() {
            Some(l) if fits(l.iter().map(|x| x.chars().count() + 1).sum::<usize>() + word.chars().count()) => l.push(word),
            _ => lines.push(vec![word]),
        }
    }
    let n = lines.len();
    if company && n >= 2 && lines[n - 1].len() == 1 && lines[n - 2].len() > 2 {
        let moved = lines[n - 2].pop().unwrap();
        lines[n - 1].insert(0, moved);
    }
    lines.iter().map(|l| l.join(" ")).collect()
}

/// A paragraph: lines `pitch` apart. Returns it and its height.
fn para(s: &str, style: Ts, c: Rgb8, w: f32, pitch: f32, company: bool) -> (Div, f32) {
    let lines = wrap(s, style.size * MONO_ADVANCE + style.tracking, w, company);
    let n = lines.len() as f32;
    let h = (n - 1.0) * pitch + style.line;
    let mut p = sized(w, h);
    for (i, l) in lines.into_iter().enumerate() {
        p = p.child(at(t(l, style, c), 0.0, i as f32 * pitch));
    }
    (p, h)
}

const BODY_PITCH: f32 = 15.0;
const CAPTION_PITCH: f32 = 13.0;

fn body_para(s: &str, c: Rgb8, w: f32, company: bool) -> (Div, f32) {
    para(s, body(), c, w, BODY_PITCH, company)
}

fn caption_para(s: &str, c: Rgb8, w: f32, company: bool) -> (Div, f32) {
    para(s, caption(), c, w, CAPTION_PITCH, company)
}

/// A pixel-ceiled width, as SwiftUI sizes a line of text.
fn px_ceil(v: f32, scale: f32) -> f32 {
    (v * scale - 0.01).ceil() / scale
}

/// A labelled element for VoiceOver.
fn labelled(e: Div, id: String, role: Role, label: impl Into<gpui::SharedString>) -> AnyElement {
    e.id(gpui::SharedString::from(id)).role(role).aria_label(label).into_any_element()
}

// MARK: - A stack, laid out by hand

/// A column of children at known heights, `gap` apart: SwiftUI's VStack.
struct Stack {
    e: Div,
    h: f32,
    gap: f32,
    first: bool,
}

impl Stack {
    fn new(w: f32, gap: f32) -> Stack {
        Stack { e: sized(w, 0.0), h: 0.0, gap, first: true }
    }
    fn push(&mut self, (child, h): (impl IntoElement, f32)) {
        self.push_at(child, 0.0, h);
    }
    fn push_at(&mut self, child: impl IntoElement, x: f32, h: f32) {
        if !self.first {
            self.h += self.gap;
        }
        self.first = false;
        let e = std::mem::replace(&mut self.e, div());
        self.e = e.child(at(child, x, self.h));
        self.h += h;
    }
    fn done(self) -> (Div, f32) {
        let h = self.h;
        (self.e.h(z(h)), h)
    }
}

// MARK: - The window

/// What the page needs from the frame: its scale, and the widths it lays out at.
struct Look {
    scale: f32,
    /// The step's column.
    col: f32,
    /// The running head's room.
    head: f32,
}

/// The whole window for `state`, drawn as the snapshot draws it (unscrolled), `height` tall
/// (its own height when `None`).
pub fn window(state: &SetupState, height: Option<f32>, scale: f32) -> Div {
    let (body, shift, slot, h) = laid_out(state, scale);
    let own = PLATE_H + slot;
    let height = height.unwrap_or(own);
    if std::env::var("IRIS_DEBUG").is_ok() {
        eprintln!("setup-{}: height {own}, page {page_w} wide", state.id, page_w = WIDTH - 2.0 * shift);
    }
    let p = Progress(state.facts);
    let grain = printed(WIDTH, height, &riso::grain(WIDTH, height, scale, 0.22));
    sized(WIDTH, height).bg(col(ink::PAPER)).overflow_hidden()
        .child(at(grain, 0.0, 0.0))
        .child(at(body, shift, PLATE_H + (slot - h) / 2.0))
        .child(at(plate(&p, scale), shift, 0.0))
}

/// The window's height (`OnboardingView.height`): it fits a 13-inch laptop's screen, and the
/// model step's dots and its Download button are in view at once.
pub const HEIGHT: f32 = 720.0;

/// The live window, `HEIGHT` tall under its title bar: the plate stays put while the page scrolls under it, so
/// the garden is always in sight.
pub fn live_window(state: &SetupState, scale: f32, scroll: &gpui::ScrollHandle) -> Div {
    let (body, shift, _, h) = laid_out(state, scale);
    let p = Progress(state.facts);
    // The window's title bar is paper too (`PaperWindow`): the page starts under it.
    let bar = crate::settings::TITLE_BAR;
    let grain = printed(WIDTH, bar + HEIGHT, &riso::grain(WIDTH, bar + HEIGHT, scale, 0.22));
    let page = div().id("setup-page").absolute().left_0().top(z(bar + PLATE_H)).w(z(WIDTH)).h(z(HEIGHT - PLATE_H))
        .overflow_y_scroll().track_scroll(scroll)
        // A scroll view lays the page out at its own height, from the top.
        .child(sized(WIDTH, h).flex_none().child(at(body, shift, 0.0)));
    sized(WIDTH, bar + HEIGHT).bg(col(ink::PAPER)).overflow_hidden()
        .child(at(grain, 0.0, 0.0))
        .child(page)
        .child(at(plate(&p, scale), shift, bar))
}

/// The page for `state`: its body, how far left it sits (a page wider than the window is
/// centred on it), the height it was sized at, and its own height.
fn laid_out(state: &SetupState, scale: f32) -> (Div, f32, f32, f32) {
    let p = Progress(state.facts);
    // SwiftUI sizes the page at 720 (the step's column 391), then lays it out as wide as its
    // fixed parts make it, centred on the window, the content centred in the height it had.
    let sizing = Look { scale, col: WIDTH - BESIDE, head: WIDTH - 80.0 - 120.0 };
    let page_w = (BESIDE + fixed_w(&p, scale)).max(WIDTH);
    let placed = Look { scale, col: page_w - BESIDE, head: page_w - 80.0 - 120.0 };
    let (_, slot) = page(&p, state, &sizing);
    let (body, h) = page(&p, state, &placed);
    let shift = -(page_w - WIDTH) / 2.0;
    (body, shift, slot, h)
}

/// How wide the step's fixed parts are: what can't wrap or shrink.
fn fixed_w(p: &Progress, scale: f32) -> f32 {
    let headline = |lines: [&str; 2], drawing: bool| title_w(&lines) + if drawing { 8.0 + 76.0 } else { 0.0 };
    match p.current() {
        Some(Step::Allow) => {
            let lines = if p.is_returning() { ["Allow", "✻ Iris again"] } else { ["Let Iris", "✻ in"] };
            let open = primary_w("Open System Settings", None, scale);
            let row = if p.status(Step::Allow) == Status::Stuck {
                open + 14.0 + secondary_w("Quit and reopen Iris", scale)
            } else {
                open + 14.0 + hand_note_w("then turn Iris on", 19.0, 5, 34.0, scale)
            };
            headline(lines, true).max(row)
        }
        Some(Step::Model) => {
            let h = headline(["Get", "✻ the model"], p.0.activity != Activity::None);
            match p.0.activity {
                Activity::None | Activity::Ready => h.max(190.0 * 2.0 + 18.0),
                _ => h,
            }
        }
        Some(Step::TryIt) => headline(["Try", "✻ it once"], true),
        None => headline(["All", "✻ set"], false),
    }
}

fn page(p: &Progress, state: &SetupState, look: &Look) -> (Div, f32) {
    let page_w = look.col + BESIDE;
    let (head, head_h) = running_head(&["Setup", if p.is_returning() { "One step" } else { "About five minutes" }, "Everything runs on this Mac"], look.head);
    let (step, step_h) = step_page(p, state, look);
    let (list, list_h) = step_list(p, look.scale);
    let body_h = step_h.max(list_h);
    let rule = drawn(1.0, body_h, pen::hand_line_vertical(31, 0.7, 0.5, 0.0, body_h), Some(1.0), over(hue::I12, 0.3, ink::PAPER));
    let body = sized(look.col + 26.0 * 2.0 + 1.0 + LIST_W, body_h)
        .child(at(step, 0.0, 0.0))
        .child(at(rule, look.col + 26.0, 0.0))
        .child(at(list, look.col + 53.0, 0.0));
    let (foot, foot_h) = foot(p, state, page_w - 80.0, look.scale);
    let mut s = Stack::new(page_w - 80.0, 16.0);
    s.push((head, head_h));
    s.push((body, body_h));
    s.push((foot, foot_h));
    let (content, h) = s.done();
    (sized(page_w, 14.0 + h + 26.0).child(at(content, 40.0, 14.0 + PAGE_DROP)), 14.0 + h + 26.0)
}

// MARK: - The plate

/// The garden's drawing for a stage of setup (tried draws ready, with more butterflies).
pub fn garden_stage(g: GardenStage) -> Stage {
    match g {
        GardenStage::Bud => Stage::Bud,
        GardenStage::Allowed => Stage::Allowed,
        GardenStage::Watering => Stage::Watering,
        GardenStage::Warming => Stage::Warming,
        GardenStage::Ready | GardenStage::Tried => Stage::Ready,
    }
}

/// The zine's cover: a screen-printed sky plate, a giant ghost word cut off by the page, the
/// wordmark, and the garden standing on the plate's edge and spilling over it.
fn plate(p: &Progress, scale: f32) -> Div {
    let word = if p.is_returning() { "AGAIN" } else { "HELLO" };
    let (logo, _, logo_h) = letters::iris_logo(30.0, scale);
    let (stage, extra) = match p.garden() {
        GardenStage::Bud => (Stage::Bud, false),
        GardenStage::Allowed => (Stage::Allowed, false),
        GardenStage::Watering => (Stage::Watering, false),
        GardenStage::Warming => (Stage::Warming, false),
        GardenStage::Ready => (Stage::Ready, false),
        GardenStage::Tried => (Stage::Ready, true),
    };
    let progress = if let Activity::Downloading(f) = p.0.activity { f } else { 0.0 };
    // Rune scales the garden's 190-point box 1.16 about its bottom middle: here it's printed
    // 1.16 times its size (with its stray room round it) and placed where that box lands.
    let (g, side) = garden_print(stage, progress, extra, scale);
    let (x0, y0) = (WIDTH - 24.0 - garden::SIZE, 30.0);
    let gx = x0 + garden::SIZE / 2.0 - (garden::SIZE / 2.0 + garden::PAD) * GARDEN_ZOOM;
    let gy = y0 + garden::SIZE - (garden::SIZE + garden::PAD) * GARDEN_ZOOM;
    sized(WIDTH, PLATE_H)
        .child(at(printed(WIDTH, PLATE_H, &sky_plate(word, scale)), 0.0, 0.0))
        .child(at(logo, 40.0, PLATE_H - 26.0 - logo_h))
        .child(at(printed(side, side, &g), gx, gy))
}

/// The moment the garden is drawn at (the Swift snapshots catch it live, so its butterflies
/// and the sun's turn fall where they fall).
const GARDEN_T: f64 = 1000.0;
const GARDEN_ZOOM: f32 = 1.16;

/// The garden printed `GARDEN_ZOOM` times its size, with, once Iris has been tried, two more
/// butterflies (garden.rs flies two at most). Returns it and its side in points.
fn garden_print(stage: Stage, progress: f64, extra: bool, scale: f32) -> (Img, f32) {
    if !extra {
        return garden::garden(stage, progress, None, GARDEN_T, GARDEN_ZOOM, scale);
    }
    let side = (garden::SIZE + 2.0 * garden::PAD) * GARDEN_ZOOM;
    let plate = Plate { w: side, h: side, scale };
    let img = riso::cached(format!("setup-garden-tried-{stage:?}"), &plate, || {
        let (pw, ph, rgba) = garden::garden_rgba(stage, progress, None, GARDEN_T, GARDEN_ZOOM, scale);
        let mut sh = Sheet::new(plate, 0.0, 0.0, 0.0);
        sh.picture_at(&rgba, pw, ph, side, side, Transform::identity(), 1.0);
        let base = Transform::from_scale(GARDEN_ZOOM, GARDEN_ZOOM).pre_translate(garden::PAD, garden::PAD);
        for i in 2..4 {
            butterfly(&mut sh, base, i, GARDEN_T);
        }
        sh.rgba()
    });
    (img, side)
}

/// A butterfly on its loop round the flower head (`GardenView`'s third and fourth), as
/// garden.rs flies the first two: flapping (squeezed across), turned to its heading.
fn butterfly(sh: &mut Sheet, base: Transform, i: usize, t: f64) {
    const LOOPS: [(f64, f64, f64, f64); 4] = [(66.0, 24.0, 0.55, 0.0), (58.0, 30.0, 0.72, 2.2), (72.0, 20.0, 0.46, 4.1), (50.0, 26.0, 0.84, 1.1)];
    let l = LOOPS[i % 4];
    let point = |t: f64| {
        let a = t * l.2 + l.3;
        (95.0 + l.0 * a.sin(), 58.0 - 8.0 + l.1 * (2.0 * a).sin() * 0.6 - 8.0 * (a * 1.3).sin())
    };
    let (here, next) = (point(t), point(t + 0.05));
    let heading = (next.1 - here.1).atan2(next.0 - here.0) + std::f64::consts::FRAC_PI_2;
    let flap = (0.28 + 0.72 * (t * 15.0 + l.3).sin().abs()) as f32;
    let s = 32.0f32;
    let b = garden::butterfly_raw();
    let local = base.pre_translate(here.0 as f32, here.1 as f32).pre_rotate(heading.to_degrees() as f32)
        .pre_translate(-s / 2.0, -s / 2.0).pre_translate(s * (1.0 - flap) / 2.0, 0.0);
    sh.sprite_at(&b.1, b.0, b.0, s * flap, s, local, 1.0);
}

/// The ghost word: Londrina at 150, paper at 32%, its box's bottom 46 below the plate's and
/// 14 left of it.
const GHOST_SIZE: f32 = 150.0;
const GHOST_LOWER: f32 = 0.0;

fn sky_plate(word: &str, scale: f32) -> Img {
    let plate = Plate { w: WIDTH, h: PLATE_H, scale };
    riso::cached(format!("setup-plate-{word}"), &plate, || {
        let mut all = plate.mask();
        all.fill_path(&PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, WIDTH, PLATE_H).unwrap()), FillRule::Winding, false, plate.transform());
        let mut out = riso::print_with(&plate, SKY, 11, 1.0, 1.0, &all, 1.0, |c| c);
        let font = FontRef::new(LONDRINA).unwrap();
        let m = font.metrics(Size::new(GHOST_SIZE), LocationRef::default());
        let baseline = PLATE_H + 46.0 + m.descent + GHOST_LOWER;
        let mut glyphs = plate.mask();
        let mut b = Outline { b: PathBuilder::new(), x: 0.0, y: baseline };
        let outlines = font.outline_glyphs();
        for (gid, x) in letters::shape_londrina(word, GHOST_SIZE).0 {
            if let Some(o) = outlines.get(skrifa::GlyphId::new(gid)) {
                b.x = -14.0 + x;
                let _ = o.draw(DrawSettings::unhinted(Size::new(GHOST_SIZE), LocationRef::default()), &mut b);
            }
        }
        if let Some(path) = b.b.finish() {
            glyphs.fill_path(&path, FillRule::Winding, true, plate.transform());
        }
        let paper = [ink::PAPER.0, ink::PAPER.1, ink::PAPER.2];
        for (c, g) in out.chunks_exact_mut(4).zip(glyphs.data()) {
            let a = *g as f32 / 255.0 * 0.32;
            for k in 0..3 {
                c[k] = (c[k] as f32 * (1.0 - a) + paper[k] as f32 * a).round() as u8;
            }
        }
        out
    })
}

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

// MARK: - Running head

/// `RunningHead`: small columns over short pen rules, `w` wide.
fn running_head(items: &[&str; 3], w: f32) -> (Div, f32) {
    let cw = (w - 36.0) / 3.0;
    let mut r = sized(w, 0.0);
    let mut h: f32 = 0.0;
    for (i, item) in items.iter().enumerate() {
        let lines = push_out(item, cw, 9.5);
        let n = lines.len() as f32;
        let th = (n - 1.0) * HEAD_PITCH + 9.5 * MONO_LINE;
        let mut c = sized(cw, th + 5.0 + 3.0);
        for (k, l) in lines.iter().enumerate() {
            c = c.child(at(t(l.clone(), micro(), hue::I10), 0.0, k as f32 * HEAD_PITCH));
        }
        c = c.child(at(pen_rule(40 + i as i64 * 7, 64.0, over(hue::I12, 0.35, ink::PAPER), 1.0), 0.0, th + 5.0));
        r = r.child(at(c, i as f32 * (cw + 18.0), 0.0));
        h = h.max(th + 8.0);
    }
    (r.h(z(h)), h)
}

const HEAD_PITCH: f32 = 11.5;

// MARK: - Headline

/// The width of a stair headline's widest line, where it starts.
fn title_w(lines: &[&str; 2]) -> f32 {
    lines.iter().enumerate().map(|(i, l)| i as f32 * 44.0 + display_w(&l.to_uppercase(), 38.0)).fold(0.0, f32::max)
}

/// Martian at width 75, with Zapf Dingbats' ✻.
fn display_w(s: &str, size: f32) -> f32 {
    let adv = letters::martian_advance(size, 400.0, 75.0);
    s.chars().map(|c| if c == '✻' { STAR_ADVANCE * size } else { adv }).sum()
}

/// Zapf Dingbats' ✻ as Core Text sets it in the headline (measured): its advance, a share of the em.
const STAR_ADVANCE: f32 = 0.579;

/// "✻" as Core Text falls back for it (Zapf Dingbats, `letters::star_into`), drawn in a box
/// `size` × 0.95 `size` from its top, ink 0.55 em wide.
fn star(size: f32, scale: f32) -> Div {
    let k = size / 38.0;
    let side = 24.0 * k;
    let plate = Plate { w: side, h: side, scale };
    let img = riso::cached(format!("setup-star-{size}"), &plate, || {
        let mut mask = plate.mask();
        letters::star_into(&mut mask, &plate, 12.0 * k, 12.0 * k, 12.0 * k);
        riso::flat(&plate, hue::I12, &mask, 1.0)
    });
    // Its ink starts a point in, as the glyph's does.
    sized(side, side).child(at(printed(side, side, &img), (1.0 - 1.5) * k, 0.0))
}

/// `Headline`: the stair of condensed caps with ✻, the step's drawing printed big beside it.
fn headline(lines: [&str; 2], drawing: Option<Div>, w: f32, scale: f32) -> (AnyElement, f32) {
    // Each line's box on whole points (45.6 takes 46).
    let lh = line_box(38.0, MONO_LINE);
    let h = 2.0 * lh - 6.0;
    let mut e = sized(w, h);
    for (i, l) in lines.iter().enumerate() {
        let (x, y) = (i as f32 * 44.0, i as f32 * (lh - 6.0));
        let upper = l.to_uppercase();
        match upper.strip_prefix('✻') {
            Some(rest) => {
                e = e.child(at(star(38.0, scale), x, y + mono_base(38.0) - 23.0))
                    .child(at(display(rest, 38.0, scale).0, x + STAR_ADVANCE * 38.0, y));
            }
            None => e = e.child(at(display(&upper, 38.0, scale).0, x, y)),
        }
    }
    if let Some(d) = drawing {
        e = e.child(at(d, w - 76.0, -4.0));
    }
    (labelled(e, format!("setup-headline-{}", lines.join(" ")), Role::Heading, lines.join(" ")), h)
}

// MARK: - Drawings (`RisoDrawing`)

/// The headline's drawing turns −6°.
const DRAWING_TURN: f32 = -6.0;

/// A riso icon printed big: a dot a point, the pink plate a few dots off the ink, blown up
/// without smoothing, then turned −6° (printed whole, turned).
fn poster(name: &'static str, pixels: u32, art: [fn(&mut Draw); 3], scale: f32) -> Div {
    let k = scale.round().max(1.0) as u32;
    let pt = pixels as f32;
    turned(format!("setup-poster-{name}-{pixels}-{DRAWING_TURN}"), pt, pt, (pt * 0.12).ceil(), DRAWING_TURN, scale, move |sh| {
        let off = (pixels as i32 / 28).max(2);
        let small = icons::print(pixels, str_seed(name, 11), &[(ink::CREMA, off, off + 1, art[0]), (ink::PINK, off + 1, -off, art[1]), (ink::ESPRESSO, 0, 0, art[2])]);
        let side = pixels * k;
        let mut big = vec![0u8; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                let s = (((y / k) * pixels + x / k) * 4) as usize;
                let d = ((y * side + x) * 4) as usize;
                big[d..d + 4].copy_from_slice(&small[s..s + 4]);
            }
        }
        sh.picture_at(&big, side, side, pt, pt, Transform::identity(), 1.0);
    })
}

fn hand_drawing(scale: f32) -> Div {
    use crate::pages::privacy::palm;
    poster("hand", 76, [
        |d| for p in palm(0.0) { d.fill(&p.transform(Transform::from_translate(1.0, 2.0)).unwrap(), 150) },
        |d| for p in palm(0.0) { d.fill(&p, 85) },
        |d| {
            d.pen(4.2);
            for p in palm(0.0) { d.stroke(&p) }
            for p in palm(2.1) { d.fill(&p, 255) }
        },
    ], scale)
}

fn bubble_tail() -> tiny_skia::Path {
    let mut b = PathBuilder::new();
    b.move_to(24.0, 52.0);
    b.line_to(17.0, 68.0);
    b.line_to(38.0, 53.0);
    b.finish().unwrap()
}

fn tones_drawing(scale: f32) -> Div {
    poster("tones", 76, [
        |d| {
            d.fill(&rounded(10.0, 14.0, 60.0, 40.0, 12.0), 150);
            d.fill(&bubble_tail(), 150);
        },
        |d| {
            d.fill(&rounded(10.0, 14.0, 60.0, 40.0, 12.0), 75);
            d.fill(&bubble_tail(), 75);
        },
        |d| {
            d.pen(4.8);
            d.stroke(&rounded(10.0, 14.0, 60.0, 40.0, 12.0));
            let mut b = PathBuilder::new();
            b.move_to(24.0, 54.0);
            b.line_to(17.0, 68.0);
            b.line_to(38.0, 54.0);
            d.stroke(&b.finish().unwrap());
            d.pen(4.2);
            let mut b = PathBuilder::new();
            b.move_to(20.0, 34.0);
            let mut x = 20.0f32;
            while x <= 60.0 {
                b.line_to(x, 34.0 + 6.0 * ((x - 20.0) / 40.0 * 2.0 * std::f32::consts::PI).sin());
                x += 1.0;
            }
            d.stroke(&b.finish().unwrap());
        },
    ], scale)
}

// MARK: - Buttons

fn title_w_body(s: &str, scale: f32) -> f32 {
    px_ceil(mono_w(s, 12.5, -0.12), scale)
}

fn primary_w(title: &str, icon: Option<&str>, scale: f32) -> f32 {
    let tw = title_w_body(title, scale);
    if icon.is_some() { 16.0 + 10.0 + 6.0 + tw + 18.0 } else { 18.0 + tw + 18.0 }
}

fn secondary_w(title: &str, scale: f32) -> f32 {
    24.0 + title_w_body(title, scale)
}

/// What a button does in the live window: hands `a` to the window (nothing in a snapshot).
fn on(a: Act) -> crate::interact::Act {
    std::rc::Rc::new(move |window, cx| {
        if let Some(f) = ON_ACT.with(|h| h.borrow().clone()) {
            f(a, window, cx);
        }
    })
}

/// `TactileButton(role: .primary)`, with a pen glyph before its words when `icon`.
fn primary(title: &str, icon: Option<&'static str>, a: Act, scale: f32) -> (AnyElement, f32) {
    let mut b = crate::controls::Button::primary(title).id(format!("setup-{title}"));
    if let Some(i) = icon {
        b = b.icon(i);
    }
    crate::controls::button(&b, scale, on(a))
}

/// `TactileButton(role: .secondary)`, `h` tall (26: compact).
fn secondary_button(title: &str, h: f32, a: Act, id: &str, scale: f32) -> (AnyElement, f32) {
    let mut b = crate::controls::Button::secondary(title).id(format!("setup-{id}"));
    if h < 30.0 {
        b = b.compact();
    }
    crate::controls::button(&b, scale, on(a))
}

/// `PenGlyph("arrow.down")`: a pen arrow pointing down, a touch off straight.
fn arrow_down(s: f32) -> Pen {
    let mut p = Pen::new();
    p.m(0.5 * s, 0.04 * s);
    p.q(0.46 * s, 0.5 * s, 0.52 * s, 0.9 * s);
    p.m(0.16 * s, 0.58 * s);
    p.l(0.52 * s, 0.94 * s);
    p.l(0.86 * s, 0.56 * s);
    p
}

// MARK: - Hand notes

/// `HandArrow(curl: .left)`: up and across, pointing right.
fn arrow_left(w: f32, h: f32) -> Pen {
    let mut p = Pen::new();
    let (a, b) = ([0.0, h - 2.0], [w, h * 0.45]);
    p.m(a[0], a[1]);
    p.q(w / 2.0, -4.0, b[0], b[1]);
    p.m(b[0] - 7.0, b[1] - 5.0);
    p.l(b[0], b[1]);
    p.l(b[0] - 6.0, b[1] + 5.0);
    p
}

fn hand_note_w(word: &str, size: f32, seed: i64, reach: f32, scale: f32) -> f32 {
    letters::hand_word(word, size, seed, &[], 1.0, scale).1 + 3.0 + reach
}

/// A GPUI picture's pixels back as straight RGBA (GPUI keeps them BGRA).
fn rgba_of(img: &Img) -> Vec<u8> {
    let mut v = img.as_bytes(0).map(|b| b.to_vec()).unwrap_or_default();
    for px in v.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    v
}

/// `HandNote`: a hand-lettered label turned −4° (printed whole, turned) with a pen arrow off its
/// trailing side (its leading side when `flipped`). Returns it, its width and height.
fn hand_note(word: &str, size: f32, seed: i64, down: bool, reach: (f32, f32), flipped: bool, scale: f32) -> (Div, f32, f32) {
    let (px, ww, wh) = letters::hand_word(word, size, seed, &[], 1.0, scale);
    let w = ww + 3.0 + reach.0;
    let h = wh.max(reach.1);
    let mut arrow = if down { pen::hand_arrow_down(reach.0, reach.1) } else { arrow_left(reach.0, reach.1) };
    if flipped {
        arrow = mirror(&arrow, reach.0);
    }
    let arrow = drawn(reach.0, reach.1, arrow, Some(1.6), col(hue::I12));
    let ay = (h - reach.1) / 2.0 + if down { reach.1 / 2.0 } else { 0.0 };
    let pad = (ww.max(wh) * 0.06).ceil() + 2.0;
    let word_el = turned(format!("setup-note-{word}-{size}-{seed}"), ww, wh, pad, -4.0, scale, move |sh| {
        sh.picture(&rgba_of(&px), ww, wh, 0.0, 0.0);
    });
    let wy = (h - wh) / 2.0;
    let e = if flipped {
        sized(w, h).child(at(arrow, 0.0, ay)).child(at(word_el, reach.0 + 3.0, wy))
    } else {
        sized(w, h).child(at(word_el, 0.0, wy)).child(at(arrow, ww + 3.0, ay))
    };
    (e, w, h)
}

fn mirror(p: &Pen, w: f32) -> Pen {
    use pen::Cmd;
    let f = |q: [f32; 2]| [w - q[0], q[1]];
    Pen(p.0.iter().map(|c| match *c {
        Cmd::M(q) => Cmd::M(f(q)),
        Cmd::L(q) => Cmd::L(f(q)),
        Cmd::Q(a, q) => Cmd::Q(f(a), f(q)),
        Cmd::C(a, b, q) => Cmd::C(f(a), f(b), f(q)),
        Cmd::Z => Cmd::Z,
    }).collect())
}

// MARK: - Problems

/// `ProblemLine`: the red ring round "?" (`ToastMark(kind: .warning)`) and the note in red brush.
fn problem_line(words: &str, scale: f32) -> (AnyElement, f32) {
    let lh = brush_line_h(24.0, scale);
    let qh = brush_line_h(20.0, scale);
    let qw = brush_w("?", 20.0);
    let mark = sized(24.0, 24.0)
        .child(at(t("?", brush_px(20.0, scale), hue::RED_TEXT), (24.0 - qw) / 2.0, (24.0 - qh) / 2.0))
        .child(at(crate::toast::brush_ring(ink::RED, 9, 24.0, 24.0, 1.0, scale), 0.0, 0.0));
    let h = lh.max(24.0);
    let e = sized(0.0, h)
        .child(at(mark, 0.0, (h - 24.0) / 2.0))
        .child(at(t(words, brush_px(24.0, scale), hue::RED_TEXT), 32.0, (h - lh) / 2.0));
    (labelled(e, format!("setup-problem-{words}"), Role::Label, words.to_string()), h)
}

/// `NumberedSteps`: a fix as numbered steps, the numbers small pink folios on the first baseline.
fn numbered_steps(items: &[&str], w: f32) -> (Div, f32) {
    let mut s = Stack::new(w, 4.0);
    let num_w = mono_w("01", 11.0, 0.0);
    for (i, item) in items.iter().enumerate() {
        let tw = w - num_w - 8.0;
        let (words, h) = body_para(item, hue::I11, tw, true);
        let n = t(format!("{:02}", i + 1), caption(), ink::PINK_TEXT);
        let dy = mono_base(12.5) - mono_base(11.0);
        let row = sized(w, h).child(at(n, 0.0, dy)).child(at(words, num_w + 8.0, 0.0));
        s.push((row, h));
    }
    s.done()
}

// MARK: - Keys

/// `KeyRow`: labelled Mac keys side by side. Returns it and its width.
fn key_row(keys: &[(Option<&str>, &str)], scale: f32) -> (AnyElement, f32) {
    use crate::pages::shortcuts::{labeled_keycap, labeled_w};
    let mut r = div();
    let mut x = 0.0;
    for (i, (symbol, word)) in keys.iter().enumerate() {
        if i > 0 {
            x += 5.0;
        }
        r = r.child(at(labeled_keycap(word, *symbol, str_seed(word, 7), false, scale), x, 0.0));
        x += labeled_w(word, *symbol);
    }
    let spoken = keys.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ");
    let e = r.relative().flex_none().w(z(x)).h(z(32.0));
    (labelled(e, format!("setup-keys-{spoken}"), Role::Group, spoken), x)
}

/// `keyLine`: words and keycaps on one line, or the keys on a line of their own when that
/// doesn't fit the column.
fn key_line(before: &str, keys: &[(Option<&str>, &str)], after: Option<&str>, meta: bool, w: f32, scale: f32) -> (Div, f32) {
    let (style, size, ink_c) = if meta { (caption(), 11.0, hue::I10) } else { (body(), 12.5, hue::I11) };
    let lh = size * MONO_LINE;
    let (keys_el, kw) = key_row(keys, scale);
    let bw = mono_w(before, size, 0.0);
    let aw = after.map_or(0.0, |a| 8.0 + mono_w(a, size, 0.0));
    let one = bw + 8.0 + kw + aw;
    if one <= w + 0.01 {
        let h = 32.0f32.max(lh);
        let mut e = sized(one, h)
            .child(at(t(before, style, ink_c), 0.0, (h - lh) / 2.0))
            .child(at(keys_el, bw + 8.0, (h - 32.0) / 2.0));
        if let Some(a) = after {
            e = e.child(at(t(a, style, ink_c), bw + 8.0 + kw + 8.0, (h - lh) / 2.0));
        }
        (e, h)
    } else {
        let (words, wh) = para(before, style, ink_c, w, if meta { CAPTION_PITCH } else { BODY_PITCH }, true);
        let h = wh + 6.0 + 32.0;
        let mut e = sized(w, h).child(at(words, 0.0, 0.0)).child(at(keys_el, 0.0, wh + 6.0));
        if let Some(a) = after {
            e = e.child(at(t(a, style, ink_c), kw + 8.0, wh + 6.0 + (32.0 - lh) / 2.0));
        }
        (e, h)
    }
}

// MARK: - The step's page

fn step_page(p: &Progress, state: &SetupState, look: &Look) -> (Div, f32) {
    match p.current() {
        Some(Step::Allow) => allow_page(p, look),
        Some(Step::Model) => model_page(p, state, look),
        Some(Step::TryIt) => try_page(state, look),
        None => done_page(state, look),
    }
}

fn allow_page(p: &Progress, look: &Look) -> (Div, f32) {
    let (w, s) = (look.col, look.scale);
    let mut st = Stack::new(w, 14.0);
    let lines = if p.is_returning() { ["Allow", "✻ Iris again"] } else { ["Let Iris", "✻ in"] };
    st.push(headline(lines, Some(hand_drawing(s)), w, s));
    if p.status(Step::Allow) == Status::Stuck {
        st.push(problem_line("the switch is on, but I can't see it", s));
        st.push(body_para("macOS sometimes keeps an old copy of the permission. Clear it and add Iris again:", hue::I11, w, true));
        st.push(numbered_steps(&["In Accessibility, select Iris and press −.", "Press +, and choose Iris in Applications.", "Quit and reopen Iris."], w));
        let (open, ow) = primary("Open System Settings", None, Act::OpenSystemSettings, s);
        let (quit, _) = secondary_button("Quit and reopen Iris", 32.0, Act::Relaunch, "relaunch", s);
        st.push((sized(w, 32.0).child(at(open, 0.0, 0.0)).child(at(quit, ow + 14.0, 0.0)), 32.0));
    } else {
        let words = if p.is_returning() {
            "macOS turned off Iris's permission, often after an update. Everything else is as you left it."
        } else {
            "Iris reads the text box you're typing in and pastes the fix back. macOS asks you to allow that once."
        };
        st.push(body_para(words, hue::I11, w, true));
        let (open, ow) = primary("Open System Settings", None, Act::OpenSystemSettings, s);
        let (note, _, nh) = hand_note("then turn Iris on", 19.0, 5, false, (34.0, 18.0), false, s);
        let h = nh.max(32.0);
        st.push((sized(w, h).child(at(open, 0.0, (h - 32.0) / 2.0)).child(at(note, ow + 14.0, (h - nh) / 2.0)), h));
        st.push(caption_para("Privacy & Security › Accessibility › Iris. This page turns to step 2 by itself. On a company Mac, IT may need to allow it.", hue::I10, w, false));
    }
    st.done()
}

fn model_page(p: &Progress, state: &SetupState, look: &Look) -> (Div, f32) {
    let (w, s) = (look.col, look.scale);
    let m = &MODELS[chosen(state)];
    let mut st = Stack::new(w, 10.0);
    let drawing = (p.0.activity != Activity::None).then(|| crate::pages::model::chip(76, DRAWING_TURN, s));
    st.push(headline(["Get", "✻ the model"], drawing, w, s));
    match p.0.activity {
        Activity::Downloading(f) => {
            st.push(body_para(&format!("Downloading {}, one time.", m.name.to_lowercase()), hue::I11, w, true));
            st.push((crate::slip::progress_track(f, w, s), 18.0));
            st.push((t(m.downloaded(f), caption(), hue::I10), 11.0 * MONO_LINE));
            st.push(body_para("You can close this window. When the model's in, the menu bar flower will say so.", hue::I11, w, true));
            let (stop, _) = secondary_button("Stop download", 32.0, Act::StopDownload, "stop", s);
            st.push((stop, 32.0));
        }
        Activity::Loading => {
            // The dots caught at the moment the Swift picture caught them (measured: the first
            // darkest, the middle palest), `sin(5t − 0.9i)` at 5t = 5.86.
            let dots = ink_dots_at(hue::I11, 4.0, 5.86 / 5.0);
            let e = sized(w, 15.0)
                .child(at(dots, 0.0, (15.0 - 4.0) / 2.0))
                .child(at(t("Waking it up. A few seconds.", body(), hue::I11), 4.0 * 3.0 + 3.2 * 2.0 + 6.0, 0.0));
            st.push((e, 15.0));
        }
        Activity::Failed(reported) => {
            let failure = if reported == Failure::NotDownloaded { Failure::Other } else { reported };
            st.push(problem_line(failure.lead(), s));
            if let Failure::NoRoom { .. } = failure {
                st.push(body_para(&format!("{} Iris tries again by itself once there's room.", failure.next_step()), hue::I11, w, true));
                let (b, _) = primary("Open Storage Settings", None, Act::OpenStorage, s);
                st.push((b, 32.0));
            } else {
                st.push(body_para(&failure.next_step(), hue::I11, w, true));
                let (b, _) = primary("Try again", None, Act::TryAgain, s);
                st.push((b, 32.0));
            }
        }
        _ => {
            st.push(model_chooser(state, s));
            // Why one is recommended, and what the dots measure (`ModelPipsNote`).
            let note_lines = crate::settings_kit::wrap_mono(&format!("{} Dots: how often an answer was good enough to send, in blind tests.",
                crate::model_compare::recommendation_line(crate::model_compare::mac_memory_gb())), 11.0 * MONO_ADVANCE, w).len();
            st.push((crate::model_compare::pips_note(w), note_lines as f32 * CAPTION_PITCH));
            let title = format!("Download {}", m.name.to_lowercase());
            let (b, bw) = primary(&title, Some("arrow.down"), Act::Download, s);
            let note = format!("One time, {}.", m.download_size());
            let e = sized(w, 32.0)
                .child(at(b, 0.0, 0.0))
                .child(at(t(note, caption(), hue::I10), bw + 14.0, (32.0 - 11.0 * MONO_LINE) / 2.0));
            st.push((e, 32.0));
        }
    }
    st.done()
}

/// `ModelChooser`: the two models drawn side by side, the chosen one ringed in pink, the one
/// that suits this Mac marked recommended, Use this one under the other.
fn model_chooser(state: &SetupState, scale: f32) -> (Div, f32) {
    use crate::pages::model::{chip, recommended_mark, ring, stacked_figure};
    let mut r = div();
    let mut h: f32 = 0.0;
    for (i, m) in MODELS.iter().enumerate() {
        let chosen = i == self::chosen(state);
        let on_disk = state.live.as_ref().is_some_and(|l| l.on_disk[i]);
        let first = i == 0;
        let size = if first { 74 } else { 60 };
        let sz = size as f32;
        let mut figure = sized(104.0, 88.0)
            .child(at(chip(size, if first { -6.0 } else { 5.0 }, scale), (104.0 - sz) / 2.0, (88.0 - sz) / 2.0));
        if chosen {
            figure = figure.child(ring(13 + i as i64, 98.0, 86.0, 52.0, 44.0, scale));
        }
        // The drawing, its name and its size are one button: choosing it (`ModelOption`).
        let mut option = Stack::new(190.0, 8.0);
        option.push((figure, 88.0));
        option.push((t(m.name, body(), hue::I12), 12.5 * MONO_LINE));
        option.push((stacked_figure(&m.figure(), "GB", if on_disk { "on disk" } else { "to download" }, scale), 26.0 * MONO_LINE));
        let (option, option_h) = option.done();
        let mut opt = Stack::new(190.0, 8.0);
        opt.push((clickable(option, &format!("model-{}", m.id), Act::Use(i)), option_h));
        // What it's good at, as dots (`ModelPips`, 2 below the figure).
        if let Some(st) = crate::model_compare::Strengths::of(m.id) {
            let ph = 4.0 * 11.0 * MONO_LINE + 3.0 * 5.0;
            opt.push((div().pt(z(2.0)).child(crate::model_compare::pips(&st, i as i64, scale)), ph + 2.0));
        }
        if !chosen {
            let (b, _) = secondary_button("Use this one", 26.0, Act::Use(i), &format!("use-{}", m.id), scale);
            opt.push((b, 26.0));
        }
        let (mut o, oh) = opt.done();
        if i == state.live.as_ref().map_or(RECOMMENDED, |l| l.recommended) {
            // Rune scales the mark 0.85 from its top left: laid out at 0.85 here.
            o = o.child(at(with_zoom(0.85, || recommended_mark(scale)), 84.0, 0.0));
        }
        r = r.child(at(o, i as f32 * (190.0 + 18.0), 0.0));
        h = h.max(oh);
    }
    (r.relative().flex_none().w(z(190.0 * 2.0 + 18.0)).h(z(h)), h)
}

fn try_page(state: &SetupState, look: &Look) -> (Div, f32) {
    let (w, s) = (look.col, look.scale);
    let mut st = Stack::new(w, 12.0);
    st.push(headline(["Try", "✻ it once"], Some(tones_drawing(s)), w, s));
    if state.shortcut_taken {
        st.push(problem_line("another app took these keys", s));
        let shown: String = open_keys(state).iter().map(|(sym, word)| sym.unwrap_or(word)).collect();
        st.push(body_para(&format!("{shown} is in use by another app, so it can't open Iris. Pick other keys, then come back."), hue::I11, w, true));
        let (b, _) = primary("Change shortcut", None, Act::ChangeShortcut, s);
        st.push((b, 32.0));
    }
    st.push(key_line("Click in the box and press", &open_keys(state), None, false, w, s));
    st.push(practice_box(w, state.live.as_ref().and_then(|l| l.practice.as_ref()), s));
    st.push(key_line("In the panel, press", &RETURN_KEY, Some("to replace it."), false, w, s));
    if let Some(quick) = quick_keys(state) {
        st.push(key_line("Skip the panel with Quick Fix:", &quick, None, true, w, s));
    }
    st.done()
}

/// The practice box: the sample in an AppKit text view on a pen-cut well, "fix me" pointing
/// down at it from its top right corner. Returns it with the 18 points above it.
fn practice_box(w: f32, field: Option<&crate::text_field::Field>, scale: f32) -> (Div, f32) {
    // Text view: 5 points of line spacing, its text inset 5 past the 14 of padding.
    let inset = 14.0 + 5.0;
    let tw = w - 2.0 * inset - SCROLLER;
    let lines = wrap(PRACTICE, 12.5 * MONO_ADVANCE, tw, false);
    // A text view is flexible: the stack gives it its least height.
    let h = 84.0;
    let mut b = sized(w, h).child(at(hand_fill(85, 0.5, w, h, col(I3)), 0.0, 0.0));
    match field {
        // Live: the text you can type in, 20 points a line (its 15-point line and 5 of spacing,
        // the glyphs centred in it, so 2.5 higher than a 15-point line's top).
        Some(f) => {
            b = b.child(at(crate::text_field::area(f, "Practice text", tw, h - 28.0 + 2.5, body(), 20.0), inset, 14.0 + 0.5 - 2.5));
            // Where the box is, for the shortcut to read it.
            b = b.child(div().absolute().top_0().left_0().size_full().child(gpui::canvas(|bounds, _, _| PRACTICE_BOUNDS.set(Some(bounds)), |_, _, _, _| {}).size_full()));
        }
        None => {
            for (i, l) in lines.iter().enumerate() {
                b = b.child(at(t(l.clone(), body(), hue::I12), inset, 14.0 + 0.5 + i as f32 * 20.0));
            }
        }
    }
    // The text runs past the box, so AppKit shows its scroller: a pale track, a grey edge.
    let track = sized(SCROLLER, h - 28.0).bg(col((250, 250, 250)))
        .child(at(div().w(z(1.0)).h(z(h - 28.0)).bg(col((217, 216, 216))), 0.0, 0.0));
    b = b.child(at(track, w - 14.0 - SCROLLER, 14.0));
    let (note, nw, _) = hand_note("fix me", 20.0, 9, true, (26.0, 24.0), true, scale);
    b = b.child(at(note, w - 6.0 - nw, -30.0));
    let b = labelled(b, "setup-practice".into(), Role::TextInput, "Practice text");
    (sized(w, 18.0 + h).child(at(b, 0.0, 18.0)), 18.0 + h)
}

thread_local! {
    /// Where the practice box was last drawn, in its window.
    static PRACTICE_BOUNDS: std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
}

/// Where the practice box was last drawn, in its window.
pub fn practice_bounds() -> Option<gpui::Bounds<gpui::Pixels>> {
    PRACTICE_BOUNDS.get()
}

/// The sample the practice box starts with (`OnboardingModel.practiceSample`).
pub const PRACTICE_SAMPLE: &str = PRACTICE;

/// Room the text view's scroller keeps at the right.
const SCROLLER: f32 = 15.0;

fn done_page(state: &SetupState, look: &Look) -> (Div, f32) {
    let (w, s) = (look.col, look.scale);
    let mut st = Stack::new(w, 12.0);
    st.push(headline(["All", "✻ set"], None, w, s));
    if let Some(text) = &state.practice {
        let (words, wh) = body_para(text, hue::I12, w - 28.0, false);
        let h = wh + 28.0;
        st.push((sized(w, h).child(at(hand_fill(85, 0.5, w, h, col(I3)), 0.0, 0.0)).child(at(words, 14.0, 14.0)), h));
        let (note, _) = done_note("that's it", 26.0, s);
        st.push((note, line_box(26.0, BRUSH_LINE)));
    }
    st.push(key_line("Same keys in any app:", &open_keys(state), None, false, w, s));
    let (b, bw) = primary("Done", None, Act::Done, s);
    st.push((sized(bw, 36.0).child(at(b, 0.0, 4.0)), 36.0));
    st.done()
}

// MARK: - The step list

/// `StepList`: the three steps on pen rules: where you are and what comes next.
fn step_list(p: &Progress, scale: f32) -> (Div, f32) {
    let mut e = div();
    let mut y = 0.0;
    let title_h = brush_line_h(21.0, scale);
    for step in STEPS {
        let status = p.status(step);
        let strong = matches!(status, Status::Now | Status::Stuck);
        let n = step as i64;
        // The folio on the title's first baseline.
        let folio_y = brush_base(21.0, scale) - mono_base(11.0);
        let mut title = sized(LIST_W - 30.0, title_h);
        if strong {
            let tw = brush_w(step.title(), 21.0);
            let (m, pad) = marker_swipe(tw + 6.0, 7.0, 8 + n, 0.0, scale);
            title = title.child(at(m, -3.0, title_h - 9.0 - 2.0 - pad));
        }
        title = title.child(at(t(step.title(), brush_px(21.0, scale), if strong { hue::I12 } else { hue::I10 }), 0.0, 0.0));
        let (word, word_h) = status_word(status, step, scale);
        // SwiftUI sets the word under the title a little higher than its boxes say (measured:
        // a point for brush, half for mono), and the row half a point shorter.
        let gap = if status == Status::Later { 2.5 } else { 2.0 };
        let words = sized(LIST_W - 30.0, title_h + gap + word_h)
            .child(at(title, 0.0, 0.0))
            .child(at(word, 0.0, title_h + gap));
        let folio_w = mono_w("01", 11.0, 0.0);
        let row_h = 10.0 + title_h + 3.0 + word_h + 10.0 - if status == Status::Later { 1.0 } else { 1.5 };
        let row = sized(LIST_W, row_h)
            .child(at(t(format!("{n:02}"), caption(), ink::PINK_TEXT), 0.0, 10.0 + folio_y))
            .child(at(words, folio_w + 10.0, 10.0));
        e = e.child(at(labelled(row, format!("setup-step-{n}"), Role::Label, format!("Step {n}, {}", step.title())), 0.0, y));
        y += row_h;
        e = e.child(at(pen_rule(60 + n * 3, LIST_W, over(hue::I12, 0.3, ink::PAPER), 1.0), 0.0, y));
        y += 3.0;
    }
    (e.relative().flex_none().w(z(LIST_W)).h(z(y)), y)
}

fn status_word(status: Status, step: Step, scale: f32) -> (Div, f32) {
    let bh = brush_line_h(18.0, scale);
    match status {
        Status::Done => {
            let tick = drawn(13.0, 10.0, pen::hand_tick(13.0, 10.0), Some(2.0), col(ink::PINK_TEXT));
            (sized(LIST_W - 30.0, bh).child(at(tick, 0.0, (bh - 10.0) / 2.0)).child(at(t("done", brush_px(18.0, scale), ink::PINK_TEXT), 17.0, 0.0)), bh)
        }
        Status::Now => (t("now", brush_px(18.0, scale), ink::PINK_TEXT), bh),
        Status::Stuck => (t(step.stuck_word(), brush_px(18.0, scale), hue::RED_TEXT), bh),
        Status::Later => {
            let words = format!("AFTER {}", step as i32 - 1);
            (t(words, caption().tracked(1.3), hue::I10), 11.0 * MONO_LINE)
        }
    }
}

// MARK: - The foot

/// The big folio for the step; Set up later only on a stuck step; Open at login once there's
/// something to open. On one last baseline, 6 points under the page.
fn foot(p: &Progress, state: &SetupState, w: f32, scale: f32) -> (Div, f32) {
    let folio = format!("/0{}", p.current().map_or(3, |s| s as i32));
    // The folio's line on whole points (55.2 takes 56).
    let fh = line_box(46.0, MONO_LINE);
    let mut bottom = 6.0 + fh;
    let base = mono_base(46.0);
    let mut e = div().child(at(display(&folio, 46.0, scale).0, 0.0, 6.0));
    let mut right = w;
    if matches!(p.current(), Some(Step::TryIt) | None) {
        // The switch and its words on their centre line, the words' baseline on the folio's.
        let lw = mono_w("Open at login", 12.5, 0.0);
        let row_w = 50.0 + 10.0 + lw;
        let words_base = (26.0 - 15.0) / 2.0 + mono_base(12.5);
        let top = 6.0 + base - words_base - 0.5;
        e = e.child(at(sized(row_w, 26.0)
            .child(at(crate::controls::toggle(state.live.as_ref().is_some_and(|l| l.launch_at_login), "Open at login", scale, on(Act::LaunchAtLogin)), 0.0, 0.0))
            .child(at(t("Open at login", body(), hue::I11), 60.0, (26.0 - 15.0) / 2.0)), right - row_w, top));
        right -= row_w;
    }
    if p.offers_set_up_later() {
        let words = "Set up later";
        let bw = 24.0 + mono_w(words, 12.5, -0.12);
        let words_base = (32.0 - 15.0) / 2.0 + mono_base(12.5);
        let gap = if right < w { 8.0 } else { 0.0 };
        let top = 6.0 + base - words_base - 0.5;
        bottom = bottom.max(top + 32.0);
        e = e.child(at(crate::controls::button(&crate::controls::Button::ghost(words).id("setup-later"), scale, on(Act::Later)).0, right - gap - bw, top));
    }
    (e.relative().flex_none().w(z(w)).h(z(bottom)), bottom)
}
