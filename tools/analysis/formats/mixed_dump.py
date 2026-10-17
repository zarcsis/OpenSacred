#!/usr/bin/env python3
"""PAK/Items.pak (the object types that Static.PAK, scripts and saves name) to the extent the world needs, and
PAK/mixed.pak (the sprites of the static objects, assembled from pieces of texture.pak atlases): layout checks over
every record and per-record tables for cross-checking the Rust readers.

Purpose (docs/02 §3.3, §6.1; PLAN.md §10, step 6): the reference prototype for `opensacred_formats::items` and
`opensacred_formats::mixed`. The layout follows openheilig (engine `formats/mixed.gd`, MIT; formats/world-sectors.md, CC
BY-SA 4.0), measured on the GOG files on 2026-10-17.

  items    "ITM" v5, 32768 records of 128 B back to back after the directory; directory type 2 for the 17408 used
           slots, 0 for the 15360 empty ones, whose records are all zeros but in slots 0 and 8191 (a few stray
           bytes). A used record: +0x00 u8 type (2 creatures
           and equipment, 9 world statics, 12 miniatures, 13 interactive objects, 25 trees and walls, ...), +0x10 u32
           the record of mixed.pak with the type's sprite (0 none), +0x37 char name[32] (Latin-1, as `Brücke`).
  mixed    "MIX" v0, 32096 records back to back after the directory, directory type 99, then 16 B to the end of the
           file (meaning unknown). Record 0 is 16 B: u32 99,
           u32 the file size - 16, u32 16, the fill 0xCCCCCCCC. A sprite: u32 n, u16 width, u16 height, i16 dx, i16
           dy (meaning unknown), the fill 0xCCCCCCCC; then n pieces of 64 B: char name[32] ("MIXnnn.444", the atlas),
           u32 texture.pak record, u16 x1, y1, x0, y0 (the piece in sprite pixels; pieces of 64-pixel blocks may run
           past width and height), u32 0, f32 u0, v0, u1, v1 (the piece in the atlas, as fractions of its size). A
           piece is as large in the atlas as in the sprite, at whole pixels: no resampling.

Example
  python3 -I tools/analysis/formats/mixed_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/mixed_dump.py --game-dir ./OriginalGame --items /tmp/items.tsv --sprites /tmp/sprites.tsv

Expected output (2.28): every check passes on all of its items; statistics follow. --items writes a row per items.pak
record (index, used, type, sprite, crc of the 128 bytes); --sprites a row per mixed.pak record (index, pieces, width,
height, crc of the record). Dependencies: numpy (tools/requirements.txt).
"""
import argparse
import collections
import os
import struct
import sys
import zlib

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ITEMS, MIXED, TEXTURES = 'PAK/Items.pak', 'PAK/mixed.pak', 'PAK/texture.pak'


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


def directory(d):
    count = struct.unpack_from('<I', d, 4)[0]
    return np.frombuffer(d, '<u4', 3 * count, 0x100).reshape(count, 3), 0x100 + 12 * count


def texture_sizes(path):
    """Width and height of every texture.pak record, (0, 0) for an empty one."""
    d = open(path, 'rb').read()
    dirs, _ = directory(d)
    sizes = []
    for kind, off, size in dirs:
        sizes.append(struct.unpack_from('<HH', d, off + 0x20) if off else (0, 0))
    return sizes


def main():
    ap = argparse.ArgumentParser(description='Check items.pak and mixed.pak; per-record tables.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--items', help='write one tab-separated row per items.pak record to this file')
    ap.add_argument('--sprites', help='write one tab-separated row per mixed.pak record to this file')
    args = ap.parse_args()
    c = Check()
    g = lambda rel: os.path.join(args.game_dir, rel)
    stats = collections.Counter()

    it = open(g(ITEMS), 'rb').read()
    dirs, first = directory(it)
    n = len(dirs)
    c('items: ITM v5, 32768 records of 128 B back to back after the directory',
      it[:4] == b'ITM\x05' and n == 32768 and bool(np.all(dirs[:, 1] == first + 128 * np.arange(n)))
      and bool(np.all(dirs[:, 2] == 128)) and len(it) == first + 128 * n)
    c('items: directory type 2 (used) or 0 (empty)', bool(np.all(np.isin(dirs[:, 0], (0, 2)))))
    mx = open(g(MIXED), 'rb').read()
    mdirs, mfirst = directory(mx)
    nm = len(mdirs)
    item_rows = []
    for i in range(n):
        r = it[first + 128 * i:first + 128 * (i + 1)]
        used = dirs[i, 0] == 2
        if not used:
            c('items: an empty slot is all zeros but slots 0 and 8191', not any(r) or i in (0, 8191), f'#{i}')
            item_rows.append((i, 0, 0, 0, zlib.crc32(r)))
            continue
        kind = r[0]
        sprite = struct.unpack_from('<I', r, 0x10)[0]
        name = r[0x37:0x57].split(b'\0')[0]
        stats[f'used items of type {kind}'] += 1
        c('items: sprite below the mixed.pak count', sprite < nm, f'#{i}')
        c('items: name without control characters (Latin-1)', all(b >= 0x20 and b != 0x7F for b in name), f'#{i}')
        item_rows.append((i, 1, kind, sprite, zlib.crc32(r)))

    sizes = texture_sizes(g(TEXTURES))
    sizes64 = mdirs[:, 2].astype(np.int64)
    c('mixed: MIX v0, directory type 99, records back to back after the directory, then 16 B',
      mx[:4] == b'MIX\x00' and bool(np.all(mdirs[:, 0] == 99))
      and bool(np.all(mdirs[:, 1] == mfirst + np.concatenate(([0], np.cumsum(sizes64[:-1])))))
      and len(mx) == mfirst + int(sizes64.sum()) + 16)
    c('mixed: record 0 is 99, the file size - 16, 16, the fill',
      struct.unpack_from('<4I', mx, mfirst) == (99, len(mx) - 16, 16, 0xCCCCCCCC))
    sprite_rows = []
    sprite_rows.append((0, 0, 0, 0, zlib.crc32(mx[mfirst:mfirst + 16])))
    for i in range(1, nm):
        off, size = int(mdirs[i, 1]), int(mdirs[i, 2])
        r = mx[off:off + size]
        count, width, height, dx, dy, pad = struct.unpack_from('<IHHhhI', r, 0)
        c('mixed: sprite of 16 + 64 n bytes, the fill at +0x0C', size == 16 + 64 * count and pad == 0xCCCCCCCC,
          f'#{i}')
        if size != 16 + 64 * count:
            print(f'  mixed #{i}: {size} bytes, n = {count}, header {r[:16].hex()}')
            sprite_rows.append((i, count, width, height, zlib.crc32(r)))
            continue
        stats['sprites with pieces'] += count > 0
        stats['pieces'] += count
        for k in range(count):
            p = r[16 + 64 * k:16 + 64 * (k + 1)]
            tex = struct.unpack_from('<I', p, 0x20)[0]
            x1, y1, x0, y0 = struct.unpack_from('<4H', p, 0x24)
            zero = struct.unpack_from('<I', p, 0x2C)[0]
            u0, v0, u1, v1 = struct.unpack_from('<4f', p, 0x30)
            where = f'#{i} piece {k}'
            c('mixed piece: texture in texture.pak with an image', tex < len(sizes) and sizes[tex] != (0, 0), where)
            c('mixed piece: x0 <= x1, y0 <= y1, u32 0', x0 <= x1 and y0 <= y1 and zero == 0, where)
            stats['pieces running past the sprite size'] += x1 > width or y1 > height
            tw, th = sizes[tex] if tex < len(sizes) else (0, 0)
            c('mixed piece: as large in the atlas as in the sprite, inside the atlas',
              0 <= u0 <= u1 <= 1 and 0 <= v0 <= v1 <= 1 and round((u1 - u0) * tw) == x1 - x0
              and round((v1 - v0) * th) == y1 - y0, where)
            c('mixed piece: atlas offsets are whole pixels', abs(u0 * tw - round(u0 * tw)) < 1e-3
              and abs(v0 * th - round(v0 * th)) < 1e-3, where)
        sprite_rows.append((i, count, width, height, zlib.crc32(r)))

    print(f'items: {n} records, {int((dirs[:, 0] == 2).sum())} used; mixed: {nm} records')
    c.report()
    print(f'  {dict(sorted(stats.items()))}')
    for path, rows, columns in ((args.items, item_rows, ('index', 'used', 'type', 'sprite', 'crc')),
                                (args.sprites, sprite_rows, ('index', 'pieces', 'width', 'height', 'crc'))):
        if path:
            os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
            with open(path, 'w', newline='\n') as out:
                out.write('\t'.join(columns) + '\n')
                for row in rows:
                    out.write('\t'.join(str(v) for v in row) + '\n')
            print(f'{len(rows)} rows -> {path}')


if __name__ == '__main__':
    main()
