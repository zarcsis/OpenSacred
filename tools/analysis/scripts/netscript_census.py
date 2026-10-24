#!/usr/bin/env python3
"""NetScript opcode census over all bin/**/{FunkCode,StartCode,QuestCode,QuestPoolCode}.bin: record format
(u16 opcode, u16 length, payload), grouping by tree (Base/Addon × Camp/MP), classes of byte-identical files,
a histogram over opcodes 0..140 with the first/last payload tag and the number of distinct lengths; optionally,
opcode names from an external TSV (columns opcode, name; e.g. the openheilig table, which is not copied into the repository).

Purpose (docs/02 §7.2–7.3): 141 record types, 113 occur, 45 cover 95%, 68 cover 99%; 20 directories; bin/TYPE_NPC_*×8
+ NetScriptCamp = the Sacred campaign, bin/Addon/* = Underworld.

Example
  python3 -I tools/analysis/scripts/netscript_census.py --game-dir ./OriginalGame --out /tmp/analysis/netscript_opcodes.tsv
  python3 -I tools/analysis/scripts/netscript_census.py --game-dir ./OriginalGame --names /path/opcode-names.tsv --out ...

Expected output: a table of files (bytes, instrs, distinct_ops, leftover, md5), classes of identical FunkCode.bin files
(9 identical Addon trees), "union of opcodes observed in any shipped file: 113". Dependencies: standard library.
"""
import sys, os, struct, hashlib, collections, csv, argparse

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='NetScript opcode census over bin/**/*Code.bin.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--names', help='optional TSV with columns opcode<TAB>name (first row may be a header)')
ap.add_argument('--out', required=True, help='output TSV')
args = ap.parse_args()
root = args.game_dir; out = args.out
names = {}
if args.names:
    for r in csv.reader(open(args.names), delimiter='\t'):
        if r and r[0] != 'opcode' and r[0].strip().isdigit(): names.setdefault(int(r[0]), r[1])
def parse(d):
    ins = []; off = 0
    while off + 4 <= len(d):
        op, ln = struct.unpack_from('<HH', d, off)
        if ln < 4 or off + ln > len(d): break
        ins.append((op, d[off + 4:off + ln])); off += ln
    return ins, off, len(d) - off
dirs = sorted(set(os.path.dirname(os.path.join(dp, f)) for dp, dn, fn in os.walk(os.path.join(root, 'bin')) for f in fn if f.lower() == 'funkcode.bin'))
def group(dpath):
    rel = os.path.relpath(dpath, root).replace('\\', '/')
    addon = 'Addon/' in rel
    leaf = rel.split('/')[-1]
    kind = 'MP' if leaf == 'NetScript' else 'Camp'
    return ('Addon' if addon else 'Base') + '-' + kind, rel, leaf
G = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))  # group -> file -> Counter(op)
FT = collections.defaultdict(collections.Counter); LT = collections.defaultdict(collections.Counter)  # first/last tag per op
LENS = collections.defaultdict(set)
md5s = {}
rows = []
for dpath in dirs:
    g, rel, leaf = group(dpath)
    for fn in ['FunkCode.bin', 'StartCode.bin', 'QuestCode.bin', 'QuestPoolCode.bin']:
        fp = os.path.join(dpath, fn)
        if not os.path.exists(fp): continue
        d = open(fp, 'rb').read(); h = hashlib.md5(d).hexdigest()[:10]; md5s[(rel, fn)] = h
        ins, cons, left = parse(d)
        c = collections.Counter(op for op, _ in ins)
        for op, pl in ins:
            G[g][fn][op] += 1
            if pl: FT[op][pl[0]] += 1; LT[op][pl[-1]] += 1
            LENS[op].add(len(pl) + 4)
        rows.append((rel, fn, len(d), len(ins), len(c), left, h))
print('file\tbytes\tinstrs\tdistinct_ops\tleftover\tmd5')
for r in rows: print('\t'.join(map(str, r)))
# identical-file classes
cls = collections.defaultdict(list)
for (rel, fn), h in md5s.items(): cls[(fn, h)].append(rel)
print('\n# identical-content classes (FunkCode):')
for (fn, h), rels in sorted(cls.items()):
    if fn == 'FunkCode.bin': print(f'  {h}: {len(rels)} dirs: {rels}')
groups = sorted(G)
allops = sorted(set(op for g in G for fn in G[g] for op in G[g][fn]))
with open(out, 'w', encoding='utf-8', newline='') as f:
    hdr = ['op', 'hex', 'name'] + [f'{g}:{fn[:-8]}' for g in groups for fn in ['FunkCode.bin', 'StartCode.bin', 'QuestCode.bin']] + ['total', 'needed_by', 'n_lens', 'top_first_tag', 'top_last_tag']
    f.write('\t'.join(hdr) + '\n')
    for op in range(0, 141):
        cells = []; tot = 0; need = []
        for g in groups:
            for fn in ['FunkCode.bin', 'StartCode.bin', 'QuestCode.bin']:
                n = G[g][fn][op]; cells.append(str(n)); tot += n
                if n: need.append(g)
        ft = ','.join(f'{t:#04x}' for t, _ in FT[op].most_common(2)); lt = ','.join(f'{t:#04x}' for t, _ in LT[op].most_common(2))
        f.write('\t'.join([str(op), f'{op:#04x}', names.get(op, '')] + cells + [str(tot), '|'.join(sorted(set(need))), str(len(LENS[op])), ft, lt]) + '\n')
print('\n# distinct opcodes per group/file:')
for g in groups:
    for fn in sorted(G[g]): print(f'  {g:12s} {fn:18s} distinct={len(G[g][fn]):3d} instrs={sum(G[g][fn].values())}')
union = set(op for g in G for fn in G[g] for op in G[g][fn])
print(f'\n# union of opcodes observed in any shipped file: {len(union)} -> {sorted(union)}')
print(f'# never observed (0..140): {sorted(set(range(141)) - union)}')
if names:
    unnamed = [op for op in union if op not in names]; print(f'# observed but unnamed in the names table: {unnamed}')
base = set(op for fn in G['Base-Camp'] for op in G['Base-Camp'][fn]); add = set(op for fn in G['Addon-Camp'] for op in G['Addon-Camp'][fn]); mp = set(op for g in ('Base-MP', 'Addon-MP') for fn in G[g] for op in G[g][fn])
print(f'# Base-Camp {len(base)}  Addon-Camp {len(add)}  MP {len(mp)}  Camp-union {len(base | add)}  only-in-MP {sorted(mp - (base | add))}  only-in-Addon {sorted(add - base)} only-in-Base {sorted(base - add)}')
