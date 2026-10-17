#!/usr/bin/env python3
"""Function matching between two PEs (GameServer.exe and Sacred.exe) by relocation-insensitive hashes.

Purpose
  ``analyse(path) -> {start_va: (size, md5)}``: linear disassembly of .text (capstone, skipdata);
  function starts = call targets ∪ push ebp/mov ebp,esp prologues ∪ 16-aligned addresses after nop/int3 padding;
  in each instruction's bytes, the rel32 of call/jmp and any imm/disp that falls in the image range are zeroed, then
  the function body (without trailing padding) is hashed with MD5. The same hash in two images = a byte-identical
  function up to addresses (docs/01-binary-analysis.md, §5.1: 1,176 game functions + 607 STL instances shared).

Usage: see pe/fn_match.py and pe/fn_census2.py (``sys.path.insert(0, <...>/common)``).

Dependencies: pefile, capstone.
"""
import sys, struct, hashlib, collections, bisect, json, time
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *


def analyse(path):
    pe = pefile.PE(path); base = pe.OPTIONAL_HEADER.ImageBase; img = bytearray(pe.get_memory_mapped_image())
    txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
    TS, TE = base + txt.VirtualAddress, base + txt.VirtualAddress + txt.Misc_VirtualSize
    IMG_END = base + pe.OPTIONAL_HEADER.SizeOfImage
    md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
    code = bytes(img[TS - base:TE - base])
    starts = set(); prev = None; norm = {}  # addr -> normalized bytes
    insns = []
    for ins in md.disasm(code, TS):
        if ins.id == 0: insns.append((ins.address, ins.size, bytes(ins.bytes))); prev = ins; continue
        b = bytearray(ins.bytes)
        if ins.id == X86_INS_CALL and ins.operands[0].type == X86_OP_IMM:
            t = ins.operands[0].imm
            if TS <= t < TE: starts.add(t)
        # normalise: zero rel32 of call/jmp and any imm/disp inside image range
        if ins.id in (X86_INS_CALL, X86_INS_JMP) and ins.operands[0].type == X86_OP_IMM and ins.size >= 5:
            b[-4:] = b'\0\0\0\0'
        else:
            for op in ins.operands:
                v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
                if v is not None and base <= v < IMG_END:
                    pv = struct.pack('<I', v & 0xffffffff); i = bytes(ins.bytes).rfind(pv)
                    if i >= 0: b[i:i + 4] = b'\0\0\0\0'
        if ins.bytes[:2] == b'\x8b\xec' and prev is not None and prev.bytes == b'\x55': starts.add(prev.address)
        insns.append((ins.address, ins.size, bytes(b)))
        prev = ins
    # padding-based starts: instruction following a run of nop/int3 at 16-aligned address
    for i in range(1, len(insns)):
        a = insns[i][0]
        if a % 16 == 0 and img[a - 1 - base] in (0x90, 0xCC) and img[a - base] not in (0x90, 0xCC): starts.add(a)
    starts = sorted(starts)
    addrs = [x[0] for x in insns]
    funcs = {}
    for i, s in enumerate(starts):
        e = starts[i + 1] if i + 1 < len(starts) else TE
        k0 = bisect.bisect_left(addrs, s); k1 = bisect.bisect_left(addrs, e)
        chunk = b''.join(x[2] for x in insns[k0:k1])
        # strip trailing padding
        chunk = chunk.rstrip(b'\x90').rstrip(b'\xcc')
        funcs[s] = (len(chunk), hashlib.md5(chunk).hexdigest())
    return funcs
