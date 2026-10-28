#!/usr/bin/env python3
"""The fixed-record tables PAK/Creature.pak (CIF), PAK/Weapon.pak (WPN), PAK/MOTIONS.PAK (MHP), World/Triggers.PAK
(TRG) and the item overlay PAK/Items03.pak (ITM): layout checks over every record and per-record tables for
cross-checking the Rust readers.

Purpose (docs/02 §6.2, §5.5, §4.1; PLAN.md §7): the reference prototype for `opensacred_formats::tables`. Measured on the
GOG files on 2026-10-17.

  creature "CIF" v0, u32 474 at +4; 474 records of 86 B from 0x100 to the end of the file, no directory. The field map
           comes from the client's own dump of the table (openheilig, creature-pak.md, read for facts): u32 id; u8
           class; u8 0; u8 flags (01 fly, 02 big, 10 no shadow, 20 ghost, 40, 80); u8 0; u16 experience[2]; u8
           base[6]; u8 0, 0; u8 skills[2]; u8 skills_extra[16]; u16 speed[2]; u8 bonus[6][2]; u8 bonus_value[6];
           u8 bonus_class[6]; u8 damping[4][5].
  weapon   "WPN" v8, u32 4883 at +4; 4883 records of 258 B from 0x100, then 4883 x 64 zero bytes to the end. Known:
           f32 at +0x00..+0x10 and +0x18 (all finite), u32 parent type +0x24, char name[88] +0x28 (Latin-1, padded
           with zeros), u32 type +0x80 (unique).
  motion   "MHP" v1, u32 3423 at +4; 3423 records of 198 B from 0x100 to the end: the parameter blocks
           MotionDesc[0x42:0x100] of the 3423 motions of models.pak and models03.pak, whose own blocks are zero.
           u16 index +0x00, u16 0, u16 index +0x04, u16 +0x06; f32[4] +0x46; f32[9] +0x62; u16 +0xBE; zeros elsewhere.
  trigger  "TRG" v1, u32 2268 at +4; a one-entry directory {u32 136, u32 0x10C, u32 size} at 0x100; 2268 records of
           16 B {u32 id; u16 16; u16 lo; u16 hi; u16 1; u32 0} from 0x10C, or 16 zero bytes for an empty one.
  items03  "ITM" v5: the items.pak layout with one used slot, 3999, which is empty in items.pak.

Example
  uv run python -I tools/analysis/formats/tables_dump.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/formats/tables_dump.py --game-dir ./OriginalGame --rows /tmp/tables.tsv

Expected output (2.28): every check passes on all of its items. --rows writes a row per record: table, index, the
named fields, crc32 of the record bytes. Dependencies: standard library.
"""
import argparse
import collections
import math
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

HEADER = 0x100
CREATURE, WEAPON, MOTION, TRIGGER = 86, 258, 198, 16
WEAPON_RESERVE = 64
CREATURE_FLAGS = 0x01 | 0x02 | 0x10 | 0x20 | 0x40 | 0x80
ITEMS_COUNT, ITEM_SIZE, ITEM_USED = 32768, 128, 2


class Check:
    """Counts how many items satisfy each named statement and keeps the first failures."""

    def __init__(self):
        self.ok = collections.Counter()
        self.total = collections.Counter()
        self.failures = collections.defaultdict(list)

    def __call__(self, name, cond, where=''):
        self.total[name] += 1
        if cond:
            self.ok[name] += 1
        elif len(self.failures[name]) < 3:
            self.failures[name].append(where)
        return cond

    def report(self, indent='  '):
        for name in self.total:
            fail = self.failures.get(name)
            print(f'{indent}{name}: {self.ok[name]}/{self.total[name]}' + (f'  first failures: {fail}' if fail else ''))


def header(data, magic, version):
    return data[:3] == magic and data[3] == version, struct.unpack_from('<I', data, 4)[0]


def zero_padded(field):
    name = field.split(b'\0')[0]
    return name, not any(field[len(name):])


def main():
    ap = argparse.ArgumentParser(description='Fixed-record tables: checks and per-record rows.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--rows', help='write a TSV row per record')
    args = ap.parse_args()
    read = lambda p: open(os.path.join(args.game_dir, p), 'rb').read()  # noqa: E731
    check = Check()
    rows = []

    data = read('PAK/Creature.pak')
    ok, n = header(data, b'CIF', 0)
    check('creature: CIF v0, records tile the file', ok and HEADER + n * CREATURE == len(data), n)
    for i in range(n):
        r = data[HEADER + CREATURE * i:HEADER + CREATURE * (i + 1)]
        cid, cls, gap1, flags, gap2 = struct.unpack_from('<I4B', r, 0)
        check('creature: zero bytes at +0x05, +0x07, +0x12, +0x13', gap1 == gap2 == 0 and r[0x12:0x14] == b'\0\0', i)
        check('creature: flags within the six named bits', flags & ~CREATURE_FLAGS == 0, i)
        rows.append(('creature', i, cid, cls, flags, f'{zlib.crc32(r):08x}'))

    data = read('PAK/Weapon.pak')
    ok, n = header(data, b'WPN', 8)
    reserve = HEADER + n * WEAPON
    check('weapon: WPN v8, records and the zero reserve tile the file',
          ok and reserve + n * WEAPON_RESERVE == len(data) and not any(data[reserve:]), n)
    ids = set()
    for i in range(n):
        r = data[HEADER + WEAPON * i:HEADER + WEAPON * (i + 1)]
        floats = struct.unpack_from('<5f', r, 0) + struct.unpack_from('<f', r, 0x18)
        check('weapon: the floats are finite', all(math.isfinite(f) for f in floats), i)
        parent, = struct.unpack_from('<I', r, 0x24)
        name, padded = zero_padded(r[0x28:0x80])
        check('weapon: the name is padded with zeros', padded, i)
        wid, = struct.unpack_from('<I', r, 0x80)
        check('weapon: type ids are unique', wid not in ids, i)
        ids.add(wid)
        rows.append(('weapon', i, wid, parent, name.hex(), f'{zlib.crc32(r):08x}'))

    data = read('PAK/MOTIONS.PAK')
    ok, n = header(data, b'MHP', 1)
    check('motion: MHP v1, records tile the file', ok and HEADER + n * MOTION == len(data), n)
    zero_spans = [(0x02, 0x04), (0x08, 0x46), (0x56, 0x62), (0x86, 0xBE), (0xC0, 0xC6)]
    for i in range(n):
        r = data[HEADER + MOTION * i:HEADER + MOTION * (i + 1)]
        index, _, again, extra = struct.unpack_from('<4H', r, 0)
        check('motion: +0x00 and +0x04 hold the record index', index == again == i, i)
        check('motion: zeros outside the known fields', not any(any(r[a:b]) for a, b in zero_spans), i)
        floats = struct.unpack_from('<4f', r, 0x46) + struct.unpack_from('<9f', r, 0x62)
        check('motion: the floats are finite', all(math.isfinite(f) for f in floats), i)
        rows.append(('motion', i, extra, struct.unpack_from('<H', r, 0xBE)[0], f'{zlib.crc32(r):08x}'))

    data = read('World/Triggers.PAK')
    ok, n = header(data, b'TRG', 1)
    kind, offset, size = struct.unpack_from('<3I', data, HEADER)
    check('trigger: TRG v1, one directory entry, records to the end',
          ok and (kind, offset, size) == (136, HEADER + 12, n * TRIGGER) and offset + size == len(data), n)
    used = 0
    for i in range(n):
        r = data[offset + TRIGGER * i:offset + TRIGGER * (i + 1)]
        tid, sixteen, lo, hi, one, zero = struct.unpack('<I4HI', r)
        empty = not any(r)
        check('trigger: {u32 id, u16 16, u16 lo, u16 hi < 16, u16 1, u32 0} or zeros',
              empty or (sixteen == 16 and hi < 16 and one == 1 and zero == 0), i)
        used += not empty
        rows.append(('trigger', i, tid, (hi << 16) | lo, f'{zlib.crc32(r):08x}'))

    data = read('PAK/Items03.pak')
    base = read('PAK/Items.pak')
    ok, n = header(data, b'ITM', 5)
    check('items03: ITM v5, 32768 slots', ok and n == ITEMS_COUNT, n)
    records = HEADER + 12 * n
    slots = [i for i in range(n) if struct.unpack_from('<I', data, HEADER + 12 * i)[0] == ITEM_USED]
    check('items03: one used slot, 3999', slots == [3999], slots)
    for i in slots:
        r = data[records + ITEM_SIZE * i:records + ITEM_SIZE * (i + 1)]
        check('items03: its slot is empty in items.pak',
              struct.unpack_from('<I', base, HEADER + 12 * i)[0] != ITEM_USED, i)
        name, padded = zero_padded(r[0x37:0x57])
        rows.append(('items03', i, r[0], name.hex(), f'{zlib.crc32(r):08x}'))

    check.report()
    print(f'{collections.Counter(r[0] for r in rows)}; {used} used triggers')
    if args.rows:
        with open(args.rows, 'w', encoding='ascii', newline='\n') as f:
            f.write('table\tindex\tfields\n')
            for row in rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(rows)} rows -> {args.rows}')


if __name__ == '__main__':
    main()
