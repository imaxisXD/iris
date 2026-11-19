#!/usr/bin/env python3
"""Cuts each panel out of the Swift app's `Iris --panel-states` sheets.

The sheets lay panels out two to a row, each cell 680 pt wide at x = 20 and 720, the paper
620 pt wide 26 pt in from the cell's left. A panel is the run of rows whose paper is lighter than
the table. Every panel is saved with the same room around the paper the Swift view has for its
shadow (26 pt either side, 12 above, 44 below), as states/NN.png, in sheet order.

usage: crop_swift.py <sheets dir> <out dir> [scale]
"""
import os, sys
from PIL import Image

src, out = sys.argv[1], sys.argv[2]
scale = int(sys.argv[3]) if len(sys.argv) > 3 else 2
os.makedirs(out, exist_ok=True)
n = 0
for sheet in sorted(f for f in os.listdir(src) if f.startswith("panel-states-")):
    im = Image.open(os.path.join(src, sheet)).convert("RGB")
    W, H = im.size
    found = []
    for col in range(2):
        x0 = (20 + col * 700 + 26) * scale
        x1 = x0 + 620 * scale
        strip = im.crop((x0 + 2, 0, x0 + 10, H)).convert("L").resize((1, H), Image.BOX)
        raw = [strip.getpixel((0, y)) for y in range(H)]
        # The paper's grain dips single rows; smooth over a few before reading it.
        rows = [sum(raw[max(0, y - 4):y + 5]) / len(raw[max(0, y - 4):y + 5]) for y in range(H)]
        y = 0
        while y < H:
            if rows[y] >= 243:
                start = y
                while y < H and rows[y] >= 242:
                    y += 1
                if y - start > 60 * scale:
                    found.append((start, col, x0, y))
            y += 1
    # Sheet order is row by row: left then right.
    found.sort(key=lambda f: (round(f[0] / (40 * scale)), f[1]))
    for top, col, x0, bottom in found:
        n += 1
        box = (x0 - 26 * scale, top - 12 * scale, x0 + (620 + 26) * scale, bottom + 44 * scale)
        im.crop(box).save(os.path.join(out, f"{n:02d}.png"))
        print(sheet, n, box, "paper", 620, "x", (bottom - top) / scale)
