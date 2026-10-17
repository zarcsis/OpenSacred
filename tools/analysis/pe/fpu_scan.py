#!/usr/bin/env python3
"""FPU / RNG / VirtualProtect scan for MSVC6 PE32 files (GameServer.exe and the GOG client Sacred.exe, read as is).

Purpose (docs/01 §7: x87 precision, rounding, random numbers, hero data protection)
  1. All fnstcw/fldcw/fninit/fclex/ldmxcsr/stmxcsr sites, split into "game code" (< --game-end) / CRT tail;
     for fldcw, the control word value (PC=24/53/64 bits, RC) if it can be recovered from the operand
     or from a preceding mov [slot], imm.
  2. For each function with fldcw (candidates _ftol/_control87/_fpreset/CRT math): its entry and the number of calls
     from game code and from libraries.
  3. RNG constants (MSVC rand 214013/2531011, MT19937, Park–Miller, ...) as imm in code and as raw LE dwords.
  4. VirtualProtect call sites with recovered push arguments (lpAddress, dwSize, flNewProtect) and section.

Examples
  python3 -I tools/analysis/pe/fpu_scan.py OriginalGame/Sacred.exe --game-end 0x7F1000
  python3 -I tools/analysis/pe/fpu_scan.py OriginalGame/GameServer.exe --game-end 0x4E1000

Expected output (client): 3 _controlfp(_PC_24) sites in the main thread, 2,816 _ftol calls, 0x343FD/0x269EC3
in CRT rand(), 72 VirtualProtect sites in one function (anti-cheat page). A full disassembly of the client takes ~1–3 min.

Dependencies: pefile, capstone.
"""
import sys, struct, argparse, collections, bisect, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='FPU control-word, RNG constant and VirtualProtect scan (MSVC6 PE32).')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--game-end', type=hexkey, required=True, help='first VA of CRT/library tail (client 0x7F1000, server 0x4E1000)')
ap.add_argument('--vp-max', type=int, default=200)
ap.add_argument('--no-vp', action='store_true')
args = ap.parse_args()
args.path = args.path or os.path.join(args.game_dir, 'Sacred.exe')

pe = pefile.PE(args.path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
TS, TE = base + txt.VirtualAddress, base + txt.VirtualAddress + txt.Misc_VirtualSize
secrng = [(base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData), s.Name.rstrip(b'\0').decode()) for s in pe.sections]
def sec_of(v):
    for a, b, n in secrng:
        if a <= v < b: return n
    return None
iat = {}
for imp in pe.DIRECTORY_ENTRY_IMPORT:
    for i in imp.imports: iat[i.address] = '%s!%s' % (imp.dll.decode(), i.name.decode() if i.name else 'ord%d' % i.ordinal)
def rd16(va):
    o = va - base
    return struct.unpack_from('<H', img, o)[0] if 0 <= o < len(img) - 1 else None
def rd32(va):
    o = va - base
    return struct.unpack_from('<I', img, o)[0] if 0 <= o < len(img) - 3 else None
def str_at(v):
    off = v - base
    if off <= 0 or off >= len(img): return None
    end = off
    while end < len(img) and 32 <= img[end] < 127 and end - off < 200: end += 1
    if end - off >= 4 and img[end] == 0: return bytes(img[off:end]).decode('ascii', 'replace')
    return None

md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
code = bytes(img[TS - base:TE - base])
print("disassembling %s .text 0x%x..0x%x (%d KB), game/lib boundary 0x%x" % (args.path.split('/')[-1], TS, TE, len(code) // 1024, args.game_end))
ins_list = list(md.disasm(code, TS))
ins_list = [i for i in ins_list if i.id != 0]
addr_idx = {i.address: k for k, i in enumerate(ins_list)}
addrs = [i.address for i in ins_list]

# call graph
call_targets = collections.defaultdict(list)
for ins in ins_list:
    if ins.id == X86_INS_CALL and ins.operands[0].type == X86_OP_IMM:
        call_targets[ins.operands[0].imm].append(ins.address)
target_sorted = sorted(call_targets)
def func_entry(a):
    """nearest call target at or before a (within 0x600)."""
    k = bisect.bisect_right(target_sorted, a) - 1
    if k >= 0 and a - target_sorted[k] < 0x600: return target_sorted[k]
    return None
def split(callers):
    g = [c for c in callers if c < args.game_end]; l = [c for c in callers if c >= args.game_end]
    return g, l

# ---------- 1. control word sites ----------
CW_MN = {'fldcw', 'fnstcw', 'fstcw', 'fninit', 'finit', 'fnclex', 'fclex', 'ldmxcsr', 'stmxcsr', 'fnstenv', 'fldenv', 'fnsave', 'frstor', 'fxsave', 'fxrstor'}
cnt_game = collections.Counter(); cnt_lib = collections.Counter()
sites = []
PC = {0: '24bit', 1: 'reserved', 2: '53bit', 3: '64bit'}
RC = {0: 'nearest', 1: 'down', 2: 'up', 3: 'trunc'}
def describe_cw(v):
    return 'cw=0x%04x PC=%s RC=%s mask=0x%02x' % (v, PC[(v >> 8) & 3], RC[(v >> 10) & 3], v & 0x3f)
for k, ins in enumerate(ins_list):
    if ins.mnemonic not in CW_MN: continue
    (cnt_game if ins.address < args.game_end else cnt_lib)[ins.mnemonic] += 1
    if ins.mnemonic not in ('fldcw', 'ldmxcsr', 'fninit', 'finit'): continue
    note = ''
    op = ins.operands[0] if ins.operands else None
    if op is not None and op.type == X86_OP_MEM:
        if op.mem.base == 0 and op.mem.index == 0:
            v = rd16(op.mem.disp); note = 'abs [0x%x] -> %s' % (op.mem.disp, describe_cw(v) if v is not None else '?')
        else:
            # look back for 'mov word ptr [same slot], imm' or 'mov ax/bx, imm; mov [slot], ax'
            slot = (op.mem.base, op.mem.index, op.mem.disp)
            imm = None; regval = {}
            for p in ins_list[max(0, k - 14):k][::-1]:
                if p.id == X86_INS_MOV and len(p.operands) == 2:
                    d, s = p.operands
                    if s.type == X86_OP_IMM and d.type == X86_OP_REG: regval.setdefault(d.reg, s.imm)
                    if d.type == X86_OP_MEM and (d.mem.base, d.mem.index, d.mem.disp) == slot:
                        if s.type == X86_OP_IMM: imm = s.imm; break
                        if s.type == X86_OP_REG and s.reg in regval: imm = regval[s.reg]; break
                        break
                if p.id in (X86_INS_OR, X86_INS_AND) and p.operands[0].type == X86_OP_MEM and (p.operands[0].mem.base, p.operands[0].mem.index, p.operands[0].mem.disp) == slot:
                    note = 'rmw %s %s' % (p.mnemonic, p.op_str); break
            if imm is not None: note = 'imm -> ' + describe_cw(imm & 0xffff)
            if not note:
                ctx = ' ; '.join(x.mnemonic + ' ' + x.op_str for x in ins_list[max(0, k - 6):k])
                note = 'ctx: ' + ctx
    sites.append((ins.address, ins.mnemonic, ins.op_str, note))
print("\n== 1. control-word instruction counts ==")
print("game code (<0x%x):" % args.game_end, dict(cnt_game))
print("CRT/lib tail:", dict(cnt_lib))
print("-- fldcw/ldmxcsr/fninit sites --")
for a, m, o, n in sites:
    print("  %s 0x%08x %-7s %-28s %s" % ('GAME' if a < args.game_end else 'lib ', a, m, o, n))

# ---------- 2. functions containing fldcw: callers ----------
print("\n== 2. functions containing fldcw (entries by nearest call target) and their callers ==")
funcs = collections.OrderedDict()
for a, m, o, n in sites:
    if m != 'fldcw': continue
    e = func_entry(a)
    funcs.setdefault(e, []).append(a)
for e, sl in funcs.items():
    if e is None:
        print("  (no entry found) sites:", [hex(x) for x in sl]); continue
    g, l = split(call_targets.get(e, []))
    # signature hint: first few instructions
    k = addr_idx.get(e); head = ' ; '.join(x.mnemonic + ' ' + x.op_str for x in ins_list[k:k + 5]) if k is not None else '?'
    tag = ''
    body = ins_list[k:k + 12] if k is not None else []
    mn = [x.mnemonic for x in body]
    if 'fnstcw' in mn and 'fistp' in mn: tag = '<-- _ftol pattern'
    print("  entry 0x%08x  callers game=%d lib=%d  fldcw sites=%s %s\n      head: %s" % (e, len(g), len(l), [hex(x) for x in sl], tag, head))
    if g and len(g) <= 12:
        print("      game callers:", [hex(x) for x in g])

# ---------- 3. RNG constants ----------
print("\n== 3. RNG constants ==")
RNG = {0x343FD: 'MSVC rand mul 214013', 0x269EC3: 'MSVC rand add 2531011', 0x9908B0DF: 'MT19937 matrix_a', 0x6C078965: 'MT19937 init mul 1812433253',
       0x41C64E6D: 'glibc/ANSI LCG mul 1103515245', 0x3039: 'LCG add 12345', 0x5BD1E995: 'MurmurHash2', 0x8088405: 'Delphi/Borland LCG 134775813',
       0x2545F491: 'xorshift mul', 0x7FFFFFFF: 'mask 2^31-1 (Park-Miller mod)', 0x41A7: 'Park-Miller mul 16807', 0xBB40E64E: 'Lehmer/other 3141592621',
       0x1F31D: 'LCG 127773 (Schrage)', 0xAEC9: 'Park-Miller  (unused)', 0x19660D: 'Numerical Recipes LCG mul 1664525', 0x3C6EF35F: 'NR LCG add 1013904223',
       0x9E3779B9: 'golden ratio (TEA/hash)', 0x2545F4914F6CDD1D & 0xffffffff: 'xorshift64* low', 0x5851F42D & 0xffffffff: 'PCG mul hi',
       0x4C957F2D: 'PCG mul lo', 0x3B9ACA07: '999999991 (name hash mod, Sacred)', 0xFFFF7FFF: 'cw mask?', 0x10DCD: 'RANDU-like 69069', 0x7FFFFFED: 'prime 2^31-19'}
imm_hits = collections.defaultdict(list)
for ins in ins_list:
    for op in ins.operands:
        v = None
        if op.type == X86_OP_IMM: v = op.imm & 0xffffffff
        elif op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.index == 0: continue
        if v in RNG: imm_hits[v].append((ins.address, ins.mnemonic + ' ' + ins.op_str))
for v, name in RNG.items():
    hits = imm_hits.get(v, [])
    if not hits: continue
    g, l = split([h[0] for h in hits])
    print("  0x%x (%s): %d code hits (game %d / lib %d)" % (v, name, len(hits), len(g), len(l)))
    for a, t in hits[:16]:
        e = func_entry(a)
        print("      0x%08x [%s] %s   (func ~0x%x, callers=%d)" % (a, 'GAME' if a < args.game_end else 'lib', t, e or 0, len(call_targets.get(e, []))))
# raw byte hits (data tables, e.g. MT constants in .rdata)
raw = bytes(img)
for v, name in RNG.items():
    if v in (0x7FFFFFFF, 0x3039, 0xFFFF7FFF, 0x41A7): continue
    b = struct.pack('<I', v); n = raw.count(b)
    if n: print("  raw LE hits of 0x%x (%s): %d  sections: %s" % (v, name, n, collections.Counter(sec_of(base + raw.find(b, i)) for i in [0] * 0) or '-'))

# ---------- 4. VirtualProtect ----------
if not args.no_vp:
    print("\n== 4. VirtualProtect call sites ==")
    vp_iat = [a for a, n in iat.items() if n.endswith('!VirtualProtect')]
    PROT = {0x01: 'NOACCESS', 0x02: 'READONLY', 0x04: 'READWRITE', 0x08: 'WRITECOPY', 0x10: 'EXECUTE', 0x20: 'EXECUTE_READ', 0x40: 'EXECUTE_READWRITE', 0x80: 'EXECUTE_WRITECOPY'}
    n = 0; classes = collections.Counter(); per_func = collections.defaultdict(list)
    for k, ins in enumerate(ins_list):
        if ins.id != X86_INS_CALL: continue
        op = ins.operands[0]
        if not (op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.disp in vp_iat): continue
        n += 1
        # collect last 4 pushes (stack order: lpflOldProtect pushed first ... lpAddress pushed last)
        pushes = []
        regs = {}
        for p in ins_list[max(0, k - 25):k][::-1]:
            if p.id == X86_INS_PUSH:
                o = p.operands[0]
                if o.type == X86_OP_IMM: pushes.append(('imm', o.imm & 0xffffffff))
                elif o.type == X86_OP_REG:
                    pushes.append(('reg', p.reg_name(o.reg), regs.get(o.reg)))
                else: pushes.append(('mem', p.op_str))
                if len(pushes) == 4: break
            elif p.id in (X86_INS_MOV, X86_INS_LEA) and p.operands[0].type == X86_OP_REG:
                d, s = p.operands
                if s.type == X86_OP_IMM and d.reg not in regs: regs[d.reg] = ('imm', s.imm & 0xffffffff)
                elif s.type == X86_OP_MEM and d.reg not in regs: regs[d.reg] = ('mem', p.op_str)
            elif p.id == X86_INS_CALL: break
        pushes = pushes[::-1]  # now [lpflOld, flNewProtect, dwSize, lpAddress] if 4 found
        def fmt(x):
            if x is None: return '?'
            if x[0] == 'imm':
                s = sec_of(x[1]); st = str_at(x[1]) if s in ('.rdata', '.data') else None
                return '0x%x%s%s' % (x[1], ' (%s)' % s if s else '', ' "%s"' % st if st else '')
            if x[0] == 'reg': return '%s=%s' % (x[1], fmt(x[2]) if x[2] else '?')
            return x[1]
        if len(pushes) == 4:
            old, prot, size, addr = pushes
        else:
            old = prot = size = addr = None
        prot_s = PROT.get(prot[1], hex(prot[1])) if prot and prot[0] == 'imm' else fmt(prot)
        e = func_entry(ins.address)
        # strings referenced in the surrounding function (±0x300)
        strs = []
        for q in ins_list[max(0, k - 120):k + 120]:
            for o in q.operands:
                v = o.imm if o.type == X86_OP_IMM else (o.mem.disp if o.type == X86_OP_MEM else None)
                if v and sec_of(v) in ('.rdata', '.data'):
                    s = str_at(v)
                    if s and s not in strs: strs.append(s)
        per_func[e].append(ins.address)
        print("  #%02d 0x%08x [%s] func~0x%x  lpAddress=%s dwSize=%s prot=%s old=%s" % (n, ins.address, 'GAME' if ins.address < args.game_end else 'lib', e or 0, fmt(addr), fmt(size), prot_s, fmt(old)))
        if strs: print("        strings nearby:", strs[:8])
        if n >= args.vp_max: break
    print("  total VirtualProtect call sites: %d in %d functions" % (n, len(per_func)))
    for e, l in per_func.items():
        print("    func 0x%x: %d sites, callers of func: %d (game %d)" % (e or 0, len(l), len(call_targets.get(e, [])), len(split(call_targets.get(e, []))[0])))
