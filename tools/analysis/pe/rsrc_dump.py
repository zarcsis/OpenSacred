#!/usr/bin/env python3
"""Summary of the PE resource section: types and sizes, large/custom resources (magic, entropy), dialogs
(DLGTEMPLATE/DLGTEMPLATEEX with controls and texts), string tables, menus/accelerators/version/manifest.

Purpose (docs/01 §4.6): Sacred.exe .rsrc is 5.3 MB, resource BINARY #107 (2.76 MB,
16-bit data, purpose unknown), settings dialogs, string tables.

Example
  python3 -I tools/analysis/pe/rsrc_dump.py OriginalGame/Sacred.exe
  python3 -I tools/analysis/pe/rsrc_dump.py --game-dir ./OriginalGame

Expected output: "== resource types ==" with count/total, "== large / custom resources ==" (BINARY #107 size=27xxxxx),
"== dialogs ==", "== string tables ==". Dependencies: pefile.
"""
import sys, struct, collections, re, math, argparse, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Summarise the PE resource section (types, dialogs, string tables).')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
args = ap.parse_args()
path = args.path or os.path.join(args.game_dir, 'Sacred.exe')
pe = pefile.PE(path)
img = pe.get_memory_mapped_image()
def rtname(e):
    return pefile.RESOURCE_TYPE.get(e.id, str(e.id)) if e.name is None else str(e.name)
def entropy(b):
    if not b: return 0
    c = collections.Counter(b); n = len(b)
    return -sum(v / n * math.log2(v / n) for v in c.values())
items = []
for t in pe.DIRECTORY_ENTRY_RESOURCE.entries:
    for n in t.directory.entries:
        for l in n.directory.entries:
            d = l.data.struct
            blob = img[d.OffsetToData:d.OffsetToData + d.Size]
            items.append((rtname(t), str(n.name) if n.name is not None else str(n.id), l.id, d.Size, blob))
summary = collections.defaultdict(lambda: [0, 0])
for ty, nm, lang, sz, blob in items:
    summary[ty][0] += 1; summary[ty][1] += sz
print("== resource types ==")
for ty, (cnt, tot) in sorted(summary.items(), key=lambda x: -x[1][1]):
    print("  %-16s count=%3d total=%9d bytes" % (ty, cnt, tot))
print("\n== large / custom resources ==")
for ty, nm, lang, sz, blob in sorted(items, key=lambda x: -x[3])[:12]:
    print("  %-14s name=%-20s lang=%d size=%d entropy=%.2f magic=%s ascii=%r" % (ty, nm, lang, sz, entropy(blob[:65536]), blob[:16].hex(), bytes(b if 32 <= b < 127 else 46 for b in blob[:32]).decode()))
def read_sz(buf, pos):
    end = pos
    while buf[end:end + 2] != b'\0\0': end += 2
    return buf[pos:end].decode('utf-16le', errors='replace'), end + 2
def parse_dialog(blob):
    # DLGTEMPLATE or DLGTEMPLATEEX
    out = []
    pos = 0
    sig = struct.unpack_from('<HH', blob, 0)
    ex = (sig[1] == 0xFFFF)
    if ex:
        ver, sigw, helpid, exstyle, style, cdit, x, y, cx, cy = struct.unpack_from('<HHIIIHhhhh', blob, 0); pos = 26
    else:
        style, exstyle, cdit, x, y, cx, cy = struct.unpack_from('<IIHhhhh', blob, 0); pos = 18
    # menu, class, title
    def arr(pos):
        w = struct.unpack_from('<H', blob, pos)[0]
        if w == 0: return '', pos + 2
        if w == 0xFFFF: return '#%d' % struct.unpack_from('<H', blob, pos + 2)[0], pos + 4
        return read_sz(blob, pos)
    menu, pos = arr(pos); cls, pos = arr(pos); title, pos = arr(pos)
    if style & 0x40:  # DS_SETFONT
        if ex:
            pt, wt, it, cs = struct.unpack_from('<HHBB', blob, pos); pos += 6
        else:
            pt = struct.unpack_from('<H', blob, pos)[0]; pos += 2
        font, pos = read_sz(blob, pos)
    else: font = ''; pt = 0
    out.append(("DIALOG%s" % ('EX' if ex else ''), title, cx, cy, font, pt, cdit))
    for i in range(cdit):
        pos = (pos + 3) & ~3
        if ex:
            helpid, cexstyle, cstyle, cx_, cy_, ccx, ccy, cid = struct.unpack_from('<IIIhhhhI', blob, pos); pos += 24
        else:
            cstyle, cexstyle, cx_, cy_, ccx, ccy, cid = struct.unpack_from('<IIhhhhH', blob, pos); pos += 18
        ccls, pos = arr(pos)
        ctext, pos = arr(pos)
        extra = struct.unpack_from('<H', blob, pos)[0]; pos += 2 + extra
        names = {'#128': 'Button', '#129': 'Edit', '#130': 'Static', '#131': 'ListBox', '#132': 'ScrollBar', '#133': 'ComboBox'}
        out.append((names.get(ccls, ccls), ctext, cid, cstyle))
    return out
print("\n== dialogs ==")
for ty, nm, lang, sz, blob in items:
    if ty != 'RT_DIALOG': continue
    try:
        res = parse_dialog(blob)
        print("  DIALOG id=%s lang=%d: title=%r size=%dx%d font=%s %d controls" % (nm, lang, res[0][1], res[0][2], res[0][3], res[0][4], res[0][6]))
        for c in res[1:]:
            print("     %-9s id=%-6s %r" % (c[0], c[2], c[1]))
    except Exception as e:
        print("  DIALOG id=%s parse error %s" % (nm, e))
print("\n== string tables ==")
for ty, nm, lang, sz, blob in items:
    if ty != 'RT_STRING': continue
    pos = 0; sid = (int(nm) - 1) * 16; strs = []
    while pos + 2 <= len(blob):
        l = struct.unpack_from('<H', blob, pos)[0]; pos += 2
        if l: strs.append((sid, blob[pos:pos + 2 * l].decode('utf-16le', errors='replace')))
        pos += 2 * l; sid += 1
    for sid, s in strs: print("  %5d: %r" % (sid, s[:120]))
print("\n== menus/accelerators/version/manifest present ==")
for ty, nm, lang, sz, blob in items:
    if ty in ('RT_MENU', 'RT_ACCELERATOR', 'RT_MANIFEST', 'RT_VERSION', 'RT_RCDATA'):
        print("  %s id=%s size=%d %r" % (ty, nm, sz, blob[:80]))
