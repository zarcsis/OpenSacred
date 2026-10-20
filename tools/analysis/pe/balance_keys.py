#!/usr/bin/env python3
"""The balance variables of the GOG client Sacred.exe 2.28 by name: the keys of the balance text's parser, each with the
address it stores its value at and the kind it stores (a float or an integer), and the values that bin/Balance.bin
holds for them.

Purpose (docs/02 §6.3, #71): the reference for the key table of `opensacred_formats::bin_tables::BALANCE_KEYS`. The
parser (the chain read off around 0x5EAC70 for #70) tests each key in turn: `push <the key's string>`, `push esi`,
`call 0x5E3700` (a search of the line for the key); when the line holds it, it takes the text after its '=', converts it
and stores it at the key's variable, with `fstp dword ptr [A]` for a float (the conversion 0x84B363) or
`mov dword ptr [A], eax` for an integer. The variables lie in the image that Balance.bin holds, 0xAD4490 to 0xAD57AF.

  keys     every `push imm32; push esi; call 0x5E3700` whose imm32 is a string: 380 tests of a key; the key's
           variable is the first store into the image after it, before the next key's test, or, for a row of values
           that it stores through a register, the word `mov esi, <the row>` sets (24 keys: ProzAW, ProzHP, OffLevel,
           LevelKap, OffLevelProz, BaseOffLevel, ...). 26 keys store elsewhere (other tables: SKILL_, MOVES,
           HorseData:, ...) and are not listed.
  Checks   354 keys with a variable in the image, each its own word; the first at +0x000 (BalanceDmg), the last at
           +0x131C (DaemonDamageBonus); every float's value finite in Balance.bin.

Example
  python3 -I tools/analysis/pe/balance_keys.py --game-dir ./OriginalGame
  python3 -I tools/analysis/pe/balance_keys.py --game-dir ./OriginalGame --table /tmp/balance_keys.tsv

Expected output (2.28): "354 keys, from BalanceDmg (+0x000) to DaemonDamageBonus (+0x131C); 276 floats, 78 integers",
every check passing, GameSpeed 1.25, TransSize 190, TransValu 50, skilllearn2 6. --table writes a row per key in the image's order (key, offset, kind).
Dependencies: pefile, capstone.
"""
import argparse
import os
import struct
import sys

import capstone
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

IMAGE, IMAGE_END = 0xAD4490, 0xAD4490 + 4896
FIND = 0x5E3700


def main():
    ap = argparse.ArgumentParser(description='The balance variables of Sacred.exe by name.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--table', help='write one tab-separated row per key to this file')
    args = ap.parse_args()
    pe = pefile.PE(os.path.join(args.game_dir, 'Sacred.exe'), fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    image = pe.get_memory_mapped_image()
    text = [s for s in pe.sections if s.Name.rstrip(b'\0') == b'.text'][0]
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    md.skipdata = True
    md.detail = True
    ins = list(md.disasm(text.get_data(), base + text.VirtualAddress))

    def string(va):
        at = va - base
        if not 0 <= at < len(image):
            return None
        end = image.find(b'\0', at, at + 64)
        raw = image[at:end] if end > at else b''
        return raw.decode('ascii') if raw and all(0x20 < b < 0x7F for b in raw) else None

    # The keys' tests: push <string>, push esi, call FIND.
    tests = [i for i in range(len(ins) - 2)
             if ins[i].mnemonic == 'push' and ins[i].op_str.startswith('0x')
             and ins[i + 1].mnemonic == 'push' and ins[i + 1].op_str == 'esi'
             and ins[i + 2].mnemonic == 'call' and ins[i + 2].op_str == hex(FIND)
             and string(int(ins[i].op_str, 16))]
    keys = []
    for n, i in enumerate(tests):
        name = string(int(ins[i].op_str, 16))
        stop = tests[n + 1] if n + 1 < len(tests) else i + 60
        found = None
        for x in ins[i + 3:stop]:
            if x.mnemonic in ('fstp', 'mov') and len(x.operands) == 2 or x.mnemonic == 'fstp':
                op = x.operands[0]
                if op.type == capstone.x86.X86_OP_MEM and op.mem.base == 0 and op.mem.index == 0 \
                        and IMAGE <= op.mem.disp < IMAGE_END:
                    found = (op.mem.disp - IMAGE, 'f32' if x.mnemonic == 'fstp' else 'i32')
                    break
        if found is None:
            # A row of values stored through a register: `mov esi, <the row's first word>`, then the stores
            # through esi, a float's by fstp.
            for j, x in enumerate(ins[i + 3:stop]):
                if x.mnemonic == 'mov' and x.op_str.startswith('esi, 0x') \
                        and IMAGE <= int(x.op_str.split(', ')[1], 16) < IMAGE_END:
                    floats = any(y.mnemonic == 'fstp' and 'esi' in y.op_str for y in ins[i + 3 + j:stop])
                    found = (int(x.op_str.split(', ')[1], 16) - IMAGE, 'f32' if floats else 'i32')
                    break
        keys.append((name, found))

    found = sorted((f[0], k, f[1]) for k, f in keys if f is not None)
    offsets = [o for o, _, _ in found]
    bal = open(os.path.join(args.game_dir, 'bin', 'Balance.bin'), 'rb').read()
    size = struct.unpack_from('<I', bal, 0)[0]
    value = lambda o, kind: struct.unpack_from('<f' if kind == 'f32' else '<i', bal, 4 + o)[0]
    checks = [
        ('380 tests of a key, 354 of them with a variable in the image', len(keys) == 380 and len(found) == 354),
        ('each its own variable, word-aligned', len(set(offsets)) == len(offsets) and all(o % 4 == 0 for o in offsets)),
        ('the first BalanceDmg at +0x000, the last DaemonDamageBonus at +0x131C',
         found[0][:2] == (0, 'BalanceDmg') and found[-1][:2] == (0x131C, 'DaemonDamageBonus')),
        ('Balance.bin holds the image, 4896 bytes', size == 4896),
        ('every float of Balance.bin finite',
         all(value(o, k) == value(o, k) and abs(value(o, k)) != float('inf') for o, _, k in found if k == 'f32')),
    ]
    print(f'{len(found)} keys, from {found[0][1]} (+0x{found[0][0]:03X}) to {found[-1][1]} (+0x{found[-1][0]:03X}); '
          f'{sum(k == "f32" for _, _, k in found)} floats, {sum(k == "i32" for _, _, k in found)} integers')
    for text_, ok in checks:
        print(f'  {text_}: {"ok" if ok else "FAILED"}')
    named = {k: (o, kind) for o, k, kind in found}
    for k in ('GameSpeed', 'TransSize', 'TransValu', 'skilllearn2'):
        if k in named:
            o, kind = named[k]
            print(f'  {k} +0x{o:03X} {kind} {value(o, kind)}')
    if args.table:
        os.makedirs(os.path.dirname(os.path.abspath(args.table)), exist_ok=True)
        with open(args.table, 'w', newline='\n') as out:
            out.write('key\toffset\tkind\n')
            for o, k, kind in found:
                out.write(f'{k}\t{o}\t{kind}\n')
        print(f'{len(found)} rows -> {args.table}')


if __name__ == '__main__':
    main()
