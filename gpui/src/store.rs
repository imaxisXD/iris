//! Iris's settings, saved on this Mac (`AppSettings` in the Swift app). Nothing here leaves it.
//!
//! One JSON file in Application Support, named after the app's bundle ID so a test build never
//! reads or writes the installed Iris's. The names and defaults are the Swift app's; tones,
//! shortcuts and the excluded apps keep the Swift app's JSON shapes (`Mode`, `KeyCombo`,
//! `AppRef`), so the brain (the Swift helper) reads them as they are.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `KeyCombo`: a Carbon key code and modifier mask, as the Swift app stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCombo {
    pub key_code: u32,
    pub modifiers: u32,
}

/// Carbon's modifier bits (`cmdKey`, `shiftKey`, `optionKey`, `controlKey`).
pub mod carbon {
    pub const CMD: u32 = 1 << 8;
    pub const SHIFT: u32 = 1 << 9;
    pub const OPTION: u32 = 1 << 11;
    pub const CONTROL: u32 = 1 << 12;
    pub const KEY_G: u32 = 0x05;
    pub const KEY_I: u32 = 0x22;
}

impl KeyCombo {
    /// ⌃⌥G.
    pub const OPEN: KeyCombo = KeyCombo { key_code: carbon::KEY_G, modifiers: carbon::CONTROL | carbon::OPTION };
    /// ⌃⌥⇧G.
    pub const QUICK_FIX: KeyCombo = KeyCombo { key_code: carbon::KEY_G, modifiers: carbon::CONTROL | carbon::OPTION | carbon::SHIFT };
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    #[serde(rename = "bundleID")]
    pub bundle_id: String,
    pub name: String,
}

/// Where the panel opens (`PanelPlacement`, stored by its Swift names).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Placement {
    AboveField,
    BelowField,
    NearPointer,
}

/// How long Iris remembers checked answers in memory (never written to disk).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnswerCache {
    Off,
    WhileOpen,
    #[default]
    Recent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub placement: Placement,
    pub show_panel_when_clean: bool,
    pub show_changes: bool,
    pub answer_cache: AnswerCache,
    /// The notes turned off: "undo" (replaced), "undoKeys", "checking", "clean", "excluded".
    pub hidden_notes: Vec<String>,
    pub open_shortcut: KeyCombo,
    pub quick_fix_shortcut: Option<KeyCombo>,
    /// `ToneList`'s parts, in the Swift app's shapes (read by the brain): custom tones, their
    /// order, the hidden ones, the starting tone, and the starting tone per app.
    pub custom_tones: Option<Value>,
    pub tone_order: Option<Vec<String>>,
    pub hidden_tones: Option<Vec<String>>,
    pub default_mode: Option<String>,
    pub app_modes: Option<Value>,
    pub style_rules: Vec<String>,
    pub glossary: Vec<String>,
    pub excluded_apps: Vec<AppRef>,
    /// `ModelChoice`'s raw value; none means the one recommended for this Mac.
    pub model: Option<String>,
    pub idle_minutes: u32,
    pub load_at_launch: bool,
    /// "Go easy on this Mac / PC": leaner model memory and quieter motion. `None` picks
    /// automatically (on for machines with 8 GB of memory or less).
    pub light_mode: Option<bool>,
    pub onboarded: bool,
    /// How many times the flower has arrived on this Mac (`BudShow`'s `budArrivals`), for
    /// `opening::arrival_is_full`; none: not counted here yet (the Swift app's count is used).
    pub bud_arrivals: Option<u32>,
    /// The start of the day it last arrived, in seconds since 1970 (`budLastDay`, as Swift
    /// stored it: local midnight).
    pub bud_last_day: Option<f64>,
}

/// Whether this is the Iris people install (`local.iris.app`), not a development build. A
/// development build starts on ⌃⌥I and ⌃⌥⇧I, so it never fights the installed Iris for ⌃⌥G.
pub fn is_installed_iris() -> bool {
    bundle_id() == "local.iris.app"
}

impl Default for Settings {
    fn default() -> Self {
        let app = |b: &str, n: &str| AppRef { bundle_id: b.into(), name: n.into() };
        let dev = |k: KeyCombo| if is_installed_iris() { k } else { KeyCombo { key_code: carbon::KEY_I, ..k } };
        Settings {
            placement: Placement::AboveField,
            show_panel_when_clean: true,
            show_changes: true,
            answer_cache: AnswerCache::Recent,
            hidden_notes: vec![],
            // ⌃⌥G and ⌃⌥⇧G on the Mac; Windows has its own (`keys::open_default`).
            open_shortcut: dev(crate::keys::open_default()),
            quick_fix_shortcut: Some(dev(crate::keys::quick_fix_default())),
            custom_tones: None,
            tone_order: None,
            hidden_tones: None,
            default_mode: None,
            app_modes: None,
            style_rules: vec![],
            glossary: vec![],
            // Password managers: never read. On Windows an app is its program's file name.
            #[cfg(not(windows))]
            excluded_apps: vec![
                app("com.1password.1password", "1Password"),
                app("com.apple.keychainaccess", "Keychain Access"),
                app("com.apple.Passwords", "Passwords"),
                app("com.bitwarden.desktop", "Bitwarden"),
            ],
            #[cfg(windows)]
            excluded_apps: vec![
                app("1password.exe", "1Password"),
                app("bitwarden.exe", "Bitwarden"),
                app("keepass.exe", "KeePass"),
                app("keepassxc.exe", "KeePassXC"),
                app("credentialuibroker.exe", "Windows Security"),
            ],
            model: None,
            idle_minutes: 30,
            load_at_launch: true,
            light_mode: None,
            onboarded: false,
            bud_arrivals: None,
            bud_last_day: None,
        }
    }
}

impl Settings {
    /// Whether "Go easy on this Mac / PC" is active: the user's explicit choice, or
    /// automatically on for machines with 8 GB of memory or less.
    pub fn light_mode_active(&self) -> bool {
        self.light_mode.unwrap_or_else(|| crate::model_compare::mac_memory_gb() <= 8)
    }

    /// Whether a note shows (problems and one-time notes always do).
    pub fn shows(&self, note: &str) -> bool {
        !self.hidden_notes.iter().any(|n| n == note)
    }

    pub fn is_excluded(&self, bundle_id: Option<&str>) -> bool {
        bundle_id.is_some_and(|b| self.excluded_apps.iter().any(|a| a.bundle_id == b))
    }

    /// The settings saved on this Mac, or the defaults (a file that can't be read is kept
    /// aside as `.unreadable`, and the defaults used).
    pub fn load() -> Settings {
        let path = file();
        let Ok(bytes) = std::fs::read(&path) else {
            // The first start of this Iris where the Swift app was: its settings come along.
            if is_installed_iris() && std::env::var("IRIS_HOME").is_err()
                && let Some(s) = from_swift_app()
            {
                eprintln!("settings: brought over from the earlier Iris");
                let _ = s.save();
                return s;
            }
            return Settings::default();
        };
        match Settings::parse(&bytes) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("settings: can't read {}: {e}; using the defaults", path.display());
                let _ = std::fs::rename(&path, path.with_extension("unreadable"));
                Settings::default()
            }
        }
    }

    /// The settings in a file's bytes. A file saved by another editor may start with a byte
    /// order mark (Notepad and PowerShell write one): the settings after it are still yours.
    fn parse(bytes: &[u8]) -> serde_json::Result<Settings> {
        serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
    }

    /// Saves them: written beside the file, then moved over it, so a crash never leaves half.
    pub fn save(&self) -> std::io::Result<()> {
        let path = file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }
}

/// The Swift app's count of the flower's arrivals and the day of the last one, for an Iris
/// that moved over before it counted them itself (its settings came along without them).
pub fn swift_bud_arrivals() -> (Option<u32>, Option<f64>) {
    if !is_installed_iris() || std::env::var("IRIS_HOME").is_ok() {
        return (None, None);
    }
    swift_preferences("local.iris.app")
        .and_then(swift_shapes)
        .map_or((None, None), |s| (s.bud_arrivals, s.bud_last_day))
}

/// The settings the Swift app kept in its preferences (`AppSettings`, domain `local.iris.app`),
/// in this app's shapes: the same names (the glossary was `glossaryList`), its JSON blobs (the
/// shortcuts, custom tones, per-app tones, the apps Iris is off in) read as JSON. None when
/// it left nothing.
pub fn from_swift_app() -> Option<Settings> {
    let stored = swift_preferences("local.iris.app")?;
    swift_shapes(stored)
}

/// Turns the Swift app's stored values into this app's settings (missing ones take defaults).
pub fn swift_shapes(mut stored: serde_json::Map<String, Value>) -> Option<Settings> {
    if stored.is_empty() {
        return None;
    }
    if let Some(g) = stored.remove("glossaryList") {
        stored.insert("glossary".into(), g);
    }
    // The flower's count and day are bookkeeping: one that doesn't read as a count or a time
    // is left behind, never the reason the rest can't come along.
    if stored.get("budArrivals").is_some_and(|v| v.as_u64().is_none_or(|n| n > u32::MAX as u64)) {
        stored.remove("budArrivals");
    }
    if stored.get("budLastDay").is_some_and(|v| !v.is_number()) {
        stored.remove("budLastDay");
    }
    // An explicit "no Quick Fix shortcut" is stored as JSON null; nothing stored is the default.
    let mut s: Settings = serde_json::from_value(Value::Object(stored.clone())).ok()?;
    if !stored.contains_key("quickFixShortcut") {
        s.quick_fix_shortcut = Settings::default().quick_fix_shortcut;
    }
    Some(s)
}

#[cfg(target_os = "macos")]
fn swift_preferences(domain: &str) -> Option<serde_json::Map<String, Value>> {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFStringRef, app: CFStringRef) -> CFTypeRef;
    }
    let app = CFString::new(domain);
    let keys = ["placement", "showPanelWhenClean", "showChanges", "hiddenNotes", "openShortcut", "quickFixShortcut",
                "customTones", "toneOrder", "hiddenTones", "defaultMode", "appModes", "styleRules", "glossaryList",
                "excludedApps", "model", "idleMinutes", "loadAtLaunch", "onboarded", "budArrivals", "budLastDay"];
    let mut out = serde_json::Map::new();
    for key in keys {
        let k = CFString::new(key);
        let v = unsafe { CFPreferencesCopyAppValue(k.as_concrete_TypeRef(), app.as_concrete_TypeRef()) };
        if v.is_null() {
            continue;
        }
        let v = unsafe { CFType::wrap_under_create_rule(v) };
        if let Some(j) = plist_to_json(&v) {
            out.insert(key.into(), j);
        }
    }
    Some(out)
}

#[cfg(not(target_os = "macos"))]
fn swift_preferences(_domain: &str) -> Option<serde_json::Map<String, Value>> {
    None
}

/// A preference value as JSON: strings, booleans, numbers, lists; data is the Swift app's
/// JSON encoding of a value, read as that value.
#[cfg(target_os = "macos")]
fn plist_to_json(v: &core_foundation::base::CFType) -> Option<Value> {
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    if let Some(s) = v.downcast::<CFString>() {
        return Some(Value::String(s.to_string()));
    }
    if let Some(b) = v.downcast::<CFBoolean>() {
        return Some(Value::Bool(b.into()));
    }
    if let Some(n) = v.downcast::<CFNumber>() {
        return n.to_i64().map(Value::from).or_else(|| n.to_f64().map(Value::from));
    }
    if let Some(d) = v.downcast::<CFData>() {
        return serde_json::from_slice(d.bytes()).ok();
    }
    if let Some(a) = v.downcast::<CFArray>() {
        let items = a.iter().map(|p| unsafe { CFType::wrap_under_get_rule(*p as core_foundation::base::CFTypeRef) });
        return Some(Value::Array(items.filter_map(|x| plist_to_json(&x)).collect()));
    }
    None
}

/// The app's bundle ID (`local.iris.gpui` outside a bundle).
pub fn bundle_id() -> String {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(read_bundle_id).clone()
}

/// Windows: the installed Iris runs from the installer's folder (%LOCALAPPDATA%\Programs\Iris);
/// any other copy is a development one, with its own settings and its own shortcut.
#[cfg(windows)]
fn read_bundle_id() -> String {
    let installed = std::env::var("LOCALAPPDATA").ok().map(|l| PathBuf::from(l).join("Programs").join("Iris"));
    let here = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf()));
    match (installed, here) {
        (Some(i), Some(h)) if h.eq_ignore_ascii_case_path(&i) => "local.iris.app".into(),
        _ => "local.iris.gpui".into(),
    }
}

#[cfg(windows)]
trait SamePath {
    fn eq_ignore_ascii_case_path(&self, other: &std::path::Path) -> bool;
}

#[cfg(windows)]
impl SamePath for PathBuf {
    fn eq_ignore_ascii_case_path(&self, other: &std::path::Path) -> bool {
        self.to_string_lossy().eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

#[cfg(not(windows))]
fn read_bundle_id() -> String {
    std::env::current_exe().ok()
        .and_then(|exe| {
            let plist = exe.parent()?.parent()?.join("Info.plist");
            let out = std::process::Command::new("/usr/libexec/PlistBuddy").args(["-c", "Print :CFBundleIdentifier"]).arg(&plist).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "local.iris.gpui".into())
}

/// Where Iris keeps its files on this Mac (`IRIS_HOME` overrides it, for tests).
pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("IRIS_HOME") {
        return PathBuf::from(h);
    }
    // Windows: %APPDATA%\Iris (roaming with the person's profile), a development copy beside it.
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join(if is_installed_iris() { "Iris" } else { "Iris (development)" });
    }
    let base = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join("Library/Application Support").join(bundle_id())
}

pub fn file() -> PathBuf {
    home().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_read_swift_shapes() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        // The Swift app's KeyCombo and AppRef encodings.
        let k: KeyCombo = serde_json::from_str(r#"{"keyCode":5,"modifiers":6144}"#).unwrap();
        assert_eq!(k, KeyCombo::OPEN);
        // A development build (not local.iris.app) starts on ⌃⌥I.
        assert_eq!(Settings::default().open_shortcut.key_code, carbon::KEY_I);
        let a: AppRef = serde_json::from_str(r#"{"bundleID":"x","name":"X"}"#).unwrap();
        assert_eq!(a.bundle_id, "x");
        // A file written by an older build, missing fields, takes the defaults for them.
        let old: Settings = serde_json::from_str(r#"{"glossary":["Northwind"]}"#).unwrap();
        assert_eq!(old.glossary, vec!["Northwind".to_string()]);
        assert_eq!(old.idle_minutes, 30);
        assert_eq!(old.light_mode, None);
        assert_eq!(old.answer_cache, AnswerCache::Recent);
        // Explicit lightMode choice overrides the hardware default and round-trips cleanly.
        let mut explicit = Settings::default();
        explicit.light_mode = Some(true);
        assert!(explicit.light_mode_active());
        let rt: Settings = serde_json::from_str(&serde_json::to_string(&explicit).unwrap()).unwrap();
        assert_eq!(rt.light_mode, Some(true));
        explicit.light_mode = Some(false);
        assert!(!explicit.light_mode_active());
        // AnswerCache serializes cleanly across all three options.
        for (raw, want) in [(r#""off""#, AnswerCache::Off), (r#""whileOpen""#, AnswerCache::WhileOpen), (r#""recent""#, AnswerCache::Recent)] {
            assert_eq!(serde_json::to_string(&want).unwrap(), raw);
            assert_eq!(serde_json::from_str::<AnswerCache>(raw).unwrap(), want);
        }
    }

    /// Reads this Mac's Swift Iris settings (read-only): `cargo test print_swift_settings -- --ignored --nocapture`.
    #[test]
    fn reads_a_file_that_starts_with_a_byte_order_mark() {
        let s = Settings::parse(b"\xEF\xBB\xBF{\"onboarded\": true, \"glossary\": [\"Northwind\"]}").unwrap();
        assert!(s.onboarded);
        assert_eq!(s.glossary, vec!["Northwind".to_string()]);
        assert!(Settings::parse(b"\xEF\xBB\xBFnot json").is_err());
    }

    #[test]
    #[ignore]
    fn print_swift_settings() {
        let s = from_swift_app();
        println!("{s:#?}");
    }

    #[test]
    fn brings_over_the_swift_apps_settings() {
        let stored: serde_json::Map<String, Value> = serde_json::from_str(r#"{
            "placement": "belowField", "onboarded": true, "model": "best", "hiddenNotes": ["undo"],
            "glossaryList": ["Northwind"], "openShortcut": {"keyCode": 5, "modifiers": 4096},
            "quickFixShortcut": null, "idleMinutes": 60,
            "excludedApps": [{"bundleID": "com.example", "name": "Example"}]
        }"#).unwrap();
        let s = swift_shapes(stored).unwrap();
        assert_eq!(s.placement, Placement::BelowField);
        assert!(s.onboarded);
        assert_eq!(s.glossary, vec!["Northwind".to_string()]);
        assert_eq!(s.open_shortcut, KeyCombo { key_code: 5, modifiers: 4096 });
        assert_eq!(s.quick_fix_shortcut, None);
        assert_eq!(s.idle_minutes, 60);
        assert_eq!(s.excluded_apps[0].bundle_id, "com.example");
        // Nothing stored about Quick Fix: its default keys.
        // The flower's count comes along; one that isn't a count is left behind, never the rest.
        let s = swift_shapes(serde_json::from_str(r#"{"onboarded": true, "budArrivals": 101, "budLastDay": 1794940200}"#).unwrap()).unwrap();
        assert_eq!((s.bud_arrivals, s.bud_last_day), (Some(101), Some(1794940200.0)));
        let s = swift_shapes(serde_json::from_str(r#"{"onboarded": true, "budArrivals": -3, "budLastDay": "x"}"#).unwrap()).unwrap();
        assert!(s.onboarded && s.bud_arrivals.is_none() && s.bud_last_day.is_none());
        let s = swift_shapes(serde_json::from_str(r#"{"onboarded": true}"#).unwrap()).unwrap();
        assert_eq!(s.quick_fix_shortcut, Settings::default().quick_fix_shortcut);
    }
}
