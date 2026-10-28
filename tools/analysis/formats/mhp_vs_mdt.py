#!/usr/bin/env python3
"""Cross-check of MOTIONS.PAK (MHP v1, 198 B records) with Models.tmp (MDT v2: header 0x118, 1,574 ModelDesc × 1,194 B,
3,423 MotionDesc × 256 B): an MHP record == (u32 id, u16 id, u16 x) + MotionDesc[0x42:0x100] byte for byte; a census
of MotionDesc fields (+0x2C flags, +0x30 u16, float columns +0x80..+0xF8 by animation-name tokens), checks for
ATTACK/RUN/WALK, the table of animation references in ModelDesc (@0x74, 256 u32).

Purpose (docs/02 §4.1): MOTIONS.PAK is a cache of animation parameters
(the output of cGrannyModelManager::saveMotionParams), not a format of its own.

Example
  uv run python -I tools/analysis/formats/mhp_vs_mdt.py --game-dir ./OriginalGame

Expected output: "MHP[8:198]==MDT[0x42:0x100]: 3423/3423; u32 id == MDT u16@0x2E: ...". The constants 1574/1194/3423/256
match the 2.28 build (overridable with options). Dependencies: standard library.
"""
import sys, struct, collections, re, argparse, os

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Verify MOTIONS.PAK records against Models.tmp MotionDesc; field census.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--mhp', help='PAK/MOTIONS.PAK (default under game-dir)')
ap.add_argument('--mdt', help='PAK/Models.tmp (default under game-dir)')
ap.add_argument('--models', type=int, default=1574, help='ModelDesc count'); ap.add_argument('--model-size', type=int, default=1194)
ap.add_argument('--motions', type=int, default=3423, help='MotionDesc count'); ap.add_argument('--motion-size', type=int, default=256)
ap.add_argument('--mdt-header', type=lambda s: int(s, 0), default=0x118)
args = ap.parse_args()
mhp = open(args.mhp or os.path.join(args.game_dir, 'PAK', 'MOTIONS.PAK'), 'rb').read()
mdt = open(args.mdt or os.path.join(args.game_dir, 'PAK', 'Models.tmp'), 'rb').read()
cnt = struct.unpack_from('<I', mhp, 4)[0]; stride = 198
H = args.mdt_header; NM = args.models; SM = args.model_size; NA = args.motions; SA = args.motion_size; base = H + NM * SM
eq = 0; diffs = collections.Counter(); id_eq = 0; x_eq = 0
names = []
for i in range(cnt):
    r = mhp[0x100 + i * stride:0x100 + (i + 1) * stride]; d = mdt[base + i * SA:base + (i + 1) * SA]
    names.append(d[:32].split(b'\0')[0].decode('latin-1'))
    if r[8:] == d[0x42:0x100]: eq += 1
    else:
        for o in range(190):
            if r[8 + o] != d[0x42 + o]: diffs[o + 0x42] += 1
    if struct.unpack_from('<I', r, 0)[0] == struct.unpack_from('<H', d, 0x2E)[0]: id_eq += 1
    if r[4:8] == d[0x2E:0x30] + d[0x40:0x42]: x_eq += 1
print(f'MHP[8:198]==MDT[0x42:0x100]: {eq}/{cnt}; u32 id == MDT u16@0x2E: {id_eq}; MHP[4:8]==MDT[0x2E:0x30]+MDT[0x40:0x42]: {x_eq}')
print('MDT offsets with mismatches:', dict(diffs.most_common(20)))
# MDT fields 0x2C..0x42 census
f = collections.Counter()
for i in range(NA):
    d = mdt[base + i * SA:base + (i + 1) * SA]
    f['flags@2C=%d' % struct.unpack_from('<H', d, 0x2C)[0]] += 1
print('MotionDesc flags@0x2C:', dict(f))
v30 = [struct.unpack_from('<H', mdt, base + i * SA + 0x30)[0] for i in range(NA)]
print('u16@0x30 range', min(v30), max(v30), 'distinct', len(set(v30)), 'sample', v30[:8], '; == model count %d?' % NM)
# Which model does @0x30 refer to? check against ModelDesc id field? print ModelDesc names for a few
def mname(k):
    o = H + k * SM; return mdt[o:o + 32].split(b'\0')[0].decode('latin-1')
for i in [1, 1406, 1467, 1470, 1275]:
    if i < len(names):
        print(' motion', i, names[i], '@0x30=', v30[i], '-> ModelDesc name if index:', mname(v30[i]) if v30[i] < NM else '-')
# token census per float column
cols = [0x80, 0x84, 0x88, 0x8C, 0x9C, 0xA0, 0xA4, 0xA8, 0xAC, 0xB0, 0xB4, 0xB8, 0xBC, 0xF8]
def tokens(n):
    n = n.upper().replace('.GRN', '')
    return set(re.split(r'[_\-]', n)[1:])
for c in cols:
    vals = []; toks = collections.Counter()
    for i in range(NA):
        d = mdt[base + i * SA:base + (i + 1) * SA]
        x = struct.unpack_from('<f', d, c)[0]
        if x != 0:
            vals.append(x)
            for t in tokens(names[i]): toks[t] += 1
    if vals:
        print(f'MDT +{c:#x}: n={len(vals)} min={min(vals):.3f} max={max(vals):.3f} top tokens={toks.most_common(8)}')
# non-float use of +0x80 region? check attack motions: how many ATTACK names have +0xA0 nonzero
att = [i for i in range(NA) if 'ATTACK' in names[i].upper()]
nzA0 = sum(1 for i in att if struct.unpack_from('<f', mdt, base + i * SA + 0xA0)[0] != 0)
print(f'ATTACK motions: {len(att)}, with +0xA0 nonzero: {nzA0}')
run = [i for i in range(NA) if re.search(r'_(RUN|WALK|FWALK)', names[i].upper())]
nz80 = sum(1 for i in run if struct.unpack_from('<f', mdt, base + i * SA + 0x80)[0] != 0)
print(f'RUN/WALK motions: {len(run)}, with +0x80 nonzero: {nz80}; values:', collections.Counter(round(struct.unpack_from('<f', mdt, base + i * SA + 0x80)[0], 3) for i in run).most_common(6))
# ModelDesc: motion reference table location (256 u32 at +0x74?) check
md = mdt[H + 1 * SM:H + 2 * SM]
print('ModelDesc#1 name', md[:32].split(b'\0')[0], 'u32@0x20..0x30', struct.unpack_from('<4I', md, 0x20), 'u16@0x30..', struct.unpack_from('<4H', md, 0x30))
refs = struct.unpack_from('<256I', md, 0x74)
print('ModelDesc#1 motion refs @0x74 (first 20):', refs[:20], 'nonzero', sum(1 for r in refs if r))
print('ModelDesc tail @0x474..', md[0x474:0x4AA].hex())
