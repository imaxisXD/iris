//! Keys as the computer in front of you writes them: the one place that decides how a key reads.
//!
//! The Mac draws modifiers as glyphs (⌃ ⌥ ⇧ ⌘ G); Windows writes words (Ctrl Alt Shift Win G).
//! Two kinds of key, as Zed and VS Code split them:
//! - A command inside Iris (Try again, Undo, Paste, Settings): `command("R")`. ⌘ on the Mac,
//!   Ctrl on Windows: what Zed calls the "secondary" modifier and VS Code `CtrlCmd`.
//! - A shortcut that works everywhere (open Iris, Quick Fix): a `KeyCombo`, stored as the Swift
//!   app stores it (a Carbon key code and modifier bits, on both systems). These can't be turned
//!   from one system's into the other's: ⌃⌥G read as Win+Alt+G is the Xbox Game Bar's, and
//!   Ctrl+Alt is AltGr on many keyboards. So each system has its own defaults (`open_default`),
//!   measured free with RegisterHotKey on Windows 11, and the bit that is ⌘ on the Mac is the
//!   Windows key on Windows.
//!
//! `IRIS_PLATFORM=windows` makes a Mac build write Windows' keys, to see them in snapshots.

use crate::store::{carbon, KeyCombo};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
}

/// The system whose keys Iris writes.
pub fn os() -> Os {
    static OS: OnceLock<Os> = OnceLock::new();
    *OS.get_or_init(|| match std::env::var("IRIS_PLATFORM").as_deref() {
        Ok("windows") => Os::Windows,
        Ok("mac") => Os::Mac,
        _ if cfg!(target_os = "macos") => Os::Mac,
        _ => Os::Windows,
    })
}

pub fn is_mac() -> bool {
    os() == Os::Mac
}

/// The modifiers in the order each system writes them, with the Carbon bit each one is.
fn modifiers() -> [(u32, &'static str); 4] {
    match os() {
        Os::Mac => [(carbon::CONTROL, "⌃"), (carbon::OPTION, "⌥"), (carbon::SHIFT, "⇧"), (carbon::CMD, "⌘")],
        Os::Windows => [(carbon::CONTROL, "Ctrl"), (carbon::CMD, "Win"), (carbon::OPTION, "Alt"), (carbon::SHIFT, "Shift")],
    }
}

/// A key's own name on this system (the Mac's glyphs for Return, Tab and Delete).
pub fn key(name: &str) -> String {
    match (os(), name) {
        (Os::Windows, "↩" | "↵") => "Enter".into(),
        (Os::Windows, "⇥") => "Tab".into(),
        (Os::Windows, "⌫") => "Backspace".into(),
        (Os::Windows, "esc") => "Esc".into(),
        _ => name.into(),
    }
}

/// Shift and the key, as one chip's text: "⇧⇥" on the Mac, "Shift+Tab" on Windows.
pub fn shifted_chip(key_name: &str) -> String {
    if is_mac() { format!("⇧{key_name}") } else { format!("Shift+{}", key(key_name)) }
}

/// A shortcut as caps, one per key, the way this system prints it.
pub fn caps(k: KeyCombo) -> Vec<String> {
    let mut out: Vec<String> = modifiers().iter().filter(|(bit, _)| k.modifiers & bit != 0).map(|(_, c)| c.to_string()).collect();
    out.push(key(&crate::app::key_name(k.key_code)));
    out
}

/// A command inside Iris: ⌘ and the key on the Mac, Ctrl and the key on Windows.
pub fn command(key_name: &str) -> Vec<String> {
    vec![if is_mac() { "⌘" } else { "Ctrl" }.into(), key(key_name)]
}

/// A command as one chip's text: "⌘R" on the Mac, "Ctrl+R" on Windows (`parts::split_keys`
/// takes either apart into caps).
pub fn command_chip(key_name: &str) -> String {
    command(key_name).join(if is_mac() { "" } else { "+" })
}

/// A shortcut written in a sentence: "⌃⌥G" on the Mac, "Ctrl+Win+G" on Windows.
pub fn inline(k: KeyCombo) -> String {
    caps(k).join(if is_mac() { "" } else { "+" })
}

/// The word printed under a modifier's cap where setup and Settings label them ("control"),
/// none for a cap that already is a word (all of Windows', and every key).
pub fn word(cap: &str) -> Option<&'static str> {
    match cap {
        "⌃" => Some("control"),
        "⌥" => Some("option"),
        "⇧" => Some("shift"),
        "⌘" => Some("command"),
        _ => None,
    }
}

/// A cap as setup and Settings draw it: (glyph, word under it) for a Mac modifier, (none, the
/// cap) for everything else.
pub fn labelled(cap: String) -> (Option<String>, String) {
    match word(&cap) {
        Some(w) => (Some(cap), w.to_string()),
        None => (None, cap),
    }
}

/// A cap read aloud (accessibility, "press Control + Option + G").
pub fn spoken_cap(cap: &str) -> String {
    match cap {
        "⌃" => "Control",
        "⌥" => "Option",
        "⇧" => "Shift",
        "⌘" => "Command",
        "↩" => "Return",
        "⇥" => "Tab",
        "⌫" => "Delete",
        "esc" | "Esc" => "Escape",
        "Ctrl" => "Control",
        "Win" => "Windows",
        other => other,
    }
    .to_string()
}

/// The modifiers a shortcut needs one of, in a sentence: "⌃, ⌥, or ⌘" / "Ctrl, Alt or Win".
pub fn needed_modifiers() -> &'static str {
    if is_mac() { "⌃, ⌥, or ⌘" } else { "Ctrl, Alt or Win" }
}

/// The shortcut that opens Iris when none is chosen: ⌃⌥G on the Mac, Ctrl+Win+G on Windows
/// (Win+Alt+G, the Mac's turned over, is the Xbox Game Bar's).
pub fn open_default() -> KeyCombo {
    match os() {
        Os::Mac => KeyCombo::OPEN,
        Os::Windows => KeyCombo { key_code: carbon::KEY_G, modifiers: carbon::CONTROL | carbon::CMD },
    }
}

/// Quick Fix's: ⌃⌥⇧G on the Mac, Ctrl+Win+Shift+G on Windows.
pub fn quick_fix_default() -> KeyCombo {
    match os() {
        Os::Mac => KeyCombo::QUICK_FIX,
        Os::Windows => KeyCombo { key_code: carbon::KEY_G, modifiers: carbon::CONTROL | carbon::CMD | carbon::SHIFT },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mac_writes_what_it_always_has() {
        if !is_mac() {
            return;
        }
        assert_eq!(caps(KeyCombo::OPEN), ["⌃", "⌥", "G"]);
        assert_eq!(caps(KeyCombo::QUICK_FIX), ["⌃", "⌥", "⇧", "G"]);
        assert_eq!(command("R"), ["⌘", "R"]);
        assert_eq!(command_chip("R"), "⌘R");
        assert_eq!(inline(KeyCombo::OPEN), "⌃⌥G");
        assert_eq!(open_default(), KeyCombo::OPEN);
        assert_eq!(labelled("⌃".into()), (Some("⌃".to_string()), "control".to_string()));
        assert_eq!(labelled("G".into()), (None, "G".to_string()));
    }
}
