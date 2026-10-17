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

Example
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --rows /tmp/amh.tsv
  python3 -I tools/analysis/formats/amh_dump.py --game-dir ./OriginalGame --saves

Expected output (2.28): every check passes on all of its items; 8 files, 64 sections (with --saves on a clean
installation: 10 files, 80 sections). --rows writes a row per section:
file, slot, tag, size, compressed (0/1), bytes kept, crc32 of the kept bytes (inflated or stored). Section contents
(hero names etc.) are not printed. Dependencies: standard library.
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
    args = ap.parse_args()
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
    check.report()
    print(f'{len(paths)} files, {len(rows)} sections; tags {sorted(collections.Counter(r[2] for r in rows).items())}')
    if args.rows:
        with open(args.rows, 'w', encoding='ascii', newline='\n') as f:
            f.write('file\tslot\ttag\tsize\tcompressed\tkept\tcrc\n')
            for row in rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(rows)} rows -> {args.rows}')


if __name__ == '__main__':
    main()
