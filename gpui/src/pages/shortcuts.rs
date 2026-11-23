//! Settings › Shortcuts (`ShortcutsPage` in SettingsView.swift): the two shortcuts with their
//! recorders (`ShortcutRecorder`, at rest), and the keys that work inside the panel.

use gpui::{Div, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div};

use crate::controls::{self as c, Button};
use crate::interact::act;

use crate::parts::*;
use crate::riso::{col, hue, ink};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// `HotkeyCenter.Action`'s raw values, which seed each recorder's keys.
const ACTION_OPEN: i64 = 1;
const ACTION_QUICK_FIX: i64 = 2;

/// `Hairline()` between the two shortcuts: its line in SettingsView.swift.
const HAIRLINE_SEED: i64 = 493;

/// A recorder that's listening for new keys (`ShortcutRecorder`'s state): which shortcut,
/// what's wrong with the keys last pressed, and keys with only a warning, waiting for Use anyway.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub action: i64,
    pub listening: bool,
    pub problem: Option<crate::tones::Problem>,
    pub pending: Option<crate::store::KeyCombo>,
}

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;

    // 01: the shortcuts. The compact button sits on the heading's baseline.
    let restore = act(|window, _| {
        let d = crate::store::Settings::default();
        crate::app::change(|s| {
            s.open_shortcut = d.open_shortcut;
            s.quick_fix_shortcut = d.quick_fix_shortcut;
        });
        stop(None);
        crate::hotkey::refresh();
        window.refresh();
    });
    let (restore, restore_w) = c::button(&Button::secondary("Restore defaults").compact(), s, restore);
    let brush_box = line_box(24.0, BRUSH_LINE);
    let title_base = (brush_box - 24.0 * BRUSH_LINE) / 2.0 + 24.0 * BRUSH_ASCENT;
    let button_base = (26.0 - 12.5 * MONO_LINE) / 2.0 + 12.5;
    let restore = div().flex_none().mt(z(title_base - button_base)).child(restore);

    let open_detail = column().gap(z(4.0))
        .child(t("Checks the box you're typing in.", caption(), hue::I10))
        .child(crate::keys::command("Z").iter().fold(row().gap(z(4.0)).items_center(), |r, k| r.child(keycap(k, s)))
            .child(t("undoes a Replace.", caption(), hue::I10))
            .id("settings-undo-keys").aria_label(format!("{} Z undoes a Replace.", crate::keys::spoken_cap(&crate::keys::command("Z")[0]))));
    let (open_rec, open_w) = recorder(v, ACTION_OPEN, s);
    let (quick_rec, quick_w) = recorder(v, ACTION_QUICK_FIX, s);
    let quick_detail = para_out("Fixes and replaces at once. Big rewrites still open the panel.", caption(), hue::I10, w - 16.0 - quick_w);
    let shortcuts = column().w(z(w)).gap(z(12.0))
        .child(shortcut_row("Open Iris", open_detail, open_rec, open_w, w))
        .child(hairline(HAIRLINE_SEED, w))
        .child(shortcut_row("Quick Fix", quick_detail, quick_rec, quick_w, w));

    // 02: the panel's keys, a table of keycaps and what they do.
    // In this system's keys (`keys.rs`): ⌘C on the Mac is Ctrl+C on Windows, ↵ is Enter.
    let next_tone = format!("Next tone ({} goes back)", crate::keys::shifted_chip("⇥"));
    let keys = [
        (crate::keys::key("↵"), "Replace your text with the result"), (crate::keys::key("⇥"), next_tone.as_str()),
        ("1–6".to_string(), "Jump to a tone"), (crate::keys::command_chip("C"), "Copy the result"),
        (crate::keys::command_chip("R"), "Try again: write a new version"),
        (crate::keys::key("esc"), "Close without changing anything"),
    ];
    let mut table = column().w(z(w));
    for (i, (combo, what)) in keys.iter().enumerate() {
        let caps = split_keys(combo).into_iter().fold(row().gap(z(3.0)), |r, k| r.child(keycap(&k, s)));
        let r = row().gap(z(12.0)).items_center()
            .child(div().flex_none().w(z(56.0)).child(caps))
            .child(t(*what, body(), hue::I11))
            .child(grow());
        table = table.child(c::table_row(&format!("panel-key-{i}"), r, i == keys.len() - 1, i as i64, w, 1.0));
    }

    vec![
        section(Some(1), "Shortcuts", Some("They work in any app. To use your own, click the keys, then press the combination you want."),
            Some((restore.into_any_element(), restore_w)), Some(shortcuts), w, s),
        section(Some(2), "In the panel", Some("Only while the panel is open, so they never clash with other apps."), None, Some(table), w, s),
    ]
}

/// One shortcut's recorder, as the page draws it (listening, its problem) and what it does.
fn recorder(v: &Values, action: i64, s: f32) -> (gpui::AnyElement, f32) {
    let keys = if action == ACTION_OPEN { Some(v.open_shortcut) } else { v.quick_fix };
    let rec = v.recording.as_ref().filter(|r| r.action == action);
    let face = c::Recorder {
        keys, action, listening: rec.is_some_and(|r| r.listening), allows_clear: action == ACTION_QUICK_FIX,
        problem: rec.and_then(|r| r.problem.clone()), pending: rec.is_some_and(|r| r.pending.is_some()),
    };
    let acts = v.fields.as_ref().map(|f| {
        let focus = f.keys[(action - 1) as usize].clone();
        let fc = focus.clone();
        c::RecorderActs {
            focus: Some(focus),
            press: act(move |window, cx| {
                if listening(action) { stop(Some(action)) } else { start(action, &fc, window, cx) }
                window.refresh();
            }),
            key: std::rc::Rc::new(move |e, window, cx| {
                if listening(action) {
                    cx.stop_propagation();
                    heard(action, e, window);
                }
            }),
            use_anyway: act(move |window, _| {
                let pending = MOMENT.with(|m| m.borrow().recording.as_ref().and_then(|r| r.pending));
                if let Some(k) = pending {
                    MOMENT.with(|m| m.borrow_mut().recording = None);
                    save(action, Some(k));
                }
                window.refresh();
            }),
        }
    });
    c::recorder(&face, s, acts)
}

use crate::live_settings::MOMENT;

/// Whether the recorder for `action` is listening.
fn listening(action: i64) -> bool {
    MOMENT.with(|m| m.borrow().recording.as_ref().is_some_and(|r| r.action == action && r.listening))
}

/// Starts listening (`ShortcutRecorder.start`): Iris's own shortcuts are let go of, so the
/// current keys can be pressed again; the recorder takes the keyboard.
fn start(action: i64, focus: &gpui::FocusHandle, window: &mut gpui::Window, cx: &mut gpui::App) {
    MOMENT.with(|m| m.borrow_mut().recording = Some(Recording { action, listening: true, problem: None, pending: None }));
    crate::hotkey::set_paused(true);
    window.focus(focus, cx);
}

/// Stops listening (Escape, a second click, clicking away): the shortcuts are taken back. A
/// problem stays on show. `action` none: whichever was.
pub fn stop(action: Option<i64>) {
    let was = MOMENT.with(|m| {
        let mut m = m.borrow_mut();
        let Some(r) = m.recording.as_mut() else { return false };
        if action.is_some_and(|a| a != r.action) || !r.listening {
            return false;
        }
        r.listening = false;
        if r.problem.is_none() {
            m.recording = None;
        }
        true
    });
    if was {
        crate::hotkey::set_paused(false);
    }
}

/// A key while listening (`ShortcutRecorder.handle`).
fn heard(action: i64, e: &gpui::KeyDownEvent, window: &mut gpui::Window) {
    use crate::store::{KeyCombo, carbon};
    let k = &e.keystroke;
    let m = k.modifiers;
    let plain = !(m.platform || m.control || m.alt || m.shift);
    if plain && k.key == "escape" {
        stop(Some(action));
        window.refresh();
        return;
    }
    if plain && action == ACTION_QUICK_FIX && (k.key == "backspace" || k.key == "delete") {
        stop(Some(action));
        MOMENT.with(|m| m.borrow_mut().recording = None);
        save(action, None);
        window.refresh();
        return;
    }
    let Some(code) = crate::app::key_code(&k.key) else { return };
    let mut mods = 0;
    for (on, bit) in [(m.platform, carbon::CMD), (m.shift, carbon::SHIFT), (m.alt, carbon::OPTION), (m.control, carbon::CONTROL)] {
        if on {
            mods |= bit;
        }
    }
    let combo = KeyCombo { key_code: code, modifiers: mods };
    let s = crate::app::settings();
    let other = if action == ACTION_OPEN { s.quick_fix_shortcut } else { Some(s.open_shortcut) };
    if let Some(p) = crate::tones::Problem::check(combo, other) {
        let blocking = p.is_blocking();
        MOMENT.with(|m| {
            if let Some(r) = m.borrow_mut().recording.as_mut() {
                r.problem = Some(p);
                r.pending = (!blocking).then_some(combo);
            }
        });
        // A warning stops listening, waiting for Use anyway; an error keeps listening.
        if !blocking {
            stop(Some(action));
        }
        window.refresh();
        return;
    }
    stop(Some(action));
    MOMENT.with(|m| m.borrow_mut().recording = None);
    save(action, Some(combo));
    window.refresh();
}

/// Saves keys no other app holds (`ShortcutRecorder.save`), so a saved shortcut never silently
/// does nothing, and registers them. None: Quick Fix turned off.
fn save(action: i64, combo: Option<crate::store::KeyCombo>) {
    let s = crate::app::settings();
    let current = if action == ACTION_OPEN { Some(s.open_shortcut) } else { s.quick_fix_shortcut };
    if let Some(k) = combo && combo != current && !crate::hotkey::is_available(k) {
        MOMENT.with(|m| m.borrow_mut().recording = Some(Recording { action, listening: false, problem: Some(crate::tones::Problem::TakenByAnotherApp), pending: None }));
        return;
    }
    crate::app::change(|s| match (action, combo) {
        (ACTION_OPEN, Some(k)) => s.open_shortcut = k,
        (ACTION_OPEN, None) => {}
        (_, k) => s.quick_fix_shortcut = k,
    });
    crate::hotkey::refresh();
}

/// `shortcutRow`: the name and what it does on the left, the recorder at the right, top-aligned.
fn shortcut_row(title: &str, detail: Div, recorder: gpui::AnyElement, recorder_w: f32, w: f32) -> Div {
    let words_w = w - 16.0 - recorder_w;
    row().w(z(w)).gap(z(16.0)).items_start()
        .child(column().w(z(words_w)).gap(z(4.0)).child(t(title, body(), hue::I12)).child(detail))
        .child(recorder)
}

fn lone_letter(key: &str, symbol: Option<&str>) -> bool {
    symbol.is_none() && key.chars().count() <= 2
}

pub fn labeled_w(key: &str, symbol: Option<&str>) -> f32 {
    if lone_letter(key, symbol) { 30.0 } else { (14.0 + key.chars().count() as f32 * 6.6).max(40.0) }
}

/// SF's line height as a share of its size (ascent + descent).
const SYSTEM_LINE: f32 = 1.193;

/// The modifier symbol in the system face, semibold: SF Pro draws ⌃ ⌥ ⇧ ⌘ cleanly at this size
/// where the mono face doesn't. GPUI reaches SF Pro as `.SystemUIFont` on a Mac.
fn symbol(sym: &str) -> Div {
    let family = if cfg!(target_os = "windows") { "Segoe UI Symbol" } else { ".SystemUIFont" };
    div().flex_none().font_family(family).font_weight(FontWeight::SEMIBOLD).text_size(z(12.0))
        .line_height(z((12.0f32 * SYSTEM_LINE).ceil())).text_color(col(hue::I12)).whitespace_nowrap().child(sym.to_string())
}

/// `Keycap(size: .labeled)`: printed like a Mac key, the symbol small at the top right and the
/// word along the bottom; a lone letter sits large in the middle. `printed`: the pink plate,
/// clearly off register (a shortcut that's set); otherwise crema, almost in register.
pub fn labeled_keycap(key: &str, sym: Option<&str>, seed: i64, printed: bool, scale: f32) -> Div {
    let w = labeled_w(key, sym);
    let h = 32.0;
    let (plate, off) = if printed { (ink::PINK, (-1.8, -1.6)) } else { (ink::CREMA, (-0.8, -0.5)) };
    let mut cap = sized(w, h)
        .child(at(hand_fill(seed + 9, 0.3, w, h, drop(0.22)), 0.0, 1.0))
        .child(at(hand_printed(seed + 3, 0.45, w, h, plate, seed + 3, scale), off.0, off.1))
        .child(at(hand_outline(seed, 0.45, 2.0, w, h, col(hue::I12), 1.2), 0.0, 0.0));
    if let Some(sy) = sym {
        cap = cap.child(at(row().w(z(w - 5.0)).justify_end().child(symbol(sy)), 0.0, 3.0));
    }
    if lone_letter(key, sym) {
        let size = 13.0;
        let lh = size * MONO_LINE;
        cap = cap.child(at(row().w(z(w)).justify_center().child(t(key, martian(size, 560), hue::I12)), 0.0, (h - 1.0 - lh) / 2.0));
    } else {
        let size = 9.5;
        cap = cap.child(at(t(key, martian(size, 560), hue::I12), 5.0, h - 4.0 - size * MONO_LINE));
    }
    cap
}
