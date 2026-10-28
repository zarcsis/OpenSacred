#!/usr/bin/env python3
"""Call arguments of granny.dll imports in Sacred.exe: references to IAT slots are found by byte patterns
(`call [iat]`, `mov reg,[iat]`, `jmp [iat]` thunk) and each host function is disassembled from its start
(boundaries from pe_deep.py <out>.funcs.json), so that desynchronization of the linear disassembler does not hide sites.
For each site, the push arguments (strings, f32 constants, numbers) are printed in declaration order.

Purpose (docs/02 §4.3, the runtime contract): GetAnimationParameter ×175,
SetAnimationParameter ×72, GetBoneState ×61, AddControlledPose with FLT_MAX on Bip01, AttachSequence with mask 0x1FF.

Examples
  uv run python -I tools/analysis/pe/granny_args.py OriginalGame/Sacred.exe /tmp/analysis/sacred_deep.json.funcs.json
  uv run python -I tools/analysis/pe/granny_args.py OriginalGame/Sacred.exe /tmp/analysis/sacred_deep.json.funcs.json --imp _GrannyAttachSequence@32

Expected output: blocks "=== <import>: N sites" with lines "VA (fn start): push ... | push ...", and at the end the
imports with no sites found. Dependencies: pefile, capstone.
"""
import sys, struct, argparse, json, bisect, re, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Call-site arguments of granny.dll imports (per-function disassembly).')
ap.add_argument('path', nargs='?', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('funcs', nargs='?', help='pe_deep <out>.funcs.json with function starts')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--funcs-json', help='alias for the positional funcs argument')
ap.add_argument('--imp', action='append', default=[])
a = ap.parse_args()
a.path = a.path or os.path.join(a.game_dir, 'Sacred.exe')
a.funcs = a.funcs or a.funcs_json
if not a.funcs: ap.error('funcs.json (from pe_deep.py) is required')
pe = pefile.PE(a.path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
TS = base + txt.VirtualAddress; TE = TS + txt.Misc_VirtualSize
iat = {}
for imp in pe.DIRECTORY_ENTRY_IMPORT:
    if b'granny' not in imp.dll.lower(): continue
    for i in imp.imports: iat[i.address] = i.name.decode()
want = set(a.imp) if a.imp else set(iat.values())
funcs = sorted(f[0] for f in json.load(open(a.funcs)))
fstart = lambda va: funcs[bisect.bisect_right(funcs, va) - 1]
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True
code = bytes(img[TS - base:TE - base])
# find all absolute references to IAT slots in .text bytes
sites = []
for slot, name in iat.items():
    if name not in want: continue
    pat = struct.pack('<I', slot)
    p = code.find(pat)
    while p != -1:
        va = TS + p
        kind = None
        if p >= 2 and code[p - 2:p] == b'\xff\x15': kind = 'call'
        elif p >= 2 and code[p - 2] == 0x8b and (code[p - 1] & 0xc7) == 0x05: kind = 'movreg'
        elif p >= 1 and code[p - 1] == 0xa1: kind = 'moveax'
        elif p >= 2 and code[p - 2:p] == b'\xff\x25': kind = 'thunk'
        if kind: sites.append((va - (2 if kind != 'moveax' else 1), kind, name))
        p = code.find(pat, p + 1)
sites.sort()
thunks = {va: name for va, kind, name in sites if kind == 'thunk'}
def str_at(v):
    o = v - base
    if o <= 0 or o >= len(img): return None
    e = o
    while e < len(img) and 32 <= img[e] < 127 and e - o < 64: e += 1
    if e - o >= 3 and img[e] == 0: return '"%s"' % bytes(img[o:e]).decode('ascii', 'replace')
    return None
def fmt_imm(v):
    s = str_at(v)
    if s: return s
    f = struct.unpack('<f', struct.pack('<I', v & 0xffffffff))[0]
    if 1e-4 < abs(f) < 1e6: return '%d(f32 %.4g)' % (v, f)
    return '%d(0x%x)' % (v, v)
res = {}
done = set()
funcs_set = set(fstart(va) for va, kind, name in sites if kind in ('call', 'movreg', 'moveax'))
for fs in sorted(funcs_set):
    fe = funcs[bisect.bisect_right(funcs, fs)] if bisect.bisect_right(funcs, fs) < len(funcs) else TE
    fe = min(fe, TE)
    insns = list(md.disasm(bytes(img[fs - base:fe - base]), fs))
    regmap = {}
    for k, ins in enumerate(insns):
        if ins.mnemonic == 'mov' and len(ins.operands) == 2 and ins.operands[0].type == X86_OP_REG and ins.operands[1].type == X86_OP_MEM and ins.operands[1].mem.base == 0 and ins.operands[1].mem.disp in iat:
            regmap[ins.operands[0].reg] = iat[ins.operands[1].mem.disp]
        if ins.mnemonic != 'call' or len(ins.operands) != 1: continue
        op = ins.operands[0]; name = None
        if op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.index == 0 and op.mem.disp in iat: name = iat[op.mem.disp]
        elif op.type == X86_OP_IMM and op.imm in thunks: name = thunks[op.imm]
        elif op.type == X86_OP_REG and op.reg in regmap: name = regmap[op.reg]
        if name is None or name not in want: continue
        nargs = int(name.split('@')[1]) // 4
        lines = []; pushes = 0; j = k - 1
        while j >= 0 and k - j <= 24 and pushes < nargs:
            p = insns[j]
            if p.mnemonic in ('call', 'ret', 'jmp'): break
            if p.mnemonic == 'push':
                pushes += 1; o = p.operands[0]
                lines.append('push %s' % (fmt_imm(o.imm) if o.type == X86_OP_IMM else p.op_str))
            elif p.mnemonic in ('fstp', 'fst', 'fld') or (p.mnemonic == 'mov' and 'esp' in p.op_str):
                lines.append('%s %s' % (p.mnemonic, p.op_str))
            j -= 1
        res.setdefault(name, []).append((ins.address, fs, list(reversed(lines))))
for name in sorted(res, key=lambda n: -len(res[n])):
    print('=== %s: %d sites' % (name, len(res[name])))
    for addr, fs, lines in res[name]:
        print('  %08x (fn %06x): %s' % (addr, fs, ' | '.join(lines)))
missing = sorted(set(iat.values()) - set(res))
print('=== imports with no site found:', missing)
