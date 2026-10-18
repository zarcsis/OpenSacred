#!/usr/bin/env python3
"""The world of the GOG installation: World/sectors.keyx and sectors.wldx (the sector grid and the 64 x 64 floor cells
of every sector), PAK/tiles.pak (the ground tiles), World/Static.PAK and World/Floor.PAK (the static objects and the
floor overlays that hang off the cells in chains). Checks every statement below on every record and writes a
per-sector table for cross-checking the Rust reader.

Purpose (docs/02 §5; PLAN.md §10, step 6): the reference prototype for `opensacred_formats::world`. The layout follows the
openheilig description (formats/world-sectors.md, CC BY-SA 4.0) and Resacred's `rs_file.h` as quoted there, measured
on the GOG files on 2026-10-17.

  keyx     PAK-style 0x100 header: "WLK", version 5, u32 count 6050, u32 grid width 100, u32 grid height 100, then
           -18432 and 112152 (meaning unknown). count records of 768 B: +0x20 u32 grid key y*100 + x, +0x24 u32 sector
           id (Static.PAK +0x0C refers to it), +0x50 three chunks {u32 offset, u32 size, u32 type} at their offsets in
           an uncompressed world file with a 0x100 header: (o, 32, 1), (o + 32, 131072, 6), (o + 131104, n, 0); +0xEC u32
           offset and +0xF0 u32 compressed size of the sector in sectors.wldx; +0x108 u32 inflated size; +0x2C0 u8
           the area; +0x2D8 u32 the sector's music (a sound id, 0 none); +0x2E0 and +0x2E1 the two liquid materials
           (0..13) of the sector's WATERA and WATERB cells.
  sector   zlib stream inflating to the three chunks: +0x00 the name "Sector x,y" with a NUL, filled to 32 B (MSVC fill
           0xCD; some sectors carry another sector's name or none, so the keyx key is the position); +0x20 4096 cells of
           32 B, row by row (r * 64 + c), which share the heights of their common corners; +0x20020 the third chunk:
           n layer headers of 36 B (n = keyx +0x6C), the inside layers' cells after them in order, then 256 B of
           unknown use. A header: u32 x, u32 y (the global cell of the layer's first cell), u16 width, u16 height,
           u32 kind 6, u32 offset of the cells in the stream, u32 their size (width * height * 32), 12 zero bytes;
           the first is the ground's, the sector's own cells: at the sector's origin, of size 0 (docs/02 §5.2).
           The inside layers' cells are the cells' layout with only +0x18..+0x1F set (the second heights, the
           offsets, the flags 0 or 4 and the ground byte), but two cells' first static.
  cell     +0x00 u32 tile (tiles.pak index); +0x04 u32 first static (Static.PAK index, 0 none); +0x08 u32 0 in every cell
           (a removed object list); +0x0C u32 first floor overlay (Floor.PAK index, 0 none); +0x10 i8[4] corner heights
           left, top, right, bottom; +0x14 u8[4] corner light; +0x18 i8[4] a second corner height; +0x1C i8[2]; +0x1E u8
           flags (bits 0..2); +0x1F u8 nature (high nibble: 9 WATERA, 10 WATERB, ..., docs/02 §5.2) and class (low nibble).
  tiles    "ISO" v3, 90132 records of 64 B: char name[32] (the source "isoNNN.tga"), u32 texture.pak index, u32 diamond
           = index % 18, u32 0, u32 65536, 16 zero bytes; 18 consecutive tiles share a name and a texture.
  Static   "OBJ" v1, type 134, records of 64 B, record 0 all zeros: +0x00 u32 own index, +0x04 u32 type (Items.pak),
           +0x08 u32 flags, +0x0C u16 sector id, +0x0E i32 x and +0x12 i32 y in isometric screen pixels (48 (cx - cy),
           24 (cx + cy) for the global cell (cx, cy)), +0x17 u32 first child (0 or the fill 0xCDCDCDCD: none), +0x1B
           u32 parent (0 none), +0x1F u32 next static of the chain (0 ends it), +0x27 u32 trigger, +0x2B u8 mask (0 or one
           bit of 1..16; openheilig); a parent's children form a chain of
           their own.
  Floor    "OBJ" v1, type 135, records of 16 B, record 0 all zeros: +0x00 u32 own index, +0x04 u32 art tile (low 17
           bits) and mask tile (high 15 bits, 0 none), +0x08 u32 0 or the fill 0xCDCDCDCD, +0x0C u32 next overlay (0
           ends it).
  Chains   linked lists: no record is linked twice, a linked record is never a cell's head, no two cells share a
           head; the chains of the cells, with the child chains of their statics, reach every record but record 0 once,
           except 2 statics.

Example
  python3 -I tools/analysis/world/world_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/world/world_dump.py --game-dir ./OriginalGame --records /tmp/analysis/world_sectors.tsv

Expected output (2.28): "keyx: WLK v5, 6050 sectors, grid 100 x 100"; every check passes on all of its items
("sector: inflated size; the cells end inside the stream: 6050/6050"); statistics follow. --records writes one
tab-separated row per sector (columns in RECORD_COLUMNS). Dependencies: numpy (tools/requirements.txt).
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

KEYX, WLDX = 'World/sectors.keyx', 'World/sectors.wldx'
TILES, STATIC, FLOOR = 'PAK/tiles.pak', 'World/Static.PAK', 'World/Floor.PAK'
KEY_RECORD = 768
FILL = 0xCDCDCDCD  # MSVC's fill of uninitialized heap memory
CELLS_AT, CELL_SIZE, CELLS = 0x20, 32, 4096
CELL_CHUNK = CELL_SIZE * CELLS
# index, x, y: the sector's grid position; id: keyx +0x24; inflated: the inflated size; tail: bytes of the third
# chunk; liquids: keyx +0x2E0, +0x2E1; cells_crc: CRC-32 of the 4096 cells as stored; tail_crc: CRC-32 of the third
# chunk; statics, floors: records in the chains of the sector's cells; static_crc, floor_crc: CRC-32 over the cells in
# order of every chained record, u32 LE cell index then the record as stored (64 or 16 B), head first, each static
# followed by its child chain; layers: the inside layers (the third chunk's headers but the ground's); layers_crc:
# CRC-32 over the inside layers in order of u32 x, u32 y, u16 width, u16 height and the layer's cells as stored;
# area: keyx +0x2C0; music: keyx +0x2D8, the sound id of the sector's music (0 none).
RECORD_COLUMNS = ('index', 'x', 'y', 'id', 'inflated', 'tail', 'liquids', 'cells_crc', 'tail_crc', 'statics',
                  'static_crc', 'floors', 'floor_crc', 'layers', 'layers_crc', 'area', 'music')
LAYER_HEADER, LAYER_KIND, LAYER_TRAILER = 36, 6, 256


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


def read_pak(path, magic, version, record_size, c, kind=None):
    """A PAK container of fixed-size records: (records as a bytes object of count * record_size, directory as an
    (n, 3) array), checking that the records lie back to back after the directory."""
    d = open(path, 'rb').read()
    count = struct.unpack_from('<I', d, 4)[0]
    rel = os.path.basename(path)
    c(f'{rel}: magic {magic.decode()} v{version}', d[:3] == magic and d[3] == version)
    directory = np.frombuffer(d, '<u4', 3 * count, 0x100).reshape(count, 3)
    first = 0x100 + 12 * count
    expected = first + record_size * np.arange(count, dtype=np.int64)
    c(f'{rel}: entries at back-to-back {record_size}-byte slots after the directory, size {record_size}',
      bool(np.all(directory[:, 1] == expected) and np.all(directory[:, 2] == record_size)))
    if kind is not None:
        c(f'{rel}: entry type {kind}', bool(np.all(directory[:, 0] == kind)))
    c(f'{rel}: file ends after the last slot', len(d) == first + record_size * count)
    return d[first:first + record_size * count], directory


def chain_checks(name, heads, nexts, c):
    """Linked-list checks for the next links `nexts` (index -> next, 0 ends) and the cell heads `heads`."""
    linked = nexts[nexts != 0]
    c(f'{name}: links in range', bool(np.all(linked < len(nexts))))
    c(f'{name}: no record linked twice', len(np.unique(linked)) == len(linked))
    c(f'{name}: a linked record is never a head', len(np.intersect1d(np.unique(heads[heads != 0]), linked)) == 0)
    c(f'{name}: no two cells share a head', len(np.unique(heads[heads != 0])) == int((heads != 0).sum()))


def walk(head, nexts, limit):
    out = []
    while head and len(out) <= limit:
        out.append(int(head))
        head = nexts[head]
    return out


def main():
    ap = argparse.ArgumentParser(description='Check the world files and write a per-sector table.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--records', help='write one tab-separated row per sector to this file')
    args = ap.parse_args()
    c = Check()
    g = lambda rel: os.path.join(args.game_dir, rel)

    keyx = open(g(KEYX), 'rb').read()
    wldx = open(g(WLDX), 'rb').read()
    count, width, height, h3, h4 = struct.unpack_from('<IIIii', keyx, 4)
    print(f'keyx: {keyx[:3].decode()} v{keyx[3]}, {count} sectors, grid {width} x {height}, then {h3}, {h4}')
    c('keyx: WLK v5, 100 x 100 grid, count records of 768 B fill the file',
      keyx[:4] == b'WLK\x05' and (width, height) == (100, 100) and len(keyx) == 0x100 + KEY_RECORD * count)

    tiles, _ = read_pak(g(TILES), b'ISO', 3, 64, c)
    n_tiles = len(tiles) // 64
    t = np.frombuffer(tiles, np.uint8).reshape(n_tiles, 64)
    tw = t.view('<u4')
    names = [bytes(r[:32]).split(b'\0')[0] for r in t]
    c('tiles: diamond == index % 18, then 0, 65536 and 16 zero bytes',
      bool(np.all(tw[:, 9] == np.arange(n_tiles) % 18) and np.all(tw[:, 10] == 0) and np.all(tw[:, 11] == 65536)
           and np.all(tw[:, 12:] == 0)))
    groups = [(names[k], int(tw[k, 8])) for k in range(0, n_tiles, 18)]
    c('tiles: 18 consecutive tiles share a name and a texture',
      all(names[k] == groups[k // 18][0] and int(tw[k, 8]) == groups[k // 18][1] for k in range(n_tiles)))
    c('tiles: names are isoNNN.tga', all(n.lower().startswith(b'iso') and n.lower().endswith(b'.tga') for n in names))

    statics, _ = read_pak(g(STATIC), b'OBJ', 1, 64, c, kind=134)
    n_static = len(statics) // 64
    s = np.frombuffer(statics, np.uint8).reshape(n_static, 64)
    own = s[:, 0:4].copy().view('<u4').ravel()
    static_next = s[:, 0x1F:0x23].copy().view('<u4').ravel()
    child = s[:, 0x17:0x1B].copy().view('<u4').ravel()
    parent = s[:, 0x1B:0x1F].copy().view('<u4').ravel()
    child = np.where(child == FILL, 0, child)
    c('Static: first child 0, the fill or a record; parent 0 or a record',
      bool(np.all(child < n_static) and np.all(parent < n_static)))
    kids = np.nonzero(parent)[0]
    c('Static: a parent\'s child chain holds exactly the records naming it as parent',
      sorted(k for p in np.unique(parent[kids]) for k in walk(child[p], static_next, n_static)) == sorted(kids))
    c('Static: record 0 all zeros, every other holds its own index',
      not np.any(s[0]) and bool(np.all(own[1:] == np.arange(1, n_static))))
    mask = s[1:, 0x2B]
    c('Static: +0x2B (the mask) is 0 or one bit of 1..16', bool(np.all(np.isin(mask, [0, 1, 2, 4, 8, 16]))))
    trigger = s[1:, 0x27:0x2B].copy().view('<u4').ravel()

    floors, _ = read_pak(g(FLOOR), b'OBJ', 1, 16, c, kind=135)
    n_floor = len(floors) // 16
    f = np.frombuffer(floors, '<u4').reshape(n_floor, 4)
    floor_next = f[:, 3]
    c('Floor: record 0 all zeros, every other holds its own index; +0x08 zero',
      not np.any(f[0]) and bool(np.all(f[1:, 0] == np.arange(1, n_floor)) and np.all(np.isin(f[1:, 2], (0, FILL)))))
    art, mask = f[1:, 1] & 0x1FFFF, f[1:, 1] >> 17
    c('Floor: art tile (low 17 bits) and mask tile (high 15, 0 none) in tiles.pak',
      bool(np.all(art < n_tiles) and np.all(mask < n_tiles)))

    rows = []
    all_static_heads, all_floor_heads = [], []
    stats = collections.Counter()
    stats['statics with mask 1'] = int(np.count_nonzero(mask == 1))
    stats['statics with a trigger (+0x27)'] = int(np.count_nonzero(trigger))
    seen_keys = set()
    for i in range(count):
        r = keyx[0x100 + KEY_RECORD * i:0x100 + KEY_RECORD * (i + 1)]
        key, sector_id = struct.unpack_from('<II', r, 0x20)
        x, y = key % 100, key // 100
        where = f'sector {i} ({x},{y})'
        c('keyx: grid key below 10000 and unique', key < 10000 and key not in seen_keys, where)
        seen_keys.add(key)
        chunks = [struct.unpack_from('<3I', r, 0x50 + 12 * k) for k in range(3)]
        o = chunks[0][0]
        c('keyx: chunks (o, 32, 1), (o + 32, 131072, 6), (o + 131104, n, 0)',
          chunks[0][1:] == (32, 1) and chunks[1] == (o + 32, 131072, 6) and chunks[2][0] == o + 131104
          and chunks[2][2] == 0, where)
        off, csize = struct.unpack_from('<II', r, 0xEC)
        inflated = struct.unpack_from('<I', r, 0x108)[0]
        liquids = (r[0x2E0], r[0x2E1])
        area, music = r[0x2C0], struct.unpack_from('<I', r, 0x2D8)[0]
        c('keyx: liquid materials 0..13', max(liquids) <= 13, where)
        d = zlib.decompress(wldx[off:off + csize])
        name = d[:32].split(b'\0')[0]
        c('sector: inflated size; the cells end inside the stream',
          len(d) == inflated and len(d) >= CELLS_AT + CELL_CHUNK, where)
        stats['sectors named after another sector or not at all'] += name != f'Sector {x},{y}'.encode()
        cell_bytes = d[CELLS_AT:CELLS_AT + CELL_CHUNK]
        tail = d[CELLS_AT + CELL_CHUNK:]
        n_layers = chunks[2][1]
        first = struct.unpack_from('<IIHHIIIIII', d, CELLS_AT + CELL_CHUNK)
        c('layers: the first header is the ground\'s, at the sector\'s origin, of size 0',
          first == (64 * x, 64 * y, 0, 0, LAYER_KIND, 0, 0, 0, 0, 0), where)
        layers_crc, end = 0, CELLS_AT + CELL_CHUNK + LAYER_HEADER * n_layers
        for k in range(1, n_layers):
            lx, ly, w, h, kind, at, size, *zeros = struct.unpack_from('<IIHHIIIIII', d,
                                                                         CELLS_AT + CELL_CHUNK + LAYER_HEADER * k)
            c('layers: kind 6, size width * height * 32, cells right after the last, zeros, within the sector',
              kind == LAYER_KIND and size == 32 * w * h and at == end and zeros == [0, 0, 0]
              and 64 * x <= lx and lx + w <= 64 * x + 64 and 64 * y <= ly and ly + h <= 64 * y + 64, where)
            raw_layer = np.frombuffer(d[at:at + size], np.uint8).reshape(w * h, 32)
            c('layers: the cells set only +0x18..+0x1F (flags 0 or 4) and, in two, their first static',
              bool(np.all(raw_layer[:, [i for i in range(0x18) if not 4 <= i < 8]] == 0)
                   and np.all(np.isin(raw_layer[:, 0x1E], (0, 4)))), where)
            layers_crc = zlib.crc32(struct.pack('<IIHH', lx, ly, w, h) + d[at:at + size], layers_crc)
            stats['inside layers'] += 1
            stats['inside layers\' cells with a static'] += int(np.count_nonzero(raw_layer[:, 4:8].any(axis=1)))
            end = at + size
        c('layers: 256 bytes after the last layer\'s cells end the stream', len(d) - end == LAYER_TRAILER, where)
        cells = np.frombuffer(cell_bytes, '<u4').reshape(CELLS, 8)
        c('cell: tile in tiles.pak, first static in Static.PAK, +0x08 zero, first floor in Floor.PAK',
          bool(np.all(cells[:, 0] < n_tiles) and np.all(cells[:, 1] < n_static) and np.all(cells[:, 2] == 0)
               and np.all(cells[:, 3] < n_floor)), where)
        raw = np.frombuffer(cell_bytes, np.uint8).reshape(CELLS, 32)
        c('cell: +0x1E uses bits 0..2 only', bool(np.all(raw[:, 0x1E] < 8)), where)
        heights = raw[:, 0x10:0x14].view(np.int8).reshape(64, 64, 4)
        stats['shared corners'] += 2 * 64 * 63
        stats['shared corners that differ'] += int((heights[:, :-1, 3] != heights[:, 1:, 0]).sum()
                                                   + (heights[:-1, :, 0] != heights[1:, :, 1]).sum())
        all_static_heads.append(cells[:, 1])
        all_floor_heads.append(cells[:, 3])

        static_crc = floor_crc = 0
        n_statics = n_floors = 0
        for cell in np.nonzero(cells[:, 1])[0]:
            for head in walk(cells[cell, 1], static_next, n_static):
                for k in [head] + walk(child[head], static_next, n_static):
                    static_crc = zlib.crc32(struct.pack('<I', cell) + statics[64 * k:64 * k + 64], static_crc)
                    n_statics += 1
        for cell in np.nonzero(cells[:, 3])[0]:
            for k in walk(cells[cell, 3], floor_next, n_floor):
                floor_crc = zlib.crc32(struct.pack('<I', cell) + floors[16 * k:16 * k + 16], floor_crc)
                n_floors += 1
        stats['statics in chains'] += n_statics
        stats['floor overlays in chains'] += n_floors
        stats['sectors with liquid cells'] += bool(np.any(np.isin(raw[:, 0x1F] >> 4, (9, 10))))
        rows.append(dict(index=key, x=x, y=y, id=sector_id, inflated=inflated, tail=len(tail),
                         liquids=f'{liquids[0]},{liquids[1]}', cells_crc=zlib.crc32(cell_bytes),
                         tail_crc=zlib.crc32(tail), statics=n_statics, static_crc=static_crc, floors=n_floors,
                         floor_crc=floor_crc, layers=max(n_layers - 1, 0), layers_crc=layers_crc, area=area, music=music))

    static_heads = np.concatenate(all_static_heads)
    floor_heads = np.concatenate(all_floor_heads)
    chain_checks('Static chains', static_heads, static_next, c)
    chain_checks('Floor chains', floor_heads, floor_next, c)
    c('Static: the chains of the cells and their children reach every record but 0 and 2 others, each once',
      stats['statics in chains'] == n_static - 3)
    c('Floor: the chains of the cells reach every record but 0, each once',
      stats['floor overlays in chains'] == n_floor - 1)

    print(f'tiles: {n_tiles} ({len(groups)} names); Static: {n_static} records; Floor: {n_floor} records')
    c.report()
    print(f'  {dict(stats)}; static heads {int((static_heads != 0).sum())}, floor heads {int((floor_heads != 0).sum())}')

    if args.records:
        os.makedirs(os.path.dirname(os.path.abspath(args.records)), exist_ok=True)
        with open(args.records, 'w', newline='\n') as out:
            out.write('\t'.join(RECORD_COLUMNS) + '\n')
            for row in rows:
                out.write('\t'.join(str(row[col]) for col in RECORD_COLUMNS) + '\n')
        print(f'records: {len(rows)} rows -> {args.records}')


if __name__ == '__main__':
    main()
