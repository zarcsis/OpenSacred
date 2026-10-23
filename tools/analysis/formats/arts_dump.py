#!/usr/bin/env python3
"""The combat arts' two tables compiled into Sacred.exe: layout checks over every record and per-record rows for
cross-checking the Rust reader.

Purpose (docs/02 §6.5, docs/re/arts.md; PLAN.md §7): the reference prototype for `opensacred_formats::arts`. Measured on
the GOG Sacred.exe 2.28 (the manifest's) on 2026-10-23; the image is read as pefile lays it out in memory, so the
virtual addresses are the code's.

  spells       VA 0x952A38: records of 0x94 B, the record 0 empty (id 0, icon INVALID.TGA), then 70 to 0x955344.
               u16 id; u16 0; u32 name index; u32 description index; char icon[32], zeros after the name; u32 0 (the
               icon's texture handle at run time); u32 sprites[3]; u16 launch type; u16 regeneration[2] (base, step);
               u16 effects[13][2]; u16 values[3][2]; u16 caps[3]; u16 element; u16 0; u32 classes; u8 flags; u8 0[3].
  weapon arts  VA 0x94FDB0: records of 0x78 B, the record 0 empty, then 94 to 0x952A38, where the spells start.
               The same first 0x3C B, then u32 +0x3C; f32 regeneration[2]; i32 distance; u8 launch type; u8 0[3];
               f32 factor[2]; f32 +0x58[2]; i32 +0x60[2]; i32 +0x68[2]; u32 classes; u8 flags; u8 0[3].

Example
  python3 -I tools/analysis/formats/arts_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/arts_dump.py --game-dir ./OriginalGame --rows /tmp/arts.tsv

Expected output (2.28): every check passes on all of its items; the counts and a few records follow. --rows writes a
row per record, the spells then the weapon arts, each in its table's order: the table, the record's index and every
field, the pairs and the arrays as space-separated numbers, the floats as the hexadecimal of their bits. Dependencies:
pefile, capstone (tools/requirements.txt, through tools/analysis/common/sacred_pe.py).
"""
import argparse
import collections
import hashlib
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, read_pe_image

EXE = 'Sacred.exe'
MANIFEST = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..', 'docs', 'manifest-gog-2.28.txt')
SPELLS, SPELL_SIZE, SPELL_COUNT = 0x952A38, 0x94, 70
ARTS, ART_SIZE, ART_COUNT = 0x94FDB0, 0x78, 94
ICON = 32
INVALID = b'INVALID.TGA'


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


def words(fmt, r, at):
    return list(struct.unpack_from('<' + fmt, r, at))


def bits(r, at, n):
    """n floats at `at` as the hexadecimal of their bits."""
    return [f'{w:08x}' for w in struct.unpack_from(f'<{n}I', r, at)]


def joined(values):
    return ' '.join(str(v) for v in values)


def head(c, r, where):
    """The fields at +0x00..+0x3C of a record of either table."""
    icon = r[0x0C:0x0C + ICON].split(b'\0')[0]
    c('a record: +0x02 0, the icon ASCII with zeros after it, its handle +0x2C 0',
      r[2:4] == b'\0\0' and icon.isascii() and not any(r[0x0C + len(icon):0x0C + ICON]) and r[0x2C:0x30] == b'\0' * 4,
      where)
    rid, = struct.unpack_from('<H', r, 0)
    name, description = struct.unpack_from('<II', r, 4)
    return rid, [rid, name, description, icon.decode('ascii'), joined(words('3I', r, 0x30))]


def table(c, img, va, size, count, what):
    """The records 1..count of a table, after its empty record 0."""
    first = img.bytes(va, size)
    c(f'{what}: the record 0 is the empty one, id 0 and the icon INVALID.TGA',
      first[:2] == b'\0\0' and first[0x0C:0x0C + ICON].split(b'\0')[0] == INVALID)
    return [img.bytes(va + size * k, size) for k in range(1, count + 1)]


def main():
    ap = argparse.ArgumentParser(description="The combat arts' two tables in Sacred.exe: checks and per-record rows.")
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--rows', help='write a TSV row per record')
    args = ap.parse_args()
    c = Check()
    path = os.path.join(args.game_dir, EXE)
    with open(MANIFEST, encoding='ascii') as manifest:
        entries = [line.rstrip().split('  ', 1) for line in manifest]
    expected = next(entry[0] for entry in entries if entry[1:] == [EXE])
    c("Sacred.exe: the manifest's (GOG 2.28)", hashlib.sha256(open(path, 'rb').read()).hexdigest() == expected)
    img = read_pe_image(path)
    rows = []
    spells = {}
    for k, r in enumerate(table(c, img, SPELLS, SPELL_SIZE, SPELL_COUNT, 'spells'), 1):
        where = f'spell #{k}'
        rid, fields = head(c, r, where)
        c('spell: +0x8A 0 and +0x91..+0x93 0', r[0x8A:0x8C] == b'\0\0' and r[0x91:0x94] == b'\0' * 3, where)
        c('spell: flags within bits 0 to 2', r[0x90] < 8, where)
        effects = words('26H', r, 0x42)
        values = words('6H', r, 0x76)
        fields += [struct.unpack_from('<H', r, 0x3C)[0], joined(words('2H', r, 0x3E)), joined(effects), joined(values),
                   joined(words('3H', r, 0x82)), struct.unpack_from('<H', r, 0x88)[0],
                   struct.unpack_from('<I', r, 0x8C)[0], r[0x90]]
        spells[rid] = fields
        rows.append(['spell', k] + fields)
    arts = {}
    for k, r in enumerate(table(c, img, ARTS, ART_SIZE, ART_COUNT, 'weapon arts'), 1):
        where = f'weapon art #{k}'
        rid, fields = head(c, r, where)
        c('weapon art: +0x4D..+0x4F 0 and +0x75..+0x77 0', r[0x4D:0x50] == b'\0' * 3 and r[0x75:0x78] == b'\0' * 3,
          where)
        fields += [struct.unpack_from('<I', r, 0x3C)[0], joined(bits(r, 0x40, 2)), struct.unpack_from('<i', r, 0x48)[0],
                   r[0x4C], joined(bits(r, 0x50, 2)), joined(bits(r, 0x58, 2)), joined(words('2i', r, 0x60)),
                   joined(words('2i', r, 0x68)), struct.unpack_from('<I', r, 0x70)[0], r[0x74]]
        arts[rid] = fields
        rows.append(['weapon-art', k] + fields)
    c('the ids: the spells 1 to 72 but 54 and 68, each once',
      sorted(spells) == [i for i in range(1, 73) if i not in (54, 68)] and len(spells) == SPELL_COUNT)
    c('the ids: the weapon arts within 1000 to 1103, each once',
      all(1000 <= i <= 1103 for i in arts) and len(arts) == ART_COUNT)
    c('the tables: the weapon arts end where the spells start', ARTS + ART_SIZE * (ART_COUNT + 1) == SPELLS)

    print(f'{EXE}: {len(spells)} spells, {len(arts)} weapon arts')
    c.report()
    print(f'  weapon art ids missing from 1000 to 1103: {[i for i in range(1000, 1104) if i not in arts]}')
    for rid in (2, 18):
        print(f'  spell {rid}: {spells[rid][1:]}')
    for rid in (1057, 1059, 1071):
        print(f'  weapon art {rid}: {arts[rid][1:]}')
    if args.rows:
        os.makedirs(os.path.dirname(os.path.abspath(args.rows)), exist_ok=True)
        with open(args.rows, 'w', encoding='ascii', newline='\n') as f:
            f.write('table\tindex\tfields\n')
            for row in rows:
                f.write('\t'.join(str(x) for x in row) + '\n')
        print(f'{len(rows)} rows -> {args.rows}')


if __name__ == '__main__':
    main()
