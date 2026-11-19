#!/usr/bin/env python3
"""Cuts fixed instances of Martian Mono for GPUI, which can't set a variable font's axes.

Iris sets Martian Mono at width 87.5 and weights 430, 450 and 560 (`DS.Font.martian`); the
variable font's default is width 112.5, weight 400. Each cut is renamed "Iris Mono <weight>"
(the OFL keeps the original name for the original font).

usage: mono_cuts.py   (needs fonttools; writes gpui/fonts/IrisMono-<weight>.ttf)
"""
import os
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

here = os.path.dirname(os.path.abspath(__file__))
src = os.path.join(here, "../../Sources/Iris/Resources/Fonts/MartianMono-Variable.ttf")
for weight in (430, 450, 560):
    font = instancer.instantiateVariableFont(TTFont(src), {"wght": weight, "wdth": 87.5})
    family = f"Iris Mono {weight}"
    name = font["name"]
    for rec in list(name.names):
        if rec.nameID in (1, 16):
            rec.string = family
        elif rec.nameID in (2, 17):
            rec.string = "Regular"
        elif rec.nameID == 4:
            rec.string = family
        elif rec.nameID == 6:
            rec.string = family.replace(" ", "")
    font["OS/2"].usWeightClass = 400
    out = os.path.join(here, f"../fonts/IrisMono-{weight}.ttf")
    font.save(out)
    print(out, os.path.getsize(out))
