#!/usr/bin/env python3
"""Function context: API calls, references to strings (ASCII/UTF-16) and GUIDs inside functions that call
given imports, reference given strings or call given addresses; also an annotated listing of
an arbitrary range.

Purpose (docs/01 §4.5: the WMV mechanism via amstream; also the SetCooperativeLevel flags in dxDriver7::init).
Function boundaries are estimated roughly (a 16-aligned start after padding; the end is ret + padding).

Examples
  uv run python -I tools/analysis/pe/fn_context.py OriginalGame/Sacred.exe --imp CoCreateInstance --max 4
  uv run python -I tools/analysis/pe/fn_context.py OriginalGame/Sacred.exe --str "cEngine::simThread"
  uv run python -I tools/analysis/pe/fn_context.py OriginalGame/Sacred.exe --range 0x645260:0x645E40

Expected output: "sites: N", then for each function found, lines like "VA mnemonic operands ; KERNEL32.dll!X | "string" | GUID IID_...".
Dependencies: pefile, capstone.
"""
import sys, struct, re, argparse, uuid, bisect, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Context (API calls, strings, GUIDs) of functions matching imports/strings/callers.')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--imp', action='append', default=[]); ap.add_argument('--str', action='append', default=[]); ap.add_argument('--va', action='append', default=[])
ap.add_argument('--max', type=int, default=6); ap.add_argument('--callers', action='append', default=[]); ap.add_argument('--range', default=None)
args = ap.parse_args()
args.path = args.path or os.path.join(args.game_dir, 'Sacred.exe')
pe = pefile.PE(args.path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
TS, TE = base + txt.VirtualAddress, base + txt.VirtualAddress + txt.Misc_VirtualSize
iat = {}
for imp in pe.DIRECTORY_ENTRY_IMPORT:
    for i in imp.imports: iat[i.address] = '%s!%s' % (imp.dll.decode(), i.name.decode() if i.name else 'ord%d' % i.ordinal)
secrng = [(base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData), s.Name.rstrip(b'\0').decode()) for s in pe.sections]
def sec_of(v):
    for a, b, n in secrng:
        if a <= v < b: return n
def str_at(v):
    off = v - base
    if off <= 0 or off >= len(img): return None
    end = off
    while end < len(img) and 32 <= img[end] < 127 and end - off < 300: end += 1
    if end - off >= 3 and img[end] == 0: return 'A"%s"' % bytes(img[off:end]).decode('ascii', 'replace')
    end = off
    while end + 1 < len(img) and 32 <= img[end] < 127 and img[end + 1] == 0 and end - off < 600: end += 2
    if end - off >= 6 and img[end] == 0 and img[end + 1] == 0: return 'W"%s"' % bytes(img[off:end]).decode('utf-16le', 'replace')
    return None
KNOWN = {}
for name, g in {'CLSID_AMMultiMediaStream': '49C47CE5-9BA4-11D0-8212-00C04FC32C45', 'IID_IAMMultiMediaStream': 'BEBE595C-9A6F-11D0-8FDE-00C04FD9189D',
 'IID_IMultiMediaStream': 'B502D1BC-9A57-11D0-8FDE-00C04FD9189D', 'IID_IMediaStream': 'B502D1BD-9A57-11D0-8FDE-00C04FD9189D', 'IID_IStreamSample': 'B502D1BE-9A57-11D0-8FDE-00C04FD9189D',
 'IID_IDirectDrawMediaStream': 'F4104FCE-9A70-11D0-8FDE-00C04FD9189D', 'IID_IDirectDrawStreamSample': 'F4104FCF-9A70-11D0-8FDE-00C04FD9189D',
 'MSPID_PrimaryVideo': 'A35FF56A-9FDA-11D0-8FDF-00C04FD9189D', 'MSPID_PrimaryAudio': 'A35FF56B-9FDA-11D0-8FDF-00C04FD9189D',
 'IID_IDirectDraw7': '15E65EC0-3B9C-11D2-B92F-00609797EA5B', 'IID_IDirectDraw': '6C14DB80-A733-11CE-A521-0020AF0BE560', 'IID_IDirectDraw4': '9C59509A-39BD-11D1-8C4A-00C04FD930C5', 'IID_IDirectDraw2': 'B3A6F3E0-2B43-11CF-A2DE-00AA00B93356',
 'IID_IDirect3D7': 'F5049E77-4861-11D2-A407-00A0C90629A8', 'IID_IDirect3DTnLHalDevice': 'F5049E78-4861-11D2-A407-00A0C90629A8', 'IID_IDirect3DHALDevice': 'F5049E7A-4861-11D2-A407-00A0C90629A8', 'IID_IDirect3DRGBDevice': 'A4665C60-2673-11CF-A31A-00AA00B93356',
 'IID_IDispatch': '00020400-0000-0000-C000-000000000046', 'IID_IUnknown': '00000000-0000-0000-C000-000000000046',
 'IID_IGraphBuilder': '56A868A9-0AD4-11CE-B03A-0020AF0BA770', 'IID_IMediaControl': '56A868B1-0AD4-11CE-B03A-0020AF0BA770', 'IID_IMediaEvent': '56A868B6-0AD4-11CE-B03A-0020AF0BA770', 'CLSID_FilterGraph': 'E436EBB3-524F-11CE-9F53-0020AF0BA770',
 'IID_IMediaSeeking': '36B73880-C2C8-11CF-8B46-00805F6CEF60', 'IID_IBasicAudio': '56A868B3-0AD4-11CE-B03A-0020AF0BA770', 'IID_IAudioMediaStream': 'F7537560-A3BE-11D0-8212-00C04FC32C45', 'IID_IAudioStreamSample': '345FEE00-ABA5-11D0-8212-00C04FC32C45',
 'IID_IAudioData': '54C719C0-AF60-11D0-8212-00C04FC32C45', 'CLSID_AMAudioData': 'F2468580-AF8A-11D0-8212-00C04FC32C45', 'IID_IDirectDrawSurface': '6C14DB81-A733-11CE-A521-0020AF0BE560'}.items():
    KNOWN[uuid.UUID(g).bytes_le] = name
def guid_at(v):
    off = v - base
    if off < 0 or off + 16 > len(img) or sec_of(v) not in ('.rdata', '.data'): return None
    b = bytes(img[off:off + 16])
    if b in KNOWN: return 'GUID %s' % KNOWN[b]
    if sum(1 for x in b if x == 0) <= 6 and not all(32 <= x < 127 for x in b[:8]):
        u = uuid.UUID(bytes_le=b)
        if u.version in (1, 4) or str(u).endswith('c000-000000000046'): return 'GUID? {%s}' % str(u).upper()
    return None
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
code = bytes(img[TS - base:TE - base])
# first pass: find sites
want_iat = {a for a, n in iat.items() if any(w.lower() in n.lower() for w in args.imp)}
want_str = set()
for s in args.str:
    sb = s.encode()
    for m in re.finditer(re.escape(sb), bytes(img)):
        # find string start
        st = m.start()
        while st > 0 and 32 <= img[st - 1] < 127: st -= 1
        want_str.add(base + st)
sites = []
for ins in md.disasm(code, TS):
    if ins.id == 0: continue
    for op in ins.operands:
        v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
        if v in want_iat: sites.append((ins.address, 'imp ' + iat[v]))
        if v in want_str: sites.append((ins.address, 'str ' + str(str_at(v))))
for v in args.va: sites.append((int(v, 16), 'va'))
callers = {int(v, 16) for v in args.callers}
if callers:
    for ins in md.disasm(code, TS):
        if ins.id == X86_INS_CALL and ins.operands[0].type == X86_OP_IMM and ins.operands[0].imm in callers:
            sites.append((ins.address, 'caller of 0x%x' % ins.operands[0].imm))
print("sites:", len(sites))
def func_bounds(a):
    # back up to a 16-aligned address preceded by nop/int3 padding or 'ret' + padding, within 24 KB
    s = a
    best = None
    while a - s < 24576 and s > TS:
        if s % 16 == 0 and img[s - 1 - base] in (0x90, 0xCC) and img[s - base] not in (0x90, 0xCC):
            best = s; break
        s -= 1
    if best is None: best = max(TS, a - 4096)
    # forward: until a ret followed by padding or 16 KB
    e = best; last_ret = None
    for ins in md.disasm(bytes(img[best - base:best - base + 16384]), best):
        if ins.id == X86_INS_RET: last_ret = ins.address + ins.size
        if ins.id in (X86_INS_NOP, X86_INS_INT3) and last_ret == ins.address and ins.address % 16 != 0 or (ins.id in (X86_INS_NOP, X86_INS_INT3) and last_ret == ins.address):
            e = ins.address; break
        if last_ret == ins.address and ins.address % 16 == 0: e = ins.address; break
    if e == best: e = best + 16384
    return best, e
done = set()
if args.range:
    s, e = [int(x, 16) for x in args.range.split(':')]
    print("### range 0x%x..0x%x" % (s, e))
    for ins in md.disasm(bytes(img[s - base:e - base]), s):
        if ins.id == 0: continue
        notes = []
        for op in ins.operands:
            v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
            if v is None: continue
            if v in iat: notes.append(iat[v])
            elif sec_of(v) in ('.rdata', '.data'):
                t = str_at(v) or guid_at(v)
                if t: notes.append(t)
            elif TS <= v < TE and ins.id == X86_INS_CALL: notes.append('sub_%x' % v)
        if notes or ins.id in (X86_INS_CALL, X86_INS_RET) or (ins.id == X86_INS_PUSH and ins.operands[0].type == X86_OP_IMM):
            print("  %08x  %-7s %-32s ; %s" % (ins.address, ins.mnemonic, ins.op_str, ' | '.join(notes)))
    sites = []
for a, what in sites[:args.max * 3]:
    s, e = func_bounds(a)
    if s in done: continue
    done.add(s)
    if len(done) > args.max: break
    print("\n### site 0x%x (%s) in function 0x%x..0x%x (%d bytes)" % (a, what, s, e, e - s))
    for ins in md.disasm(bytes(img[s - base:e - base]), s):
        if ins.id == 0: continue
        notes = []
        for op in ins.operands:
            v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
            if v is None: continue
            if v in iat: notes.append(iat[v])
            elif sec_of(v) in ('.rdata', '.data'):
                t = str_at(v) or guid_at(v)
                if t: notes.append(t)
            elif TS <= v < TE and ins.id == X86_INS_CALL: notes.append('sub_%x' % v)
        if notes or ins.id == X86_INS_CALL:
            print("  %08x  %-7s %-32s ; %s" % (ins.address, ins.mnemonic, ins.op_str, ' | '.join(notes)))
