#!/usr/bin/env python3
"""Comparison of client and server classes (RTTI) from two pe_deep.py JSON files: shared / client only / server only,
grouping of client classes by subsystem, hierarchy tables (direct bases, number of virtual slots),
inheritance depth, multiple inheritance, root classes.

Purpose (docs/01 §5.2): 258 shared classes, 1 server only
(cGSclass), 159 client only (98 cUI_* + 36 Command*, fonts, cDxDevice, IME, cEngine, cPath*).

Example
  python3 -I tools/analysis/pe/class_compare.py /tmp/analysis/sacred_deep.json /tmp/analysis/gs_deep.json --out /tmp/analysis/class_compare.md

Expected output: stdout has "client classes 417 server classes 259 shared 258 client-only 159 server-only 1",
depth histograms, roots; <out> is Markdown with tables. pe_deep.py can be run with --no-disasm
(RTTI does not need the disassembler). Dependencies: standard library only.
"""
import json, re, collections, argparse

ap = argparse.ArgumentParser(description='Compare RTTI class sets/hierarchies of client and server (pe_deep JSON).')
ap.add_argument('client_json', help='pe_deep.py output for Sacred.exe')
ap.add_argument('server_json', help='pe_deep.py output for GameServer.exe')
ap.add_argument('--out', required=True, help='Markdown report path')
args = ap.parse_args()
S = json.load(open(args.client_json)); G = json.load(open(args.server_json))
sc = set(S['class_list']); gc = set(G['class_list'])
shared = sorted(sc & gc); conly = sorted(sc - gc); sonly = sorted(gc - sc)
print("client classes", len(sc), "server classes", len(gc), "shared", len(shared), "client-only", len(conly), "server-only", len(sonly))
def grp(n):
    for p, l in [('_STL', 'STLport'), ('cUI_', 'UI widgets'), ('Command', 'UI property commands'), ('cCommand', 'UI commands'), ('cParticleSystem', 'particle FX'), ('cEvent', 'events'), ('cFile', 'file I/O'), ('cFont', 'fonts'), ('cFsFont', 'fonts'), ('NA2dLib', 'IME (NA2dLib)'), ('CIMHandler', 'IME'), ('CW32', 'IME'), ('dx', 'DirectX driver'), ('cDx', 'DirectX driver'), ('cTexture', 'textures'), ('cGranny', 'Granny'), ('cGrn', 'Granny'), ('cMSS', 'audio (Miles)'), ('cSound', 'audio'), ('TinCat', 'TinCat'), ('ITinCat', 'TinCat'), ('cCreature', 'creatures'), ('cObject', 'objects'), ('cItem', 'items'), ('cWorld', 'world'), ('cSector', 'world'), ('cMacro', 'UI commands')]:
        if n.startswith(p): return l
    return 'other'
with open(args.out, 'w') as f:
    f.write("## Shared classes (RTTI present in both Sacred.exe and GameServer.exe): %d\n\n" % len(shared))
    f.write(', '.join('`%s`' % c for c in shared) + "\n\n")
    f.write("## Server-only classes: %d\n\n" % len(sonly)); f.write(', '.join('`%s`' % c for c in sonly) + "\n\n")
    f.write("## Client-only classes: %d (grouped)\n\n" % len(conly))
    g = collections.defaultdict(list)
    for c in conly: g[grp(c)].append(c)
    f.write("| group | count | classes |\n|---|---|---|\n")
    for k, v in sorted(g.items(), key=lambda x: -len(x[1])):
        f.write("| %s | %d | %s |\n" % (k, len(v), ', '.join('`%s`' % c for c in (v if len(v) <= 60 else v[:60] + ['... (+%d)' % (len(v) - 60)]))))
    f.write("\n## GameServer.exe class hierarchy (RTTI, %d classes with vtables; 'vslots' = virtual slots in primary vtable)\n\n" % len(G['class_table']))
    f.write("| class | direct base(s) | vslots | in client? |\n|---|---|---|---|\n")
    for c in G['class_table']:
        f.write("| `%s` | %s | %d | %s |\n" % (c['name'], ', '.join('`%s`' % b for b in c['bases']) or '-', c['vslots'], 'yes' if c['name'] in sc else 'no'))
    f.write("\nType descriptors without vtable/COL in GameServer: %s\n" % ', '.join('`%s`' % x for x in G['typedesc_without_vtable_names']))
    f.write("\n## Sacred.exe (client) class hierarchy (RTTI, %d classes with vtables)\n\n" % len(S['class_table']))
    f.write("| class | direct base(s) | vslots |\n|---|---|---|\n")
    for c in S['class_table']:
        f.write("| `%s` | %s | %d |\n" % (c['name'], ', '.join('`%s`' % b for b in c['bases']) or '-', c['vslots']))
    f.write("\nType descriptors without vtable/COL in Sacred: %s\n" % ', '.join('`%s`' % x for x in S['typedesc_without_vtable_names']))
# root classes and depth stats
def depth(table):
    d = {c['name']: len(c['all_bases']) for c in table}
    return collections.Counter(d.values())
print("GS depth hist", sorted(depth(G['class_table']).items()), "Sacred depth hist", sorted(depth(S['class_table']).items()))
print("GS MI classes", [c['name'] for c in G['class_table'] if c['mi']], "Sacred MI classes", [c['name'] for c in S['class_table'] if c['mi']][:40])
roots = collections.Counter(c['all_bases'][-1] if c['all_bases'] else c['name'] for c in S['class_table'])
print("Sacred root classes (top):", roots.most_common(25))
rootsg = collections.Counter(c['all_bases'][-1] if c['all_bases'] else c['name'] for c in G['class_table'])
print("GS root classes (top):", rootsg.most_common(25))
print("client-only sample:", conly[:80])
print("server-only:", sonly)
