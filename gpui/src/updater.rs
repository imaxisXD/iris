//! Check for Updates (`Updater`, `UpdateCheck` in the Swift app). Iris is built from a git
//! checkout on each Mac, so an update means pulling from GitHub and rebuilding; a commit made in
//! the checkout counts too, until this copy is rebuilt with it. scripts/build-app.sh records the
//! checkout's path in Info.plist (`IrisSourcePath`) with the commit it was built from. A copy
//! downloaded as a release (`IrisChannel` = release) asks GitHub's latest release instead, and
//! installs it itself, as Zed's updater does: it downloads the release's Iris.zip, checks it's
//! Iris, that version, signed by whoever signed this copy, then quits while a small script puts
//! it in this copy's place and opens it (`install_release`). Nobody needs Rust or Xcode.
//!
//! One check at a time, for everyone who shows it (the slip's Updates page, Settings › Updates):
//! `start` looks, `phase` says where it stands, and `State` (a global entity) tells them when it
//! changes. Under test it never asks GitHub: the answer is `IRIS_UPDATE_ANSWER` (up-to-date,
//! behind, release, failed, no-checkout, no-remote; up-to-date when unset), as the Swift app's
//! check on the test display is always up to date.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gpui::*;

use crate::textbridge::under_test;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    UpToDate,
    /// How many commits this copy is missing, and their subjects (newest first, at most 5).
    Behind(usize, Vec<String>),
    NoCheckout,
    /// A downloaded copy: a newer release (its tag), its page, and its Iris.zip when this copy
    /// can install it itself (none: download it from the page).
    NewRelease(String, String, Option<String>),
    NoRemote,
    Failed(String),
    /// A downloaded release that wasn't installed (why, in words); this copy is unchanged.
    InstallFailed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Looking(Instant),
    Answered(Status, Instant),
    /// Update & reopen: a new Iris building since then.
    Updating(Instant),
}

/// Where the check stands, for anything that wants to watch it.
pub struct State {
    pub phase: Phase,
}

struct Handle(Entity<State>);
impl Global for Handle {}

/// The check's state (made the first time it's asked for).
pub fn state(cx: &mut App) -> Entity<State> {
    if let Some(h) = cx.try_global::<Handle>() {
        return h.0.clone();
    }
    let e = cx.new(|_| State { phase: Phase::Idle });
    cx.set_global(Handle(e.clone()));
    e
}

pub fn phase(cx: &App) -> Phase {
    cx.try_global::<Handle>().map_or(Phase::Idle, |h| h.0.read(cx).phase.clone())
}

// MARK: - This copy

fn info(key: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let plist = exe.parent()?.parent()?.join("Info.plist");
    let out = Command::new("/usr/libexec/PlistBuddy").args(["-c", &format!("Print :{key}")]).arg(&plist).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// "0.1.0", or "dev" for a build without one.
pub fn version() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| info("CFBundleShortVersionString").unwrap_or_else(|| "dev".into())).clone()
}

/// The commit this copy was built from, short.
pub fn commit() -> Option<String> {
    static C: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    C.get_or_init(|| info("IrisCommit").filter(|c| c != "unknown")).clone()
}

/// The checkout this copy was built from, if it's still there.
pub fn source_path() -> Option<String> {
    let path = info("IrisSourcePath")?;
    std::path::Path::new(&path).join("scripts/update.sh").exists().then_some(path)
}

// MARK: - The check

/// Looks for updates: fetches, then compares the commit this copy was built from with GitHub
/// and the checkout. Blocks (git and the network): run it off the main thread.
pub fn check() -> Status {
    // Under test, a stand-in for GitHub on this Mac (`IRIS_RELEASES_API`) is asked for real.
    if under_test() && local_releases().is_some() {
        return check_releases();
    }
    if under_test() {
        return match std::env::var("IRIS_UPDATE_ANSWER").as_deref() {
            Ok("behind") => Status::Behind(2, vec!["The menu bar flower in pen".into(), "Names each fix in the margin".into()]),
            Ok("release") => Status::NewRelease("v0.2.0".into(), RELEASES.into(), Some(format!("{DOWNLOADS}v0.2.0/Iris.zip"))),
            Ok("failed") => Status::Failed(OFFLINE.into()),
            Ok("no-checkout") => Status::NoCheckout,
            Ok("no-remote") => Status::NoRemote,
            _ => Status::UpToDate,
        };
    }
    match source_path() {
        Some(path) => status_of(&path, commit().as_deref()),
        None => check_releases(),
    }
}

/// Where releases live, and the latest one as GitHub's API gives it.
pub const RELEASES: &str = "https://github.com/imaxisXD/iris/releases/latest";
const LATEST_API: &str = "https://api.github.com/repos/imaxisXD/iris/releases/latest";
/// Where a release's files are (`browser_download_url`): the only place Iris downloads from.
const DOWNLOADS: &str = "https://github.com/imaxisXD/iris/releases/download/";

/// For a test: `IRIS_RELEASES_API` on this Mac (`http://127.0.0.1:<port>/…` only) stands in for
/// GitHub, and its files may come from there too.
fn local_releases() -> Option<String> {
    std::env::var("IRIS_RELEASES_API").ok().filter(|u| u.starts_with("http://127.0.0.1:"))
}
const OFFLINE: &str = "Couldn't reach GitHub. Check your connection and try again.";

/// A downloaded copy: its version against GitHub's latest release. Nothing about you or your
/// text is sent; only when you choose Check for Updates.
fn check_releases() -> Status {
    if info("IrisChannel").as_deref() != Some("release") && local_releases().is_none() {
        return Status::NoCheckout;
    }
    let out = Command::new("/usr/bin/curl")
        .args(["--silent", "--max-time", "15", "--header", "Accept: application/vnd.github+json", "--write-out", "\n%{http_code}"])
        .arg(local_releases().unwrap_or_else(|| LATEST_API.into()))
        .output();
    let Ok(out) = out else { return Status::Failed(OFFLINE.into()) };
    let body = String::from_utf8_lossy(&out.stdout);
    let (json, code) = body.rsplit_once('\n').unwrap_or((&body, ""));
    if code == "404" {
        return Status::UpToDate; // no release yet
    }
    if !out.status.success() || code != "200" {
        return Status::Failed(OFFLINE.into());
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Status::Failed("GitHub sent an answer Iris couldn't read. Try again later.".into());
    };
    let Some(tag) = v["tag_name"].as_str() else {
        return Status::Failed("GitHub sent an answer Iris couldn't read. Try again later.".into());
    };
    let page = v["html_url"].as_str().filter(|u| u.starts_with("https://github.com/")).unwrap_or(RELEASES).to_string();
    if !is_newer(tag, &version()) {
        return Status::UpToDate;
    }
    // Its Iris.zip (release.sh attaches it under that name), from GitHub's release files only.
    let zip = v["assets"].as_array().into_iter().flatten()
        .find(|a| a["name"].as_str() == Some("Iris.zip"))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(DOWNLOADS) || (local_releases().is_some() && u.starts_with("http://127.0.0.1:")))
        .map(String::from)
        .filter(|_| app_path().is_some_and(|a| can_replace(&a)));
    Status::NewRelease(tag.into(), page, zip)
}

/// `ReleaseVersion.isNewer`: "v0.2.0" against "0.1.3", number by number.
pub fn is_newer(tag: &str, installed: &str) -> bool {
    let parts = |s: &str| -> Vec<u64> { s.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parts(tag), parts(installed));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn status_of(path: &str, built_from: Option<&str>) -> Status {
    if git(&["remote", "get-url", "origin"], path, None).0 != 0 {
        return Status::NoRemote;
    }
    // A stalled connection gives up instead of looking forever: git itself after 15 seconds
    // below 1 KB/s, and the process is stopped at 30 whatever it's doing.
    if git(&["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=15", "fetch", "--quiet", "origin"], path, Some(30)).0 != 0 {
        return Status::Failed(OFFLINE.into());
    }
    // What this copy is missing: new commits on GitHub, and commits made in the checkout since
    // it was built.
    let mut missing: Vec<String> = vec!["HEAD..@{u}".into()];
    if let Some(b) = built_from
        && git(&["cat-file", "-e", &format!("{b}^{{commit}}")], path, None).0 == 0
    {
        missing = vec![format!("^{b}"), "HEAD".into(), "@{u}".into()];
    }
    let refs: Vec<&str> = missing.iter().map(String::as_str).collect();
    let (code, count) = git(&[&["rev-list", "--count"][..], &refs].concat(), path, None);
    let Some(behind) = (code == 0).then(|| count.trim().parse::<usize>().ok()).flatten() else {
        return Status::Failed("Iris doesn't know which version to follow on GitHub yet.".into());
    };
    if behind == 0 {
        return Status::UpToDate;
    }
    let (code, log) = git(&[&["log", "--format=%s", "--max-count=5"][..], &refs].concat(), path, None);
    let subjects = if code == 0 {
        log.lines().map(|l| l.strip_prefix("Iris: ").unwrap_or(l).to_string()).collect()
    } else {
        vec![]
    };
    Status::Behind(behind, subjects)
}

/// git in `path`, never waiting on a password prompt, stopped after `timeout` seconds.
fn git(args: &[&str], path: &str, timeout: Option<u64>) -> (i32, String) {
    let child = Command::new("/usr/bin/git").arg("-C").arg(path).args(args)
        .env("GIT_TERMINAL_PROMPT", "0").env_remove("GIT_DIR").env_remove("GIT_WORK_TREE")
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let Ok(mut child) = child else { return (-1, String::new()) };
    if let Some(t) = timeout {
        let start = Instant::now();
        loop {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            if start.elapsed() > Duration::from_secs(t) {
                let _ = child.kill();
                return (-1, String::new());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    match child.wait_with_output() {
        Ok(o) => (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
        Err(_) => (-1, String::new()),
    }
}

// MARK: - Looking

/// One loop of the looking scene (`UpdateCheckScene.loop`): an answer lands as a loop ends.
const LOOP: f64 = 1.4;

/// Starts a look (one at a time); the slip and Settings follow `State`.
pub fn start(cx: &mut App) {
    let st = state(cx);
    if matches!(st.read(cx).phase, Phase::Looking(_)) {
        return;
    }
    let began = Instant::now();
    st.update(cx, |s, cx| {
        s.phase = Phase::Looking(began);
        cx.notify();
    });
    crate::live::news("updates: looking");
    cx.spawn(async move |cx| {
        let status = cx.background_executor().spawn(async { check() }).await;
        // The scene's loop comes round before the answer shows (`UpdateCheckScene.wait`).
        let elapsed = began.elapsed().as_secs_f64();
        let wait = (elapsed / LOOP).ceil().max(1.0) * LOOP - elapsed;
        cx.background_executor().timer(Duration::from_secs_f64(wait * crate::parts::slow() as f64)).await;
        let _ = cx.update(|cx| {
            crate::live::news(&format!("updates: {status:?}"));
            st.update(cx, |s, cx| {
                s.phase = Phase::Answered(status, Instant::now());
                cx.notify();
            });
        });
    })
    .detach();
}

/// What an answer says, the same in the slip and in Settings (`UpdateCheck.Words`): the title,
/// the lead, whether it's a warning.
pub fn words(status: &Status) -> (String, String, bool) {
    match status {
        Status::UpToDate => ("Up to date".into(), "You have the latest Iris.".into(), false),
        Status::Behind(n, _) => (if *n == 1 { "1 new change".into() } else { format!("{n} new changes") }, "Ready to install.".into(), false),
        Status::NewRelease(tag, _, Some(_)) => (format!("Iris {} is out", tag.trim_start_matches('v')), "Ready to install. Your settings stay.".into(), false),
        Status::NewRelease(tag, _, None) => (format!("Iris {} is out", tag.trim_start_matches('v')), "Download it, then replace Iris in Applications. Your settings stay.".into(), false),
        Status::NoCheckout => ("Can't update".into(), "This copy of Iris wasn't installed from its project folder, so it can't update itself.".into(), true),
        Status::NoRemote => ("Can't update".into(), "Iris doesn't know where to get updates yet: its project folder isn't linked to GitHub.".into(), true),
        Status::Failed(m) => ("Couldn't check".into(), m.chars().take(80).collect(), true),
        Status::InstallFailed(m) => ("Couldn't update".into(), m.chars().take(80).collect(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_number_by_number() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("1.0", "0.9.9"));
    }
}

// MARK: - Installing

/// Update & reopen (`Updater.install`): the checkout's update script (`scripts/update.sh`) pulls,
/// builds and reopens Iris, detached (it quits this one), its lines added to the update log.
/// Never under test: it would replace the app.
pub fn install(cx: &mut App) {
    if under_test() {
        return crate::live::news("updates: an update would have been installed");
    }
    let Some(path) = source_path() else { return };
    let script = std::path::Path::new(&path).join("scripts/update.sh");
    let log = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("Library/Logs/Iris-update.log");
    // Not Iris's own build settings (`UpdateEnvironment.cleaned`): one left over would rebuild
    // the version already installed.
    let started = Command::new("/bin/zsh").arg("-c").arg("nohup \"$0\" >> \"$1\" 2>&1 &").arg(&script).arg(&log)
        .current_dir(&path)
        .env_remove("IRIS_COMMIT").env_remove("IRIS_SOURCE_PATH").env_remove("SIGN_IDENTITY")
        .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").spawn();
    if started.is_err() {
        crate::live::news("updates: the update couldn't start");
        return;
    }
    // The slip's header says Updating while the new Iris builds; the script quits and reopens it.
    let st = state(cx);
    st.update(cx, |s, cx| {
        s.phase = Phase::Updating(Instant::now());
        cx.notify();
    });
}

/// How far along an update is after `elapsed` seconds (`UpdateRun.progress`): about a minute
/// and a quarter usually, never quite done until Iris reopens.
pub fn progress(elapsed: f64) -> f64 {
    0.97 * (1.0 - (-2.0 * elapsed.max(0.0) / 75.0).exp())
}

// MARK: - Installing a release (a downloaded copy)

/// This copy's app bundle (…/Iris.app), from the running program.
fn app_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let app = exe.parent()?.parent()?.parent()?.to_path_buf();
    (app.extension()? == "app").then_some(app)
}

/// Whether this copy may be swapped for another where it stands (its folder takes new files).
fn can_replace(app: &std::path::Path) -> bool {
    let Some(dir) = app.parent() else { return false };
    let probe = dir.join(format!(".iris-update-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

fn update_log(line: &str) {
    use std::io::Write;
    let log = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("Library/Logs/Iris-update.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(f, "{line}");
    }
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    } else {
        Err(format!("{cmd} {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn plist_value(app: &std::path::Path, key: &str) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    run("/usr/libexec/PlistBuddy", &["-c", &format!("Print :{key}"), plist.to_str()?]).ok().map(|s| s.trim().to_string())
}

/// Downloads release `tag`'s zip from `url` into `dir` and makes sure it's what it says: one
/// app, this Iris (the same bundle ID), the version the release names, signed whole and by the
/// same signer as `current` (its designated requirement: the same certificate). The new app's
/// path, or what went wrong in words for the person. Blocks: run it off the main thread.
pub fn fetch_release(url: &str, tag: &str, dir: &std::path::Path, current: &std::path::Path) -> Result<std::path::PathBuf, String> {
    const BROKEN: &str = "The download wasn't a whole Iris, so nothing changed.";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|_| "Iris couldn't make room for the download.".to_string())?;
    let zip = dir.join("Iris.zip");
    let zip_s = zip.to_str().unwrap_or_default();
    run("/usr/bin/curl", &["--location", "--fail", "--silent", "--show-error", "--max-time", "900", "--output", zip_s, url])
        .map_err(|e| { update_log(&e); OFFLINE.to_string() })?;
    let unpacked = dir.join("unpacked");
    run("/usr/bin/ditto", &["-x", "-k", zip_s, unpacked.to_str().unwrap_or_default()]).map_err(|e| { update_log(&e); BROKEN.to_string() })?;
    let apps: Vec<_> = std::fs::read_dir(&unpacked).map_err(|_| BROKEN.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "app"))
        .collect();
    let [new] = apps.as_slice() else { return Err(BROKEN.into()) };
    let same_app = plist_value(new, "CFBundleIdentifier") == plist_value(current, "CFBundleIdentifier");
    let version = plist_value(new, "CFBundleShortVersionString").unwrap_or_default();
    if !same_app || version != tag.trim_start_matches('v') {
        update_log(&format!("release {tag}: the download is {version}, not this Iris at that version"));
        return Err(BROKEN.into());
    }
    let new_s = new.to_str().unwrap_or_default();
    run("/usr/bin/codesign", &["--verify", "--deep", "--strict", new_s]).map_err(|e| { update_log(&e); BROKEN.to_string() })?;
    // Signed by the same hand as this copy: its designated requirement holds for the new one.
    const SIGNER: &str = "The download isn't signed like this Iris, so nothing changed.";
    let shown = run("/usr/bin/codesign", &["--display", "--requirements", "-", current.to_str().unwrap_or_default()]).map_err(|_| SIGNER.to_string())?;
    let requirement = shown.lines().find_map(|l| l.strip_prefix("designated => ")).ok_or_else(|| SIGNER.to_string())?;
    // An ad hoc signature's requirement is its own code: no other build can meet it.
    if requirement.starts_with("cdhash") {
        return Err(SIGNER.into());
    }
    run("/usr/bin/codesign", &["--verify", &format!("-R={requirement}"), new_s]).map_err(|e| { update_log(&e); SIGNER.to_string() })?;
    let _ = run("/usr/bin/xattr", &["-dr", "com.apple.quarantine", new_s]);
    Ok(new.clone())
}

/// The script that swaps `new` in for `old` once Iris (`pid`) has quit, and opens it: the old
/// copy waits beside it until the new one is in place, and comes back if the move fails.
pub fn swap_script() -> &'static str {
    r#"pid=$1; old=$2; new=$3; previous=$4
for i in {1..200}; do kill -0 $pid 2>/dev/null || break; sleep 0.1; done
echo "── $(date '+%Y-%m-%d %H:%M:%S') installing ${new:t} in place of $old"
rm -rf "$previous"
if mv "$old" "$previous"; then
  if mv "$new" "$old"; then
    rm -rf "$previous"
    echo "Installed. Opening it."
  else
    echo "Update failed: couldn't move the new Iris in. The old one is back."
    mv "$previous" "$old"
  fi
else
  echo "Update failed: couldn't move the old Iris aside. Nothing changed."
fi
open "$old""#
}

/// Update & reopen for a downloaded copy: downloads release `tag` from `url`, checks it
/// (`fetch_release`), then quits while `swap_script` puts it in this copy's place and opens it.
/// Under test, only from a stand-in for GitHub on this Mac (`IRIS_RELEASES_API`): the test
/// made both copies.
pub fn install_release(tag: String, url: String, cx: &mut App) {
    if under_test() && !(local_releases().is_some() && url.starts_with("http://127.0.0.1:")) {
        return crate::live::news("updates: a release would have been installed");
    }
    let Some(current) = app_path() else { return };
    let st = state(cx);
    st.update(cx, |s, cx| {
        s.phase = Phase::Updating(Instant::now());
        cx.notify();
    });
    let dir = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default()
        .join("Library/Caches").join(info("CFBundleIdentifier").unwrap_or_else(|| "local.iris.app".into())).join("update");
    update_log(&format!("── downloading Iris {tag} from {url}"));
    cx.spawn(async move |cx| {
        let (d, c, t) = (dir.clone(), current.clone(), tag.clone());
        let fetched = cx.background_executor().spawn(async move { fetch_release(&url, &t, &d, &c) }).await;
        let _ = cx.update(|cx| match fetched {
            Ok(new) => {
                let log = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("Library/Logs/Iris-update.log");
                let started = Command::new("/bin/zsh").arg("-c")
                    .arg(format!("nohup /bin/zsh -c {} iris-update \"$@\" >> \"$0\" 2>&1 &", shell_quote(swap_script())))
                    .arg(&log).arg(std::process::id().to_string()).arg(&current).arg(&new).arg(dir.join("previous.app"))
                    .spawn();
                match started {
                    Ok(_) => cx.quit(),
                    Err(e) => answered(Status::InstallFailed("Iris couldn't start installing. Try again.".into()), &format!("swap: {e}"), cx),
                }
            }
            Err(words) => answered(Status::InstallFailed(words), "release: not installed", cx),
        });
    })
    .detach();
}

fn answered(status: Status, log: &str, cx: &mut App) {
    update_log(log);
    let st = state(cx);
    st.update(cx, |s, cx| {
        s.phase = Phase::Answered(status, Instant::now());
        cx.notify();
    });
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
