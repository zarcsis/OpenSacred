#!/usr/bin/env python3
"""NetScript bytecode (bin/**/{FunkCode,StartCode,QuestCode,QuestPoolCode}.bin): the operands of every record, checks
over all 20 directories and per-file tables for cross-checking the Rust reader `opensacred_formats::netscript`.

Purpose (docs/02 §7.1; docs/re/netscript.md): the record framing (u16 type, u16 length with the header) was confirmed by
netscript_census.py; this script decodes the operands with the tag table recovered from the client's operand reader
(`FUN_00472BC0`: tag - 1 indexes the byte table at 0x4755D4, which selects one of 40 handlers in the jump table at
0x475534) and checks, on Sacred.exe itself, that every tag dispatches to the handler the table below assumes. The
record type names come from the five keyword resolvers of the script compiler (`--names`). Measured on the GOG files
on 2026-10-17.

Operand classes (the cursor advance of each handler, read from its code; docs/re/netscript.md lists the handlers):
  marker  zero width: the 66 tags of the default handler 0x474C47, and 0x71
  u8 u16 u32, u16x2 u16x4, i32x2 i32x3 i32x4: fixed payloads of 1, 2, 4, 4, 8, 8, 12, 16 bytes
  str     a NUL-terminated string
  strs    two NUL-terminated strings back to back (0x3A, 0x92)
  numstr  a u32 and a string (0x6F, 0x7A); 0x48 0x49 0x4A 0x5D 0x5E 0x6D 0x6E add a second string ("expression") when
          the u32 is negative and the byte after the first string is not 0
  vecname an i32x3, or a name: u32 0xFFFFFFFE and a string (0x04 0x0C 0x0D 0x3C)
  intname an i32, or a name: u32 0xFFED2979 and a string (0x0B 0x38 0x5F)
  flagstr a u8, and a string when it is 1 (0x37 0x4B)
  var     0x1F: a u32, or a variable reference 9F EF BE ED FE and a string in its place; 0x9F: the reference itself
  end     0x00, 0x17 0x18 0x21 0x22 0x76 0x94 and every tag from 0xA2: the reader logs an error and stops the record
Record type 40 is not a tag stream: its body is an 80-byte struct {i32 -1; char name[64]; i32 a, b, c}. Type 41's body is
a u16 from 1 to 5 (the walker `FUN_00475680` switches on it).

Vectoren.bin, read by the client at 0x46FDCA: three counted tables back to back to the end of the file.
  functions u32 n; n x 84 B {char name[64]; i32 offset; i32 length; i32 quest; i32 region; i32 0}: the function blocks
            of FunkCode.bin, contiguous from offset 0, on record bounds, to the end of FunkCode.bin
  quests    u32 n; n x 292 B {u32 id; char name[64]; 192 zero bytes; i32 values[8]}
  regions   u32 n; n entries {u32 id; char name[64]; i32 values[5]; 3 x (u32 k; u32 list[k]); u32 m; m x
            (u32 k; u32 list[k]; i32 values[4]; u8 tail[8])} (22 in the Sacred campaign trees, none elsewhere)
DefPos.bin is a cache the client writes (first u32 0x4D2, 0x46FC5C); the shipped files lack that mark, so the client
rebuilds them from the scripts, and this script does not read them.

Example
  uv run python -I tools/analysis/scripts/netscript_dump.py --game-dir ./OriginalGame
  uv run python -I tools/analysis/scripts/netscript_dump.py --game-dir ./OriginalGame --files /tmp/f.tsv --names /tmp/n.tsv

Expected output (2.28): every check passes on all of its items; 2,203,278 records in 51 non-empty files; 11 records of
type 50 end with tag 0x2A whose 12 bytes run past the record. --files writes a row per script file (path, bytes,
records, operands, truncated, crc32 of the canonical listing, see `canonical`); --signatures a row per record type
that occurs (its count, the number of distinct operand tag sequences and the three commonest); --names a row per record type with a
name; --vectors a row per Vectoren.bin (path, functions, quests, regions, crc32 of `canonical_vectors`).
Dependencies: pefile, capstone (pyproject.toml).
"""
import argparse
import collections
import glob
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, read_pe_image

SCRIPT_FILES = ('FunkCode.bin', 'StartCode.bin', 'QuestCode.bin', 'QuestPoolCode.bin')
RECORD_TYPES = 141
DIALOG, BLOCK, TRIGGER_PATCH = 40, 41, 50
DIALOG_SIZE = 80
NAME_SENTINEL, INT_NAME_SENTINEL, VARIABLE_MAGIC = 0xFFFFFFFE, 0xFFED2979, 0xFEEDBEEF

# Operand reader FUN_00472BC0: the byte table and the jump table, and the handler of each class.
INDEX_TABLE, JUMP_TABLE, ERROR_EXIT = 0x4755D4, 0x475534, 0x475504
CLASSES = [
    # (class, payload width, handler VA, tags)
    ('marker', 0, 0x474C47, None),  # every tag 0x01..0xA1 not listed below
    ('marker', 0, 0x474C52, [0x71]),
    ('u8', 1, 0x4747F3, [0x28]),
    ('u8', 1, 0x473A11, [0x8B]),
    ('u16', 2, 0x474DA2, [0x03, 0x0A, 0x6B, 0x6C, 0x9B, 0x9C]),
    ('u16', 2, 0x473A58, [0x93]),
    ('u16x2', 4, 0x4739D2, [0x84]),
    ('u16x4', 8, 0x47478C, [0x15]),
    ('u32', 4, 0x473A27, [0x02]),
    ('u32', 4, 0x474A46, [0x11, 0x36, 0x75]),
    ('u32', 4, 0x474A1E, [0x1C, 0x53, 0x54, 0x55, 0x56, 0x57, 0x86]),
    ('u32', 4, 0x474F90, [0x1D]),
    ('u32', 4, 0x473F54, [0x3B, 0x73]),
    ('u32', 4, 0x473A86, [0x7E, 0x7F, 0x8C, 0x90]),
    ('i32x2', 8, 0x473FBC, [0x33, 0x87, 0x88, 0x89]),
    ('i32x2', 8, 0x47402C, [0x34, 0x79]),
    ('i32x3', 12, 0x473F69, [0x19]),
    ('i32x3', 12, 0x474065, [0x20, 0x2A, 0x4D]),
    ('i32x3', 12, 0x473FF5, [0x35]),
    ('i32x4', 16, 0x474731, [0x3D]),
    ('str', None, 0x472C4D, [0x01, 0x29, 0x63, 0x68, 0x69, 0x6A, 0x9D]),
    ('str', None, 0x474823, [0x05, 0x09, 0x41, 0x60, 0x83]),
    ('str', None, 0x474DCD, [0x16]),
    ('str', None, 0x4751C4, [0x1E]),
    ('str', None, 0x473DF1, [0x3E, 0x77, 0x81, 0x82]),
    ('str', None, 0x4738E8, [0x40, 0x95]),
    ('str', None, 0x473A99, [0x47, 0x52, 0x7D, 0x8F]),
    ('str', None, 0x473257, [0x67]),
    ('strs', None, 0x473607, [0x3A]),
    ('strs', None, 0x4740AF, [0x92]),
    ('numstr', None, 0x473AF2, [0x6F]),
    ('numstr', None, 0x474A6E, [0x7A]),
    ('numstr2', None, 0x473C48, [0x48, 0x49, 0x4A, 0x5D, 0x5E, 0x6D, 0x6E]),
    ('vecname', None, 0x474181, [0x04, 0x0C, 0x0D]),
    ('vecname', None, 0x47463F, [0x3C]),
    ('intname', None, 0x47487C, [0x0B, 0x38, 0x5F]),
    ('flagstr', None, 0x474C4F, [0x37, 0x4B]),
    ('var', None, 0x4749AC, [0x1F]),
    ('varref', None, 0x4754A7, [0x9F]),
    ('end', None, ERROR_EXIT, [0x17, 0x18, 0x21, 0x22, 0x76, 0x94]),
]
TAG_CLASS = {}
for _cls, _width, _handler, _tags in CLASSES:
    for _t in _tags or []:
        TAG_CLASS[_t] = (_cls, _width)
FIXED_FORMAT = {'u8': '<B', 'u16': '<H', 'u16x2': '<2H', 'u16x4': '<4H', 'u32': '<i', 'i32x2': '<2i', 'i32x3': '<3i',
                'i32x4': '<4i'}

# Keyword resolvers of the script compiler: (function, stack slot of the first name pointer, stack slot of the first
# code); each fills two parallel stack arrays, the names ending in a null pointer, and returns codes[i] for names[i].
RESOLVERS = [(0x451BE0, 0x0C, 0x104), (0x452370, 0x14, 0x108), (0x452130, 0x0C, 0x2C), (0x452200, 0x0C, 0x28),
             (0x4522C0, 0x0C, 0x1C)]


def tag_class(tag):
    if tag == 0 or tag >= 0xA2:
        return ('end', None)
    return TAG_CLASS.get(tag, ('marker', 0))


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


class Truncated(Exception):
    pass


def cstr(body, p):
    end = body.find(b'\0', p)
    if end < 0:
        raise Truncated('string')
    return body[p:end], end + 1


def u32_at(body, p):
    if p + 4 > len(body):
        raise Truncated('u32')
    return struct.unpack_from('<I', body, p)[0]


def variable_at(body, p):
    """A variable reference `9F EF BE ED FE name\\0` at p (the helper FUN_00453970): (name, next) or None."""
    if p + 5 <= len(body) and body[p] == 0x9F and u32_at(body, p + 1) == VARIABLE_MAGIC:
        return cstr(body, p + 5)
    return None


def operands(body):
    """Decodes a tag stream: a list of (tag, kind, value); the last one is ('truncated', None) when a payload runs past
    the end. Raises ValueError on an end tag or a malformed operand."""
    out, p = [], 0
    while p < len(body):
        tag = body[p]
        p += 1
        cls, width = tag_class(tag)
        try:
            if cls == 'end':
                raise ValueError(f'end tag {tag:#04x} at body offset {p - 1}')
            if cls == 'marker':
                out.append((tag, 'marker', None))
            elif width is not None:
                if p + width > len(body):
                    raise Truncated(cls)
                value = struct.unpack_from(FIXED_FORMAT[cls], body, p)
                out.append((tag, cls, value if len(value) > 1 else value[0]))
                p += width
            elif cls == 'str':
                s, p = cstr(body, p)
                out.append((tag, 'str', s))
            elif cls == 'strs':
                s, p = cstr(body, p)
                s2, p = cstr(body, p)
                if not s2:
                    # The 0x3A handler measures the second string from its second byte.
                    raise ValueError(f'empty second string of tag {tag:#04x}')
                out.append((tag, 'strs', (s, s2)))
            elif cls in ('numstr', 'numstr2'):
                n = struct.unpack('<i', struct.pack('<I', u32_at(body, p)))[0]
                s, p = cstr(body, p + 4)
                expression = None
                if cls == 'numstr2' and n < 0 and p < len(body) and body[p]:
                    expression, p = cstr(body, p)
                out.append((tag, 'numstr', (n, s, expression)))
            elif cls == 'vecname':
                if u32_at(body, p) == NAME_SENTINEL:
                    s, p = cstr(body, p + 4)
                    out.append((tag, 'name', s))
                else:
                    if p + 12 > len(body):
                        raise Truncated(cls)
                    out.append((tag, 'i32x3', struct.unpack_from('<3i', body, p)))
                    p += 12
            elif cls == 'intname':
                n = u32_at(body, p)
                p += 4
                if n == INT_NAME_SENTINEL:
                    s, p = cstr(body, p)
                    out.append((tag, 'name', s))
                else:
                    out.append((tag, 'u32', struct.unpack('<i', struct.pack('<I', n))[0]))
            elif cls == 'flagstr':
                if p >= len(body):
                    raise Truncated(cls)
                flag = body[p]
                p += 1
                s = None
                if flag == 1:
                    s, p = cstr(body, p)
                out.append((tag, 'flagstr', (flag, s)))
            elif cls in ('var', 'varref'):
                ref = variable_at(body, p if cls == 'var' else p - 1)
                if ref is not None:
                    out.append((tag, 'var', ref[0]))
                    p = ref[1]
                elif cls == 'var':
                    out.append((tag, 'u32', struct.unpack('<i', struct.pack('<I', u32_at(body, p)))[0]))
                    p += 4
                else:
                    raise ValueError('tag 0x9f without a variable reference')
        except Truncated:
            out.append((tag, 'truncated', None))
            return out
    return out


def hexs(b):
    return '"' + b.hex() + '"'


def canonical_operand(tag, kind, value):
    """One operand of the canonical listing: `tt` or `tt=value`, strings as quoted hex."""
    t = f'{tag:02x}'
    if kind == 'marker':
        return t
    if kind == 'truncated':
        return t + '=truncated'
    if kind == 'str':
        return t + '=' + hexs(value)
    if kind == 'name':
        return t + '=name' + hexs(value)
    if kind == 'var':
        return t + '=var' + hexs(value)
    if kind == 'strs':
        return t + '=' + hexs(value[0]) + hexs(value[1])
    if kind == 'numstr':
        n, s, e = value
        return t + f'={n}' + hexs(s) + (hexs(e) if e is not None else '')
    if kind == 'flagstr':
        flag, s = value
        return t + f'={flag}' + (hexs(s) if s is not None else '')
    if isinstance(value, tuple):
        return t + '=' + ','.join(str(v) for v in value)
    return t + f'={value}'


def canonical(kind, body, ops):
    """One line per record: `type:` then the operands separated by spaces; type 40 as `40:dialog name a,b,c` and type
    41 as `41:block n`. The crc32 of the lines joined by newlines is the file's fingerprint."""
    if kind == DIALOG:
        head, = struct.unpack_from('<i', body, 0)
        name = body[4:68].split(b'\0')[0]
        a, b, c = struct.unpack_from('<3i', body, 68)
        return f'{kind}:dialog {head} {hexs(name)} {a},{b},{c}'
    if kind == BLOCK:
        return f'{kind}:block {struct.unpack_from("<H", body, 0)[0]}'
    return f'{kind}:' + ' '.join(canonical_operand(*o) for o in ops)


def records(data):
    """(offset, type, body) of every record; raises ValueError on a broken frame."""
    off = 0
    while off < len(data):
        if off + 4 > len(data):
            raise ValueError(f'record header at {off:#x} runs past the end')
        kind, length = struct.unpack_from('<HH', data, off)
        if length < 4 or off + length > len(data):
            raise ValueError(f'record at {off:#x} has length {length}')
        yield off, kind, data[off + 4:off + length]
        off += length


def zero_padded(field):
    """A NUL-terminated name and only zeros after it."""
    name = field.split(b'\0')[0]
    return not any(field[len(name):])


def read_vectoren(data):
    """The three tables of Vectoren.bin; raises ValueError when the file does not end with the last table."""
    p = 0

    def u32():
        nonlocal p
        if p + 4 > len(data):
            raise ValueError(f'u32 at {p:#x} runs past the end')
        p += 4
        return struct.unpack_from('<I', data, p - 4)[0]

    def take(n):
        nonlocal p
        if p + n > len(data):
            raise ValueError(f'{n} bytes at {p:#x} run past the end')
        p += n
        return data[p - n:p]

    def u32s(n):
        return list(struct.unpack('<%dI' % n, take(4 * n)))

    functions = []
    for _ in range(u32()):
        r = take(84)
        functions.append((r[:64], *struct.unpack_from('<5i', r, 64)))
    quests = []
    for _ in range(u32()):
        r = take(292)
        quests.append((struct.unpack_from('<I', r, 0)[0], r[4:68], r[68:260], struct.unpack_from('<8i', r, 260)))
    regions = []
    for _ in range(u32()):
        rid = u32()
        name = take(64)
        values = struct.unpack('<5i', take(20))
        lists = [u32s(u32()) for _ in range(3)]
        entries = []
        for _ in range(u32()):
            lst = u32s(u32())
            entries.append((lst, struct.unpack('<4i', take(16)), take(8)))
        regions.append((rid, name, values, lists, entries))
    if p != len(data):
        raise ValueError(f'{len(data) - p} bytes after the last table')
    return functions, quests, regions


def name_hex(field):
    return hexs(field.split(b'\0')[0])


def ints(values):
    return ','.join(str(v) for v in values)


def canonical_vectors(functions, quests, regions):
    """One line per function, quest, region and region entry; names as quoted hex of their bytes before the NUL."""
    lines = [f'f {name_hex(n)} {o} {length} {q} {r}' for n, o, length, q, r, _ in functions]
    lines += [f'q {i} {name_hex(n)} {ints(v)}' for i, n, _, v in quests]
    for rid, n, values, lists, entries in regions:
        lines.append(f'r {rid} {name_hex(n)} {ints(values)} ' + '|'.join(ints(lst) for lst in lists))
        lines += [f'e {ints(lst)} {ints(v)} {tail.hex()}' for lst, v, tail in entries]
    return lines


def check_dispatch(img, check):
    """Every tag 0x01..0xA1 reaches the handler of its class; 0x00 and 0xA2.. take the error exit (`ja` on tag - 1 >
    0xA0)."""
    index = [img.u8(INDEX_TABLE + i) for i in range(0xA1)]
    handlers = [img.u32(JUMP_TABLE + 4 * i) for i in range(max(index) + 1)]
    check('the operand reader has 40 handlers', len(handlers) == 40, len(handlers))
    expected = {}
    for _cls, _width, handler, tags in CLASSES:
        for t in tags or []:
            expected[t] = handler
    for tag in range(1, 0xA2):
        got = handlers[index[tag - 1]]
        check('every tag dispatches to the handler of its class', got == expected.get(tag, 0x474C47),
              f'{tag:#04x} -> {got:#x}')


def read_names(img):
    """Record type -> keyword, from the five resolvers: name pointers and codes stored into parallel stack slots."""
    names = {}
    for func, name_slot, code_slot in RESOLVERS:
        regs, slots = {}, {}
        for ins in img.disasm(func, func + 0x1000):
            m, o = ins.mnemonic, ins.op_str
            if m.startswith('ret'):
                break
            if m == 'xor' and o.split(', ')[0] == o.split(', ')[-1]:
                regs[o.split(', ')[0]] = 0
            elif m == 'mov' and o.startswith('e') and '[' not in o:
                reg, src = o.split(', ')
                regs[reg] = int(src, 16) if src.startswith('0x') else None
            elif m == 'mov' and o.startswith('dword ptr [esp + '):
                dst, src = o.split('], ')
                value = int(src, 16) if src.startswith('0x') else (int(src) if src.isdigit() else regs.get(src))
                slots[int(dst.split('+ ')[1], 16)] = value
        i = 0
        while slots.get(name_slot + 4 * i):
            name = img.bytes(slots[name_slot + 4 * i], 64).split(b'\0')[0].decode('ascii').rstrip(':')
            names.setdefault(slots[code_slot + 4 * i], set()).add(name)
            i += 1
    return names


def main():
    ap = argparse.ArgumentParser(description='NetScript operands: checks and per-file tables.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    ap.add_argument('--files', help='write a TSV row per script file')
    ap.add_argument('--names', help='write a TSV row per named record type')
    ap.add_argument('--vectors', help='write a TSV row per Vectoren.bin')
    ap.add_argument('--signatures', help='write a TSV row per record type: its count and its commonest operand tag '
                                         'sequences')
    args = ap.parse_args()

    check = Check()
    img = read_pe_image(os.path.join(args.game_dir, 'Sacred.exe'))
    check_dispatch(img, check)
    names = read_names(img)
    check('the resolvers name record types below 141', all(0 < k < RECORD_TYPES for k in names), sorted(names))

    rows = []
    bounds = {}
    kinds = collections.Counter()
    signatures = collections.defaultdict(collections.Counter)
    tags = collections.Counter()
    stats = collections.Counter()
    paths = sorted(p for f in SCRIPT_FILES for p in glob.glob(os.path.join(args.game_dir, 'bin', '**', f),
                                                              recursive=True))
    for path in paths:
        rel = os.path.relpath(path, args.game_dir).replace('\\', '/')
        data = open(path, 'rb').read()
        lines = []
        n_ops = n_truncated = n_records = 0
        try:
            frames = list(records(data))
            check('every file is a sequence of whole records', True)
        except ValueError as e:
            check('every file is a sequence of whole records', False, f'{rel}: {e}')
            continue
        for off, kind, body in frames:
            where = f'{rel}@{off:#x}'
            n_records += 1
            kinds[kind] += 1
            check('every record type is below 141', kind < RECORD_TYPES, where)
            check('every record type that occurs has a name or is 39-42', kind in names or 39 <= kind <= 42, where)
            ops = []
            if kind == DIALOG:
                ok = len(body) == DIALOG_SIZE and struct.unpack_from('<i', body, 0)[0] == -1 \
                    and not any(body[4:68][len(body[4:68].split(b'\0')[0]):]) and struct.unpack_from('<i', body, 76)[0] == 0
                check('type 40: {i32 -1, name[64] padded with zeros, i32, i32, i32 0}', ok, where)
            elif kind == BLOCK:
                check('type 41: a u16 from 1 to 5', len(body) == 2 and 1 <= struct.unpack('<H', body)[0] <= 5, where)
            else:
                try:
                    ops = operands(body)
                except ValueError as e:
                    check('every other record decodes to its end', False, f'{where}: {e}')
                    continue
                check('every other record decodes to its end', True)
                if ops and ops[-1][1] == 'truncated':
                    n_truncated += 1
                    check('only type 50 has a truncated operand, tag 0x2a at the end',
                          kind == TRIGGER_PATCH and ops[-1][0] == 0x2A and body[-1] == 0x2A, where)
                for tag, op_kind, value in ops:
                    tags[tag] += 1
                    stats[op_kind] += 1
                signatures[kind][' '.join(f'{tag:02x}' for tag, _, _ in ops)] += 1
            n_ops += len(ops)
            lines.append(canonical(kind, body, ops))
        stats['truncated records'] += n_truncated
        if rel.endswith('/FunkCode.bin'):
            bounds[os.path.dirname(rel)] = ({off for off, _, _ in frames} | {len(data)}, len(data))
        crc = zlib.crc32('\n'.join(lines).encode('ascii'))
        rows.append((rel, len(data), n_records, n_ops, n_truncated, f'{crc:08x}'))

    vector_rows = []
    for path in sorted(glob.glob(os.path.join(args.game_dir, 'bin', '**', 'Vectoren.bin'), recursive=True)):
        rel = os.path.relpath(path, args.game_dir).replace('\\', '/')
        try:
            functions, quests, regions = read_vectoren(open(path, 'rb').read())
            check('every Vectoren.bin ends with its third table', True)
        except ValueError as e:
            check('every Vectoren.bin ends with its third table', False, f'{rel}: {e}')
            continue
        starts, funk_size = bounds[os.path.dirname(rel)]
        offset = 0
        for name, start, length, _quest, _region, zero in functions:
            where = f'{rel}: {name_hex(name)}'
            check('function names are padded with zeros, the last field is 0', zero_padded(name) and zero == 0, where)
            check('functions are contiguous from 0, on record bounds of FunkCode.bin',
                  start == offset and start in starts and start + length in starts, where)
            offset = start + length
        check('the functions cover FunkCode.bin', offset == funk_size, rel)
        for qid, name, gap, values in quests:
            check('quest names are padded with zeros, then 192 zero bytes', zero_padded(name) and not any(gap),
                  f'{rel}: quest {qid}')
        for rid, name, *_ in regions:
            check('region names are padded with zeros', zero_padded(name), f'{rel}: region {rid}')
        crc = zlib.crc32('\n'.join(canonical_vectors(functions, quests, regions)).encode('ascii'))
        vector_rows.append((rel, len(functions), len(quests), len(regions), f'{crc:08x}'))

    check.report()
    total = sum(r[2] for r in rows)
    print(f'{len(rows)} files ({sum(1 for r in rows if r[1])} non-empty), {total} records, {len(kinds)} record types, '
          f'{len(tags)} operand tags')
    print(f'  operand kinds: {dict(sorted(stats.items()))}')
    print(f'  named record types: {len(names)}; shared names: '
          f'{ {k: sorted(v) for k, v in names.items() if len(v) > 1} }')
    print(f'  Vectoren.bin: {len(vector_rows)} files, {sum(r[1] for r in vector_rows)} functions, '
          f'{sum(r[2] for r in vector_rows)} quests, {sum(r[3] for r in vector_rows)} regions')
    for path, header, table in [(args.files, 'path\tbytes\trecords\toperands\ttruncated\tcrc', rows),
                                (args.vectors, 'path\tfunctions\tquests\tregions\tcrc', vector_rows),
                                (args.names, 'type\tname', [(k, '/'.join(sorted(v))) for k, v in sorted(names.items())]),
                                (args.signatures, 'type\tname\trecords\tsignatures\tcommonest',
                                 [(k, '/'.join(sorted(names.get(k, ()))), n, len(signatures[k]),
                                   '; '.join(f'{sig or "-"} ({c})' for sig, c in signatures[k].most_common(3)))
                                  for k, n in kinds.most_common()])]:
        if path:
            with open(path, 'w', encoding='ascii', newline='\n') as f:
                f.write(header + '\n')
                for row in table:
                    f.write('\t'.join(str(x) for x in row) + '\n')
            print(f'{len(table)} rows -> {path}')


if __name__ == '__main__':
    main()
