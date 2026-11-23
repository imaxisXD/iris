//! Windows: the English dictionaries Fix and the language check lean on, from Windows' own
//! spell checker (ISpellChecker, Windows 8 and later): American English always, British English
//! too when Windows has it (Indian English spells the British way, as on the Mac).
//!
//! The Mac computes these in the brain (NSSpellChecker, NLLanguageRecognizer); on Windows the
//! app does, and sends them with each check (`brain::check`).

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Globalization::{ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

pub struct Dictionaries {
    american: Option<ISpellChecker>,
    british: Option<ISpellChecker>,
    com_init: bool,
}

impl Drop for Dictionaries {
    fn drop(&mut self) {
        // Release COM interfaces before uninitializing COM on this thread.
        self.american = None;
        self.british = None;
        if self.com_init {
            unsafe { CoUninitialize() };
        }
    }
}

impl Dictionaries {
    pub fn open() -> Dictionaries {
        unsafe {
            let com_init = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let Ok(f) = CoCreateInstance::<_, ISpellCheckerFactory>(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER) else {
                return Dictionaries { american: None, british: None, com_init };
            };
            let make = |tag: &str| {
                let tag = HSTRING::from(tag);
                if f.IsSupported(&tag).map(|b| b.as_bool()).unwrap_or(false) { f.CreateSpellChecker(&tag).ok() } else { None }
            };
            Dictionaries { american: make("en-US"), british: make("en-GB"), com_init }
        }
    }

    pub fn available(&self) -> bool {
        self.american.is_some() || self.british.is_some()
    }

    fn flags(checker: &ISpellChecker, text: &str) -> Vec<(usize, usize)> {
        let mut out = vec![];
        unsafe {
            let Ok(errors) = checker.Check(&HSTRING::from(text)) else { return out };
            loop {
                let mut e = None;
                if errors.Next(&mut e).is_err() {
                    break;
                }
                let Some(e) = e else { break };
                if let (Ok(start), Ok(len)) = (e.StartIndex(), e.Length()) {
                    out.push((start as usize, len as usize));
                }
            }
        }
        out
    }

    /// Whether either dictionary knows the word.
    pub fn knows(&self, word: &str) -> bool {
        let checkers: Vec<&ISpellChecker> = [&self.american, &self.british].into_iter().flatten().collect();
        checkers.is_empty() || checkers.iter().any(|c| Self::flags(c, word).is_empty())
    }

    /// The words the American dictionary flags (and the British one doesn't know either), with
    /// where each starts (in UTF-16 units, as Swift's String.utf16 counts) and its guesses.
    pub fn misspelled(&self, text: &str) -> Vec<(String, usize, Vec<String>)> {
        let Some(us) = self.american.as_ref().or(self.british.as_ref()) else { return vec![] };
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let mut out = vec![];
        for (start, len) in Self::flags(us, text) {
            let Some(slice) = utf16.get(start..start + len) else { continue };
            let word = String::from_utf16_lossy(slice);
            if self.knows(&word) {
                continue;
            }
            let guesses = self.guesses(&word);
            out.push((word, start, guesses));
        }
        out
    }

    /// Up to three guesses, the British dictionary's first (as the Mac asks).
    fn guesses(&self, word: &str) -> Vec<String> {
        let Some(c) = self.british.as_ref().or(self.american.as_ref()) else { return vec![] };
        let mut out = vec![];
        unsafe {
            let Ok(list) = c.Suggest(&HSTRING::from(word)) else { return out };
            while out.len() < 3 {
                let mut item = [PWSTR::null()];
                let mut got = 0u32;
                if list.Next(&mut item, Some(&mut got)).is_err() || got == 0 {
                    break;
                }
                if let Ok(s) = item[0].to_string() {
                    out.push(s);
                }
                CoTaskMemFree(Some(item[0].0 as _));
            }
        }
        out
    }
}

/// `LanguageCheck.looksNonEnglish` for Windows, without the Mac's language recognizer: a
/// non-Latin script, or (romanized Hindi) enough Hinglish words and words no English dictionary
/// knows. The thresholds are tuned against the Mac's own answers.
pub fn looks_foreign(text: &str, dict: &Dictionaries) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 8 {
        return false;
    }
    let latin = letters.iter().filter(|c| (**c as u32) < 0x250).count();
    if (latin as f64) / (letters.len() as f64) < 0.6 {
        return true;
    }
    let words: Vec<String> = text.split(|c: char| !c.is_alphabetic() && c != '\'')
        .filter(|w| w.chars().count() > 1 && !w.chars().next().is_some_and(char::is_uppercase))
        .map(str::to_string).collect();
    if words.len() < 4 || !dict.available() {
        return false;
    }
    let n = words.len() as f64;
    let hinglish = words.iter().filter(|w| HINGLISH.contains(&w.to_lowercase().as_str())).count() as f64 / n;
    let unknown = words.iter().filter(|w| !dict.knows(w)).count() as f64 / n;
    // Tuned on 3,214 drafts against the Mac's own answers: 99.7% the same. Of the 11 others, six
    // are the Mac's misses (Hinglish it took for English, "fyi im wfh tmrw" taken for Hindi);
    // five are Windows', mostly English with a Hinglish word or two ("accha, so the demo is at
    // 3 tomorrow na?").
    (hinglish >= 0.2 && unknown >= 0.25) || (hinglish >= 0.08 && unknown >= 0.5)
}

/// Words of romanized Hindi that are never English words, the ones a Hinglish draft can't do
/// without. Ambiguous ones ("main", "do", "to", "me", "the") are left out.
pub const HINGLISH: &[&str] = &[
    "aa", "aaj", "aana", "aap", "aapka", "aapki", "aata", "aate", "aati", "aaya", "aaye", "aayi", "abe",
    "abhi", "accha", "acha", "achha", "agle", "agli", "apna", "apne", "apni", "arre", "aur", "baad", "baar",
    "bahut", "baje", "bas", "bata", "batao", "bataya", "bhai", "bhej", "bheja", "bhejo", "bhi", "bohot", "bol",
    "bola", "bolo", "chahiye", "chalo", "de", "dedo", "dekh", "dekha", "dekho", "dena", "di", "dijiye", "din",
    "diya", "dunga", "dungi", "ek", "fir", "gaya", "gaye", "gayi", "ghar", "h", "haan", "haanji", "hafte",
    "hai", "hain", "hamara", "hamare", "hi", "ho", "hoga", "hogi", "hona", "hoon", "hota", "hote", "hoti",
    "hu", "hua", "hui", "hum", "humare", "humein", "humse", "hun", "isko", "itna", "itni", "jaata", "jaate",
    "jaati", "jaega", "jaldi", "jana", "jao", "jayega", "jayegi", "ji", "ka", "kaafi", "kaam", "kab", "kahan",
    "kaise", "kal", "kam", "kar", "karenge", "karke", "karna", "karo", "karta", "karte", "karti", "karunga",
    "karungi", "kaun", "kb", "ke", "ki", "kijiye", "kitna", "kitne", "kiya", "kiye", "ko", "koi", "kuch",
    "kya", "kyon", "kyu", "kyuki", "kyun", "le", "lekin", "lena", "lijiye", "liya", "liye", "lo", "log",
    "lunga", "lungi", "magar", "mai", "mat", "matlab", "mein", "mera", "mere", "meri", "mil", "mila", "mili",
    "mujhe", "mushkil", "na", "nahi", "nahin", "nai", "nhi", "par", "pata", "paunga", "payega", "pe", "pehle",
    "phir", "raat", "raha", "rahe", "rahi", "ruko", "saath", "sab", "sakta", "sakte", "sakti", "samajh", "se",
    "shaam", "shuru", "sirf", "subah", "suno", "teen", "tera", "tere", "teri", "tha", "theek", "thi", "thik",
    "thoda", "tk", "toh", "tu", "tujhe", "tum", "tumhara", "tumhe", "tumko", "unka", "unki", "unko", "usko",
    "utna", "voh", "wala", "wale", "wali", "wapas", "waqt", "wo", "woh", "ya", "yaar", "ye", "yeh", "zyada",
];
