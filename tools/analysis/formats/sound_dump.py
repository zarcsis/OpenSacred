#!/usr/bin/env python3
"""PAK/sound.pak (the samples: RIFF WAV and MP3), PAK/sndProfiles.pak (the sound profiles that name them) and the table
of sound names in Sacred.exe: layout checks over every record and per-record tables for cross-checking the Rust readers.

Purpose (docs/02 §10.1–10.2; PLAN.md §7): the reference prototype for `opensacred_formats::sound`. The container layout of
sound.pak was confirmed by snd_ls.py; this script adds the profile layout and the tables, measured on the GOG files
on 2026-10-17.

  sound    "SND" v1, a directory of 50000 {u32 flags, u32 offset, u32 size}; 6598 used slots (offset != 0), sample id =
           slot. The data follows the directory back to back to the end of the file. flags 32: a RIFF WAV whose RIFF size
           is the record size, with a "fmt " chunk (tag 1 PCM or 0x11 IMA ADPCM) and a "data" chunk; flags 33: MPEG-1
           Layer III frames, after up to 417 zero bytes.
  profiles "SPF" v1, a directory of 8192 {u32 type 35, u32 offset, u32 184}, records of 184 B back to back; a used
           record has a name: char name[32], u32 1, 10 zero u16, then 8 groups of 8 u16 sample ids. A group holds its
           variants repeated in turn to fill the 8 slots (3 variants: a b c a b c a b), or nothing.
  names    Sacred.exe, .data, VA 0x964870: records of 68 B {u32 id; char name[64]}, zeros after the name, from id 0
           SOUND_FX_INVALID to the last whose name starts with SOUND_FX_ (measured 2026-10-17): the sound ids' names.

Example
  python3 -I tools/analysis/formats/sound_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/sound_dump.py --game-dir ./OriginalGame --samples /tmp/s.tsv --profiles /tmp/p.tsv
  python3 -I tools/analysis/formats/sound_dump.py --game-dir ./OriginalGame --names /tmp/n.tsv

Expected output (2.28): every check passes on all of its items; statistics follow. --samples writes a row per used
sound.pak slot (id, flags, size, kind, crc of the record); --profiles a row per used profile (slot, name, the groups'
variants as space-separated ids joined by ";"); --names a row per name of the table in Sacred.exe (id, name), in its
order. Dependencies: numpy (tools/requirements.txt).
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

SOUND, PROFILES, EXE = 'PAK/sound.pak', 'PAK/sndProfiles.pak', 'Sacred.exe'
FIRST_NAME, NAME_RECORD, NAME_PREFIX = b'\0\0\0\0SOUND_FX_INVALID\0', 68, b'SOUND_FX_'
WAV, MP3 = 32, 33
PROFILE_SIZE, GROUPS, SLOTS = 184, 8, 8


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


def wav_chunks(b):
    """The (id, offset, length) of the chunks of a RIFF WAV, or None if they do not fill it."""
    p, out = 12, []
    while p + 8 <= len(b):
        cid, length = b[p:p + 4], struct.unpack_from('<I', b, p + 4)[0]
        out.append((cid, p + 8, length))
        p += 8 + length + (length & 1)
    return out if p >= len(b) else None


def variants(group):
    """The variants of a group of 8 slots: the shortest prefix that the slots repeat in turn; None if they do not."""
    if not any(group):
        return []
    for n in range(1, SLOTS + 1):
        if all(group[k] == group[k % n] for k in range(SLOTS)) and 0 not in group[:n]:
            return list(group[:n])
    return None


def main():
    ap = argparse.ArgumentParser(description='Check sound.pak and sndProfiles.pak; per-record tables.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--samples', help='write one tab-separated row per used sound.pak slot to this file')
    ap.add_argument('--profiles', help='write one tab-separated row per used profile to this file')
    ap.add_argument('--names', help='write one tab-separated row per sound name of Sacred.exe to this file')
    args = ap.parse_args()
    c = Check()
    stats = collections.Counter()
    g = lambda rel: os.path.join(args.game_dir, rel)

    d = open(g(SOUND), 'rb').read()
    count = struct.unpack_from('<I', d, 4)[0]
    dirs = np.frombuffer(d, '<u4', 3 * count, 0x100).reshape(count, 3)
    first = 0x100 + 12 * count
    used = np.nonzero(dirs[:, 1])[0]
    order = used[np.argsort(dirs[used, 1])]
    ends = dirs[order, 1].astype(np.int64) + dirs[order, 2]
    c('sound: SND v1, 50000 slots; the used ones back to back from the directory to the end of the file',
      d[:4] == b'SND\x01' and count == 50000 and int(dirs[order[0], 1]) == first
      and bool(np.all(dirs[order[1:], 1] == ends[:-1])) and int(ends[-1]) == len(d))
    c('sound: an unused slot is all zeros', bool(np.all(dirs[dirs[:, 1] == 0] == 0)))
    sample_rows = []
    for i in used:
        flags, off, size = (int(v) for v in dirs[i])
        b = d[off:off + size]
        where = f'#{i}'
        if flags == WAV:
            chunks = wav_chunks(b)
            ids = [cid for cid, _, _ in chunks or []]
            ok = (b[:4] == b'RIFF' and b[8:12] == b'WAVE' and struct.unpack_from('<I', b, 4)[0] + 8 == size
                  and chunks is not None and b'fmt ' in ids and b'data' in ids)
            c('sound WAV: RIFF WAVE of the record size, chunks fill it, with fmt and data', ok, where)
            tag = next((struct.unpack_from('<H', b, o)[0] for cid, o, _ in chunks or [] if cid == b'fmt '), None)
            c('sound WAV: tag 1 (PCM) or 0x11 (IMA ADPCM)', tag in (1, 0x11), where)
            kind = {1: 'pcm', 0x11: 'adpcm'}.get(tag, 'wav')
        else:
            c('sound: flags 32 or 33', flags == MP3, where)
            lead = len(b) - len(b.lstrip(b'\0'))
            c('sound MP3: an MPEG-1 Layer III frame after at most 417 zero bytes',
              lead <= 417 and b[lead] == 0xFF and (b[lead + 1] & 0xFE) == 0xFA, where)
            stats[f'MP3 with {lead} leading zero bytes'] += 1
            kind = 'mp3'
        stats[f'samples {kind}'] += 1
        sample_rows.append((int(i), flags, size, kind, zlib.crc32(b)))

    p = open(g(PROFILES), 'rb').read()
    pcount = struct.unpack_from('<I', p, 4)[0]
    pdirs = np.frombuffer(p, '<u4', 3 * pcount, 0x100).reshape(pcount, 3)
    pfirst = 0x100 + 12 * pcount
    c('profiles: SPF v1, 8192 records of 184 B back to back after the directory, type 35',
      p[:4] == b'SPF\x01' and pcount == 8192 and bool(np.all(pdirs[:, 0] == 35))
      and bool(np.all(pdirs[:, 1] == pfirst + PROFILE_SIZE * np.arange(pcount)))
      and bool(np.all(pdirs[:, 2] == PROFILE_SIZE)) and len(p) == pfirst + PROFILE_SIZE * pcount)
    profile_rows = []
    sample_ids = {r[0] for r in sample_rows}
    referenced = set()
    for k in range(pcount):
        r = p[pfirst + PROFILE_SIZE * k:pfirst + PROFILE_SIZE * (k + 1)]
        name = r[:32].split(b'\0')[0]
        if not name:
            c('profiles: a record without a name is all zeros', not any(r), f'#{k}')
            continue
        one = struct.unpack_from('<I', r, 32)[0]
        ids = struct.unpack_from('<74H', r, 36)
        c('profiles: u32 1 after the name, then 10 zero u16', one == 1 and not any(ids[:10]), f'#{k}')
        groups = [ids[10 + SLOTS * j:10 + SLOTS * (j + 1)] for j in range(GROUPS)]
        found = [variants(grp) for grp in groups]
        c('profiles: every group repeats its variants in turn or is empty', None not in found, f'#{k}')
        for v in found:
            for s in v or []:
                referenced.add(s)
        stats['profiles'] += 1
        profile_rows.append((k, name.decode('latin-1'), ';'.join(' '.join(map(str, v or [])) for v in found)))
    stats['profile sample ids'] = len(referenced)
    stats['profile sample ids not in sound.pak'] = len(referenced - sample_ids)

    e = open(g(EXE), 'rb').read()
    at = e.find(FIRST_NAME)
    c('names: one table in Sacred.exe, from id 0 SOUND_FX_INVALID', at >= 0 and e.find(FIRST_NAME, at + 1) < 0)
    name_rows = []
    while at >= 0 and e[at + 4:at + 4 + len(NAME_PREFIX)] == NAME_PREFIX:
        raw = e[at + 4:at + NAME_RECORD]
        name = raw.split(b'\0')[0]
        c('names: {u32 id; char name[64]}, ASCII, zeros after the name', name.isascii() and not any(raw[len(name):]),
          f'#{len(name_rows)}')
        name_rows.append((struct.unpack_from('<I', e, at)[0], name.decode('ascii')))
        at += NAME_RECORD
    named = {i for i, _ in name_rows}
    c('names: every name is distinct', len({n for _, n in name_rows}) == len(name_rows))
    c('names: every sample of sound.pak is named', sample_ids <= named)
    stats['names'] = len(name_rows)
    stats['names of an id named twice'] = len(name_rows) - len(named)
    stats['profile ids named'] = len(referenced & named)

    print(f'sound: {count} slots, {len(used)} used; profiles: {pcount} slots')
    c.report()
    print(f'  {dict(sorted(stats.items()))}')
    for path, rows, columns in ((args.samples, sample_rows, ('id', 'flags', 'size', 'kind', 'crc')),
                                (args.profiles, profile_rows, ('slot', 'name', 'groups')),
                                (args.names, name_rows, ('id', 'name'))):
        if path:
            os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
            with open(path, 'w', newline='\n', encoding='utf-8') as out:
                out.write('\t'.join(columns) + '\n')
                for row in rows:
                    out.write('\t'.join(str(v) for v in row) + '\n')
            print(f'{len(rows)} rows -> {path}')


if __name__ == '__main__':
    main()
