//! Setup, live (`OnboardingModel` and the window `showOnboarding` opens): the facts read from
//! the Mac (the permission, polled every 0.8 s while the window is open; the model, as the
//! brain reports it), turned into `Progress`, drawn by `setup::live_window`. One step at a
//! time, each ticked by the real thing happening.
//!
//! Under test (`IRIS_CONTROL`) nothing leaves the test display: Open System Settings, Open
//! Storage Settings and Quit and reopen only say what they would do, and `pretend` can make the
//! permission or the model look missing, so every step can be walked without touching either.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::*;

use crate::brain;
use crate::setup::{self, Act, Activity, Facts, Failure, Live, Progress, SetupState, Step};
use crate::textbridge::{self, under_test};

thread_local! {
    static OPEN: RefCell<Option<(WindowHandle<SetupLive>, WeakEntity<SetupLive>)>> = const { RefCell::new(None) };
    /// Under test: the permission looks missing.
    static PRETEND_UNTRUSTED: Cell<bool> = const { Cell::new(false) };
    /// Under test: the model looks as if it's downloading, this far in.
    static PRETEND_DOWNLOAD: Cell<Option<f64>> = const { Cell::new(None) };
    /// Under test: what the shortcut reads can't be replaced (as a selection in a page).
    static PRETEND_READ_ONLY: Cell<bool> = const { Cell::new(false) };
    /// Under test: Open at login as the switch last set it.
    static PRETEND_LOGIN: Cell<Option<bool>> = const { Cell::new(None) };
    /// Under test: the model looks missing, until a download is asked for.
    static PRETEND_NO_MODEL: Cell<bool> = const { Cell::new(false) };
}

/// `pretend untrusted|trusted|no-model|downloading|off` (control socket, under test only).
pub fn pretend(what: &str) -> bool {
    match what {
        "untrusted" => PRETEND_UNTRUSTED.set(true),
        "trusted" => PRETEND_UNTRUSTED.set(false),
        "no-model" => PRETEND_NO_MODEL.set(true),
        // Downloading (the model not on disk yet), 37% in: the page Stop download is on.
        "downloading" => {
            PRETEND_NO_MODEL.set(true);
            PRETEND_DOWNLOAD.set(Some(0.37));
        }
        "read-only" => PRETEND_READ_ONLY.set(true),
        "off" => {
            PRETEND_READ_ONLY.set(false);
            PRETEND_UNTRUSTED.set(false);
            PRETEND_NO_MODEL.set(false);
            PRETEND_DOWNLOAD.set(None);
        }
        _ => return false,
    }
    true
}

/// Under test: whether what's read should count as read-only (`pretend read-only`).
pub fn pretend_read_only() -> bool {
    under_test() && PRETEND_READ_ONLY.get()
}

/// Iris may read other apps' text boxes (`TextBridge.isTrusted`).
pub fn trusted() -> bool {
    !(under_test() && PRETEND_UNTRUSTED.get()) && textbridge::is_trusted()
}

/// The model the brain will load: chosen in Settings, else the one it starts on.
fn chosen_id(cx: &App) -> Option<String> {
    crate::app::settings().model.clone()
        .or_else(|| brain::get(cx).and_then(|b| b.read(cx).chosen.clone()))
}

/// A failure's figures live as long as the app (they're few, and `Failure` is `Copy`).
fn interned(s: &str) -> &'static str {
    thread_local! { static KEPT: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }
    KEPT.with(|k| {
        let mut k = k.borrow_mut();
        if let Some(found) = k.iter().find(|x| **x == s) {
            return *found;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        k.push(leaked);
        leaked
    })
}

fn failure_of(f: &brain::ModelFailure) -> Failure {
    match f.kind.as_str() {
        "noRoom" => Failure::NoRoom { needed: interned(&f.needed), free: interned(&f.free) },
        "offline" => Failure::Offline,
        "notDownloaded" => Failure::NotDownloaded,
        _ => Failure::Other,
    }
}

/// What the Mac says now (`SetupProgress.Facts.now`); the window adds what happened in it.
pub fn facts_now(tried: bool, back_from_settings_untrusted: bool, cx: &App) -> Facts {
    let chosen = chosen_id(cx);
    let (on_disk, activity) = match brain::get(cx) {
        Some(b) => {
            let b = b.read(cx);
            let on_disk = b.models.iter().any(|m| Some(&m.id) == chosen.as_ref() && m.downloaded);
            let activity = match &b.engine {
                brain::Engine::Failed(f) => Activity::Failed(failure_of(f)),
                brain::Engine::Downloading(x) => Activity::Downloading(*x),
                brain::Engine::Loading => Activity::Loading,
                brain::Engine::Ready(m) if Some(m) == chosen.as_ref() => Activity::Ready,
                _ => Activity::None,
            };
            (on_disk, activity)
        }
        None => (false, Activity::None),
    };
    let on_disk = on_disk && !(under_test() && PRETEND_NO_MODEL.get());
    let activity = match PRETEND_DOWNLOAD.get() {
        Some(f) if under_test() => Activity::Downloading(f),
        _ => activity,
    };
    Facts {
        trusted: trusted(),
        model_on_disk: on_disk,
        activity,
        tried,
        finished_before: crate::app::settings().onboarded,
        back_from_settings_untrusted,
    }
}

/// Where setup stands, from outside its window.
pub fn progress(cx: &App) -> Progress {
    Progress(facts_now(false, false, cx))
}

/// The shortcut and Quick Fix may check (`canCheck`): allowed, and the model on disk. A brain
/// that isn't running can't say, so it doesn't stop them (the panel says what's wrong).
pub fn can_check(cx: &App) -> bool {
    if brain::get(cx).is_none_or(|b| !b.read(cx).alive) {
        return trusted();
    }
    let f = facts_now(false, false, cx);
    f.trusted && f.model_on_disk
}

/// The slip's line under SETTING UP (`slipLine`); nil once setup is complete.
pub fn slip_line(cx: &App) -> Option<String> {
    let p = progress(cx);
    let step = p.current()?;
    let numbered = |what: &str| format!("Step {} of 3: {what}.", step as i32);
    Some(match step {
        Step::Allow if p.is_returning() => "Iris lost its permission to read text. Allow it again.".into(),
        Step::Allow => numbered("allow Iris to read the box you type in"),
        Step::Model => match p.0.activity {
            Activity::Failed(f) => brain::get(cx)
                .and_then(|b| if let brain::Engine::Failed(m) = &b.read(cx).engine { Some(m.sentence.clone()) } else { None })
                .unwrap_or_else(|| f.next_step()),
            Activity::Downloading(_) => numbered("getting the model"),
            _ if p.is_returning() => "The model isn't on this Mac any more. Download it again.".into(),
            _ => numbered("get the model"),
        },
        Step::TryIt => numbered("try it once"),
    })
}

/// The shortcut's keys as setup shows them: (symbol, word).
fn keys_of(k: crate::store::KeyCombo) -> Vec<(Option<String>, String)> {
    crate::app::shortcut_caps(k).into_iter().map(|cap| {
        let word = match cap.as_str() {
            "⌃" => Some("control"),
            "⌥" => Some("option"),
            "⇧" => Some("shift"),
            "⌘" => Some("command"),
            _ => None,
        };
        match word {
            Some(w) => (Some(cap), w.to_string()),
            None => (None, cap),
        }
    }).collect()
}

/// Opens setup at its current step, or brings the open one forward.
pub fn show(cx: &mut App) {
    let open = OPEN.with(|o| o.borrow().clone());
    if let Some((h, _)) = open
        && h.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        if !under_test() {
            cx.activate(true);
        }
        return;
    }
    let (x, y) = crate::test_display().map(|(x, y)| (x + 60.0, y + 30.0)).unwrap_or_else(|| {
        // Centred on the main screen, a little above the middle.
        cx.primary_display().map(|d| {
            let b = d.bounds();
            let (w, h): (f32, f32) = (b.size.width.into(), b.size.height.into());
            (((w - setup::WIDTH) / 2.0).max(0.0) as f64, ((h - setup::HEIGHT) / 3.0).max(30.0) as f64)
        }).unwrap_or((240.0, 80.0))
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(x as f32), px(y as f32)), size: size(px(setup::WIDTH), px(crate::settings::TITLE_BAR + setup::HEIGHT)) })),
        titlebar: Some(TitlebarOptions { title: Some("Set up Iris".into()), appears_transparent: true, traffic_light_position: None }),
        focus: crate::takes_keys(),
        show: true,
        kind: WindowKind::Normal,
        is_resizable: false,
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        crate::color_managed(window);
        cx.new(|cx| SetupLive::new(window, cx))
    });
    if let Ok(h) = opened {
        let weak = h.update(cx, |_, _, cx| cx.entity().downgrade()).ok();
        if let Some(weak) = weak {
            OPEN.with(|o| *o.borrow_mut() = Some((h, weak)));
        }
        setup::set_on_act(Some(Rc::new(|a, window, cx| {
            let Some(view) = OPEN.with(|o| o.borrow().as_ref().and_then(|(_, w)| w.upgrade())) else { return };
            view.update(cx, |v, cx| v.act(a, window, cx));
        })));
        news("setup open");
    }
    if !under_test() {
        cx.activate(true);
    }
}

/// As if Iris came to the front again (back from System Settings), for a test, which never
/// brings Iris forward.
pub fn came_back(cx: &mut App) -> bool {
    let Some(view) = OPEN.with(|o| o.borrow().as_ref().and_then(|(_, w)| w.upgrade())) else { return false };
    view.update(cx, |v, cx| v.came_back(cx));
    true
}

/// The practice box as the shortcut reads it (`TextBridge.capture` of Iris's own window): when
/// a test names Iris (`target` is this process), or the box has the keyboard. None otherwise.
pub fn own_capture(target: Option<i32>, cx: &mut App) -> Option<crate::textbridge::Captured> {
    let (h, view) = OPEN.with(|o| o.borrow().clone())?;
    let view = view.upgrade()?;
    let me = std::process::id() as i32;
    let named = target == Some(me);
    let (focused, origin) = h.update(cx, |v, window, cx| {
        let origin = crate::window_origin(window).unwrap_or_default();
        (v.practice.read(cx).focus_handle().is_focused(window) && window.is_window_active(), origin)
    }).ok()?;
    if !(named || (target.is_none() && focused)) {
        return None;
    }
    let text = view.read(cx).practice.read(cx).content.clone();
    let field_frame = setup::practice_bounds().map(|b| {
        let (x, y): (f32, f32) = (b.origin.x.into(), b.origin.y.into());
        let (w, hh): (f32, f32) = (b.size.width.into(), b.size.height.into());
        [origin.0 + x as f64, origin.1 + y as f64, w as f64, hh as f64]
    });
    news(&format!("setup: reading the practice box at {field_frame:?}"));
    Some(crate::textbridge::Captured {
        pid: me,
        app_name: Some("Iris".into()),
        bundle_id: Some(crate::store::bundle_id().to_string()),
        #[cfg(target_os = "macos")]
        element: None,
        text,
        scope: crate::textbridge::Scope::WholeField,
        field_frame,
        can_replace: true,
        own: true,
    })
}

/// Iris replaced the practice text (`irisPastedIntoIris`): the box holds `text`, setup counts it
/// tried, and the box has the keyboard again. True if setup was there to take it.
pub fn write_practice(text: &str, cx: &mut App) -> bool {
    let Some((h, _)) = OPEN.with(|o| o.borrow().clone()) else { return false };
    let text = text.to_string();
    h.update(cx, |v, window, cx| {
        v.practice.update(cx, |f, cx| f.set(&text, cx));
        v.tried = true;
        news("setup: tried");
        if !under_test() {
            window.activate_window();
            let focus = v.practice.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
        }
        cx.notify();
    }).is_ok()
}

/// For a test: types `text` into setup's window (the field with the keyboard). True if it took it.
pub fn type_text(text: &str, cx: &mut App) -> bool {
    let Some((h, _)) = OPEN.with(|o| o.borrow().clone()) else { return false };
    cx.update_window(h.into(), |_, window, cx| {
        let mut took = true;
        for ch in text.chars() {
            let key = if ch == ' ' { "space".to_string() } else { ch.to_lowercase().to_string() };
            let k = Keystroke { modifiers: Modifiers { shift: ch.is_uppercase(), ..Default::default() }, key, key_char: Some(ch.to_string()) };
            took &= window.dispatch_keystroke(k, cx);
        }
        took
    }).unwrap_or(false)
}

/// For a test: a key pressed in setup's window (`cmd-a`, `enter`…).
pub fn key(keystroke: &str, cx: &mut App) -> bool {
    let Some((h, _)) = OPEN.with(|o| o.borrow().clone()) else { return false };
    cx.update_window(h.into(), |_, window, cx| crate::text_field::key_into(window, keystroke, cx)).unwrap_or(false)
}

/// For a test: what the practice box holds.
pub fn practice_text(cx: &App) -> Option<String> {
    let (_, v) = OPEN.with(|o| o.borrow().clone())?;
    Some(v.upgrade()?.read(cx).practice.read(cx).content.clone())
}

/// Setup's window is open.
pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().as_ref().is_some_and(|(_, w)| w.upgrade().is_some()))
}

/// Says what setup did, for a test reading Iris's stderr.
fn news(what: &str) {
    if under_test() {
        eprintln!("iris: {what}");
    }
}

fn open_url(url: &str) {
    if under_test() {
        return news(&format!("would open {url}"));
    }
    let _ = std::process::Command::new("open").arg(url).spawn();
}

pub struct SetupLive {
    tried: bool,
    back_untrusted: bool,
    sent_to_settings: bool,
    trusted: bool,
    preloaded: bool,
    /// The practice box: what's in it, typed or fixed.
    practice: Entity<crate::text_field::TextField>,
    shown_step: Option<Option<Step>>,
    scroll: ScrollHandle,
    /// Its controls' hover and press, as they move.
    anim: Rc<RefCell<crate::interact::Anim>>,
    reduce_motion: bool,
    _watch: Vec<Subscription>,
}

impl SetupLive {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut watch = vec![];
        if let Some(b) = brain::get(cx) {
            watch.push(cx.observe(&b, |v, _, cx| {
                v.preload_if_ready(cx);
                cx.notify();
            }));
        }
        // Back from System Settings (or Storage): look again at once, and if Iris still isn't
        // allowed after a moment, say what to try.
        watch.push(cx.observe_window_activation(window, |v, window, cx| {
            if window.is_window_active() {
                v.came_back(cx);
            }
        }));
        // The permission is granted in System Settings: look while the window is open.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(800)).await;
                if this.update(cx, |v, cx| v.poll(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        let mut v = SetupLive {
            tried: false, back_untrusted: false, sent_to_settings: false, trusted: trusted(), preloaded: false,
            practice: { crate::text_field::init(cx); crate::text_field::TextField::new_multiline(setup::PRACTICE_SAMPLE, cx) },
            shown_step: None, scroll: ScrollHandle::new(),
            anim: Rc::new(RefCell::new(crate::interact::Anim::default())), reduce_motion: crate::live_slip::reduce_motion(), _watch: watch,
        };
        v.preload_if_ready(cx);
        v
    }

    fn launch_at_login(&self) -> bool {
        PRETEND_LOGIN.get().unwrap_or_else(|| !under_test() && crate::login_item::enabled())
    }

    fn facts(&self, cx: &App) -> Facts {
        facts_now(self.tried, self.back_untrusted, cx)
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let now = trusted();
        if now != self.trusted {
            self.trusted = now;
            news(if now { "setup: allowed" } else { "setup: not allowed" });
            cx.notify();
        }
        if now && self.back_untrusted {
            self.back_untrusted = false;
            cx.notify();
        }
        self.preload_if_ready(cx);
    }

    fn came_back(&mut self, cx: &mut Context<Self>) {
        self.poll(cx);
        if self.sent_to_settings && !self.trusted {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                let _ = this.update(cx, |v, cx| {
                    if !trusted() {
                        v.back_untrusted = true;
                        news("setup: back from System Settings, still not allowed");
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        // Space freed in Storage: try the download again by itself.
        if matches!(self.facts(cx).activity, Activity::Failed(Failure::NoRoom { .. })) {
            brain::load(true);
        }
    }

    /// At Try it, wake the model on disk so the practice check is quick. Never a download.
    fn preload_if_ready(&mut self, cx: &mut Context<Self>) {
        let p = Progress(self.facts(cx));
        if p.current() == Some(Step::TryIt) && p.0.activity == Activity::None {
            // Once until the model's state moves on (a load asked twice is still one load).
            if !self.preloaded {
                self.preloaded = true;
                brain::load(false);
            }
        } else {
            self.preloaded = false;
        }
    }

    fn act(&mut self, a: Act, window: &mut Window, cx: &mut Context<Self>) {
        news(&format!("setup: pressed {a:?}"));
        match a {
            Act::OpenSystemSettings => {
                self.sent_to_settings = true;
                if !under_test() {
                    textbridge::request_trust();
                }
                open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
            }
            Act::Relaunch => relaunch(cx),
            Act::Download | Act::TryAgain => {
                PRETEND_NO_MODEL.set(false);
                brain::load(true);
            }
            // Asked first (`ModelDialogs.stop`): what's in stays, and picks up next time.
            Act::StopDownload => crate::confirm::ask(window, "Stop downloading the model?",
                "What's downloaded so far stays on this Mac, and the download picks up where it stopped the next time.",
                "Stop download", |_, _| {
                    news("setup: download stopped");
                    PRETEND_DOWNLOAD.set(None);
                    brain::unload();
                }),
            Act::OpenStorage => open_url("x-apple.systempreferences:com.apple.settings.Storage"),
            Act::Use(i) => {
                crate::app::change(|s| s.model = Some(setup::model_id(i).to_string()));
                brain::configure(cx);
            }
            Act::ChangeShortcut => crate::open_settings(crate::settings::Page::Shortcuts, cx),
            Act::Done => {
                if !crate::app::settings().onboarded {
                    crate::app::change(|s| s.onboarded = true);
                }
                window.remove_window();
                // Iris has no Dock icon or window: say where it went (`showMenuBarHint`).
                crate::live_hint::find_me(cx);
            }
            Act::Later => window.remove_window(),
            // Open at login (`SMAppService`), the same switch as Settings › General's. Under test
            // it's only remembered here: a test never adds a login item to this Mac.
            Act::LaunchAtLogin => {
                let on = !self.launch_at_login();
                if under_test() {
                    PRETEND_LOGIN.set(Some(on));
                    news(&format!("setup: open at login {}", if on { "on" } else { "off" }));
                } else {
                    crate::login_item::set(on);
                }
            }
        }
        cx.notify();
    }

    fn state(&self, window: &Window, cx: &App) -> SetupState {
        let s = crate::app::settings();
        let (recommended, on_disk) = match brain::get(cx) {
            Some(b) => {
                let b = b.read(cx);
                let mut on_disk = [false; 2];
                for m in &b.models {
                    if let Some(i) = setup::model_index(&m.id) {
                        on_disk[i] = m.downloaded && !(under_test() && PRETEND_NO_MODEL.get());
                    }
                }
                (b.recommended.as_deref().and_then(setup::model_index).unwrap_or(0), on_disk)
            }
            None => (0, [false; 2]),
        };
        let chosen = chosen_id(cx).as_deref().and_then(setup::model_index).unwrap_or(recommended);
        SetupState {
            id: "live",
            facts: self.facts(cx),
            shortcut_taken: crate::app::shortcut_taken(),
            // The done page shows the box as Iris left it.
            practice: self.tried.then(|| self.practice.read(cx).content.clone()),
            live: Some(Live {
                chosen,
                recommended,
                on_disk,
                open_keys: keys_of(s.open_shortcut),
                quick_keys: s.quick_fix_shortcut.map(keys_of),
                launch_at_login: self.launch_at_login(),
                practice: Some(crate::text_field::Field::of(&self.practice, window, cx)),
            }),
        }
    }
}

/// Quit and reopen Iris: open a new copy, and quit this one once it's running. If it doesn't
/// open, this one stays, so Iris never just vanishes.
fn relaunch(cx: &mut App) {
    let bundle = std::env::current_exe().ok().and_then(|exe| exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(|p| p.to_path_buf()));
    let Some(bundle) = bundle else { return news("setup: not in an app bundle, can't reopen") };
    if under_test() {
        return news(&format!("would reopen {}", bundle.display()));
    }
    match std::process::Command::new("open").arg("-n").arg(&bundle).status() {
        Ok(s) if s.success() => cx.quit(),
        _ => news("setup: couldn't reopen"),
    }
}

impl Drop for SetupLive {
    fn drop(&mut self) {
        setup::set_on_act(None);
        OPEN.with(|o| *o.borrow_mut() = None);
    }
}

impl Render for SetupLive {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::riso::let_go_of_last_frame(window);
        let state = self.state(window, cx);
        // A new step starts at the top of the page.
        let step = Progress(state.facts).current();
        if self.shown_step.is_some_and(|s| s != step) {
            self.scroll.set_offset(point(px(0.0), px(0.0)));
            // A new page starts at rest: nothing on it replays.
            self.anim.borrow_mut().reset();
            news(&format!("setup: step {}", step.map_or("done".into(), |s| format!("{}", s as i32))));
        }
        self.shown_step = Some(step);
        if step.is_none() && self.tried && !crate::app::settings().onboarded {
            crate::app::change(|s| s.onboarded = true);
        }
        let scale = window.scale_factor();
        let anim = self.anim.clone();
        let page = crate::interact::build(&anim, self.reduce_motion, window, cx, || setup::live_window(&state, scale, &self.scroll));
        div().size_full().child(page).child(crate::confirm::layer(window, cx))
    }
}
