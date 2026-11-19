#!/usr/bin/env python3
"""One page that sets every finished surface of the GPUI copy beside the Swift app.

  tools/report.py [out dir] [surface …]     (default /tmp/iris-gpui/report, every surface)

Draws the GPUI side fresh from this checkout, so the page is what the branch draws now. The
Swift side is the Swift app's own design pictures, made once from the repo root and kept outside
the repo (never committed: they are build output). The Rune copy's report uses the same ones:

  /tmp/iris-rune/swift/panel-states-*.png      Iris --panel-states
  /tmp/iris-rune/snap/*.png                    Iris --snapshot (the slip, Settings, note demos…)
  /tmp/iris-rune/toasts-swift/toast-*.png      Iris --toast-states
  /tmp/rig-swift/full/NNN.png                  Iris --rig-frames (the ⌃G moment, frame by frame)

Live captures (taken on the test display, `live/` in the out dir) are shown where they exist.
"""
import html, os, shutil, subprocess, sys
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REF = "/tmp/iris-rune"
OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/iris-gpui/report"
ONLY = set(sys.argv[2:])
IRIS = os.path.join(ROOT, "target", "debug", "iris")
os.makedirs(OUT, exist_ok=True)

subprocess.run(["cargo", "build", "-q"], cwd=ROOT, check=True)


def want(name):
    return not ONLY or name in ONLY


def snap(name, scene, w, h, at=None):
    """The GPUI scene drawn at 2×, into OUT/name."""
    path = os.path.join(OUT, name)
    cmd = [IRIS, "--snap", path, "--scene", scene, "--size", f"{w}x{h}"]
    if at is not None:
        cmd += ["--at", str(at)]
    subprocess.run(cmd, cwd=ROOT, capture_output=True, check=True)
    return path


def save(im, name):
    im.save(os.path.join(OUT, name))
    return name


def ref(path, name):
    shutil.copy(path, os.path.join(OUT, name))
    return name


sections = []  # (id, title, what, [(caption, swift file, gpui file)], stacked)

if want("panel"):
    # The answer panel: compare.py cuts each state out of the Swift sheets and the GPUI snaps alike.
    snaps = os.path.join(OUT, "panel-gpui")
    subprocess.run([IRIS, "--snap", snaps], cwd=ROOT, check=True, capture_output=True)
    subprocess.run([sys.executable, os.path.join(HERE, "compare.py"), os.path.join(REF, "swift"), snaps, os.path.join(OUT, "panel")],
                   check=True, capture_output=True)
    states = subprocess.run([IRIS, "--list-states"], capture_output=True, text=True).stdout.strip().splitlines()
    sections.append(("panel", "The panel by your text",
                     "What opens when you press the shortcut: the fix, the tones, the buttons, and every message the panel can show.",
                     [(c, f"panel/{n}-swift.png", f"panel/{n}-gpui.png") for n, c in (l.split("\t") for l in states)], False))

if want("opening"):
    # The ⌃G moment, frame by frame (the frames `Iris --rig-frames` draws, 25 a second from -0.12 s).
    frames = []
    for i, what in [(10, "the bud comes up"), (24, "it reads your text"), (33, "it opens, and the panel starts to unroll"),
                    (35, "the panel unrolling"), (38, "open"), (73, "Replace pressed"), (86, "the petal stamps the box")]:
        t = round(-0.12 + 0.04 * i, 2)
        g = Image.open(snap(f"open-{i}-full.png", "opening-full", 900, 460, t)).convert("RGB")
        s = Image.open(f"/tmp/rig-swift/full/{i:03d}.png").convert("RGB")
        box = (100, 160, 1800, 880)
        frames.append((f"{t:.2f} s: {what}", save(s.crop(box), f"open-{i}-swift.png"), save(g.crop(box), f"open-{i}-gpui.png")))
    sections.append(("opening", "The flower opening the panel",
                     "The moment after the shortcut: the iris grows beside the text box, opens, and the panel unrolls out of it. Then Replace: the panel goes and a petal stamps the box.",
                     frames, False))

if want("slip"):
    # The menu bar slip in every state the Swift snapshot draws.
    slips = ref(f"{REF}/snap/after-status-menu-light.png", "slips-swift.png")
    snap("slips-gpui.png", "slips", 1060, 958)
    sections.append(("slip", "The menu bar slip",
                     "What hangs from the flower in the menu bar: the model getting ready, downloading, ready, an update waiting.",
                     [("Every state", slips, "slips-gpui.png")], True))

if want("settings"):
    # Settings, page by page, drawn at the Swift picture's height.
    pages = []
    for page, label in [("general", "General"), ("shortcuts", "Shortcuts"), ("tones", "Tones"), ("words", "Words"), ("apps", "Apps"),
                        ("model", "Model & memory"), ("privacy", "Privacy"), ("updates", "Updates"), ("about", "About")]:
        swift = f"{REF}/snap/screen-settings-{page}-light.png"
        h = Image.open(swift).size[1] // 2
        snap(f"settings-{page}-gpui.png", f"settings-{page}", 940, h)
        pages.append((label, ref(swift, f"settings-{page}-swift.png"), f"settings-{page}-gpui.png"))
    sections.append(("settings", "Settings", "Every page of the Settings window, scrolled to the top and run long to show the whole page.", pages, False))

if want("setup"):
    # First-time setup, every state Swift's `--snapshot` draws, at the Swift picture's height.
    steps = []
    for st, label in [("1-allow", "Step 1: allow Iris to read your text"), ("1-allow-stuck", "Step 1, when the permission doesn't take"),
                      ("1-allow-returning", "Step 1, coming back to it"), ("2-model", "Step 2: pick a model"), ("2-downloading", "Step 2, downloading"),
                      ("2-loading", "Step 2, getting it ready"), ("2-no-room", "Step 2, not enough room on the Mac"), ("2-offline", "Step 2, no internet"),
                      ("2-returning", "Step 2, coming back to it"), ("3-try", "Step 3: try it once"), ("3-try-shortcut-taken", "Step 3, when another app has the keys"),
                      ("done", "All done")]:
        swift = f"{REF}/snap/setup-{st}-light.png"
        h = Image.open(swift).size[1] // 2
        try:
            snap(f"setup-{st}-gpui.png", f"setup-{st}", 720, h)
        except subprocess.CalledProcessError:
            continue  # not drawn in GPUI yet
        steps.append((label, ref(swift, f"setup-{st}-swift.png"), f"setup-{st}-gpui.png"))
    if steps:
        sections.append(("setup", "Setup", "The window that walks you through the first start: allow, get the model, try it once.", steps, False))

if want("messages"):
    # Messages by your text: GPUI draws them in one column, each block the size of its Swift picture.
    names = "replaced replaced-no-undo fixed-1 fixed-4 looks-good copied kept checking turned-off-app turned-off-unknown left-alone password nothing-to-read empty too-long no-model no-disk offline unknown".split()
    swift = [Image.open(f"{REF}/toasts-swift/toast-{n}-light.png").convert("RGB") for n in names]
    sheet = Image.open(snap("toasts-gpui-sheet.png", "toasts", max(s.size[0] for s in swift) // 2, sum(s.size[1] for s in swift) // 2)).convert("RGB")
    toasts, y = [], 0
    for n, s in zip(names, swift):
        toasts.append((n.replace("-", " "), save(s, f"toast-{n}-swift.png"), save(sheet.crop((0, y, s.size[0], y + s.size[1])), f"toast-{n}-gpui.png")))
        y += s.size[1]
    sections.append(("messages", "Messages by your text",
                     "The small notes Iris leaves by the text box when there's no panel to show: fixed, nothing to fix, turned off here, no internet…",
                     toasts, False))

if want("notes"):
    notes = []
    for scene, label in [("note-demos", "The note demo in Settings › General, each clip"), ("note-demo-keys", "Its keys, mid-press"),
                         ("menubar-hint", "The note under the menu bar flower")]:
        s = f"{REF}/snap/after-{scene}-light.png"
        w, h = Image.open(s).size
        try:
            snap(f"{scene}-gpui.png", scene, w // 2, h // 2)
        except subprocess.CalledProcessError:
            continue  # not drawn in GPUI yet
        notes.append((label, ref(s, f"{scene}-swift.png"), f"{scene}-gpui.png"))
    sections.append(("notes", "The notes", "The little film in Settings that shows what a note looks like, and the note that points at the menu bar.", notes, False))

page = ["<!doctype html><meta charset=utf-8><title>Iris: the Swift app beside the GPUI copy</title><style>",
        "body{font:15px -apple-system,sans-serif;background:#f0ebe6;margin:0;color:#2a1a16}",
        "header{padding:28px 32px 8px}h1{font-size:24px;margin:0 0 6px}header p{margin:4px 0;max-width:760px;line-height:1.45}",
        "nav{position:sticky;top:0;background:#f0ebe6ee;padding:10px 32px;border-bottom:1px solid #ded4cb;z-index:2}",
        "nav a{margin-right:18px;color:#a8326a;text-decoration:none;font-weight:600}",
        "section{padding:8px 32px 28px}h2{font-size:20px;margin:26px 0 4px}.what{color:#6b5f58;margin:0 0 14px;max-width:760px}",
        ".pair{margin:18px 0}.pair h3{font-size:13px;font-weight:600;margin:0 0 6px;color:#6b5f58}",
        ".two{display:grid;grid-template-columns:1fr 1fr;gap:12px}.stack{display:grid;gap:12px}",
        ".two div,.stack div{font-size:12px;color:#6b5f58}img{width:100%;display:block;border:1px solid #ded4cb;margin-top:3px;background:#fff}",
        "</style><header><h1>Iris: the Swift app beside the GPUI copy</h1>",
        "<p>Each picture pair shows the app you use today (left, or top) and the same screen rebuilt in GPUI (right, or below). "
        "Both are drawn by the apps themselves, from the code as it is now.</p>",
        "<p>The Swift app is unchanged; the GPUI copy lives on its own branch.</p></header><nav>"]
page += [f"<a href='#{sid}'>{html.escape(title)}</a>" for sid, title, *_ in sections]
page.append("</nav>")
for sid, title, what, pairs, stacked in sections:
    page.append(f"<section id={sid}><h2>{html.escape(title)}</h2><p class=what>{html.escape(what)}</p>")
    for caption, a, b in pairs:
        layout = "stack" if stacked else "two"
        page.append(f"<div class=pair><h3>{html.escape(caption)}</h3><div class={layout}>"
                    f"<div>Swift app<img loading=lazy src='{a}'></div><div>GPUI copy<img loading=lazy src='{b}'></div></div></div>")
    page.append("</section>")
open(os.path.join(OUT, "index.html"), "w").write("\n".join(page))
print(os.path.join(OUT, "index.html"))
