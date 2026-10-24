#!/usr/bin/env python3
"""Decoding of 16-bit TEX textures (fmt 4) from a "TEX v3" PAK as ARGB4444 / RGB565 / ARGB1555 with plausibility
metrics (gradient smoothness, channel correlation, alpha distribution) and PNG output; additionally,
partial decoding of a single TEX entry from a truncated copy (--entry), e.g. the head of a texture.pak record.

Purpose (docs/02 §3.1): the 16-bit format is
ARGB4444 (fmt 4), 256×256, without mipmaps/DXT/color key. Layout: directory 0x100 + count×12 (fmt, offset, size);
an entry is an 80-byte header (name[32], u16 w, u16 h, u8 fmt, u32 csize @37) + a zlib stream of w×h×2 bytes.

Examples
  python3 -I tools/analysis/formats/tex_decode.py --game-dir ./OriginalGame --out /tmp/analysis/tex          # texture03.pak (27 KB, 2 entries)
  python3 -I tools/analysis/formats/tex_decode.py --pak OriginalGame/PAK/texture.pak --out /tmp/analysis/tex  # 860 MB
  python3 -I tools/analysis/formats/tex_decode.py --entry /tmp/analysis/tex_entry.bin --out /tmp/analysis/tex # truncated entry

Output: <out>/tex_decode.txt, <out>/tex_<name>_{4444,565,1555}.png and _compare.png. The PNGs are derived from the
game assets; do not commit. Dependencies: numpy, pillow.
"""
import struct, zlib, sys, os, argparse
import numpy as np
from PIL import Image

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Decode 16-bit TEX payloads as ARGB4444/RGB565/ARGB1555 and compare plausibility.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--pak', help='TEX pak (default <game-dir>/PAK/texture03.pak)')
ap.add_argument('--entry', help='optional raw TEX entry (80-byte header + possibly truncated zlib stream)')
ap.add_argument('--max-entries', type=int, default=16, help='decode at most this many pak entries')
ap.add_argument('--out', required=True, help='output directory')
args = ap.parse_args()
PAK = args.pak or os.path.join(args.game_dir, 'PAK', 'texture03.pak')
os.makedirs(args.out, exist_ok=True)
OUT = os.path.join(args.out, '')
log = open(OUT + 'tex_decode.txt', 'w', encoding='utf-8', newline='')
def w(s):
    log.write(s + '\n'); print(s)
def dec4444(p):  # D3DFMT_A4R4G4B4 little-endian u16: A15-12 R11-8 G7-4 B3-0
    a = (p >> 12) & 15; r = (p >> 8) & 15; g = (p >> 4) & 15; b = p & 15
    return np.stack([r * 17, g * 17, b * 17, a * 17], -1).astype(np.uint8)
def dec565(p):
    r = (p >> 11) & 31; g = (p >> 5) & 63; b = p & 31
    return np.stack([r * 255 // 31, g * 255 // 63, b * 255 // 31, np.full_like(r, 255)], -1).astype(np.uint8)
def dec1555(p):
    a = (p >> 15) & 1; r = (p >> 10) & 31; g = (p >> 5) & 31; b = p & 31
    return np.stack([r * 255 // 31, g * 255 // 31, b * 255 // 31, a * 255], -1).astype(np.uint8)
def metrics(rgba):
    f = rgba.astype(np.float32)
    rgb = f[..., :3]
    grad = np.abs(np.diff(rgb, axis=1)).mean() + np.abs(np.diff(rgb, axis=0)).mean()
    c = np.corrcoef(rgb.reshape(-1, 3).T)
    corr = (c[0, 1] + c[0, 2] + c[1, 2]) / 3
    a = f[..., 3]
    return grad, corr, float((a == 255).mean()), float((a == 0).mean()), float(a.mean())
def analyse(name, raw, wd, ht):
    p = np.frombuffer(raw[:wd * ht * 2], dtype='<u2').reshape(ht, wd)
    w('\n== %s %dx%d  first u16: %s' % (name, wd, ht, ' '.join('%04x' % v for v in p.ravel()[:8])))
    # bit statistics
    bits = [(float(((p >> i) & 1).mean())) for i in range(16)]
    w('   bit mean (bit15..bit0): ' + ' '.join('%.2f' % b for b in bits[::-1]))
    imgs = []
    for lbl, fn in (('4444', dec4444), ('565', dec565), ('1555', dec1555)):
        rgba = fn(p.astype(np.uint32))
        g, c, a255, a0, am = metrics(rgba)
        w('   %-5s gradient=%.2f (lower=smoother) channel-corr=%.3f alpha==max %.1f%% alpha==0 %.1f%% mean alpha %.1f' % (lbl, g, c, 100 * a255, 100 * a0, am))
        im = Image.fromarray(rgba, 'RGBA'); im.save(OUT + 'tex_%s_%s.png' % (name, lbl)); imgs.append(im)
    # composite on checker background
    W = sum(i.width for i in imgs) + 20; H = max(i.height for i in imgs)
    comp = Image.new('RGBA', (W, H), (90, 90, 90, 255))
    xo = 0
    for im in imgs:
        bg = Image.new('RGBA', im.size, (200, 0, 200, 255)); bg.alpha_composite(im); comp.paste(bg, (xo, 0)); xo += im.width + 10
    comp.convert('RGB').save(OUT + 'tex_%s_compare.png' % name)
if os.path.exists(PAK):
    d = open(PAK, 'rb').read()
    cnt = struct.unpack_from('<I', d, 4)[0]
    dirs = [struct.unpack_from('<III', d, 0x100 + 12 * i) for i in range(cnt)]
    w('%s directory: %s' % (os.path.basename(PAK), dirs if cnt <= 16 else '%d entries' % cnt))
    done = 0
    for fmt, o, sz in dirs:
        if o == 0: continue
        if done >= args.max_entries: break
        hdr = d[o:o + 0x50]; name = hdr[:32].split(b'\0')[0].decode('latin1')
        wd, ht, f = struct.unpack_from('<HHB', hdr, 32); csize = struct.unpack_from('<I', hdr, 37)[0]
        raw = zlib.decompress(d[o + 0x50:o + 0x50 + csize])
        w('entry %s fmt=%d w=%d h=%d csize=%d raw=%d (=w*h*2: %s)' % (name, f, wd, ht, csize, len(raw), len(raw) == wd * ht * 2))
        if f == 4:
            analyse(name.replace('.', '_'), raw, wd, ht)
        else:
            w('   fmt %d is not 16-bit; skipped (fmt 6 = ARGB8888, w*h*4 = %s)' % (f, len(raw) == wd * ht * 4))
        done += 1
elif not args.entry:
    sys.exit('no pak found at %s and no --entry given' % PAK)
if args.entry:
    # truncated entry (e.g. the 80-byte header and the first bytes of its zlib stream, cut from texture.pak)
    e = open(args.entry, 'rb').read()
    name = e[:32].split(b'\0')[0].decode('latin1'); wd, ht, f = struct.unpack_from('<HHB', e, 32)
    dobj = zlib.decompressobj(); raw = dobj.decompress(e[0x50:])
    rows = len(raw) // (wd * 2)
    w('entry %s fmt=%d %dx%d: partial inflate %d bytes = %d full rows' % (name, f, wd, ht, len(raw), rows))
    if rows >= 8: analyse(name.replace('.', '_') + '_partial', raw, wd, rows)
log.close()
