#!/usr/bin/env python3
"""Client↔server function matching by normalized hashes (fn_match_lib.analyse) with a breakdown of the matched
bytes by subsystem labels (from pe_deep <out>.funcs.json) and by 64 KB blocks of .text in both images.

Purpose (docs/01-binary-analysis.md, §5.1): what share of the server code is byte-identical to the client, in which
modules, and at which addresses the shared code lies.

Example
  python3 -I tools/analysis/pe/fn_match.py --game-dir ./OriginalGame \\
      --client-funcs /tmp/analysis/sacred_deep.json.funcs.json --server-funcs /tmp/analysis/gs_deep.json.funcs.json \\
      --out /tmp/analysis/fn_match2.json

Expected output: "GS matched bytes by GS label (matched/total)", "Sacred matched bytes by Sacred label", tables
per 64 KB; JSON with gs_label_match / sacred_label_match / sacred_rows. Two full disassemblies take several minutes.
The executables are read as is. Dependencies: pefile, capstone.
"""
import json, collections, bisect, sys, time, argparse, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey
from fn_match_lib import analyse

ap = argparse.ArgumentParser(description='Client<->server function matching by relocation-insensitive hashes.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--client', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--server', help='GameServer.exe (default <game-dir>/GameServer.exe)')
ap.add_argument('--client-funcs', required=True, help='pe_deep funcs.json of the client')
ap.add_argument('--server-funcs', required=True, help='pe_deep funcs.json of the server')
ap.add_argument('--out', required=True, help='output JSON')
args = ap.parse_args()
client = args.client or os.path.join(args.game_dir, 'Sacred.exe')
server = args.server or os.path.join(args.game_dir, 'GameServer.exe')
def text_range(path):
    pe = pefile.PE(path, fast_load=True); base = pe.OPTIONAL_HEADER.ImageBase
    t = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
    return base + t.VirtualAddress, base + t.VirtualAddress + t.Misc_VirtualSize
STS, STE = text_range(client); GTS, GTE = text_range(server)
t0 = time.time()
G = analyse(server); S = analyse(client)
print("analysed in %.0fs" % (time.time() - t0))
gf = json.load(open(args.server_funcs)); sf = json.load(open(args.client_funcs))
def labeller(fl):
    st = [f[0] for f in fl]; lab = [f[2] for f in fl]
    def L(a):
        i = bisect.bisect_right(st, a) - 1
        return lab[i] if i >= 0 else '?'
    return L
GL = labeller(gf); SL = labeller(sf)
MIN = 32
gh = collections.defaultdict(list); sh = collections.defaultdict(list)
for a, (sz, h) in G.items():
    if sz >= MIN: gh[h].append(a)
for a, (sz, h) in S.items():
    if sz >= MIN: sh[h].append(a)
common = set(gh) & set(sh)
# matched bytes by GS label and by Sacred label
gm = collections.Counter(); gt = collections.Counter(); sm = collections.Counter(); stt = collections.Counter()
for a, (sz, h) in G.items():
    if sz < MIN: continue
    gt[GL(a)] += sz
    if h in common: gm[GL(a)] += sz
for a, (sz, h) in S.items():
    if sz < MIN: continue
    stt[SL(a)] += sz
    if h in common: sm[SL(a)] += sz
print("GS matched bytes by GS label (matched/total):", {k: (gm[k], gt[k]) for k in gt})
print("Sacred matched bytes by Sacred label (matched/total):", {k: (sm[k], stt[k]) for k in stt})
# per 64KB layout for Sacred: total func bytes, matched bytes, top labels
rows = []
Ssorted = sorted(S.items())
for k in range(0, (STE - STS) // 0x10000 + 1):
    lo = STS + k * 0x10000; hi = lo + 0x10000
    tot = 0; mat = 0; labs = collections.Counter(); nf = 0
    for a, (sz, h) in Ssorted:
        if lo <= a < hi:
            nf += 1; tot += sz; labs[SL(a)] += sz
            if sz >= MIN and h in common: mat += sz
    rows.append((lo, nf, tot, mat, labs.most_common(3)))
json.dump({'gs_label_match': {k: (gm[k], gt[k]) for k in gt}, 'sacred_label_match': {k: (sm[k], stt[k]) for k in stt}, 'sacred_rows': [(hex(r[0]), r[1], r[2], r[3], r[4]) for r in rows]}, open(args.out, 'w', encoding='utf-8', newline=''), indent=1)
print("Sacred per-64KB: start, funcs, bytes, matched-with-GS bytes, top labels")
for r in rows: print("  0x%x %4d %6d %6d %s" % (r[0], r[1], r[2], r[3], r[4]))
rows = []
Gsorted = sorted(G.items())
for k in range(0, (GTE - GTS) // 0x10000 + 1):
    lo = GTS + k * 0x10000; hi = lo + 0x10000
    tot = 0; mat = 0; labs = collections.Counter(); nf = 0
    for a, (sz, h) in Gsorted:
        if lo <= a < hi:
            nf += 1; tot += sz; labs[GL(a)] += sz
            if sz >= MIN and h in common: mat += sz
    rows.append((lo, nf, tot, mat, labs.most_common(3)))
print("GameServer per-64KB:")
for r in rows: print("  0x%x %4d %6d %6d %s" % (r[0], r[1], r[2], r[3], r[4]))
