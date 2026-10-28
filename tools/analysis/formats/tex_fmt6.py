#!/usr/bin/env python3
"""32-bit TEX textures (fmt 6) from texture.pak: listing, size check, per-position byte statistics
(which byte is alpha) and assembly of menu screens from tiles in two channel orders for visual inspection.

Purpose (docs/02-data-formats.md, §3.1; PLAN.md §13): the channel order of format 6. Result of the check on GOG 2.28:
pixel bytes B, G, R, A (D3DFMT_A8R8G8B8, u32 LE), rows top to bottom, uncompressed. The decisive check is
visual: tiles GUI_OPTION1…12 (4×3 of 256×256 = the 1024×768 main menu) in BGRA order give natural
colors (sandstone, green foliage), in RGBA order they give light-blue stone and blue foliage.

Example
  uv run python -I tools/analysis/formats/tex_fmt6.py --game-dir ./OriginalGame --out /tmp/analysis/tex6

Expected output: "fmt empty: 1, fmt 4: 25307, fmt 6: 227"; every fmt 6 entry has dataSize == w·h·4; for both formats
the directory size == dataSize, i.e. it excludes the 80-byte entry header; byte 3 is alpha (77% of values are 255 and 11%
are zero, versus 4–7% for bytes 0–2).
Output: <out>/<screen>_{bgra,rgba}.png, derived from the game assets; do not commit. Dependencies: numpy, pillow.
"""
import sys, os, re, struct, mmap, collections, argparse
import numpy as np
from PIL import Image

DEFAULT_GAME_DIR = './OriginalGame'  # as in common/sacred_pe.py; the module is not imported to avoid pulling in capstone

ap = argparse.ArgumentParser(description='Inspect 32-bit (fmt 6) TEX entries and render menu mosaics in BGRA/RGBA order.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--pak', help='TEX pak (default <game-dir>/PAK/texture.pak)')
ap.add_argument('--out', required=True, help='output directory for mosaics')
args = ap.parse_args()
PAK = args.pak or os.path.join(args.game_dir, 'PAK', 'texture.pak')
os.makedirs(args.out, exist_ok=True)

f = open(PAK, 'rb'); d = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
if len(d) < 0x100:
    sys.exit('%s is too short for a PAK (%d bytes)' % (PAK, len(d)))
cnt = struct.unpack_from('<I', d, 4)[0]
fmts = collections.Counter(); dirsize = collections.Counter(); six = {}
for i in range(cnt):
    t, o, sz = struct.unpack_from('<III', d, 0x100 + 12 * i)
    if not o: fmts['empty'] += 1; continue
    name = d[o:o + 32].split(b'\0')[0].decode('latin1').upper()
    w, h, fmt = struct.unpack_from('<HHB', d, o + 32); ds = struct.unpack_from('<I', d, o + 37)[0]
    fmts[fmt] += 1
    dirsize[(fmt, 'dir size == dataSize' if sz == ds else 'dir size == 80 + dataSize' if sz == ds + 0x50 else 'other')] += 1
    if fmt == 6:
        assert ds == w * h * 4, (name, w, h, ds)
        six[name] = np.frombuffer(d[o + 0x50:o + 0x50 + ds], np.uint8).reshape(h, w, 4)
print(', '.join('fmt %s: %d' % kv for kv in sorted(fmts.items(), key=str)))
print('directory size field:', dict(dirsize))
print('fmt 6 sizes:', dict(collections.Counter('%dx%d' % (v.shape[1], v.shape[0]) for v in six.values())))

px = np.concatenate([v.reshape(-1, 4) for v in six.values()])
for b in range(4):
    c = px[:, b]
    print('byte %d: ==255 %5.1f%%  ==0 %5.1f%%  distinct %3d  mean %5.1f' % (b, 100 * (c == 255).mean(), 100 * (c == 0).mean(), len(np.unique(c)), c.mean()))

for prefix in ('GUI_OPTION', 'GUI_UW_OPTION', 'GUI_CHAR', 'GUI_UW_CHAR'):
    tiles = {int(m.group(1)): v for k, v in six.items() if (m := re.fullmatch(prefix + r'_?(\d+)\.TGA', k))}
    if sorted(tiles) != list(range(1, 13)):
        print(prefix, 'incomplete:', sorted(tiles)); continue
    mos = np.concatenate([np.concatenate([tiles[r * 4 + c + 1] for c in range(4)], 1) for r in range(3)], 0)
    for order, idx in (('bgra', [2, 1, 0]), ('rgba', [0, 1, 2])):
        Image.fromarray(np.ascontiguousarray(mos[..., idx])).save(os.path.join(args.out, '%s_%s.png' % (prefix, order)))
    print('%s: 12 tiles -> %s_{bgra,rgba}.png (1024x768)' % (prefix, prefix))
