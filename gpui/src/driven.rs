//! `iris --mock-answers --on-display <screen>`: this build of Iris, sealed on the test display,
//! driven end to end by `scripts/verify` while the person keeps working (the Swift app's
//! `Driven.swift` and `OnDisplay.swift`, ported). Canned answers (`mock.rs`), no shortcuts
//! registered, no menu bar flower, no brain, no control socket (four journeys run side by side).
//!
//! The run clicks the panel's tabs and buttons, the slip's rows and Settings' controls itself,
//! through the accessibility tree. What a person does with a global key press it can't do in the
//! background, so it asks here instead, one line per command on stdin, each entering where that
//! key press's handler does (the control socket's commands, `app::command`):
//!
//!     open-panel <pid>   the open shortcut, reading the text box of the app with this pid
//!     quick-fix <pid>    the Quick Fix shortcut, the same way
//!     key <name>         a key in the open panel (tab, shift-tab, escape, return, cmd-r, 1…5)
//!     slip               the menu bar flower clicked: the slip hangs from a stand-in flower
//!     settings <page>    Settings asked for at a page
//!     engine <state>     the model's state as the slip and panel hear it (idle, loading, ready)
//!     snapshot <path>    the open panel, slip or Settings drawn again offscreen (GPUI's renderer
//!                        on the window's last scene, not the window server's composite)
//!     quit
//! and any other control-socket command (`pretend`, `hover`, `state`…).
//!
//! Answers on stdout ("iris: ok <command>", "iris: error <why>"), with what happened as it
//! happens ("iris: panel settled …", "iris: replaced box", "iris: note …"): the run waits on
//! these lines, never on a sleep.
//!
//! Sealed, so a check can never cause a problem of its own:
//! - every window goes on the named screen (`IRIS_TEST_DISPLAY`, set from it); it quits (3) if
//!   that screen is missing or goes away;
//! - it never comes to the front or takes the keyboard (`refuse_keys`), never touches the
//!   clipboard or posts a key press (`textbridge::under_test`), and reads only the app it's told to;
//! - settings start at their defaults (as a Mac that has finished setup) in a folder of its own,
//!   deleted when it quits: nothing of this Mac's is read or written;
//! - it quits when the run that started it ends (4: `IRIS_VERIFY_PARENT`, or stdin closing),
//!   after `IRIS_VERIFY_MAX_SECONDS` (5), and cleanly on SIGTERM (0);
//! - anything that would reach the person's screen or keyboard anyway is reported as a tripwire
//!   ("iris: tripwire …"), which fails the run's step.
//!
//! `--practice-box` (the run's stand-in text box, a plain AppKit text view) and `--setup-preview
//! <state>` (a setup page with its snapshot) are launched the same way, with `--on-display` last.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use gpui::*;

use crate::live::OpeningView;
use crate::live_settings::SettingsView;
use crate::live_slip::SlipView;

/// The screen named after `--on-display`, if asked.
pub fn screen_name() -> Option<&'static str> {
    static NAME: OnceLock<Option<String>> = OnceLock::new();
    NAME.get_or_init(|| {
        let args: Vec<String> = std::env::args().collect();
        let i = args.iter().position(|a| a == "--on-display")?;
        args.get(i + 1).cloned()
    })
    .as_deref()
}

/// Whether this Iris runs sealed on a named screen.
pub fn on() -> bool {
    screen_name().is_some()
}

/// One line for the run, which waits on these instead of sleeping.
pub fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "iris: {line}");
    let _ = out.flush();
}

/// Something that must never happen on the test display: refused, and reported.
pub fn trip(what: &str) {
    say(&format!("tripwire {what}"));
}

/// This process's settings folder, deleted on the way out.
fn home() -> PathBuf {
    std::env::temp_dir().join(format!("iris-gpui-driven-{}", std::process::id()))
}

/// Quits at once with `status`, saying why (3: the screen went, 4: the run ended, 5: out of time).
pub fn quit_now(status: i32, why: &str) -> ! {
    say(&format!("quit {status} {why}"));
    eprintln!("Iris --on-display: {why}");
    let _ = std::fs::remove_dir_all(home());
    unsafe { sys::_exit(status) }
}

mod sys {
    unsafe extern "C" {
        pub fn _exit(status: i32) -> !;
        pub fn kill(pid: i32, sig: i32) -> i32;
        pub fn signal(sig: i32, handler: usize) -> usize;
    }
    pub const SIGTERM: i32 = 15;
}

static ASKED_TO_STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_term(_: i32) {
    ASKED_TO_STOP.store(true, Ordering::SeqCst);
}

/// Whether a process is still there.
fn alive(pid: i32) -> bool {
    unsafe { sys::kill(pid, 0) == 0 }
}

/// Before anything else: finds the screen (or quits 3), points every window at it, seals the
/// settings, and starts the watch on the run (its pid, its clock) and on SIGTERM.
pub fn prepare() {
    let name = screen_name().unwrap_or("");
    let Some(frame) = screen_frame(name) else {
        quit_now(3, &format!("no screen named “{name}”: open it first (claude-display up)"));
    };
    let [x, y, w, _] = frame;
    let _ = std::fs::create_dir_all(home());
    // Single-threaded still: nothing else reads the environment yet.
    unsafe {
        // `test_display()` takes physical pixels, as the Rune copy did.
        std::env::set_var("IRIS_TEST_DISPLAY", format!("{},{}", x * 2.0, y * 2.0));
        // The slip hangs from a stand-in flower at the top middle of the screen.
        std::env::set_var("IRIS_SLIP_ANCHOR", format!("{},{},24,24", x + w / 2.0 - 12.0, y));
        std::env::set_var("IRIS_HOME", home());
        // The slip's garden and General's note demo hold still, so two captures agree.
        std::env::set_var("IRIS_REDUCE_MOTION", "1");
        std::env::remove_var("IRIS_TAKE_KEYS");
        std::env::remove_var("IRIS_CONTROL");
        sys::signal(sys::SIGTERM, on_term as usize);
    }
    let parent = std::env::var("IRIS_VERIFY_PARENT").ok().and_then(|p| p.parse::<i32>().ok());
    let deadline = std::env::var("IRIS_VERIFY_MAX_SECONDS").ok().and_then(|s| s.parse::<f64>().ok())
        .map(|s| Instant::now() + Duration::from_secs_f64(s));
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        if ASKED_TO_STOP.load(Ordering::SeqCst) {
            quit_now(0, "asked to stop");
        }
        if parent.is_some_and(|p| !alive(p)) {
            quit_now(4, "the run that started it ended");
        }
        if deadline.is_some_and(|d| Instant::now() > d) {
            quit_now(5, "it ran past its time limit");
        }
    });
}

/// The named screen's frame, (x, y, w, h) in points from the top left of the first screen.
fn screen_frame(name: &str) -> Option<[f64; 4]> {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSScreen;
        let mtm = MainThreadMarker::new()?;
        let screens = NSScreen::screens(mtm);
        let top = screens.firstObject()?.frame().size.height;
        for s in screens.iter() {
            if s.localizedName().to_string() == name {
                let f = s.frame();
                return Some([f.origin.x, top - (f.origin.y + f.size.height), f.size.width, f.size.height]);
            }
        }
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        None
    }
}

// MARK: - The driven app

/// `--mock-answers --on-display <screen>`: Iris with no shortcuts, flower or brain, driven on stdin.
pub fn run() {
    prepare();
    crate::refuse_keys();
    gpui_platform::application().run(move |cx: &mut App| {
        crate::start(cx);
        // A Mac that has finished setup, as a person's is (in this run's own folder).
        crate::app::change(|s| s.onboarded = true);
        crate::brain::start(cx);
        watch(cx);
        listen(cx);
        say("ready driven");
    });
}

/// Reads commands on stdin (a thread), and runs them on the main thread as they come.
fn listen(cx: &mut App) {
    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
        // stdin closed: the run that started this Iris is gone.
        quit_now(4, "the run's command line closed");
    });
    pump(rx, cx);
}

fn pump(rx: Receiver<String>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(20)).await;
            while let Ok(line) = rx.try_recv() {
                let _ = cx.update(|cx| command(line.trim(), cx));
            }
        }
    })
    .detach();
}

fn command(line: &str, cx: &mut App) {
    let mut words = line.splitn(2, ' ');
    let verb = words.next().unwrap_or("");
    let arg = words.next().unwrap_or("").trim();
    match verb {
        "" => {}
        "open-panel" | "quick-fix" => match arg.parse::<i32>() {
            Ok(pid) if alive(pid) => {
                say(&format!("ok {line}"));
                crate::app::command(line, cx);
            }
            _ => say(&format!("error {verb}: no app with pid {arg}")),
        },
        "key" => {
            if crate::live::key(arg, cx) { say(&format!("ok {line}")) } else { say(&format!("error key {arg}: no panel to take it")) }
        }
        "slip" => {
            let before = windows::<SlipView>(cx);
            crate::app::command("slip", cx);
            say("ok slip");
            // Said once its first frames have drawn in, as the Swift app says it.
            if let Some(h) = windows::<SlipView>(cx).into_iter().find(|h| !before.contains(h)) {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(800)).await;
                    let _ = cx.update(|cx| {
                        if let Some(n) = number(h.into(), cx) {
                            say(&format!("slip open window {n}"));
                        }
                    });
                })
                .detach();
            }
        }
        "engine" => {
            use crate::brain::Engine;
            let engine = match arg {
                "idle" => Engine::Idle,
                "loading" => Engine::Loading,
                "ready" => Engine::Ready(crate::app::settings().model.unwrap_or_else(|| crate::mock::MODEL.into())),
                _ => return say(&format!("error engine: unknown state {arg}")),
            };
            match crate::brain::get(cx) {
                Some(b) => {
                    b.update(cx, |b, cx| {
                        b.engine = engine;
                        cx.notify();
                    });
                    cx.refresh_windows();
                    say(&format!("ok {line}"));
                }
                None => say("error engine: no brain"),
            }
        }
        "snapshot" => match snapshot(arg, cx) {
            Ok(()) => say(&format!("ok {line}")),
            Err(e) => say(&format!("error snapshot: {e}")),
        },
        "quit" => quit_now(0, "asked to"),
        // Everything else the control socket takes.
        _ => {
            let reply = crate::app::command(line, cx);
            if reply == "ok" { say(&format!("ok {line}")) } else { say(&format!("answer {verb}: {reply}")) }
        }
    }
}

/// Every open window showing a `V`.
fn windows<V: 'static>(cx: &App) -> Vec<WindowHandle<V>> {
    cx.windows().into_iter().filter_map(|w| w.downcast::<V>()).collect()
}

/// The window's number (its `NSWindow.windowNumber`: what `screencapture -l` and cua-driver call it).
fn number(h: AnyWindowHandle, cx: &mut App) -> Option<i64> {
    h.update(cx, |_, window, _| window_number(window)).ok().flatten()
}

pub fn window_number(window: &Window) -> Option<i64> {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let h = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(a) = h.as_raw() else { return None };
        let view = a.ns_view.as_ptr() as *mut AnyObject;
        let w: *mut AnyObject = msg_send![view, window];
        if w.is_null() {
            return None;
        }
        let n: isize = msg_send![w, windowNumber];
        Some(n as i64)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        None
    }
}

// MARK: - Snapshots

/// Draws the open panel, else the slip, else Settings again offscreen (`Window::render_to_image`:
/// the window's last scene through GPUI's Metal renderer into a texture of its own) and writes
/// it to `path` as a PNG, making its folder.
fn snapshot(path: &str, cx: &mut App) -> Result<(), String> {
    if path.is_empty() {
        return Err("snapshot <path>".into());
    }
    let target: Option<AnyWindowHandle> = crate::live::open_panel().map(Into::into)
        .filter(|h: &AnyWindowHandle| cx.windows().contains(h))
        .or_else(|| windows::<SlipView>(cx).first().map(|h| (*h).into()))
        .or_else(|| windows::<SettingsView>(cx).first().map(|h| (*h).into()));
    let Some(h) = target else { return Err("nothing open to draw".into()) };
    render(h, path, cx)
}

#[cfg(feature = "tooling")]
fn render(h: AnyWindowHandle, path: &str, cx: &mut App) -> Result<(), String> {
    let image = h.update(cx, |_, window, _| window.render_to_image()).map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    image.save(path).map_err(|e| e.to_string())
}

#[cfg(not(feature = "tooling"))]
fn render(_h: AnyWindowHandle, _path: &str, _cx: &mut App) -> Result<(), String> {
    Err("this build has no snapshot renderer (build with the tooling feature)".into())
}

// MARK: - Watching

/// What the run is told about the panel: "panel settled <tone> <phase> window <n>" once it's
/// held still (the flower open, the answer in or not, the marker landed) for 0.8 s.
#[derive(Default)]
struct Settle {
    seen: Option<(WindowId, usize, &'static str, bool)>,
    since: Option<Instant>,
    said: bool,
}

/// Every 100 ms on the main thread: the tripwires, the screen, the panel settling, and Settings
/// saying it's ready.
fn watch(cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut settle = Settle::default();
        let mut settings_seen: Vec<(WindowId, Instant, bool)> = vec![];
        let mut primed: Vec<WindowId> = vec![];
        let mut tick = 0u64;
        loop {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            tick += 1;
            let _ = cx.update(|cx| {
                if tick % 5 == 0 && screen_frame(screen_name().unwrap_or("")).is_none() {
                    quit_now(3, "its screen went away");
                }
                tripwires();
                let ids: Vec<WindowId> = cx.windows().iter().map(|w| w.window_id()).collect();
                if ids.iter().any(|id| !primed.contains(id)) {
                    prime_accessibility();
                }
                primed = ids;
                panel_settling(&mut settle, cx);
                for h in windows::<SettingsView>(cx) {
                    let id = h.window_id();
                    if !settings_seen.iter().any(|s| s.0 == id) {
                        settings_seen.push((id, Instant::now(), false));
                    }
                }
                settings_seen.retain(|s| cx.windows().iter().any(|w| w.window_id() == s.0));
                for s in settings_seen.iter_mut() {
                    if !s.2 && s.1.elapsed() > Duration::from_millis(1200) {
                        s.2 = true;
                        say("ready Iris Settings");
                    }
                }
            });
        }
    })
    .detach();
}

fn panel_settling(s: &mut Settle, cx: &mut App) {
    let Some(h) = crate::live::open_panel().filter(|h| cx.windows().iter().any(|w| w.window_id() == h.window_id())) else {
        *s = Settle::default();
        return;
    };
    let Ok((sig, t, went)) = h.read_with(cx, |v: &OpeningView, cx| {
        use crate::model::Phase;
        let phase = match v.panel.phase {
            Phase::Preparing => "preparing",
            Phase::Generating(_) => "writing",
            Phase::Done(_) => "done",
            Phase::Failed(_) => "failed",
            #[allow(unreachable_patterns)]
            _ => "other",
        };
        let tone = v.panel.selected;
        let _ = cx;
        ((h.window_id(), tone, phase, v.due.is_some()), v.start.elapsed().as_secs_f64(), v.went.is_some())
    }) else { return };
    if went {
        return;
    }
    if s.seen != Some(sig) {
        *s = Settle { seen: Some(sig), since: Some(Instant::now()), said: false };
        return;
    }
    // The flower is still from 3.05 s in (`opening`: bloom + 2.15) and the window stops drawing
    // at 3.2 s; the marker's slide and a hover take under 0.8 s.
    if s.said || t < 3.4 || s.since.is_none_or(|i| i.elapsed() < Duration::from_millis(800)) {
        return;
    }
    s.said = true;
    let tone = crate::brain::get(cx).and_then(|b| b.read(cx).tones.get(sig.1).map(|m| m.id.clone())).unwrap_or_else(|| sig.1.to_string());
    if let Some(n) = number(h.into(), cx) {
        say(&format!("panel settled {tone} {} window {n}", sig.2));
    }
}

/// Asks this app's windows for their accessibility trees, as a screen reader would, from a
/// thread of its own (the main thread answers). GPUI builds a window's tree only once something
/// asks, and answers that first question with an empty window (it draws a frame to fill it in):
/// a native window has its tree from the start, and the run reads it the moment Iris says a
/// window is ready, so Iris asks first.
fn prime_accessibility() {
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| unsafe {
        use accessibility_sys::*;
        use core_foundation::array::CFArray;
        use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
        use core_foundation::string::CFString;
        let app = AXUIElementCreateApplication(std::process::id() as i32);
        AXUIElementSetMessagingTimeout(app, 2.0);
        let ask = |e: AXUIElementRef, attr: &str| -> Option<CFArray<CFTypeRef>> {
            let mut v: CFTypeRef = std::ptr::null();
            let name = CFString::new(attr);
            (AXUIElementCopyAttributeValue(e, name.as_concrete_TypeRef(), &mut v) == kAXErrorSuccess && !v.is_null())
                .then(|| CFArray::wrap_under_create_rule(v as _))
        };
        if let Some(windows) = ask(app, "AXWindows") {
            for w in windows.iter() {
                let _ = ask(*w as AXUIElementRef, "AXChildren");
            }
        }
        CFRelease(app as CFTypeRef);
    });
}

/// Refuses, and reports, anything reaching the person's screen or keyboard: Iris in front, a
/// window with the keyboard, a window off the test display.
fn tripwires() {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject, Bool};
        use objc2_foundation::NSRect;
        let me = std::process::id() as i32;
        let (Some(app_class), Some(ws_class)) = (AnyClass::get(c"NSApplication"), AnyClass::get(c"NSWorkspace")) else { return };
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        let ws: *mut AnyObject = msg_send![ws_class, sharedWorkspace];
        let front: *mut AnyObject = msg_send![ws, frontmostApplication];
        if !front.is_null() {
            let pid: i32 = msg_send![front, processIdentifier];
            if pid == me {
                trip("Iris came to the front");
                let _: () = msg_send![app, hide: std::ptr::null_mut::<AnyObject>()];
            }
        }
        let Some(ours) = screen_frame(screen_name().unwrap_or("")) else { return };
        let windows: *mut AnyObject = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        for i in 0..count {
            let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
            let visible: Bool = msg_send![w, isVisible];
            if !visible.as_bool() {
                continue;
            }
            let key: Bool = msg_send![w, isKeyWindow];
            if key.as_bool() {
                trip("a window took the keyboard");
                let _: () = msg_send![w, resignKeyWindow];
            }
            let alpha: f64 = msg_send![w, alphaValue];
            let f: NSRect = msg_send![w, frame];
            if alpha <= 0.0 || f.size.width < 2.0 {
                continue;
            }
            // Cocoa's frame (from the bottom left of the first screen) into top-left points.
            let first_h = first_screen_height();
            let (x, y) = (f.origin.x, first_h - (f.origin.y + f.size.height));
            let inside = x < ours[0] + ours[2] && x + f.size.width > ours[0] && y < ours[1] + ours[3] && y + f.size.height > ours[1];
            if !inside {
                trip(&format!("a window opened off the test display at ({x}, {y}, {}, {})", f.size.width, f.size.height));
                let _: () = msg_send![w, orderOut: std::ptr::null_mut::<AnyObject>()];
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn first_screen_height() -> f64 {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    MainThreadMarker::new().and_then(|mtm| NSScreen::screens(mtm).firstObject().map(|s| s.frame().size.height)).unwrap_or(0.0)
}

// MARK: - Setup's preview

/// `--setup-preview <state> --on-display <screen>`: a setup state in its window, in the middle
/// of the screen; once drawn, its snapshot written to `IRIS_VERIFY_SNAPSHOT` and "ready window".
pub fn setup_preview(id: String) {
    prepare();
    crate::refuse_keys();
    gpui_platform::application().run(move |cx: &mut App| {
        crate::start(cx);
        // The keys the settings have, shown as Swift's preview shows them.
        crate::app::tell_keys();
        let Some(state) = crate::setup::states().into_iter().find(|s| s.id == id) else {
            quit_now(2, &format!("no setup state {id}"));
        };
        let (dx, dy) = crate::test_display().unwrap_or((0.0, 0.0));
        let (w, h) = (crate::setup::WIDTH, 680.0);
        let origin = point(px((dx + 960.0) as f32 - w / 2.0), px((dy + 540.0) as f32 - h / 2.0));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(w), px(h)) })),
            titlebar: Some(TitlebarOptions { title: Some("Set up Iris".into()), appears_transparent: true, traffic_light_position: Some(point(px(12.0), px(9.0))) }),
            focus: false,
            show: true,
            is_resizable: false,
            ..Default::default()
        };
        let Ok(handle) = cx.open_window(options, move |window, cx| {
            crate::color_managed(window);
            cx.new(|_| crate::SetupView { state })
        }) else { quit_now(2, "setup's window didn't open") };
        watch(cx);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(1200)).await;
            let _ = cx.update(|cx| {
                if let Ok(path) = std::env::var("IRIS_VERIFY_SNAPSHOT")
                    && let Err(e) = render(handle.into(), &path, cx)
                {
                    say(&format!("error snapshot: {e}"));
                }
                say("ready window");
            });
        })
        .detach();
    });
}

// MARK: - The practice box

/// What the box holds when it opens (`OnboardingModel.practiceSample`'s kind of draft).
const PRACTICE: &str = "hey, i have checked the PR and its look fine but one thing need to change before merge";

/// `--practice-box --on-display <screen>`: the run's text box, standing in for the app you type
/// in. A plain AppKit window with a text view, in an app of its own that never comes forward,
/// in a spot of its own (`IRIS_VERIFY_SLOT`, 0…3) so journeys run side by side.
pub fn practice_box() {
    prepare();
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::rc::Retained;
        use objc2::runtime::{AnyClass, AnyObject, Bool};
        use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
        let class = |n: &std::ffi::CStr| AnyClass::get(n).unwrap_or_else(|| quit_now(2, "AppKit isn't there"));
        let app: *mut AnyObject = msg_send![class(c"NSApplication"), sharedApplication];
        // Prohibited: it never comes forward, not even at launch.
        let _: Bool = msg_send![app, setActivationPolicy: 2isize];
        let mtm = objc2::MainThreadMarker::new().unwrap_or_else(|| quit_now(2, "not on the main thread"));
        let name = screen_name().unwrap_or("");
        let screen = objc2_app_kit::NSScreen::screens(mtm).iter().find(|s| s.localizedName().to_string() == name)
            .unwrap_or_else(|| quit_now(3, "its screen went away"));
        let v = screen.visibleFrame();
        let slot: f64 = std::env::var("IRIS_VERIFY_SLOT").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let frame = NSRect::new(
            NSPoint::new(v.origin.x + 40.0 + (slot % 2.0) * 960.0, v.origin.y + 60.0 + ((slot / 2.0).floor() % 2.0) * 540.0),
            NSSize::new(600.0, 120.0),
        );
        let window: *mut AnyObject = msg_send![class(c"NSWindow"), alloc];
        let window: *mut AnyObject = msg_send![window, initWithContentRect: frame, styleMask: 1usize, backing: 2usize, defer: false];
        let title = NSString::from_str("Iris practice box");
        let _: () = msg_send![window, setTitle: &*title];
        let scroll: Option<Retained<AnyObject>> = msg_send![class(c"NSTextView"), scrollableTextView];
        let scroll = scroll.unwrap_or_else(|| quit_now(2, "no text view"));
        let text: *mut AnyObject = msg_send![&*scroll, documentView];
        let sample = NSString::from_str(PRACTICE);
        let _: () = msg_send![text, setString: &*sample];
        let font: *mut AnyObject = msg_send![class(c"NSFont"), systemFontOfSize: 15.0f64];
        let _: () = msg_send![text, setFont: font];
        let _: () = msg_send![text, setTextContainerInset: NSSize::new(10.0, 12.0)];
        let _: () = msg_send![window, setContentView: &*scroll];
        let outer: NSRect = msg_send![window, frameRectForContentRect: frame];
        let _: () = msg_send![window, setFrame: outer, display: true];
        let _: () = msg_send![window, orderFrontRegardless];
        // The box has the app's focus (what Accessibility reads) without the app coming forward.
        let _: Bool = msg_send![window, makeFirstResponder: text];
        let n: isize = msg_send![window, windowNumber];
        say(&format!("ready practice-box window {n}"));
        let _: () = msg_send![app, run];
    }
    quit_now(0, "the box closed");
}
