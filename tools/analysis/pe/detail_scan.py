#!/usr/bin/env python3
"""Targeted code reports after one full disassembly (a second pass on top of fpu_scan.py).

Modes (can be combined)
  --rand VA        CRT rand() entry: game/lib callers, the seed slot and its writers (srand candidates), the usage
                   pattern after the call (% n / fild / & mask / cmp)
  --callers VA     game/lib callers and the top calling functions
  --check VA ...   whether VA lies inside a jump table (neighboring aligned dwords are .text addresses)
  --range A:B      instruction listing of a range with annotations (imports, strings, sub_xxx with call counts)
  --vp             for each VirtualProtect: the instruction that sets lpAddress, and a summary between paired calls
  --threads [--beginthread VA]  CreateThread/_beginthreadex sites with push arguments (thread entry points)
  --refs VA        instructions that carry VA as an immediate operand (not call)
  --writers VA     instructions that write to the global [VA], with the 12 preceding ones
  --strfunc SUBSTR functions that reference a string: a summary of x87/operand widths + listing

Purpose: the evidence for the threads, rounding, random numbers and VirtualProtect facts of docs/01 §7, and dumps
of render ranges (dxDriver7::init, cWorldView).

Example
  uv run python -I tools/analysis/pe/detail_scan.py OriginalGame/Sacred.exe --game-end 0x7F1000 \\
      --rand 0x849b40 --callers 0x84a1f4 --check 0x68eb05 0x6a0241 --vp --threads --beginthread 0x84bb9d \\
      --range 0x817b30:0x817fb0 --writers 0x182ee5c --strfunc "OBJECT_SET_POS/FLY_STOP"

Expected output (client): rand() callers game=1828; 4 engine threads; 72 VirtualProtect sites (36 pairs).
Dependencies: pefile, capstone.
"""
import sys, struct, argparse, collections, bisect, re, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Targeted code reports after one full disassembly (MSVC6 PE32).')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--game-end', type=hexkey, required=True)
ap.add_argument('--rand', type=hexkey); ap.add_argument('--check', nargs='*', default=[])
ap.add_argument('--range', action='append', default=[]); ap.add_argument('--vp', action='store_true')
ap.add_argument('--callers', action='append', default=[], help='VA: list callers split game/lib')
ap.add_argument('--refs', action='append', default=[], help='VA: instructions carrying this immediate (push/mov), e.g. thread entry points')
ap.add_argument('--writers', action='append', default=[], help='abs VA: instructions writing this global, with 10 preceding instructions')
ap.add_argument('--beginthread', action='append', default=[], help='VA of CRT _beginthreadex wrapper: treat its calls like CreateThread')
ap.add_argument('--threads', action='store_true', help='list CreateThread call sites with pushed immediates (thread entry points)')
ap.add_argument('--strfunc', action='append', default=[], help='substring: find referencing code, print x87/mem summary + listing of that function')
args = ap.parse_args()
args.path = args.path or os.path.join(args.game_dir, 'Sacred.exe')

pe = pefile.PE(args.path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
TS, TE = base + txt.VirtualAddress, base + txt.VirtualAddress + txt.Misc_VirtualSize
secrng = [(base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData), s.Name.rstrip(b'\0').decode()) for s in pe.sections]
def sec_of(v):
    for a, b, n in secrng:
        if a <= v < b: return n
iat = {}
for imp in pe.DIRECTORY_ENTRY_IMPORT:
    for i in imp.imports: iat[i.address] = '%s!%s' % (imp.dll.decode(), i.name.decode() if i.name else 'ord%d' % i.ordinal)
def rd32(va):
    o = va - base
    return struct.unpack_from('<I', img, o)[0] if 0 <= o < len(img) - 3 else None
def str_at(v):
    off = v - base
    if off <= 0 or off >= len(img): return None
    end = off
    while end < len(img) and 32 <= img[end] < 127 and end - off < 200: end += 1
    if end - off >= 4 and img[end] == 0: return bytes(img[off:end]).decode('ascii', 'replace')
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
ins_list = [i for i in md.disasm(bytes(img[TS - base:TE - base]), TS) if i.id != 0]
idx = {i.address: k for k, i in enumerate(ins_list)}
call_targets = collections.defaultdict(list)
for ins in ins_list:
    if ins.id == X86_INS_CALL and ins.operands[0].type == X86_OP_IMM: call_targets[ins.operands[0].imm].append(ins.address)
tsorted = sorted(call_targets)
def func_entry(a):
    k = bisect.bisect_right(tsorted, a) - 1
    return tsorted[k] if k >= 0 and a - tsorted[k] < 0x800 else None
def split(cs): return [c for c in cs if c < args.game_end], [c for c in cs if c >= args.game_end]
def annot(ins):
    notes = []
    for op in ins.operands:
        v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
        if v is None: continue
        if v in iat: notes.append(iat[v])
        elif sec_of(v) in ('.rdata', '.data'):
            s = str_at(v)
            if s: notes.append('"%s"' % s)
        elif TS <= v < TE and ins.id == X86_INS_CALL: notes.append('sub_%x(callers=%d)' % (v, len(call_targets.get(v, []))))
    return ' | '.join(notes)
def listing(a, b):
    k = bisect.bisect_left([i.address for i in ins_list], a)
    while k < len(ins_list) and ins_list[k].address < b:
        i = ins_list[k]; print("  %08x  %-7s %-36s ; %s" % (i.address, i.mnemonic, i.op_str, annot(i))); k += 1

for r in args.range:
    a, b = [int(x, 16) for x in r.split(':')]
    print("\n### range 0x%x..0x%x" % (a, b)); listing(a, b)

for v in args.callers:
    t = int(v, 16); g, l = split(call_targets.get(t, []))
    print("\n### callers of 0x%x: game=%d lib=%d" % (t, len(g), len(l)))
    funcs = collections.Counter(func_entry(c) for c in g)
    print("  distinct game functions calling it: %d; top:" % len(funcs), [(hex(f or 0), n) for f, n in funcs.most_common(10)])

if args.threads:
    ct = [a for a, n in iat.items() if n.endswith('!CreateThread') or n.endswith('!CreateRemoteThread')]
    print("\n### CreateThread call sites")
    for k, i in enumerate(ins_list):
        bt = {int(v, 16) for v in args.beginthread}
        if i.id == X86_INS_CALL and ((i.operands[0].type == X86_OP_MEM and i.operands[0].mem.base == 0 and i.operands[0].mem.disp in ct) or (i.operands[0].type == X86_OP_REG) or (i.operands[0].type == X86_OP_IMM and i.operands[0].imm in bt)):
            if i.operands[0].type == X86_OP_REG:
                # register-indirect: check a preceding mov reg, [iat CreateThread]
                ok = False
                for q in ins_list[max(0, k - 12):k]:
                    if q.id == X86_INS_MOV and q.operands[0].type == X86_OP_REG and q.operands[0].reg == i.operands[0].reg and q.operands[1].type == X86_OP_MEM and q.operands[1].mem.disp in ct: ok = True
                if not ok: continue
            pushes = [x for x in ins_list[max(0, k - 20):k] if x.id == X86_INS_PUSH][-6:]
            vals = []
            for x in pushes:
                o = x.operands[0]
                vals.append(('0x%x' % o.imm + ('(.text)' if TS <= o.imm < TE else '')) if o.type == X86_OP_IMM else x.op_str)
            print("  0x%08x func~0x%x pushes(attr,stack,START,param,flags,id)=%s" % (i.address, func_entry(i.address) or 0, vals))
for v in args.refs:
    t = int(v, 16)
    hits = [i for i in ins_list if any(op.type == X86_OP_IMM and op.imm == t for op in i.operands) and i.id != X86_INS_CALL]
    print("\n### references to 0x%x as immediate: %d" % (t, len(hits)))
    for h in hits[:10]:
        k = idx[h.address]; print("  at 0x%x: %s" % (h.address, ' ; '.join(x.mnemonic + ' ' + x.op_str + ((' <' + annot(x) + '>') if annot(x) else '') for x in ins_list[k - 3:k + 4])))
for v in args.writers:
    t = int(v, 16)
    hits = [i for i in ins_list if i.id == X86_INS_MOV and i.operands[0].type == X86_OP_MEM and i.operands[0].mem.base == 0 and i.operands[0].mem.disp == t]
    print("\n### writers of [0x%x]: %d" % (t, len(hits)))
    for h in hits[:10]:
        k = idx[h.address]; print("  at 0x%x (func~0x%x):" % (h.address, func_entry(h.address) or 0)); listing(ins_list[max(0, k - 12)].address, ins_list[k + 2].address)
for sub in args.strfunc:
    sb = sub.encode(); raw = bytes(img); want = set()
    for m in re.finditer(re.escape(sb), raw):
        st = m.start()
        while st > 0 and 32 <= img[st - 1] < 127: st -= 1
        want.add(base + st)
    sites = [i for i in ins_list if any((op.type == X86_OP_IMM and op.imm in want) or (op.type == X86_OP_MEM and op.mem.disp in want) for op in i.operands)]
    print("\n### strfunc '%s': %d code refs" % (sub, len(sites)))
    seen = set()
    for st in sites[:3]:
        e = func_entry(st.address)
        if e in seen: continue
        seen.add(e)
        if e is None: e = ((st.address - 0x300) & ~0xf)
        k = bisect.bisect_right(tsorted, e)  # next call target = rough end
        end = tsorted[k] if k < len(tsorted) else e + 0x800
        end = min(end, e + 0x1800)
        ke, kend = bisect.bisect_left([i.address for i in ins_list], e), bisect.bisect_left([i.address for i in ins_list], end)
        body = ins_list[ke:kend] if ke is not None else []
        c = collections.Counter(i.mnemonic for i in body if i.mnemonic.startswith('f'))
        ld = collections.Counter()
        for i in body:
            for op in i.operands:
                if op.type == X86_OP_MEM and op.mem.base not in (0, X86_REG_ESP, X86_REG_EBP): ld['%s%d' % ('x87' if i.mnemonic.startswith('f') else 'int', op.size)] += 1
        print("  ref at 0x%x, function ~0x%x..0x%x (%d instr): x87 mnemonics=%s ; mem operand widths=%s" % (st.address, e or 0, end, len(body), dict(c), dict(ld)))
        listing(e, min(end, e + 0x400))
if args.rand:
    t = args.rand; g, l = split(call_targets.get(t, []))
    print("\n### rand() at 0x%x: callers game=%d lib=%d" % (t, len(g), len(l)))
    print("  body:"); listing(t, t + 0x30)
    # find the seed slot: first abs memory operand read in body, or _getptd pattern
    k = idx[t]; seed = None; mt = False
    for i in ins_list[k:k + 12]:
        for op in i.operands:
            if op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.index == 0 and sec_of(op.mem.disp) == '.data': seed = op.mem.disp
        if i.id == X86_INS_CALL: mt = True
    print("  seed slot:", hex(seed) if seed else None, "| calls a helper (multithreaded CRT _getptd?):", mt)
    if seed:
        writers = [i for i in ins_list if any(op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.disp == seed for op in i.operands) and i.id == X86_INS_MOV and i.operands[0].type == X86_OP_MEM]
        print("  writers of seed slot (srand candidates):", [(hex(w.address), w.mnemonic + ' ' + w.op_str) for w in writers])
        for w in writers:
            e = func_entry(w.address); cs = call_targets.get(e, []); gg, ll = split(cs)
            print("    srand entry ~0x%x callers game=%d lib=%d -> %s" % (e or 0, len(gg), len(ll), [hex(c) for c in gg[:20]]))
            for c in gg[:20]:
                kk = idx[c]; ctx = ins_list[max(0, kk - 6):kk]
                print("      0x%x: %s" % (c, ' ; '.join(x.mnemonic + ' ' + x.op_str + (' <' + annot(x) + '>' if annot(x) else '') for x in ctx)))
    funcs = collections.Counter(func_entry(c) for c in g)
    print("  distinct game functions calling rand: %d; top-10 by call count:" % len(funcs), [(hex(f or 0), n) for f, n in funcs.most_common(10)])
    # histogram of what follows rand(): 'cdq; idiv' (rand % n) vs float conversion (fild)
    follow = collections.Counter()
    for c in g:
        kk = idx[c]; nxt = [x.mnemonic for x in ins_list[kk + 1:kk + 6]]
        if 'idiv' in nxt or 'div' in nxt: follow['% n (idiv)'] += 1
        elif 'fild' in nxt: follow['-> float (fild)'] += 1
        elif 'and' in nxt: follow['& mask'] += 1
        elif 'cmp' in nxt: follow['cmp'] += 1
        else: follow['other'] += 1
    print("  usage pattern after rand():", dict(follow))

for v in args.check:
    a = int(v, 16); al = a & ~3
    ok = 0; tot = 0
    for d in range(-16, 20, 4):
        w = rd32(al + d); tot += 1
        if w is not None and TS <= w < TE: ok += 1
    print("jump-table check 0x%x: %d/%d aligned dwords around it are .text addresses -> %s" % (a, ok, tot, 'DATA (jump/offset table)' if ok >= 6 else 'code?'))

if args.vp:
    vp_iat = [a for a, n in iat.items() if n.endswith('!VirtualProtect')]
    sites = [k for k, i in enumerate(ins_list) if i.id == X86_INS_CALL and i.operands[0].type == X86_OP_MEM and i.operands[0].mem.base == 0 and i.operands[0].mem.disp in vp_iat]
    print("\n### VirtualProtect sites: %d" % len(sites))
    REGW = (X86_INS_MOV, X86_INS_LEA, X86_INS_ADD, X86_INS_SUB, X86_INS_AND, X86_INS_OR, X86_INS_XOR, X86_INS_POP, X86_INS_SHL, X86_INS_SHR, X86_INS_IMUL)
    for n, k in enumerate(sites):
        ins = ins_list[k]
        # the 4 pushes
        pushes = []; j = k - 1
        while j > k - 40 and len(pushes) < 4:
            p = ins_list[j]
            if p.id == X86_INS_PUSH: pushes.append(j)
            j -= 1
        pushes = pushes[::-1]
        if len(pushes) < 4: print("  #%02d 0x%08x: could not find 4 pushes" % (n + 1, ins.address)); continue
        lp = ins_list[pushes[-1]]  # lpAddress pushed last
        desc = lp.op_str
        if lp.operands[0].type == X86_OP_REG:
            reg = lp.operands[0].reg
            for j in range(pushes[-1] - 1, max(0, pushes[-1] - 40), -1):
                q = ins_list[j]
                if q.id in REGW and q.operands and q.operands[0].type == X86_OP_REG and q.operands[0].reg == reg:
                    desc = '%s <- [%08x] %s %s %s' % (lp.op_str, q.address, q.mnemonic, q.op_str, annot(q)); break
                if q.id == X86_INS_CALL:
                    desc = '%s <- return value of %s %s %s' % (lp.op_str, q.mnemonic, q.op_str, annot(q)); break
        prot = ins_list[pushes[1]]
        print("  #%02d 0x%08x prot=%s  lpAddress: %s" % (n + 1, ins.address, prot.op_str, desc))
        if n < 4 or n in (16, 17, 43, 44, 71):
            print("     --- listing %08x..%08x ---" % (ins_list[max(0, k - 14)].address, ins_list[min(len(ins_list) - 1, k + 8)].address))
            listing(ins_list[max(0, k - 14)].address, ins_list[min(len(ins_list) - 1, k + 8)].address)
    # what happens between paired calls (even -> odd): summarize memory writes to [reg] where reg == lpAddress
    print("\n  between-pairs summary (instructions between site 2i and 2i+1):")
    for n in range(0, len(sites) - 1, 2):
        a, b = sites[n], sites[n + 1]
        seg = ins_list[a + 1:b]
        c = collections.Counter(i.mnemonic for i in seg)
        calls = [annot(i) for i in seg if i.id == X86_INS_CALL]
        print("   pair %02d/%02d 0x%08x..0x%08x: %d instr, calls=%s, top mnemonics=%s" % (n + 1, n + 2, ins_list[a].address, ins_list[b].address, len(seg), [x for x in calls if x][:6], c.most_common(6)))
