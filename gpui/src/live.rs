//! The ⌃G moment, live: a see-through window the size of the scene in which the flower grows,
//! opens and the panel unrolls; click a tone (or Tab, 1–5 with `IRIS_TAKE_KEYS`), Replace or
//! Return puts the answer in, Escape puts it away; then the window goes.
//!
//! The flower is a CPU drawing (`rig.rs`, about 11 ms a frame in a release build). Drawn on the
//! main thread, as the Rune copy does, it would take most of each frame. Here two background
//! threads draw it a frame ahead (`RigFeed`), the main thread only shows the newest picture,
//! and once the flower has settled it's drawn once and kept.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::*;

use crate::model::{self, Phase};
use crate::opening::{self, Script};
use crate::riso::Img;
use crate::store::AnswerCache;

/// A picture of the flower drawn for a moment of the script.
struct Drawn {
    seq: u64,
    t: f64,
    img: Img,
    /// When it was asked for.
    asked: Instant,
}

struct Job {
    seq: u64,
    t: f64,
    script: Script,
    scale: f32,
    asked: Instant,
}

/// Draws the flower on background threads, a frame or two ahead of the screen.
pub struct RigFeed {
    jobs: Sender<Job>,
    done: Arc<Mutex<Option<Drawn>>>,
    in_flight: Arc<AtomicUsize>,
    threads: usize,
    seq: u64,
    shown: Option<Drawn>,
    /// How long a picture takes from asking to ready (smoothed), so it's asked for the moment
    /// it will be shown.
    lead: f64,
    /// The time between frames (smoothed), and when the last one was asked for.
    interval: f64,
    last_frame: Option<Instant>,
}

impl RigFeed {
    pub fn new(threads: usize) -> RigFeed {
        let threads = threads.max(1);
        let (jobs, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let done = Arc::new(Mutex::new(None::<Drawn>));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let trace = std::env::var("IRIS_TRACE").is_ok();
        for n in 0..threads {
            let (rx, done, in_flight): (Arc<Mutex<Receiver<Job>>>, _, _) = (rx.clone(), done.clone(), in_flight.clone());
            std::thread::Builder::new().name(format!("iris-rig-{n}")).spawn(move || loop {
                let job = match rx.lock().unwrap().recv() {
                    Ok(j) => j,
                    Err(_) => return,
                };
                let began = Instant::now();
                let (w, h, rgba) = opening::rig_rgba(job.t, &job.script, job.scale);
                let img = crate::riso::image(w, h, rgba);
                if trace {
                    eprintln!("rig draw_ms={:.2} thread={n}", began.elapsed().as_secs_f64() * 1000.0);
                }
                let mut d = done.lock().unwrap();
                if d.as_ref().is_none_or(|old| old.seq < job.seq) {
                    *d = Some(Drawn { seq: job.seq, t: job.t, img, asked: job.asked });
                }
                in_flight.fetch_sub(1, Ordering::SeqCst);
            }).unwrap();
        }
        RigFeed { jobs, done, in_flight, threads, seq: 0, shown: None, lead: 0.03, interval: 1.0 / 60.0, last_frame: None }
    }

    fn ask(&mut self, t: f64, script: &Script, scale: f32) {
        self.seq += 1;
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let _ = self.jobs.send(Job { seq: self.seq, t, script: *script, scale, asked: Instant::now() });
    }

    /// The picture to show at `t`. Fast transitions (arriving, blooming, leaving) keep frames on
    /// the way at display rate; while waiting/streaming after arrival, paces to 8 Hz (matching
    /// the riso line boil) or holds the settled bud still when `quiet` or `!waiting_active`.
    fn at(&mut self, t: f64, script: &Script, scale: f32, window: &mut Window, waiting_active: bool, quiet: bool) -> Option<Img> {
        let now = Instant::now();
        if let Some(last) = self.last_frame.replace(now) {
            let gap = (now - last).as_secs_f64();
            if gap < 0.1 {
                self.interval = self.interval * 0.9 + gap * 0.1;
            }
        }
        if let Some(d) = self.done.lock().unwrap().take()
            && self.shown.as_ref().is_none_or(|s| s.seq < d.seq)
        {
            let took = d.asked.elapsed().as_secs_f64();
            self.lead = self.lead * 0.8 + took * 0.2;
            // The old picture leaves GPUI's texture atlas, or every frame would stay in memory.
            if let Some(old) = self.shown.replace(d) {
                let _ = window.drop_image(old.img);
            }
        }
        if self.shown.is_none() {
            // The first picture is drawn here, so the window never opens without its flower.
            let (w, h, rgba) = opening::rig_rgba(t, script, scale);
            self.seq += 1;
            self.shown = Some(Drawn { seq: self.seq, t, img: crate::riso::image(w, h, rgba), asked: Instant::now() });
        }
        if opening::rig_active(t, script) {
            // Fast transitions (arrival, bloom, leaving): keep up with the display, capped by thread count.
            let want = ((self.lead / self.interval).ceil() as usize).clamp(1, self.threads.min(2));
            while self.in_flight.load(Ordering::SeqCst) < want {
                let ahead = self.lead + self.interval * self.in_flight.load(Ordering::SeqCst) as f64;
                self.ask(t + ahead, script, scale);
            }
        } else if script.bloom.is_finite() {
            // Bloom settled: draw the still open flower once.
            let still = script.bloom + 2.15;
            let have = self.shown.as_ref().is_some_and(|s| s.t >= still);
            if !have && self.in_flight.load(Ordering::SeqCst) == 0 {
                self.ask(still, script, scale);
            }
        } else if waiting_active && !quiet {
            // Waiting / streaming on a normal machine: gentle 8 Hz reading nod (matches 8 Hz boil)
            // on a single worker slot so we never starve the local model.
            let stale = self.shown.as_ref().is_none_or(|s| (t - s.t) >= 0.12);
            if stale && self.in_flight.load(Ordering::SeqCst) == 0 {
                self.ask(t, script, scale);
            }
        } else {
            // Quiet waiting (Reduce Motion / light mode) or failed check: draw the settled bud once.
            let still_bud = opening::arrival_done(script.full);
            let have = self.shown.as_ref().is_some_and(|s| s.t >= still_bud);
            if !have && self.in_flight.load(Ordering::SeqCst) == 0 {
                self.ask(still_bud, script, scale);
            }
        }
        self.shown.as_ref().map(|s| s.img.clone())
    }
}

/// Worker thread count for `RigFeed`: one thread on ≤ 4-core CPUs or in light mode so background
/// rasterization never competes with local CPU inference.
fn rig_threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    if cores <= 4 || crate::app::settings().light_mode_active() { 1 } else { 2 }
}

/// The live moment.
pub struct OpeningView {
    pub start: Instant,
    pub panel: model::Panel,
    /// When the panel went, in seconds into the moment, and whether by Replace.
    pub went: Option<(f64, bool)>,
    pub due: Option<Instant>,
    /// Answers already computed while this panel stays open, keyed by tone id.
    pub results: HashMap<String, model::Answer>,
    /// The tones this panel shows, in tab order (matches `panel.tones` 1-for-1).
    pub modes: Vec<crate::brain::Mode>,
    /// Generation counter for the 1.2 s "copied" flash so rapid ⌘C presses don't clear early.
    pub copied_gen: u64,
    pub focus: FocusHandle,
    pub paper_h: Rc<Cell<f32>>,
    pub feed: RigFeed,
    /// The check under way with the brain, when it's running (made-up answers otherwise).
    pub check: Option<i64>,
    /// The text box the panel is for (none: the stand-in draft).
    pub captured: Option<crate::textbridge::Captured>,
    /// Its controls' hover and press, as they move.
    pub anim: Rc<std::cell::RefCell<crate::interact::Anim>>,
    /// Where the drawing sits in its window: moved when the window was kept on the screen.
    pub shift: (f32, f32),
    /// When the shortcut that opened it was pressed, until the panel has started to unroll
    /// (then it's said, for tests and `IRIS_NEWS`: how long a press takes to show something).
    pub pressed: Option<Instant>,
    /// This arrival plays in full (`next_arrival`) or quick.
    pub full: bool,
    /// When the answer came and the flower opened, in seconds into the moment.
    pub bloomed: Option<f64>,
    /// When there was first something else to read with the flower shut: a failure, or the
    /// model waking up.
    pub readable: Option<f64>,
    /// How tall this window has been asked to be (it grows for a long answer, never shrinks).
    pub asked_h: f32,
    /// Whether an 8 Hz waiting tick is already scheduled (prevents 60 FPS spin while streaming).
    pub tick_scheduled: bool,
}

const MEMO_LIMIT: usize = 32;

/// In-memory LRU of recent answers (`PanelModel.memo`), kept only while Iris runs and never
/// written to disk. Keyed by everything an answer depends on (`memo_key`).
#[derive(Default)]
struct AnswerMemo {
    entries: HashMap<String, model::Answer>,
    order: VecDeque<String>,
}

impl AnswerMemo {
    fn get(&self, key: &str) -> Option<model::Answer> {
        self.entries.get(key).cloned()
    }

    fn put(&mut self, key: String, answer: model::Answer) {
        self.order.retain(|k| k != &key);
        self.order.push_back(key.clone());
        self.entries.insert(key, answer);
        while self.order.len() > MEMO_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }

    fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.order.retain(|k| k != key);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

/// Names one answer (`CheckSetup.key`): this draft, in this tone (with its instruction), under
/// this glossary, style rules and model.
fn memo_key(text: &str, mode: &crate::brain::Mode, glossary: &[String], style_rules: &[String], model_id: &str) -> String {
    [
        mode.id.as_str(),
        mode.instruction.as_str(),
        &glossary.join("\x01"),
        &style_rules.join("\x01"),
        model_id,
        text,
    ]
    .join("\x02")
}

/// Drops all remembered cross-panel answers (called when Remember answers leaves Recent).
pub fn clear_memo() {
    MEMO.with(|m| m.borrow_mut().clear());
}

thread_local! {
    /// When the open or Quick Fix shortcut was last pressed (`pressed`).
    static PRESSED: Cell<Option<Instant>> = const { Cell::new(None) };
    /// The panel that's open, for a second press of the shortcut and for tests' keys.
    static OPEN: std::cell::RefCell<Option<WindowHandle<OpeningView>>> = const { std::cell::RefCell::new(None) };
    /// Recent answers across panels (up to 32), used when `Settings.answer_cache == Recent`.
    static MEMO: RefCell<AnswerMemo> = RefCell::new(AnswerMemo::default());
}

/// Whether this arrival of the flower plays in full (`BudShow.next`), counted on this Mac: the
/// first five ever, the first each day, and now and then after (`opening::arrival_is_full`).
/// An Iris that moved over from the Swift app carries on from that app's count.
pub fn next_arrival() -> bool {
    let s = crate::app::settings();
    if s.light_mode_active() {
        return false;
    }
    let (count, last_day) = match s.bud_arrivals {
        Some(n) => (n, s.bud_last_day),
        None => {
            let (n, day) = crate::store::swift_bud_arrivals();
            (n.unwrap_or(0), day)
        }
    };
    let today = local_midnight();
    let first_today = last_day != Some(today);
    crate::app::change(|s| {
        s.bud_arrivals = Some(count.saturating_add(1));
        s.bud_last_day = Some(today);
    });
    opening::arrival_is_full(count, first_today, fastrand::u32(0..opening::ARRIVAL_NOW_AND_THEN))
}

/// The start of today here, in seconds since 1970 (`Calendar.current.startOfDay`).
fn local_midnight() -> f64 {
    use chrono::{Local, TimeZone};
    let now = Local::now();
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).expect("midnight");
    // A day that starts in a clock change has no 00:00; its first moment is the next one.
    Local.from_local_datetime(&midnight).earliest().map_or(now.timestamp(), |m| m.timestamp()) as f64
}

/// A shortcut was pressed: the panel it opens says how long it took to unroll.
pub fn pressed() {
    PRESSED.with(|p| p.set(Some(Instant::now())));
}

/// The open panel, if there is one.
pub fn open_panel() -> Option<WindowHandle<OpeningView>> {
    OPEN.with(|o| o.borrow().clone())
}

pub fn set_open_panel(h: Option<WindowHandle<OpeningView>>) {
    OPEN.with(|o| *o.borrow_mut() = h);
}

/// A key pressed in the open panel (tab, shift-tab, 1…5, return, escape, cmd-r), as tests send
/// it. False when there's no panel to take it.
pub fn key(name: &str, cx: &mut App) -> bool {
    let Some(h) = open_panel() else { return false };
    h.update(cx, |v, window, cx| v.key(name, window, cx)).is_ok()
}

/// Tells a test what happened ("iris: panel open", "iris: replaced …"), on stderr.
pub fn news(s: &str) {
    static NEWS_ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // Under test, or asked for (`IRIS_NEWS=1`, a person's run with the steps written down).
    if crate::textbridge::under_test() || *NEWS_ENV.get_or_init(|| std::env::var("IRIS_NEWS").is_ok()) {
        eprintln!("iris: {s}");
    }
}

impl OpeningView {
    pub fn new(cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        let modes = current_modes(cx);
        v.panel.tones = modes.iter().map(|m| m.label.clone()).collect();
        v.modes = modes;
        // With the brain running, the panel opens on the real answer for its draft; without
        // it, on the stand-in's, the flower opening as the design frames have it.
        if crate::brain::get(cx).is_some() {
            v.ask(0, false, cx);
        } else {
            v.bloomed = Some(opening::FULL.bloom);
        }
        v
    }

    fn new_blank(cx: &mut Context<Self>) -> OpeningView {
        OpeningView {
            start: Instant::now(),
            panel: opening::panel_state(),
            went: None,
            due: None,
            results: HashMap::new(),
            modes: vec![crate::brain::Mode::fix()],
            copied_gen: 0,
            focus: cx.focus_handle(),
            paper_h: Rc::new(Cell::new(173.0)),
            feed: RigFeed::new(rig_threads()),
            check: None,
            captured: None,
            anim: Default::default(),
            shift: (0.0, 0.0),
            pressed: PRESSED.with(|p| p.take()),
            full: true,
            bloomed: None,
            readable: None,
            asked_h: 0.0,
            tick_scheduled: false,
        }
    }

    /// The panel for a text box that was read: its words, its app, the tone it starts on.
    pub fn for_captured(captured: crate::textbridge::Captured, starting: usize, cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        let modes = current_modes(cx);
        let starting = starting.min(modes.len().saturating_sub(1));
        v.panel.tones = modes.iter().map(|m| m.label.clone()).collect();
        v.modes = modes;
        v.panel.original = captured.text.clone();
        v.panel.app_name = captured.app_name.clone();
        v.panel.app_bundle = captured.bundle_id.clone();
        v.panel.can_replace = captured.can_replace;
        v.panel.selected = starting;
        v.captured = Some(captured);
        v.full = next_arrival();
        v.ask(starting, false, cx);
        v
    }

    /// The panel for a Quick Fix a person should look at: its answer already in.
    pub fn with_answer(captured: crate::textbridge::Captured, answer: model::Answer, cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        let modes = current_modes(cx);
        v.panel.tones = modes.iter().map(|m| m.label.clone()).collect();
        v.modes = modes;
        v.panel.original = captured.text.clone();
        v.panel.app_name = captured.app_name.clone();
        v.panel.app_bundle = captured.bundle_id.clone();
        v.panel.can_replace = captured.can_replace;
        v.panel.selected = 0;
        let s = crate::app::settings();
        if s.answer_cache != AnswerCache::Off {
            v.results.insert("fix".into(), answer.clone());
            if s.answer_cache == AnswerCache::Recent {
                let model_id = crate::brain::get(cx)
                    .and_then(|b| b.read(cx).chosen.clone())
                    .or_else(|| s.model.clone())
                    .unwrap_or_default();
                let key = memo_key(&captured.text, &crate::brain::Mode::fix(), &s.glossary, &s.style_rules, &model_id);
                MEMO.with(|m| m.borrow_mut().put(key, answer.clone()));
            }
        }
        v.panel.phase = Phase::Done(answer);
        v.captured = Some(captured);
        v.full = next_arrival();
        v.bloomed = Some(0.0);
        v
    }

    /// Asks the brain for tone `tone` on the draft; uses the per-panel or recent memory cache
    /// when enabled (bypassed on `retry` / ⌘R), otherwise waits while the answer streams in.
    fn ask(&mut self, tone: usize, retry: bool, cx: &mut Context<Self>) {
        // A new answer coming: "copied" was about the last one (`clearCopiedFlash`).
        self.panel.copied = false;
        self.panel.retry_note = None;
        let Some(brain) = crate::brain::get(cx) else { return };
        if let Some(old) = self.check.take() {
            crate::brain::cancel(old);
        }
        let b = brain.read(cx);
        let mode = self.modes.get(tone).cloned().or_else(|| b.tones.get(tone).cloned()).unwrap_or_else(crate::brain::Mode::fix);
        let s = crate::app::settings();
        let model_id = b.chosen.as_deref().or(s.model.as_deref()).unwrap_or("");
        let key = memo_key(&self.panel.original, &mode, &s.glossary, &s.style_rules, model_id);
        let policy = s.answer_cache;
        if retry {
            self.results.remove(&mode.id);
            MEMO.with(|m| m.borrow_mut().remove(&key));
        } else {
            let cached = match policy {
                AnswerCache::Off => None,
                AnswerCache::WhileOpen => self.results.get(&mode.id).cloned(),
                AnswerCache::Recent => self.results.get(&mode.id).cloned().or_else(|| MEMO.with(|m| m.borrow().get(&key))),
            };
            if let Some(answer) = cached {
                if policy != AnswerCache::Off {
                    self.results.insert(mode.id.clone(), answer.clone());
                }
                self.panel.phase = Phase::Done(answer);
                if self.bloomed.is_none() {
                    self.bloomed = Some(self.t());
                }
                return;
            }
        }
        self.panel.phase = if matches!(b.engine, crate::brain::Engine::Ready(_)) { Phase::Generating(String::new()) } else { Phase::Preparing };
        if matches!(self.panel.phase, Phase::Preparing) {
            // The panel explains the wait (downloading, waking up).
            self.read_now();
        }
        let mode_id = mode.id.clone();
        let this = cx.entity().downgrade();
        let id = crate::brain::check(&self.panel.original, &mode, retry, move |event, cx| {
            let _ = this.update(cx, |v, cx| {
                match event {
                    crate::brain::Event::Partial(p) => v.panel.phase = Phase::Generating(p),
                    crate::brain::Event::Finished(a, _) => {
                        let policy = crate::app::settings().answer_cache;
                        if policy != AnswerCache::Off {
                            v.results.insert(mode_id.clone(), a.clone());
                        }
                        if policy == AnswerCache::Recent {
                            MEMO.with(|m| m.borrow_mut().put(key.clone(), a.clone()));
                        }
                        v.panel.phase = Phase::Done(a);
                        v.check = None;
                        // The answer is in: the flower opens (once: a later tone doesn't open
                        // it again), and the panel follows.
                        if v.bloomed.is_none() {
                            v.bloomed = Some(v.t());
                        }
                    }
                    crate::brain::Event::Failed(p) => {
                        v.panel.phase = Phase::Failed(model::Failure { note: p.note.into(), can_retry: p.can_retry });
                        v.check = None;
                        // A failed check leaves the flower shut; the panel says what's wrong.
                        v.read_now();
                    }
                }
                cx.notify();
            });
        });
        self.check = Some(id);
    }

    fn t(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Something to read with the flower shut, now (the first time counts).
    fn read_now(&mut self) {
        if self.readable.is_none() {
            self.readable = Some(self.t());
        }
    }

    /// The moment so far: the flower opens when the answer comes and the panel unrolls when
    /// there's something to read (`opening::unfold_at`), as the Swift app's bud and panel do.
    fn script(&self) -> Script {
        let (replace, replaced) = self.went.unwrap_or((f64::INFINITY, false));
        Script {
            full: self.full,
            bloom: self.bloomed.unwrap_or(f64::INFINITY),
            readable: self.readable.unwrap_or(f64::INFINITY),
            replace,
            replaced,
            backdrop: false,
        }
    }

    fn open(&self) -> bool {
        let (u, _) = opening::unroll(self.t(), &self.script());
        self.went.is_none() && u > 0.5
    }

    fn pick(&mut self, tone: usize, cx: &mut Context<Self>) {
        if !self.open() || tone >= self.panel.tones.len().max(1) || (tone == self.panel.selected && self.due.is_none()) {
            return;
        }
        self.panel.selected = tone;
        if crate::brain::get(cx).is_some() {
            self.ask(tone, false, cx);
            cx.notify();
            return;
        }
        let key = tone.to_string();
        if crate::app::settings().answer_cache != AnswerCache::Off
            && let Some(cached) = self.results.get(&key).cloned()
        {
            self.due = None;
            self.panel.phase = Phase::Done(cached);
            cx.notify();
            return;
        }
        self.panel.phase = Phase::Generating(String::new());
        let due = Instant::now() + Duration::from_millis(350);
        self.due = Some(due);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(350)).await;
            let _ = this.update(cx, |v, cx| {
                if v.due == Some(due) {
                    v.due = None;
                    let answer = opening::answer(v.panel.selected);
                    if crate::app::settings().answer_cache != AnswerCache::Off {
                        v.results.insert(v.panel.selected.to_string(), answer.clone());
                    }
                    v.panel.phase = Phase::Done(answer);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Replace (the panel closes at once, a petal stamps the box) or put away (it folds back
    /// into the flower); the flower folds and sinks, and the window goes.
    /// Once the flower has sunk, the window goes (the app stays: in `--menubar` the flower and
    /// the slip live on; `--live-panel` quits when its only window goes).
    fn go(&mut self, replaced: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.went.is_some() || (replaced && self.due.is_some()) {
            return;
        }
        self.went = Some((self.t(), replaced));
        // Clear the active-panel slot as soon as closing starts so a quick follow-up ⌃⌥G
        // opens a fresh panel instead of trying to dismiss this wilting window.
        if open_panel().is_some_and(|h| h.window_id() == window.window_handle().window_id()) {
            set_open_panel(None);
        }
        if let Some(c) = self.check.take() {
            crate::brain::cancel(c);
        }
        if replaced
            && let (Some(captured), Phase::Done(answer)) = (self.captured.clone(), &self.panel.phase)
        {
            crate::flow::replace_and_say(captured, answer.text.clone(), answer.keep.clone(), crate::flow::Said::Replaced, cx);
        }
        news(if replaced { "panel replaced" } else { "panel put away" });
        // Replaced: the paste's keys must reach the app, not the panel (`close(immediately:)`).
        // Swift's panel leaves at once and its bud wilts in a window of its own; GPUI draws both
        // in one window, so that window lets go of the keyboard instead (taken off screen and
        // straight back, without it) and the flower wilts on in it.
        if replaced {
            crate::give_up_keys(window);
        }
        let wait = if replaced { 1.6 } else { 0.8 };
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_secs_f64(wait)).await;
            let _ = cx.update(|window, _| window.remove_window());
        })
        .detach();
        cx.notify();
    }
}

impl OpeningView {
    /// Return, or the footer's button (`PanelModel.accept`): what the button says. Replace;
    /// Copy when the text can't be replaced (a selection in a page): copied, the panel goes and
    /// says so by the text; Done when there's nothing to fix; Try again after a failure.
    fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.went.is_some() {
            return;
        }
        match self.panel.footer().action {
            model::Action::Replace => self.go(true, window, cx),
            model::Action::Done => self.go(false, window, cx),
            model::Action::Copy => {
                let Phase::Done(answer) = &self.panel.phase else { return };
                let text = answer.text.clone();
                put_on_clipboard(&text);
                let frame = self.captured.as_ref().and_then(|c| c.field_frame);
                self.go(false, window, cx);
                crate::notes_live::show(crate::toast::Message::copied(), None, frame, cx);
            }
            model::Action::TryAgain => {
                let tone = self.panel.selected;
                self.ask(tone, true, cx);
                cx.notify();
            }
            _ => {}
        }
    }

    /// ⌘C (`PanelModel.copy`): the answer on the clipboard, "copied" in the footer for a moment.
    fn copy(&mut self, cx: &mut Context<Self>) {
        if self.went.is_some() {
            return;
        }
        let Phase::Done(answer) = &self.panel.phase else { return };
        put_on_clipboard(&answer.text.clone());
        news("panel copied");
        self.panel.copied = true;
        self.copied_gen = self.copied_gen.wrapping_add(1);
        let gen_id = self.copied_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs_f64(1.2)).await;
            let _ = this.update(cx, |v, cx| {
                if v.copied_gen == gen_id {
                    v.panel.copied = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// A key in the panel (`PanelController.handle`): Tab, ⇧Tab, Left/Right and 1–6 move through
    /// the tones, Return replaces, Escape puts it away, ⌘R tries again, ⌘C copies.
    pub fn key(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.went.is_some() {
            return false;
        }
        let n = self.panel.tones.len().max(1);
        match name {
            "tab" | "right" => self.pick((self.panel.selected + 1) % n, cx),
            "shift-tab" | "left" => self.pick((self.panel.selected + n - 1) % n, cx),
            "1" | "2" | "3" | "4" | "5" | "6" => {
                let idx = name.parse::<usize>().unwrap() - 1;
                if idx < n {
                    self.pick(idx, cx);
                }
            }
            "enter" | "return" => self.accept(window, cx),
            // A long answer scrolls a line at a time (← → are the tones).
            "up" | "down" => {
                crate::panel::scroll_answer(&self.panel, if name == "up" { -22.0 } else { 22.0 });
                cx.notify();
            }
            "cmd-c" => self.copy(cx),
            "escape" => self.go(false, window, cx),
            "cmd-r" => {
                let tone = self.panel.selected;
                self.ask(tone, true, cx);
                cx.notify();
            }
            _ => return false,
        }
        true
    }
}

/// The tones the panel should display right now: from settings (or `mock::tones()` under mock mode),
/// always keeping Fix first and never returning an empty list.
pub fn current_modes(cx: &App) -> Vec<crate::brain::Mode> {
    if crate::mock::on() {
        if let Some(b) = crate::brain::get(cx) {
            let tones = b.read(cx).tones.clone();
            if !tones.is_empty() {
                return tones;
            }
        }
        return crate::mock::tones();
    }
    let shown = crate::tones::ToneList::from_settings(&crate::app::settings()).shown();
    if shown.is_empty() { vec![crate::brain::Mode::fix()] } else { shown }
}

impl Render for OpeningView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame_began = Instant::now();
        crate::riso::let_go_of_last_frame(window);
        let t = self.t();
        let script = self.script();
        let scale = window.scale_factor();
        let reduce = crate::live_slip::reduce_motion();
        let quiet = reduce || crate::app::settings().light_mode_active();
        let working = matches!(self.panel.phase, Phase::Preparing | Phase::Generating(_));
        let rig = self.feed.at(t, &script, scale, window, working && self.went.is_none(), quiet);
        if self.went.is_none() && opening::unroll(t, &script).0 > 0.0
            && let Some(at) = self.pressed.take()
        {
            news(&format!("panel unrolls {} ms after the press ({} ms after its window)", at.elapsed().as_millis(), (t * 1000.0) as u64));
        }
        let entity = cx.entity();
        let pick_fn = {
            let e = entity.clone();
            move |i: usize| -> crate::interact::Act {
                let e = e.clone();
                crate::interact::act(move |_, cx| e.update(cx, |v, cx| v.pick(i, cx)))
            }
        };
        let act = {
            let e = entity.clone();
            crate::interact::act(move |window, cx| e.update(cx, |v, cx| v.accept(window, cx)))
        };
        let look = crate::panel::Look { scale, icon: &crate::app_icon, pick: Some(&pick_fn), act: Some(act) };
        self.room(window, cx);
        let anim = self.anim.clone();
        let (panel, paper_h) = (self.panel.clone(), self.paper_h.clone());
        let body = crate::interact::build(&anim, reduce, window, cx, || opening::scene(t, &script, &panel, &look, paper_h.get(), paper_h.clone(), rig.as_ref()));

        // Drive fast transitions (arrival, unroll, bloom, going) at full frame rate. Once the
        // panel is open and waiting/streaming, never spin at 60 FPS: pace waiting motion to 8 Hz
        // (matching the riso line boil) on normal machines, or stay event-driven (0 Hz idle) under
        // Reduce Motion / "Go easy on this Mac / PC" so incoming tokens and height springs still
        // animate smoothly without starving local inference.
        let rig_active = opening::rig_active(t, &script);
        let unrolling = opening::is_unrolling(t, &script);
        let waiting_still_flower = !rig_active && script.bloom.is_finite()
            && self.feed.shown.as_ref().is_none_or(|s| s.t < script.bloom + 2.15);
        let waiting_still_bud = !rig_active && !script.bloom.is_finite() && (quiet || !working)
            && self.feed.shown.as_ref().is_none_or(|s| s.t < opening::arrival_done(script.full));
        if rig_active || unrolling || self.went.is_some() || waiting_still_flower || waiting_still_bud {
            window.request_animation_frame();
        } else if working && !quiet && !self.tick_scheduled {
            self.tick_scheduled = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(125)).await;
                let _ = this.update(cx, |v, cx| {
                    v.tick_scheduled = false;
                    if matches!(v.panel.phase, Phase::Preparing | Phase::Generating(_)) && v.went.is_none() {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        static TRACE_ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let trace = *TRACE_ENV.get_or_init(|| std::env::var("IRIS_TRACE").is_ok());
        let done = canvas(|_, _, _| (), move |_, _, _, _| {
            if trace {
                eprintln!("frame cpu_ms={:.3} t={t:.3}", frame_began.elapsed().as_secs_f64() * 1000.0);
            }
        }).absolute().size_0();
        div().size_full().track_focus(&self.focus)
            .on_key_down(cx.listener(|v, e: &KeyDownEvent, window, cx| {
                let k = &e.keystroke;
                let name = match (k.key.as_str(), k.modifiers.shift, k.modifiers.platform) {
                    ("tab", true, _) => "shift-tab".to_string(),
                    ("r", _, true) => "cmd-r".to_string(),
                    ("c", _, true) => "cmd-c".to_string(),
                    ("left" | "right", _, true) => String::new(),
                    (key, _, _) => key.to_string(),
                };
                // Handled: said so, or GPUI hands the same press on by its other routes (macOS
                // delivers a key more than one way) and it counts two or three times.
                if v.key(&name, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(div().relative().left(px(self.shift.0)).top(px(self.shift.1)).child(body))
            .child(done)
    }
}

impl OpeningView {
    /// The room this window gives the paper, read from its size now (so the drawing always
    /// matches the window it's in), and, when the answer is about to outgrow it, a taller window:
    /// grown towards the paper's open side, as far as the screen allows, once per need. Past the
    /// screen's edge the paper slides over the box instead (`opening::place`), as Swift's did.
    fn room(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let b = window.bounds();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        let grown = (h - opening::SCENE.1).max(0.0);
        let (ox, oy) = crate::window_origin(window).unwrap_or((f32::from(b.origin.x) as f64, f32::from(b.origin.y) as f64));
        let [_, vy, _, vh] = crate::visible_frame_at(ox + w as f64 / 2.0, oy + h as f64 / 2.0);
        let (oy, vy, vh) = (oy as f32, vy as f32, vh as f32);
        let below = opening::below();
        let sy = self.shift.1;
        crate::panel::set_max_body(crate::panel::max_body(vh));
        opening::set_room(opening::Room {
            above: if below { 0.0 } else { grown },
            below: if below { grown } else { 0.0 },
            top: (vy + 8.0).max(oy) - oy - sy,
            bottom: (vy + vh - 8.0).min(oy + h) - oy - sy,
        });
        if self.went.is_some() {
            return;
        }
        let chrome = self.anim.borrow().panel_chrome_h.unwrap_or(crate::panel::CHROME);
        let paper = chrome + crate::panel::body_height(&self.panel);
        let Some((up, down)) = opening::more_room(paper, sy, grown) else { return };
        let up = up.min((oy - vy).max(0.0));
        let down = down.min((vy + vh - oy - h).max(0.0));
        let want = h + up + down;
        if up + down < 1.0 || want <= self.asked_h + 0.5 {
            return;
        }
        self.asked_h = want;
        news(&format!("panel window grows {up:.0} up, {down:.0} down"));
        crate::grow_window(window, up as f64, down as f64, cx);
    }
}

/// The person's clipboard gets `text` (`TextBridge.copyToClipboard`); under test a clipboard of
/// the test's own, never the Mac's.
fn put_on_clipboard(text: &str) {
    if crate::textbridge::under_test() {
        crate::text_field::set_test_clipboard(text);
    } else {
        crate::textbridge::put_on_clipboard(text, false);
    }
}

#[cfg(test)]
mod tests {
    use super::{AnswerMemo, MEMO_LIMIT, memo_key};
    use crate::brain::Mode;
    use crate::model::Answer;

    #[test]
    fn answer_memo_lru_cap_and_remove() {
        let mut memo = AnswerMemo::default();
        for i in 0..(MEMO_LIMIT + 4) {
            memo.put(format!("k{i}"), Answer::tone("a", &format!("v{i}"), false));
        }
        assert_eq!(memo.entries.len(), MEMO_LIMIT);
        assert!(memo.get("k0").is_none() && memo.get("k3").is_none());
        assert_eq!(memo.get("k4").unwrap().text, "v4");
        // Re-putting an existing key refreshes its recency without growing past the cap.
        memo.put("k4".into(), Answer::tone("a", "v4-fresh", false));
        memo.put("k-new".into(), Answer::tone("a", "v-new", false));
        assert_eq!(memo.entries.len(), MEMO_LIMIT);
        assert_eq!(memo.get("k4").unwrap().text, "v4-fresh");
        assert!(memo.get("k5").is_none());
        memo.remove("k4");
        assert!(memo.get("k4").is_none());
        memo.clear();
        assert!(memo.entries.is_empty());
    }

    #[test]
    fn memo_key_separates_every_answer_input() {
        let fix = Mode::fix();
        let custom = Mode { id: "custom-1".into(), label: "Pirate".into(), instruction: "talk like a pirate".into() };
        let custom_edited = Mode { instruction: "keep it subtle".into(), ..custom.clone() };
        let base = memo_key("hello", &fix, &[], &[], "best");
        assert_ne!(base, memo_key("hello!", &fix, &[], &[], "best"));
        assert_ne!(base, memo_key("hello", &custom, &[], &[], "best"));
        assert_ne!(memo_key("hello", &custom, &[], &[], "best"), memo_key("hello", &custom_edited, &[], &[], "best"));
        assert_ne!(base, memo_key("hello", &fix, &["Iris".into()], &[], "best"));
        assert_ne!(base, memo_key("hello", &fix, &[], &["keep it brief".into()], "best"));
        assert_ne!(base, memo_key("hello", &fix, &[], &[], "lighter"));
    }
}
