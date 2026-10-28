#!/usr/bin/env python3
"""sndProfiles.pak (SPF v1): header, directory 8,192×12, 184 B records (name[32], u32, then u16 sample ids);
list of used profiles, grouping by name prefix, range and number of distinct sample ids.

Purpose (docs/02 §10.1: 8,192 slots of 184 B, 175 profiles, 2,355 distinct sample ids; docs/06 §2: the profiles
in the new sound backend).

Example
  uv run python -I tools/analysis/formats/spf_names.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/formats/spf_names.py OriginalGame/PAK/sndProfiles.pak

Expected output: "used 175", "prefix groups: ...", "sample id range: ... distinct 2355", then one line per profile
(index, name, v, number of ids, first 12 ids). Dependencies: standard library.
"""
import sys, struct, collections, argparse, os

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='List sound profiles of sndProfiles.pak (SPF v1).')
ap.add_argument('path', nargs='?', help='sndProfiles.pak (default <game-dir>/PAK/sndProfiles.pak)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
args = ap.parse_args()
p = args.path or os.path.join(args.game_dir, 'PAK', 'sndProfiles.pak')
d = open(p, 'rb').read()
rec = 184
start = 0x100 + 8192 * 12
print("magic", d[:4], "size", len(d), "records start", start, "check", start + 8192 * rec == len(d))
hdr = struct.unpack_from('<16I', d, 4)
print("header u32s:", hdr[:12])
cat = [struct.unpack_from('<3I', d, 0x100 + i * 12) for i in range(8192)]
nz = [c for c in cat if any(c)]
print("catalog nonzero entries:", len(nz), "first 5:", nz[:5])
used = []
for i in range(8192):
    r = d[start + i * rec:start + (i + 1) * rec]
    name = r[:32].split(b'\0', 1)[0].decode('latin1', 'replace')
    if name:
        one = struct.unpack_from('<I', r, 32)[0]
        ids = struct.unpack_from('<%dH' % ((rec - 36) // 2), r, 36)
        used.append((i, name, one, ids))
print("used", len(used))
groups = collections.Counter()
for i, name, one, ids in used:
    groups[name.split('_')[0].lower()] += 1
print("prefix groups:", groups.most_common(60))
allids = []
for i, name, one, ids in used:
    allids += [x for x in ids if x]
print("sample id range:", min(allids), max(allids), "distinct", len(set(allids)))
print("---- all names ----")
for i, name, one, ids in used:
    z = [x for x in ids if x]
    print(f"{i:4d} {name:32s} v={one} n={len(z):3d} ids={z[:12]}")
