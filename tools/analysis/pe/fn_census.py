#!/usr/bin/env python3
"""Normalization of the Sacred.exe function database: merges the 10,823 starts from pe_deep (<out>.funcs.json) with the
Ghidra export from sacred-bild (functions_eng.tsv, if given), identifies EH thunks by a byte pattern
(mov eax, FuncInfo; jmp), separates the library tail (>= --tail), sizes of modules and of the cParticleSystem_*/cUI_*
class families by vtable from RTTI, slices by log strings, a table of own-code modules.

Purpose (docs/01-binary-analysis.md, §4.1–4.2): 10,823 = 6,660 own + 1,625 library + 2,538 EH thunks; modules
game_sim 2,343 functions / 1,814 KB, UI 987 / 572 KB, etc.

Example
  uv run python -I tools/analysis/pe/fn_census.py --game-dir ./OriginalGame --funcs /tmp/analysis/sacred_deep.json.funcs.json \\
      --ghidra-tsv /path/to/sacred-bild/tools/data/functions_eng.tsv --fn-match2 /tmp/analysis/fn_match2.json \\
      --out /tmp/analysis/fn_census.json

functions_eng.tsv (GPL-3.0, kirides/sacred-bild) is not copied into the repository; its path is passed explicitly. Without it
the Ghidra statistics are empty and the other sections are still computed. Without --fn-match2 the sacred_label_match section is skipped.
The executable is read as is. Dependencies: pefile.
"""
import json, struct, bisect, re, collections, sys, statistics, argparse, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Normalise the client function base (pe_deep starts + Ghidra export + RTTI).')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--exe', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--tail', type=hexkey, default=0x7F1000, help='first VA of the CRT/library tail')
ap.add_argument('--funcs', required=True, help='pe_deep <out>.funcs.json of the client')
ap.add_argument('--ghidra-tsv', help='sacred-bild functions_eng.tsv (optional)')
ap.add_argument('--fn-match2', help='fn_match.py output JSON (optional)')
ap.add_argument('--out', required=True, help='output JSON')
args = ap.parse_args()
PATH = args.exe or os.path.join(args.game_dir, 'Sacred.exe'); TAIL = args.tail
pe = pefile.PE(PATH); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
secs = [(s.Name.rstrip(b'\0').decode(), base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData)) for s in pe.sections]
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
TS = base + txt.VirtualAddress; TE = TS + txt.Misc_VirtualSize
def sec_of(va):
    for nm, a, b in secs:
        if a <= va < b: return nm
def rd32(va): return struct.unpack_from('<I', img, va - base)[0]
def in_text(va): return TS <= va < TE

funcs = json.load(open(args.funcs))  # [start,size,label,direct]
starts = [f[0] for f in funcs]; size = {f[0]: f[1] for f in funcs}; lab = {f[0]: f[2] for f in funcs}; direct = {f[0]: f[3] for f in funcs}
A = set(starts)
def func_of(a):
    i = bisect.bisect_right(starts, a) - 1
    return starts[i] if i >= 0 else None
# EH thunks: mov eax, imm32 ; jmp rel32  (10 bytes)
def is_eh_thunk(s):
    o = s - base
    return img[o] == 0xB8 and img[o + 5] == 0xE9 and size[s] <= 16
thunks = {s for s in starts if is_eh_thunk(s)}
# IAT thunks jmp [imm32]
iat_thunk = {s for s in starts if img[s - base] == 0xFF and img[s - base + 1] == 0x25 and size[s] <= 8}
R = {}
R['A_total'] = len(A); R['eh_thunks_pattern'] = len(thunks); R['iat_thunks'] = len(iat_thunk)
R['eh_thunk_bytes'] = sum(size[s] for s in thunks)

# Ghidra export
G = {}; cats = collections.Counter()
if args.ghidra_tsv:
    for line in open(args.ghidra_tsv):
        if line.startswith('#') or not line.strip(): continue
        a, nm = line.rstrip('\n').split('\t')[:2]; a = int(a, 16)
        if nm.startswith('Catch@'): c = 'catch'
        elif nm.startswith('Unwind@'): c = 'unwind'
        elif nm.startswith('FUN_'): c = 'FUN'
        elif nm.startswith('thunk_'): c = 'thunk'
        else: c = 'named'
        cats[c] += 1; G[a] = (nm, c)
R['ghidra_categories'] = dict(cats)
Gf = {a for a, (nm, c) in G.items() if c in ('FUN', 'named', 'thunk')}
Geh = {a for a, (nm, c) in G.items() if c in ('catch', 'unwind')}
R['ghidra_functions'] = len(Gf); R['ghidra_eh_labels'] = len(Geh)
R['ghidra_funcs_in_text'] = sum(1 for a in Gf if in_text(a))
both = A & Gf; onlyA = A - Gf; onlyG = Gf - A
R['join'] = dict(both=len(both), onlyA=len(onlyA), onlyG=len(onlyG))
# classify onlyA
oa = collections.Counter()
for s in onlyA:
    if s in thunks: oa['eh_thunk'] += 1
    elif s in Geh: oa['ghidra_catch_or_unwind_label'] += 1
    elif s >= TAIL: oa['tail_lib_other'] += 1
    elif s in iat_thunk: oa['iat_thunk'] += 1
    else: oa['game_region_other'] += 1
R['onlyA_classes'] = dict(oa)
# onlyA game-region other: size distribution and how many are direct call targets? (not available) -> sizes
ga = [size[s] for s in onlyA if s not in thunks and s not in Geh and s < TAIL and s not in iat_thunk]
R['onlyA_game_other_sizes'] = dict(n=len(ga), bytes=sum(ga), median=statistics.median(ga) if ga else 0, le16=sum(1 for x in ga if x <= 16), le64=sum(1 for x in ga if x <= 64))
# sample a few onlyA game-region others with first bytes
samp = sorted(s for s in onlyA if s not in thunks and s not in Geh and s < TAIL and s not in iat_thunk)[:12]
R['onlyA_game_other_sample'] = [(hex(s), size[s], bytes(img[s - base:s - base + 8]).hex()) for s in samp]
# classify onlyG
og = collections.Counter(); inside = []
for a in onlyG:
    if not in_text(a): og['outside_text'] += 1; continue
    f = func_of(a)
    if a >= TAIL: og['tail'] += 1
    elif f is not None and f < a < f + size[f]: og['inside_A_function'] += 1; inside.append((a, f))
    else: og['gap_or_padding'] += 1
R['onlyG_classes'] = dict(og)
R['onlyG_inside_sample'] = [(hex(a), hex(f), size[f], bytes(img[a - base:a - base + 6]).hex()) for a, f in sorted(inside)[:10]]
# Ghidra functions by region
R['ghidra_funcs_tail'] = sum(1 for a in Gf if a >= TAIL); R['ghidra_funcs_game'] = sum(1 for a in Gf if in_text(a) and a < TAIL)
# Normalised own-function base
own = [s for s in starts if s < TAIL and s not in thunks and s not in iat_thunk and s not in Geh]
R['own_A'] = len(own); R['own_A_bytes'] = sum(size[s] for s in own)
R['tail_A'] = sum(1 for s in starts if s >= TAIL); R['tail_A_bytes'] = sum(size[s] for s in starts if s >= TAIL)
R['tail_A_nonthunk'] = sum(1 for s in starts if s >= TAIL and s not in thunks)
# both sets intersection restricted to game region
R['both_game'] = sum(1 for a in both if a < TAIL)

# ---- RTTI vtables -> class ownership ----
def demangle(nm):
    s = nm[4:]
    def parse_name(s, pos):
        parts = []
        while True:
            if s.startswith('?$', pos):
                pos += 2; end = s.index('@', pos); tname = s[pos:end]; pos = end + 1; depth = 0
                while not (s.startswith('@', pos) and depth == 0):
                    pos += 1
                pos += 1; parts.append(tname + '<...>')
            else:
                end = s.index('@', pos); parts.append(s[pos:end]); pos = end + 1
            if s.startswith('@', pos): pos += 1; break
        return '::'.join(reversed(parts)), pos
    try: return parse_name(s, 0)[0]
    except Exception: return nm
tds = {}
for m in re.finditer(rb'\.\?A[VU][^\0]{1,400}?\0', bytes(img)):
    raw = m.group(0)[:-1]
    if raw.endswith(b'@@'): tds[base + m.start() - 8] = raw.decode(errors='replace')
cols = {}
for nm, a, b in secs:
    if nm not in ('.rdata', '.data'): continue
    d = bytes(img[a - base:b - base])
    for o in range(0, len(d) - 20, 4):
        if d[o:o + 4] != b'\0\0\0\0': continue
        ptd = struct.unpack_from('<I', d, o + 12)[0]
        if ptd in tds:
            pchd = struct.unpack_from('<I', d, o + 16)[0]
            if sec_of(pchd) in ('.rdata', '.data') and rd32(pchd) == 0:
                cols[a + o] = dict(offset=struct.unpack_from('<I', d, o + 4)[0], td=ptd, chd=pchd)
vtables = {}; colset = set(cols)
for nm, a, b in secs:
    if nm not in ('.rdata', '.data'): continue
    d = bytes(img[a - base:b - base])
    for o in range(0, len(d) - 4, 4):
        v = struct.unpack_from('<I', d, o)[0]
        if v in colset:
            oo = o + 4; entries = []
            while oo + 4 <= len(d):
                p = struct.unpack_from('<I', d, oo)[0]
                if in_text(p): entries.append(p); oo += 4
                else: break
            vtables[a + o + 4] = (v, entries)
# class depth (number of bases) for ownership resolution
depth = {}
for col_va, c in cols.items():
    chd = c['chd']; nb = rd32(chd + 8); depth[c['td']] = nb
owner = {}  # func -> (class, depth)
vt_of_class = collections.defaultdict(list)
for vt, (col_va, entries) in vtables.items():
    td = cols[col_va]['td']; cn = demangle(tds[td]); dp = depth.get(td, 99)
    vt_of_class[cn].append(len(entries))
    for p in entries:
        if p not in A: continue
        if p not in owner or dp < owner[p][1]: owner[p] = (cn, dp)
R['vtable_funcs_owned'] = len(owner)
def family(cn):
    if cn.startswith('cParticleSystem'): return 'particles'
    if cn.startswith('cUI_'): return 'ui_widgets'
    if cn.startswith(('Command', 'cCommand', 'cMacroCommand')): return 'ui_commands'
    if cn.startswith('cEvent'): return 'events'
    if cn.startswith('_STL'): return 'stl'
    if cn.startswith('cFile'): return 'fileio'
    return 'other'
fam = collections.defaultdict(lambda: dict(classes=set(), funcs=0, bytes=0, vslots=0))
per_class = collections.defaultdict(lambda: dict(funcs=0, bytes=0))
for f, (cn, dp) in owner.items():
    fm = family(cn); fam[fm]['classes'].add(cn); fam[fm]['funcs'] += 1; fam[fm]['bytes'] += size[f]
    per_class[cn]['funcs'] += 1; per_class[cn]['bytes'] += size[f]
for cn, lst in vt_of_class.items(): fam[family(cn)]['vslots'] += max(lst)
allclasses = collections.Counter(family(demangle(tds[cols[c]['td']])) for c in cols)
R['families'] = {k: dict(classes=len(v['classes']), funcs=v['funcs'], bytes=v['bytes'], vslots_max_sum=v['vslots']) for k, v in fam.items()}
R['class_counts_by_family_all_cols'] = dict(allclasses)
pc = {k: v for k, v in per_class.items() if k.startswith('cParticleSystem')}
pb = sorted(v['bytes'] for v in pc.values()); pf = sorted(v['funcs'] for v in pc.values())
R['particles_per_class'] = dict(n=len(pc), bytes_median=statistics.median(pb), bytes_mean=statistics.mean(pb), bytes_p90=pb[int(len(pb) * 0.9)], bytes_max=max(pb), funcs_median=statistics.median(pf), funcs_max=max(pf),
                                top=sorted(((v['bytes'], v['funcs'], k) for k, v in pc.items()), reverse=True)[:8])
uc = {k: v for k, v in per_class.items() if k.startswith('cUI_')}
ub = sorted(v['bytes'] for v in uc.values())
R['ui_per_class'] = dict(n=len(uc), bytes_median=statistics.median(ub), bytes_mean=statistics.mean(ub), bytes_max=max(ub), top=sorted(((v['bytes'], v['funcs'], k) for k, v in uc.items()), reverse=True)[:12])
cc = {k: v for k, v in per_class.items() if family(k) == 'ui_commands'}
R['ui_commands'] = dict(n=len(cc), funcs=sum(v['funcs'] for v in cc.values()), bytes=sum(v['bytes'] for v in cc.values()))
# how many cParticleSystem classes have exactly the same vslot count / near-identical function sizes
vs = collections.Counter(max(vt_of_class[k]) for k in pc)
R['particles_vslot_hist'] = dict(vs.most_common(6))

# ---- log-string class references (byte scan for pointers to "Class::method" strings) ----
strs = {}
for m in re.finditer(rb'[A-Za-z_][A-Za-z0-9_]{1,40}::~?[A-Za-z_][A-Za-z0-9_]{1,60}', bytes(img)):
    va = base + m.start()
    if sec_of(va) in ('.rdata', '.data'):
        cls = m.group(0).split(b'::')[0].decode()
        strs[va] = cls
pat = re.compile(b'|'.join(re.escape(struct.pack('<I', va)) for va in strs))
code = bytes(img[TS - base:TE - base]); refs = collections.defaultdict(set)
for m in pat.finditer(code):
    va = struct.unpack('<I', m.group(0))[0]; f = func_of(TS + m.start())
    if f is not None: refs[strs[va]].add(f)
R['logstring_classes_funcs'] = {k: (len(v), sum(size[f] for f in v)) for k, v in sorted(refs.items(), key=lambda kv: -len(kv[1]))[:40]}
def slice_of(prefixes):
    fs = set()
    for k, v in refs.items():
        if k.startswith(prefixes): fs |= v
    return dict(funcs=len(fs), bytes=sum(size[f] for f in fs))
R['slice_audio_cMSS'] = slice_of(('cMSS',)); R['slice_anim_cGranny'] = slice_of(('cGranny', 'cGrn')); R['slice_render_strings'] = slice_of(('dxDriver7', 'dxDisplay', 'cDx', 'cDX', 'cTexture', 'cFont'))
R['slice_engine_core'] = slice_of(('cEngine', 'cKernel', 'cWorld', 'cObjectManager', 'cSector'))

# ---- module table (own functions only; tail and thunks removed) ----
mod = collections.defaultdict(lambda: dict(funcs=0, bytes=0, simple=0, complex=0, direct=0))
for s in own:
    L = lab[s]; m = mod[L]; m['funcs'] += 1; m['bytes'] += size[s]
    if size[s] <= 256: m['simple'] += 1
    else: m['complex'] += 1
    if direct[s]: m['direct'] += 1
tailmod = collections.defaultdict(lambda: dict(funcs=0, bytes=0))
for s in starts:
    if s >= TAIL: tailmod[lab[s]]['funcs'] += 1; tailmod[lab[s]]['bytes'] += size[s]
thmod = collections.Counter(lab[s] for s in thunks)
R['modules_own'] = dict(mod); R['modules_tail'] = dict(tailmod); R['thunks_by_label'] = dict(thmod)
# server overlap from fn_match
if args.fn_match2:
    fm2 = json.load(open(args.fn_match2)); R['fn_match2_keys'] = list(fm2.keys())
    R['sacred_label_match'] = fm2['sacred_label_match']
# ghidra-vs-A per module (game region)
gm = collections.Counter(); am = collections.Counter()
for s in own:
    am[lab[s]] += 1
    if s in Gf: gm[lab[s]] += 1
R['ghidra_coverage_by_module'] = {k: (gm[k], am[k]) for k in am}
json.dump(R, open(args.out, 'w', encoding='utf-8', newline=''), indent=1, default=str)
for k, v in R.items(): print(k, '=', json.dumps(v, default=str)[:1800])
