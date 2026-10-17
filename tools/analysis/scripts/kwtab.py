#!/usr/bin/env python3
"""Keyword tables of the script compiler (cScriptCompiler) in the client's .data.

Two parts:
  1. A search for an array of pointers to names + a parallel array of u32 opcodes near the anchor strings
     (MachVersteck, GiveSkill, CreateNPC, AddExp). On Sacred.exe 2.28 the result is negative (pointer hits: []):
     the table is built by code in .text, and there is no static pointer array in .data.
  2. A dump of the run of strings around the anchor (file offset 0x54CE78 − 0x300 … + 0x900): exactly 61 names of table A
     (MachVersteck … AddExp, in reverse index order) and 60 names of table B (GiveSkill … CreateNPC) in the same
     order as in openheilig, plus quest directives (ToDoBloecke:, Delay:, Dialog:, ...). With a --names TSV
     (columns opcode, name[, table, index]) the known opcode number is appended to each line.

Purpose (docs/02 §7.3; docs/re/netscript.md, "Keyword resolvers of the compiler"): the compiler's keyword tables in
.data (file offset 0x54CE78). The .text section is not read.

Examples
  python3 -I tools/analysis/scripts/kwtab.py --game-dir ./OriginalGame
  python3 -I tools/analysis/scripts/kwtab.py --game-dir ./OriginalGame --names /path/opcode-names.tsv

Expected output: "anchor offsets: [...]", "pointer hits: []" for each anchor, then "N strings between 0x54cb78
and 0x54d778" and lines "offset<TAB>name<TAB>op=...". Dependencies: pefile.
"""
import sys, struct, argparse, os, csv
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Script-compiler keyword tables in Sacred.exe data (pointer search + string runs).')
ap.add_argument('path', nargs='?', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--names', help='optional TSV: opcode<TAB>name[<TAB>table<TAB>index]')
ap.add_argument('--anchor', default='MachVersteck', help='anchor keyword for the string-run dump')
ap.add_argument('--before', type=lambda s: int(s, 0), default=0x300); ap.add_argument('--after', type=lambda s: int(s, 0), default=0x900)
args = ap.parse_args()
p = args.path or os.path.join(args.game_dir, 'Sacred.exe')
pe = pefile.PE(p, fast_load=True); base = pe.OPTIONAL_HEADER.ImageBase
data = open(p, 'rb').read()
secs = [(s.Name.rstrip(b'\0').decode(), s.VirtualAddress, s.Misc_VirtualSize, s.PointerToRawData, s.SizeOfRawData) for s in pe.sections]
def rva2off(rva):
    for n, va, vs, po, ps in secs:
        if va <= rva < va + max(vs, ps): return po + (rva - va)
    return None
def off2rva(off):
    for n, va, vs, po, ps in secs:
        if po <= off < po + ps: return va + (off - po)
    return None

# ---- part 1: pointer arrays next to anchor strings ----
anchors = [b'MachVersteck\0', b'GiveSkill\0', b'CreateNPC\0', b'AddExp\0']
found = []
for a in anchors:
    i = data.find(a)
    while i >= 0:
        found.append((a, i)); i = data.find(a, i + 1)
print('anchor offsets:', [(a, hex(o)) for a, o in found])
for a, o in found:
    rva = off2rva(o)
    if rva is None: continue
    va = base + rva
    needle = struct.pack('<I', va)
    j = data.find(needle)
    hits = []
    while j >= 0:
        hits.append(j); j = data.find(needle, j + 1)
    print(f'{a!r} @file {o:#x} va {va:#x} pointer hits: {[hex(h) for h in hits]}')
    for h in hits:
        if h % 4: continue
        # walk backwards while entries are pointers to printable strings
        def isstrptr(v):
            r = v - base; off = rva2off(r) if 0 < r < 0x2000000 else None
            if off is None: return None
            e = data.find(b'\0', off)
            s = data[off:e]
            return s.decode('latin1') if 0 < len(s) < 40 and all(32 <= c < 127 for c in s) else None
        start = h
        while start - 4 >= 0 and isstrptr(struct.unpack_from('<I', data, start - 4)[0]): start -= 4
        end = h
        while isstrptr(struct.unpack_from('<I', data, end + 4)[0] if end + 8 <= len(data) else 0): end += 4
        n = (end - start) // 4 + 1
        names = [isstrptr(struct.unpack_from('<I', data, start + 4 * k)[0]) for k in range(n)]
        nums = list(struct.unpack_from('<%dI' % (n + 3), data, end + 4))
        print(f'  array @file {start:#x}..{end:#x} rva {off2rva(start):#x}: {n} pointers; following u32s: {nums[:n + 3]}')
        # pairing: pointer i <-> number i+1 (first slot zero sentinel); otherwise i <-> i
        if nums[0] == 0:
            pairs = [(nums[k + 1], names[k]) for k in range(n)]
        else:
            pairs = [(nums[k], names[k]) for k in range(n)]
        for op, nm in sorted(pairs): print(f'    {op:4d}\t{nm}')
        break

# ---- part 2: string runs around the anchor (the actual keyword lists) ----
byname = {}
if args.names:
    for r in csv.reader(open(args.names), delimiter='\t'):
        if not r or r[0] == 'opcode' or not r[0].strip().isdigit(): continue
        op = int(r[0]); name = r[1]; tab = r[2] if len(r) > 2 else ''; idx = r[3] if len(r) > 3 else ''
        byname.setdefault(name, (op, tab, idx))
o = data.find(args.anchor.encode() + b'\0')
if o < 0:
    sys.exit('anchor %r not found' % args.anchor)
lo = o - args.before; hi = o + args.after
i = lo; run = []
while i < hi:
    j = data.find(b'\0', i)
    s = data[i:j]
    if 2 <= len(s) < 40 and all(32 <= c < 127 for c in s): run.append((i, s.decode()))
    i = j + 1
print(f'\n{len(run)} strings between {lo:#x} and {hi:#x} (file offsets, section {next((n for n, va, vs, po, ps in secs if po <= o < po + ps), "?")})')
for off, s in run:
    key = s.rstrip(':')
    info = byname.get(key) or byname.get(s)
    print(f'{off:#x}\t{s:28s}\t{"op=%d %s[%s]" % info if info else ""}')
