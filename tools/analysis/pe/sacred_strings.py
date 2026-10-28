#!/usr/bin/env python3
"""Symbol-like and configuration strings from the data sections of Sacred.exe (.rdata/.data/.rsrc):
a "Class::method" map from log strings, string categories (registry, URL, ProgID/COM, media, paths, DLL, network,
third-party libraries, build, debug), a histogram of file extensions, known binary GUIDs.

Purpose (docs/01 §4.5, §6: the map of subsystems from log strings): 74 classes / 215 methods with log strings
(dxDriver7 21 methods, cObjectManager 15, cEngine 14, ...), DirectDraw7/Direct3D7 GUIDs, no WMP/DirectShow graph.
The .text section is not read.

Example
  uv run python -I tools/analysis/pe/sacred_strings.py OriginalGame/Sacred.exe --out /tmp/analysis/strings

Output: <out>/class_methods.md, <out>/string_categories.md, <out>/guids.md, <out>/strings.json; stdout gets
per-category counters and the GUIDs found. Dependencies: pefile.
"""
import sys, re, struct, collections, json, uuid, argparse, os
import pefile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

ap = argparse.ArgumentParser(description='Symbol-like strings, categories and GUIDs from the data sections.')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--out', required=True, help='output directory')
args = ap.parse_args()
path = args.path or os.path.join(args.game_dir, 'Sacred.exe'); outdir = args.out
os.makedirs(outdir, exist_ok=True)
pe = pefile.PE(path, fast_load=True)
base = pe.OPTIONAL_HEADER.ImageBase
data = open(path, 'rb').read()
secmap = {}
for s in pe.sections:
    n = s.Name.rstrip(b'\0').decode()
    secmap[n] = (s.PointerToRawData, s.PointerToRawData + s.SizeOfRawData, s.VirtualAddress)
def sec_of_off(off):
    for n, (a, b, va) in secmap.items():
        if a <= off < b: return n
    return '?'
# all printable strings >= 4 with offsets, from the data sections only
strings = []
for n in ('.rdata', '.data', '.rsrc'):
    a, b, va = secmap[n]
    for m in re.finditer(rb'[\x20-\x7e]{4,}', data[a:b]):
        strings.append((a + m.start(), n, m.group(0).decode()))
print("strings:", len(strings), collections.Counter(n for _, n, _ in strings))
# Class::method
cm = collections.defaultdict(collections.Counter)
cm_re = re.compile(r'\b([A-Za-z_][A-Za-z0-9_]*)::(~?[A-Za-z_][A-Za-z0-9_]*)')
for off, n, s in strings:
    for m in cm_re.finditer(s):
        cm[m.group(1)][m.group(2)] += 1
with open(outdir + '/class_methods.md', 'w', encoding='utf-8', newline='') as f:
    f.write("| Class | #methods | methods (count of log strings) |\n|---|---|---|\n")
    for cls in sorted(cm, key=lambda c: (-len(cm[c]), c)):
        f.write("| %s | %d | %s |\n" % (cls, len(cm[cls]), ', '.join('%s(%d)' % (m, c) if c > 1 else m for m, c in sorted(cm[cls].items()))))
print("classes with ::", len(cm), "methods", sum(len(v) for v in cm.values()))
# registry / paths / urls / progids / misc
cats = collections.OrderedDict((k, []) for k in ['registry', 'url', 'progid_com', 'media', 'path_dir', 'file_ext', 'dll', 'network', 'thirdparty', 'build', 'cheat_debug'])
seen = set()
for off, n, s in strings:
    sl = s.lower()
    key = (n, s)
    if key in seen: continue
    seen.add(key)
    if re.search(r'software\\|hkey_|hklm|currentversion|\\\\\.\\|registry', sl): cats['registry'].append((off, n, s))
    if re.search(r'https?://|ftp://|www\.|\.com\b|\.de\b|\.net\b|ascaron|@', sl) and len(s) > 6: cats['url'].append((off, n, s))
    if re.search(r'wmplayer|mediaplayer|\.ocx|quartz|directshow|activemovie|igraphbuilder|wmvcore|clsid|progid|dispatch|ole32|iwmp|wmp', sl): cats['progid_com'].append((off, n, s))
    if re.search(r'\.wmv|\.avi|\.mpg|\.bik|\.smk|movie|video|mci', sl): cats['media'].append((off, n, s))
    if re.search(r'^[a-z]:\\|^\\\\|\\[a-z0-9_]+\\', sl) and 'software\\' not in sl: cats['path_dir'].append((off, n, s))
    if re.search(r'\.(dll|asi|m3d|flt|mix)$', sl): cats['dll'].append((off, n, s))
    if re.search(r'port|socket|lobby|tincat|udp|tcp|winsock|server|lan\b|gamespy|cdkey|cd-key', sl) and len(s) > 5: cats['network'].append((off, n, s))
    if re.search(r'copyright|\(c\)|granny|miles|rad game|stlport|d3dx|intel|ijl|libxml|zlib|inflate|deflate|libpng|freetype|protectdisc|securom|safedisc|starforce|bink|smacker|vorbis|ogg|mp3|na2d|unicows|msvcrt|microsoft', sl): cats['thirdparty'].append((off, n, s))
    if re.search(r'\.cpp|\.h\b|\.c\b|buildserver|source\\|assert|__file__|debug', sl): cats['build'].append((off, n, s))
    if re.search(r'cheat|god ?mode|console|ctrl\+|debugmenu|editor|devmode|developer', sl): cats['cheat_debug'].append((off, n, s))
with open(outdir + '/string_categories.md', 'w', encoding='utf-8', newline='') as f:
    for k, v in cats.items():
        f.write("\n## %s (%d)\n\n" % (k, len(v)))
        for off, n, s in v[:400]:
            f.write("- `%s` @0x%x %s\n" % (s.replace('`', "'"), off, n))
print({k: len(v) for k, v in cats.items()})
# File extension histogram
exts = collections.Counter()
for off, n, s in strings:
    for m in re.finditer(r'\.([A-Za-z0-9]{2,4})\b', s):
        e = m.group(1).lower()
        if e in ('tga', 'pak', 'wmv', 'mp3', 'wav', 'bmp', 'txt', 'bin', 'cfg', 'ttf', 'res', 'xml', 'dll', 'exe', 'log', 'ini', 'pax', 'ptx', 'jpg', 'dds', 'gr2', 'grn', 'gsn', 'sbk', 'zip', 'dat', 'lst', 'html', 'htm', 'ptm', 'bik', 'avi', 'cpp', 'h'): exts[e] += 1
print("ext histogram:", exts.most_common())
# Known GUIDs scan over the whole file (binary GUIDs)
known = {
 'CLSID_WMPlayer (WMP 7+ OCX)': '6BF52A52-394A-11D3-B153-00C04F79FAA6',
 'IID_IWMPPlayer': '6BF52A4F-394A-11D3-B153-00C04F79FAA6',
 'IID_IWMPCore': 'D84CCA99-CCE2-11D2-9ECC-0000F8085981',
 'CLSID_MediaPlayer (WMP 6.4 OCX)': '22D6F312-B0F6-11D0-94AB-0080C74C7E95',
 'IID_IMediaPlayer (WMP6.4)': '22D6F311-B0F6-11D0-94AB-0080C74C7E95',
 'CLSID_FilterGraph (DirectShow)': 'E436EBB3-524F-11CE-9F53-0020AF0BA770',
 'IID_IGraphBuilder': '56A868A9-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IMediaControl': '56A868B1-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IMediaEvent': '56A868B6-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IMediaEventEx': '56A868C0-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IVideoWindow': '56A868B4-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IBasicAudio': '56A868B3-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IBasicVideo': '56A868B5-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IMediaSeeking': '36B73880-C2C8-11CF-8B46-00805F6CEF60',
 'IID_IMediaPosition': '56A868B2-0AD4-11CE-B03A-0020AF0BA770',
 'IID_IVMRWindowlessControl': '0EB1088C-4DCD-46F0-878F-39DAE86A51B7',
 'CLSID_VideoMixingRenderer': 'B87BEB7B-8D29-423F-AE4D-6582C10175AC',
 'IID_ISampleGrabber': '6B652FFF-11FE-4FCE-92AD-0266B5D7C78F',
 'CLSID_WMAsfReader': '187463A0-5BB7-11D3-ACBE-0080C75E246E',
 'IID_IWMReader': '96406BD6-2B2B-11D3-B36B-00C04F6108FF',
 'IID_IDirectDraw7': '15E65EC0-3B9C-11D2-B92F-00609797EA5B',
 'IID_IDirectDraw4': '9C59509A-39BD-11D1-8C4A-00C04FD930C5',
 'IID_IDirectDraw2': 'B3A6F3E0-2B43-11CF-A2DE-00AA00B93356',
 'IID_IDirect3D7': 'F5049E77-4861-11D2-A407-00A0C90629A8',
 'IID_IDirect3D3': 'BB223240-E72B-11D0-A9B4-00AA00C0993E',
 'IID_IDirect3DDevice7': 'F5049E79-4861-11D2-A407-00A0C90629A8',
 'IID_IDirect3DTnLHalDevice': 'F5049E78-4861-11D2-A407-00A0C90629A8',
 'IID_IDirect3DHALDevice': 'F5049E7A-4861-11D2-A407-00A0C90629A8',
 'IID_IDirect3DRGBDevice': 'A4665C60-2673-11CF-A31A-00AA00B93356',
 'IID_IDirect3DRefDevice': '50936643-13E9-11D1-89AA-00A0C9054129',
 'IID_IDirectDrawSurface7': '06675A80-3B9B-11D2-B92F-00609797EA5B',
 'IID_IDirectDrawSurface4': '0B2B8630-AD35-11D0-8EA6-00609797EA5B',
 'IID_IDirectDrawGammaControl': '69C11C3E-B46B-11D1-AD7A-00C04FC29B4E',
 'IID_IDirectDrawColorControl': '4B9F0EE0-0D7E-11D0-9B06-00A0C903A3B8',
 'IID_IDirect3DVertexBuffer7': '7A503555-4A83-11D1-A5DB-00A0C90367F8',
 'IID_IDirectInput8A': 'BF798030-483A-4DA2-AA99-5D64ED369700',
 'IID_IDirectInput7A': '9A4CB684-236D-11D3-8E9D-00C04F6844AE',
 'IID_IDirectSound': '279AFA83-4981-11CE-A521-0020AF0BE560',
 'IID_IDirectSound8': 'C50A7E93-F395-4834-9EF6-7FA99DE50966',
 'IID_IDirectPlay4A': '0AB1C531-4745-11D1-A7A1-0000F803ABFC',
 'IID_IDispatch': '00020400-0000-0000-C000-000000000046',
 'IID_IUnknown': '00000000-0000-0000-C000-000000000046',
 'IID_IOleObject': '00000112-0000-0000-C000-000000000046',
 'IID_IOleInPlaceObject': '00000113-0000-0000-C000-000000000046',
 'IID_IConnectionPointContainer': 'B196B284-BAB4-101A-B69C-00AA00341D07',
 'IID_IWMPEvents': '19A6627B-DA9E-47C1-BB23-00B5E668236A',
 'DIID__WMPOCXEvents': '6BF52A51-394A-11D3-B153-00C04F79FAA6',
 'IID_IMMNotificationClient': '7991EEC9-7E89-4D85-8390-6C703CEC60C0',
 'IID_IShellLinkA': '000214EE-0000-0000-C000-000000000046',
 'CLSID_ShellLink': '00021401-0000-0000-C000-000000000046',
 'IID_IPersistFile': '0000010B-0000-0000-C000-000000000046',
 'CLSID_WMPMediaPlayer? (WMPlayer.OCX.7)': '6BF52A50-394A-11D3-B153-00C04F79FAA6',
}
found = []
for name, g in known.items():
    b = uuid.UUID(g).bytes_le
    idx = data.find(b)
    while idx >= 0:
        found.append((name, g, idx, sec_of_off(idx)))
        idx = data.find(b, idx + 1)
with open(outdir + '/guids.md', 'w', encoding='utf-8', newline='') as f:
    f.write("| GUID name | value | file offset | section |\n|---|---|---|---|\n")
    for name, g, idx, sec in found: f.write("| %s | %s | 0x%x | %s |\n" % (name, g, idx, sec))
print("known GUIDs found:", [(n, s) for n, g, i, s in found])
# generic GUID-looking blobs in .rdata/.data: look for the 'c000-000000000046' tail (OLE GUIDs)
# textual GUIDs
tg = [(off, n, s) for off, n, s in strings if re.search(r'\{?[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}?', s)]
print("textual GUIDs:", tg[:20])
json.dump({'classes': {k: dict(v) for k, v in cm.items()}, 'cats': {k: v for k, v in cats.items()}, 'guids': found, 'textual_guids': tg, 'exts': exts.most_common()}, open(outdir + '/strings.json', 'w', encoding='utf-8', newline=''), indent=1)
