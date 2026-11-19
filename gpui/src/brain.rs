//! Talking to the brain: the helper program that runs the model and the rewriter
//! (`brain/`, the Swift app's own engine and rules). One JSON object a line each way.
//!
//! The brain is started once and kept; its news (the model's state, answers as they stream)
//! comes back on a reader thread and is handed to the main thread, where `Brain` (a GPUI
//! entity the slip, Settings and setup watch) holds the model's state, and each check's
//! answers go to the callback that asked for them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use gpui::*;
use serde_json::{Value, json};

use crate::model::{Answer, Seg, Verdict};

/// The model's state, as the engine reports it (`EngineState`).
#[derive(Clone, Debug, PartialEq)]
pub enum Engine {
    Idle,
    Downloading(f64),
    Loading,
    Ready(String),
    Failed(ModelFailure),
}

/// Why getting the model failed (`ModelFailure`), in the engine's words.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelFailure {
    pub kind: String,
    pub sentence: String,
    pub can_retry: bool,
    /// No room: how much the model needs and how much is free ("6.2 GB").
    pub needed: String,
    pub free: String,
}

/// What a check that failed says (`ProblemNote`).
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub note: String,
    pub lead: String,
    pub detail: String,
    pub can_retry: bool,
    pub circled: bool,
    /// The check itself failed (rather than getting the model).
    pub check_failed: bool,
}

/// A tone as the rewriter takes it (`Mode`).
#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub id: String,
    pub label: String,
    pub instruction: String,
}

impl Mode {
    pub fn fix() -> Mode {
        Mode { id: "fix".into(), label: "Fix".into(), instruction: String::new() }
    }
    fn json(&self) -> Value {
        json!({"id": self.id, "label": self.label, "instruction": self.instruction})
    }
    fn from(v: &Value) -> Option<Mode> {
        Some(Mode { id: v["id"].as_str()?.into(), label: v["label"].as_str()?.into(), instruction: v["instruction"].as_str().unwrap_or("").into() })
    }
}

/// One of the models this Mac can run (`ModelChoice`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub download_size: String,
    pub downloaded: bool,
}

/// What a check sends back.
#[derive(Clone, Debug)]
pub enum Event {
    Partial(String),
    /// The answer, and how many words it changed (`changedWords`).
    Finished(Answer, usize),
    Failed(Problem),
}

/// The brain as the app sees it.
pub struct Brain {
    pub engine: Engine,
    pub models: Vec<ModelInfo>,
    pub recommended: Option<String>,
    /// The model the brain will load: the one chosen in Settings, or the one it starts on.
    pub chosen: Option<String>,
    /// The tones the panel shows (Fix first), and the one it starts on, from the settings.
    pub tones: Vec<Mode>,
    pub starting: String,
    /// The brain is running (false if it couldn't be started).
    pub alive: bool,
}

/// The `Brain` entity, for anything that wants to watch it.
pub struct BrainHandle(pub Entity<Brain>);
impl Global for BrainHandle {}

type OnEvent = Box<dyn FnMut(Event, &mut App)>;

thread_local! {
    static STDIN: RefCell<Option<ChildStdin>> = const { RefCell::new(None) };
    static CHILD: RefCell<Option<Child>> = const { RefCell::new(None) };
    static CHECKS: RefCell<HashMap<i64, OnEvent>> = RefCell::new(HashMap::new());
    static NEXT: RefCell<i64> = const { RefCell::new(1) };
}

/// Where the brain is: beside the app's own program in a bundle (`Contents/MacOS/iris-brain`),
/// `IRIS_BRAIN`, or the development build in `brain/`.
fn path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("IRIS_BRAIN") {
        return Some(PathBuf::from(p));
    }
    let beside = std::env::current_exe().ok()?.with_file_name("iris-brain");
    if beside.exists() {
        return Some(beside);
    }
    // A development run from the checkout (`cargo run`): the brain built in `brain/`. Only in
    // debug builds, so an app built for others names no folder on this Mac.
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("brain/.build/xcode/Build/Products/Release/iris-brain");
        if dev.exists() {
            return Some(dev);
        }
    }
    None
}

/// Sends a request.
fn send(v: Value) {
    STDIN.with(|s| {
        if let Some(stdin) = s.borrow_mut().as_mut() {
            let _ = writeln!(stdin, "{v}");
            let _ = stdin.flush();
        }
    });
}

/// Starts the brain and listens to it. The `Brain` entity goes in a global.
pub fn start(cx: &mut App) {
    let brain = cx.new(|_| Brain { engine: Engine::Idle, models: vec![], recommended: None, chosen: None, tones: vec![Mode::fix()], starting: "fix".into(), alive: false });
    cx.set_global(BrainHandle(brain.clone()));
    // `--mock-answers`: no brain, canned answers (`mock.rs`).
    if crate::mock::on() {
        return crate::mock::start(&brain, cx);
    }
    let Some(path) = path() else {
        eprintln!("brain: not found (build it with gpui/brain/build.sh)");
        return;
    };
    let mut cmd = Command::new(&path);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    // MLX finds its Metal shaders beside the brain (the bundle's Resources, or its build folder).
    if let Some(dir) = path.parent() {
        cmd.current_dir(dir);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("brain: can't start {}: {e}", path.display());
            return;
        }
    };
    let stdout = child.stdout.take().unwrap();
    STDIN.with(|s| *s.borrow_mut() = child.stdin.take());
    CHILD.with(|c| *c.borrow_mut() = Some(child));
    brain.update(cx, |b, _| b.alive = true);
    let (tx, rx) = channel::<Value>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(v) => {
                    if tx.send(v).is_err() {
                        return;
                    }
                }
                Err(_) => eprintln!("brain: not JSON: {line}"),
            }
        }
        let _ = tx.send(json!({"event": "gone"}));
    });
    pump(rx, brain, cx);
    send(json!({"op": "hello"}));
    configure(cx);
}

/// Hands the brain's news to the main thread, as it comes.
fn pump(rx: Receiver<Value>, brain: Entity<Brain>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            let mut news = vec![];
            while let Ok(v) = rx.try_recv() {
                news.push(v);
            }
            if news.is_empty() {
                continue;
            }
            let gone = cx.update(|cx| {
                let mut gone = false;
                for v in news {
                    gone |= handle(v, &brain, cx);
                }
                gone
            });
            if gone {
                return;
            }
        }
    })
    .detach();
}

/// One piece of news. True when the brain has gone.
fn handle(v: Value, brain: &Entity<Brain>, cx: &mut App) -> bool {
    match v["event"].as_str().unwrap_or("") {
        "state" => {
            let engine = match v["state"].as_str().unwrap_or("") {
                "downloading" => Engine::Downloading(v["fraction"].as_f64().unwrap_or(0.0)),
                "loading" => Engine::Loading,
                "ready" => Engine::Ready(v["model"].as_str().unwrap_or("").into()),
                "failed" => Engine::Failed(ModelFailure {
                    kind: v["failure"]["kind"].as_str().unwrap_or("other").into(),
                    sentence: v["failure"]["sentence"].as_str().unwrap_or("").into(),
                    can_retry: v["failure"]["canRetry"].as_bool().unwrap_or(true),
                    needed: v["failure"]["needed"].as_str().unwrap_or("").into(),
                    free: v["failure"]["free"].as_str().unwrap_or("").into(),
                }),
                _ => Engine::Idle,
            };
            // Ready: fill in the fixed part of the prompt for the tone the panel opens on, as the
            // Swift app does, so the first check doesn't pay for it.
            let became_ready = matches!(engine, Engine::Ready(_)) && !matches!(brain.read(cx).engine, Engine::Ready(_));
            brain.update(cx, |b, cx| {
                b.engine = engine;
                cx.notify();
            });
            if became_ready {
                let starting = brain.read(cx).starting.clone();
                let mode = brain.read(cx).tones.iter().find(|m| m.id == starting).cloned().unwrap_or_else(Mode::fix);
                prewarm(&mode);
            }
        }
        "hello" => {
            let models = v["models"].as_array().cloned().unwrap_or_default().iter().map(|m| ModelInfo {
                id: m["id"].as_str().unwrap_or("").into(),
                download_size: m["downloadSize"].as_str().unwrap_or("").into(),
                downloaded: m["downloaded"].as_bool().unwrap_or(false),
            }).collect();
            brain.update(cx, |b, cx| {
                b.models = models;
                b.recommended = v["recommended"].as_str().map(Into::into);
                b.chosen = v["chosen"].as_str().map(Into::into);
                cx.notify();
            });
        }
        "tones" => {
            let shown: Vec<Mode> = v["shown"].as_array().cloned().unwrap_or_default().iter().filter_map(Mode::from).collect();
            brain.update(cx, |b, cx| {
                if !shown.is_empty() {
                    b.tones = shown;
                }
                b.starting = v["starting"].as_str().unwrap_or("fix").into();
                cx.notify();
            });
        }
        "partial" | "finished" | "failed" => {
            let Some(id) = v["id"].as_i64() else { return false };
            let event = match v["event"].as_str() {
                Some("partial") => Event::Partial(v["text"].as_str().unwrap_or("").into()),
                Some("finished") => Event::Finished(answer(&v["result"]), v["result"]["changedWords"].as_u64().unwrap_or(0) as usize),
                _ => Event::Failed(Problem { check_failed: v["checkFailed"].as_bool().unwrap_or(false), ..problem(&v["problem"]) }),
            };
            deliver(id, event, cx);
        }
        "gone" => {
            eprintln!("brain: gone");
            brain.update(cx, |b, cx| {
                b.alive = false;
                cx.notify();
            });
            return true;
        }
        _ => {}
    }
    false
}

/// Hands check `id`'s news to whoever asked (nobody, once it's finished or cancelled).
pub fn deliver(id: i64, event: Event, cx: &mut App) {
    let last = !matches!(event, Event::Partial(_));
    let cb = CHECKS.with(|c| c.borrow_mut().remove(&id));
    if let Some(mut cb) = cb {
        cb(event, cx);
        if !last {
            CHECKS.with(|c| c.borrow_mut().insert(id, cb));
        }
    }
}

fn answer(r: &Value) -> Answer {
    let segments = r["segments"].as_array().cloned().unwrap_or_default().iter().filter_map(|s| {
        let kind = s.get(0)?.as_str()?;
        let text = s.get(1)?.as_str()?.to_string();
        Some(match kind {
            "added" => Seg::Added(text),
            "removed" => Seg::Removed(text),
            _ => Seg::Same(text),
        })
    }).collect();
    let verdict = match r["verdict"].as_str() {
        Some("unchanged") => Some(Verdict::Unchanged),
        Some("minor") => Some(Verdict::Minor),
        Some("heavy") => Some(Verdict::Heavy),
        _ => None,
    };
    let keep = r["keep"].as_array().map(|a| a.iter().filter_map(|c| Some((c["from"].as_str()?.to_string(), c["to"].as_str()?.to_string()))).collect()).unwrap_or_default();
    Answer { fix: r["mode"].as_str() == Some("fix"), text: r["text"].as_str().unwrap_or("").into(), segments, verdict, translated: r["translated"].as_bool().unwrap_or(false), keep }
}

fn problem(p: &Value) -> Problem {
    Problem {
        note: p["note"].as_str().unwrap_or("").into(),
        lead: p["lead"].as_str().unwrap_or("").into(),
        detail: p["detail"].as_str().unwrap_or("").into(),
        can_retry: p["canRetry"].as_bool().unwrap_or(false),
        circled: p["circled"].as_bool().unwrap_or(false),
        check_failed: false,
    }
}

// MARK: - Asking

/// Hands the settings the brain needs to it: the model, when to free memory, and the tones.
pub fn configure(_cx: &mut App) {
    let s = crate::app::settings();
    send(json!({"op": "configure", "model": s.model, "idleMinutes": s.idle_minutes, "onboarded": s.onboarded}));
    tones(None);
}

/// Asks which tones the panel shows (and starts on, for the app `bundle`).
pub fn tones(bundle: Option<&str>) {
    let s = crate::app::settings();
    send(json!({"op": "tones", "customTones": s.custom_tones, "toneOrder": s.tone_order, "hiddenTones": s.hidden_tones,
                "defaultMode": s.default_mode, "appModes": s.app_modes, "bundleID": bundle}));
}

/// Loads the model (downloading it only when `allow_download`).
pub fn load(allow_download: bool) {
    send(json!({"op": "load", "allowDownload": allow_download}));
}

/// Deletes the model `id` from disk (`ModelDialogs.delete`); the brain lets go of it first if
/// it's the chosen one, then says which models are on disk.
pub fn delete(id: &str) {
    send(json!({"op": "delete", "model": id}));
}

pub fn unload() {
    send(json!({"op": "unload"}));
}

/// Checks `text` in `mode`; `on` hears its answers as they stream, then the end. Returns its
/// number, for `cancel`.
pub fn check(text: &str, mode: &Mode, retry: bool, on: impl FnMut(Event, &mut App) + 'static) -> i64 {
    let id = NEXT.with(|n| {
        let mut n = n.borrow_mut();
        *n += 1;
        *n
    });
    CHECKS.with(|c| c.borrow_mut().insert(id, Box::new(on)));
    if crate::mock::on() {
        crate::mock::answer_later(id, text, mode);
        return id;
    }
    let s = crate::app::settings();
    send(json!({"op": "check", "id": id, "text": text, "mode": mode.json(), "retry": retry,
                "glossary": s.glossary, "styleRules": s.style_rules}));
    id
}

/// Stops a check; its callback hears nothing more.
pub fn cancel(id: i64) {
    CHECKS.with(|c| c.borrow_mut().remove(&id));
    send(json!({"op": "cancel", "id": id}));
}

/// Fills in the fixed part of the prompt for `mode` ahead of time.
pub fn prewarm(mode: &Mode) {
    let s = crate::app::settings();
    send(json!({"op": "prewarm", "mode": mode.json(), "glossary": s.glossary, "styleRules": s.style_rules}));
}

/// The brain, if it's running.
pub fn get(cx: &App) -> Option<Entity<Brain>> {
    cx.try_global::<BrainHandle>().map(|h| h.0.clone())
}

/// Stops the brain (when the app quits).
pub fn stop() {
    STDIN.with(|s| s.borrow_mut().take());
    CHILD.with(|c| {
        if let Some(mut child) = c.borrow_mut().take() {
            let _ = child.kill();
        }
    });
}
