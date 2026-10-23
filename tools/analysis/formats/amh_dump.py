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
           templates), statistics
  items    tag 0xC8: a block per item, the item's type (u32), the marker 0xFEEDF00D, the object's id (u16) 10 bytes
           after the marker, the type again 14 bytes after it, and the item's record of 0x82 bytes 252 bytes after it,
           which begins with the type (the layout of Weapon.pak +0x80, docs/re/items.md); the backpack's grid (tag
           0xCA: 0x40C bytes, then 256 cells of 12 bytes) names its items by their ids (u32), the others are worn
  arts     tag 0xC7 (the creature): a u16 count of the hero's combat arts at its byte 1227, then entries of 22 bytes
           {u32 table (1 or 2); u16 id; u8 level; u8 bonus; u16 flags; f32 time; f32 factor; f32 remaining}
           (docs/re/arts.md); in a template the section is 1881 bytes for two arts and 22 more for each further one

Example
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --rows /tmp/amh.tsv
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --items /tmp/amh-items.tsv
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --arts /tmp/amh-arts.tsv

Expected output (2.28): every check passes on all of its items; 8 files, 64 sections (with --saves on a clean
installation: 10 files, 80 sections); 46 items in the templates, 14 of them worn; 19 combat arts in the templates.
--rows writes a row per section: file, slot, tag, size, compressed (0/1), bytes kept, crc32 of the kept bytes (inflated
or stored); --items a row per item: file, id, type, in the backpack (0/1), crc32 of its record; --arts a row per combat
art: file, its index, table, id, level, bonus, flags, and the time, the factor and the remaining as the hexadecimal of
their bits. Section contents (hero names etc.) are not printed.
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
INVENTORY, BACKPACK = 0xC8, 0xCA
CREATURE, ARTS, ART, CREATURE_SIZE = 0xC7, 1227, 22, 1881
ITEM_MARK, ITEM_ID, ITEM_TYPE, ITEM_RECORD, RECORD = b'\x0d\xf0\xed\xfe', 10, 14, 252, 0x82
GRID, CELL, CELLS = 12 + 0x400, 12, 256
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
    args = ap.parse_args()
    item_rows = []
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
            rows.append((rel, slot, tag, size, int(compressed), len(kept), f'{zlib.crc32(kept):08x}'))
        inflated = {tag: data for _, tag, _, _, data, _, _, _ in secs}
        if CREATURE in inflated:
            creature = inflated[CREATURE]
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
                record = inventory[at + ITEM_RECORD:at + ITEM_RECORD + RECORD]
                check('items: the type before the marker, after it and at the record agree',
                      len(record) == RECORD and struct.unpack_from('<I', inventory, at + ITEM_TYPE)[0] == item
                      and struct.unpack_from('<I', record, 0)[0] == item, f'{rel} at {at}')
                oid = struct.unpack_from('<H', inventory, at + ITEM_ID)[0]
                ids.append(oid)
                item_rows.append((rel, oid, item, int(oid in packed), f'{zlib.crc32(record):08x}'))
            check('items: the backpack names items of the inventory', packed <= set(ids), rel)
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
    print(f'{len(art_rows)} combat arts')
    if args.arts:
        with open(args.arts, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tindex\ttable\tid\tlevel\tbonus\tflags\ttime\tfactor\tremaining\n')
            for row in art_rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(art_rows)} rows -> {args.arts}')


if __name__ == '__main__':
    main()
