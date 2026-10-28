#!/usr/bin/env python3
"""(1) How many client functions are byte-identical (up to relocations) to GameServer.exe functions, by module
(pe_deep labels) and by size class (simple <= 256 B / complex); (2) how "templated" the 178 cParticleSystem_*
classes are: the number of distinct bodies per virtual slot, fully duplicated body sets.

Purpose (docs/01-binary-analysis.md, §4.2 and §5.1): client↔server shared code by module, and how much of the
particle classes is generated from templates.

Example
  uv run python -I tools/analysis/pe/fn_census2.py --game-dir ./OriginalGame --funcs /tmp/analysis/sacred_deep.json.funcs.json \\
      --out /tmp/analysis/fn_census2.json

Expected output: shared_by_region_label, shared_game_total (≈1,176 functions / 227 KB without STL), particle_slot_stats for
7 slots, particle_classes_with_vtable=178. Two full disassemblies (fn_match_lib) take several minutes.
The executables are read as is. Dependencies: pefile, capstone.
"""
import json, sys, bisect, collections, statistics, struct, re, argparse, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey
from fn_match_lib import analyse

ap = argparse.ArgumentParser(description='Client functions byte-identical to the server, per module; particle-class template-ness.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--client', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--server', help='GameServer.exe (default <game-dir>/GameServer.exe)')
ap.add_argument('--tail', type=hexkey, default=0x7F1000, help='first VA of the CRT/library tail of the client')
ap.add_argument('--funcs', required=True, help='pe_deep <out>.funcs.json of the client')
ap.add_argument('--out', required=True, help='output JSON')
args = ap.parse_args()
client = args.client or os.path.join(args.game_dir, 'Sacred.exe')
server = args.server or os.path.join(args.game_dir, 'GameServer.exe')
TAIL = args.tail
G = analyse(server); S = analyse(client)
funcs = json.load(open(args.funcs))
starts = [f[0] for f in funcs]; size = {f[0]: f[1] for f in funcs}; lab = {f[0]: f[2] for f in funcs}
def func_of(a):
    i = bisect.bisect_right(starts, a) - 1
    return starts[i] if i >= 0 else None
MIN = 32
gh = collections.defaultdict(list)
for a, (sz, h) in G.items():
    if sz >= MIN: gh[h].append(a)
R = {}
shared = collections.defaultdict(lambda: dict(funcs=0, bytes=0, simple=0, complex=0)); tot = collections.defaultdict(lambda: dict(funcs=0, bytes=0))
shared_fn = set()
for a, (sz, h) in S.items():
    if sz < MIN: continue
    f = func_of(a); L = lab.get(f, '?') if f is not None else '?'
    region = 'tail' if a >= TAIL else 'game'
    tot[(region, L)]['funcs'] += 1; tot[(region, L)]['bytes'] += sz
    if h in gh:
        d = shared[(region, L)]; d['funcs'] += 1; d['bytes'] += sz; shared_fn.add(a)
        if sz <= 256: d['simple'] += 1
        else: d['complex'] += 1
R['shared_by_region_label'] = {'%s/%s' % k: dict(shared=v, total=tot[k]) for k, v in sorted(shared.items())}
R['shared_game_total'] = dict(funcs=sum(v['funcs'] for k, v in shared.items() if k[0] == 'game'), bytes=sum(v['bytes'] for k, v in shared.items() if k[0] == 'game'))
R['shared_game_nonlib'] = dict(funcs=sum(v['funcs'] for k, v in shared.items() if k[0] == 'game' and k[1] not in ('stl', 'crt')), bytes=sum(v['bytes'] for k, v in shared.items() if k[0] == 'game' and k[1] not in ('stl', 'crt')))
R['shared_all'] = dict(funcs=sum(v['funcs'] for v in shared.values()), bytes=sum(v['bytes'] for v in shared.values()))
# ---- particle classes: per-vslot similarity ----
pe = pefile.PE(client); base = pe.OPTIONAL_HEADER.ImageBase; img = bytes(pe.get_memory_mapped_image())
secs = [(s.Name.rstrip(b'\0').decode(), base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData)) for s in pe.sections]
txt = [x for x in secs if x[0] == '.text'][0]; TS, TE = txt[1], txt[2]
def rd32(va): return struct.unpack_from('<I', img, va - base)[0]
def sec_of(va):
    for nm, a, b in secs:
        if a <= va < b: return nm
tds = {}
for m in re.finditer(rb'\.\?A[VU][^\0]{1,400}?\0', img):
    raw = m.group(0)[:-1]
    if raw.endswith(b'@@'): tds[base + m.start() - 8] = raw.decode(errors='replace')
cols = {}
for nm, a, b in secs:
    if nm not in ('.rdata', '.data'): continue
    d = img[a - base:b - base]
    for o in range(0, len(d) - 20, 4):
        if d[o:o + 4] != b'\0\0\0\0': continue
        ptd = struct.unpack_from('<I', d, o + 12)[0]
        if ptd in tds:
            pchd = struct.unpack_from('<I', d, o + 16)[0]
            if sec_of(pchd) in ('.rdata', '.data') and rd32(pchd) == 0: cols[a + o] = ptd
colset = set(cols); vt = {}
for nm, a, b in secs:
    if nm not in ('.rdata', '.data'): continue
    d = img[a - base:b - base]
    for o in range(0, len(d) - 4, 4):
        v = struct.unpack_from('<I', d, o)[0]
        if v in colset:
            oo = o + 4; entries = []
            while oo + 4 <= len(d):
                p = struct.unpack_from('<I', d, oo)[0]
                if TS <= p < TE: entries.append(p); oo += 4
                else: break
            name = tds[cols[v]]
            if b'cParticleSystem_' in name.encode() or name.startswith('.?AVcParticleSystem_'):
                vt[a + o + 4] = (name, entries)
# dedupe by class (keep the longest vtable per class)
best = {}
for va, (nm, e) in vt.items():
    if nm not in best or len(e) > len(best[nm][1]): best[nm] = (va, e)
R['particle_classes_with_vtable'] = len(best)
slots = collections.defaultdict(list)
for nm, (va, e) in best.items():
    for k, p in enumerate(e): slots[k].append(p)
slot_stats = {}
for k, ps in sorted(slots.items()):
    hs = [S.get(p, (0, None))[1] for p in ps]; szs = [S.get(p, (0, 0))[0] for p in ps]
    cnt = collections.Counter(hs)
    uniq_fn = len(set(ps)); uniq_body = len(set(h for h in hs if h))
    dup_classes = sum(c for h, c in cnt.items() if h and c > 1)
    slot_stats[k] = dict(classes=len(ps), distinct_funcs=uniq_fn, distinct_bodies=uniq_body, classes_sharing_a_body=dup_classes, size_median=statistics.median(szs), size_p90=sorted(szs)[int(len(szs) * .9)], size_max=max(szs))
R['particle_slot_stats'] = slot_stats
# how many particle classes are *entirely* duplicates of another class (all 7 bodies identical)?
sig = collections.Counter(tuple(S.get(p, (0, None))[1] for p in e) for nm, (va, e) in best.items())
R['particle_classes_fully_duplicate_body_sets'] = sum(c for s, c in sig.items() if c > 1)
R['particle_vtable_len_hist'] = dict(collections.Counter(len(e) for va, e in best.values()))
json.dump(R, open(args.out, 'w', encoding='utf-8', newline=''), indent=1, default=str)
for k, v in R.items(): print(k, '=', json.dumps(v, default=str)[:2500])
