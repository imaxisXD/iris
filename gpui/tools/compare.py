#!/usr/bin/env python3
"""Builds the Swift-vs-GPUI comparison page for the answer panel.

  compare.py <swift sheets dir> <gpui snaps dir> <out dir>

Swift: the `Iris --panel-states` sheets, cut into panels (crop_swift.py's rule).
GPUI: one `iris --snap sNN.png --scene sNN` per state, the panel 26 pt in from the left.
Each panel is cut the same way (the paper, plus the shadow's room: 26 pt each side, 12 above,
44 below), then laid out Swift | GPUI | difference, with the paper heights side by side.
"""
import html, os, subprocess, sys
from PIL import Image, ImageChops, ImageOps

SCALE = 2
sheets, snaps, out = sys.argv[1], sys.argv[2], sys.argv[3]
# An optional HTML file with the findings, set above the states.
intro = open(sys.argv[4]).read() if len(sys.argv) > 4 else ""
os.makedirs(out, exist_ok=True)


def paper_runs(im, x0):
    """Rows where the paper is: the median of the left margin's first 5 points, so the grain's
    specks don't move the edge; the heading rule's few dark rows don't break a run."""
    import statistics
    H = im.size[1]
    g = im.convert("L")
    xs = range(x0 + 2, x0 + 12)
    rows = [statistics.median(g.getpixel((x, y)) for x in xs) >= 245 for y in range(H)]
    runs, y = [], 0
    while y < H:
        if rows[y]:
            start = end = y
            while y < H and (rows[y] or any(rows[y:y + 8])):
                if rows[y]:
                    end = y + 1
                y += 1
            if end - start > 60 * SCALE:
                runs.append((start, end))
        y += 1
    return runs


def cut(im, x0, top, bottom):
    return im.crop((x0 - 26 * SCALE, top - 12 * SCALE, x0 + 646 * SCALE - 26 * SCALE + 26 * SCALE, bottom + 44 * SCALE))


swift = []
for sheet in sorted(f for f in os.listdir(sheets) if f.startswith("panel-states-")):
    im = Image.open(os.path.join(sheets, sheet)).convert("RGB")
    found = []
    for c in range(2):
        x0 = (20 + c * 700 + 26) * SCALE
        for top, bottom in paper_runs(im, x0):
            found.append((top, c, x0, bottom))
    found.sort(key=lambda f: (round(f[0] / (40 * SCALE)), f[1]))
    for top, c, x0, bottom in found:
        swift.append((cut(im, x0, top, bottom), (bottom - top) / SCALE))

captions = subprocess.run([os.path.join(os.path.dirname(__file__), "..", "target", "debug", "iris"), "--list-states"],
                          capture_output=True, text=True).stdout.strip().splitlines()
rows = []
for i, line in enumerate(captions):
    name, caption = line.split("\t")
    s_im, s_h = swift[i]
    r_full = Image.open(os.path.join(snaps, f"{name}.png")).convert("RGB")
    runs = paper_runs(r_full, 26 * SCALE)
    if not runs:
        print("no paper in", name); continue
    top, bottom = runs[0]
    r_im, r_h = cut(r_full, 26 * SCALE, top, bottom), (bottom - top) / SCALE
    h = max(s_im.size[1], r_im.size[1])
    pad = lambda im: ImageOps.pad(im, (im.size[0], h), color=(240, 235, 230), centering=(0, 0))
    a, b = pad(s_im), pad(r_im)
    diff = ImageOps.autocontrast(ImageChops.difference(a, b).convert("L"), cutoff=0.5)
    diff = ImageOps.invert(diff).convert("RGB")
    for img, kind in ((a, "swift"), (b, "gpui"), (diff, "diff")):
        img.save(os.path.join(out, f"{name}-{kind}.png"))
    rows.append((name, caption, s_h, r_h))
    print(f"{name}  paper swift {s_h:.1f} pt, gpui {r_h:.1f} pt  {caption}")

page = ["<!doctype html><meta charset=utf-8><title>Iris panel: Swift vs GPUI</title><style>",
        "body{font:14px -apple-system,sans-serif;background:#f0ebe6;margin:24px;color:#2a1a16}",
        "h1{font-size:20px}.row{margin:28px 0}.row h2{font-size:15px;margin:0 0 2px}.meta{color:#6b5f58;font-size:12px;margin-bottom:6px}",
        ".imgs{display:grid;grid-template-columns:1fr 1fr 1fr;gap:10px}.imgs div{font-size:11px;color:#6b5f58}",
        ".imgs img{width:100%;display:block;border:1px solid #e1d9d2}</style>",
        intro,
        "<h1>Every state: Swift app (left), GPUI (middle), where they differ (right, darker = more different)</h1>"]
for name, caption, s_h, r_h in rows:
    page.append(f"<div class=row><h2>{html.escape(caption)}</h2><div class=meta>{name} · paper height Swift {s_h:.0f} pt, GPUI {r_h:.0f} pt</div><div class=imgs>"
                f"<div>Swift<img src='{name}-swift.png'></div><div>GPUI<img src='{name}-gpui.png'></div><div>difference<img src='{name}-diff.png'></div></div></div>")
open(os.path.join(out, "index.html"), "w").write("\n".join(page))
print("wrote", os.path.join(out, "index.html"))
