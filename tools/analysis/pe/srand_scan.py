#!/usr/bin/env python3
"""Search for CRT srand() (multithreaded CRT: call _getptd; mov [eax+0x14], arg) and its callers with context;
additionally, the callers of given functions (e.g. _control87) with the immediate operands of their pushes.

Purpose (docs/01 §7: random numbers and x87 precision)
  Shows where the game seeds the RNG (srand(time) ×9, deterministic seeds of the name generator, etc.) and where
  _control87/_controlfp is called from.

Examples
  uv run python -I tools/analysis/pe/srand_scan.py OriginalGame/Sacred.exe --getptd 0x84ec63 --game-end 0x7F1000 --ctl 0x84d7e5
  uv run python -I tools/analysis/pe/srand_scan.py OriginalGame/GameServer.exe --getptd 0x514953 --game-end 0x4E1000 --ctl 0x517603 --ctl 0x51c8b4

The _getptd/_control87 addresses come from the output of fpu_scan.py (its part 2, "functions containing fldcw") and
detail_scan.py --rand.
Expected output: "srand candidate: entry 0x... callers game=N lib=M" and a list of callers with the 9 preceding instructions.

Dependencies: pefile, capstone.
"""
import sys, struct, argparse, collections, bisect, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Find CRT srand() and callers of given functions (e.g. _control87).')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--getptd', type=hexkey, help='VA of CRT _getptd'); ap.add_argument('--game-end', type=hexkey, default=0)
ap.add_argument('--ctl', action='append', default=[], help='VA of _control87-like function: list callers + pushed immediates')
ap.add_argument('--all-exec', action='store_true', help='disassemble all executable sections (DLLs)')
args = ap.parse_args()
args.path = args.path or os.path.join(args.game_dir, 'Sacred.exe')
pe = pefile.PE(args.path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
secs = [s for s in pe.sections if (s.Characteristics & 0x20000000) or s.Name.startswith(b'.text')]
if not args.all_exec: secs = [s for s in secs if s.Name.startswith(b'.text')][:1]
iat = {}
try:
    for imp in pe.DIRECTORY_ENTRY_IMPORT:
        for i in imp.imports: iat[i.address] = '%s!%s' % (imp.dll.decode(), i.name.decode() if i.name else 'ord%d' % i.ordinal)
except AttributeError: pass
secrng = [(base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData), s.Name.rstrip(b'\0').decode()) for s in pe.sections]
def sec_of(v):
    for a, b, n in secrng:
        if a <= v < b: return n
def str_at(v):
    off = v - base
    if off <= 0 or off >= len(img): return None
    end = off
    while end < len(img) and 32 <= img[end] < 127 and end - off < 120: end += 1
    if end - off >= 4 and img[end] == 0: return bytes(img[off:end]).decode('ascii', 'replace')
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
ins_list = []
for s in secs:
    TS = base + s.VirtualAddress; TE = TS + s.Misc_VirtualSize
    ins_list += [i for i in md.disasm(bytes(img[TS - base:TE - base]), TS) if i.id != 0]
idx = {i.address: k for k, i in enumerate(ins_list)}
calls = collections.defaultdict(list)
for i in ins_list:
    if i.id == X86_INS_CALL and i.operands[0].type == X86_OP_IMM: calls[i.operands[0].imm].append(i.address)
tsorted = sorted(calls)
def func_entry(a):
    k = bisect.bisect_right(tsorted, a) - 1
    return tsorted[k] if k >= 0 and a - tsorted[k] < 0x800 else None
def annot(ins):
    notes = []
    for op in ins.operands:
        v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
        if v is None: continue
        if v in iat: notes.append(iat[v])
        elif sec_of(v) in ('.rdata', '.data'):
            s = str_at(v)
            if s: notes.append('"%s"' % s)
    return ' | '.join(notes)
def ctx(c, n=9):
    k = idx[c]; return ' ; '.join(x.mnemonic + ' ' + x.op_str + ((' <' + annot(x) + '>') if annot(x) else '') for x in ins_list[max(0, k - n):k])
def split(cs): return [c for c in cs if c < args.game_end], [c for c in cs if c >= args.game_end]
if args.getptd:
    print("callers of _getptd 0x%x: %d" % (args.getptd, len(calls[args.getptd])))
    for c in calls[args.getptd]:
        k = idx[c]
        # srand: within 6 instrs a write to [eax+0x14]
        for x in ins_list[k + 1:k + 7]:
            if x.id == X86_INS_MOV and x.operands[0].type == X86_OP_MEM and x.operands[0].mem.base == X86_REG_EAX and x.operands[0].mem.disp == 0x14:
                e = func_entry(c) or c
                # entry may be the call itself if it is the first instruction
                if c in calls: e = c
                g, l = split(calls.get(e, []))
                print("srand candidate: entry 0x%x (write at 0x%x '%s %s'), callers game=%d lib=%d" % (e, x.address, x.mnemonic, x.op_str, len(g), len(l)))
                for cc in (g + l)[:40]:
                    print("   caller 0x%x [%s] func~0x%x: %s" % (cc, 'GAME' if cc < args.game_end else 'lib', func_entry(cc) or 0, ctx(cc)))
                break
for v in args.ctl:
    t = int(v, 16); cs = calls.get(t, [])
    print("\ncallers of 0x%x: %d" % (t, len(cs)))
    for c in cs[:60]:
        k = idx[c]; pushes = [x for x in ins_list[max(0, k - 8):k] if x.id == X86_INS_PUSH]
        imms = [hex(x.operands[0].imm & 0xffffffff) if x.operands[0].type == X86_OP_IMM else x.op_str for x in pushes[-2:]]
        print("   0x%x func~0x%x pushes=%s | %s" % (c, func_entry(c) or 0, imms, ctx(c, 6)))
