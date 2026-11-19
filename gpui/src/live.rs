//! The ⌃G moment, live: a see-through window the size of the scene in which the flower grows,
//! opens and the panel unrolls; click a tone (or Tab, 1–5 with `IRIS_TAKE_KEYS`), Replace or
//! Return puts the answer in, Escape puts it away; then the window goes.
//!
//! The flower is a CPU drawing (`rig.rs`, about 11 ms a frame in a release build). Drawn on the
//! main thread, as the Rune copy does, it would take most of each frame. Here two background
//! threads draw it a frame ahead (`RigFeed`), the main thread only shows the newest picture,
//! and once the flower has settled it's drawn once and kept.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::*;

use crate::model::{self, Phase};
use crate::opening::{self, Script};
use crate::riso::Img;

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
        RigFeed { jobs, done, in_flight, seq: 0, shown: None, lead: 0.03, interval: 1.0 / 60.0, last_frame: None }
    }

    fn ask(&mut self, t: f64, script: &Script, scale: f32) {
        self.seq += 1;
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let _ = self.jobs.send(Job { seq: self.seq, t, script: *script, scale, asked: Instant::now() });
    }

    /// The picture to show at `t`. While the flower moves, keeps two pictures on the way, each
    /// asked for the moment it'll be ready; once it's still, asks for the still pose once.
    fn at(&mut self, t: f64, script: &Script, scale: f32, window: &mut Window) -> Option<Img> {
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
        if opening::rig_moving(t, script) {
            // As many pictures on the way as it takes to have one each frame: one at 60 Hz (a
            // picture takes about 11 ms), two at 120.
            let want = ((self.lead / self.interval).ceil() as usize).clamp(1, 2);
            while self.in_flight.load(Ordering::SeqCst) < want {
                let ahead = self.lead + self.interval * self.in_flight.load(Ordering::SeqCst) as f64;
                self.ask(t + ahead, script, scale);
            }
        } else {
            // Still: the settled pose, drawn once.
            let still = script.bloom + 2.15;
            let have = self.shown.as_ref().is_some_and(|s| s.t >= still);
            if !have && self.in_flight.load(Ordering::SeqCst) == 0 {
                self.ask(still, script, scale);
            }
        }
        self.shown.as_ref().map(|s| s.img.clone())
    }
}

/// The live moment.
pub struct OpeningView {
    pub start: Instant,
    pub panel: model::Panel,
    /// When the panel went, in seconds into the moment, and whether by Replace.
    pub went: Option<(f64, bool)>,
    pub due: Option<Instant>,
    pub flight: Option<(usize, usize)>,
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
}

thread_local! {
    /// The panel that's open, for a second press of the shortcut and for tests' keys.
    static OPEN: std::cell::RefCell<Option<WindowHandle<OpeningView>>> = const { std::cell::RefCell::new(None) };
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
    // Under test, or asked for (`IRIS_NEWS=1`, a person's run with the steps written down).
    if crate::textbridge::under_test() || std::env::var("IRIS_NEWS").is_ok() {
        eprintln!("iris: {s}");
    }
}

impl OpeningView {
    pub fn new(cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        // With the brain running, the panel opens on the real answer for its draft.
        if crate::brain::get(cx).is_some() {
            v.ask(0, false, cx);
        }
        v
    }

    fn new_blank(cx: &mut Context<Self>) -> OpeningView {
        OpeningView {
            start: Instant::now(),
            panel: opening::panel_state(),
            went: None,
            due: None,
            flight: None,
            focus: cx.focus_handle(),
            paper_h: Rc::new(Cell::new(173.0)),
            feed: RigFeed::new(2),
            check: None,
            captured: None,
            anim: Default::default(),
            shift: (0.0, 0.0),
        }
    }

    /// The panel for a text box that was read: its words, its app, the tone it starts on.
    pub fn for_captured(captured: crate::textbridge::Captured, starting: usize, cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        v.panel.original = captured.text.clone();
        v.panel.app_name = captured.app_name.clone();
        v.panel.app_bundle = captured.bundle_id.clone();
        v.panel.can_replace = captured.can_replace;
        v.panel.selected = starting;
        v.captured = Some(captured);
        v.ask(starting, false, cx);
        v
    }

    /// The panel for a Quick Fix a person should look at: its answer already in.
    pub fn with_answer(captured: crate::textbridge::Captured, answer: model::Answer, cx: &mut Context<Self>) -> OpeningView {
        let mut v = OpeningView::new_blank(cx);
        v.panel.original = captured.text.clone();
        v.panel.app_name = captured.app_name.clone();
        v.panel.app_bundle = captured.bundle_id.clone();
        v.panel.can_replace = captured.can_replace;
        v.panel.selected = 0;
        v.panel.phase = Phase::Done(answer);
        v.captured = Some(captured);
        v
    }

    /// Asks the brain for tone `tone` on the draft; the panel waits (waking the model first
    /// if it isn't in memory) and the answer streams in.
    fn ask(&mut self, tone: usize, retry: bool, cx: &mut Context<Self>) {
        // A new answer coming: "copied" was about the last one (`clearCopiedFlash`).
        self.panel.copied = false;
        let Some(brain) = crate::brain::get(cx) else { return };
        if let Some(old) = self.check.take() {
            crate::brain::cancel(old);
        }
        let b = brain.read(cx);
        let mode = b.tones.get(tone).cloned().unwrap_or_else(crate::brain::Mode::fix);
        self.panel.phase = if matches!(b.engine, crate::brain::Engine::Ready(_)) { Phase::Generating(String::new()) } else { Phase::Preparing };
        let this = cx.entity().downgrade();
        let id = crate::brain::check(&self.panel.original, &mode, retry, move |event, cx| {
            let _ = this.update(cx, |v, cx| {
                match event {
                    crate::brain::Event::Partial(p) => v.panel.phase = Phase::Generating(p),
                    crate::brain::Event::Finished(a, _) => {
                        v.panel.phase = Phase::Done(a);
                        v.check = None;
                    }
                    crate::brain::Event::Failed(p) => {
                        v.panel.phase = Phase::Failed(model::Failure { note: p.note.into(), can_retry: p.can_retry });
                        v.check = None;
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

    fn script(&self) -> Script {
        let (replace, replaced) = self.went.unwrap_or((f64::INFINITY, false));
        Script { full: true, bloom: 0.9, replace, replaced, backdrop: false }
    }

    fn open(&self) -> bool {
        let (u, _) = opening::unroll(self.t(), &self.script());
        self.went.is_none() && u > 0.5
    }

    fn pick(&mut self, tone: usize, cx: &mut Context<Self>) {
        if !self.open() || (tone == self.panel.selected && self.due.is_none()) {
            return;
        }
        let moves = self.flight.map_or(0, |f| f.1) + 1;
        self.flight = Some((self.panel.selected, moves));
        self.panel.selected = tone;
        if crate::brain::get(cx).is_some() {
            self.ask(tone, false, cx);
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
                    v.panel.phase = Phase::Done(opening::answer(v.panel.selected));
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
        let Phase::Done(answer) = &self.panel.phase else { return };
        put_on_clipboard(&answer.text.clone());
        news("panel copied");
        self.panel.copied = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs_f64(1.2)).await;
            let _ = this.update(cx, |v, cx| {
                v.panel.copied = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// A key in the panel (`PanelController.handle`): Tab and ⇧Tab move through the tones,
    /// 1–5 pick one, Return replaces, Escape puts it away, ⌘R tries again.
    pub fn key(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let n = model::TONES.len();
        match name {
            "tab" => self.pick((self.panel.selected + 1) % n, cx),
            "shift-tab" => self.pick((self.panel.selected + n - 1) % n, cx),
            "1" | "2" | "3" | "4" | "5" => self.pick(name.parse::<usize>().unwrap() - 1, cx),
            "enter" | "return" => self.accept(window, cx),
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

impl Render for OpeningView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame_began = Instant::now();
        let t = self.t();
        let script = self.script();
        let scale = window.scale_factor();
        let rig = self.feed.at(t, &script, scale, window);
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
        let look = crate::panel::Look { scale, icon: &crate::app_icon, pick: Some(&pick_fn), flight: self.flight, act: Some(act) };
        let anim = self.anim.clone();
        let (panel, paper_h) = (self.panel.clone(), self.paper_h.clone());
        let body = crate::interact::build(&anim, false, window, cx, || opening::scene(t, &script, &panel, &look, paper_h.get(), paper_h.clone(), rig.as_ref()));

        // Keep drawing while anything moves: the flower, the unroll, the waiting dots, the going.
        let working = matches!(self.panel.phase, Phase::Generating(_));
        if opening::rig_moving(t, &script) || t < 3.2 || working || self.went.is_some() {
            window.request_animation_frame();
        } else if self.feed.shown.as_ref().is_none_or(|s| s.t < script.bloom + 2.15) {
            // The still flower is on its way.
            window.request_animation_frame();
        }
        let trace = std::env::var("IRIS_TRACE").is_ok();
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

/// The person's clipboard gets `text` (`TextBridge.copyToClipboard`); under test a clipboard of
/// the test's own, never the Mac's.
fn put_on_clipboard(text: &str) {
    if crate::textbridge::under_test() {
        crate::text_field::set_test_clipboard(text);
    } else {
        crate::textbridge::put_on_clipboard(text, false);
    }
}
