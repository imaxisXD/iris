//! The menu bar slip, live: hanging from the flower in the menu bar (`--menubar`), or on its own
//! where it's put (`--live-slip [x y]`, for tests and the benchmark). The garden lives while the
//! slip is up (held still with Reduce Motion on); rows light under the pointer and do what the
//! Swift slip does, with made-up answers. Keys (↑ ↓, Return, Escape, ⌘Q, ⌘,) need
//! `IRIS_TAKE_KEYS`, as the test slip never takes the keyboard from the app in front.

use std::time::{Duration, Instant};

use gpui::*;

use crate::page::{Outcome, Page};
use crate::slip::{self, Engine, Slip};

pub struct SlipView {
    pub start: Instant,
    /// When it opened, for its grow out of the flower.
    pub opened: Instant,
    pub engine: Engine,
    pub highlighted: Option<&'static str>,
    pub page: bool,
    pub busy: Vec<&'static str>,
    pub focus: FocusHandle,
    pub reduce_motion: bool,
    /// What Settings… does (opens the Settings window, when there is one).
    pub on_settings: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>>,
    /// Why the model couldn't be got, in the engine's words, when it couldn't.
    pub problem: Option<String>,
    /// Setup's progress: complete (the full rows), else which step (the header's line).
    pub setup_complete: bool,
    pub setup_line: Option<String>,
    pub setup_garden: Option<crate::garden::Stage>,
    /// Where Check for Updates stands (the Updates page follows it).
    pub update: crate::updater::Phase,
    /// The page's buttons' hover and press, as they move.
    pub anim: std::rc::Rc<std::cell::RefCell<crate::interact::Anim>>,
    _watch: Option<Subscription>,
    _watch_update: Subscription,
}

impl SlipView {
    pub fn new(cx: &mut Context<Self>) -> SlipView {
        let mut v = SlipView {
            start: Instant::now(),
            opened: Instant::now(),
            engine: Engine::Idle,
            highlighted: None,
            page: false,
            busy: vec![],
            focus: cx.focus_handle(),
            reduce_motion: reduce_motion(),
            on_settings: None,
            problem: None,
            setup_complete: true,
            setup_line: None,
            setup_garden: None,
            update: crate::updater::phase(cx),
            anim: Default::default(),
            _watch_update: {
                let st = crate::updater::state(cx);
                cx.observe(&st, |v, st, cx| {
                    v.update = st.read(cx).phase.clone();
                    // Up to date: long enough to read the letter, then the page turns back
                    // (`UpdateCheckScene.ending + 2`).
                    if let crate::updater::Phase::Answered(crate::updater::Status::UpToDate, at) = v.update {
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_secs_f64(3.7 * crate::parts::slow() as f64)).await;
                            let _ = this.update(cx, |v, cx| {
                                if matches!(v.update, crate::updater::Phase::Answered(_, a) if a == at) {
                                    v.page = false;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }
                    cx.notify();
                })
            },
            // The model's state changes: draw the slip again.
            _watch: crate::brain::get(cx).map(|b| cx.observe(&b, |v, _, cx| { v.sync(cx); cx.notify() })),
        };
        v.sync(cx);
        v
    }

    /// The model's state from the brain, when it's running (the made-up one otherwise).
    fn sync(&mut self, cx: &App) {
        let Some(brain) = crate::brain::get(cx) else { return };
        self.setup_complete = crate::live_setup::progress(cx).is_complete();
        self.setup_line = crate::live_setup::slip_line(cx);
        let p = crate::live_setup::progress(cx);
        self.setup_garden = (!p.is_complete()).then(|| crate::setup::garden_stage(p.garden()));
        let b = brain.read(cx);
        self.problem = None;
        self.engine = match &b.engine {
            crate::brain::Engine::Idle => Engine::Idle,
            crate::brain::Engine::Downloading(f) => Engine::Downloading(*f),
            crate::brain::Engine::Loading => Engine::Loading,
            crate::brain::Engine::Ready(_) => Engine::Ready,
            crate::brain::Engine::Failed(f) => {
                self.problem = Some(f.sentence.clone());
                Engine::Idle
            }
        };
        self.busy = if matches!(self.engine, Engine::Loading | Engine::Downloading(_)) { vec!["model"] } else { vec![] };
    }

    fn shown(&self) -> Slip {
        let ready = self.engine == Engine::Ready;
        let page = self.page.then(|| update_page(&self.update));
        Slip { engine: self.engine, groups: slip::rows(crate::app::shortcut_taken(), self.setup_complete, ready), highlighted: self.highlighted, busy: self.busy.clone(),
            setup_line: self.setup_line.clone(), setup_garden: self.setup_garden, problem_line: self.problem.clone(), updating: match self.update {
                // A new Iris building: the header says Updating, the garden grows in the rain.
                crate::updater::Phase::Updating(since) => Some(crate::updater::progress(since.elapsed().as_secs_f64())),
                _ => None,
            }, page }
    }

    /// A row (or Back) pressed: what the Swift slip does, with made-up answers.
    fn press(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "quit" => cx.quit(),
            "back" => self.page = false,
            // Check for Updates turns the page.
            // Check for Updates turns the page and looks (`checkForUpdates`).
            "update" => {
                self.page = true;
                crate::updater::start(cx);
            }
            // The answer's own button: Update & reopen, Download, Try again.
            "page-action" => match &self.update {
                // A downloaded copy installs the release itself when it can; else its page.
                crate::updater::Phase::Answered(crate::updater::Status::NewRelease(tag, _, Some(zip)), _) => {
                    let (tag, zip) = (tag.clone(), zip.clone());
                    window.remove_window();
                    crate::updater::install_release(tag, zip, cx);
                }
                crate::updater::Phase::Answered(crate::updater::Status::NewRelease(_, page, None), _) => crate::controls::open_url(page),
                crate::updater::Phase::Answered(crate::updater::Status::Failed(_) | crate::updater::Status::InstallFailed(_), _) => crate::updater::start(cx),
                crate::updater::Phase::Answered(crate::updater::Status::Behind(..), _) => {
                    window.remove_window();
                    crate::updater::install(cx);
                }
                _ => {}
            },
            // With the brain: Load model now loads Gemma; Free model memory lets it go. The slip
            // stays open and follows the model's state.
            "model" if crate::brain::get(cx).is_some() => {
                if self.engine == Engine::Ready { crate::brain::unload() } else { crate::brain::load(false) }
            }
            // Load model now keeps the slip open: the row dims, the header wakes up, then it's ready.
            "model" if self.engine == Engine::Ready => self.engine = Engine::Idle,
            "model" => {
                self.engine = Engine::Loading;
                self.busy = vec!["model"];
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(1400)).await;
                    let _ = this.update(cx, |v, cx| {
                        v.engine = Engine::Ready;
                        v.busy.clear();
                        cx.notify();
                    });
                })
                .detach();
            }
            // Check text and Quick Fix do what their shortcuts do, once the slip has gone and the
            // keyboard is back in the app you were writing in.
            "check" | "quick" if crate::brain::get(cx).is_some() => {
                window.remove_window();
                let quick = id == "quick";
                cx.spawn(async move |_, cx| {
                    cx.background_executor().timer(Duration::from_millis(120)).await;
                    let _ = cx.update(|cx| if quick { crate::flow::quick_fix(None, cx) } else { crate::flow::open_panel(None, cx) });
                })
                .detach();
            }
            "setup" if crate::brain::get(cx).is_some() => {
                window.remove_window();
                crate::live_setup::show(cx);
            }
            "settings" => {
                if let Some(open) = self.on_settings.clone() {
                    open(window, cx);
                }
                window.remove_window();
            }
            _ => window.remove_window(),
        }
        cx.notify();
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let ids: Vec<&'static str> = slip::rows(crate::app::shortcut_taken(), self.setup_complete, self.engine == Engine::Ready)
            .into_iter().flatten().map(|c| c.id).filter(|id| !self.busy.contains(id)).collect();
        match k.key.as_str() {
            "escape" if self.page => self.page = false,
            "escape" => window.remove_window(),
            "down" | "up" if !self.page => {
                let at = self.highlighted.and_then(|h| ids.iter().position(|i| *i == h));
                let n = ids.len();
                let next = match (at, k.key == "down") {
                    (None, true) => 0,
                    (None, false) => n - 1,
                    (Some(i), true) => (i + 1).min(n - 1),
                    (Some(i), false) => i.saturating_sub(1),
                };
                self.highlighted = Some(ids[next]);
            }
            "enter" | "space" if !self.page => {
                if let Some(id) = self.highlighted {
                    self.press(id, window, cx);
                }
            }
            "q" if k.modifiers.platform => cx.quit(),
            "," if k.modifiers.platform => self.press("settings", window, cx),
            _ => return,
        }
        cx.notify();
    }
}

impl Render for SlipView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let began = Instant::now();
        crate::riso::let_go_of_last_frame(window);
        let scale = window.scale_factor();
        // The garden lives while the slip is up; Reduce Motion holds it still, as Swift's does.
        let t = if self.reduce_motion { 1000.0 } else { 1000.0 + self.start.elapsed().as_secs_f64() };
        if !self.reduce_motion {
            window.request_animation_frame();
        }
        let entity = cx.entity();
        let hover = {
            let e = entity.clone();
            move |id: &'static str| -> Box<dyn Fn(&bool, &mut Window, &mut App)> {
                let e = e.clone();
                Box::new(move |inside, _, cx| if *inside { e.update(cx, |v, cx| { v.highlighted = Some(id); cx.notify() }) })
            }
        };
        let press = {
            let e = entity.clone();
            move |id: &'static str| -> Box<dyn Fn(&ClickEvent, &mut Window, &mut App)> {
                let e = e.clone();
                Box::new(move |_, window, cx| e.update(cx, |v, cx| v.press(id, window, cx)))
            }
        };
        let acts = slip::Acts { hover: &hover, press: &press };
        // Opens from the tail: a small grow out of the flower (0.94 → 1 on SwiftUI's
        // `.spring(duration: 0.28, bounce: 0.18)`), fading in over `DS.Motion.fast.enter`.
        let since = self.opened.elapsed().as_secs_f32() / crate::parts::slow();
        let (k, alpha) = if self.reduce_motion { (1.0, crate::parts::ds_curve(since / 0.15)) } else {
            (0.94 + 0.06 * crate::parts::swift_spring(since, 0.28, 0.18), crate::parts::ds_curve(since / 0.15))
        };
        if since < 0.6 || self.opened.elapsed().as_secs_f32() < 0.6 * crate::parts::slow() {
            window.request_animation_frame();
        }
        let shown = self.shown();
        // The Updates page's letter plays from the check's clocks.
        crate::page::set_phase(&self.update);
        let anim = self.anim.clone();
        let reduce = self.reduce_motion;
        let body = crate::interact::build(&anim, reduce, window, cx, || crate::parts::with_zoom(k, || slip::slip_live(&shown, t, Some(&acts), scale)));
        let body = div().relative().left(px(slip::WIDTH * (1.0 - k) / 2.0)).opacity(alpha).child(body);
        let trace = std::env::var("IRIS_TRACE").is_ok();
        // The window fits the slip: measure it and resize.
        let fit = canvas(|_, _, _| (), move |b, _, window, _| {
            if trace {
                eprintln!("frame cpu_ms={:.3}", began.elapsed().as_secs_f64() * 1000.0);
            }
            let want = size(b.size.width, b.size.height);
            if (window.bounds().size.height - want.height).abs() > px(0.5) {
                window.resize(want);
            }
        }).absolute().top_0().left_0().size_full();
        // The window's root fills the window; the slip inside it takes its own height.
        div().size_full().track_focus(&self.focus)
            // The slip's keys are its own: handled once (see the panel's).
            .on_key_down(cx.listener(|v, e: &KeyDownEvent, window, cx| {
                v.key(e, window, cx);
                cx.stop_propagation();
            }))
            // Room round the paper for its shadow (none on top: the tail reaches up to the bar).
            .child(div().relative().pl(px(18.0)).pr(px(18.0)).pb(px(26.0)).child(body).child(fit))
    }
}

/// Whether Reduce Motion is on (`IRIS_REDUCE_MOTION=1` says so for tests).
pub fn reduce_motion() -> bool {
    if let Ok(v) = std::env::var("IRIS_REDUCE_MOTION") {
        return v == "1";
    }
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::runtime::{AnyClass, AnyObject, Bool};
        use objc2::msg_send;
        let Some(class) = AnyClass::get(c"NSWorkspace") else { return false };
        let ws: *mut AnyObject = msg_send![class, sharedWorkspace];
        let on: Bool = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        on.as_bool()
    }
    // Windows: Settings › Accessibility › Visual effects › Animation effects.
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};
        let mut on = windows::core::BOOL(1);
        let asked = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        asked.is_ok() && !on.as_bool()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    false
}

/// The slip's window options at `origin` (screen points).
pub fn options(origin: Point<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(slip::WIDTH + 36.0), px(480.0)) })),
        titlebar: None,
        focus: crate::takes_keys(),
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        inactive_frame_interval: None,
        ..Default::default()
    }
}

/// Opens the slip at `origin`. A click anywhere else closes it, like a menu (never under test:
/// the clicks on the test display's neighbour are someone's work).
pub fn open(origin: Point<Pixels>, on_settings: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>>, cx: &mut App) -> Option<WindowHandle<SlipView>> {
    crate::slip::set_solid_paper(true);
    let handle = cx.open_window(options(origin), move |window, cx| {
        crate::no_system_shadow(window);
        let view = cx.new(|cx| SlipView { on_settings, ..SlipView::new(cx) });
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        view
    })
    .ok()?;
    if !crate::textbridge::under_test() {
        let clicked = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = clicked.clone();
        let id = crate::monitor::watch(crate::monitor::Watch::Clicks, move || c.set(true));
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let gone = cx.update(|cx| {
                    if clicked.get() {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                        return true;
                    }
                    handle.update(cx, |_, _, _| ()).is_err()
                });
                if gone {
                    crate::monitor::stop(id);
                    return;
                }
            }
        })
        .detach();
    }
    Some(handle)
}

/// The Updates page for where the check stands (`UpdateCheck.Words`, the slip's page).
fn update_page(phase: &crate::updater::Phase) -> Page {
    use crate::updater::{Phase, Status};
    match phase {
        Phase::Idle | Phase::Looking(_) | Phase::Updating(_) => Page { title: "Updates".into(), lead: "Looking for a new Iris…".into(), outcome: Outcome::Looking,
            changes: vec![], button: None, warning: false },
        Phase::Answered(status, _) => {
            let (title, lead, warning) = crate::updater::words(status);
            let outcome = match status {
                Status::UpToDate => Outcome::UpToDate,
                Status::Behind(n, _) => Outcome::Changes(*n),
                Status::NewRelease(..) => Outcome::Changes(1),
                _ => Outcome::Failed,
            };
            let changes = if let Status::Behind(_, s) = status { s.clone() } else { vec![] };
            let button = match status {
                Status::NewRelease(_, _, Some(_)) => Some("Update & reopen"),
                Status::NewRelease(_, _, None) => Some("Download"),
                Status::Failed(_) | Status::InstallFailed(_) => Some("Try again"),
                _ => None,
            };
            Page { title, lead, outcome, changes, button, warning }
        }
    }
}
