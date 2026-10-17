#!/usr/bin/env python3
"""Shared PE image loader for the tools/analysis scripts (the project's own code).

Purpose
  * ``read_pe_image(path)`` -> ``Img``: the in-memory image of the GOG client ``Sacred.exe``
    (pefile.get_memory_mapped_image), read as is;
  * ``Img``: methods u8/u16/u32/f32/f64/str_at/sec_of/disasm/note/dump/func_end.

Usage from a script (``python3 -I`` does not add the script's directory to sys.path, so the path to
``common`` is set explicitly)::

    import os, sys
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
    from sacred_pe import read_pe_image, DEFAULT_GAME_DIR

Dependencies: pefile, capstone.
"""
import struct

import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DEFAULT_GAME_DIR = './OriginalGame'


class Img:
    """In-memory PE image (read-only)."""

    def __init__(self, path):
        pe = pefile.PE(path)
        self.pe = pe
        self.path = path
        self.base = pe.OPTIONAL_HEADER.ImageBase
        self.img = bytearray(pe.get_memory_mapped_image())
        txt = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
        self.txt = txt
        self.TS = self.base + txt.VirtualAddress
        self.TE = self.TS + txt.Misc_VirtualSize
        self.secs = [(self.base + s.VirtualAddress, self.base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData),
                      s.Name.rstrip(b'\0').decode(errors='replace')) for s in pe.sections]
        self.iat = {}
        if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
            for imp in pe.DIRECTORY_ENTRY_IMPORT:
                for i in imp.imports:
                    self.iat[i.address] = '%s!%s' % (imp.dll.decode(), i.name.decode() if i.name else 'ord%d' % i.ordinal)
        self.md = Cs(CS_ARCH_X86, CS_MODE_32)
        self.md.detail = True
        self.md.skipdata = True

    def sec_of(self, v):
        for a, b, n in self.secs:
            if a <= v < b:
                return n

    def u8(self, va): return self.img[va - self.base]
    def u16(self, va): return struct.unpack_from('<H', self.img, va - self.base)[0]
    def u32(self, va): return struct.unpack_from('<I', self.img, va - self.base)[0]
    def f32(self, va): return struct.unpack_from('<f', self.img, va - self.base)[0]
    def f64(self, va): return struct.unpack_from('<d', self.img, va - self.base)[0]
    def bytes(self, va, n): return bytes(self.img[va - self.base: va - self.base + n])

    def str_at(self, v, maxlen=300):
        off = v - self.base
        if off <= 0 or off >= len(self.img):
            return None
        end = off
        while end < len(self.img) and 32 <= self.img[end] < 127 and end - off < maxlen:
            end += 1
        if end - off >= 3 and self.img[end] == 0:
            return bytes(self.img[off:end]).decode('ascii', 'replace')
        return None

    def disasm(self, s, e):
        return [i for i in self.md.disasm(bytes(self.img[s - self.base:e - self.base]), s) if i.id != 0]

    def note(self, ins):
        from capstone.x86 import X86_OP_IMM, X86_OP_MEM
        notes = []
        for op in ins.operands:
            v = op.imm if op.type == X86_OP_IMM else (op.mem.disp if op.type == X86_OP_MEM else None)
            if v is None:
                continue
            if v in self.iat:
                notes.append(self.iat[v])
            elif self.sec_of(v) in ('.rdata', '.data'):
                t = self.str_at(v)
                if t:
                    notes.append('"%s"' % t)
                else:
                    try:
                        f = self.f32(v); d = self.f64(v); u = self.u32(v)
                        notes.append('[%s]=0x%x f=%g d=%g' % (self.sec_of(v), u, f, d))
                    except Exception:
                        pass
            elif self.TS <= v < self.TE and ins.mnemonic == 'call':
                notes.append('sub_%x' % v)
            if op.type == X86_OP_IMM and 0x3C000000 <= v <= 0x47000000:
                notes.append('f=%g' % struct.unpack('<f', struct.pack('<I', v))[0])
        return ' | '.join(notes)

    def dump(self, s, e, out=None):
        lines = []
        for ins in self.disasm(s, e):
            lines.append('  %08x  %-7s %-40s ; %s' % (ins.address, ins.mnemonic, ins.op_str, self.note(ins)))
        txt = '\n'.join(lines)
        if out:
            out.write(txt + '\n')
        return txt

    def func_end(self, s, maxlen=0x8000):
        """End of a function: the first ret followed by int3/nop padding or a 16-aligned start of the next one; rough."""
        last = s
        for ins in self.disasm(s, s + maxlen):
            if ins.mnemonic == 'ret':
                nxt = ins.address + ins.size
                b = self.img[nxt - self.base]
                if b in (0xCC, 0x90) or nxt % 16 == 0:
                    return nxt
            last = ins.address + ins.size
        return last


def read_pe_image(path):
    return Img(path)


def hexkey(s):
    """argparse type: a hexadecimal number with or without the 0x prefix."""
    return int(s, 16)
