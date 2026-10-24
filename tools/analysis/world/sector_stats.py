#!/usr/bin/env python3
"""Statistics over the 32-byte floor cells of the sectors in sectors.wldx (via offsets from sectors.keyx): histograms
for each byte, int8 corner heights (+0x10..+0x13), consistency of shared corners for two traversal orders
(A: idx = r*64+c, B: idx = c*64+r), light fields (+0x14..+0x17), other fields, the most "hilly" sectors and PNG
height/light maps for three of them.

Purpose (docs/02 §5.2–5.3): the terrain is a vertex-shared
int8 heightfield (step 5, 78.4% flat cells), the cell layout is 32 B. keyx record format: 768 B, with
+0x20 (idx, ordinal), +0xEC (offset, csize), +0x108 (usize); sector data is zlib; cells from +0x120, 64×64×32 B.

Example
  python3 -I tools/analysis/world/sector_stats.py --game-dir ./OriginalGame --out /tmp/analysis/geom          # all 6050 sectors (~2–4 min)
  python3 -I tools/analysis/world/sector_stats.py --game-dir ./OriginalGame --out /tmp/analysis/geom --max-sectors 300

Reads World/sectors.keyx (4.6 MB) and World/sectors.wldx (64 MB). Output: <out>/sector_stats.txt,
<out>/sector<N>_height.png, sector<N>_light.png. Dependencies: numpy, pillow (PNG optional).
"""
import sys, os, struct, zlib, collections, argparse
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Floor-cell statistics over sectors.wldx (heights, light, shared-corner tests).')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--keyx', help='World/sectors.keyx (default under game-dir)')
ap.add_argument('--wldx', help='World/sectors.wldx (default under game-dir)')
ap.add_argument('--max-sectors', type=int, default=6050)
ap.add_argument('--out', required=True, help='output directory')
args = ap.parse_args()
KEYX = args.keyx or os.path.join(args.game_dir, 'World', 'sectors.keyx')
WLDX = args.wldx or os.path.join(args.game_dir, 'World', 'sectors.wldx')
os.makedirs(args.out, exist_ok=True)
OUT = os.path.join(args.out, '')
MAXSEC = args.max_sectors
keyx = open(KEYX, 'rb').read()
wldx = open(WLDX, 'rb').read()
if len(wldx) < 1024:
    sys.exit('%s is too short (%d bytes) to be the sector data file' % (WLDX, len(wldx)))
hdr = struct.unpack_from('<4sIIIiI', keyx, 0)
print('keyx header', hdr, 'size', len(keyx), 'records', (len(keyx) - 0x100) / 768.0)
BASE = 0x100
NREC = (len(keyx) - BASE) // 768
recs = []
for i in range(NREC):
    r = BASE + i * 768
    idx, ordn = struct.unpack_from('<II', keyx, r + 0x20)
    off, csz = struct.unpack_from('<II', keyx, r + 0xec)
    usz = struct.unpack_from('<I', keyx, r + 0x108)[0]
    recs.append((i, idx, ordn, off, csz, usz))
print('first recs', recs[:3])
print('wldx bytes at first offset', wldx[recs[0][3]:recs[0][3] + 2].hex())

hist = np.zeros((32, 256), dtype=np.int64)
ncells = 0
rel = {'A': collections.Counter(), 'B': collections.Counter()}
reltot = {'A': 0, 'B': 0}
lrel = {'A': collections.Counter(), 'B': collections.Counter()}
hvar = []
hspan = collections.Counter()
allzero = 0
abs_sum = 0; abs_n = 0
hsample = {}
# fields of interest
u16_0 = collections.Counter(); u16_2 = collections.Counter(); u16_4 = collections.Counter(); u16_6 = collections.Counter()
u32_8 = collections.Counter(); u32_c_nonzero = 0; u16_1c = collections.Counter(); u16_1e = collections.Counter()
b18 = collections.Counter(); b19 = collections.Counter(); b1a = collections.Counter(); b1b = collections.Counter()
b1c = collections.Counter(); b1d = collections.Counter(); b1e = collections.Counter(); b1f = collections.Counter()
hi1f = collections.Counter()
def corner_tests(H, key):
    """H: (64,64,4) int arrays [left, top, right, bottom] indexed [a, b]. Test shared-corner equalities assuming
    tile (r,c)=(a,b) (key 'A') or (r,c)=(b,a) (key 'B')."""
    if key == 'A': G = H
    else: G = H.transpose(1, 0, 2)
    L, T, R, Bm = G[..., 0], G[..., 1], G[..., 2], G[..., 3]
    tests = {
        'bottom(r,c)==left(r,c+1)': (Bm[:, :-1], L[:, 1:]),
        'right(r,c)==top(r,c+1)': (R[:, :-1], T[:, 1:]),
        'left(r,c)==top(r+1,c)': (L[:-1, :], T[1:, :]),
        'bottom(r,c)==right(r+1,c)': (Bm[:-1, :], R[1:, :]),
        'bottom(r,c)==top(r+1,c+1)': (Bm[:-1, :-1], T[1:, 1:]),
    }
    out = {}
    for k, (x, y) in tests.items():
        out[k] = (int((x == y).sum()), int(x.size))
    return out
done = 0
for (i, idx, ordn, off, csz, usz) in recs[:MAXSEC]:
    try:
        d = zlib.decompress(wldx[off:off + csz])
    except Exception as e:
        print('sector', i, 'inflate failed', e); continue
    if len(d) < 0x20120: print('sector', i, 'short', len(d)); continue
    cells = np.frombuffer(d[0x120:0x20120], dtype=np.uint8).reshape(4096, 32)
    ncells += 4096
    for b in range(32):
        hist[b] += np.bincount(cells[:, b], minlength=256)
    H = cells[:, 0x10:0x14].astype(np.int8).astype(np.int32).reshape(64, 64, 4)
    Lg = cells[:, 0x14:0x18].astype(np.int32).reshape(64, 64, 4)
    for key in ('A', 'B'):
        for k, (eq, n) in corner_tests(H, key).items():
            rel[key][k] += eq; reltot[key] += 0
        for k, (eq, n) in corner_tests(Lg, key).items():
            lrel[key][k] += eq
    reltot['n'] = reltot.get('n', 0) + 63 * 64
    v = float(H.var()); hvar.append((v, i, idx))
    span = H.max(axis=2) - H.min(axis=2)
    hspan.update(np.bincount(span.ravel(), minlength=1).nonzero()[0].tolist())  # presence only
    allzero += int((H == 0).all(axis=2).sum())
    abs_sum += int(np.abs(H).sum()); abs_n += H.size
    u16 = cells.view('<u2').reshape(4096, 16)
    u32 = cells.view('<u4').reshape(4096, 8)
    u16_2.update(u16[:, 1].tolist()); u16_6.update(u16[:, 3].tolist()); u16_1c.update(u16[:, 14].tolist()); u16_1e.update(u16[:, 15].tolist())
    u32_8.update(u32[:, 2].tolist()); u32_c_nonzero += int((u32[:, 3] != 0).sum())
    b18.update(cells[:, 0x18].tolist()); b19.update(cells[:, 0x19].tolist()); b1a.update(cells[:, 0x1a].tolist()); b1b.update(cells[:, 0x1b].tolist())
    b1c.update(cells[:, 0x1c].tolist()); b1d.update(cells[:, 0x1d].tolist()); b1e.update(cells[:, 0x1e].tolist()); b1f.update(cells[:, 0x1f].tolist())
    hi1f.update((cells[:, 0x1f] & 0xF0).tolist())
    u16_4.update((u16[:, 2] != 0).tolist())
    if len(hsample) < 3 and v > 40: hsample[i] = (H.copy(), Lg.copy(), idx)
    done += 1
    if done % 500 == 0: print('..', done, file=sys.stderr)
hvar.sort(reverse=True)
with open(OUT + 'sector_stats.txt', 'w', encoding='utf-8', newline='') as f:
    f.write('sectors %d cells %d\n' % (done, ncells))
    f.write('\n## per-byte histograms (offset: distinct values, top-8 value:count)\n')
    for b in range(32):
        h = hist[b]; nz = np.nonzero(h)[0]
        top = sorted(((int(h[v]), int(v)) for v in nz), reverse=True)[:8]
        f.write('+0x%02x: distinct=%3d min=%3d max=%3d  %s\n' % (b, len(nz), nz.min(), nz.max(), ' '.join('%d:%d' % (v, c) for c, v in top)))
    f.write('\n## int8 corner heights +0x10..+0x13\n')
    hs = hist[0x10:0x14].sum(axis=0)
    vals = np.arange(256).astype(np.int8).astype(np.int32)
    tot = hs.sum(); mean = (hs * vals).sum() / tot
    f.write('mean=%.3f  min=%d max=%d  all-four-zero cells=%d (%.1f%%)  mean|h|=%.3f\n' % (mean, vals[hs > 0].min(), vals[hs > 0].max(), allzero, 100.0 * allzero / ncells, abs_sum / abs_n))
    f.write('value histogram (int8): ' + ' '.join('%d:%d' % (vals[v], hs[v]) for v in np.argsort(vals) if hs[v]) + '\n')
    f.write('\n## shared-corner equality (heights) -- order A: idx=r*64+c, order B: idx=c*64+r; n per test ~ %d\n' % (done * 63 * 64))
    for key in ('A', 'B'):
        for k, eq in rel[key].items():
            n = done * (63 * 64 if 'c+1)' in k and 'r+1' not in k or ('r+1' in k and 'c+1' not in k) else 63 * 63)
            f.write('  %s %-28s %8d / %8d = %.2f%%\n' % (key, k, eq, n, 100.0 * eq / n))
    f.write('\n## shared-corner equality (light bytes +0x14..+0x17)\n')
    for key in ('A', 'B'):
        for k, eq in lrel[key].items():
            n = done * (63 * 64 if 'c+1)' in k and 'r+1' not in k or ('r+1' in k and 'c+1' not in k) else 63 * 63)
            f.write('  %s %-28s %8d / %8d = %.2f%%\n' % (key, k, eq, n, 100.0 * eq / n))
    f.write('\n## hilliest sectors (height variance, keyx row, sectorIndex y*100+x)\n')
    for v, i, idx in hvar[:15]: f.write('  var=%.1f row=%d sector=%d (x=%d,y=%d)\n' % (v, i, idx, idx % 100, idx // 100))
    f.write('flat sectors (var==0): %d of %d\n' % (sum(1 for v, i, idx in hvar if v == 0), done))
    def top(c, n=12): return ' '.join('%s:%d' % (k, v) for k, v in c.most_common(n))
    f.write('\n## other fields\n')
    f.write('u16 +0x02 flags: distinct=%d top: %s\n' % (len(u16_2), top(u16_2)))
    f.write('u16 +0x04 nonzero: %s\n' % top(u16_4))
    f.write('u16 +0x06: distinct=%d top: %s\n' % (len(u16_6), top(u16_6)))
    f.write('u32 +0x08: distinct=%d top: %s\n' % (len(u32_8), top(u32_8)))
    f.write('u32 +0x0c nonzero cells: %d (%.1f%%)\n' % (u32_c_nonzero, 100.0 * u32_c_nonzero / ncells))
    for nm, c in (('b+0x18', b18), ('b+0x19', b19), ('b+0x1a', b1a), ('b+0x1b', b1b), ('b+0x1c', b1c), ('b+0x1d', b1d), ('b+0x1e', b1e), ('b+0x1f', b1f), ('b+0x1f&0xF0', hi1f)):
        f.write('%s: distinct=%d top: %s\n' % (nm, len(c), top(c)))
    f.write('u16 +0x1c: distinct=%d top: %s\n' % (len(u16_1c), top(u16_1c)))
    f.write('u16 +0x1e: distinct=%d top: %s\n' % (len(u16_1e), top(u16_1e)))
print(open(OUT + 'sector_stats.txt').read())
# PNGs of sample hilly sectors: heights (left corner) and light (left corner), order A
try:
    from PIL import Image
    for i, (H, Lg, idx) in hsample.items():
        h = H[..., 0]; lo, hi = h.min(), h.max()
        img = ((h - lo) * (255.0 / max(1, hi - lo))).astype(np.uint8)
        Image.fromarray(img).resize((256, 256), Image.NEAREST).save(OUT + 'sector%d_height.png' % idx)
        Image.fromarray(Lg[..., 0].astype(np.uint8)).resize((256, 256), Image.NEAREST).save(OUT + 'sector%d_light.png' % idx)
        print('png for sector', idx, 'height range', lo, hi)
except Exception as e:
    print('PIL failed', e)
