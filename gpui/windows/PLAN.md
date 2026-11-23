# Iris for Windows (beta): plan, 2026-11-20

## Measured on this Mac (M5 Pro, 48 GB), raw files in /tmp/iris-measure

Setup: the real brain (this worktree, built with gpui/brain/build.sh) with only its TextGenerator swapped for
one that streams from llama.cpp's llama-server (b12110). Same Rewriter, prompts, Protector, FixGuard.
15 drafts (the model card's 5 + 10 of mine: typos, @mention, email, link, `code`, #channel, a long one) x 6 modes.

- GGUF files: both downloaded, sha256 matches HF. Both load in llama.cpp (0.85 s from a warm file cache).
- Prompt: llama.cpp's chat template gives the same 87 token IDs as the MLX model's (system, few-shot,
  Hindi, link, code; thinking off). The only difference was where BOS is added.
- Answers, MLX vs llama.cpp on Metal:
  - Best (e4b): 75/90 identical. Fix 14/15, Professional 13, Friendly 12, Shorter 14, Clearer 14, Funny 8.
  - Lighter (e2b): 79/90 identical. Fix 15/15.
  - No Fix verdict changed. Differences are word choices ("nobody"/"no one", a contraction, "?" at the end)
    and different jokes. Same failure on both: Funny dropped a link once in e2b, the guard kept the text.
- Speed, whole check (first streamed text / total, medians):
  - e4b: MLX 0.06 / 0.18 s, llama.cpp Metal 0.07 / 0.39 s (MLX has a drafter; llama.cpp ran without one).
  - e2b: MLX 0.04 / 0.13 s, llama.cpp Metal 0.03 / 0.20 s.
- CPU only (llama.cpp, 4 threads, Fix + Professional, 30 checks each), all finished within Iris's time limits:
  - e2b: median 0.6 s, slowest 2.0 s. e4b: median 1.4 s, slowest 4.7 s.
  - Raw CPU throughput (4 threads, GPU off, weights repacked as on x86): e2b 294 tok/s prompt, 46 tok/s writing;
    e4b 81 / 23. A Windows laptop will be slower; estimate 2 to 4 times for an ordinary U-series laptop.
  - Memory on CPU: private 1.5 GB (e2b) / 3.0 GB (e4b) on top of the mapped model file (2.6 / 4.3 GiB,
    reclaimable). Fits the site's 8 GB / 16 GB.
- The GPUI app already type-checks for x86_64-pc-windows-msvc with zero errors (needs llvm-rc for GPUI's
  manifest; a stub did for a check). The Mac-only parts are stubbed out on Windows, not implemented.

## Decision 1: where the rules run on Windows

The brain is the model plus ~1,540 lines of IrisCore rules (Rewriter, Prompts, Protector, FixGuard,
FixNotes, WordDiff, ToneList, ProblemNote, SpellingHint...) under ~2,000 lines of tests. Engine plugs in
through one protocol (TextGenerator), so the seam is clean.

Recommended: the same Swift brain on Windows, with a new engine (IrisLlama: llama.cpp's C API in-process,
prefix reuse, idle unload, GGUF download) and Windows spelling/language checks. One copy of the rules;
IrisCoreTests run on Windows too.
Fallback if Swift on Windows fails the gate: a Rust brain (llama-cpp-2 crate) with the rules ported and
pinned by shared test vectors generated from the Swift tests.

## Platform parts (all behind cfg(windows); the Mac paths don't change)

- Shortcuts: RegisterHotKey. Ctrl+Alt+G collides with AltGr on some layouts (decision).
- Read/paste: UI Automation (TextPattern/ValuePattern selection, else whole box) + Ctrl+C fallback; write by
  clipboard save -> SetForegroundWindow(target) -> SendInput Ctrl+V -> restore clipboard. Same shape as
  paste.rs's activate-then-paste. Can't read/paste into apps run as administrator (UIPI): one message.
- Panel focus: GPUI's PopUp is TOPMOST+TOOLWINDOW but answers WM_MOUSEACTIVATE with MA_ACTIVATE, so the
  panel takes focus. Keys need focus on Windows anyway: let the panel take it and hand focus back on Replace.
- Tray: Shell_NotifyIcon, light/dark icon, Shell_NotifyIconGetRect anchors the slip (taskbar on any edge).
- Login: HKCU\...\Run.
- Placement: MonitorFromPoint/GetMonitorInfo work area, per-monitor DPI (UIA rects are physical pixels).
- Settings file: %APPDATA%\Iris. Apps are identified by exe name/AUMID instead of bundle ID (34 call sites);
  icons via SHGetFileInfo; Apps page's chooser picks an .exe or a running app.
- Fonts: Menlo/.SystemUIFont fallbacks -> Cascadia Mono/Consolas, Segoe UI.
- mock.rs language check: a plain heuristic (test-only path).
- Spelling hints: Windows ISpellChecker (en-US, en-GB if installed). Language check: re-tune thresholds on
  the same drafts with whatever replaces NLLanguageRecognizer.
- Copy: ~50 strings say Mac / menu bar / System Settings / Accessibility, plus the brain's own wording.
  One words table per platform; Mac strings unchanged.
- Setup: Windows has no permission to grant, so no Allow step (decision).

## Installer and updates (like Zed)

Per-user Inno Setup installer (no admin prompt) in %LOCALAPPDATA%\Programs\Iris: iris.exe, iris-brain.exe,
Swift runtime DLLs, llama/ggml DLLs (CPU variants + Vulkan, picked at runtime), small update helper.
Updater checks GitHub releases, downloads Iris-Setup-<v>.exe, runs it with /verysilent /update=true; it stages
files, the helper swaps them after Iris quits and reopens it. Checks the installer's signature when signed.
Model files: GGUF from the two HF repos, into the HF cache layout.

## Build and test with no PC here

- Here: cargo check/clippy for windows-msvc on every change; cargo-xwin can link a real iris.exe.
- GitHub Actions windows runner (needs your OK): build iris.exe, the Swift brain, llama.cpp; swift test of
  IrisCore on Windows; run the brain on the runner's CPU over the same 90 checks (a real x64 speed number);
  launch iris.exe --mock-answers and drive it through UI Automation with screenshots.
- A Windows 11 ARM VM on this Mac (Parallels/UTM), if you have one: installer, tray, UIA in real apps.
  Not for speed or GPU.
- Only a real PC proves: GPU (Vulkan) and laptop speed, 8 GB under pressure, Slack/Chrome/Office/Outlook
  pastes, SmartScreen and antivirus on the installer, HiDPI multi-monitor placement, Windows 10 vs 11 tray.

## Risks

1. Antivirus/SmartScreen: a global hotkey + reading other apps + synthetic keys + clipboard looks like a
   keylogger to heuristics; unsigned makes it worse. Signing matters more here than on Mac.
2. Speed on CPU-only laptops: Best on a slow CPU could reach the time limits on long drafts. Measure on the
   runner and a real PC; maybe recommend Lighter unless there's a usable GPU, or scale the limits.
3. Swift on Windows: toolchain in CI, ~60-80 MB of runtime DLLs, Foundation gaps (URLSession streaming,
   disk free space APIs). Gate before building on it.
4. UIA coverage: Electron/Chromium need accessibility turned on; some apps expose no text pattern
   (fallback Ctrl+C/Ctrl+V works but can't tell a selection from the whole box).
5. Transparent always-on-top panel and DPI/placement in GPUI on Windows: unproven until a real screen.

## Phases

0. Gate (needs your OK for a workflow): Windows CI builds iris.exe, IrisCore + swift test, llama.cpp;
   brain-on-runner speed run. Decide Swift vs Rust brain.
1. Windows brain (IrisLlama engine, GGUF download, model choice by memory/GPU, spelling/language).
   Built and tested on this Mac first (llama.cpp Metal), compared with MLX like today.
2. Platform layer (hotkey, textbridge, tray+slip, login, placement, storage, app identity, words).
3. Installer + updater + signing.
4. Beta on real PCs; release on GitHub (your OK); site download.

## Decisions for you

1. Swift brain on Windows (recommended) or Rust port.
2. GitHub Actions Windows workflow (Phase 0) on the private repo: OK to add?
3. Code signing: Azure Trusted Signing / OV certificate, or unsigned beta with SmartScreen warnings.
4. GPU: CPU + Vulkan for the beta (recommended), CUDA later?
5. Windows default shortcuts (Ctrl+Alt+G vs something else).
6. Words: "this PC" / "this computer"; "tray" / "taskbar"; setup without the Allow step.
7. A Windows PC or VM for testing: do you have one, or beta testers?

## Added after review
- MLX is deterministic: a second e4b run was 90/90 identical, so mlx-e4b.jsonl / mlx-e2b.jsonl are a Mac baseline.
- Cold vs warm (llama.cpp): CPU e4b cold prompt (228 tok) 3.2-3.4 s, warm (31 new tok) 0.56 s; e2b 0.9 s / 0.19 s.
  Metal e4b cold 0.13 s. A cache per tone is a requirement for the Windows engine.
- llama-cpp-2 0.1.161 (Rust) loads both GGUFs on this Mac (gemma4 supported).
- More stubs: monitor.rs (global click/key watch -> low-level hooks), native_menu.rs (pop-up menus, app chooser),
  model_compare.rs (memory hard-coded 16 GB), visible_frame_at (hard-coded 1920x1080), app.rs control socket (unix only).
- GPUI Windows exposes UIA through accesskit_windows (read in source, not run).

## After discussion with Fable (advisor), 2026-11-20
Adopted:
- Beta helper = Swift brain (rules) + official prebuilt llama-server.exe as its child (the only measured setup;
  no llama.cpp build in CI; crash isolation; kill = memory back). Random API key on loopback. Brain picks the
  slot per prefix (id_slot) with its own eviction; pass test: interleaved run where cache_n never drops to 0.
  Brain in a Job Object (kill on close), opted out of power throttling. C API later only if caching needs it.
- Spelling hints + language check computed in the Rust app (Windows COM) and sent with `check` (optional
  fields; the Mac path unchanged). Thresholds re-tuned on Windows dictionaries.
- CPU by default; Vulkan only after a golden self-test (temperature-0 answer must match and be faster).
- Lighter by default on CPU; Best offered after an on-device speed test. --no-repack at 8 GB (~2.9 GB resident).
- No low-level hooks: panel takes focus, closes on WM_ACTIVATE deactivation. Never call GPUI's activate()
  from the hotkey path (it taps Alt via SendInput, window.rs:845, verified). Wait for modifiers to release
  before Ctrl+C/Ctrl+V. No Ctrl+C fallback in console windows.
- Shortcut proposal: Ctrl+Win+G / Ctrl+Win+Shift+G, fallback if taken, setup shows what registered.
- Updater for beta: verify (embedded public key) -> quit -> Inno Setup replaces and relaunches. Zed's helper not reused.
- Tray: setup asks to pin the icon (Windows hides new ones in the overflow).
- Start signing identity validation in week one.
Sequencing: (1) Mac-only measurements: long drafts 1.5k/3.75k/6k chars + long Hindi, interleaved modes,
prewarm, n_ctx overflow, local eval set + judge over llama backend, memory with N slots; (2) iris.exe
--mock-answers in a Windows VM: hotkey, focus, read/paste in Notepad, Chrome, Word, a terminal;
(3) CI job A: swift test of IrisCore on Windows; (4) CI job B: prebuilt llama-server + drive.py on the runner;
(5) a real 8 GB x64 laptop.
Open risk found: Best on CPU at 6,000 characters needs ~93 s against a 96 s limit even on the M5 Pro.

## Measured 2026-11-20, after "let's do it"
Quality (64-draft eval set, score.py, both models, MLX vs llama.cpp Metal, same brain):
- Fix: 64/64 identical answers, both models. Tones scored the same (within 1-2 points, the run-to-run noise).
CPU only on the Mac (4 threads, llama-server b12110, tones with their own slots):
- --kv-unified breaks reuse: another slot's work evicts a tone's sliding-window cache, every tone re-reads
  its whole prompt. Without it 215 of 248 prompt tokens are reused. Use per-slot caches, 6,144 tokens a slot.
- llama-server keeps up to 8 GB of old prompts in RAM by default (--cache-ram 8192): cap it (512 MB costs no speed).
- Slots save to disk and restore in ms (/slots?action=save|restore, 11 MB for a Lighter tone prompt): a tone
  is cold once per install, not once per launch.
- Lighter (no repack, cache 512 MB): worst check 34% of its limit; 6k-char draft 33 s of 96 s; peak RSS 4.0 GB
  (2.6 of it the mapped model), private 1.2 GB.
- Best (repack): 3.7k-char Fix 91 s of 95 s; 1.5k Professional 46 of 54 s. Best on CPU is not safe for long drafts.
GitHub's Windows runner (private repo: 1 core / 2 threads AMD EPYC, noisy): Lighter repacked median 3-5 s,
2-12 of 60 over the limit (long drafts); not repacked median 5-9 s; Best median 14 s, 32 over. A floor, not a laptop.
Memory there: Lighter 2.0 GB working set (no repack) / 2.9 GB (repack); Best 5.5 GB.
Swift on Windows: IrisCore builds with Swift 6.3.3 and all 192 of its tests pass (CI job core-tests).
Windows 11 ARM VM on this Mac (QEMU, headless, ~/.cache/iris-winvm, `vm` helper): iris.exe cross-built here
with cargo-xwin (aarch64-pc-windows-msvc, static CRT) runs; setup, the panel, the slip and Settings render with
transparency, fonts and the riso print. Found:
- release builds need GPUI's shaders compiled by fxc.exe (Windows only): build releases on Windows CI.
- needs the C runtime linked in (+crt-static) or VCRUNTIME140.dll is missing on a fresh PC.
- console subsystem: a terminal opens beside Iris (#![windows_subsystem = "windows"]).
- Windows 11 draws a thin border around the transparent panel window (turn off with DWMWA_BORDER_COLOR).
- the doodle I of the wordmark draws hollow with ClearType fringes (text through DirectWrite); draw it as a
  path like the rest of the wordmark. Text is grayscale-vs-ClearType: check subpixel fringes elsewhere.
- Mac key symbols (⌘ ⌥ ⌃) and "this Mac" copy; memory reads 16 GB (stub); model sizes are MLX's, not GGUF's.
- cx.open_window errors are dropped silently (let _ =): log them.

## Built and tried, 2026-11-20 (windows-beta branch)
- Tray (tray.rs): the flower in the notification area, tooltip = the model's state, left click opens
  Settings (Iris's window on Windows), right click = the system's menu (Open Iris, Quit Iris). No Mac slip.
  Closing windows doesn't quit (GPUI quits with the last window on Windows by default). No console window.
- Keys (keys.rs): one place decides how keys read. Commands inside Iris take the system's command key
  (⌘R → Ctrl+R; ↩ ⇥ ⌫ esc → Enter Tab Backspace Esc). The shortcuts that work everywhere have per-system
  defaults: Win+Alt+G (⌃⌥G turned over) is the Xbox Game Bar's, Win+G / Win+V / Ctrl+Win+V are taken too
  (RegisterHotKey on Windows 11); Windows starts on Ctrl+Win+G / Ctrl+Win+Shift+G (free). Registered with
  RegisterHotKey; Settings checks Windows' own reserved keys. IRIS_PLATFORM=windows draws Windows' keys on a Mac.
- Helper: IrisLlama (Sources/IrisLlama) behind a small engine interface in the brain; the Mac stays MLX
  (64/64 identical answers). Windows brain built in CI (63 MB with Swift's runtime) and run in the VM:
  ready in 3.2 s, checks 1.1-3.0 s on Lighter, server stopped with the brain, tones' prompts saved.
Found on the way:
- Foundation on Windows has no URLSession.bytes (stream through a data delegate); Swift's networking had
  Hugging Face's CDN reset the connection in the VM, curl.exe didn't: downloads go through curl.exe (resumes).
- llama.cpp's prebuilt server needs the Visual C++ runtime (VCRUNTIME140.dll): absent on a fresh PC,
  present on GitHub's runners. The installer ships it beside the server (app-local).
- Windows has no language check yet: a Hinglish draft is fixed as English instead of translated.
- Settings lives under "<working dir>\Library\Application Support" on Windows (store.rs reads HOME).
- Default "off" apps are Mac bundle IDs; Windows needs exe names.
- Iris's windows expose no names to UI Automation (screen readers see nothing in Settings).

## Installer, updates and the first run, 2026-11-21
Since the list above: the language check runs in the app (ISpellChecker, a Hinglish word list), settings
live in %APPDATA%\Iris, the "off" apps are exe names, and the windows have names for screen readers.

Tried on the VM with the CI installer (x64, under Windows' emulation) the way a person would:
- Install (per user, no admin prompt), Start menu, Finish with "Open Iris": setup found the model already
  on disk. Uninstall closes Iris and removes the program, Start menu and open-at-login entries; it keeps
  settings and the model (2.8 / 4.6 GB): whether to offer removing the model is the user's call.
- Found and fixed: opened from the installer's Finish, the first fix sat on "Loading the model…". The new
  server started slowly (its files scanned, its code translated) and the brain's first health check to a
  port nobody listened on yet waited out its 600 s timeout. Each check now has a 2 s deadline of its own
  (f6c26a3); proven with a stand-in server 6 s and 20 s late (ready in 7.6 s / 21.5 s, was 600.3 s), then
  uninstall, reinstall, Finish, first press. A model server that went away is started again (seen).
- Found and fixed: a grey box round the panel and messages. Windows 11 drew its own frame (a veil, a top
  edge, a shadow) round the whole see-through window; non-client rendering is off for them (bbba674).
  Seen on this VM only (no graphics card, an Insider build); check on a real PC.
- Found and fixed: Update & reopen quit Iris and installed nothing. PowerShell 5 read the launcher (no
  byte order mark) as ANSI and a "──" broke its parse; it was also started detached from any console with
  no standard handles. Fixed together (6cd8d46: BOM + ASCII, a hidden console, null handles, breakaway
  from Iris's job): which of the last three mattered isn't isolated. With Iris started from Explorer and a
  stand-in for GitHub (scripts/verify/windows/fake-releases.ps1): update, quit, silent install, reopen in
  ~11 s. Refused, Iris left running, nothing installed: a manifest changed after signing, no signature, an
  older signed version (before downloading the installer), an installer that doesn't match its hash.
- The update key built into CI's installer was the TEST key until 004cfa7 (the release key since).

Decided: `host_level` (main's 4d3e691) stays a no-op on Windows for the beta. GPUI makes every pop-up
WS_EX_TOPMOST, and the panel opened above an always-on-top Notepad and took Tab there.

Timing (press → panel unrolls, VM, canned 350 ms answers): quick arrival 632–687 ms, full 1,066–1,098 ms,
with the canned engine reporting "ready". As main's mock has it (engine Idle) every check starts in
"Preparing", which unrolls the panel at once (294–357 ms): those numbers don't wait for the answer (likely
the Mac's too; not checked there). About 200 ms of each Windows figure is reading the box and making the window in the VM.

## Settings, the Apps page and a full pass, 2026-11-21
- Found and fixed: every dropdown in Settings did nothing on Windows (the native menu was the Mac's
  only). Windows' popup menu now, under the control, the choice ticked (5d3c816).
- The Apps page lists the Start menu's programs and the apps open now (Store apps have no shortcut
  file), known by program file ("notepad.exe"); "Choose another app…" is Windows' Open dialog.
- A settings file that starts with a byte order mark is read (Notepad and PowerShell write one).
- Found and fixed: "find me up here" opened half off the screen's top left (no menu bar to hang from);
  on Windows that news is a message on the taskbar by the clock (3098dfb).
- Edge: a password box says "password field" (also focused on load); a chat-style box (contenteditable,
  role=textbox, as Slack, Teams and Gmail have) reads, fixes and replaces with the clipboard put back.
  Twice, with Edge's "Restore pages" bubble up, the same box said "nothing to read"; not reproduced since.
- Full pass on the CI installer (5d3c816): clean install, setup's practice and Done, Starts in changed
  with the dropdown, Update & reopen from Explorer through the stand-in (reopened in ~18 s, setup didn't
  come back, the setting kept), the unsigned message whole, Quick Fix and its message over an
  always-on-top Notepad, Ctrl+Z.
- Not tried: Slack, Teams, Word, Outlook themselves (they need accounts); a real x64 PC.
- Found and fixed (Mac too): Start in a tone saved a tone per app, but the panel always opened on
  General's (15f85fc, with a changelog line in 0.4.1). "Shortcut taken" wrote the keys as "CtrlWinI";
  now "Ctrl+Win+I" (ee3b15a). Both news messages by the clock seen, all on the screen.
- CI installer of ee3b15a installed over the 5d3c816 copy: settings, setup's done and the per-app tone
  kept; Edge opens on Shorter (its own), Notepad on Professional (General's).
- toast::tests::sizes_match_swift fails on main too (turned-off-app 474.76 against Swift's 476): not
  from this branch.
- Uninstall (decided 2026-11-21: remove the model, but ask): the uninstaller asks "Delete the model
  Iris downloaded too? It takes 2.6 GB." with Yes the default; a silent uninstall deletes it. Tried
  on the CI installer of 9f06294: Enter (Yes) removed %LOCALAPPDATA%\Iris whole, the program too,
  and kept %APPDATA%\Iris (settings); No kept the model. The size is Windows' GB (GiB), as Explorer shows.

## Main's speed-ups and remembered answers, merged 2026-11-21
- bcc7235 (faster answers): the shortcut press goes straight to GPUI on the Mac; Windows' window
  procedure does the same (and "show Settings" from a second copy). Main now warms every tone
  once the model is ready; Windows still warms only the tone the panel opens on (llama-server
  has no gate putting a real check first, and four slots). core-tests (IrisCore on Windows) green.
- b205a2a (remembered answers, gliding tabs): works on Windows as on the Mac. Reduce Motion
  reads Windows' Animation effects switch (it was always off); with it off the marker jumps.
- Tried on the CI installer of ce103f8 with the real Lighter model: shortcut in Notepad, Tab
  (marker glides, Friendly writes), Shift+Tab back shows the remembered answer at once, Enter
  replaces, Ctrl+Z, the clipboard put back; "Remember answers" saves Off/Recent; a second copy
  opens Settings. A cold start answered within 2 s (tones restored from disk).
- Once (the first press after a fresh model download, on b2becc3's installer) the panel didn't
  take the keyboard and Tab went to Notepad. Not seen again in five tries of the same build and
  the pre-merge one, including straight after setup and with the model still loading.
- Uninstall now gives the model's size as setup does (2.8 GB, not Explorer's 2.6).

## The release update key, the Settings icons, and main's clipboard guards, 2026-11-21
- Update key: the release key and a backup (Ed25519) were made on the maintainer's Mac; their
  private halves are in ~/.config/iris/update-keys (never in the repo or CI), the public halves
  are CI's IRIS_UPDATE_KEYS (004cfa7). Sign a release: `uv run --with cryptography python
  gpui/windows/sign-release.py <Iris-Setup-x64.exe> <version> ~/.config/iris/update-keys/iris-update-primary.pem`.
- Tried on the VM, CI installer of 004cfa7 (0.4.1) and the stand-in for GitHub: 0.4.2 signed with
  the old test key refused ("isn't signed as Iris", Iris left open, the installer never downloaded);
  0.4.2 signed with the release key installed and reopened (Updates said 0.4.2); 0.4.3 signed
  with the backup key installed too. The 0.4.2/0.4.3 installers were made in the VM (Inno Setup 6.7.4
  installed there per user) from the installed bundle with an iris.exe built for each version.
- Settings icons: at 100 % (and up to 150 %) Windows has fewer pixels than an icon's 44-pixel print;
  shrinking it pixel by pixel dropped rows of dots (the ⌘, the squiggle, the keyhole went).
  `cached_crisp` blends the print down on Windows instead (eda4955); compared live against the Mac's
  Settings: rail, Privacy, Model & memory, About. The model cards said "For Macs with…": PCs now.
- The About meadow's blur is live on Windows as on the Mac (checked live against live; `--snap`
  draws it without blur on purpose).
- 9fb75f0 merged (235fbe7): turned-off apps refused before Iris reads them (by program file name),
  the copy waits while a Replace holds the clipboard, checks the keyboard and watches late answers;
  Replace re-checks the window before Ctrl+V; the brain keeps stdout for its answers (_dup/_dup2).
  Tried with the ARM build: shortcut, Right/Left through the tones (Left back is instant), Enter
  replaced, clipboard put back; Notepad turned off: "turned off here", the clipboard untouched.
- CI installer of 235fbe7 (x64, real Lighter model, the brain with its stdout kept for answers):
  shortcut in Notepad, Professional fixed it, Right to Friendly, Left back at once, Enter replaced,
  clipboard put back; Settings from the Start menu opens in front, its icons the blended ones.
- 4f8aa5c merged (6512ff1): main's own starting tone per app replaces the branch's (its changelog
  line covers it, so the branch's is gone); a check sent to a brain that's gone fails at once.
  CI installer on the VM: six tones shown, 6 jumps to Funny and writes, Right wraps to Fix,
  Enter replaced, clipboard put back. Settings from the Start menu (Windows key, "Iris", Enter)
  opens in front of Notepad; started from a scheduled task (the VM helper) it can open behind,
  as Windows gives such a start no right to the front: a test artefact, not a person's path.


## A release candidate, 2026-11-23
- 0542973 merged (the paint-on underline, the panel's height easing); CI of the merge built the
  installer (a rerun: the runner's Swift install met another install in progress, MSI 1618).
- That exact installer (Iris 0.4.1, sha256 05784233…1d34) signed with the release key; the
  signature and hash checked again on their own against the public key CI builds in.
- Tried on the VM: uninstalled, installed it through the wizard, setup (model kept, not downloaded
  again), the practice box: Fix with the real model, Tab to Professional, Enter replaced. The Fix
  tab's brush compared with the Mac's live capture of 0542973 (/verify run): the same stroke.
- Update: an ARM iris.exe saying 0.4.0 in place, the stand-in serving the signed release: Check
  found 0.4.1, Update & reopen installed it and reopened; the running iris.exe is the CI one and
  Updates says 0.4.1. Opened from the Start menu, Iris didn't see the stand-in's variable and asked
  GitHub itself: imaxisXD/iris is private, so the answer was "up to date".
- Both systems' updaters read the same releases/latest: a release only for Windows would tell
  every Mac "new version" with nothing to install. Prereleases aren't "latest", so neither sees one.
- Then 0.5.0 (the changelog's open issue, raised for its new Windows chapter): CI of 164fdfb by
  hand (the changelog isn't in the workflow's paths), signed (sha256 eedecdc9…d943). On the VM the
  installed 0.4.1 found it through the stand-in, Update & reopen installed it, Updates says 0.5.0
  with "Iris on Windows" first; in Notepad the shortcut fixed a draft and Enter replaced it.
