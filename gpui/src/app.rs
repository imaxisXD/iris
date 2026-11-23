//! Iris, the app: the flower in the menu bar, the slip hanging from it, Settings and the panel,
//! with the settings saved on this Mac. `iris` with no arguments runs it.
//!
//! Tests drive it without touching anyone's screen through a control socket, opened only when
//! `IRIS_CONTROL=1` is set (`<home>/control.sock`): one command a line, one answer a line.
//! - `slip`: hang the slip from the flower (or `IRIS_SLIP_ANCHOR`), or put it away.
//! - `settings [page]`: open Settings.
//! - `panel`: the panel over a stand-in draft.
//! - `open-panel <pid>`: the open shortcut, reading the text box of the app with this pid.
//! - `quick-fix <pid>`: the Quick Fix shortcut, the same way.
//! - `key <name>`: a key in the open panel (tab, shift-tab, 1…5, return, escape, cmd-r).
//! - `state`: the settings as saved, as JSON.
//! - `quit`.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use gpui::*;

use crate::store::{self, Settings};

thread_local! {
    /// The settings as they stand (saved on every change).
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
}

thread_local! {
    /// Another app holds the open shortcut (`openShortcutTaken`).
    static TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_shortcut_taken(t: bool) {
    TAKEN.with(|x| x.set(t));
}

pub fn shortcut_taken() -> bool {
    TAKEN.with(|x| x.get())
}

/// The settings as they stand.
pub fn settings() -> Settings {
    SETTINGS.with(|s| s.borrow().clone())
}

/// Changes the settings and saves them.
pub fn change(f: impl FnOnce(&mut Settings)) {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        f(&mut s);
        if let Err(e) = s.save() {
            eprintln!("settings: couldn't save: {e}");
        }
    });
    tell_keys();
}

/// Tells the slip which keys the shortcuts are, as the settings have them: on every load and
/// change, so the slip, setup and Settings never show different keys.
pub fn tell_keys() {
    let s = settings();
    crate::slip::set_keys(shortcut_caps(s.open_shortcut), s.quick_fix_shortcut.map(shortcut_caps));
}

/// A shortcut's keys as caps, the way this system prints them (`keys::caps`).
pub fn shortcut_caps(k: store::KeyCombo) -> Vec<String> {
    crate::keys::caps(k)
}

/// The Carbon key code of a key as GPUI names it (`a`, `5`, `space`, `f3`…), for a recorded
/// shortcut; none for keys a shortcut can't use (arrows, punctuation).
pub fn key_code(key: &str) -> Option<u32> {
    let named = [("space", 49), ("enter", 36), ("tab", 48), ("escape", 53), ("backspace", 51), ("f1", 122), ("f2", 120), ("f3", 99), ("f4", 118), ("f5", 96)];
    if let Some((_, c)) = named.iter().find(|(n, _)| *n == key) {
        return Some(*c);
    }
    let upper = key.to_uppercase();
    (0..128).find(|c| key.chars().count() == 1 && key_name(*c) == upper)
}

/// A Carbon key code's letter or name.
pub fn key_name(code: u32) -> String {
    const LETTERS: [(u32, &str); 36] = [(0, "A"), (11, "B"), (8, "C"), (2, "D"), (14, "E"), (3, "F"), (5, "G"), (4, "H"), (34, "I"), (38, "J"),
        (40, "K"), (37, "L"), (46, "M"), (45, "N"), (31, "O"), (35, "P"), (12, "Q"), (15, "R"), (1, "S"), (17, "T"), (32, "U"), (9, "V"),
        (13, "W"), (7, "X"), (16, "Y"), (6, "Z"), (29, "0"), (18, "1"), (19, "2"), (20, "3"), (21, "4"), (23, "5"), (22, "6"), (26, "7"), (28, "8"), (25, "9")];
    let named = [(49, "Space"), (36, "↩"), (48, "⇥"), (53, "esc"), (51, "⌫"), (96, "F5"), (122, "F1"), (120, "F2"), (99, "F3"), (118, "F4")];
    LETTERS.iter().chain(named.iter()).find(|(c, _)| *c == code).map(|(_, n)| n.to_string()).unwrap_or_else(|| format!("key {code}"))
}

/// Starts Iris.
pub fn run() {
    // Windows: Iris opened again while it runs (the Start menu): the running one shows Settings.
    #[cfg(windows)]
    if crate::hotkey::already_running() {
        return;
    }
    // Iris's panel and slip take the keyboard (Tab, Return, ↑ ↓) without bringing Iris to the
    // front, as the Swift app's do; under test they never do (someone may be typing).
    if crate::textbridge::under_test() && std::env::var("IRIS_TAKE_KEYS").is_err() {
        crate::refuse_keys();
    }
    gpui_platform::application().run(move |cx: &mut App| {
        SETTINGS.with(|s| *s.borrow_mut() = Settings::load());
    tell_keys();
        // The first start writes the defaults down, so the brain reads the same settings.
        if !store::file().exists() {
            change(|_| {});
        }
        crate::start(cx);
        crate::brain::start(cx);
        // Once the brain has said which models are on disk: setup at its step if it isn't
        // complete, else the model loads in the background so the first check is fast (never a
        // download: the model is on disk, or setup would be open). Event-driven via observe.
        let finish_boot = |cx: &mut App| {
            if !crate::live_setup::progress(cx).is_complete() {
                crate::live_setup::show(cx);
            } else if crate::app::settings().load_at_launch {
                crate::brain::load(false);
            }
        };
        if let Some(brain) = crate::brain::get(cx) {
            let ready_now = {
                let b = brain.read(cx);
                !b.models.is_empty() || !b.alive
            };
            if ready_now {
                finish_boot(cx);
            } else {
                let booted = std::rc::Rc::new(std::cell::Cell::new(false));
                cx.observe(&brain, move |b, cx| {
                    if booted.get() {
                        return;
                    }
                    let br = b.read(cx);
                    if !br.models.is_empty() || !br.alive {
                        booted.set(true);
                        finish_boot(cx);
                    }
                })
                .detach();
            }
        } else {
            finish_boot(cx);
        }
        cx.on_app_quit(|_| async { crate::brain::stop() }).detach();
        if std::env::var("IRIS_OPEN_SETTINGS").is_ok() {
            crate::open_settings(crate::settings::Page::General, cx);
        }
        if std::env::var("IRIS_OPEN_PANEL").is_ok() {
            crate::call_panel(cx);
        }
        crate::hotkey::start(cx);
        #[cfg(windows)]
        crate::hotkey::listen();
        // Another app holds the shortcut: said under the menu bar flower, where Iris lives, a
        // moment after launch once the flower has its place (and kept in the slip after).
        if shortcut_taken() {
            let keys = crate::keys::inline(settings().open_shortcut);
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = cx.update(|cx| crate::live_hint::show(vec![], Some("shortcut taken".into()),
                    Some(crate::words::ws(format!("Another app uses {keys}. Open me to change it."))), cx));
            })
            .detach();
        }
        // The model came in while setup's window was closed: say so from the flower, instead of
        // opening a window that would take the keyboard from whatever you're in.
        if let Some(brain) = crate::brain::get(cx) {
            let downloaded = std::rc::Rc::new(std::cell::Cell::new(false));
            cx.observe(&brain, move |b, cx| {
                match b.read(cx).engine {
                    crate::brain::Engine::Downloading(_) => downloaded.set(true),
                    crate::brain::Engine::Ready(_) if downloaded.get() && !crate::live_setup::is_open()
                        && !crate::live_setup::progress(cx).is_complete() => {
                        downloaded.set(false);
                        crate::live_hint::show(vec![], Some("model's in".into()), Some(crate::words::w("Open the menu bar flower to finish setup.").into()), cx);
                    }
                    _ => {}
                }
            })
            .detach();
        }
        // Windows: the flower in the tray, opening Settings (no slip there).
        #[cfg(windows)]
        crate::tray::install(cx);
        let acx = cx.to_async();
        crate::menubar::install(Box::new(move |frame| {
            let acx = acx.clone();
            acx.foreground_executor().spawn({
                let acx = acx.clone();
                async move {
                    let _ = acx.update(|cx| crate::toggle_slip(frame, cx));
                }
            }).detach();
        }));
        if std::env::var("IRIS_CONTROL").is_ok() {
            listen(cx);
        }
    });
}

/// `--live-settings`: Settings on its own, working as in the app. The settings saved on this Mac
/// (`IRIS_HOME`), the brain started so a change reaches it (the model is never loaded at start
/// here), and the control socket when `IRIS_CONTROL` is set.
pub fn serve_settings(cx: &mut App) {
    SETTINGS.with(|s| *s.borrow_mut() = Settings::load());
    tell_keys();
    crate::brain::start(cx);
    cx.on_app_quit(|_| async { crate::brain::stop() }).detach();
    if std::env::var("IRIS_CONTROL").is_ok() {
        listen(cx);
    }
}

/// A command from the control socket, and where its answer goes.
type Command = (String, Sender<String>);

/// The control socket is a Unix socket: none on Windows yet (tests drive the Mac build).
#[cfg(not(unix))]
fn listen(_cx: &mut App) {
    eprintln!("control: not on this system");
}

/// Opens the control socket and answers its commands on the main thread.
#[cfg(unix)]
fn listen(cx: &mut App) {
    let path = store::home().join("control.sock");
    let _ = std::fs::create_dir_all(store::home());
    let _ = std::fs::remove_file(&path);
    let listener = match std::os::unix::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control: can't listen at {}: {e}", path.display());
            return;
        }
    };
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let (tx, rx): (Sender<Command>, Receiver<Command>) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let Ok(mut out) = stream.try_clone() else { return };
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let (atx, arx) = channel();
                    if tx.send((line, atx)).is_err() {
                        return;
                    }
                    let answer = arx.recv_timeout(Duration::from_secs(10)).unwrap_or_else(|_| "timeout".into());
                    if writeln!(out, "{answer}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    eprintln!("control: listening at {}", path.display());
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(30)).await;
            while let Ok((line, answer)) = rx.try_recv() {
                let reply = cx.update(|cx| command(&line, cx));
                let _ = answer.send(reply);
            }
        }
    })
    .detach();
}

pub(crate) fn command(line: &str, cx: &mut App) -> String {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("slip") => {
            crate::toggle_slip(crate::menubar::anchor(), cx);
            "ok".into()
        }
        Some("settings") => {
            let page = words.next().and_then(crate::settings::Page::from_slug).unwrap_or(crate::settings::Page::General);
            crate::open_settings(page, cx);
            "ok".into()
        }
        Some("setup") => {
            crate::live_setup::show(cx);
            "ok".into()
        }
        // `pretend untrusted|trusted|no-model|off`: what setup reads, made up (never saved).
        Some("pretend") => {
            if crate::live_setup::pretend(words.next().unwrap_or("")) { cx.refresh_windows(); "ok".into() } else { "error: pretend untrusted|trusted|no-model|downloading|read-only|off".into() }
        }
        // `hover|press <control> on|off`: as if the pointer were on (or pressing) a control, in
        // every live window; `controls` lists their names.
        Some(what @ ("hover" | "press")) => {
            let id = words.next().unwrap_or("").to_string();
            let on = words.next() != Some("off");
            if crate::interact::poke(what, &id, on, cx) { "ok".into() } else { format!("no control {id:?} (yet: it shows once drawn)") }
        }
        // The component workbench (every control, every state), on the test display.
        Some("components") => {
            let (x, y) = crate::test_display().map(|(x, y)| (x + 900.0, y + 30.0)).unwrap_or((200.0, 120.0));
            crate::components::open(gpui::point(gpui::px(x as f32), gpui::px(y as f32)), cx);
            "ok".into()
        }
        Some("controls") => crate::interact::names().join(" | "),
        // `type <text>` / `press-key <keystroke>`: typed into Settings, as if at the keyboard.
        Some("type") => {
            let text = line.splitn(2, ' ').nth(1).unwrap_or("");
            if crate::live_settings::type_text(text, cx) || crate::live_setup::type_text(text, cx) { "ok".into() } else { "error: nothing took it".into() }
        }
        Some("press-key") => {
            let k = words.next().unwrap_or("");
            if crate::live_settings::key(k, cx) || crate::live_setup::key(k, cx) { "ok".into() } else { "not taken".into() }
        }
        Some("clipboard") => crate::text_field::test_clipboard().unwrap_or_default(),
        // `set-clipboard <text>`: the test clipboard holds this (`\n` for a line break).
        Some("set-clipboard") => {
            crate::text_field::set_test_clipboard(&line.splitn(2, ' ').nth(1).unwrap_or("").replace("\\n", "\n"));
            "ok".into()
        }
        Some("setup-came-back") => {
            if crate::live_setup::came_back(cx) { "ok".into() } else { "error: setup isn't open".into() }
        }
        Some("practice") => crate::live_setup::practice_text(cx).unwrap_or_else(|| "error: setup isn't open".into()),
        Some("setup-state") => {
            let p = crate::live_setup::progress(cx);
            format!("step {:?} activity {:?} open {} line {:?}", p.current(), p.0.activity, crate::live_setup::is_open(), crate::live_setup::slip_line(cx))
        }
        Some("panel") => {
            crate::call_panel(cx);
            "ok".into()
        }
        Some("open-panel") => match words.next().and_then(|p| p.parse().ok()) {
            Some(pid) => {
                crate::flow::open_panel(Some(pid), cx);
                "ok".into()
            }
            None => "error: open-panel <pid>".into(),
        },
        Some("quick-fix") => match words.next().and_then(|p| p.parse().ok()) {
            Some(pid) => {
                crate::flow::quick_fix(Some(pid), cx);
                "ok".into()
            }
            None => "error: quick-fix <pid>".into(),
        },
        Some("key") => {
            let name = words.next().unwrap_or("");
            if crate::live::key(name, cx) { "ok".into() } else { "error: no panel to take it".into() }
        }
        Some("state") => serde_json::to_string(&settings()).unwrap_or_default(),
        // What the brain last said: the model's state, the models, the panel's tones.
        Some("brain") => match crate::brain::get(cx) {
            Some(b) => {
                let b = b.read(cx);
                format!("engine {:?} chosen {:?} models {:?} tones {:?} starting {}", b.engine, b.chosen,
                        b.models.iter().map(|m| (m.id.as_str(), m.downloaded)).collect::<Vec<_>>(),
                        b.tones.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), b.starting)
            }
            None => "no brain".into(),
        },
        Some("quit") => {
            cx.quit();
            "ok".into()
        }
        _ => format!("unknown: {line}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{key_code, key_name};

    #[test]
    fn key_codes_round_trip() {
        for k in ["a", "g", "k", "z", "0", "9"] {
            let code = key_code(k).unwrap();
            assert_eq!(key_name(code), k.to_uppercase());
        }
        assert_eq!(key_code("k"), Some(40));
        assert_eq!(key_code("space"), Some(49));
        assert_eq!(key_code("f3"), Some(99));
        // Keys a shortcut can't use.
        assert_eq!(key_code("left"), None);
        assert_eq!(key_code("["), None);
    }
}
