#!/usr/bin/env python3
"""Static.PAK: a check of the record stride and of the field distributions (z, scale, x, y, flags) over all records, cross-tables
with the template type from Items.pak (ITM: directory 32,768×12 + 128 B records, the type is the first byte of a record).

Purpose: Static.PAK =
header 0x100 + directory count×12 (type, offset, size) + 1,038,014 records of 64 B; fields +0 index, +4 templateId,
+8 flags, +0xC sectorOrdinal, +0xE x, +0x10 z, +0x12 y, +0x2A scale. These are the early guesses the script tests;
the confirmed layout is in docs/02 §5.5 (i32 x at +0x0E, whose high half is the "z" here, and i32 y at +0x12),
checked on every record by world_dump.py.

Example
  uv run python -I tools/analysis/world/static_stats.py --game-dir ./OriginalGame --out /tmp/analysis/geom

Reads World/Static.PAK (79 MB) and PAK/Items.pak. Output: <out>/static_stats.txt (+ stdout).
Dependencies: numpy.
"""
import struct, collections, sys, os, argparse
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Static.PAK record layout check and field distributions.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--static', help='World/Static.PAK (default under game-dir)')
ap.add_argument('--items', help='PAK/Items.pak (default under game-dir)')
ap.add_argument('--out', required=True, help='output directory')
args = ap.parse_args()
STATIC = args.static or os.path.join(args.game_dir, 'World', 'Static.PAK')
ITEMS = args.items or os.path.join(args.game_dir, 'PAK', 'Items.pak')
os.makedirs(args.out, exist_ok=True)
OUT = os.path.join(args.out, '')
d = open(STATIC, 'rb').read()
if len(d) < 0x100:
    sys.exit('%s is too short for a PAK (%d bytes)' % (STATIC, len(d)))
magic, count = struct.unpack_from('<4sI', d, 0)
out = open(OUT + 'static_stats.txt', 'w', encoding='utf-8', newline='')
def w(s):
    out.write(s + '\n'); print(s)
w('magic %r count %d filesize %d' % (magic, count, len(d)))
for stride in (64, 76, 80):
    w('stride %d: (filesize-0x100)/stride = %.3f ; (filesize-0x100)/count = %.3f' % (stride, (len(d) - 0x100) / stride, (len(d) - 0x100) / count))
# detect base/stride by u32 index monotonic
for base in (0x100,):
    for stride in (64, 76):
        idxs = [struct.unpack_from('<I', d, base + k * stride)[0] for k in range(8)]
        w('base 0x%x stride %d first u32 fields: %s' % (base, stride, idxs))
# dump first 3 records hex at stride 76
for k in range(3):
    r = d[0x100 + k * 76: 0x100 + (k + 1) * 76]
    w('rec %d: %s' % (k, ' '.join('%02x' % b for b in r)))
# Layout: 0x100 header, then count x 12-byte catalog (u32 type, u32 offset, u32 size), then count x 64-byte records
cat = np.frombuffer(d[0x100:0x100 + count * 12], dtype='<u4').reshape(count, 3)
w('catalog: types %s sizes %s first offset 0x%x expected 0x%x; offsets contiguous: %s' % (
    sorted(set(cat[:, 0].tolist()))[:5], sorted(set(cat[:, 2].tolist()))[:5], cat[0, 1], 0x100 + count * 12, bool((np.diff(cat[:, 1]) == 64).all())))
RB = int(cat[0, 1]); STRIDE = 64; n = count
arr = np.frombuffer(d[RB:RB + n * STRIDE], dtype=np.uint8).reshape(n, STRIDE)
w('records: %d x 64 bytes at 0x%x; file end check: %d == %d' % (n, RB, RB + n * 64, len(d)))
for k in range(3): w('rec %d: %s' % (k, ' '.join('%02x' % b for b in arr[k])))
u16 = lambda o: arr[:, o:o + 2].copy().view('<u2').ravel()
i16 = lambda o: arr[:, o:o + 2].copy().view('<i2').ravel()
u32 = lambda o: arr[:, o:o + 4].copy().view('<u4').ravel()
idx = u32(0); tmpl = u16(4); f6 = u16(6); flags = u16(8); fa = u16(0xa); sec = u16(0xc); x = i16(0xe); z = i16(0x10); y = i16(0x12); f14 = u16(0x14)
scale = u16(0x2a)
w('index monotonic: %s (first %d last %d)' % (bool((np.diff(idx) == 1).all()), idx[0], idx[-1]))
def hist(name, a, top=20):
    c = collections.Counter(a.tolist())
    w('%s: distinct=%d min=%d max=%d top: %s' % (name, len(c), a.min(), a.max(), ' '.join('%d:%d' % kv for kv in c.most_common(top))))
hist('templateId +4', tmpl, 10); hist('+6', f6); hist('flags +8', flags); hist('+0xa', fa); hist('sectorOrdinal +0xc', sec, 5)
hist('x +0xe', x, 10); hist('z +0x10', z); hist('y +0x12', y, 10); hist('+0x14', f14); hist('scale +0x2a', scale)
w('x range %d..%d  y range %d..%d' % (x.min(), x.max(), y.min(), y.max()))
# per-sector local coordinate check: distribution of x within a sector
for s in (915, 1498, 1731):
    m = sec == s
    if m.any(): w('sector ordinal %d: n=%d x %d..%d y %d..%d z set %s' % (s, m.sum(), x[m].min(), x[m].max(), y[m].min(), y[m].max(), sorted(set(z[m].tolist()))))
# other u16 fields in the record
for o in range(0x16, 0x4c, 2):
    a = u16(o); c = collections.Counter(a.tolist())
    w('u16 +0x%02x: distinct=%d top: %s' % (o, len(c), ' '.join('%d:%d' % kv for kv in c.most_common(6))))
# cross z x scale
ct = collections.Counter(zip(z.tolist(), scale.tolist()))
w('z x scale: ' + ' '.join('(z=%d,s=%d):%d' % (k[0], k[1], v) for k, v in sorted(ct.items())))
# template type from Items.pak
it = open(ITEMS, 'rb').read()
ib = 0x100 + 32768 * 12
types = np.frombuffer(it[ib:ib + 32768 * 128], dtype=np.uint8).reshape(32768, 128)[:, 0]
tt = types[np.clip(tmpl, 0, 32767)]
ct2 = collections.Counter(zip(tt.tolist(), z.tolist()))
w('templateType x z: ' + ' '.join('(t=%d,z=%d):%d' % (k[0], k[1], v) for k, v in sorted(ct2.items())))
ct3 = collections.Counter(zip(tt.tolist(), scale.tolist()))
w('templateType x scale: ' + ' '.join('(t=%d,s=%d):%d' % (k[0], k[1], v) for k, v in sorted(ct3.items())))
ct4 = collections.Counter(zip(flags.tolist(), z.tolist()))
w('flags x z: ' + ' '.join('(f=%d,z=%d):%d' % (k[0], k[1], v) for k, v in sorted(ct4.items())))
out.close()
