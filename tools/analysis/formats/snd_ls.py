#!/usr/bin/env python3
"""sound.pak (SND v1): header, directory 50,000×12 ({flags, offset, size}), parsing of each entry as RIFF WAV
(codec, channels, sample rate, blockAlign, duration) or MP3 (header of the first MPEG frame); cross-check of sample ids
with the sndProfiles.pak profiles.

Purpose (docs/02-data-formats.md, §10.2; PLAN.md §13): settles the question of the sound.pak container and codecs
and was the first reference for the reader `opensacred_formats::sound` (now cross-checked against sound_dump.py).
Extracts and writes nothing.

Example
  uv run python -I tools/analysis/formats/snd_ls.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/formats/snd_ls.py --game-dir ./OriginalGame --list | head

Expected output (GOG 2.28): "SND v1, 50000 slots, 6598 used", a directory without gaps or overlaps up to the end of the file;
flags 32 = 3194 RIFF WAV, flags 33 = 3404 MP3; summaries of the WAV and MP3 formats; coverage of the sndProfiles.pak ids.
With --list, one line per entry (id, flags, offset, size, kind, parameters). Dependencies: standard library.
"""
import sys, os, struct, mmap, collections, argparse

DEFAULT_GAME_DIR = './OriginalGame'  # as in common/sacred_pe.py; the module is not imported to avoid pulling in capstone

ap = argparse.ArgumentParser(description='List and classify sound.pak (SND v1) entries.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--pak', help='sound.pak (default <game-dir>/PAK/sound.pak)')
ap.add_argument('--profiles', help='sndProfiles.pak (default <game-dir>/PAK/sndProfiles.pak)')
ap.add_argument('--list', action='store_true', help='print one line per entry')
args = ap.parse_args()
PAK = args.pak or os.path.join(args.game_dir, 'PAK', 'sound.pak')
SPF = args.profiles or os.path.join(args.game_dir, 'PAK', 'sndProfiles.pak')

f = open(PAK, 'rb'); d = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
assert d[:3] == b'SND', d[:4]
ver = d[3]; cnt = struct.unpack_from('<I', d, 4)[0]
ents = []
for i in range(cnt):
    fl, o, sz = struct.unpack_from('<III', d, 0x100 + 12 * i)
    if o: ents.append((i, fl, o, sz))
print('SND v%d, %d slots, %d used, file %d bytes' % (ver, cnt, len(ents), len(d)))

# directory: entries follow each other from the end of the table to the end of the file
pos = 0x100 + 12 * cnt; gaps = overlaps = 0
for i, fl, o, sz in sorted(ents, key=lambda e: e[2]):
    gaps += o > pos; overlaps += o < pos; pos = max(pos, o + sz)
print('directory: data from %d, gaps %d, overlaps %d, data end == file size: %s' % (0x100 + 12 * cnt, gaps, overlaps, pos == len(d)))

MP3_BR = {(1, 3): [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
          (2, 3): [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]}
MP3_SR = {1: [44100, 48000, 32000], 2: [22050, 24000, 16000], 25: [11025, 12000, 8000]}

def mp3_head(b):
    """First MPEG frame: (sync word offset, version, layer, kbit/s, Hz, mono)."""
    j = 0
    while j + 4 <= len(b):
        if b[j] == 0xFF and b[j + 1] & 0xE0 == 0xE0:
            h = struct.unpack_from('>I', b, j)[0]
            v = {3: 1, 2: 2, 0: 25}.get((h >> 19) & 3); layer = 4 - ((h >> 17) & 3)
            bri = (h >> 12) & 15; sri = (h >> 10) & 3
            if v and layer == 3 and 0 < bri < 15 and sri < 3:
                return j, v, layer, MP3_BR[(1 if v == 1 else 2, 3)][bri], MP3_SR[v][sri], (h >> 6) & 3 == 3
        j += 1
    return None

def wav_info(b):
    p = 12; fmt = data = None; chunks = []
    while p + 8 <= len(b):
        cid = b[p:p + 4]; cl = struct.unpack_from('<I', b, p + 4)[0]; chunks.append(cid.decode('latin1'))
        if cid == b'fmt ':
            fmt = struct.unpack_from('<HHIIHH', b, p + 8)
            spb = struct.unpack_from('<H', b, p + 26)[0] if fmt[0] == 0x11 and cl >= 20 else None
        elif cid == b'data':
            data = cl
        p += 8 + cl + (cl & 1)
    tag, ch, sr, bps, ba, bits = fmt
    sec = data / bps if tag == 1 else (data // ba) * spb / sr
    return tag, ch, sr, bits, ba, spb, sec, chunks, struct.unpack_from('<I', b, 4)[0] + 8 == len(b)

kinds = collections.Counter(); wavfmt = collections.Counter(); mp3fmt = collections.Counter()
lead = collections.Counter(); chunkset = collections.Counter(); riff_ok = 0; secs = collections.Counter()
for i, fl, o, sz in ents:
    b = d[o:o + sz]
    if b[:4] == b'RIFF' and b[8:12] == b'WAVE':
        tag, ch, sr, bits, ba, spb, sec, chunks, ok = wav_info(b)
        kind = {1: 'WAV PCM', 0x11: 'WAV IMA ADPCM'}.get(tag, 'WAV tag 0x%x' % tag)
        wavfmt[(kind, ch, sr, bits, ba, spb)] += 1; riff_ok += ok; secs[kind] += sec
        for c in chunks: chunkset[c] += 1
        desc = '%s ch=%d sr=%d bits=%d blockAlign=%d %.2fs' % (kind, ch, sr, bits, ba, sec)
    else:
        m = mp3_head(b[:4096])
        if m:
            j, v, layer, br, sr, mono = m
            kind = 'MP3'; lead[j > 0] += 1
            mp3fmt[('MPEG-%s L%d' % ({1: '1', 2: '2', 25: '2.5'}[v], layer), br, sr, 'mono' if mono else 'stereo')] += 1
            secs[kind] += (sz - j) * 8 / (br * 1000)
            desc = 'MPEG-%s L%d %dkbps %dHz %s, sync @%d' % ({1: '1', 2: '2', 25: '2.5'}[v], layer, br, sr, 'mono' if mono else 'stereo', j)
        else:
            kind = 'unknown'; desc = b[:16].hex(' ')
    kinds[(fl, kind)] += 1
    if args.list:
        print('%6d fl=%d off=%d size=%d %s' % (i, fl, o, sz, desc))

print('\nflags x kind:')
for k, v in sorted(kinds.items()): print('  flags %-3d %-14s %5d' % (k[0], k[1], v))
print('\nWAV (kind, ch, sr, bits, blockAlign, samplesPerBlock): RIFF size consistent %d' % riff_ok)
for k, v in wavfmt.most_common(): print('  %-48s %5d' % (k, v))
print('  chunks:', dict(chunkset))
print('\nMP3 first frame (version/layer, kbps, Hz, mode); %d start with sync word, %d have leading bytes before it' % (lead[False], lead[True]))
for k, v in mp3fmt.most_common(): print('  %-48s %5d' % (k, v))
print('\nduration, hours: ' + ', '.join('%s %.2f' % (k, v / 3600) for k, v in secs.items()) + ' (MP3 — by first-frame bitrate)')

if os.path.exists(SPF):
    s = open(SPF, 'rb').read(); start = 0x100 + 8192 * 12; ids = set()
    for k in range(8192):
        r = s[start + k * 184:start + (k + 1) * 184]
        if r[0]: ids.update(x for x in struct.unpack_from('<74H', r, 36) if x)
    used = {i for i, fl, o, sz in ents}
    print('\nsndProfiles ids %d: present in sound.pak %d, missing %d %s; sound.pak entries not in any profile %d'
          % (len(ids), len(ids & used), len(ids - used), sorted(ids - used)[:10], len(used - ids)))
    ranges = []
    for x in sorted(used - ids):
        if ranges and x == ranges[-1][1] + 1: ranges[-1][1] = x
        else: ranges.append([x, x])
    print('  unreferenced id ranges: %d, largest: %s' % (len(ranges), sorted(ranges, key=lambda r: r[0] - r[1])[:8]))
