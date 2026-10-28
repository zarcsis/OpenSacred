#!/usr/bin/env python3
"""Sacred resource-name hash (the function at 0x80E780 in the GOG ENG build) and its check against scripts/us/global.res.

Algorithm (a fact documented by the community in bssth/sacred-sdk and confirmed locally; the implementation here is our own):
  h = 0; for each character c: c = toupper_ascii(c); h = (c + h*113) as uint32 (with overflow);
  then h = h % 999999991 as signed C division (sign of the dividend) and back to uint32; result & 0x7FFFFFFF.
See docs/02 §8 (the formula and the global.res layout).

What the script does
  1. Self-test on three known name→id pairs.
  2. Reads the ids from global.res, collects candidate names from NetScript bytecode (FunkCode/StartCode: string
     operands and "res:NAME" keys), Vectoren.bin, DefPos.bin and from the given text files; computes the hit rate
     per source, separately for ids with bit 30 set ("hi", hashed) and without it ("lo", numeric).
  3. Tests the hypothesis "a numeric res:N is hashed as the string str(N)" and how many hi ids are hashes of decimal strings.

Examples
  uv run python -I tools/analysis/formats/global_res_hash.py --name ID_NPC_MERC                  # just compute the hash
  uv run python -I tools/analysis/formats/global_res_hash.py --game-dir ./OriginalGame [names.txt ...]

Expected output: "self-test OK", "global.res ids=23123 hi=... lo=...", a source/candidates/hits table, samples of the
reproduced ids with their sources. Prints no text of the game. Dependencies: standard library.
"""
import sys, os, struct, re, collections, argparse

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

HASH_MOD = 999999991          # 0x3B9AC9F7, prime
HASH_MUL = 113                # 0x71
U32 = 0xFFFFFFFF


def sacred_hash(name):
    """Resource-name hash: uint32 overflow, signed remainder as in C, bit 31 masked off."""
    acc = 0
    for ch in name:
        code = ord(ch)
        if 'a' <= ch <= 'z':          # ASCII toupper (CRT fast path)
            code -= 0x20
        acc = (code + acc * HASH_MUL) & U32     # uint32 wrap of c + h*113
        if acc & 0x80000000:                     # reinterpret as int32 before the signed '%'
            acc -= 1 << 32
        rem = abs(acc) % HASH_MOD                # C: remainder takes the sign of the dividend
        acc = (rem if acc >= 0 else -rem) & U32  # back to uint32
    return acc & 0x7FFFFFFF


SELF_TEST = [("DQ1_TOETE_NPC_ZIEL18", 2123824365), ("RB_5082_LOG_TITLE", 636419051), ("ID_NPC_MERC", 1269826129)]

ap = argparse.ArgumentParser(description='Sacred resource-name hash and its verification against global.res ids.')
ap.add_argument('extra', nargs='*', help='text files with one candidate name per line')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--res', help='global.res (default <game-dir>/scripts/us/global.res)')
ap.add_argument('--name', action='append', default=[], help='print the hash of this name and exit')
args = ap.parse_args()
for name, exp in SELF_TEST:
    assert sacred_hash(name) == exp, (name, sacred_hash(name), exp)
print('self-test OK')
if args.name:
    for nm in args.name:
        print('%s -> %d (0x%08x)' % (nm, sacred_hash(nm), sacred_hash(nm)))
    sys.exit(0)
root = args.game_dir
res = args.res or os.path.join(root, 'scripts', 'us', 'global.res')
d = open(res, 'rb').read()
# records of 16 B {u32 id, u32 offset, u32 flags, u32 length} from byte 4; offsets count from byte 4 and the first
# string starts right after the records, so the first offset is 16 * count (docs/02 §8)
first = struct.unpack_from('<I', d, 8)[0]; n = first // 16
tab = [struct.unpack_from('<4I', d, 4 + 16 * i) for i in range(n)]
ids = set(t[0] for t in tab); hi = set(i for i in ids if i >= 1 << 30); lo = ids - hi
print('global.res ids=%d hi(>=2^30)=%d lo=%d' % (len(ids), len(hi), len(lo)))
src = collections.defaultdict(set)
def walk_bin(fp, label):
    b = open(fp, 'rb').read(); off = 0
    while off + 4 <= len(b):
        op, ln = struct.unpack_from('<HH', b, off)
        if ln < 4: break
        for m in re.finditer(rb'[\x20-\x7e\x80-\xff]{2,80}\0', b[off + 4:off + ln]):
            s = m.group()[:-1].decode('latin1'); src[label].add(s)
            if re.match(r'(?i)res:', s): src[label + ':res-keys'].add(s[4:])
        off += ln
for tree in ['bin/NetScript', 'bin/NetScriptCamp', 'bin/Addon/NetScriptCamp', 'bin/TYPE_NPC_SERAPHIM']:
    for fn in ['FunkCode.bin', 'StartCode.bin']:
        fp = os.path.join(root, tree, fn)
        if os.path.exists(fp): walk_bin(fp, fn[:-4])
    for fn in ['Vectoren.bin', 'DefPos.bin']:
        fp = os.path.join(root, tree, fn)
        if not os.path.exists(fp): continue
        b = open(fp, 'rb').read()
        for m in re.finditer(rb'[\x20-\x7e\x80-\xff]{2,64}\0', b): src[fn[:-4]].add(m.group()[:-1].decode('latin1'))
for sf in args.extra:
    for l in open(sf, encoding='latin1'):
        l = l.rstrip('\n')
        if 2 <= len(l) <= 80: src[os.path.basename(sf)].add(l)
allmatched = {}
print('\nsource                     cands   hits  hi-hits  (hit = sacred_hash(cand) is a global.res id)')
for label, cs in sorted(src.items()):
    hits = 0; hh = 0
    for c in cs:
        v = sacred_hash(c)
        if v in ids:
            hits += 1; allmatched.setdefault(v, (label, c))
            if v in hi: hh += 1
    print('%-26s %6d %6d %7d' % (label, len(cs), hits, hh))
mh = [v for v in allmatched if v in hi]
print('\nTOTAL distinct ids reproduced: %d / %d; hi-bit ids reproduced: %d / %d = %.1f%%' % (len(allmatched), len(ids), len(mh), len(hi), 100 * len(mh) / max(1, len(hi))))
for v in sorted(mh)[:8]: print('  ', v, allmatched[v][0])   # sorted: reproducible sample; id and source only
# numeric res:N via str(N)
nums = set()
for label, cs in src.items():
    for c in cs:
        m = re.match(r'(?i)res:([0-9]+)$', c)
        if m: nums.add(int(m.group(1)))
direct = [x for x in nums if x in ids]; viastr = [x for x in nums if sacred_hash(str(x)) in ids]
print('\nnumeric res:N operands distinct=%d; N itself is an id: %d; sacred_hash(str(N)) is an id: %d (%.1f%%)' % (len(nums), len(direct), len(viastr), 100 * len(viastr) / max(1, len(nums))))
for x in sorted(viastr)[:6]: print('   res:%d -> %d' % (x, sacred_hash(str(x))))
missing = sorted(x for x in nums if x not in viastr)[:15]; print('   numeric N whose hash is NOT an id (first 15):', missing)
# how many hi ids are hashes of decimal strings 0..99999?
dec = {sacred_hash(str(i)): i for i in range(0, 100000)}
dechits = [v for v in hi if v in dec]; lohits = [v for v in lo if v in dec]
print('\nhi ids that are sacred_hash(decimal 0..99999): %d; lo ids that are: %d; e.g. %s' % (len(dechits), len(lohits), [(v, dec[v]) for v in sorted(dechits)[:5]]))
# are the lo ids (non-hashed) small numbers? distribution
print('lo id ranges: <100:', sum(1 for i in lo if i < 100), ' <100000:', sum(1 for i in lo if i < 100000), ' >=100000:', sum(1 for i in lo if i >= 100000))
