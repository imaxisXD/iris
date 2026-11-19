//! The tones and the shortcuts' keys as Settings shows them: `ToneList`'s rules (IrisCore's
//! ToneList.swift) over the parts the settings store, and `KeyCombo`'s keycaps and the rules a
//! recorded shortcut is checked against (`ShortcutProblem`, Hotkey.swift).

use serde_json::{Value, json};

use crate::brain::Mode;
use crate::store::{KeyCombo, Settings, carbon};

/// `Mode.builtInTones` (Prompts.swift), in their order.
pub fn built_in() -> Vec<Mode> {
    let m = |id: &str, label: &str, instruction: &str| Mode { id: id.into(), label: label.into(), instruction: instruction.into() };
    vec![
        m("professional", "Professional", "Sound clear and professional, like a capable colleague. Not stiff or flowery: no \"I hope this finds you well\", no \"kindly\". Professional changes how a thing is said, never whether it is said: a complaint, a disagreement, a refusal, frustration, an apology, thanks, and a courtesy like \"I know it's late\" all stay, put calmly and plainly. Never drop a point because it is blunt, and never soften a complaint into a request. Keep the point, not the wording: say each point in plain professional words, turning slang, sarcasm, insults and filler words like \"really\" or \"super\" into calm, direct language, so it reads as a colleague wrote it, not as the draft with its spelling fixed."),
        m("friendly", "Friendly", "Sound warm and easygoing, like a teammate people enjoy working with. Use contractions and plain, everyday words. Show warmth with words, never emoji: every rewrite gets one warm touch, such as a thanks, a word of acknowledgment, or a friendly opener like \"Hey\" or \"Quick heads-up:\". When the message is routine, one light, good-natured turn of phrase can make it friendlier; leave it out for bad news, apologies, complaints, or anything urgent, and never use sarcasm or joke about anyone. At most one exclamation mark. No filler openers like \"Hope you're doing well\". Be warm in the wording, never by hedging: if it says something will happen, it still will."),
        m("concise", "Shorter", "Make it as short as possible by cutting filler words and repetition. Every fact, question, request, and condition stays."),
        m("clearer", "Clearer", "Restructure it so the reader knows what to do on the first read: put the main point or request in the first sentence, then the details. Split long sentences, use everyday words, and use a short list when there are several items or steps."),
        m("funny", "Funny", "Add one light, good-natured touch of humor beside the message and change nothing else: keep the message's own words, and invent no facts or details. Never joke about people, mistakes, or anything sensitive. When humor doesn't fit (bad news, an apology, a complaint), only tidy the message."),
    ]
}

/// `Mode.customPrefix`.
pub const CUSTOM: &str = "custom-";

/// `ToneList`: the tones people have, their order, which the panel shows and starts on.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneList {
    pub custom: Vec<Mode>,
    pub order: Vec<String>,
    pub hidden: Vec<String>,
    pub default_id: String,
    /// Bundle ID → the tone the panel opens on there, in bundle ID order.
    pub per_app: Vec<(String, String)>,
}

impl ToneList {
    /// `ToneList.panelRoom`.
    pub const PANEL_ROOM: usize = 6;

    /// The list the settings hold (`ToneList.newMac` for what they don't).
    pub fn from_settings(s: &Settings) -> ToneList {
        let custom = s.custom_tones.as_ref().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|t| {
            Some(Mode { id: t["id"].as_str()?.into(), label: t["label"].as_str().unwrap_or("").into(), instruction: t["instruction"].as_str().unwrap_or("").into() })
        }).collect();
        let mut per_app: Vec<(String, String)> = s.app_modes.as_ref().and_then(|v| v.as_object().cloned()).unwrap_or_default().into_iter()
            .filter_map(|(k, v)| Some((k, v.as_str()?.to_string()))).collect();
        per_app.sort();
        ToneList {
            custom,
            order: s.tone_order.clone().unwrap_or_else(|| built_in().into_iter().map(|m| m.id).collect()),
            hidden: s.hidden_tones.clone().unwrap_or_else(|| vec!["funny".into()]),
            default_id: s.default_mode.clone().unwrap_or_else(|| "fix".into()),
            per_app,
        }
    }

    /// Writes the list back into the settings, in the Swift app's shapes.
    pub fn store(&self, s: &mut Settings) {
        s.custom_tones = Some(Value::Array(self.custom.iter().map(|m| json!({"id": m.id, "label": m.label, "instruction": m.instruction})).collect()));
        s.tone_order = Some(self.order.clone());
        s.hidden_tones = Some(self.hidden.clone());
        s.default_mode = Some(self.default_id.clone());
        s.app_modes = Some(Value::Object(self.per_app.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()));
    }

    /// Every tone in order, hidden ones included.
    pub fn all(&self) -> Vec<Mode> {
        let tones: Vec<Mode> = built_in().into_iter().chain(self.custom.iter().cloned()).collect();
        let mut out: Vec<Mode> = self.order.iter().filter_map(|id| tones.iter().find(|t| &t.id == id).cloned()).collect();
        for t in &tones {
            if !out.iter().any(|o| o.id == t.id) {
                out.push(t.clone());
            }
        }
        out
    }

    /// What the panel shows: Fix, then the tones that are on, as many as fit.
    pub fn shown(&self) -> Vec<Mode> {
        std::iter::once(Mode::fix()).chain(self.all().into_iter().filter(|t| !self.hidden.contains(&t.id))).take(Self::PANEL_ROOM).collect()
    }

    /// The number key that picks this tone in the panel, if the panel shows it.
    pub fn key(&self, id: &str) -> Option<usize> {
        self.shown().iter().position(|m| m.id == id).map(|i| i + 1)
    }

    pub fn move_by(&mut self, id: &str, by: isize) {
        let mut ids: Vec<String> = self.all().into_iter().map(|m| m.id).collect();
        let Some(i) = ids.iter().position(|x| x == id) else { return };
        let j = i as isize + by;
        if j < 0 || j as usize >= ids.len() {
            return;
        }
        ids.swap(i, j as usize);
        self.order = ids;
    }

    /// Adds a tone of the writer's own, with an id no built-in tone can have.
    pub fn add(&mut self, name: &str, instruction: &str) {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let id = format!("{CUSTOM}{:08X}", (n ^ (n >> 32)) as u32);
        self.custom.push(Mode { id, label: name.into(), instruction: instruction.into() });
        self.order = self.all().into_iter().map(|m| m.id).collect();
    }

    /// Removes a tone of the writer's own, and everything that pointed at it.
    pub fn remove(&mut self, id: &str) {
        self.custom.retain(|m| m.id != id);
        self.order.retain(|x| x != id);
        self.hidden.retain(|x| x != id);
        if self.default_id == id {
            self.default_id = "fix".into();
        }
        self.per_app.retain(|(_, m)| m != id);
    }

    pub fn set_hidden(&mut self, id: &str, hidden: bool) {
        self.hidden.retain(|x| x != id);
        if hidden {
            self.hidden.push(id.into());
        }
    }
}

/// Changes the tones, saves them, and tells the brain (the panel's tones come from it).
pub fn change(f: impl FnOnce(&mut ToneList)) {
    crate::app::change(|s| {
        let mut list = ToneList::from_settings(s);
        f(&mut list);
        list.store(s);
    });
    crate::brain::tones(None);
}

// MARK: - Keys

/// One key as the keyboard prints it: its symbol, if it has one, and its word.
pub type Cap = crate::pages::notes::Cap;

/// A word that lives for the whole run (keys named once, kept).
fn keep(s: String) -> &'static str {
    thread_local! { static WORDS: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) }; }
    WORDS.with(|w| {
        if let Some(found) = w.borrow().iter().find(|x| **x == s) {
            return *found;
        }
        let leaked: &'static str = Box::leak(s.into_boxed_str());
        w.borrow_mut().push(leaked);
        leaked
    })
}

/// `KeyCombo.keycaps`: ⌃ ⌥ ⇧ ⌘ with their words, then the key.
pub fn caps(k: KeyCombo) -> Vec<Cap> {
    let mut out: Vec<Cap> = vec![];
    for (bit, sym, word) in [(carbon::CONTROL, "⌃", "control"), (carbon::OPTION, "⌥", "option"), (carbon::SHIFT, "⇧", "shift"), (carbon::CMD, "⌘", "command")] {
        if k.modifiers & bit != 0 {
            out.push((Some(sym), word));
        }
    }
    let key = crate::app::key_name(k.key_code);
    out.push(match key.as_str() {
        "↩" => (Some("↩"), "return"),
        "⇥" => (Some("⇥"), "tab"),
        "⌫" => (Some("⌫"), "delete"),
        _ => (None, keep(key)),
    });
    out
}

/// `KeyCombo.spoken`.
pub fn spoken(k: KeyCombo) -> String {
    let words = [("⌃", "Control"), ("⌥", "Option"), ("⇧", "Shift"), ("⌘", "Command"), ("↩", "Return"), ("⇥", "Tab"), ("⌫", "Delete"), ("esc", "Escape")];
    crate::app::shortcut_caps(k).iter().map(|c| words.iter().find(|w| w.0 == c).map(|w| w.1.to_string()).unwrap_or_else(|| c.clone())).collect::<Vec<_>>().join(" + ")
}

/// `ShortcutProblem`: what's wrong with a shortcut, if anything.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    NeedsModifier,
    ReservedBySystem(&'static str),
    CommonConflict(&'static str),
    UsedByOtherIrisShortcut,
    TakenByAnotherApp,
}

impl Problem {
    pub fn message(&self) -> String {
        match self {
            Problem::NeedsModifier => "Add ⌃, ⌥, or ⌘ so it doesn't fire while you type.".into(),
            Problem::ReservedBySystem(w) => format!("macOS uses this for {w}. Pick another."),
            Problem::CommonConflict(w) => format!("Usually taken by {w}. It may not reach Iris."),
            Problem::UsedByOtherIrisShortcut => "Your other Iris shortcut already uses this.".into(),
            Problem::TakenByAnotherApp => "Another app already uses this shortcut.".into(),
        }
    }
    /// Warnings can still be saved; errors can't.
    pub fn is_blocking(&self) -> bool {
        !matches!(self, Problem::CommonConflict(_))
    }

    pub fn check(combo: KeyCombo, other: Option<KeyCombo>) -> Option<Problem> {
        use carbon::{CMD, CONTROL, OPTION};
        if combo.modifiers & (CONTROL | OPTION | CMD) == 0 {
            return Some(Problem::NeedsModifier);
        }
        if other == Some(combo) {
            return Some(Problem::UsedByOtherIrisShortcut);
        }
        // Carbon key codes: Space 49, Tab 48, Q 12, W 13, C 8, V 9, X 7, Z 6, A 0, S 1, H 4, M 46, K 40, B 11, I 34.
        let reserved: [(u32, u32, &str); 14] = [
            (49, CMD, "Spotlight"), (49, CONTROL, "switching input languages"), (49, CONTROL | OPTION, "switching input languages"),
            (48, CMD, "switching apps"), (12, CMD, "Quit"), (13, CMD, "Close window"), (8, CMD, "Copy"), (9, CMD, "Paste"), (7, CMD, "Cut"),
            (6, CMD, "Undo"), (0, CMD, "Select All"), (1, CMD, "Save"), (4, CMD, "Hide"), (46, CMD, "Minimize"),
        ];
        if let Some(r) = reserved.iter().find(|r| r.0 == combo.key_code && r.1 == combo.modifiers) {
            return Some(Problem::ReservedBySystem(r.2));
        }
        let common: [(u32, u32, &str); 5] = [
            (49, OPTION, "ChatGPT and Raycast"), (49, CMD | OPTION, "Finder search"), (40, CMD, "Slack's quick switcher"),
            (11, CMD, "Bold in Slack and most editors"), (34, CMD, "Italic in Slack and most editors"),
        ];
        if let Some(c) = common.iter().find(|c| c.0 == combo.key_code && c.1 == combo.modifiers) {
            return Some(Problem::CommonConflict(c.2));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_list_rules() {
        let s = Settings::default();
        let mut l = ToneList::from_settings(&s);
        assert_eq!(l.shown().iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["fix", "professional", "friendly", "concise", "clearer"]);
        assert_eq!(l.key("funny"), None);
        l.move_by("friendly", -1);
        assert_eq!(l.all()[0].id, "friendly");
        l.add("Pirate", "Arr");
        let id = l.all().last().unwrap().id.clone();
        assert!(id.starts_with(CUSTOM));
        l.default_id = id.clone();
        l.per_app.push(("com.apple.mail".into(), id.clone()));
        l.remove(&id);
        assert_eq!(l.default_id, "fix");
        assert!(l.per_app.is_empty());
    }

    #[test]
    fn shortcut_problems() {
        let cmd_c = KeyCombo { key_code: 8, modifiers: carbon::CMD };
        assert_eq!(Problem::check(cmd_c, None), Some(Problem::ReservedBySystem("Copy")));
        assert_eq!(Problem::check(KeyCombo { key_code: 5, modifiers: 0 }, None), Some(Problem::NeedsModifier));
        assert!(!Problem::check(KeyCombo { key_code: 40, modifiers: carbon::CMD }, None).unwrap().is_blocking());
        assert_eq!(Problem::check(KeyCombo::OPEN, None), None);
    }
}
