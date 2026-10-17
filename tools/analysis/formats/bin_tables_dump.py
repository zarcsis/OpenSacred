#!/usr/bin/env python3
"""The data tables of bin/ (Balance.bin, World.bin, World2.bin, sets.bin, wpmod.bin, wea.bin, Rust.bin, treppe.bin,
static10_18.bin, merc.bin, Addon/merc.bin, MultiStart.bin): layout checks and a fingerprint per file for cross-checking
the Rust reader `opensacred_formats::bin_tables`.

Purpose (docs/02 §6.3): the reference prototype. The layouts below are checked to the last byte; wpmod.bin's records
follow the client's loader (0x42F8CE: fread of 0xD4 bytes, a u32 count, count x 0x18). Measured on the GOG files on
2026-10-17.

  Balance.bin     u32 4896, 4896 words (f32 or i32 by key; 0xFFFFFFFF "not set"), 1185 more words to the end
  World.bin       u32 n, n x {i32, i32, i32}
  World2.bin      u32 32768, 32768 bytes
  sets.bin        u32 66, 66 x 112 B: the first all 0xCC, the others {u32 items[10]; u32 hash; u8 bytes[4]; 64 zeros}
  wpmod.bin       u32 n, n x {u32 words[53]; u32 k; k x u32[6]}
  wea.bin         {u32 k; u32 ids[k]} lists to the end of the file (256)
  Rust.bin        u32 n, n x {u32 k; k x (u32, u32)}
  treppe.bin      u32[4] rows to the end
  static10_18.bin "map\\0", u32 n, zeros to 0x100, n x u32[4]: two (old, new) pairs, the second
                  0xCCCCCCCC twice when unset (8 rows)
  merc.bin        {u32 0; u32 x; u32 y; u32 kind} rows to the end (both files)
  MultiStart.bin  {u32 0; u32 x; u32 y; u32 0} rows to the end

Example
  python3 -I tools/analysis/formats/bin_tables_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/bin_tables_dump.py --game-dir ./OriginalGame --rows /tmp/bin.tsv

Expected output (2.28): every check passes. --rows writes a row per file: path, records, crc32 of the canonical
listing (`canonical`: one line per record, numbers in decimal separated by spaces). Dependencies: standard library.
"""
import argparse
import collections
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR


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


class Cursor:
    def __init__(self, data, pos=0):
        self.d, self.p = data, pos

    def u32s(self, n, fmt='I'):
        if self.p + 4 * n > len(self.d):
            raise ValueError(f'{n} words at {self.p:#x} run past the end')
        v = struct.unpack_from(f'<{n}{fmt}', self.d, self.p)
        self.p += 4 * n
        return list(v)

    def u32(self):
        return self.u32s(1)[0]

    def left(self):
        return len(self.d) - self.p


def line(values):
    return ' '.join(str(v) for v in values)


def balance(d, check):
    c = Cursor(d)
    n = c.u32()
    values = c.u32s(n)
    tail = c.u32s(c.left() // 4)
    check('Balance.bin: 4896 words and a tail of whole words', n == 4896 and c.left() == 0, n)
    return [line(values), line(tail)]


def world(d, check):
    c = Cursor(d)
    n = c.u32()
    rows = [c.u32s(3, 'i') for _ in range(n)]
    check('World.bin: n rows of 3 words fill the file', c.left() == 0, n)
    return [line(r) for r in rows]


def world2(d, check):
    n = struct.unpack_from('<I', d, 0)[0]
    check('World2.bin: u32 32768 and 32768 bytes', n == 32768 and len(d) == 4 + n, n)
    return [line(d[4:])]


def sets(d, check):
    c = Cursor(d)
    n = c.u32()
    check('sets.bin: 66 records of 112 bytes fill the file', len(d) == 4 + 112 * n, n)
    out = []
    for i in range(n):
        r = d[4 + 112 * i:4 + 112 * (i + 1)]
        if i == 0:
            check('sets.bin: the first record is all 0xCC', r == b'\xcc' * 112)
            out.append('-')
            continue
        check('sets.bin: 64 zero bytes end a record', not any(r[0x30:]), i)
        out.append(line(list(struct.unpack_from('<11I', r, 0)) + list(r[0x2C:0x30])))
    return out


def wpmod(d, check):
    c = Cursor(d)
    n = c.u32()
    out = []
    for _ in range(n):
        words = c.u32s(53)
        k = c.u32()
        entries = [c.u32s(6) for _ in range(k)]
        out.append(line(words) + ' | ' + ' '.join(line(e) for e in entries))
    check('wpmod.bin: the records fill the file', c.left() == 0, n)
    return out


def wea(d, check):
    c = Cursor(d)
    out = []
    while c.left():
        out.append(line(c.u32s(c.u32())))
    check('wea.bin: 256 lists fill the file', len(out) == 256, len(out))
    return out


def rust(d, check):
    c = Cursor(d)
    n = c.u32()
    out = [line(c.u32s(2 * c.u32())) for _ in range(n)]
    check('Rust.bin: the records fill the file', c.left() == 0, n)
    return out


def rows4(d, check, name, first_zero, last_zero):
    check(f'{name}: rows of 16 bytes fill the file', len(d) % 16 == 0, len(d))
    rows = [struct.unpack_from('<4I', d, 16 * i) for i in range(len(d) // 16)]
    if first_zero:
        check(f'{name}: the first word of a row is 0', all(r[0] == 0 for r in rows))
    if last_zero:
        check(f'{name}: the last word of a row is 0', all(r[3] == 0 for r in rows))
    return [line(r[1:3] if last_zero else (r[1:] if first_zero else r)) for r in rows]


def static10_18(d, check):
    n = struct.unpack_from('<I', d, 4)[0]
    check('static10_18.bin: "map", n, zeros to 0x100, n rows of 16 bytes',
          d[:4] == b'map\0' and not any(d[8:0x100]) and len(d) == 0x100 + 16 * n, n)
    return [line(struct.unpack_from('<4I', d, 0x100 + 16 * i)) for i in range(n)]


FILES = [
    ('bin/Balance.bin', balance),
    ('bin/World.bin', world),
    ('bin/World2.bin', world2),
    ('bin/sets.bin', sets),
    ('bin/wpmod.bin', wpmod),
    ('bin/wea.bin', wea),
    ('bin/Rust.bin', rust),
    ('bin/treppe.bin', lambda d, c: rows4(d, c, 'treppe.bin', False, False)),
    ('bin/static10_18.bin', static10_18),
    ('bin/merc.bin', lambda d, c: rows4(d, c, 'merc.bin', True, False)),
    ('bin/Addon/merc.bin', lambda d, c: rows4(d, c, 'Addon/merc.bin', True, False)),
    ('bin/MultiStart.bin', lambda d, c: rows4(d, c, 'MultiStart.bin', True, True)),
]


def main():
    ap = argparse.ArgumentParser(description='bin/ data tables: checks and per-file fingerprints.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--rows', help='write a TSV row per file')
    args = ap.parse_args()
    check = Check()
    rows = []
    for path, reader in FILES:
        data = open(os.path.join(args.game_dir, path), 'rb').read()
        try:
            lines = reader(data, check)
        except ValueError as e:
            check(f'{path} reads', False, str(e))
            continue
        rows.append((path, len(lines), f'{zlib.crc32(chr(10).join(lines).encode("ascii")):08x}'))
    check.report()
    for row in rows:
        print('  ' + '\t'.join(str(x) for x in row))
    if args.rows:
        with open(args.rows, 'w', encoding='ascii', newline='\n') as f:
            f.write('path\trecords\tcrc\n')
            for row in rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(rows)} rows -> {args.rows}')


if __name__ == '__main__':
    main()
