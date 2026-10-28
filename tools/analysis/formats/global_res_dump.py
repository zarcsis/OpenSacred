#!/usr/bin/env python3
"""The global.res string table ("SZ\\0\\0", scripts/us/global.res): layout checks over every record and a per-record
table for cross-checking the Rust reader.

Purpose (docs/02 §8): the reference prototype for `opensacred_formats::text`. Measured on the GOG file on
2026-10-17, correcting the earlier count of 23,122 strings:

  "SZ\\0\\0", then 23123 records of 16 B {u32 id, u32 offset, u32 flags, u32 length}, sorted by id, ids unique; then
  the UTF-16LE strings without terminators. An offset counts from byte 4, the start of the records: string k lies at
  file offset 4 + offset[k] and runs length[k] bytes (even), each string right after the previous one, the first right
  after the records, the last to the end of the file. flags is 0 but in 4 records (1, 1, 3, 5; meaning unknown). Every
  string decodes as UTF-16 without unpaired surrogates.

Example
  uv run python -I tools/analysis/formats/global_res_dump.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/formats/global_res_dump.py --game-dir ./OriginalGame --records /tmp/strings.tsv

Expected output (2.28): "23123 strings"; every check passes; --records writes one row per record (id, flags, length,
CRC-32 of the UTF-8 encoding of the string). Prints no text of the game. Dependencies: standard library.
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


def main():
    ap = argparse.ArgumentParser(description='Check global.res and write a per-record table.')
    ap.add_argument('path', nargs='?', help='global.res (default <game-dir>/scripts/us/global.res)')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--records', help='write one tab-separated row per record to this file')
    args = ap.parse_args()
    path = args.path or os.path.join(args.game_dir, 'scripts', 'us', 'global.res')
    d = open(path, 'rb').read()
    c = Check()
    c('magic SZ\\0\\0', d[:4] == b'SZ\0\0')
    # The records end where the first string starts: at 4 + the first offset.
    first = struct.unpack_from('<I', d, 8)[0]
    n = first // 16
    c('the first string follows the records', first % 16 == 0)
    recs = [struct.unpack_from('<4I', d, 4 + 16 * i) for i in range(n)]
    ids = [r[0] for r in recs]
    c('ids rise strictly', all(a < b for a, b in zip(ids, ids[1:])))
    rows = []
    flags = collections.Counter()
    for k, (sid, off, flag, length) in enumerate(recs):
        end = recs[k + 1][1] if k + 1 < n else len(d) - 4
        c('string k runs from 4 + offset[k] for length[k] (even) bytes up to the next one or the end of the file',
          off + length == end and length % 2 == 0, f'#{k}')
        raw = d[4 + off:4 + off + length]
        try:
            text = raw.decode('utf-16-le')
            ok = True
        except UnicodeDecodeError:
            text, ok = '', False
        c('string decodes as UTF-16LE', ok, f'#{k}')
        flags[flag] += 1
        rows.append((sid, flag, length, zlib.crc32(text.encode('utf-8'))))
    print(f'{n} strings, file {len(d)} bytes; flags {dict(flags)}')
    c.report()
    if args.records:
        os.makedirs(os.path.dirname(os.path.abspath(args.records)), exist_ok=True)
        with open(args.records, 'w', newline='\n') as out:
            out.write('id\tflags\tlength\tcrc\n')
            for row in rows:
                out.write('\t'.join(map(str, row)) + '\n')
        print(f'records: {len(rows)} rows -> {args.records}')


if __name__ == '__main__':
    main()
