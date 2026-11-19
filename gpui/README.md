# Iris in GPUI (trial)

Iris's interface rebuilt in [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui),
Zed's GPU-rendered Rust UI framework (pinned at a rev of Zed's main in `Cargo.toml`), to compare
with the Rune copy (branch `rune-ui`) and the Swift app. The Swift app is untouched.

Shared with the Rune copy (ported from Swift seed for seed, pure Rust):

- `src/riso.rs`: the inks and the riso printing.
- `src/pen.rs`: the pen and brush shapes.
- `src/model.rs`: the word diff and the panel's rules.
- `src/rig.rs`: the flower as a rig of parts.
- `src/icons.rs`: the riso icons.
- `src/letters.rs`: the hand lettering and wordmark.
- `src/menubar.rs`: the menu bar flower, a native `NSStatusItem`.

In GPUI:

- `src/parts.rs`: type, keycaps, buttons, tone row, tape and notes. Every size goes through
  `z()`, so a part can be laid out smaller (`with_zoom`), because GPUI can't scale a box.
- `src/raster.rs`: whatever is turned or leaning is printed whole as one picture on the CPU
  (`Sheet`), because GPUI can't rotate text, pictures or boxes.
- `src/panel.rs`: the panel.
- `src/opening.rs` and `src/live.rs`: the ⌃G moment. The flower is drawn on background threads
  a frame ahead.
- `src/slip.rs`, `src/garden.rs`, `src/page.rs` and `src/live_slip.rs`: the menu bar slip, its
  garden (printed whole each frame) and its Updates page.
- `src/toast.rs`: the messages by your text and the note under the menu bar flower.
- `src/settings.rs`, `src/settings_kit.rs`, `src/pages/` and `src/live_settings.rs`: Settings,
  its nine pages and the note demo (`pages/notes.rs`).
- `src/setup.rs`: first-time setup, its twelve states.
- `src/scenes.rs`: the headless scenes, named as the Rune copy names them.
- `fonts/`: fixed cuts of Martian Mono (GPUI can't set a variable font's axes), made by
  `tools/mono_cuts.py`.

Run live (on the test display; `IRIS_TRACE=1` prints each frame's timing):

- `iris --live [x y]`: the panel alone, top left at `x, y` in screen points. Click a tone.
- `iris --live-panel [x y]`: the ⌃G moment. The flower grows and opens, and the panel unrolls;
  click a tone; Replace stamps the box, and the window goes.
- `iris --live-slip [x y]`: the menu bar slip alone, its garden alive. Click the rows.
- `iris --live-settings [x y] [page]`: Settings; the rail turns the page, each page scrolls.
- `iris --live-setup <state> [x y]`: a first-time setup state (`1-allow` … `done`) in its window.
- `iris --menubar`: the flower in the menu bar; a click hangs the slip from it, and its
  Settings… opens Settings (`IRIS_OPEN_SETTINGS=1` opens it at once).
  - `IRIS_SLIP_ANCHOR=x,y,w,h` hangs the slip from that frame instead.
  - `IRIS_OPEN_PANEL=1` with `IRIS_TEST_DISPLAY=x,y` (physical pixels) calls the panel at once,
    as the Rune copy's menu bar mode does.

Where to click, for scripts and the benchmark (points in the window; at 2× in pixels):

- `--live`: the tones' centres at y 43.5 and x 55, 133.5, 236.5, 318.5, 398, the same as the
  Rune copy's `--live`.
- `--live-panel`: the panel's top left is at (174, 330 − 10 − the paper's height − 12). With the
  opening answer (paper 176 tall) the tones sit at y 175.5, x 229, 307.5, 410.5, 492.5, 572,
  and Replace at (718, 294). An answer of three lines grows the paper upward by 22 points and
  moves the tones up with it, so click tones by their accessibility names (each tone is a tab
  named for itself) once the answer changes.
- `--live-slip`: rows are 32 points tall from y 132 (Check text); Load model now is at y 215,
  Settings… 288, Check for Updates… 320; Back on the Updates page at (50, 35). Each row is a
  menu item named for itself.
- `--live-settings`: the rail's rows are 33 points apart, from General at (110, 149) (title band
  included) to About at (110, 413); each page scrolls.

None of these take the keyboard unless `IRIS_TAKE_KEYS=1` is set. With it, Tab, 1–5, Return,
Escape and the slip's ↑ ↓ work as in the Swift app. `IRIS_REDUCE_MOTION=1` holds the garden
still, as Reduce Motion does.

Draw headlessly (GPUI's offscreen Metal renderer, nothing on screen):

- `iris --snap <dir>`: every panel state, one PNG each.
- `iris --snap <file.png> --scene <name> --size WxH [--at t]`: one scene. The scenes are
  `opening-full`, `opening-quick`, `slips`, `live-slip`, `toasts`, `menubar-hint`, and the
  Settings and setup pages.
- `python3 tools/report.py [out] [surface…]`: the page setting every finished surface beside the
  Swift app's own pictures.

Build:

- `cargo build --release --no-default-features`: the app alone, without the snapshot tooling.
- `cargo check --target x86_64-pc-windows-msvc --no-default-features`: type-check for Windows.
  GPUI's build embeds a manifest with `llvm-rc`; a Mac without it needs a stand-in on `PATH`
  (one that writes an empty `.res`) for the check to run.
