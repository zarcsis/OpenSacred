#!/usr/bin/env python3
"""Probing of MOTIONS.PAK (MHP v1, 3,423 × 198 B) and MotionDesc from Models.tmp (256 B) for field semantics:
census of nonzero/distinct values per byte offset, an "id == index" check, a search for float columns,
a census of the MotionDesc tail (>= 0x2C), a full dump of several records by known animation names.

Purpose (docs/02 §4.1: MOTIONS.PAK and MotionDesc). Read-only; prints statistics and hex.

Example
  python3 -I tools/analysis/formats/mhp_probe.py --game-dir ./OriginalGame

Expected output: "count 3423 stride 198 exact True", "id==index: 3423 / 3423", a list of float columns, "Models.tmp
size check: ... ". Dependencies: standard library.
"""
import sys, struct, collections, math, argparse, os

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Probe MOTIONS.PAK and Models.tmp MotionDesc field semantics (read-only).')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--mhp', help='PAK/MOTIONS.PAK (default under game-dir)')
ap.add_argument('--mdt', help='PAK/Models.tmp (default under game-dir)')
args = ap.parse_args()
mhp = open(args.mhp or os.path.join(args.game_dir, 'PAK', 'MOTIONS.PAK'), 'rb').read()
mdt = open(args.mdt or os.path.join(args.game_dir, 'PAK', 'Models.tmp'), 'rb').read()
print('MHP header:', mhp[:4], struct.unpack_from('<6I', mhp, 4))
cnt = struct.unpack_from('<I', mhp, 4)[0]
stride = (len(mhp) - 0x100) // cnt
print('count', cnt, 'stride', stride, 'exact', (len(mhp) - 0x100) % cnt == 0)
recs = [mhp[0x100 + i * stride:0x100 + (i + 1) * stride] for i in range(cnt)]
# byte-column census: for each offset, count distinct values / nonzero
nz = [0] * stride; distinct = [set() for _ in range(stride)]
for r in recs:
    for o, b in enumerate(r):
        if b: nz[o] += 1
        distinct[o].add(b)
print('nonzero count per byte offset (offset:nonzero/distinct):')
line = []
for o in range(stride):
    if nz[o]: line.append(f'{o:#x}:{nz[o]}/{len(distinct[o])}')
print(' '.join(line))
# interpret u32 at 0 and 4
ids = [struct.unpack_from('<I', r, 0)[0] for r in recs]
alias = [struct.unpack_from('<I', r, 4)[0] for r in recs]
print('id==index:', sum(1 for i, v in enumerate(ids) if v == i), '/', cnt, ' alias==id:', sum(1 for i, v in enumerate(alias) if v == ids[i]), ' alias distinct:', len(set(alias)))
# float columns: try every byte offset from 0x08..stride-4 where values look like floats in (0, 1e4)
def floats_at(off):
    vals = []
    for r in recs:
        f = struct.unpack_from('<f', r, off)[0]
        vals.append(f)
    return vals
print('float-like columns (fraction of records with finite |f|<1e4 and nonzero):')
for off in range(8, stride - 3):
    vals = floats_at(off)
    good = sum(1 for f in vals if math.isfinite(f) and 0 < abs(f) < 1e4)
    if good > cnt * 0.3:
        nzv = [f for f in vals if f != 0 and math.isfinite(f)]
        print(f'  +{off:#x}: {good}/{cnt} nonzero min={min(nzv):.4g} max={max(nzv):.4g} mean={sum(nzv) / len(nzv):.4g}')
# Models.tmp: header 0x118, then 1574 x 1194 ModelDesc, then 3423 x 256 MotionDesc
H = 0x118; NM = 1574; SM = 1194; NA = 3423; SA = 256
print('Models.tmp size check:', len(mdt), H + NM * SM + NA * SA)
mot_base = H + NM * SM
names = []
for i in range(NA):
    d = mdt[mot_base + i * SA:mot_base + (i + 1) * SA]
    names.append(d[:32].split(b'\0')[0].decode('latin-1'))
print('first motion names:', names[:6])
# dump the MotionDesc tail (after +0x2C) byte census
nz2 = [0] * SA
for i in range(NA):
    d = mdt[mot_base + i * SA:mot_base + (i + 1) * SA]
    for o, b in enumerate(d):
        if o >= 0x2C and b: nz2[o] += 1
print('MotionDesc nonzero per offset >=0x2C:', ' '.join(f'{o:#x}:{nz2[o]}' for o in range(0x2C, SA) if nz2[o]))
# sample a few records fully (hex) for motions with known names
want = ['SERA_IDLE_BH.GRN', 'SERA_RUN_1H.GRN', 'SERA_ATTACK_1H_A.GRN', 'GLAD_ATTACK_1H_A.GRN', 'BATX_ATTACK_BH_A.GRN']
for w in want:
    if w in names:
        i = names.index(w)
        r = recs[i]
        print(f'--- {w} motion#{i}')
        print(' MHP id/alias', ids[i], alias[i])
        print(' MHP bytes 0x08-0x45:', r[8:0x46].hex())
        print(' MHP f32 @0x46:', [round(x, 4) for x in struct.unpack_from('<4f', r, 0x46)])
        print(' MHP f32 @0x62:', [round(x, 4) for x in struct.unpack_from('<22f', r, 0x62)])
        print(' MHP tail @0xBA:', r[0xBA:].hex())
        d = mdt[mot_base + i * SA:mot_base + (i + 1) * SA]
        print(' MDT u32 @0x20..0x2C:', struct.unpack_from('<4I', d, 0x20))
        print(' MDT f32 @0x80:', [round(x, 4) for x in struct.unpack_from('<16f', d, 0x80)])
        print(' MDT bytes 0x2C-0x80:', d[0x2C:0x80].hex())
