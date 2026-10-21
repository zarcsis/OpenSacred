#!/usr/bin/env python3
"""MDL model containers (PAK/models.pak, PAK/models03.pak) and the Granny 1.2b container of their records:
layout checks over every record, a census of chunk tags, and a per-record table for cross-checking the Rust readers.

Purpose (docs/02 §4.1–4.2; PLAN.md §10, step 5): the reference prototype for `opensacred_formats::mdl` and the GRN container
reader. The layout below was measured on the GOG files on 2026-10-16; the script checks every statement on every record
and prints how many records satisfy it.

  MDL v3   0x100-byte PAK header: count, nModels, nMotions, sizeof(ModelDesc) = 1194, sizeof(MotionDesc) = 256, X;
           directory (kind, offset, size): nModels models (kind 0x40), then nMotions motions (kind 0x41).
           Record = descriptor + GRN file. Descriptor: name[32], u32 0xFF, u32 dataOffset = offset + descriptor size,
           u32 dataSize, u16 flags. One name is 33 characters long and its last character and NUL overwrite the 0xFF.
           Directory size = 1194 + 2 * dataSize for a model, 256 + dataSize for a motion. Records lie back to back in
           directory order up to the end of the file. ModelDesc +0x70: 256 u32 motion ordinals (0 = none). ModelDesc
           +0x470: three equal f32, the model's own scale, which the client applies to the model and its motions as it
           loads them (0x411F50, 0x4118B0). Fields that the Models.tmp cache fills in hold MSVC fill 0xCC in the pak.
  Models.tmp (MDT v2)  0x118 + the descriptors of models.pak and models03.pak in order: all models, then all motions.
           It differs from the pak descriptors only in the global number (ModelDesc +0x4A8, MotionDesc +0x2E u16), in
           ModelDesc +0x47C (the pak index: 0 for models.pak, 4 for models03.pak, as in Texture.TMP), in the MotionDesc
           parameter block +0x80..+0x100 (MOTIONS.PAK) and in motion references that point to custom motions.
  GRN      +0x00 64-byte signature, the same in every file. +0x40 root header (32 B): tag 0xCA5E0000, section count 3,
           CRC-32 of the section table, 0, size from +0x40 to the end of the file, 12 zero bytes. +0x60 section table:
           3 x 20 B {tag, 0, offset from the start of the file, CRC-32 of the section bytes, 0}, tags 0102, 0103, 0101;
           a section runs to the next section (the last one to the end of the file). A section is a chunk tree: u32 node
           count, 12 unused bytes, then nodes {u32 tag, u32 offset from the section start, u32 descendant count} in
           pre-order; a node's own data runs to the offset of the next node; the last node is the terminator
           0xCA5EFFFF at the end of the section.
  Skeleton (section 0103; tag names per grn2glb `grn_types.h`, MIT) Top level, always in this order: 0200 0F03 0A01
           0B01 0602 0507 0304 0D01 0C01 0E01 1205. 0200 string table: u32 count, u32 len, then count NUL-terminated
           strings filling len bytes, the first one empty; the node is 8 + len rounded up to 4. 0F03 data extensions:
           0F00 objects (8 B: 1, 1), each with one 0F05 holding 0F01 properties (u32 key string), each with one 0F06
           holding 0F02 values {f32, u32 string}; the float is 0 except in the note-like properties of a few motions.
           0B01 transform channels: 0B00, each with one 0F04 (u32 object, 1-based). 0507 -> one 0505 skeleton -> one
           0508 -> 0506 bones of 68 B: u32 parent, f32 translation[3], f32 rotation quaternion [x, y, z, w], f32
           scale/shear[9]. Bone 0 is __Root with parent 0 (itself); every other bone's parent precedes it. 0C01 -> one
           0C00 form -> 0C07 -> 0C08 (u32 skeleton, 1-based) -> 0C02 (u32 channel per bone, 1-based) and 0C05. A bone's
           name: 0C02[bone] -> channel -> 0F04 object -> its __ObjectName. Scale/shear matrices are symmetric.
  Model    (section 0103) 0602 -> 0601 meshes: 0604 -> 0603 -> 0801 positions, 0802 normals (f32[3]), 0804 -> 0803
           texture-coordinate sets (u32 3, f32[3] entries); 0702 weights: u32 count (0 rigid, else one per position),
           u32 highest local bone, u32 most influences, per vertex u32 n, n {u32 local bone, f32 weight}; 0901
           triangles {u32 position[3], u32 normal[3]}; 0F04 object (the mesh name). Some meshes lack 0604 and are empty.
           0C00 form -> 0C06 -> 0C03 form meshes (u32 mesh, 1-based) -> 0C04 (empty), 0C09 -> 0C0A {u32 bone, f32
           radius, f32 min[3], f32 max[3]}: local bone k is entry k; the bounds hold the positions weighted on the bone
           in its bind-pose frame. 0304 -> 0301 textures: 0305 -> 0303 {u32 width, height, format, payload}, 0F04.
           0D01 -> 0D00 materials: 0D03 {0, u32 texture 1-based, 1} with 0F04, or 0D02 rgba f32[4] with an optional
           0F04. 0E01 -> 0E00 (u32 1) -> 0E07 -> 0E02 passes {u32 form mesh 0-based, u32 material 1-based}: 0E03 holds
           two sources, color and texture coordinates, each 0E05 rgb f32[3] or 0E04 u32 set; 0E06 u32 count, then per
           triangle u32 triangle and 3 u32 per 0E04 source (corner by corner); a mesh's passes claim each triangle once.
  Animation (section 0103; record layout per openheilig formats/granny-grn.md) 1205 -> one 1200 -> 1201 and 1203,
           all three without own data; 1203 -> 1204 tracks. A track starts with u32 channel (1-based transform
           channel), u32 0, u32 layout: 1 split, 0 interleaved. Split (52 B header): u32 interpolation modes of the
           translation, the rotation and the scale/shear (0, 1 or 2), u32 key counts nt, nq, ns (at least 1 each),
           u32 time selectors 0, 1, 2, u32 0; then the times of the three curves (f32), then the keys: f32[3]
           translations, f32[4] rotation quaternions (x, y, z, w; control points, not normalized), f32[9] scale/shear;
           the record is exactly 52 + 16 nt + 20 nq + 40 ns bytes. Interleaved: N frames of 68 B after the 12-byte
           header, f32 time, translation[3], rotation[4], scale/shear[9]. Times rise strictly from 0. A track
           animates the bone whose 0C02 entry is its channel.

Examples
  python3 -I tools/analysis/formats/grn_dump.py --game-dir ./OriginalGame
  python3 -I tools/analysis/formats/grn_dump.py --game-dir ./OriginalGame --records /tmp/analysis/grn_records.tsv

Expected output (2.28): "PAK/models.pak: MDL v3 ... models 1572, motions 3421" and "PAK/models03.pak: ... models 2,
motions 2", then "PAK/Models.tmp: ... True" and "GRN: 4993 non-empty records"; every check passes on all of its items
("dataOffset == offset + descriptor size: 4993/4993", "sections: CRC-32 of the section bytes: 14979/14979"), followed
by the tag census, the skeleton census ("skeleton: bone 0 has parent 0, every other parent precedes it:
4993/4993") and the model census ("model: files with meshes 1568, meshes 2555, ..."; "pass: triangles and set
indices in range: 3193/3193"). --records writes one tab-separated row per record (columns in RECORD_COLUMNS); record
names are printable ASCII. Dependencies: standard library (the shared sacred_pe module supplies the default game
directory; it imports pefile and capstone from tools/requirements.txt).
"""
import argparse
import collections
import math
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

PAKS = ('PAK/models.pak', 'PAK/models03.pak')
MODELS_TMP = 'PAK/Models.tmp'
KIND_MODEL, KIND_MOTION = 0x40, 0x41
DESC_SIZE = {KIND_MODEL: 1194, KIND_MOTION: 256}
KIND_NAME = {KIND_MODEL: 'model', KIND_MOTION: 'motion'}
MDT_HEADER = 0x118
GRN_SIGNATURE_SIZE = 0x40
TAG_ROOT, TAG_END = 0xCA5E0000, 0xCA5EFFFF
SECTION_TAGS = (0xCA5E0102, 0xCA5E0103, 0xCA5E0101)
# motions: the nonzero ModelDesc motion slots as "slot:ordinal" joined by ";"; grn: "ok", "empty" or the failed part;
# section_nodes: node counts of the three sections, terminators included; tree_crc: CRC-32 of the u32 LE triples
# (tag, own data length, descendants) of every node but the terminators, all sections in order, a fingerprint of the
# whole tree; bones, meshes, tracks: counts of the tags below; skeleton_crc: CRC-32 over the bones in order of the name,
# a NUL, the u32 LE parent and the 64 bytes of floats as stored, empty if the skeleton does not read; mesh_crc,
# material_crc: the fingerprints of mesh_crc() and material_crc() over read_model(), empty if the model does not read;
# animation_crc: the fingerprint of animation_crc() over read_animation(), empty if the animation does not read.
RECORD_COLUMNS = ('pak', 'index', 'kind', 'name', 'data_offset', 'data_size', 'flags', 'motions', 'scale', 'grn',
                  'section_nodes', 'tree_crc', 'bones', 'meshes', 'tracks', 'skeleton_crc', 'mesh_crc', 'material_crc',
                  'animation_crc')
# Tags counted per record for the table (names per openheilig formats/granny-grn.md): 0506 bone, 0601 mesh,
# 1204 per-bone animation track.
TAG_BONE, TAG_MESH, TAG_TRACK = 0xCA5E0506, 0xCA5E0601, 0xCA5E1204
# The object section 0103 (names per grn2glb grn_types.h): its top-level nodes in the order of every file.
OBJECT_TOP = (0x0200, 0x0F03, 0x0A01, 0x0B01, 0x0602, 0x0507, 0x0304, 0x0D01, 0x0C01, 0x0E01, 0x1205)
BONE_SIZE = 68
# Animation tracks: the split header, the interleaved header and frame.
SPLIT_HEADER, INTERLEAVED_HEADER, FRAME_SIZE = 52, 12, 68
LAYOUT_INTERLEAVED, LAYOUT_SPLIT = 0, 1


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


def u32s(data, off, n):
    return struct.unpack_from(f'<{n}I', data, off)


def read_mdl(path):
    """Directory and descriptors of an MDL container; records are (index, kind, offset, size, desc, grn)."""
    d = open(path, 'rb').read()
    magic, version = d[:3], d[3]
    count, n_models, n_motions, model_desc, motion_desc, x = u32s(d, 4, 6)
    entries = [u32s(d, 0x100 + 12 * i, 3) for i in range(count)]
    records = []
    for i, (kind, off, size) in enumerate(entries):
        desc = d[off:off + DESC_SIZE.get(kind, 0)]
        data_off, data_size = struct.unpack_from('<II', desc, 0x24) if len(desc) >= 0x2C else (0, 0)
        records.append((i, kind, off, size, desc, d[data_off:data_off + data_size]))
    header = dict(magic=magic, version=version, count=count, models=n_models, motions=n_motions,
                  model_desc=model_desc, motion_desc=motion_desc, x=x, size=len(d))
    return header, records


def check_mdl(path, header, records):
    print(f'{path}: {header["magic"].decode("latin-1")} v{header["version"]}, header ({header["count"]}, '
          f'{header["models"]}, {header["motions"]}, {header["model_desc"]}, {header["motion_desc"]}, {header["x"]}); '
          f'models {sum(r[1] == KIND_MODEL for r in records)}, motions {sum(r[1] == KIND_MOTION for r in records)}')
    c = Check()
    c('magic MDL v3', header['magic'] == b'MDL' and header['version'] == 3)
    c('count == nModels + nMotions', header['count'] == header['models'] + header['motions'])
    c('descriptor sizes 1194, 256', (header['model_desc'], header['motion_desc']) == (1194, 256))
    flags = collections.Counter()
    scales = collections.Counter()
    empty = []
    long_names = []
    end = None
    for i, kind, off, size, desc, grn in records:
        where = f'#{i}'
        c('kind: models first, then motions', kind == (KIND_MODEL if i < header['models'] else KIND_MOTION), where)
        hs = DESC_SIZE.get(kind, 0)
        name = desc[:32].split(b'\0')[0]
        ff, data_off, data_size, flag = struct.unpack_from('<IIIH', desc, 0x20)
        c('descriptor +0x20 == 0xFF, unless a 32-character name runs on into it',
          ff == 0xFF or (len(name) == 32 and desc[0x20] != 0 and desc[0x21] == 0), where)
        if len(name) == 32:
            long_names.append(f'#{i} {(name + desc[0x20:0x24].split(b"\0")[0]).decode("latin-1")}')
        c('dataOffset == offset + descriptor size', data_off == off + hs, where)
        c('directory size == descriptor + (2 if model else 1) * dataSize',
          size == hs + (2 if kind == KIND_MODEL else 1) * data_size, where)
        c('record follows the previous one', end is None or off == end, where)
        end = data_off + data_size
        c('name is printable ASCII', all(0x20 <= b < 0x7F for b in name), where)
        c('model #0 / motion #nModels is INVALID_* with dataSize 0',
          (data_size == 0) == (i in (0, header['models'])), where)
        if data_size == 0:
            empty.append(f'#{i} {name.decode("latin-1")}')
        flags[(KIND_NAME.get(kind), flag)] += 1
        if kind == KIND_MODEL:
            refs = u32s(desc, 0x70, 256)
            c('ModelDesc: motion ordinals < nMotions, slot 0 is 0',
              refs[0] == 0 and max(refs) < header['motions'], where)
            c('ModelDesc +0x47C == 0xCCCCCCCC, +0x4A8 == 0xCCCC',
              desc[0x47C:0x480] == b'\xcc' * 4 and desc[0x4A8:0x4AA] == b'\xcc' * 2, where)
            scale = struct.unpack_from('<3f', desc, 0x470)
            c('ModelDesc +0x470: three equal positive floats', scale[0] == scale[1] == scale[2] > 0, where)
            scales[scale[0]] += 1
        else:
            c('MotionDesc +0x2E == 0xCCCC', desc[0x2E:0x30] == b'\xcc' * 2, where)
    c('last record ends at the end of the file', end == header['size'])
    c.report()
    print(f'  flags by kind: {dict(sorted(flags.items()))}; empty records: {empty}; names without NUL: {long_names}')
    print(f'  model scales: {", ".join(f"{s:.4g} x{n}" for s, n in sorted(scales.items()))}')


def check_models_tmp(path, paks):
    d = open(path, 'rb').read()
    models = [r for _, records in paks for r in records if r[1] == KIND_MODEL]
    motions = [r for _, records in paks for r in records if r[1] == KIND_MOTION]
    fields = u32s(d, 4, 6)
    print(f'{MODELS_TMP}: {d[:3].decode("latin-1")} v{d[3]}, header {fields}; size {len(d)} == 0x118 + {len(models)} * 1194 '
          f'+ {len(motions)} * 256: {len(d) == MDT_HEADER + len(models) * 1194 + len(motions) * 256}')
    c = Check()
    base = MDT_HEADER + len(models) * 1194
    for n, rec in enumerate(models):
        pak_desc = rec[4]
        tmp = d[MDT_HEADER + n * 1194:MDT_HEADER + (n + 1) * 1194]
        where = f'model {n}'
        c('ModelDesc +0x4A8 == global model number', struct.unpack_from('<H', tmp, 0x4A8)[0] == n, where)
        pak_index = 0 if n < paks[0][0]['models'] else 4
        c('ModelDesc +0x47C == pak index (0 models.pak, 4 models03.pak)',
          struct.unpack_from('<I', tmp, 0x47C)[0] == pak_index, where)
        same = [o for o in range(1194) if not (0x47C <= o < 0x480 or 0x4A8 <= o < 0x4AA)
                and not (0x70 <= o < 0x470) and tmp[o] != pak_desc[o]]
        c('ModelDesc equal to the pak outside +0x70..+0x470, +0x47C, +0x4A8', not same, where)
        pak_refs, tmp_refs = u32s(pak_desc, 0x70, 256), u32s(tmp, 0x70, 256)
        # Motion ordinals are local to the pak; in the cache they are global, and custom motions fill extra slots.
        offset = 0 if n < paks[0][0]['models'] else paks[0][0]['motions']
        changed = [(s, p, t) for s, (p, t) in enumerate(zip(pak_refs, tmp_refs)) if p and p + offset != t]
        c('ModelDesc pak motion ordinals keep their slot in the cache (offset by preceding paks)', not changed, where)
    for n, rec in enumerate(motions):
        pak_desc = rec[4]
        tmp = d[base + n * 256:base + (n + 1) * 256]
        where = f'motion {n}'
        c('MotionDesc +0x2E == global motion number', struct.unpack_from('<H', tmp, 0x2E)[0] == n, where)
        diff = [o for o in range(256) if not (0x2E <= o < 0x30 or 0x80 <= o < 0x100) and tmp[o] != pak_desc[o]]
        c('MotionDesc equal to the pak outside +0x2E and +0x80..+0x100', not diff, where)
    c.report()


def walk_section(g, start, end, c, where):
    """Chunk tree of a section: list of (tag, own data start, own data end, descendants) in file offsets."""
    if not c('section: node directory fits', start + 16 <= end, where):
        return None
    count = struct.unpack_from('<I', g, start)[0]
    if not c('section: node directory fits', start + 16 + 12 * count <= end, where):
        return None
    nodes = [u32s(g, start + 16 + 12 * i, 3) for i in range(count)]
    rels = [r for _, r, _ in nodes]
    c('section: node offsets do not decrease', all(a <= b for a, b in zip(rels, rels[1:])), where)
    c('section: first node data right after the directory', count > 0 and rels[0] == 16 + 12 * count, where)
    c('section: last node is the terminator at the section end',
      count > 0 and nodes[-1] == (TAG_END, end - start, 0), where)
    # Pre-order with descendant counts: every subtree lies inside its parent's subtree.
    ok = True
    stack = []  # index one past the last descendant of each open ancestor
    for i, (_, _, desc) in enumerate(nodes):
        while stack and stack[-1] <= i:
            stack.pop()
        last = i + 1 + desc
        if last > count or (stack and last > stack[-1]):
            ok = False
            break
        stack.append(last)
        if i == count - 1 and len(stack) > 1:
            ok = False  # the terminator lies inside another node's subtree
    c('section: descendant counts nest; the terminator is a top-level node', ok, where)
    c('section: tag in 0xCA5E0000..0xCA5EFFFF', all(t >> 16 == 0xCA5E for t, _, _ in nodes), where)
    out = []
    for i, (tag, rel, desc) in enumerate(nodes):
        nxt = rels[i + 1] if i + 1 < count else end - start
        out.append((tag, start + rel, start + nxt, desc))
    return out


def check_grn(g, c, where, signature):
    """Container checks of one GRN file; returns (status, sections) where sections are node lists."""
    c('signature: the same 64 bytes as the first file', g[:GRN_SIGNATURE_SIZE] == signature, where)
    if not c('root header and section table fit', len(g) >= 0x9C, where):
        return 'short', None
    tag, n_sections, crc, zero, size = u32s(g, 0x40, 5)
    c('root: tag 0xCA5E0000, 3 sections, 0 at +0xC', (tag, n_sections, zero) == (TAG_ROOT, 3, 0), where)
    c('root: size == file length - 0x40', size == len(g) - 0x40, where)
    c('root: +0x54..+0x60 zero', g[0x54:0x60] == b'\0' * 12, where)
    c('root: CRC-32 of the section table', crc == zlib.crc32(g[0x60:0x9C]), where)
    table = [u32s(g, 0x60 + 20 * j, 5) for j in range(3)]
    c('sections: tags 0102, 0103, 0101 with zeros at +4 and +0x10',
      [(t, a, b) for t, a, _, _, b in table] == [(s, 0, 0) for s in SECTION_TAGS], where)
    offsets = [off for _, _, off, _, _ in table] + [len(g)]
    c('sections: first at 0x9C, second at 0x1B8', offsets[:2] == [0x9C, 0x1B8], where)
    if not c('sections: increasing offsets inside the file', all(a <= b for a, b in zip(offsets, offsets[1:])), where):
        return 'section table', None
    sections = []
    for j, (_, _, off, sec_crc, _) in enumerate(table):
        c('sections: CRC-32 of the section bytes', sec_crc == zlib.crc32(g[off:offsets[j + 1]]), f'{where} s{j}')
        nodes = walk_section(g, off, offsets[j + 1], c, f'{where} s{j}')
        if nodes is None:
            return f'section {j}', None
        sections.append(nodes)
    return 'ok', sections


class ObjectTree:
    """Node tree of the object section 0103 given as walk_section() nodes. Nodes are addressed by index, and None as
    a parent of kids and tags() is the top level. child() and node_list() return None for a missing part and for a
    None parent, so their lookups chain without checks in between."""

    def __init__(self, g, nodes):
        self.g = g
        self.nodes = nodes[:-1]  # without the terminator
        self.kids = {None: []}
        open_ = []  # (index one past the subtree, node index) of every open ancestor
        for i, (_, _, _, descendants) in enumerate(self.nodes):
            while open_ and open_[-1][0] <= i:
                open_.pop()
            self.kids[open_[-1][1] if open_ else None].append(i)
            self.kids[i] = []
            open_.append((i + 1 + descendants, i))
        self.top = dict(zip(OBJECT_TOP, self.kids[None]))

    def tag(self, i):
        return self.nodes[i][0] & 0xFFFF

    def data(self, i):
        return self.g[self.nodes[i][1]:self.nodes[i][2]]

    def tags(self, parent):
        return [self.tag(k) for k in self.kids[parent]] if parent in self.kids else None

    def child(self, parent, tag, size=0):
        """The only child of parent with tag, if its own data is size bytes (any size for size=None); None
        otherwise."""
        if parent is None:
            return None
        found = [k for k in self.kids[parent] if self.tag(k) == tag]
        return found[0] if len(found) == 1 and (size is None or len(self.data(found[0])) == size) else None

    def node_list(self, parent, tag, size):
        """The children of parent, if all of them carry tag and size bytes of data (any size for size=None);
        None otherwise."""
        if parent is None:
            return None
        found = self.kids[parent]
        return found if all(self.tag(k) == tag and (size is None or len(self.data(k)) == size)
                            for k in found) else None


def read_skeleton(t, c, where):
    """Skeleton of an ObjectTree: (bones, objects, channels) or None if a statement fails. bones is [(name, parent,
    floats)] in bone order, floats the 64 stored bytes after the parent; objects is the list of data extension objects
    as {key: [(float, string)]}; channels holds the transform channel of every bone, from 0."""
    tag, data, child, node_list, kids = t.tag, t.data, t.child, t.node_list, t.kids
    if not c('objects: top level 0200 0F03 0A01 0B01 0602 0507 0304 0D01 0C01 0E01 1205',
             t.tags(None) == list(OBJECT_TOP), where):
        return None
    top = t.top

    table = data(top[0x0200])
    count, size = struct.unpack_from('<II', table) if len(table) >= 8 else (0, -1)
    body = table[8:8 + size]
    strings = body.split(b'\0')[:-1]
    if not c('strings: count NUL-terminated strings fill len bytes, the first empty; node = 8 + len rounded up to 4',
             len(table) == 8 + (size + 3) // 4 * 4 and body.endswith(b'\0') and len(strings) == count
             and strings[0] == b'', where):
        return None

    object_nodes = node_list(top[0x0F03], 0x0F00, 8)
    objects = [] if object_nodes is not None else None
    for o in object_nodes or []:
        properties = node_list(child(o, 0x0F05), 0x0F01, 4)
        obj = {} if properties is not None and data(o) == struct.pack('<II', 1, 1) else None
        for p in properties or []:
            key, = struct.unpack('<I', data(p))
            values = [struct.unpack('<fI', data(v)) for v in node_list(child(p, 0x0F06), 0x0F02, 8) or []]
            if key >= count or strings[key] in obj or not values or any(s >= count for _, s in values):
                obj = None
                break
            obj[strings[key]] = [(f, strings[s]) for f, s in values]
        if obj is None:
            objects = None
            break
        objects.append(obj)
    if not c('data extensions: 0F03 -> 0F00 objects (8 B: 1, 1) -> one 0F05 -> 0F01 properties (u32 key) -> one 0F06 '
             '-> 0F02 values (f32, u32 string); strings in range, keys unique in an object', objects is not None, where):
        return None

    channel_nodes = node_list(top[0x0B01], 0x0B00, 0)
    refs = [child(ch, 0x0F04, 4) for ch in channel_nodes or []]
    channels = [struct.unpack('<I', data(r))[0] for r in refs if r is not None]
    if not c('channels: 0B01 -> 0B00 -> one 0F04 (u32 object, 1-based, in range)',
             channel_nodes is not None and len(channels) == len(refs)
             and all(1 <= r <= len(objects) for r in channels), where):
        return None

    bones = node_list(child(child(top[0x0507], 0x0505), 0x0508), 0x0506, BONE_SIZE)
    if not c('skeleton: 0507 -> one 0505 -> one 0508 -> 0506 bones of 68 B, at least one, no children',
             bones and len(kids[top[0x0507]]) == 1 and all(not kids[b] for b in bones), where):
        return None

    c08 = child(child(child(top[0x0C01], 0x0C00), 0x0C07), 0x0C08, 4)
    c02 = child(c08, 0x0C02, 4 * len(bones))
    if not c('form: 0C01 -> one 0C00 -> one 0C07 -> one 0C08 (u32 skeleton 1) -> one 0C02 (u32 per bone), 0C05',
             c02 is not None and struct.unpack('<I', data(c08))[0] == 1 and child(c08, 0x0C05) is not None
             and len(kids[top[0x0C01]]) == 1, where):
        return None
    order = struct.unpack(f'<{len(bones)}I', data(c02))
    c('form: 0C02 is a permutation of the channels, one per bone',
      sorted(order) == list(range(1, len(channels) + 1)), where)
    if not c('bones: 0C02 channel in range, its object has one __ObjectName value',
             all(1 <= ch <= len(channels) and len(objects[channels[ch - 1] - 1].get(b'__ObjectName', ())) == 1
                 for ch in order), where):
        return None
    names = [objects[channels[ch - 1] - 1][b'__ObjectName'][0][1] for ch in order]

    out = [(names[i], struct.unpack_from('<I', data(b))[0], data(b)[4:]) for i, b in enumerate(bones)]
    if not c('skeleton: bone 0 has parent 0, every other parent precedes it',
             out[0][1] == 0 and all(parent < i for i, (_, parent, _) in enumerate(out) if i), where):
        return None
    c('skeleton: bone 0 is __Root', names[0] == b'__Root', where)
    c('bones: rotation is a unit quaternion within 1e-5',
      all(abs(math.sqrt(sum(x * x for x in struct.unpack_from('<4f', f, 12))) - 1) < 1e-5 for _, _, f in out), where)
    # Then whether the 9 values are rows or columns does not matter.
    c('bones: scale/shear is symmetric within 1e-6 of its largest value',
      all(abs(s[3 * i + j] - s[3 * j + i]) <= 1e-6 * max(map(abs, s))
          for s in (struct.unpack_from('<9f', f, 28) for _, _, f in out) for i in range(3) for j in range(i)), where)
    return out, objects, [ch - 1 for ch in order]


def read_model(t, objects, n_bones, c, where):
    """Meshes, textures, materials and render passes of an ObjectTree whose skeleton read with objects and n_bones
    bones: a dict of the lists below, or None if a statement fails. Indices are from 0, resolved from the file's
    1-based references; names are __ObjectName values.
      meshes     [{name, positions, normals, sets, triangles, weights, bones}] in file order. positions, normals:
                 [(x, y, z)]; sets: texture-coordinate sets, each [(u, v, w)]; triangles: [(p0, p1, p2, n0, n1, n2)],
                 position then normal indices; weights: [[(local bone, weight)]] per position, [] for a rigid mesh;
                 bones: the form mesh's [(bone, radius, min x, y, z, max x, y, z)], local bone k is bones[k]
      textures   [(name, width, height, format, payload)]
      materials  [(name or None, texture or None, rgba or None)]: a texture or a color
      passes     [(mesh, material, sources, rows)] in file order; sources: the color and the texture-coordinate
                 source, each ('rgb', (r, g, b)) or ('set', set); rows: [(triangle, indices...)] with three set
                 indices per 'set' source, corner by corner and source by source within a corner"""
    tag, data, child, node_list, kids, top = t.tag, t.data, t.child, t.node_list, t.kids, t.top

    def name(node):
        """__ObjectName of the 0F04 object of node, None if it has none or the reference is broken."""
        ref = child(node, 0x0F04, 4)
        k = struct.unpack('<I', data(ref))[0] if ref is not None else 0
        values = objects[k - 1].get(b'__ObjectName', ()) if 1 <= k <= len(objects) else ()
        return values[0][1] if len(values) == 1 else None

    def vec3s(blob):
        return [struct.unpack_from('<3f', blob, 12 * k) for k in range(len(blob) // 12)]

    mesh_nodes = node_list(top[0x0602], 0x0601, 0)
    if not c('meshes: 0602 -> 0601 meshes without own data', mesh_nodes is not None, where):
        return None
    meshes = []
    for m in mesh_nodes:
        mesh = dict(name=name(m), positions=[], normals=[], sets=[], triangles=[], weights=[], bones=[])
        empty = t.tags(m) == [0x0702, 0x0901, 0x0F04]
        if not c('mesh: children 0604 0702 0901 0F04, or 0702 0901 0F04 without vertices; 0F04 object with a name',
                 (empty or t.tags(m) == [0x0604, 0x0702, 0x0901, 0x0F04]) and mesh['name'] is not None, where):
            return None
        if not empty:
            vertex_sets = child(m, 0x0604)
            vs = child(vertex_sets, 0x0603)
            positions, normals = child(vs, 0x0801, None), child(vs, 0x0802, None)
            set_nodes = node_list(child(vs, 0x0804), 0x0803, None)
            if not c('mesh: 0604 -> one 0603 -> 0801 positions, 0802 normals (f32[3] each), 0804 -> 0803 sets',
                     vs is not None and len(kids[vertex_sets]) == 1 and t.tags(vs) == [0x0801, 0x0802, 0x0804]
                     and len(data(positions)) % 12 == 0 and len(data(normals)) % 12 == 0 and set_nodes is not None,
                     where):
                return None
            mesh['positions'], mesh['normals'] = vec3s(data(positions)), vec3s(data(normals))
            for s in set_nodes:
                d = data(s)
                if not c('mesh: 0803 set: u32 components 3, then f32[3] entries',
                         len(d) >= 4 and struct.unpack_from('<I', d)[0] == 3 and (len(d) - 4) % 12 == 0, where):
                    return None
                mesh['sets'].append(vec3s(d[4:]))

        d = data(child(m, 0x0901, None))
        triangles = [struct.unpack_from('<6I', d, 24 * k) for k in range(len(d) // 24)]
        if not c('mesh: 0901 triangles of 24 B: u32 position[3], u32 normal[3], in range',
                 len(d) % 24 == 0 and all(x < len(mesh['positions']) for tr in triangles for x in tr[:3])
                 and all(x < len(mesh['normals']) for tr in triangles for x in tr[3:]), where):
            return None
        mesh['triangles'] = triangles

        # 0702: u32 vertices (0 for a rigid mesh), u32 highest local bone, u32 most influences of a vertex, then per
        # vertex u32 n and n {u32 local bone, f32 weight}.
        d = data(child(m, 0x0702, None))
        count, highest, most = struct.unpack_from('<3I', d) if len(d) >= 12 else (0, 0, 0)
        o = 12
        weights = []
        while o + 4 <= len(d) and len(weights) < count:
            n = struct.unpack_from('<I', d, o)[0]
            weights.append([struct.unpack_from('<If', d, o + 4 + 8 * k) for k in range(n)] if o + 4 + 8 * n <= len(d)
                           else None)
            o += 4 + 8 * n
        if not c('mesh: 0702 weights fill the node; one list per position, or none and the header 0, 0, 0',
                 len(d) >= 12 and o == len(d) and len(weights) == count and None not in weights
                 and (count == len(mesh['positions']) and count > 0 or (count, highest, most) == (0, 0, 0)), where):
            return None
        if not c('mesh: weights: at least one per vertex, distinct bones, positive, summing to 1 within 1e-5; the '
                 'header holds the highest local bone and the most influences',
                 not weights or all(w and len({b for b, _ in w}) == len(w) and all(x > 0 for _, x in w)
                                    and abs(sum(x for _, x in w) - 1) < 1e-5 for w in weights)
                 and highest == max(b for w in weights for b, _ in w) and most == max(len(w) for w in weights), where):
            return None
        mesh['weights'] = weights
        meshes.append(mesh)

    # The form binds each mesh to bones of the skeleton.
    form = child(top[0x0C01], 0x0C00)
    form_meshes = node_list(child(form, 0x0C06), 0x0C03, 4)
    refs = [struct.unpack('<I', data(fm))[0] for fm in form_meshes or []]
    if not c('form: 0C00 -> one 0C06 -> 0C03 form meshes (u32 mesh, 1-based), each mesh once',
             form_meshes is not None and sorted(refs) == list(range(1, len(meshes) + 1)), where):
        return None
    for fm, ref in zip(form_meshes, refs):
        entries = node_list(child(fm, 0x0C09), 0x0C0A, 32)
        if not c('form mesh: 0C04 (empty), 0C09 -> 0C0A bones of 32 B: u32 bone, f32 radius, f32 min[3], f32 max[3]',
                 t.tags(fm) == [0x0C04, 0x0C09] and child(fm, 0x0C04) is not None and not kids[child(fm, 0x0C04)]
                 and entries is not None, where):
            return None
        mesh = meshes[ref - 1]
        mesh['bones'] = [struct.unpack('<I7f', data(e)) for e in entries]
        if not c('form mesh: bones in range; local bones of the weights in the list; a rigid mesh lists one bone',
                 all(e[0] < n_bones for e in mesh['bones'])
                 and (max((b for w in mesh['weights'] for b, _ in w), default=0) < len(mesh['bones'])
                      if mesh['weights'] else len(mesh['bones']) == 1), where):
            return None

    textures = []
    texture_nodes = node_list(top[0x0304], 0x0301, 0)
    if not c('textures: 0304 -> 0301 textures without own data', texture_nodes is not None, where):
        return None
    for tm in texture_nodes:
        images = child(tm, 0x0305)
        image = child(images, 0x0303, None)
        if not c('texture: 0305 -> one 0303 {u32 width, height, format, then the payload}; 0F04 object with a name',
                 t.tags(tm) == [0x0305, 0x0F04] and image is not None and len(kids[images]) == 1
                 and len(data(image)) >= 12 and name(tm) is not None, where):
            return None
        width, height, fmt = struct.unpack_from('<3I', data(image))
        payload = data(image)[12:]
        c('texture: payload: none for format 7, width * height * 4 bytes for 0 and 1, from "BIKi" for 4 and 5',
          {7: not payload, 0: len(payload) == width * height * 4, 1: len(payload) == width * height * 4,
           4: payload[:4] == b'BIKi', 5: payload[:4] == b'BIKi'}.get(fmt, False), where)
        textures.append((name(tm), width, height, fmt, payload))

    materials = []
    material_nodes = node_list(top[0x0D01], 0x0D00, 0)
    if not c('materials: 0D01 -> 0D00 materials without own data', material_nodes is not None, where):
        return None
    for mt in material_nodes:
        texture, color = child(mt, 0x0D03, 12), child(mt, 0x0D02, 16)
        if t.tags(mt) == [0x0D03, 0x0F04] and texture is not None:
            zero, ref, one = struct.unpack('<3I', data(texture))
            ok = zero == 0 and one == 1 and 1 <= ref <= len(textures) and name(mt) is not None
            materials.append((name(mt), ref - 1, None))
        else:
            ok = color is not None and (t.tags(mt) == [0x0D02]
                                         or t.tags(mt) == [0x0D02, 0x0F04] and name(mt) is not None)
            materials.append((name(mt), None, struct.unpack('<4f', data(color)) if color is not None else None))
        if not c('material: 0D03 {0, u32 texture 1-based, 1} and a named 0F04, or 0D02 color f32 rgba[4] and an '
                 'optional named 0F04', ok, where):
            return None

    # The model draws form meshes in passes, one material each.
    model = child(top[0x0E01], 0x0E00, 4)
    passes_node = child(model, 0x0E07)
    pass_nodes = node_list(passes_node, 0x0E02, 8) if meshes else []
    if not c('model: 0E01 -> one 0E00 (u32 1) -> one 0E07 -> 0E02 passes {u32 form mesh from 0, u32 material from 1}; '
             'no 0E07 without meshes',
             model is not None and len(kids[top[0x0E01]]) == 1 and struct.unpack('<I', data(model))[0] == 1
             and t.tags(model) == ([0x0E07] if meshes else []) and pass_nodes is not None, where):
        return None
    passes = []
    claimed = collections.defaultdict(list)
    for p in pass_nodes:
        form_mesh, material = struct.unpack('<II', data(p))
        source_list = child(p, 0x0E03)
        sources = []
        for s in kids[source_list] if source_list is not None else []:
            if tag(s) == 0x0E05 and len(data(s)) == 12:
                sources.append(('rgb', struct.unpack('<3f', data(s))))
            elif tag(s) == 0x0E04 and len(data(s)) == 4:
                sources.append(('set', struct.unpack('<I', data(s))[0]))
            else:
                sources.append(None)
        rows_data = data(child(p, 0x0E06, None)) if child(p, 0x0E06, None) is not None else b''
        n_sets = sum(s is not None and s[0] == 'set' for s in sources)
        stride = 4 + 12 * n_sets
        n_rows = struct.unpack_from('<I', rows_data)[0] if len(rows_data) >= 4 else -1
        if not c('pass: 0E03 -> two sources, each 0E05 rgb f32[3] or 0E04 u32 set; 0E06 {u32 count, then per triangle '
                 'u32 triangle and 3 u32 per 0E04 source}; form mesh and material in range',
                 t.tags(p) == [0x0E03, 0x0E06] and len(sources) == 2 and None not in sources
                 and len(rows_data) == 4 + n_rows * stride and form_mesh < len(refs)
                 and 1 <= material <= len(materials), where):
            return None
        mesh = refs[form_mesh] - 1
        rows = [struct.unpack_from(f'<{stride // 4}I', rows_data, 4 + stride * k) for k in range(n_rows)]
        set_sizes = [len(s) for s in meshes[mesh]['sets']]
        sets = [s[1] for s in sources if s[0] == 'set']
        if not c('pass: triangles and set indices in range',
                 all(st < len(set_sizes) for st in sets)
                 and all(r[0] < len(meshes[mesh]['triangles'])
                         and all(r[1 + k] < set_sizes[sets[k % n_sets]] for k in range(3 * n_sets)) for r in rows),
                 where):
            return None
        claimed[mesh].extend(r[0] for r in rows)
        passes.append((mesh, material - 1, sources, rows))
    c('passes: those of a mesh claim each of its triangles once; a mesh without triangles has none',
      all(sorted(claimed.get(k, [])) == list(range(len(mesh['triangles']))) for k, mesh in enumerate(meshes)), where)
    return dict(meshes=meshes, textures=textures, materials=materials, passes=passes)


def read_animation(t, n_channels, c, where):
    """Animation tracks of an ObjectTree with n_channels transform channels: [(channel, layout, curves)] in file order,
    or None if a statement fails. channel is from 0; curves are the translation, rotation and scale/shear curves, each
    (mode, times, keys) with keys of 3, 4 and 9 floats. An interleaved track gives its frames as three curves of mode
    1 (linear) with the same times."""
    data, child, node_list, kids, top = t.data, t.child, t.node_list, t.kids, t.top
    anim = child(top[0x1205], 0x1200, 0)
    if not c('animation: 1205 -> one 1200 -> 1201 and 1203, all three without own data',
             anim is not None and len(kids[top[0x1205]]) == 1 and t.tags(anim) == [0x1201, 0x1203]
             and child(anim, 0x1201, 0) is not None and not kids[child(anim, 0x1201, 0)]
             and child(anim, 0x1203, 0) is not None, where):
        return None
    track_nodes = node_list(child(anim, 0x1203, 0), 0x1204, None)
    if not c('animation: 1203 -> 1204 tracks without children',
             track_nodes is not None and all(not kids[k] for k in track_nodes), where):
        return None
    tracks = []
    for k in track_nodes:
        d = data(k)
        channel, zero, layout = struct.unpack_from('<3I', d) if len(d) >= 12 else (0, 0, -1)
        if not c('track: u32 channel (1-based, in range), u32 0, u32 layout 0 or 1',
                 1 <= channel <= n_channels and zero == 0 and layout in (LAYOUT_INTERLEAVED, LAYOUT_SPLIT), where):
            return None
        if layout == LAYOUT_SPLIT:
            fields = struct.unpack_from('<10I', d, 12) if len(d) >= SPLIT_HEADER else (3,) * 10
            modes, counts, selectors, pad = fields[0:3], fields[3:6], fields[6:9], fields[9]
            if not c('split track: modes 0..2, counts at least 1, selectors 0 1 2, u32 0; 52 + 16 nt + 20 nq + 40 ns '
                     'bytes', all(m <= 2 for m in modes) and all(n >= 1 for n in counts) and selectors == (0, 1, 2)
                     and pad == 0 and len(d) == SPLIT_HEADER + 16 * counts[0] + 20 * counts[1] + 40 * counts[2],
                     where):
                return None
            o = SPLIT_HEADER
            times = []
            for n in counts:
                times.append(struct.unpack_from(f'<{n}f', d, o))
                o += 4 * n
            keys = []
            for n, width in zip(counts, (3, 4, 9)):
                keys.append([struct.unpack_from(f'<{width}f', d, o + 4 * width * i) for i in range(n)])
                o += 4 * width * n
            curves = list(zip(modes, times, keys))
        else:
            n, rest = divmod(len(d) - INTERLEAVED_HEADER, FRAME_SIZE)
            if not c('interleaved track: 68-byte frames after the 12-byte header', rest == 0, where):
                return None
            frames = [struct.unpack_from('<17f', d, INTERLEAVED_HEADER + FRAME_SIZE * i) for i in range(n)]
            times = tuple(f[0] for f in frames)
            curves = [(1, times, [f[a:b] for f in frames]) for a, b in ((1, 4), (4, 8), (8, 17))]
        if not c('track: times rise strictly from 0',
                 all(ts[0] == 0 and all(a < b for a, b in zip(ts, ts[1:])) for _, ts, _ in curves if ts), where):
            return None
        tracks.append((channel - 1, layout, curves))
    return tracks


# Skeleton census statistics in print order; scale/shear matrices are compared with the identity within 1e-4.
SKELETON_STATS = ('files', 'bones', 'files with repeated bone names', 'bones with a diagonal scale/shear',
                  'bones with a general scale/shear', 'bone names with bytes outside printable ASCII',
                  'bone names with bytes 0x80-0x9F', 'properties with several values', 'values with a nonzero float')


def census_skeleton(sk, kind, bones, objects):
    """Adds one skeleton to the census counter sk: (kind, statistic) -> count."""
    names = [n for n, _, _ in bones]
    sk[kind, 'files'] += 1
    sk[kind, 'bones'] += len(bones)
    sk[kind, 'files with repeated bone names'] += len(set(names)) < len(names)
    for _, _, floats in bones:
        m = struct.unpack_from('<9f', floats, 28)
        if any(abs(v - (k % 4 == 0)) > 1e-4 for k, v in enumerate(m)):
            diagonal = all(abs(v) <= 1e-4 for k, v in enumerate(m) if k % 4)
            sk[kind, f'bones with a {"diagonal" if diagonal else "general"} scale/shear'] += 1
    sk[kind, 'bone names with bytes outside printable ASCII'] += sum(any(not 0x20 <= b < 0x7F for b in n) for n in names)
    sk[kind, 'bone names with bytes 0x80-0x9F'] += sum(any(0x80 <= b < 0xA0 for b in n) for n in names)
    for obj in objects:
        for values in obj.values():
            sk[kind, 'properties with several values'] += len(values) > 1
            sk[kind, 'values with a nonzero float'] += sum(f != 0 for f, _ in values)


# Model census statistics in print order; texture formats follow as 'textures of format N'.
MODEL_STATS = ('files with meshes', 'meshes', 'meshes without vertices', 'positions', 'normals', 'triangles',
               'meshes with 0 sets', 'meshes with 1 set', 'meshes with 2 sets', 'rigid meshes', 'skinned meshes',
               'files with a skinned mesh', 'skinned meshes with more than 4 influences on a vertex',
               'vertices with more than 4 influences', 'zero normals', 'triangle corners on a zero normal',
               'meshes with a corner on a zero normal', 'triangles with a winding test',
               'triangles counter-clockwise around their normals', 'textures', 'texture materials',
               'color materials', 'passes', 'passes with sources rgb, set', 'passes with sources rgb, rgb',
               'passes with sources set, set', 'meshes drawn with texture materials only',
               'meshes drawn with color materials only')


def census_model(md, kind, model):
    """Adds one model to the census counter md: (kind, statistic) -> count."""
    meshes = model['meshes']
    md[kind, 'files with meshes'] += bool(meshes)
    md[kind, 'files with a skinned mesh'] += any(m['weights'] for m in meshes)
    for m in meshes:
        md[kind, 'meshes'] += 1
        md[kind, 'meshes without vertices'] += not m['positions']
        for key in ('positions', 'normals', 'triangles'):
            md[kind, key] += len(m[key])
        md[kind, f'meshes with {len(m["sets"])} set{"" if len(m["sets"]) == 1 else "s"}'] += bool(m['positions'])
        md[kind, 'skinned meshes' if m['weights'] else 'rigid meshes'] += bool(m['positions'])
        many = sum(len(w) > 4 for w in m['weights'])
        md[kind, 'skinned meshes with more than 4 influences on a vertex'] += many > 0
        md[kind, 'vertices with more than 4 influences'] += many
        zero = {k for k, n in enumerate(m['normals']) if n == (0.0, 0.0, 0.0)}
        corners = sum(n in zero for tr in m['triangles'] for n in tr[3:])
        md[kind, 'zero normals'] += len(zero)
        md[kind, 'triangle corners on a zero normal'] += corners
        md[kind, 'meshes with a corner on a zero normal'] += corners > 0
        p, n = m['positions'], m['normals']
        for tr in m['triangles']:
            e1 = [p[tr[1]][a] - p[tr[0]][a] for a in range(3)]
            e2 = [p[tr[2]][a] - p[tr[0]][a] for a in range(3)]
            cross = (e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0])
            dot = sum(x * sum(n[k][a] for k in tr[3:]) for a, x in enumerate(cross))
            md[kind, 'triangles with a winding test'] += dot != 0
            md[kind, 'triangles counter-clockwise around their normals'] += dot > 0
    md[kind, 'textures'] += len(model['textures'])
    for _, _, _, fmt, _ in model['textures']:
        md[kind, f'textures of format {fmt}'] += 1
    for _, texture, _ in model['materials']:
        md[kind, 'color materials' if texture is None else 'texture materials'] += 1
    drawn = collections.defaultdict(set)
    for mesh, material, sources, _ in model['passes']:
        md[kind, 'passes'] += 1
        md[kind, f'passes with sources {sources[0][0]}, {sources[1][0]}'] += 1
        drawn[mesh].add('color' if model['materials'][material][1] is None else 'texture')
    for kinds in drawn.values():
        if len(kinds) == 1:
            md[kind, f'meshes drawn with {next(iter(kinds))} materials only'] += 1


def mesh_crc(model):
    """CRC-32 over the meshes in order: the name and a NUL, then each list as a u32 count and its items, all u32 and
    f32 LE: positions, normals (f32[3]), sets (each a u32 count and f32[3] entries), triangles (u32[6]), weights (per
    vertex u32 n and n (u32 local bone, f32 weight)), bones (u32 bone, f32[7])."""
    crc = 0
    for m in model['meshes']:
        parts = [m['name'] + b'\0']
        for vectors in (m['positions'], m['normals']):
            parts += [struct.pack('<I', len(vectors))] + [struct.pack('<3f', *v) for v in vectors]
        parts.append(struct.pack('<I', len(m['sets'])))
        for entries in m['sets']:
            parts += [struct.pack('<I', len(entries))] + [struct.pack('<3f', *v) for v in entries]
        parts += [struct.pack('<I', len(m['triangles']))] + [struct.pack('<6I', *tr) for tr in m['triangles']]
        parts.append(struct.pack('<I', len(m['weights'])))
        for w in m['weights']:
            parts += [struct.pack('<I', len(w))] + [struct.pack('<If', *bw) for bw in w]
        parts += [struct.pack('<I', len(m['bones']))] + [struct.pack('<I7f', *b) for b in m['bones']]
        crc = zlib.crc32(b''.join(parts), crc)
    return crc


def material_crc(model):
    """CRC-32, all u32 and f32 LE, over: the textures (u32 count; each the name and a NUL, u32 width, height, format,
    payload length, the payload), the materials (u32 count; each u32 1, the name and a NUL, u32 texture for a texture
    material, or u32 0, u32 1 with the name and a NUL or u32 0 without a name, f32 rgba[4] for a color material) and
    the passes (u32 count; each u32 mesh, u32 material, per source u32 0 and f32[3] for rgb or u32 1 and u32 set, u32
    row count, the rows as u32s)."""
    parts = [struct.pack('<I', len(model['textures']))]
    for name, width, height, fmt, payload in model['textures']:
        parts += [name + b'\0', struct.pack('<4I', width, height, fmt, len(payload)), payload]
    parts.append(struct.pack('<I', len(model['materials'])))
    for name, texture, rgba in model['materials']:
        if texture is not None:
            parts += [struct.pack('<I', 1), name + b'\0', struct.pack('<I', texture)]
        else:
            parts += [struct.pack('<II', 0, name is not None), name + b'\0' if name is not None else b'',
                      struct.pack('<4f', *rgba)]
    parts.append(struct.pack('<I', len(model['passes'])))
    for mesh, material, sources, rows in model['passes']:
        parts.append(struct.pack('<II', mesh, material))
        for kind, value in sources:
            parts.append(struct.pack('<I3f', 0, *value) if kind == 'rgb' else struct.pack('<II', 1, value))
        parts += [struct.pack('<I', len(rows))] + [struct.pack(f'<{len(r)}I', *r) for r in rows]
    return zlib.crc32(b''.join(parts))


# Animation census statistics in print order; mode, frame interval and key count tallies follow.
ANIMATION_STATS = ('files with tracks', 'tracks', 'split tracks', 'interleaved tracks', 'interleaved frames',
                   'interleaved tracks without frames', 'files whose tracks animate every bone once',
                   'files with two tracks of a channel', 'files whose tracks end at different times',
                   'rotation keys', 'rotation keys off unit length by more than 1e-3',
                   'rotation keys in the other hemisphere than the previous key',
                   'rotation keys in the other hemisphere than the key two before',
                   'scale/shear keys', 'scale/shear keys off the identity', 'scale/shear keys off the diagonal',
                   'mode 0 curves with keys that differ by more than 1e-6')


def census_animation(an, durations, kind, tracks, channels):
    """Adds one animation to the census counter an: (kind, statistic) -> count; durations collects clip lengths."""
    if not tracks:
        return
    an[kind, 'files with tracks'] += 1
    by_channel = collections.Counter(ch for ch, _, _ in tracks)
    an[kind, 'files whose tracks animate every bone once'] += sorted(by_channel) == sorted(channels) and max(
        by_channel.values()) == 1
    an[kind, 'files with two tracks of a channel'] += max(by_channel.values()) > 1
    ends = {ts[-1] for _, _, curves in tracks for _, ts, _ in curves if ts}
    an[kind, 'files whose tracks end at different times'] += len(ends) > 1
    if ends:
        durations[kind].append(max(ends))
    for _, layout, curves in tracks:
        an[kind, 'tracks'] += 1
        if layout == LAYOUT_SPLIT:
            an[kind, 'split tracks'] += 1
            for name, (mode, _, keys) in zip(('translation', 'rotation', 'scale/shear'), curves):
                an[kind, f'{name} mode {mode}'] += 1
                if mode == 0 and any(abs(a - b) > 1e-6 for k in keys for a, b in zip(k, keys[0])):
                    an[kind, 'mode 0 curves with keys that differ by more than 1e-6'] += 1
                an[kind, f'{name} key counts {min(len(keys), 4)}{"+" if len(keys) >= 4 else ""}'] += 1
        else:
            an[kind, 'interleaved tracks'] += 1
            times = curves[0][1]
            an[kind, 'interleaved frames'] += len(times)
            an[kind, 'interleaved tracks without frames'] += not times
            for step in {round(b - a, 4) for a, b in zip(times, times[1:])}:
                an[kind, f'interleaved frame interval {step}'] += 1
        rotations = curves[1][2]
        an[kind, 'rotation keys'] += len(rotations)
        an[kind, 'rotation keys off unit length by more than 1e-3'] += sum(
            abs(math.sqrt(sum(x * x for x in q)) - 1) > 1e-3 for q in rotations)
        dot = lambda a, b: sum(x * y for x, y in zip(a, b))
        an[kind, 'rotation keys in the other hemisphere than the previous key'] += sum(
            dot(a, b) < 0 for a, b in zip(rotations, rotations[1:]))
        an[kind, 'rotation keys in the other hemisphere than the key two before'] += sum(
            dot(a, b) < 0 for a, b in zip(rotations, rotations[2:]))
        for m in curves[2][2]:
            an[kind, 'scale/shear keys'] += 1
            an[kind, 'scale/shear keys off the identity'] += any(abs(v - (k % 4 == 0)) > 1e-4 for k, v in enumerate(m))
            an[kind, 'scale/shear keys off the diagonal'] += any(abs(v) > 1e-4 for k, v in enumerate(m) if k % 4)


def animation_crc(tracks):
    """CRC-32, all u32 and f32 LE, over: the track count; per track its channel (from 0) and layout (1 split, 0
    interleaved), then per curve its mode, its key count, its times and its keys."""
    parts = [struct.pack('<I', len(tracks))]
    for channel, layout, curves in tracks:
        parts.append(struct.pack('<II', channel, layout))
        for mode, times, keys in curves:
            parts.append(struct.pack(f'<II{len(times)}f', mode, len(keys), *times))
            parts += [struct.pack(f'<{len(k)}f', *k) for k in keys]
    return zlib.crc32(b''.join(parts))


def skeleton_crc(bones):
    crc = 0
    for name, parent, floats in bones:
        crc = zlib.crc32(name + b'\0' + struct.pack('<I', parent) + floats, crc)
    return crc


def main():
    ap = argparse.ArgumentParser(description='Check MDL containers and the GRN container of their records; tag census.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--records', help='write one tab-separated row per record to this file')
    args = ap.parse_args()

    paks = []
    for rel in PAKS:
        header, records = read_mdl(os.path.join(args.game_dir, rel))
        check_mdl(rel, header, records)
        paks.append((header, records))
    check_models_tmp(os.path.join(args.game_dir, MODELS_TMP), paks)

    c = Check()
    signature = next(r[5][:GRN_SIGNATURE_SIZE] for _, records in paks for r in records if r[5])
    tags = {KIND_MODEL: collections.Counter(), KIND_MOTION: collections.Counter()}
    files_with = {KIND_MODEL: collections.Counter(), KIND_MOTION: collections.Counter()}
    rows = []
    non_empty = 0
    sk = collections.Counter()  # skeleton census: (kind, statistic) -> count
    md = collections.Counter()  # model census: (kind, statistic) -> count
    an = collections.Counter()  # animation census: (kind, statistic) -> count
    durations = collections.defaultdict(list)  # kind -> clip lengths in seconds
    for rel, (_, records) in zip(PAKS, paks):
        for i, kind, off, size, desc, g in records:
            name = desc[:32].split(b'\0')[0].decode('latin-1')
            data_off, data_size, flags = struct.unpack_from('<IIH', desc, 0x24)
            counts = collections.Counter()
            status, nodes, tree_crc, sk_crc, m_crc, mat_crc, an_crc = 'empty', [], '', '', '', '', ''
            if g:
                non_empty += 1
                status, sections = check_grn(g, c, f'{rel}#{i}', signature)
                if sections:
                    nodes = [len(s) for s in sections]
                    tree_crc = 0
                    for s in sections:
                        counts.update(t for t, _, _, _ in s)
                        for tag, start, end, descendants in s[:-1]:  # without the terminator
                            tree_crc = zlib.crc32(struct.pack('<3I', tag, end - start, descendants), tree_crc)
                    tags[kind].update(counts)
                    files_with[kind].update(counts.keys())
                    tree = ObjectTree(g, sections[1])
                    skeleton = read_skeleton(tree, c, f'{rel}#{i}')
                    if skeleton:
                        bones, objects, channels = skeleton
                        sk_crc = skeleton_crc(bones)
                        census_skeleton(sk, KIND_NAME[kind], bones, objects)
                        model = read_model(tree, objects, len(bones), c, f'{rel}#{i}')
                        if model:
                            m_crc, mat_crc = mesh_crc(model), material_crc(model)
                            census_model(md, KIND_NAME[kind], model)
                        n_channels = len(tree.kids[tree.top[0x0B01]])
                        tracks = read_animation(tree, n_channels, c, f'{rel}#{i}')
                        if tracks is not None:
                            an_crc = animation_crc(tracks)
                            c('track: its channel is the channel of a bone',
                              all(ch in set(channels) for ch, _, _ in tracks), f'{rel}#{i}')
                            census_animation(an, durations, KIND_NAME[kind], tracks, channels)
            motions = ''
            scale = ''
            if kind == KIND_MODEL:
                motions = ';'.join(f'{slot}:{m}' for slot, m in enumerate(u32s(desc, 0x70, 256)) if m)
                scale = ','.join(f'{bits:08x}' for bits in u32s(desc, 0x470, 3))
            rows.append(dict(pak=rel, index=i, kind=KIND_NAME.get(kind, hex(kind)), name=name, data_offset=data_off,
                             data_size=data_size, flags=flags, motions=motions, scale=scale, grn=status,
                             section_nodes=','.join(map(str, nodes)), tree_crc=tree_crc, bones=counts[TAG_BONE],
                             meshes=counts[TAG_MESH], tracks=counts[TAG_TRACK], skeleton_crc=sk_crc,
                             mesh_crc=m_crc, material_crc=mat_crc, animation_crc=an_crc))
    print(f'GRN: {non_empty} non-empty records, signature {signature[:8].hex()}…')
    c.report()
    for kind in (KIND_MODEL, KIND_MOTION):
        parsed = [r for r in rows if r['kind'] == KIND_NAME[kind] and r['grn'] == 'ok']
        print(f'  {KIND_NAME[kind]} records parsed: {len(parsed)}; tags (nodes / files):')
        for tag, total in sorted(tags[kind].items()):
            print(f'    {tag:#010x}: {total} / {files_with[kind][tag]}')
    for label, col in (('bones (0506)', 'bones'), ('meshes (0601)', 'meshes'), ('tracks (1204)', 'tracks')):
        for kind in (KIND_MODEL, KIND_MOTION):
            sel = [r[col] for r in rows if r['kind'] == KIND_NAME[kind] and r['grn'] == 'ok']
            print(f'  {label} in {KIND_NAME[kind]} records: files with any {sum(1 for v in sel if v)}, '
                  f'total {sum(sel)}, max {max(sel, default=0)}')
    print('  skeletons:')
    for kind in KIND_NAME.values():
        stats = ', '.join(f'{stat} {sk[kind, stat]}' for stat in SKELETON_STATS)
        print(f'    {kind}: {stats}')
    print('  models:')
    formats = sorted({stat for k, stat in md if stat.startswith('textures of format')})
    for kind in KIND_NAME.values():
        stats = ', '.join(f'{stat} {md[kind, stat]}' for stat in MODEL_STATS + tuple(formats))
        print(f'    {kind}: {stats}')
    print('  animations:')
    tallies = sorted({stat for k, stat in an if stat not in ANIMATION_STATS})
    for kind in KIND_NAME.values():
        stats = ', '.join(f'{stat} {an[kind, stat]}' for stat in ANIMATION_STATS + tuple(tallies) if an[kind, stat])
        lengths = sorted(durations[kind])
        if lengths:
            stats += (f'; clip length min {lengths[0]:.3f} s, median {lengths[len(lengths) // 2]:.3f} s, '
                      f'max {lengths[-1]:.3f} s')
        print(f'    {kind}: {stats}')

    if args.records:
        os.makedirs(os.path.dirname(os.path.abspath(args.records)), exist_ok=True)
        with open(args.records, 'w', newline='\n') as f:
            f.write('\t'.join(RECORD_COLUMNS) + '\n')
            for row in rows:
                f.write('\t'.join(str(row[col]) for col in RECORD_COLUMNS) + '\n')
        print(f'records: {len(rows)} rows -> {args.records}')


if __name__ == '__main__':
    main()
