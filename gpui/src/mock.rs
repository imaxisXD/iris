//! `iris --mock-answers`: canned answers instead of the model (`MockAnswers.swift`, ported), so
//! the whole panel and Quick Fix can be driven (`scripts/verify`, see `driven.rs`) without the
//! brain, the same every time. Iris still reads the real text box and writes the answer back;
//! only the writing is made up.
//!
//! What you type decides the answer:
//! - mistakes from `FIXES` ("i have", "its look", "need to"): Fix corrects them (a small fix);
//! - "rewrite all of this": Fix comes back as a different sentence (a big rewrite);
//! - text with none of those: Fix finds nothing to change ("looks good");
//! - a draft mostly in another language: it comes back marked as translated, with no verdict;
//! - any tone: the fixed text with the tone's name in front, so every tone has something to paste.
//!
//! The brain is never started (`brain::start`): its entity is seeded as a Mac that has finished
//! setup (the five built-in tones, a model on disk), and each check (`brain::check`) is answered
//! here after `DELAY`.

use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::Duration;

use gpui::*;

use crate::brain::{self, Brain, Engine, Event, Mode, ModelInfo};
use crate::model::{Answer, Seg, Verdict};

/// How long an answer takes, so the panel goes through its working state first.
pub const DELAY: Duration = Duration::from_millis(350);

pub const FIXES: [(&str, &str); 5] = [("i have", "I have"), ("its look", "it looks"), ("need to", "needs to"), ("teh ", "the "), ("recieve", "receive")];
pub const REWRITE_CUE: &str = "rewrite all of this";
const REWRITTEN: &str = "Here is a completely different sentence that says the same thing in other words.";
/// Drafts in another language the stand-in can "translate", so a translated answer reads as one.
const TRANSLATIONS: [(&str, &str); 1] = [("bhai order kal tak aa jayega, customer ko bol do", "The order will arrive by tomorrow, please tell the customer.")];

/// The model the seeded brain has on disk (the one a new Mac gets).
pub const MODEL: &str = "mlx-community/gemma-4-E4B-it-assistant-bf16";

/// `--mock-answers` was asked for.
pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::args().any(|a| a == "--mock-answers"))
}

/// The built-in tones a new Mac shows, Fix first (`Mode.builtInTones`' first four).
pub fn tones() -> Vec<Mode> {
    [("fix", "Fix"), ("professional", "Professional"), ("friendly", "Friendly"), ("concise", "Shorter"), ("clearer", "Clearer")]
        .iter()
        .map(|(id, label)| Mode { id: (*id).into(), label: (*label).into(), instruction: String::new() })
        .collect()
}

thread_local! {
    static APP: RefCell<Option<AsyncApp>> = const { RefCell::new(None) };
}

/// Seeds the brain's entity as a set-up Mac's, and keeps a handle for answering later.
pub fn start(brain: &Entity<Brain>, cx: &mut App) {
    APP.with(|a| *a.borrow_mut() = Some(cx.to_async()));
    brain.update(cx, |b, cx| {
        b.alive = true;
        b.models = vec![ModelInfo { id: MODEL.into(), download_size: "3.4 GB".into(), downloaded: true }];
        b.recommended = Some(MODEL.into());
        b.chosen = Some(MODEL.into());
        b.tones = tones();
        b.starting = "fix".into();
        b.engine = Engine::Idle;
        cx.notify();
    });
}

/// Answers check `id` after `DELAY` (unless it's cancelled first: `brain::deliver` finds no one).
pub fn answer_later(id: i64, text: &str, mode: &Mode) {
    let Some(acx) = APP.with(|a| a.borrow().clone()) else { return };
    let (text, mode) = (text.to_string(), mode.clone());
    acx.clone().foreground_executor().spawn(async move {
        acx.background_executor().timer(DELAY).await;
        let foreign = looks_non_english(&text);
        let (a, changed) = answer(&text, &mode, foreign);
        let _ = acx.update(|cx| brain::deliver(id, Event::Finished(a, changed), cx));
    }).detach();
}

/// The canned answer for `text` in `mode` (`MockAnswers.answer`), and how many words it changed.
pub fn answer(text: &str, mode: &Mode, translated: bool) -> (Answer, usize) {
    let mut fixed = text.to_string();
    for (wrong, right) in FIXES {
        fixed = fixed.replace(wrong, right);
    }
    if text.to_lowercase().contains(REWRITE_CUE) {
        fixed = REWRITTEN.into();
    }
    if translated && let Some((_, english)) = TRANSLATIONS.iter().find(|(from, _)| text.trim() == *from) {
        fixed = english.to_string();
    }
    let is_fix = mode.id == "fix";
    if is_fix && !translated {
        let segments = crate::model::diff(text, &fixed);
        let verdict = verdict(text, &fixed, &segments);
        let changed = changed_words(&segments);
        return (Answer { fix: true, text: fixed, segments, verdict: Some(verdict), translated: false, keep: vec![] }, changed);
    }
    let toned = if is_fix { fixed } else { format!("{}: {fixed}", mode.label) };
    let segments = crate::model::diff(text, &toned);
    let changed = changed_words(&segments);
    (Answer { fix: is_fix, text: toned, segments, verdict: None, translated, keep: vec![] }, changed)
}

/// `FixGuard.evaluate`'s verdict: nothing changed, a real proofread, or a rewrite (more than
/// 40% of the words changed, on a draft of six words or more).
fn verdict(original: &str, corrected: &str, segments: &[Seg]) -> Verdict {
    let total = words(original).max(1);
    if corrected == original || segments.iter().all(|s| matches!(s, Seg::Same(_))) {
        Verdict::Unchanged
    } else if total >= 6 && changed_words(segments) as f64 / total as f64 > 0.4 {
        Verdict::Heavy
    } else {
        Verdict::Minor
    }
}

fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// `FixGuard.changedWords`: each run of changes counts the larger of what it removed and added.
pub fn changed_words(segments: &[Seg]) -> usize {
    let (mut changed, mut removed, mut added) = (0, 0, 0);
    for s in segments {
        match s {
            Seg::Removed(t) => removed += words(t),
            Seg::Added(t) => added += words(t),
            Seg::Same(_) => {
                changed += removed.max(added);
                removed = 0;
                added = 0;
            }
        }
    }
    changed + removed.max(added)
}

/// `LanguageCheck.looksNonEnglish`: mostly non-Latin letters, or (four lowercase words or more)
/// the Mac's language recognizer gives English under 0.3. (Swift also asks the spell checker how
/// many words it doesn't know; the canned drafts don't need it.)
pub fn looks_non_english(text: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 8 {
        return false;
    }
    let latin = letters.iter().filter(|c| (**c as u32) < 0x250).count();
    if (latin as f64) / (letters.len() as f64) < 0.6 {
        return true;
    }
    let lower = text.split(|c: char| !c.is_alphabetic() && c != '\'')
        .filter(|w| w.chars().count() > 1 && !w.chars().next().is_some_and(char::is_uppercase))
        .count();
    if lower < 4 {
        return false;
    }
    english_share(text) < 0.3
}

/// How English `NLLanguageRecognizer` thinks `text` is, 0 to 1.
#[cfg(target_os = "macos")]
fn english_share(text: &str) -> f64 {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    #[link(name = "NaturalLanguage", kind = "framework")]
    unsafe extern "C" {}
    let Some(class) = AnyClass::get(c"NLLanguageRecognizer") else { return 1.0 };
    unsafe {
        let r: Option<Retained<AnyObject>> = msg_send![class, new];
        let Some(r) = r else { return 1.0 };
        let s = NSString::from_str(text);
        let _: () = msg_send![&*r, processString: &*s];
        let hyp: *mut AnyObject = msg_send![&*r, languageHypothesesWithMaximum: 3usize];
        if hyp.is_null() {
            return 0.0;
        }
        let en = NSString::from_str("en");
        let n: *mut AnyObject = msg_send![hyp, objectForKey: &*en];
        if n.is_null() { 0.0 } else { msg_send![n, doubleValue] }
    }
}

#[cfg(not(target_os = "macos"))]
fn english_share(_text: &str) -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::{REWRITTEN, answer, looks_non_english, tones};
    use crate::brain::Mode;
    use crate::model::Verdict;

    // The drafts scripts/verify/journeys.py puts in the practice box.
    const FIXABLE: &str = "hey, i have checked the PR and its look fine but one thing need to change before merge";
    const FIXED: &str = "hey, I have checked the PR and it looks fine but one thing needs to change before merge";
    const CLEAN: &str = "Can you review this when you get a chance?";
    const REWRITE: &str = "please rewrite all of this, i have a draft that need to change";
    const FOREIGN: &str = "bhai order kal tak aa jayega, customer ko bol do";

    fn fix() -> Mode {
        Mode::fix()
    }

    #[test]
    fn fixable_is_a_small_fix() {
        let (a, changed) = answer(FIXABLE, &fix(), looks_non_english(FIXABLE));
        assert_eq!(a.text, FIXED);
        assert_eq!(a.verdict, Some(Verdict::Minor));
        assert!(!a.translated);
        assert_eq!(changed, 4);
    }

    #[test]
    fn clean_is_unchanged() {
        let (a, _) = answer(CLEAN, &fix(), looks_non_english(CLEAN));
        assert_eq!(a.text, CLEAN);
        assert_eq!(a.verdict, Some(Verdict::Unchanged));
    }

    #[test]
    fn rewrite_is_heavy() {
        let (a, _) = answer(REWRITE, &fix(), looks_non_english(REWRITE));
        assert_eq!(a.text, REWRITTEN);
        assert_eq!(a.verdict, Some(Verdict::Heavy));
    }

    #[test]
    fn foreign_is_translated() {
        assert!(looks_non_english(FOREIGN));
        let (a, _) = answer(FOREIGN, &fix(), true);
        // Translated, so the panel's "Translated to English" reads true.
        assert_eq!(a.text, "The order will arrive by tomorrow, please tell the customer.");
        assert!(a.translated);
        assert_eq!(a.verdict, None);
    }

    #[test]
    fn a_tone_names_itself() {
        let t = &tones()[1];
        let (a, _) = answer(FIXABLE, t, false);
        assert_eq!(a.text, format!("Professional: {FIXED}"));
        assert!(!a.fix);
    }
}
