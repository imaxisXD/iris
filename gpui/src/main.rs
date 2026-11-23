// On Windows Iris is a windowed app: no console beside it.
#![cfg_attr(windows, windows_subsystem = "windows")]
//! Iris's answer panel in GPUI: a trial of the cross-platform interface.
//!
//! - `iris --snap <dir> [--scale 2]` draws every state the Swift app's design check draws
//!   (`Iris --panel-states`) with GPUI's offscreen Metal renderer, one PNG each (sNN.png).
//!   Nothing appears on screen. Needs the `tooling` feature (on by default).
//! - `iris --live [x y]` opens the panel live as a borderless panel that never activates the app
//!   (top left at `x, y` in screen points), with made-up answers: click a tone or press Tab or
//!   1–5; Return or Escape closes it.
//! - `iris --list-states` names the states.

mod app;
mod appicon;
mod brain;
mod components;
mod confirm;
mod flow;
mod garden;
mod host_level;
mod hotkey;
mod keys;
mod words;
mod icons;
mod learn_words;
mod letters;
mod live;
mod live_settings;
mod live_hint;
mod live_setup;
mod live_slip;
mod login_item;
mod menubar;
mod mock;
mod model;
mod model_compare;
mod monitor;
mod native_menu;
mod notes_live;
mod opening;
mod page;
mod pages;
mod panel;
mod paste;
mod parts;
mod pen;
mod raster;
mod riso;
mod rig;
mod scenes;
mod settings;
mod controls;
mod driven;
mod settings_kit;
mod text_field;
mod interact;
mod setup;
mod slip;
mod store;
mod textbridge;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod spelling_win;
#[cfg(windows)]
mod apps_win;
#[cfg(windows)]
mod update_win;
mod tones;
mod toast;
mod updater;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::*;

use crate::model::{Answer, Phase, Verdict};
use crate::riso::{col, hue};

thread_local! {
    /// `IRIS_TRACE`: when this frame's render began, and when the last frame finished painting.
    static FRAME: Cell<(Option<Instant>, Option<Instant>)> = const { Cell::new((None, None)) };
    /// `IRIS_TRACE`: when the last tone was picked.
    static PICKED: Cell<Option<Instant>> = const { Cell::new(None) };
    /// The live panel's clock for the waiting dots; snapshots hold it at 0.
    static CLOCK: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Seconds since the live panel opened (0 in a snapshot).
pub fn clock() -> f32 {
    CLOCK.with(|c| c.get().map_or(0.0, |t| t.elapsed().as_secs_f32()))
}

fn fonts() -> Vec<std::borrow::Cow<'static, [u8]>> {
    vec![parts::MONO_430.into(), parts::MONO_450.into(), parts::MONO_560.into(), parts::CAVEAT.into(), letters::RUBIK.into()]
}

// MARK: - App icons

/// The app icons found so far, by bundle ID (shared, so a background thread can find one early).
static ICONS: std::sync::LazyLock<std::sync::Mutex<HashMap<String, Option<(Vec<u8>, u32)>>>> =
    std::sync::LazyLock::new(Default::default);

/// The app's real icon, 44 pixels square (RGBA), found by its bundle ID. Finding it asks
/// Spotlight and converts the icon (tens of milliseconds), so live windows find theirs early on
/// a background thread (`prefetch_icon`).
pub fn app_icon(bundle: &str) -> Option<(Vec<u8>, u32)> {
    if let Some(hit) = ICONS.lock().unwrap().get(bundle).cloned() {
        return hit;
    }
    let found = load_icon(bundle);
    let mut cache = ICONS.lock().unwrap();
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(bundle.to_string(), found.clone());
    found
}

/// Finds an app's icon on a background thread, before it's needed.
pub fn prefetch_icon(bundle: &'static str) {
    std::thread::spawn(move || {
        let _ = app_icon(bundle);
    });
}

/// An app's icon from its path, `n` pixels square (RGBA), as the Mac draws it.
#[cfg(target_os = "macos")]
pub fn icon_of_app(app: &str, n: u32) -> Option<Vec<u8>> {
    appicon::of_app(app, n)
}

#[cfg(not(target_os = "macos"))]
pub fn icon_of_app(_app: &str, _n: u32) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "macos")]
fn load_icon(bundle: &str) -> Option<(Vec<u8>, u32)> {
    let app = appicon::path_of(bundle)?;
    Some((appicon::of_app(&app, 44)?, 44))
}

#[cfg(not(target_os = "macos"))]
fn load_icon(_bundle: &str) -> Option<(Vec<u8>, u32)> {
    None
}

// MARK: - The view

struct PanelView {
    panel: model::Panel,
    live: bool,
    /// Live: when the made-up answer for the picked tone lands.
    due: Option<Instant>,
    anim: std::rc::Rc<std::cell::RefCell<crate::interact::Anim>>,
    focus: FocusHandle,
}

/// What each tone comes back with in the live panel (the design check's texts).
fn answer(tone: usize, original: &str) -> Answer {
    match tone {
        0 => Answer::fix(original, "hey, I have checked the PR and it looks fine, but one thing needs to change before merge @lead", Some(Verdict::Minor), false),
        1 => Answer::tone(original, "Hi @lead, I've reviewed the PR. It looks good overall, but one change is needed before we merge.", false),
        2 => Answer::tone(original, "Hey @lead! I checked the PR and it looks good. Just one small thing to change before we merge it.", false),
        3 => Answer::tone(original, "PR looks good, @lead. One change needed before merge.", false),
        _ => Answer::tone(original, "I checked the PR and it looks fine. One thing needs to change before we merge, @lead.", false),
    }
}

impl PanelView {
    /// Picks a tone. Live, the draft waits a moment in grey (`MockAnswers.delay`, 350 ms) and
    /// the answer comes in.
    fn pick(&mut self, tone: usize, cx: &mut Context<Self>) {
        if tone == self.panel.selected && self.due.is_none() {
            return;
        }
        PICKED.with(|p| p.set(Some(Instant::now())));
        self.panel.selected = tone;
        self.panel.retry_note = None;
        self.panel.copied = false;
        self.panel.phase = Phase::Generating(String::new());
        let due = Instant::now() + Duration::from_millis(350);
        self.due = Some(due);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(350)).await;
            let _ = this.update(cx, |v, cx| {
                if v.due == Some(due) {
                    v.due = None;
                    v.panel.phase = Phase::Done(answer(v.panel.selected, &v.panel.original));
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for PanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.live {
            riso::let_go_of_last_frame(window);
        }
        let tracing = std::env::var("IRIS_TRACE").is_ok();
        if std::env::var("IRIS_DEBUG").is_ok() {
            eprintln!("render: window active {}", window.is_window_active());
        }
        if tracing {
            FRAME.with(|f| f.set((Some(Instant::now()), f.get().1)));
        }
        let scale = window.scale_factor();
        let entity = cx.entity();
        let pick_fn = move |i: usize| -> crate::interact::Act {
            let e = entity.clone();
            crate::interact::act(move |_, cx| e.update(cx, |v, cx| v.pick(i, cx)))
        };
        let look = panel::Look { scale, icon: &app_icon, pick: if self.live { Some(&pick_fn) } else { None }, act: None };
        let working = matches!(self.panel.phase, Phase::Preparing | Phase::Generating(_));
        if self.live && working {
            // The waiting dots move: draw again next frame.
            window.request_animation_frame();
        }
        let panel_state = &self.panel;
        let body = if self.live {
            let anim = self.anim.clone();
            crate::interact::build(&anim, crate::live_slip::reduce_motion(), window, cx, || panel::panel(panel_state, &look))
        } else {
            panel::panel(panel_state, &look)
        };
        let root = div().track_focus(&self.focus).key_context("Panel")
            .on_any_mouse_down(|e, _, _| if std::env::var("IRIS_DEBUG").is_ok() { eprintln!("mouse down at {:?}", e.position) })
            .on_key_down(cx.listener(|v, e: &KeyDownEvent, window, cx| {
                let n = v.panel.tones.len().max(1);
                let key = e.keystroke.key.as_str();
                match key {
                    "tab" => {
                        let next = if e.keystroke.modifiers.shift { (v.panel.selected + n - 1) % n } else { (v.panel.selected + 1) % n };
                        v.pick(next, cx);
                    }
                    "right" if !e.keystroke.modifiers.platform => v.pick((v.panel.selected + 1) % n, cx),
                    "left" if !e.keystroke.modifiers.platform => v.pick((v.panel.selected + n - 1) % n, cx),
                    "1" | "2" | "3" | "4" | "5" | "6" => {
                        let idx = key.parse::<usize>().unwrap() - 1;
                        if idx < n {
                            v.pick(idx, cx);
                        }
                    }
                    "escape" | "enter" => window.remove_window(),
                    _ => {}
                }
            }))
            .pl(px(panel::MARGIN.0)).pt(px(panel::MARGIN.1)).pr(px(panel::MARGIN.0)).pb(px(panel::MARGIN.2))
            .child(body);
        // Frame timing: CPU from render to the end of paint, and the time since the last frame.
        let trace = canvas(|_, _, _| (), move |_, _, _, _| {
            if !tracing {
                return;
            }
            let now = Instant::now();
            FRAME.with(|f| {
                let (start, last) = f.get();
                let cpu = start.map_or(0.0, |s| (now - s).as_secs_f64() * 1000.0);
                let gap = last.map_or(0.0, |l| (now - l).as_secs_f64() * 1000.0);
                let picked = PICKED.with(|p| p.get()).map_or(-1.0, |p| (now - p).as_secs_f64() * 1000.0);
                eprintln!("frame cpu_ms={cpu:.3} since_last_ms={gap:.3} since_pick_ms={picked:.1}");
                f.set((None, Some(now)));
            });
        }).absolute().size_0();
        let root = root.child(trace);
        if self.live {
            // The window fits the panel: measure what was laid out, and resize to it.
            let fit = canvas(|_, _, _| (), |b, _, window, _| {
                let want = size(b.size.width, b.size.height);
                if std::env::var("IRIS_DEBUG").is_ok() { eprintln!("fit: want {want:?}, window {:?}", window.bounds().size); }
                if (window.bounds().size.height - want.height).abs() > px(0.5) {
                    window.resize(want);
                }
            }).absolute().top_0().left_0().size_full();
            div().size_full().child(root.relative().child(fit))
        } else {
            div().size_full().bg(col(hue::GROUND)).child(root)
        }
    }
}

// MARK: - Snapshots

#[cfg(feature = "tooling")]
fn snap(dir: &str, scale: f32) -> anyhow::Result<()> {
    let _ = scale; // GPUI's test window draws at 2x.
    std::fs::create_dir_all(dir)?;
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, std::sync::Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(|cx| cx.text_system().add_fonts(fonts()))?;
    for (i, st) in model::states().into_iter().enumerate() {
        let win = cx.open_window(size(px(672.0), px(340.0)), |_, cx| {
            cx.new(|cx| PanelView { panel: st.panel, live: false, due: None, anim: Default::default(), focus: cx.focus_handle() })
        })?;
        cx.run_until_parked();
        let shot = cx.capture_screenshot(win.into())?;
        let out = format!("{dir}/s{:02}.png", i + 1);
        shot.save(&out)?;
        println!("{out}\t{}x{}\t{}", shot.width(), shot.height(), st.caption);
    }
    Ok(())
}

/// A scene drawn headlessly at one moment (`--snap <png> --scene <name> --size WxH --at <t>`).
#[cfg(feature = "tooling")]
struct SceneView {
    scene: String,
    at: f64,
    paper_h: std::rc::Rc<Cell<f32>>,
}

#[cfg(feature = "tooling")]
impl Render for SceneView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let scene = scenes::build(&self.scene, self.at, scale, &self.paper_h).unwrap_or_else(|| {
            eprintln!("no scene called {}", self.scene);
            std::process::exit(2)
        });
        div().size_full().child(scene)
    }
}

#[cfg(feature = "tooling")]
fn snap_scene(out: &str, scene: &str, w: f32, h: f32, at: f64) -> anyhow::Result<()> {
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, std::sync::Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(|cx| cx.text_system().add_fonts(fonts()))?;
    CLOCK.with(|c| c.set(None));
    let scene = scene.to_string();
    let win = cx.open_window(size(px(w), px(h)), move |_, cx| {
        cx.new(|_| SceneView { scene, at, paper_h: std::rc::Rc::new(Cell::new(173.0)) })
    })?;
    cx.run_until_parked();
    // The first frame measures the paper; the second places it.
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    cx.run_until_parked();
    let shot = cx.capture_screenshot(win.into())?;
    shot.save(out)?;
    Ok(())
}

// MARK: - Live

/// GPUI makes its floating panel the key window when it opens, even when asked not to focus it,
/// so key presses meant for the app in front would land in the panel. The real Iris panel takes
/// keys on purpose (Tab, Return); a test panel opened beside someone's work must not. Unless
/// `IRIS_TAKE_KEYS` is set, GPUI's panel class is told it can never be the key window.
#[cfg(target_os = "macos")]
pub fn refuse_keys() {
    use objc2::ffi::class_replaceMethod;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::sel;
    extern "C-unwind" fn no(_: &AnyObject, _: Sel) -> Bool {
        Bool::NO
    }
    // The panels and the slip (`GPUIPanel`), and Settings (`GPUIWindow`).
    for name in [c"GPUIPanel", c"GPUIWindow"] {
        let Some(class) = AnyClass::get(name) else { continue };
        unsafe {
            let imp: Imp = std::mem::transmute(no as extern "C-unwind" fn(&AnyObject, Sel) -> Bool);
            class_replaceMethod(class as *const AnyClass as *mut AnyClass, sel!(canBecomeKeyWindow), imp, c"c@:".as_ptr());
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn refuse_keys() {}

fn live(origin: Option<(f32, f32)>) {
    CLOCK.with(|c| c.set(Some(Instant::now())));
    if std::env::var("IRIS_TAKE_KEYS").is_err() {
        refuse_keys();
    }
    gpui_platform::application().run(move |cx: &mut App| {
        // Like the Swift app: no Dock icon, and the panel never activates Iris.
        cx.set_activation_policy(ActivationPolicy::Accessory);
        cx.text_system().add_fonts(fonts()).unwrap();
        let (x, y) = origin.unwrap_or((200.0, 200.0));
        let bounds = Bounds { origin: point(px(x), px(y)), size: size(px(672.0), px(340.0)) };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            // The Swift panel takes keys without activating Iris; set IRIS_TAKE_KEYS to do the same.
            focus: std::env::var("IRIS_TAKE_KEYS").is_ok(),
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            // GPUI draws a window that isn't in front at 30 frames a second. The panel never
            // is (it floats over the app you're writing in), so it opts out.
            inactive_frame_interval: None,
            ..Default::default()
        };
        let states = model::states();
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| PanelView { panel: states[1].panel.clone(), live: true, due: None, anim: Default::default(), focus: cx.focus_handle() });
            let focus = view.read(cx).focus.clone();
            window.focus(&focus, cx);
            view
        })
        .unwrap();
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}

/// Starts the app the way every live mode does: no Dock icon, the fonts and the stand-in
/// app's icon found before any window needs them.
pub fn start(cx: &mut App) {
    cx.set_activation_policy(ActivationPolicy::Accessory);
    always_light();
    cx.text_system().add_fonts(fonts()).unwrap();
    prefetch_icon("com.tinyspeck.slackmacgap");
    for family in ["Iris Mono 430", "Iris Mono 450", "Iris Mono 560", "Caveat Brush", "Rubik Doodle Shadow", "Menlo", ".SystemUIFont"] {
        let _ = cx.text_system().resolve_font(&font(family));
    }
    cx.on_window_closed(|_, id| {
        riso::forget_window(id);
        confirm::forget_window(id);
    })
    .detach();
}

/// Iris is paper in Dark Mode too: its windows' title bar buttons and menus draw light, as the
/// Swift app's do (`NSApp.appearance = .aqua`).
fn always_light() {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject};
        use objc2_foundation::NSString;
        let (Some(app_class), Some(appearance_class)) = (AnyClass::get(c"NSApplication"), AnyClass::get(c"NSAppearance")) else { return };
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        let name = NSString::from_str("NSAppearanceNameAqua");
        let aqua: *mut AnyObject = msg_send![appearance_class, appearanceNamed: &*name];
        if !app.is_null() && !aqua.is_null() {
            let _: () = msg_send![app, setAppearance: aqua];
        }
    }
}

/// `--live-slip [x y]`: the slip on its own at `x, y` (screen points); the app quits when it goes.
fn live_slip(origin: Option<(f32, f32)>) {
    if std::env::var("IRIS_TAKE_KEYS").is_err() {
        refuse_keys();
    }
    gpui_platform::application().run(move |cx: &mut App| {
        start(cx);
        let (x, y) = origin.unwrap_or((200.0, 200.0));
        live_slip::open(point(px(x), px(y)), None, cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}

/// A setup state, live: the page whole, the window fitted to it.
pub(crate) struct SetupView {
    pub(crate) state: setup::SetupState,
}

impl Render for SetupView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        riso::let_go_of_last_frame(window);
        let page = setup::window(&self.state, None, window.scale_factor());
        let fit = canvas(|_, _, _| (), |b, _, window, _| {
            if (window.bounds().size.height - b.size.height).abs() > px(0.5) {
                window.resize(size(b.size.width, b.size.height));
            }
        }).absolute().top_0().left_0().size_full();
        div().size_full().child(div().relative().child(page).child(fit))
    }
}

thread_local! {
    /// The slip hanging from the menu bar flower, while it's up.
    static SLIP: RefCell<Option<WindowHandle<live_slip::SlipView>>> = const { RefCell::new(None) };
}

/// Settings at `page`: on the test display when `IRIS_TEST_DISPLAY` says where it is (tests
/// never put a window on someone's screen), else near the top left of the main screen.
pub fn open_settings(page: settings::Page, cx: &mut App) {
    thread_local! { static OPEN: RefCell<Option<WindowHandle<live_settings::SettingsView>>> = const { RefCell::new(None) }; }
    // Already open: turned to the page and brought forward, never a second window.
    let open = OPEN.with(|o| o.borrow().clone());
    let reused = open.is_some_and(|h| h.update(cx, |v, window, cx| {
        v.page = page;
        window.activate_window();
        cx.notify();
    }).is_ok());
    if !reused {
        let (x, y) = test_display().map(|(x, y)| (x + 40.0, y + 40.0)).unwrap_or((240.0, 120.0));
        let h = live_settings::open(point(px(x as f32), px(y as f32)), page, cx);
        OPEN.with(|o| *o.borrow_mut() = h);
    }
    // Settings is a window you work in: Iris comes to the front with it (never under test).
    if !textbridge::under_test() {
        cx.activate(true);
    }
}

/// A click on the flower (its frame in screen points): hangs the slip from it, centred under
/// it with its tail pointing up, 2 points below the bar; another click puts it away.
pub fn toggle_slip(frame: [f64; 4], cx: &mut App) {
    if let Some(h) = SLIP.with(|s| s.borrow_mut().take())
        && h.update(cx, |_, window, _| window.remove_window()).is_ok()
    {
        return;
    }
    let [x, y, w, hh] = frame;
    let width = slip::WIDTH as f64 + 36.0;
    let at = point(px((x + w / 2.0 - width / 2.0) as f32), px((y + hh + 2.0) as f32));
    let h = live_slip::open(at, Some(std::rc::Rc::new(|_: &mut Window, cx: &mut App| open_settings(settings::Page::General, cx))), cx);
    SLIP.with(|s| *s.borrow_mut() = h);
}

/// `--menubar`: the flower in the menu bar; a click hangs the slip from it. `IRIS_SLIP_ANCHOR=x,y,w,h`
/// hangs it from that frame instead (the test display's), so a test never puts it on someone's
/// screen.
fn menubar_app() {
    app::run();
}

/// Calls the panel as the shortcut does: the text box's corner at the pointer (on the test
/// display, `IRIS_TEST_DISPLAY` + (220, 360) points, when a test says where it is).
pub fn call_panel(cx: &mut App) {
    let (x, y) = test_display().map(|(x, y)| (x + 220.0, y + 360.0)).unwrap_or_else(menubar::pointer);
    let (bx, by) = opening::BOX_AT;
    open_opening(point(px((x - bx as f64) as f32), px((y - by as f64) as f32)), cx);
}

/// The live moment's window at `origin` (screen points).
fn open_opening(origin: Point<Pixels>, cx: &mut App) {
    open_opening_with(origin, cx, live::OpeningView::new);
}

/// The live moment's window at `origin`, its view made by `make`.
pub fn open_opening_with(origin: Point<Pixels>, cx: &mut App, make: impl FnOnce(&mut Context<live::OpeningView>) -> live::OpeningView + 'static) -> Option<WindowHandle<live::OpeningView>> {
    open_opening_keys(origin, takes_keys(), cx, make)
}

/// The moment's window, taking the keyboard or not (the flower wilting after a Replace never
/// does: the paste's keys must reach the app).
pub fn open_opening_keys(origin: Point<Pixels>, keys: bool, cx: &mut App, make: impl FnOnce(&mut Context<live::OpeningView>) -> live::OpeningView + 'static) -> Option<WindowHandle<live::OpeningView>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(opening::SCENE.0), px(opening::SCENE.1)) })),
        titlebar: None,
        focus: keys,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        inactive_frame_interval: None,
        ..Default::default()
    };
    cx.open_window(options, move |window, cx| {
        no_system_shadow(window);
        let view = cx.new(make);
        if keys {
            let focus = view.read(cx).focus.clone();
            window.focus(&focus, cx);
            // Once it's on screen: a window not shown yet can't take the keyboard.
            #[cfg(windows)]
            window.on_next_frame(|window, _| take_keyboard(window));
        }
        view
    })
    .ok()
}

/// Tags the window's Metal layer as sRGB, so macOS converts Iris's colours for the screen
/// they're on, as it does for the Swift app. GPUI leaves the layer untagged (unless asked for
/// Display P3), and an untagged layer's values go to the screen as they are: on a wide-colour
/// screen (every recent Mac's own) sRGB pinks and blues would show more saturated than Iris's.
pub fn color_managed(window: &Window) {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(h) = HasWindowHandle::window_handle(window) else { return };
        let RawWindowHandle::AppKit(a) = h.as_raw() else { return };
        let view = a.ns_view.as_ptr() as *mut AnyObject;
        let Some(srgb) = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })) else { return };
        unsafe {
            let layer: *mut AnyObject = msg_send![view, layer];
            if !layer.is_null() {
                let _: () = msg_send![layer, setColorspace: &*srgb];
                if std::env::var("IRIS_DEBUG").is_ok() {
                    let now: *const CGColorSpace = msg_send![layer, colorspace];
                    let name = if now.is_null() { None } else { CGColorSpace::name(Some(&*now)) };
                    eprintln!("colour: the layer's colour space is {:?}", name.map(|n| n.to_string()));
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Turns off the shadow and edge macOS draws round a window. Iris's see-through windows (the
/// panel, the slip, the messages) draw their paper's own shadow; macOS's would outline the whole
/// window's rectangle, a dark frame round the paper.
pub fn no_system_shadow(window: &Window) {
    color_managed(window);
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(h) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::AppKit(a) = h.as_raw()
        {
            let view = a.ns_view.as_ptr() as *mut AnyObject;
            unsafe {
                let w: *mut AnyObject = msg_send![view, window];
                if !w.is_null() {
                    let _: () = msg_send![w, setHasShadow: false];
                    // GPUI makes every window titled, even one without a title bar, and macOS
                    // draws a titled window's light top edge: a hairline across the top of the
                    // see-through window. Borderless, as Swift's floating notes are (still a
                    // panel that never activates Iris; GPUI's panel can still take the keys).
                    const BORDERLESS_NONACTIVATING: usize = 1 << 7;
                    let _: () = msg_send![w, setStyleMask: BORDERLESS_NONACTIVATING];
                }
            }
        }
    }
    // Windows 11 draws a thin border round every window, even a see-through one, and a frame
    // of its own (a light veil, a top edge and a shadow) round the whole window, most of which
    // is see-through room for the flower and the fold: a grey box round the panel. Neither here.
    #[cfg(windows)]
    if let Some(hwnd) = win32_handle(window) {
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_NCRENDERING_POLICY};
        let none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
        let no_frame = DWMNCRP_DISABLED.0;
        unsafe {
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &none as *const u32 as *const _, 4);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY, &no_frame as *const i32 as *const _, 4);
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = window;
}

#[cfg(windows)]
fn win32_handle(window: &Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(windows::Win32::Foundation::HWND(h.hwnd.get() as _)),
        _ => None,
    }
}

/// Windows: brings Iris's window to the front with the keyboard. In a Windows 11 VM the panel
/// opened straight after the shortcut already was in front; this is for when it isn't (an app
/// slow to read). Windows refuses a plain SetForegroundWindow from a program in the background,
/// so Iris joins the front window's input queue for the call, the documented way to pass focus
/// between programs. GPUI's own activation isn't used: it taps
/// Alt to get the same right, and that Alt would reach the app you're writing in.
#[cfg(windows)]
pub fn take_keyboard(window: &Window) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };
    let Some(hwnd) = win32_handle(window) else { return };
    unsafe {
        let front = GetForegroundWindow();
        let theirs = GetWindowThreadProcessId(front, None);
        let ours = GetCurrentThreadId();
        let joined = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if joined {
            let _ = AttachThreadInput(ours, theirs, false);
        }
    }
}

/// Whether Iris's windows take the keyboard: in the app itself yes, never under test (unless
/// `IRIS_TAKE_KEYS` says so).
pub fn takes_keys() -> bool {
    std::env::var("IRIS_TAKE_KEYS").is_ok() || (!textbridge::under_test() && std::env::args().len() == 1)
}

/// Makes `window` let go of the keyboard while staying on screen: taken off screen (which
/// resigns its key status, and Iris isn't the active app, so the keyboard goes back to the app
/// in front) and straight back, in front but without the keyboard.
pub fn give_up_keys(window: &Window) {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::{AnyObject, Bool};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(h) = HasWindowHandle::window_handle(window) else { return };
        let RawWindowHandle::AppKit(a) = h.as_raw() else { return };
        let view = a.ns_view.as_ptr() as *mut AnyObject;
        let w: *mut AnyObject = msg_send![view, window];
        if w.is_null() {
            return;
        }
        let key: Bool = msg_send![w, isKeyWindow];
        if key.as_bool() {
            let _: () = msg_send![w, orderOut: std::ptr::null_mut::<AnyObject>()];
            let _: () = msg_send![w, orderFront: std::ptr::null_mut::<AnyObject>()];
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Where `window`'s top left is, in screen points from the top left of the first screen (GPUI
/// only gives a window's place on its own display).
pub fn window_origin(window: &Window) -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSScreen;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let h = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(a) = h.as_raw() else { return None };
        let view = a.ns_view.as_ptr() as *mut AnyObject;
        let ns_window: *mut AnyObject = unsafe { msg_send![view, window] };
        if ns_window.is_null() {
            return None;
        }
        let frame: objc2_foundation::NSRect = unsafe { msg_send![ns_window, frame] };
        let top = MainThreadMarker::new().and_then(|mtm| NSScreen::screens(mtm).firstObject().map(|s| s.frame().size.height))?;
        Some((frame.origin.x, top - (frame.origin.y + frame.size.height)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let b = window.bounds();
        Some((f32::from(b.origin.x) as f64, f32::from(b.origin.y) as f64))
    }
}

/// The room on the screen holding a point (its visible frame: no menu bar, no Dock), in screen
/// points from the top left of the first screen: (x, y, w, h).
pub fn visible_frame_at(x: f64, y: f64) -> [f64; 4] {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSScreen;
        if let Some(mtm) = MainThreadMarker::new() {
            let screens = NSScreen::screens(mtm);
            let top = screens.firstObject().map_or(0.0, |s| s.frame().size.height);
            let flip = |r: objc2_foundation::NSRect| [r.origin.x, top - (r.origin.y + r.size.height), r.size.width, r.size.height];
            let mut first = None;
            for s in screens.iter() {
                let f = flip(s.frame());
                first.get_or_insert(flip(s.visibleFrame()));
                if x >= f[0] && x < f[0] + f[2] && y >= f[1] && y < f[1] + f[3] {
                    return flip(s.visibleFrame());
                }
            }
            if let Some(f) = first {
                return f;
            }
        }
    }
    // Windows: the work area (the screen less the taskbar) of the monitor at the point.
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        unsafe {
            let m = MonitorFromPoint(POINT { x: x as i32, y: y as i32 }, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if GetMonitorInfoW(m, &mut info).as_bool() {
                let r = info.rcWork;
                let s = screen_scale_at(r.left, r.top);
                return [r.left as f64 / s, r.top as f64 / s, (r.right - r.left) as f64 / s, (r.bottom - r.top) as f64 / s];
            }
        }
    }
    let _ = (x, y);
    [0.0, 0.0, 1920.0, 1080.0]
}

/// Windows: the scale (DPI / 96) of the monitor at a screen pixel.
#[cfg(windows)]
pub fn screen_scale_at(x: i32, y: i32) -> f64 {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    unsafe {
        let m = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        dx.max(96) as f64 / 96.0
    }
}

/// Whether a point (screen points) is on the test display (`IRIS_TEST_DISPLAY`, 1920×1080).
pub fn on_test_display(x: f64, y: f64) -> bool {
    test_display().is_some_and(|(dx, dy)| x >= dx && y >= dy && x < dx + 1920.0 && y < dy + 1080.0)
}

/// `IRIS_TEST_DISPLAY=x,y` (physical pixels, as the Rune copy takes it), in points.
pub fn test_display() -> Option<(f64, f64)> {
    let v = std::env::var("IRIS_TEST_DISPLAY").ok()?;
    let n: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
    (n.len() == 2).then(|| (n[0] / 2.0, n[1] / 2.0))
}

/// `--live-panel [x y]`: the ⌃G moment live, the scene's top left at `x, y` (screen points).
fn live_opening(origin: Option<(f32, f32)>) {
    if std::env::var("IRIS_TAKE_KEYS").is_err() {
        refuse_keys();
    }
    gpui_platform::application().run(move |cx: &mut App| {
        start(cx);
        let (x, y) = origin.unwrap_or((200.0, 200.0));
        open_opening(point(px(x), px(y)), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name);
    // `scripts/verify`'s modes, sealed on the test display (`driven.rs`).
    if driven::on() {
        if arg("--practice-box").is_some() {
            return driven::practice_box();
        }
        if let Some(i) = arg("--setup-preview") {
            return driven::setup_preview(args.get(i + 1).cloned().unwrap_or("1-allow".into()));
        }
        return driven::run();
    }
    // Windows tooling: what the language check sees in each draft of a JSON list (the script's
    // share, each word and whether Windows' dictionaries know it), to tune it against the Mac's.
    #[cfg(windows)]
    if let Some(i) = arg("--language-features") {
        let drafts: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&args[i + 1]).unwrap()).unwrap();
        let dict = spelling_win::Dictionaries::open();
        let out: Vec<serde_json::Value> = drafts.iter().map(|t| {
            let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
            let latin = letters.iter().filter(|c| (**c as u32) < 0x250).count();
            let words: Vec<serde_json::Value> = t.split(|c: char| !c.is_alphabetic() && c != '\'')
                .filter(|w| w.chars().count() > 1)
                .map(|w| serde_json::json!([w, dict.knows(w)])).collect();
            serde_json::json!({"letters": letters.len(), "latin": latin, "words": words, "windows": spelling_win::looks_foreign(t, &dict)})
        }).collect();
        std::fs::write(&args[i + 2], serde_json::to_string(&out).unwrap()).unwrap();
        return;
    }
    if arg("--list-states").is_some() {
        for (i, st) in model::states().iter().enumerate() {
            println!("s{:02}\t{}", i + 1, st.caption);
        }
        return;
    }
    if let Some(at) = arg("--snap") {
        #[cfg(feature = "tooling")]
        if args.get(at + 1).is_some_and(|a| a.ends_with(".png")) {
            let val = |name: &str| arg(name).and_then(|i| args.get(i + 1)).cloned();
            let (w, h) = val("--size").and_then(|s| { let (a, b) = s.split_once('x')?; Some((a.parse().ok()?, b.parse().ok()?)) }).unwrap_or((900.0, 460.0));
            let t = val("--at").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            snap_scene(&args[at + 1], &val("--scene").unwrap_or("opening-full".into()), w, h, t).unwrap();
            return;
        }
        #[cfg(feature = "tooling")]
        {
            let dir = args.get(at + 1).cloned().unwrap_or("/tmp/iris-gpui/gpui".into());
            snap(&dir, 2.0).unwrap();
            return;
        }
        #[cfg(not(feature = "tooling"))]
        {
            let _ = at;
            eprintln!("--snap needs the tooling feature");
            std::process::exit(2);
        }
    }
    if arg("--menubar").is_some() {
        menubar_app();
        return;
    }
    if let Some(i) = arg("--live-settings") {
        let origin = (|| Some((args.get(i + 1)?.parse::<f32>().ok()?, args.get(i + 2)?.parse::<f32>().ok()?)))();
        let page = args.get(i + 3).and_then(|n| live_settings::page_named(n)).unwrap_or(settings::Page::General);
        if std::env::var("IRIS_TAKE_KEYS").is_err() {
            refuse_keys();
        }
        gpui_platform::application().run(move |cx: &mut App| {
            start(cx);
            // The settings saved in `IRIS_HOME`, the brain told of changes, the control socket.
            app::serve_settings(cx);
            let (x, y) = origin.unwrap_or((240.0, 120.0));
            live_settings::open(point(px(x), px(y)), page, cx);
            cx.on_window_closed(|cx, _| cx.quit()).detach();
        });
        return;
    }
    if let Some(i) = arg("--live-setup") {
        // `--live-setup <state> [x y]`: a setup state in a window of its own (as `Iris
        // --setup-preview` opens one), its height fitted to the page.
        let id = args.get(i + 1).cloned().unwrap_or("1-allow".into());
        let origin = (|| Some((args.get(i + 2)?.parse::<f32>().ok()?, args.get(i + 3)?.parse::<f32>().ok()?)))().unwrap_or((240.0, 120.0));
        if std::env::var("IRIS_TAKE_KEYS").is_err() {
            refuse_keys();
        }
        gpui_platform::application().run(move |cx: &mut App| {
            start(cx);
            let Some(state) = setup::states().into_iter().find(|s| s.id == id) else { cx.quit(); return };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(origin.0), px(origin.1)), size: size(px(setup::WIDTH), px(680.0)) })),
                titlebar: Some(TitlebarOptions { title: Some("Set up Iris".into()), appears_transparent: true, traffic_light_position: None }),
                focus: std::env::var("IRIS_TAKE_KEYS").is_ok(),
                show: true,
                is_resizable: false,
                ..Default::default()
            };
            let _ = cx.open_window(options, move |window, cx| {
                color_managed(window);
                cx.new(|_| SetupView { state })
            });
            cx.on_window_closed(|cx, _| cx.quit()).detach();
        });
        return;
    }
    if let Some(i) = arg("--live-slip") {
        let origin = (|| Some((args.get(i + 1)?.parse().ok()?, args.get(i + 2)?.parse().ok()?)))();
        live_slip(origin);
        return;
    }
    if let Some(i) = arg("--live-panel") {
        let origin = (|| Some((args.get(i + 1)?.parse().ok()?, args.get(i + 2)?.parse().ok()?)))();
        live_opening(origin);
        return;
    }
    if let Some(at) = arg("--live") {
        let origin = (|| Some((args.get(at + 1)?.parse().ok()?, args.get(at + 2)?.parse().ok()?)))();
        live(origin);
        return;
    }
    // No mode: Iris itself.
    app::run();
}

/// Windows: Settings and setup have no title bar (their paper runs to the top edge, as on the
/// Mac), so a strip along the top drags the window and an ink brush X at the top right closes
/// it. Both are Windows' own controls through GPUI's window control areas (snapping, a
/// double-click, Alt+F4 all behave). Nothing on the Mac, where the traffic lights are.
pub fn windows_chrome(scale: f32) -> Option<AnyElement> {
    if keys::is_mac() {
        return None;
    }
    let cross = crate::toast::brush_cross(31, 12.0, 12.0, scale);
    Some(div().absolute().top_0().left_0().w_full().h(px(44.0))
        .child(div().id("window-drag").absolute().top_0().left_0().right(px(52.0)).h(px(30.0))
            .window_control_area(WindowControlArea::Drag))
        .child(div().id("window-close").absolute().top(px(6.0)).right(px(8.0)).size(px(32.0))
            .flex().items_center().justify_center().cursor_pointer()
            // Over Settings' own title band, which starts a window move on any press: the X
            // keeps its clicks.
            .occlude()
            .window_control_area(WindowControlArea::Close).child(cross))
        .into_any_element())
}
