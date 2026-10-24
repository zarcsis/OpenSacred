#!/usr/bin/env python3
"""Reconstruction of the UV table of the 18 ISO-tile diamonds computed by the cWorldView constructor (0x624671..0x6247A1
in the GOG ENG 2.28 build): 9 rows × 2 columns, row pitch 25 texels, column pitch 104,
odd-row shift 52, EPS = 0.002, normalization 1/256. Prints the table and draws the 18 diamonds on a synthetic 256×256
sheet (and, if given, over a decoded ISO sheet PNG).

Purpose (docs/02 §5.4: 18 diamonds per 256² sheet).
Vertex order as in renderTileRow: v0 left, v1 top, v2 bottom, v3 right.

Example
  python3 -I tools/analysis/world/uv_table.py --out /tmp/analysis/geom [--sheet /tmp/analysis/iso_sheet.png]

Output: <out>/uv_table.txt, <out>/uv_layout_synthetic.png, and with --sheet <out>/uv_layout_on_sheet.png. Needs no game
input files. Dependencies: numpy, pillow.
"""
import sys, os, argparse
import numpy as np
from PIL import Image, ImageDraw

ap = argparse.ArgumentParser(description='Reconstruct the 18-cell ISO UV table of cWorldView and render it.')
ap.add_argument('--out', required=True, help='output directory')
ap.add_argument('--sheet', help='optional PNG of a decoded ISO sheet to overlay the rhombi on')
args = ap.parse_args()
os.makedirs(args.out, exist_ok=True)
OUT = os.path.join(args.out, '')
EPS = 0.002; INV = 1.0 / 256.0
table = []  # cell -> [(u,v) left, top, bottom, right]  (vertex order used by renderTileRow: v0 left, v1 top, v2 bottom, v3 right)
for i in range(9):          # outer loop: 9 rows (ecx 0..8)
    T = i * 25.0
    S = 52.0 if (i & 1) else 0.0
    for j in range(2):      # inner loop: 2 columns (edx 0..1)
        J = j * 104.0
        left = ((J + S + 2.0) * INV + EPS, (T + 23.5) * INV + EPS)
        top = ((J + S + 50.0) * INV + EPS, (T + 0.5) * INV + EPS)
        bottom = ((J + S + 50.0) * INV, (T + 48.0) * INV + EPS)
        right = ((J + S + 97.5) * INV + EPS, (T + 23.5) * INV)
        table.append((left, top, bottom, right))
with open(OUT + 'uv_table.txt', 'w', encoding='utf-8', newline='') as f:
    f.write('cell  left(u,v)            top(u,v)             bottom(u,v)          right(u,v)           | texel-space rhombus bbox x0..x1 y0..y1\n')
    for c, (l, t, b, r) in enumerate(table):
        f.write('%2d   (%.5f,%.5f)  (%.5f,%.5f)  (%.5f,%.5f)  (%.5f,%.5f) | x %.1f..%.1f  y %.1f..%.1f\n' % (
            c, l[0], l[1], t[0], t[1], b[0], b[1], r[0], r[1], l[0] * 256, r[0] * 256, t[1] * 256, b[1] * 256))
print(open(OUT + 'uv_table.txt').read())
def render(base, path):
    img = base.convert('RGBA'); dr = ImageDraw.Draw(img, 'RGBA')
    cols = [(255, 0, 0), (0, 255, 0), (0, 128, 255), (255, 255, 0), (255, 0, 255), (0, 255, 255)]
    for c, (l, t, b, r) in enumerate(table):
        pts = [(l[0] * 256, l[1] * 256), (t[0] * 256, t[1] * 256), (r[0] * 256, r[1] * 256), (b[0] * 256, b[1] * 256)]
        col = cols[c % 6]
        dr.polygon(pts, outline=col + (255,), fill=col + (40,))
        cx = sum(p[0] for p in pts) / 4; cy = sum(p[1] for p in pts) / 4
        dr.text((cx - 6, cy - 6), str(c), fill=(255, 255, 255, 255))
    img = img.resize((768, 768), Image.NEAREST); img.save(path)
base = Image.new('RGB', (256, 256), (30, 30, 30))
dr = ImageDraw.Draw(base)
for x in range(0, 256, 16): dr.line([(x, 0), (x, 255)], fill=(60, 60, 60))
for y in range(0, 256, 16): dr.line([(0, y), (255, y)], fill=(60, 60, 60))
render(base, OUT + 'uv_layout_synthetic.png')
if args.sheet:
    render(Image.open(args.sheet).convert('RGB').resize((256, 256)), OUT + 'uv_layout_on_sheet.png')
# geometry sanity: rhombus size in texels vs screen rhombus (96.4 x 48.4 px at zoom 1)
l, t, b, r = table[0]
print('cell 0 texel extents: width %.2f height %.2f ; row pitch 25 texels, column pitch 104, odd-row shift 52' % ((r[0] - l[0]) * 256, (b[1] - t[1]) * 256))
print('sheet usage: max x %.1f, max y %.1f of 256' % (max(r[0] for _, _, _, r in table) * 256, max(b[1] for _, _, b, _ in table) * 256))
