//! Iris's own words as the computer in front of you calls things: "this Mac" on a Mac, "this PC"
//! on Windows; the menu bar flower there, the Iris icon by the clock here. Like `keys.rs` for
//! keys, the one place that knows both.
//!
//! Only Iris's own labels go through `w`, never a person's text (a draft that says "my Mac"
//! stays as written). On the Mac `w` hands the same string back: Mac wording can't change here.
//! `IRIS_PLATFORM=windows` (keys.rs) shows Windows' words on a Mac, for snapshots.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::keys::is_mac;

/// The Mac's phrases and Windows' for them, longest first, so a sentence's whole phrase goes
/// before any of its words do.
const WINDOWS: [(&str, &str); 24] = [
    ("Open me to change it.", "Click the Iris icon by the clock to change it."),
    ("Open the menu bar flower and finish setup to download it.", "Click the Iris icon by the clock and finish setup to download it."),
    ("open setup from the menu bar", "open setup from the iris icon by the clock"),
    ("Open the menu bar flower to finish setup.", "Click the Iris icon by the clock to finish setup."),
    ("When the model's in, the menu bar flower will say so.", "When the model's in, the Iris icon by the clock will say so."),
    ("Start Iris in the menu bar when you log in.", "Start Iris in the background when you sign in."),
    ("Download it, then replace Iris in Applications. Your settings stay.", "Download it and run the installer. Your settings stay."),
    ("The menu bar flower in pen", "The Iris icon in pen"),
    ("the menu bar flower", "the Iris icon by the clock"),
    ("The version on this Mac.", "The version on this PC."),
    ("On a company Mac", "On a company PC"),
    ("on a company Mac", "on a company PC"),
    ("Runs on your Mac", "Runs on your PC"),
    ("Earlier models on this Mac", "Earlier models on this PC"),
    ("for this Mac", "for this PC"),
    ("For Macs with", "For PCs with"),
    ("This Mac has", "This PC has"),
    ("this Mac", "this PC"),
    ("This Mac", "This PC"),
    ("Your Mac", "Your PC"),
    ("your Mac", "your PC"),
    ("the menu bar", "the taskbar"),
    ("System Settings", "Windows Settings"),
    ("macOS", "Windows"),
];

/// One of Iris's own strings in this computer's words.
pub fn w(s: &'static str) -> &'static str {
    if is_mac() {
        return s;
    }
    static SEEN: Mutex<Option<HashMap<&'static str, &'static str>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap();
    let map = seen.get_or_insert_with(HashMap::new);
    if let Some(found) = map.get(s) {
        return found;
    }
    let out: &'static str = Box::leak(windows(s).into_boxed_str());
    map.insert(s, out);
    out
}

/// A string made at run time ("This Mac has 16 GB of memory…") in this computer's words.
pub fn ws(s: String) -> String {
    if is_mac() { s } else { windows(&s) }
}

fn windows(s: &str) -> String {
    let mut out = s.to_string();
    for (mac, win) in WINDOWS {
        out = out.replace(mac, win);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mac_keeps_its_words() {
        if !is_mac() {
            return;
        }
        assert_eq!(w("Runs on this Mac. Nothing leaves it."), "Runs on this Mac. Nothing leaves it.");
        assert_eq!(ws("This Mac has 16 GB".into()), "This Mac has 16 GB");
    }

    #[test]
    fn windows_says_pc_and_the_icon_by_the_clock() {
        assert_eq!(windows("Runs on this Mac. Nothing leaves it."), "Runs on this PC. Nothing leaves it.");
        assert_eq!(windows("Open the menu bar flower and finish setup to download it."),
            "Click the Iris icon by the clock and finish setup to download it.");
        assert_eq!(windows("i need my model first. open setup from the menu bar"),
            "i need my model first. open setup from the iris icon by the clock");
        assert_eq!(windows("macOS didn't let Iris open at login."), "Windows didn't let Iris open at login.");
        assert_eq!(windows("Weaker in Hindi. For Macs with 8 GB."), "Weaker in Hindi. For PCs with 8 GB.");
    }
}
