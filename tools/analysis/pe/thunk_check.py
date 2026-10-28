#!/usr/bin/env python3
"""Check of the EH thunks in the client's function database: function starts (pe_deep <out>.funcs.json) that begin with
"mov eax, <FuncInfo>" (magic 0x19930520), their sizes, the share with a "jmp" right after the mov, alignment, position in
the library tail; with --ghidra-tsv, additionally the composition of the tail without thunks, the library block before the
first thunk and the "Ghidra-only" starts inside functions, by module.

Purpose (docs/01-binary-analysis.md, §4.1): 2,538 EH thunks among 10,823 starts; the tail is >= 0x7F1000.

Examples
  uv run python -I tools/analysis/pe/thunk_check.py --game-dir ./OriginalGame --funcs /tmp/analysis/sacred_deep.json.funcs.json --out /tmp/analysis/thunk_check.json
  uv run python -I tools/analysis/pe/thunk_check.py --funcs ... --ghidra-tsv /path/to/functions_eng.tsv

Expected output: "starts beginning with mov eax,FuncInfo: 2538", a size histogram, "next byte after mov is E9".
The executable is read as is. Dependencies: pefile.
"""
import json, struct, collections, statistics, bisect, re, argparse, sys, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='EH thunk check over the pe_deep function base of the client.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--exe', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--tail', type=hexkey, default=0x7F1000)
ap.add_argument('--funcs', required=True, help='pe_deep <out>.funcs.json of the client')
ap.add_argument('--ghidra-tsv', help='sacred-bild functions_eng.tsv (optional, adds the join statistics)')
ap.add_argument('--out', help='optional JSON with thunk count and sizes')
args = ap.parse_args()
path = args.exe or os.path.join(args.game_dir, 'Sacred.exe')
pe = pefile.PE(path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]; TS = base + txt.VirtualAddress; TE = TS + txt.Misc_VirtualSize
funcs = json.load(open(args.funcs)); starts = [f[0] for f in funcs]; size = {f[0]: f[1] for f in funcs}; lab = {f[0]: f[2] for f in funcs}
TAIL = args.tail
# FuncInfo magic set
fi = set()
for m in re.finditer(re.escape(struct.pack('<I', 0x19930520)), bytes(img)):
    if m.start() % 4 == 0: fi.add(base + m.start())
th = [s for s in starts if img[s - base] == 0xB8 and struct.unpack_from('<I', img, s - base + 1)[0] in fi]
print('starts beginning with mov eax,FuncInfo:', len(th))
szs = [size[s] for s in th]; print('size hist', collections.Counter(min(x, 64) // 8 * 8 for x in szs).most_common(12))
print('next byte after mov is E9 (jmp):', sum(1 for s in th if img[s - base + 5] == 0xE9))
print('aligned16:', sum(1 for s in th if s % 16 == 0), ' >=TAIL:', sum(1 for s in th if s >= TAIL))
# what follows the 10-byte thunk inside the 'function'? show 3 examples with size>16
ex = [s for s in th if size[s] > 16][:5]
for s in ex: print(hex(s), size[s], bytes(img[s - base:s - base + 24]).hex())
# How many 'starts' are immediately preceded by a thunk start (i.e. thunk sits between two functions)?
ss = set(starts); print('thunk followed by another start within 10..16 bytes:', sum(1 for s in th if any((s + d) in ss for d in range(10, 17))))
if args.out:
    json.dump(dict(n=len(th), sizes=szs), open(args.out, 'w', encoding='utf-8', newline=''))
if args.ghidra_tsv:
    ths = set(th)
    print('thunk starts', len(ths), 'min %x max %x' % (min(ths), max(ths)), 'bytes incl. attached funclets', sum(size[s] for s in ths))
    print('thunk label mix', collections.Counter(lab[s] for s in ths).most_common())
    tail_non = [s for s in starts if s >= TAIL and s not in ths]
    print('tail non-thunk starts', len(tail_non), 'bytes', sum(size[s] for s in tail_non))
    print('tail non-thunk label mix (funcs,bytes):', {k: (v, sum(size[s] for s in tail_non if lab[s] == k)) for k, v in collections.Counter(lab[s] for s in tail_non).items()})
    # region between last game start and first thunk: library block
    lib = [s for s in tail_non if s < min(ths)]; print('tail library block before first thunk: funcs', len(lib), 'bytes', sum(size[s] for s in lib), 'range %x-%x' % (min(lib), max(lib)))
    after = [s for s in tail_non if s > max(ths)]; print('non-thunk starts after last thunk', len(after), sum(size[s] for s in after))
    # Ghidra-only starts inside A functions by module (game region)
    G = set()
    for line in open(args.ghidra_tsv):
        if line.startswith('#') or not line.strip(): continue
        a, nm = line.rstrip('\n').split('\t')[:2]
        if not nm.startswith(('Catch@', 'Unwind@')): G.add(int(a, 16))
    A = set(starts); onlyG = [a for a in G - A if TS <= a < TAIL]
    def func_of(a):
        i = bisect.bisect_right(starts, a) - 1; return starts[i]
    print('ghidra-only game-region starts', len(onlyG), 'by host module', collections.Counter(lab[func_of(a)] for a in onlyG).most_common())
    own = [s for s in starts if s < TAIL]; print('own union = A(game) %d + ghidra-only %d = %d ; of which stl/crt-labelled %d' % (len(own), len(onlyG), len(own) + len(onlyG), sum(1 for s in own if lab[s] in ('stl', 'crt'))))
