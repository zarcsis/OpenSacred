#!/usr/bin/env python3
"""Callers of the CRT function _control87 in a PE (GameServer.exe by default): from the address of an instruction
inside the function (the fldcw after the mask merge), the function entry is found (the nearest call target), then the
call sites are listed, split into "game code" / CRT tail, with the context of the 5 preceding instructions.

Purpose (docs/01 §7, x87 precision): the server does not touch the control word from game
code (53 bits), the client sets _PC_24 three times in the main thread.

Example
  uv run python -I tools/analysis/pe/control87_callers.py OriginalGame/GameServer.exe --inner 0x51761d --crt-tail 0x4E1000

Expected output: "entry candidate: 0x517603 callers: N", lists of callers. srand_scan.py --ctl lists the same
callers together with the immediate operands of their pushes. Dependencies: pefile, capstone.
"""
import argparse
import os
import sys

import capstone
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Callers of a CRT _control87-like function in a PE.')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/GameServer.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--inner', type=hexkey, required=True, help='VA of an instruction inside the target function (e.g. the fldcw)')
ap.add_argument('--crt-tail', type=hexkey, required=True, help='first VA of the CRT/library tail')
args = ap.parse_args()
path = args.path or os.path.join(args.game_dir, 'GameServer.exe')
target_inner = args.inner; crt_tail = args.crt_tail
pe = pefile.PE(path, fast_load=True); base = pe.OPTIONAL_HEADER.ImageBase
text = [s for s in pe.sections if s.Name.rstrip(b'\0') == b'.text'][0]
code = text.get_data(); va0 = base + text.VirtualAddress
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32); md.skipdata = True
ins_list = list(md.disasm(code, va0))
# find function entry: nearest preceding address that is a call target
calls = {}
for ins in ins_list:
    if ins.mnemonic == 'call' and ins.op_str.startswith('0x'):
        t = int(ins.op_str, 16); calls.setdefault(t, []).append(ins.address)
cands = [t for t in calls if t <= target_inner and target_inner - t < 0x200]
entry = max(cands) if cands else None
print("entry candidate:", hex(entry) if entry else None, "callers:", len(calls.get(entry, [])))
if entry:
    cs = calls[entry]
    game = [c for c in cs if c < crt_tail]; lib = [c for c in cs if c >= crt_tail]
    print("callers in game code (<%s): %d -> %s" % (hex(crt_tail), len(game), [hex(c) for c in game][:40]))
    print("callers in CRT/lib tail: %d -> %s" % (len(lib), [hex(c) for c in lib][:20]))
    # second level: who calls the game-code callers' functions? print the surrounding instructions for each game caller
    idx = {ins.address: i for i, ins in enumerate(ins_list)}
    for c in game[:12]:
        i = idx[c]; ctx = ins_list[max(0, i - 5):i + 2]
        print(hex(c), ' ; '.join(x.mnemonic + ' ' + x.op_str for x in ctx))
