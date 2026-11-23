//! Windows: installing a release Iris found (`updater::install_release`), like the Mac's but with
//! no code signature to lean on (the beta is unsigned), so a release is trusted by a signature
//! of Iris's own:
//!
//! - A release carries Iris-Setup-x64.exe, Iris-Setup-x64.manifest and its .sig. The manifest is
//!   three lines (`LABEL`, the version, the installer's SHA-256), signed with Ed25519 by a key
//!   whose public half is built into Iris (`IRIS_UPDATE_KEYS` at build time, room for two, so a
//!   key can be changed). gpui/windows/sign-release.py makes them.
//! - The version comes from the signed manifest, never from GitHub's answer, and must be newer
//!   than this copy: an old signed installer can't be played back as a downgrade.
//! - Fails closed: no signature, a bad one, no key built in, a hash that differs: nothing is
//!   installed, and the person is told why.
//! - The installer runs from %LOCALAPPDATA%\Iris\update (not %TEMP%), the file that was hashed.
//! - A launcher waits for Iris (and its brain and model server) to be gone, runs the installer
//!   silently, and starts Iris again whatever happened: a failed install still leaves Iris open.

use std::path::{Path, PathBuf};
use std::process::Command;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

pub const SETUP: &str = "Iris-Setup-x64.exe";
pub const MANIFEST: &str = "Iris-Setup-x64.manifest";
pub const SIGNATURE: &str = "Iris-Setup-x64.manifest.sig";
const LABEL: &str = "iris windows release v1";

const BROKEN: &str = "The download wasn't a whole Iris, so nothing changed.";
const UNSIGNED: &str = "The download isn't signed as Iris, so nothing changed.";
const OFFLINE: &str = "Couldn't reach GitHub. Check your connection and try again.";

/// The public keys built into this copy (hex, comma-separated, at build time).
fn keys() -> Vec<VerifyingKey> {
    option_env!("IRIS_UPDATE_KEYS").unwrap_or("").split(',').filter_map(|h| {
        let bytes: Vec<u8> = (0..h.trim().len() / 2).filter_map(|i| u8::from_str_radix(h.trim().get(2 * i..2 * i + 2)?, 16).ok()).collect();
        VerifyingKey::from_bytes(&bytes.try_into().ok()?).ok()
    }).collect()
}

/// Whether this copy can install a release at all (a key was built in).
pub fn can_verify() -> bool {
    !keys().is_empty()
}

/// The manifest's version and SHA-256, if it's one, signed by a built-in key.
pub fn verified(manifest: &[u8], signature: &[u8]) -> Option<(String, String)> {
    let sig = Signature::from_slice(signature).ok()?;
    if !keys().iter().any(|k| k.verify(manifest, &sig).is_ok()) {
        return None;
    }
    let text = std::str::from_utf8(manifest).ok()?;
    let mut lines = text.lines();
    if lines.next()? != LABEL {
        return None;
    }
    let version = lines.next()?.strip_prefix("version ")?.trim().to_string();
    let sha = lines.next()?.strip_prefix("sha256 ")?.trim().to_lowercase();
    (sha.len() == 64).then_some((version, sha))
}

fn sha256_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let out = Command::new("curl.exe")
        .args(["--location", "--fail", "--silent", "--show-error", "--retry", "3", "--max-time", "900", "--output"])
        .arg(to).arg(url).output().map_err(|_| OFFLINE.to_string())?;
    if out.status.success() { Ok(()) } else { Err(OFFLINE.into()) }
}

/// Where updates are downloaded to: %LOCALAPPDATA%\Iris\update.
pub fn folder() -> PathBuf {
    std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("Iris").join("update")
}

/// Downloads the release whose installer is at `setup_url` (its manifest and signature beside
/// it) and checks it. The installer's path and its version, or what went wrong in words.
pub fn fetch(setup_url: &str, installed: &str) -> Result<(PathBuf, String), String> {
    let base = setup_url.strip_suffix(SETUP).ok_or_else(|| BROKEN.to_string())?;
    let dir = folder();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|_| "Iris couldn't make room for the download.".to_string())?;
    let (setup, manifest, sig) = (dir.join(SETUP), dir.join(MANIFEST), dir.join(SIGNATURE));
    download(&format!("{base}{MANIFEST}"), &manifest).map_err(|_| UNSIGNED.to_string())?;
    download(&format!("{base}{SIGNATURE}"), &sig).map_err(|_| UNSIGNED.to_string())?;
    let (m, s) = (std::fs::read(&manifest).unwrap_or_default(), std::fs::read(&sig).unwrap_or_default());
    let Some((version, sha)) = verified(&m, &s) else { return Err(UNSIGNED.into()) };
    if !crate::updater::is_newer(&version, installed) {
        return Err(format!("The download is Iris {version}, not newer than this one, so nothing changed."));
    }
    download(setup_url, &setup)?;
    if sha256_of(&setup).as_deref() != Some(sha.as_str()) {
        let _ = std::fs::remove_file(&setup);
        return Err(BROKEN.into());
    }
    Ok((setup, version))
}

/// Starts the launcher that installs `setup` once Iris (`pid`) is gone and opens Iris again.
pub fn launch(setup: &Path, pid: u32) -> Result<(), String> {
    let dir = setup.parent().unwrap_or(Path::new("."));
    let app = std::env::current_exe().map_err(|e| e.to_string())?;
    let log = dir.join("update.log");
    let script = dir.join("install.ps1");
    // Waits for Iris, then for the brain and model server from this folder (the job that holds
    // them ends with Iris), installs, and opens Iris again whatever the installer said.
    // Windows PowerShell reads a script with no byte order mark as the PC's ANSI code page: the
    // mark keeps the paths in it whole (a user name like José), and the script's own words stay
    // ASCII (a "──" read that way had a closing quote in it, and the script never ran).
    let body = format!(r#"$ErrorActionPreference = "Continue"
"-- $(Get-Date -Format s) installing {setup}" | Add-Content "{log}"
for ($i = 0; $i -lt 300 -and (Get-Process -Id {pid} -ErrorAction SilentlyContinue); $i++) {{ Start-Sleep -Milliseconds 100 }}
for ($i = 0; $i -lt 100 -and (Get-Process iris-brain, llama-server -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -like "{folder}\*" }}); $i++) {{ Start-Sleep -Milliseconds 100 }}
$p = Start-Process -FilePath "{setup}" -ArgumentList "/VERYSILENT","/SUPPRESSMSGBOXES","/NORESTART","/LOG=`"{dir}\setup.log`"" -Wait -PassThru
"installer exit $($p.ExitCode)" | Add-Content "{log}"
Start-Process -FilePath "{app}"
"#, setup = setup.display(), log = log.display(), pid = pid, folder = app.parent().unwrap_or(Path::new(".")).display(),
        dir = dir.display(), app = app.display());
    std::fs::write(&script, format!("\u{feff}{body}")).map_err(|e| e.to_string())?;
    // A console of its own, hidden (PowerShell won't run detached from every console), no
    // standard handles (Iris, a windowed app, has none to pass on), and out of any job Iris was
    // started in, so it outlives Iris; where the job doesn't allow that, inside it.
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let start = |flags: u32| {
        Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .creation_flags(flags)
            .spawn()
    };
    start(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| start(CREATE_NO_WINDOW))
        .map_err(|e| e.to_string())?;
    Ok(())
}
