#!/usr/bin/env python3
"""The AMH container of the hero templates Templates/Hero*.ptx (and, with --saves, of the saves save/hero*.pax): layout
checks over every file and section, and per-section tables for cross-checking the Rust reader `opensacred_formats::amh`.
The saves are optional: the game rewrites save/, so they are not in the manifest and the Rust test leaves them out.

Purpose (docs/02 §9): the reference prototype. Measured on the GOG files on 2026-10-17.

  header   0x100 bytes: "AMH", u8 0x1B, u32 0x10 @4, u32 0x22C @8 (the summary size), u32 1 @0x1C, u32 9 @0x20,
           u32 2 @0x24, u32 20000 @0x28, zeros elsewhere (the same in the 8 templates and the 2 saves of a clean
           installation)
  table    16 slots x {u32 tag; u32 offset; u32 size} at 0x100; a used slot has an offset; the sections follow the table
           in slot order, back to back, the last one to the end of the file
  section  compressed: u32 0xBAADC0DE, u32 csize, 24 zero bytes, then a zlib stream of csize bytes that inflates to
           exactly `size`; stored: the bytes up to the next section, which are `size` bytes but for tag 0xC4, whose
           64 bytes on disk stand for a size of 556
  summary  tag 0xC3, stored, 556 bytes: u32 level, u32 class (1..9), wchar name[32] (UTF-16LE, empty in the
           templates), statistics; at 0x88 the equipment slots' count (u32, 19) and the types of the items in the
           slots (u32 each, 0 for none), each type that of an item the hero carries outside its backpack; at 0xED a
           byte for each of six difficulties, bronze's first, set where the hero won it
  items    tag 0xC8: a block per item, the item's type (u32), the marker 0xFEEDF00D, the object's id (u16) 10 bytes
           after the marker, the type again 14 bytes after it, the marker 0xBAADC0DE 247 bytes after it and the count
           of its sockets' items (u8) after that, then each socketed item's record of 0x82 bytes and the item's own,
           which begins with the type (the layout of Weapon.pak +0x80, docs/re/items.md); the backpack's grid (tag
           0xCA: six u16, the fifth the grid's width, 0x400 bytes, then 256 cells of 12 bytes, row by row) names its
           items by their ids (u32) at their top left cells, each followed by u8 1, u8 width, u8 height, u8 0 and the
           item's type (u32); the others are worn
  creature tag 0xC7: 0xFEEDF00D at 16, 0xC0DEF00D at 263, the equipment's 19 slots at 435 (u32 object ids of the
           inventory's items, 0 none), the stats' marker 0xDEADBEEF at 961 and the stats' +0x08..+0xF5 from 989 (so the
           stats' +x at 981 + x): +0x08 class u32, +0x0C experience u32, +0x10 attributes 6 x u16, +0x1C the points spent
           6 x u8, +0x23 the attribute points left u8, +0x24 the slots' skills 8 x u8, +0x2C their bases, +0x34 the items'
           bonuses, +0x42 the skill points u16, +0x44 the skills to learn u16, +0x46 gold u32, +0x4A the base attributes
           6 x u16, +0x56 the level u16 (docs/02 §9)
  arts     tag 0xC7 (the creature): a u16 count of the hero's combat arts at its byte 1227, then entries of 22 bytes
           {u32 table (1 or 2); u16 id; u8 level; u8 bonus; u16 flags; f32 time; f32 factor; f32 remaining}
           (docs/re/arts.md); in a template the section is 1881 bytes for two arts and 22 more for each further one

Example
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --rows /tmp/amh.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --items /tmp/amh-items.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --arts /tmp/amh-arts.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --backpack /tmp/amh-backpack.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves --summaries /tmp/amh-summaries.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves --stats /tmp/amh-stats.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves --sets /tmp/amh-sets.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves --sockets /tmp/amh-sockets.tsv
  uv run python -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves --art-slots /tmp/amh-art-slots.tsv

Expected output (2.28): every check passes on all of its items; 8 files, 64 sections (with --saves on a clean
installation: 10 files, 80 sections); 46 items in the templates, 14 of them worn, 32 in their backpacks; 19 combat arts
in the templates.
--rows writes a row per section: file, slot, tag, size, compressed (0/1), bytes kept, crc32 of the kept bytes (inflated
or stored); --items a row per item: file, id, type, in the backpack (0/1), crc32 of its record; --arts a row per combat
art: file, its index, table, id, level, bonus, flags, and the time, the factor and the remaining as the hexadecimal of
their bits; --backpack a row per item of the backpack's grid: file, id, column, row, width, height, type (the templates'
32: three potions in the first column's top cells and a torch of 1 x 2 below them each); --summaries a row per file:
file, level, class, the equipment's slots and types as slot:type joined by commas (the templates' weapons alone, 15 and
14 items in the two saves), and the six won bytes as 0/1 (all 0 in the 10 files); --stats a row per file of its creature's
stats: file, class, level, experience, gold, the attributes, the base attributes and the points spent on each (comma-joined),
the attribute points left, the skills as slot:skill:base:bonus, the skill points, the skills to learn, and the equipment as
slot:object id; --sets a row per half of a weapon set that holds an item: file, set, half (0 the left hand), the item's
object id (the 0x78 bytes after the backpack's grid, five sets of two halves of 12 bytes, a half's first u32 the id);
--sockets a row per socketed item's record: file, the item's object id, the socket's index, the record's type (u32) and
the crc32 of its 0x82 bytes; --art-slots a row per file of the bar's art slots after the weapon sets (five slots in two
banks, a record of 20 bytes each, the slot s of the bank b the record 2s + b, its kind and its art's id the first two
u32) and the selections after them (u16 each, the weapon set and the art slot selected): file, the slots of the bank 0
and of the bank 1 as slot:kind:id joined by commas, the selections joined by a comma. Section contents (hero names
etc.) are not printed.
Dependencies: standard library.
"""
import argparse
import collections
import glob
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

HEADER, SLOTS = 0x100, 16
COMPRESSED, PREFIX = 0xBAADC0DE, 32
SUMMARY, MINI = 0xC3, 0xC4
EQUIPMENT_COUNT, EQUIPMENT, WON, WON_COUNT = 0x88, 19, 0xED, 6
INVENTORY, BACKPACK = 0xC8, 0xCA
CREATURE, ARTS, ART, CREATURE_SIZE = 0xC7, 1227, 22, 1881
# The creature's markers, its equipment's slots, and its stats' +0 (docs/02 §9).
CREATURE_MARKS = {16: 0xFEEDF00D, 263: 0xC0DEF00D, 961: 0xDEADBEEF}
SLOTS_AT, STATS = 435, 981
ITEM_MARK, ITEM_ID, ITEM_TYPE, ITEM_RECORD, RECORD = b'\x0d\xf0\xed\xfe', 10, 14, 252, 0x82
# The marker before the records and the count of the socketed items' records before the item's own.
SOCKETS_MARK, SOCKETS = b'\xde\xc0\xad\xba', 251
GRID, CELL, CELLS = 12 + 0x400, 12, 256
# The weapon sets after the grid: five sets of two halves of 12 bytes, a half's first u32 its item's object id.
WEAPON_SETS, HALVES, HALF = 5, 2, 12
# The bar's art slots after the sets: five slots in two banks of 20-byte records, then two u16 selections.
ART_SLOTS, ART_SLOT = 5, 20
EXPECTED_HEADER = {4: 0x10, 8: 0x22C, 0x1C: 1, 0x20: 9, 0x24: 2, 0x28: 20000}


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


def sections(d):
    """(slot, tag, size, compressed, kept bytes, offset, extent) of the used slots."""
    table = [(i, *struct.unpack_from('<3I', d, HEADER + 12 * i)) for i in range(SLOTS)]
    used = [t for t in table if t[2]]
    out = []
    for k, (slot, tag, off, size) in enumerate(used):
        end = used[k + 1][2] if k + 1 < len(used) else len(d)
        blk = d[off:end]
        compressed = struct.unpack_from('<I', blk, 0)[0] == COMPRESSED if len(blk) >= 4 else False
        if compressed:
            csize = struct.unpack_from('<I', blk, 4)[0]
            kept = zlib.decompress(blk[PREFIX:PREFIX + csize])
        else:
            kept = blk
        out.append((slot, tag, size, compressed, kept, off, end - off, blk))
    return table, out


def main():
    ap = argparse.ArgumentParser(description='AMH hero templates and saves: checks and per-section rows.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--rows', help='write a TSV row per section')
    ap.add_argument('--saves', action='store_true', help='also check the saves of save/ (the game rewrites them)')
    ap.add_argument('--items', help='write a TSV row per item of the inventory sections')
    ap.add_argument('--arts', help='write a TSV row per combat art of the creature sections')
    ap.add_argument('--backpack', help='write a TSV row per item of the backpack grids')
    ap.add_argument('--summaries', help='write a TSV row per file of its summary\'s equipment and won difficulties')
    ap.add_argument('--stats', help='write a TSV row per file of its creature\'s stats and equipment')
    ap.add_argument('--sets', help='write a TSV row per weapon set\'s half that holds an item')
    ap.add_argument('--sockets', help='write a TSV row per socketed item\'s record')
    ap.add_argument('--art-slots', help='write a TSV row per file of its bar\'s art slots and its selections')
    args = ap.parse_args()
    art_slot_rows = []
    set_rows = []
    socket_rows = []
    summary_rows = []
    stats_rows = []
    item_rows = []
    backpack_rows = []
    art_rows = []
    paths = sorted(glob.glob(os.path.join(args.game_dir, 'Templates', '*.ptx')))
    if args.saves:
        paths += sorted(glob.glob(os.path.join(args.game_dir, 'save', '*.pax')))
    check = Check()
    rows = []
    for path in paths:
        rel = os.path.relpath(path, args.game_dir).replace('\\', '/')
        d = open(path, 'rb').read()
        check('magic AMH, version 0x1B', d[:4] == b'AMH\x1b', rel)
        words = {k: struct.unpack_from('<I', d, k)[0] for k in range(4, HEADER, 4)}
        check('header words as expected, zeros elsewhere',
              all(v == EXPECTED_HEADER.get(k, 0) for k, v in words.items()), rel)
        table, secs = sections(d)
        check('unused slots are zero', all(not any(t[1:]) for t in table if not t[2]), rel)
        check('sections start after the table, in slot order, back to back',
              secs[0][5] == HEADER + 12 * SLOTS and all(a[5] < b[5] for a, b in zip(secs, secs[1:])), rel)
        for slot, tag, size, compressed, kept, off, extent, blk in secs:
            where = f'{rel} slot {slot}'
            if compressed:
                csize = struct.unpack_from('<I', blk, 4)[0]
                check('compressed: 24 zero bytes after csize, the stream fills the section',
                      not any(blk[8:PREFIX]) and PREFIX + csize == extent, where)
                check('compressed: inflates to the table size', len(kept) == size, where)
            elif tag == MINI:
                check('stored 0xC4: 64 bytes on disk for a size of 556', (extent, size) == (64, 556), where)
            else:
                check('stored: the section is the table size', extent == size, where)
            if tag == SUMMARY:
                level, cls = struct.unpack_from('<II', kept, 0)
                name = kept[8:72].decode('utf-16le').split('\0')[0]
                check('summary: stored, class 1..9, level 1..', not compressed and 1 <= cls <= 9 and level >= 1, where)
                check('summary: the name is padded with zeros',
                      not any(kept[8 + 2 * len(name):72]), where)
                count = struct.unpack_from('<I', kept, EQUIPMENT_COUNT)[0]
                check('summary: 19 equipment slots', count == EQUIPMENT, where)
                equipment = struct.unpack_from(f'<{EQUIPMENT}I', kept, EQUIPMENT_COUNT + 4)
                won = kept[WON:WON + WON_COUNT]
                summary_rows.append((rel, level, cls,
                                     ','.join(f'{k}:{t}' for k, t in enumerate(equipment) if t),
                                     ''.join(str(int(b != 0)) for b in won)))
            rows.append((rel, slot, tag, size, int(compressed), len(kept), f'{zlib.crc32(kept):08x}'))
        inflated = {tag: data for _, tag, _, _, data, _, _, _ in secs}
        if CREATURE in inflated:
            creature = inflated[CREATURE]
            check('creature: its markers at 16, 263 and 961',
                  all(struct.unpack_from('<I', creature, at)[0] == mark for at, mark in CREATURE_MARKS.items()), rel)
            stat = lambda x, f: struct.unpack_from(f, creature, STATS + x)
            level = stat(0x56, '<H')[0]
            summary = inflated.get(SUMMARY)
            if summary:
                check('creature: its class and level are the summary\'s',
                      stat(8, '<I')[0] == struct.unpack_from('<I', summary, 4)[0]
                      and level == struct.unpack_from('<I', summary, 0)[0], rel)
            slots = struct.unpack_from('<19I', creature, SLOTS_AT)
            skills = [f'{k}:{s}:{b}:{n}' for k, (s, b, n) in enumerate(zip(stat(0x24, '<8B'), stat(0x2C, '<8B'),
                                                                             stat(0x34, '<8B'))) if s]
            stats_rows.append((rel, stat(8, '<I')[0], level, stat(0xC, '<I')[0], stat(0x46, '<I')[0],
                               ','.join(map(str, stat(0x10, '<6H'))), ','.join(map(str, stat(0x4A, '<6H'))),
                               ','.join(map(str, stat(0x1C, '<6B'))), stat(0x23, '<B')[0], ','.join(skills),
                               stat(0x42, '<H')[0], stat(0x44, '<H')[0],
                               ','.join(f'{k}:{i}' for k, i in enumerate(slots) if i)))
            count = struct.unpack_from('<H', creature, ARTS)[0]
            check('arts: the entries fit the creature section, each of the table 1 or 2',
                  ARTS + 2 + ART * count <= len(creature)
                  and all(struct.unpack_from('<I', creature, ARTS + 2 + ART * k)[0] in (1, 2) for k in range(count)),
                  rel)
            if rel.startswith('Templates/'):
                check('arts: a template\'s creature section is 1881 bytes for two arts, 22 more for each further one',
                      len(creature) == CREATURE_SIZE + ART * (count - 2), rel)
            for k in range(count):
                table, art, level, bonus, flags = struct.unpack_from('<IHBBH', creature, ARTS + 2 + ART * k)
                floats = struct.unpack_from('<3I', creature, ARTS + 2 + ART * k + 10)
                art_rows.append((rel, k, table, art, level, bonus, flags) + tuple(f'{w:08x}' for w in floats))
        if INVENTORY in inflated and BACKPACK in inflated:
            inventory, grid = inflated[INVENTORY], inflated[BACKPACK][GRID:GRID + CELL * CELLS]
            packed = {struct.unpack_from('<I', grid, CELL * i)[0] for i in range(CELLS)} - {0}
            ids = []
            for at in range(4, len(inventory) - 3):
                if inventory[at:at + 4] != ITEM_MARK:
                    continue
                item = struct.unpack_from('<I', inventory, at - 4)[0]
                check('items: the sockets\' count follows its marker', inventory[at + SOCKETS - 4:at + SOCKETS] == SOCKETS_MARK,
                      f'{rel} at {at}')
                own = at + ITEM_RECORD + RECORD * inventory[at + SOCKETS]
                record = inventory[own:own + RECORD]
                for k in range(inventory[at + SOCKETS]):
                    socket = inventory[at + ITEM_RECORD + RECORD * k:at + ITEM_RECORD + RECORD * (k + 1)]
                    socket_rows.append((rel, struct.unpack_from('<H', inventory, at + ITEM_ID)[0], k,
                                        struct.unpack_from('<I', socket, 0)[0], f'{zlib.crc32(socket):08x}'))
                check('items: the type before the marker and after it agree, a whole record',
                      len(record) == RECORD and struct.unpack_from('<I', inventory, at + ITEM_TYPE)[0] == item,
                      f'{rel} at {at}')
                # The record's own type, the item's in every template; one item of save/hero07.pax holds
                # another's (docs/02 §9).
                if len(record) == RECORD and struct.unpack_from('<I', record, 0)[0] != item:
                    print(f'  note: {rel} at {at}: the item {item} holds the record of {struct.unpack_from("<I", record, 0)[0]}')
                oid = struct.unpack_from('<H', inventory, at + ITEM_ID)[0]
                ids.append(oid)
                item_rows.append((rel, oid, item, int(oid in packed), f'{zlib.crc32(record):08x}'))
            check('items: the backpack names items of the inventory', packed <= set(ids), rel)
            types = {struct.unpack_from('<H', inventory, at + ITEM_ID)[0]: struct.unpack_from('<I', inventory, at - 4)[0]
                     for at in range(4, len(inventory) - 3) if inventory[at:at + 4] == ITEM_MARK}
            width = struct.unpack_from('<H', inflated[BACKPACK], 8)[0]
            for i in range(CELLS):
                oid, one, w, h, zero, item = struct.unpack_from('<IBBBBI', grid, CELL * i)
                if not oid:
                    continue
                check('backpack: a cell holds 1, the size, 0 and the type of the item of its id',
                      (one, zero) == (1, 0) and w >= 1 and h >= 1 and types.get(oid) == item, f'{rel} cell {i}')
                backpack_rows.append((rel, oid, i % width, i // width, w, h, item))
            sets = inflated[BACKPACK][GRID + CELL * CELLS:GRID + CELL * CELLS + WEAPON_SETS * HALVES * HALF]
            for k in range(WEAPON_SETS * HALVES):
                oid = struct.unpack_from('<I', sets, HALF * k)[0]
                if oid:
                    check('sets: a half names an item of the inventory outside the backpack',
                          oid in types and oid not in packed, f'{rel} set {k // HALVES}')
                    set_rows.append((rel, k // HALVES, k % HALVES, oid))
            slots_at = GRID + CELL * CELLS + WEAPON_SETS * HALVES * HALF
            banks = [[], []]
            for k in range(ART_SLOTS * 2):
                kind, art = struct.unpack_from('<II', inflated[BACKPACK], slots_at + ART_SLOT * k)
                check('art slots: each slot\'s kind is none, a spell, a weapon art or a combo', kind in (0, 2, 3, 4),
                      f'{rel} slot {k // 2}')
                if kind:
                    banks[k % 2].append(f'{k // 2}:{kind}:{art}')
            selections = struct.unpack_from('<2H', inflated[BACKPACK], slots_at + ART_SLOT * ART_SLOTS * 2)
            check('art slots: the selections a set and a slot', selections[0] < WEAPON_SETS and
                  selections[1] < ART_SLOTS, rel)
            art_slot_rows.append((rel, ','.join(banks[0]), ','.join(banks[1]), ','.join(map(str, selections))))
            names = [r[1] for r in backpack_rows if r[0] == rel]
            check('backpack: each item at one cell, its top left', len(names) == len(set(names)), rel)
            outside = collections.Counter(types[oid] for oid in types if oid not in packed)
            slots = collections.Counter(int(p.split(':')[1]) for r in summary_rows if r[0] == rel
                                        for p in r[3].split(',') if p)
            check('summary: the equipment\'s items are items outside the backpack', not slots - outside, rel)
            if CREATURE in inflated:
                held = struct.unpack_from('<19I', inflated[CREATURE], SLOTS_AT)
                summary_types = struct.unpack_from('<19I', inflated[SUMMARY], 0x8C) if SUMMARY in inflated else ()
                check('creature: each slot\'s item is outside the backpack, of the summary\'s type for the slot',
                      all(i not in packed and types.get(i) == t for i, t in zip(held, summary_types) if i), rel)
    check.report()
    print(f'{len(paths)} files, {len(rows)} sections; tags {sorted(collections.Counter(r[2] for r in rows).items())}')
    if args.rows:
        with open(args.rows, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tslot\ttag\tsize\tcompressed\tkept\tcrc\n')
            for row in rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(rows)} rows -> {args.rows}')
    print(f'{len(item_rows)} items, {sum(1 for r in item_rows if not r[3])} of them worn')
    if args.items:
        with open(args.items, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tid\ttype\tpacked\tcrc\n')
            for row in item_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(item_rows)} rows -> {args.items}')
    print(f'{len(backpack_rows)} items in the backpacks')
    if args.backpack:
        with open(args.backpack, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tid\tcolumn\trow\twidth\theight\ttype\n')
            for row in backpack_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(backpack_rows)} rows -> {args.backpack}')
    for name, rows_out, head, path in [('art slots\' files', art_slot_rows, 'file\tbank0\tbank1\tselected\n',
                                        args.art_slots),
                                       ('weapon sets\' halves', set_rows, 'file\tset\thalf\tid\n', args.sets),
                                       ('socketed records', socket_rows, 'file\tid\tindex\ttype\tcrc\n', args.sockets)]:
        print(f'{len(rows_out)} {name}')
        if path:
            with open(path, 'w', encoding='ascii', newline='\n') as f:
                f.write(head)
                for row in rows_out:
                    f.write('\t'.join(str(x) for x in row) + '\n')
            print(f'{len(rows_out)} rows -> {path}')
    print(f'{len(stats_rows)} creatures\' stats')
    if args.stats:
        with open(args.stats, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tclass\tlevel\texperience\tgold\tattributes\tbase\tspent\tpoints\tskills\tskill_points'
                    '\tto_learn\tequipment\n')
            for row in stats_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(stats_rows)} rows -> {args.stats}')
    print(f'{len(summary_rows)} summaries')
    if args.summaries:
        with open(args.summaries, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tlevel\tclass\tequipment\twon\n')
            for row in summary_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(summary_rows)} rows -> {args.summaries}')
    print(f'{len(art_rows)} combat arts')
    if args.arts:
        with open(args.arts, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tindex\ttable\tid\tlevel\tbonus\tflags\ttime\tfactor\tremaining\n')
            for row in art_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(art_rows)} rows -> {args.arts}')


if __name__ == '__main__':
    main()
