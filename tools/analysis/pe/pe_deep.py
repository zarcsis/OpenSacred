#!/usr/bin/env python3
"""Deep static analysis of an MSVC6 PE32: functions, calling conventions, import call sites,
class hierarchy from RTTI, vtables, exception tables (FuncInfo), SSE/x87, padding, subsystem labeling.

Purpose
  The basis of the "census" of the client and the server (docs/01 §4–5):
  10,823 client function starts, 410 vtables / 417 RTTI, 2,539 FuncInfo, import sites by DLL, layout per 64 KB,
  subsystem labels (ui / fx_particles / render / game_sim / stl / crt ...).

Examples
  python3 -I tools/analysis/pe/pe_deep.py --game-dir ./OriginalGame --exe OriginalGame/GameServer.exe --out /tmp/analysis/gs_deep.json
  python3 -I tools/analysis/pe/pe_deep.py OriginalGame/Sacred.exe --probe 84d964 --out /tmp/analysis/sacred_deep.json
  python3 -I tools/analysis/pe/pe_deep.py OriginalGame/Sacred.exe --no-disasm --out /tmp/analysis/rtti_only.json   # fast: RTTI/EH only

Output
  <out>: JSON with all counters (instructions, func_count, class_table, import_sites_*, layout_64k, ...);
  <out>.funcs.json: a list of [start, size, label, labelled_directly] for all function starts (input for
  granny_args.py, fn_census.py, fn_census2.py, thunk_check.py and fn_match.py). A full disassembly of the client takes several minutes.

Dependencies: pefile, capstone.
"""
import sys, struct, collections, bisect, re, json, argparse, time, math, statistics, os
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import *

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, hexkey

ap = argparse.ArgumentParser(description='Deep static analysis of an MSVC6 PE32 (RTTI, vtables, EH, imports, functions).')
ap.add_argument('path', nargs='?', help='PE file (default <game-dir>/Sacred.exe)')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--exe', help='alias for the positional path')
ap.add_argument('--out', required=True, help='output JSON (also writes <out>.funcs.json unless --no-disasm)')
ap.add_argument('--probe', type=hexkey, action='append', default=[], help='VA to hex-dump 48 bytes from the memory image')
ap.add_argument('--no-disasm', action='store_true')
args = ap.parse_args()
args.path = args.path or args.exe or os.path.join(args.game_dir, 'Sacred.exe')
t0 = time.time()
pe = pefile.PE(args.path)
base = pe.OPTIONAL_HEADER.ImageBase
img = bytearray(pe.get_memory_mapped_image())
secs = []
for s in pe.sections:
    name = s.Name.rstrip(b'\0').decode(errors='replace')
    secs.append((name, base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData), s))
def sec_of(va):
    for n, a, b, s in secs:
        if a <= va < b: return n
    return None
def rd32(va):
    off = va - base
    if off < 0 or off + 4 > len(img): return None
    return struct.unpack_from('<I', img, off)[0]
TXT = [x for x in secs if x[0] == '.text'][0]
TSTART, TSIZE_RAW = TXT[1], TXT[3].SizeOfRawData
TEND = TXT[1] + TXT[3].Misc_VirtualSize
def in_text(va): return TSTART <= va < TEND
RD = [(a, b) for n, a, b, s in secs if n in ('.rdata', '.data')]
def in_rd(v): return any(a <= v < b for a, b in RD)
R = {}

R['probes'] = {}
for p in args.probe:
    R['probes']['0x%x' % p] = bytes(img[p - base:p - base + 48]).hex()
    print("probe 0x%x: %s" % (p, R['probes']['0x%x' % p]))

iat = {}
for imp in pe.DIRECTORY_ENTRY_IMPORT:
    dll = imp.dll.decode().lower()
    for i in imp.imports:
        iat[i.address] = (dll, i.name.decode() if i.name else 'ord%d' % i.ordinal)
R['import_count'] = len(iat)
R['import_dlls'] = sorted(set(v[0] for v in iat.values()))

# ---------- RTTI ----------
def demangle(n):
    s = n[4:]
    def simple_t(c):
        return {'D': 'char', 'C': 'signed char', 'E': 'unsigned char', 'F': 'short', 'G': 'unsigned short', 'H': 'int', 'I': 'unsigned int', 'J': 'long', 'K': 'unsigned long', 'M': 'float', 'N': 'double', 'X': 'void'}.get(c, c)
    def parse_name(s, pos):
        parts = []
        while True:
            if s.startswith('?$', pos):
                pos += 2; end = s.index('@', pos); tname = s[pos:end]; pos = end + 1; targs = []
                while not s.startswith('@', pos):
                    if s[pos] in 'VU':
                        a, pos = parse_name(s, pos + 1); targs.append(a)
                    elif s[pos] == 'P':
                        pos += 1
                        if pos < len(s) and s[pos] in 'ABCD': pos += 1
                        if pos < len(s) and s[pos] in 'VU':
                            a, pos = parse_name(s, pos + 1); targs.append(a + '*')
                        else: targs.append(simple_t(s[pos]) + '*'); pos += 1
                    elif s[pos] == '$':
                        pos += 1
                        if s[pos] == '0':
                            pos += 1
                            if s[pos] in '0123456789': targs.append(str(int(s[pos]) + 1)); pos += 1
                            else:
                                end = s.index('@', pos); targs.append('0x' + s[pos:end].translate(str.maketrans('ABCDEFGHIJKLMNOP', '0123456789ABCDEF')).lower()); pos = end + 1
                        else: targs.append('?'); pos += 1
                    elif s[pos] == '_': targs.append({'N': 'bool', 'W': 'wchar_t', 'J': '__int64', 'K': 'unsigned __int64'}.get(s[pos + 1], '_' + s[pos + 1])); pos += 2
                    else: targs.append(simple_t(s[pos])); pos += 1
                pos += 1; parts.append(tname + '<' + ','.join(targs) + '>')
            else:
                end = s.index('@', pos); parts.append(s[pos:end]); pos = end + 1
            if s.startswith('@', pos): pos += 1; break
        return '::'.join(reversed(parts)), pos
    try: return parse_name(s, 0)[0]
    except Exception: return n
tds = {}
for m in re.finditer(rb'\.\?A[VU][^\0]{1,400}?\0', bytes(img)):
    raw = m.group(0)[:-1]
    if raw.endswith(b'@@'): tds[base + m.start() - 8] = raw.decode(errors='replace')
R['typedescriptors'] = len(tds)
cols = {}
for n, a, b, s in secs:
    if n not in ('.rdata', '.data'): continue
    d = bytes(img[a - base:b - base])
    for off in range(0, len(d) - 20, 4):
        if d[off:off + 4] != b'\0\0\0\0': continue
        ptd = struct.unpack_from('<I', d, off + 12)[0]
        if ptd in tds:
            pchd = struct.unpack_from('<I', d, off + 16)[0]
            if sec_of(pchd) in ('.rdata', '.data') and rd32(pchd) == 0:
                cols[a + off] = dict(offset=struct.unpack_from('<I', d, off + 4)[0], cdoff=struct.unpack_from('<I', d, off + 8)[0], td=ptd, chd=pchd)
R['col_count'] = len(cols)
vtables = {}; colset = set(cols)
for n, a, b, s in secs:
    if n not in ('.rdata', '.data'): continue
    d = bytes(img[a - base:b - base])
    for off in range(0, len(d) - 4, 4):
        v = struct.unpack_from('<I', d, off)[0]
        if v in colset:
            o = off + 4; entries = []
            while o + 4 <= len(d):
                p = struct.unpack_from('<I', d, o)[0]
                if in_text(p): entries.append(p); o += 4
                else: break
            vtables[a + off + 4] = (v, entries)
R['vtable_count'] = len(vtables); R['vtable_entries_total'] = sum(len(e) for _, e in vtables.values())
virt_funcs = set(p for _, e in vtables.values() for p in e); R['vtable_distinct_targets'] = len(virt_funcs)
classes = {}
for col_va, c in cols.items():
    td = c['td']; name = demangle(tds[td])
    cl = classes.setdefault(td, dict(name=name, mangled=tds[td], bases=[], all_bases=[], vtables=0, vslots=0, multiple=False, virtual_inh=False))
    cl['vtables'] += 1
    if c['offset'] == 0:
        chd = c['chd']; sig, attrs, nb, pbca = rd32(chd), rd32(chd + 4), rd32(chd + 8), rd32(chd + 12)
        cl['multiple'] = bool(attrs & 1); cl['virtual_inh'] = bool(attrs & 2)
        infos = []
        for i in range(nb):
            p = rd32(pbca + 4 * i); infos.append(dict(td=rd32(p), ncb=rd32(p + 4)))
        cl['all_bases'] = [demangle(tds.get(i['td'], '?')) for i in infos[1:]]
        idx = 1; direct = []
        while idx < len(infos):
            direct.append(demangle(tds.get(infos[idx]['td'], '?'))); idx += 1 + infos[idx]['ncb']
        cl['bases'] = direct
for vt, (col_va, entries) in vtables.items():
    td = cols[col_va]['td']
    if td in classes: classes[td]['vslots'] = max(classes[td]['vslots'], len(entries))
no_vtable = [demangle(n) for td, n in tds.items() if td not in classes]
R['classes_with_vtable'] = len(classes); R['typedesc_without_vtable'] = len(no_vtable)
R['class_table'] = sorted([dict(name=c['name'], bases=c['bases'], all_bases=c['all_bases'], vtables=c['vtables'], vslots=c['vslots'], mi=c['multiple'], vi=c['virtual_inh']) for c in classes.values()], key=lambda x: x['name'].lower())
R['typedesc_without_vtable_names'] = sorted(no_vtable)
R['class_list'] = sorted(set(demangle(n) for n in tds.values()))

funcinfos = []
for n, a, b, s in secs:
    if n not in ('.rdata', '.data', '.text'): continue
    d = bytes(img[a - base:b - base])
    for m in re.finditer(re.escape(struct.pack('<I', 0x19930520)), d):
        if m.start() % 4: continue
        va = a + m.start()
        maxstate, punwind, ntry, ptry, nip, pip = [rd32(va + 4 + 4 * i) for i in range(6)]
        if 0 <= maxstate < 5000 and ntry < 1000 and (ntry == 0 or sec_of(ptry)) and (maxstate == 0 or sec_of(punwind)):
            funcinfos.append((va, maxstate, ntry, nip))
R['funcinfo_count'] = len(funcinfos); R['funcinfo_try_blocks'] = sum(f[2] for f in funcinfos)
R['funcinfo_with_try'] = sum(1 for f in funcinfos if f[2] > 0); R['funcinfo_total_states'] = sum(f[1] for f in funcinfos)
if args.no_disasm:
    json.dump(R, open(args.out, 'w', encoding='utf-8', newline=''), indent=1); sys.exit(0)

# ---------- string helpers ----------
def str_at(v):
    off = v - base
    if off <= 0 or off >= len(img): return None
    if 32 <= img[off - 1] < 127: return None
    end = off
    while end < len(img) and 32 <= img[end] < 127 and end - off < 300: end += 1
    if end - off >= 4 and end < len(img) and img[end] == 0: return bytes(img[off:end]).decode('ascii', 'replace')
    return None
CRT_MARK = re.compile(r'^R6\d\d|runtime error|Microsoft Visual C\+\+|GetLastActivePopup|1#QNAN|1#INF|1#IND|1#SNAN|^\(null\)$|^TLOSS|^SING$|^DOMAIN$|_nextafter|^LC_|^KERNEL32|^GetProcAddress|GetEnvironmentStrings|CRT|^mscoree|_matherr|^-\s?floating|not enough space|pure virtual function call|^abnormal program termination|Runtime Library')
def label_of_string(s):
    if s.startswith('D3DX') or 'd3dx' in s.lower(): return 'd3dx'
    if CRT_MARK.search(s): return 'crt'
    if '_STL' in s or 'ios_base' in s or 'basic_string' in s or 'locale' in s.lower() or 'facet' in s.lower(): return 'stl'
    return None
def subsys_of_class(cn):
    c = cn
    if c.startswith('_STL') or '_STL::' in c: return 'stl'
    if c.startswith('cUI_') or c.startswith('Command') or c.startswith('cCommand'): return 'ui'
    if c.startswith('cParticleSystem'): return 'fx_particles'
    if c.startswith('cEvent'): return 'events'
    if c.startswith('cFile'): return 'fileio'
    if c.startswith(('dxDriver', 'cDx', 'dxDisplay', 'cTexture', 'cDX', 'cFont', 'cFsFont', 'cRender', 'cSprite', 'cScreen', 'cShadow', 'cLight')): return 'render'
    if c.startswith(('cMSS', 'cSound', 'cMusic', 'cSnd')): return 'audio'
    if c.startswith(('cGranny', 'cGrn', 'cGranny')): return 'animation'
    if c.startswith(('TinCat', 'ITinCat', 'cNet', 'cEventNetwork', 'cLobby', 'cClient', 'cServer')): return 'network'
    if c.startswith(('NA2dLib', 'CIMHandler', 'CW32', 'cIME')): return 'ime'
    if c.startswith(('cScript', 'cInterpret', 'TypeManager', 'cCompiler')): return 'script'
    if c.startswith(('cKernel', 'cEngine', 'cObserver', 'cTimer', 'cLog', 'cCalendar', 'cResource', 'cStats')): return 'core'
    return 'game_sim'

# ---------- disassembly ----------
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True; md.skipdata = True
code = bytes(img[TSTART - base:TEND - base])
insn_addr = []; mnems = collections.Counter(); grp = collections.Counter()
call_targets = collections.Counter(); calls_out = 0; ind_mem_calls = 0; ind_reg_calls = 0
iat_calls = collections.Counter(); thunks = {}; switch_jmps = 0; fs0 = 0
prologues = set(); rets = {}; leaves = set(); ecxacc = {}; edxacc = {}
push_imm_text = collections.Counter(); mov_eax_imm = {}; strrefs = []
rep_movs = 0; rep_stos = 0; cpuid = 0; fnstsw = 0; fcomi = 0; sahf = 0; nbyte = 0; prev = None; simd_layout = collections.Counter()
ECX = {X86_REG_ECX, X86_REG_CX, X86_REG_CL, X86_REG_CH}; EDX = {X86_REG_EDX, X86_REG_DX, X86_REG_DL, X86_REG_DH}
FGRP = ('sse1', 'sse2', 'sse3', 'ssse3', 'sse41', 'sse42', 'mmx', 'fpu', '3dnow', 'cmov', 'avx')
for ins in md.disasm(code, TSTART):
    insn_addr.append(ins.address)
    if ins.id == 0: nbyte += 1; prev = ins; continue
    mnems[ins.mnemonic] += 1
    for g in ins.groups:
        gn = ins.group_name(g)
        if gn in FGRP:
            grp[gn] += 1
            if gn in ('sse1', 'sse2', 'mmx', '3dnow'): simd_layout[(ins.address - TSTART) // 0x10000] += 1
    iid = ins.id; ops = ins.operands
    if iid == X86_INS_CALL:
        op = ops[0]
        if op.type == X86_OP_IMM:
            if in_text(op.imm): call_targets[op.imm] += 1
            else: calls_out += 1
        elif op.type == X86_OP_MEM:
            if op.mem.base == 0 and op.mem.index == 0: iat_calls[op.mem.disp] += 1
            else: ind_mem_calls += 1
        else: ind_reg_calls += 1
    elif iid == X86_INS_JMP:
        op = ops[0]
        if op.type == X86_OP_MEM:
            if op.mem.base == 0 and op.mem.index == 0:
                if op.mem.disp in iat: thunks[ins.address] = op.mem.disp
            elif op.mem.scale == 4: switch_jmps += 1
    elif iid == X86_INS_RET: rets[ins.address] = ops[0].imm if ops else -1
    elif iid == X86_INS_LEAVE: leaves.add(ins.address)
    elif iid == X86_INS_CPUID: cpuid += 1
    elif iid == X86_INS_FNSTSW: fnstsw += 1
    elif ins.mnemonic in ('fcomi', 'fcomip', 'fucomi', 'fucomip'): fcomi += 1
    elif iid == X86_INS_SAHF: sahf += 1
    elif iid == X86_INS_PUSH and ops[0].type == X86_OP_IMM and in_text(ops[0].imm): push_imm_text[ops[0].imm] += 1
    elif iid == X86_INS_MOV and ops[0].type == X86_OP_REG and ops[0].reg == X86_REG_EAX and ops[1].type == X86_OP_IMM: mov_eax_imm[ins.address] = ops[1].imm
    if iid in (X86_INS_MOVSD, X86_INS_MOVSB, X86_INS_MOVSW) and ins.bytes[0] == 0xf3: rep_movs += 1
    if iid in (X86_INS_STOSD, X86_INS_STOSB) and ins.bytes[0] == 0xf3: rep_stos += 1
    if ins.bytes[:2] == b'\x8b\xec' and prev is not None and prev.bytes == b'\x55': prologues.add(prev.address)
    if 'fs:[0]' in ins.op_str: fs0 += 1
    for op in ops:
        v = None
        if op.type == X86_OP_IMM: v = op.imm
        elif op.type == X86_OP_MEM and op.mem.disp > 0x10000: v = op.mem.disp
        if v is not None and in_rd(v):
            s = str_at(v)
            if s: strrefs.append((ins.address, s))
    try: r, w = ins.regs_access()
    except Exception: r, w = (), ()
    rs, ws = set(r), set(w)
    if iid == X86_INS_PUSH or (iid in (X86_INS_XOR, X86_INS_SUB) and ops[0].type == X86_OP_REG and ops[1].type == X86_OP_REG and ops[0].reg == ops[1].reg): rs = set()
    if iid == X86_INS_LEA: rs -= {ops[0].reg}
    if rs & ECX or ws & ECX: ecxacc[ins.address] = ('r' if rs & ECX else '') + ('w' if ws & ECX else '')
    if rs & EDX or ws & EDX: edxacc[ins.address] = ('r' if rs & EDX else '') + ('w' if ws & EDX else '')
    prev = ins
print("disassembled %d instructions in %.1fs" % (len(insn_addr), time.time() - t0))
R['instructions'] = len(insn_addr); R['skipdata_bytes'] = nbyte
R['mnemonic_top40'] = mnems.most_common(40); R['groups'] = dict(grp)
R['calls_direct_sites'] = sum(call_targets.values()); R['calls_direct_targets'] = len(call_targets)
R['calls_outside_text'] = calls_out; R['calls_indirect_mem'] = ind_mem_calls; R['calls_indirect_reg'] = ind_reg_calls
R['switch_jump_tables'] = switch_jmps; R['fs0_refs'] = fs0; R['cpuid'] = cpuid
R['fnstsw'] = fnstsw; R['fcomi'] = fcomi; R['sahf'] = sahf; R['rep_movs'] = rep_movs; R['rep_stos'] = rep_stos
R['leave_count'] = len(leaves); R['prologue_count'] = len(prologues); R['strrefs'] = len(strrefs)
imp_sites = collections.Counter()
for ia, c in iat_calls.items():
    if ia in iat: imp_sites[iat[ia]] += c
for t, ia in thunks.items(): imp_sites[iat[ia]] += call_targets.get(t, 0)
bydll = collections.Counter()
for k, v in imp_sites.items(): bydll[k[0]] += v
R['import_sites_by_dll'] = dict(bydll)
R['import_sites_top'] = [('%s!%s' % k, v) for k, v in imp_sites.most_common(100)]
R['import_sites_all'] = sorted([('%s!%s' % k, v) for k, v in imp_sites.items()])
R['imports_never_called_directly'] = sorted('%s!%s' % v for a, v in iat.items() if imp_sites.get(v, 0) == 0)
R['thunk_count'] = len(thunks)
fi_set = set(f[0] for f in funcinfos)
eh_thunks = [a for a, imm in mov_eax_imm.items() if imm in fi_set]
R['eh_handler_thunks_mov_eax_funcinfo'] = len(eh_thunks)
ehh = set(eh_thunks)
R['cxx_eh_frames_push_handler'] = sum(c for a, c in push_imm_text.items() if a in ehh)
seh3 = set(t for t, ia in thunks.items() if iat[ia][1] == '_except_handler3')
R['seh3_frames_push_handler_import'] = sum(push_imm_text[t] for t in seh3)
# push handler where handler is a text address that is not an EH thunk and is pushed right before fs:[0] (approx: count pushes of text addresses not in call targets and not EH thunks)
R['push_text_addr_non_eh'] = sum(c for a, c in push_imm_text.items() if a not in ehh)
# ---------- functions ----------
starts = set(call_targets) | prologues | (virt_funcs & set(insn_addr)) | set(eh_thunks)
starts = sorted(a for a in starts if in_text(a))
R['func_starts_union'] = len(starts)
R['func_starts_call_targets_only'] = len(set(call_targets) - prologues)
R['func_starts_prologue_only'] = len(prologues - set(call_targets))
R['func_virtual_not_called_directly'] = len((virt_funcs & set(insn_addr)) - set(call_targets))
addr_index = {a: i for i, a in enumerate(insn_addr)}
ret_addrs = sorted(rets); leave_sorted = sorted(leaves)
sizes = []; pad_cc = 0; pad_90 = 0; gaps = collections.Counter(); align16 = 0; align4 = 0
conv = collections.Counter(); frame = collections.Counter(); noret = 0; lf = 0
for i, s in enumerate(starts):
    e = starts[i + 1] if i + 1 < len(starts) else TEND
    j = e - 1 - base; pad = 0
    while j > s - base and img[j] in (0xCC, 0x90): pad += 1; j -= 1
    if pad:
        if img[e - 1 - base] == 0xCC: pad_cc += 1
        else: pad_90 += 1
    gaps[pad] += 1; sz = e - s - pad; sizes.append(sz)
    if s % 16 == 0: align16 += 1
    if s % 4 == 0: align4 += 1
    k = addr_index.get(s); ecx_in = edx_in = False; ecx_seen = edx_seen = False
    if k is not None:
        for q in range(k, min(k + 25, len(insn_addr))):
            a = insn_addr[q]
            if a >= e: break
            if not ecx_seen and a in ecxacc: ecx_seen = True; ecx_in = 'r' in ecxacc[a]
            if not edx_seen and a in edxacc: edx_seen = True; edx_in = 'r' in edxacc[a]
            if ecx_seen and edx_seen: break
    k0 = bisect.bisect_left(ret_addrs, s); k1 = bisect.bisect_left(ret_addrs, e)
    retimms = [rets[a] for a in ret_addrs[k0:k1]]
    has_retn = any(x > 0 for x in retimms); has_ret = bool(retimms)
    if not has_ret: noret += 1
    if ecx_in and edx_in: c = 'fastcall?'
    elif ecx_in: c = 'thiscall' + ('(n)' if has_retn else '')
    elif has_retn: c = 'stdcall'
    else: c = 'cdecl/noargs'
    conv[c] += 1; frame['ebp-frame' if s in prologues else 'no-ebp-frame'] += 1
    kl = bisect.bisect_left(leave_sorted, s)
    if kl < len(leave_sorted) and leave_sorted[kl] < e: lf += 1
R['func_count'] = len(sizes); R['func_size_mean'] = statistics.mean(sizes); R['func_size_median'] = statistics.median(sizes)
R['func_size_deciles'] = statistics.quantiles(sizes, n=10)
buckets = [(0, 16), (17, 64), (65, 256), (257, 1024), (1025, 4096), (4097, 10 ** 9)]
R['func_size_hist'] = {('%d-%d' % (a, b) if b < 10 ** 9 else '>%d' % (a - 1)): sum(1 for x in sizes if a <= x <= b) for a, b in buckets}
R['func_bytes_total'] = sum(sizes); R['pad_runs_cc'] = pad_cc; R['pad_runs_90'] = pad_90
R['gap_hist'] = dict(sorted(gaps.items())[:20]); R['align16'] = align16; R['align4'] = align4
R['conv'] = dict(conv); R['frame'] = dict(frame); R['funcs_without_ret'] = noret; R['funcs_with_leave'] = lf
R['ret_plain'] = sum(1 for v in rets.values() if v == -1); R['ret_n'] = sum(1 for v in rets.values() if v > 0)
R['largest_funcs'] = [('0x%x' % a, sz) for sz, a in sorted(zip(sizes, starts), reverse=True)[:15]]
# ---------- subsystem attribution ----------
func_of = lambda a: starts[bisect.bisect_right(starts, a) - 1] if a >= starts[0] else None
labels = {}  # func start -> label
label_src = collections.Counter()
for vt, (col_va, entries) in vtables.items():
    cn = demangle(tds[cols[col_va]['td']]); lab = subsys_of_class(cn)
    for p in entries:
        f = func_of(p)
        if f == p: labels.setdefault(p, lab); label_src['vtable'] += 1
cm_re = re.compile(r'\b([A-Za-z_][A-Za-z0-9_]*)::(~?[A-Za-z_][A-Za-z0-9_]*)')
d3dx_strings = collections.Counter(); crt_strings = collections.Counter()
for a, s in strrefs:
    f = func_of(a)
    if f is None: continue
    lab = label_of_string(s)
    if lab == 'd3dx': d3dx_strings[s] += 1
    if lab == 'crt': crt_strings[s] += 1
    if lab is None:
        m = cm_re.search(s)
        if m: lab = subsys_of_class(m.group(1))
        elif re.search(r'\.(tga|pak|grn|wmv|bmp|ptx|pax|bin|txt|cfg)\b', s, re.I): lab = 'game_sim'
    if lab:
        if f not in labels: labels[f] = lab; label_src['string'] += 1
R['label_sources'] = dict(label_src); R['labelled_funcs'] = len(labels)
R['d3dx_strings_sample'] = d3dx_strings.most_common(25); R['crt_strings_sample'] = crt_strings.most_common(25)
size_of = dict(zip(starts, sizes))
lab_bytes = collections.Counter(); lab_funcs = collections.Counter()
for f, lab in labels.items(): lab_bytes[lab] += size_of[f]; lab_funcs[lab] += 1
R['labelled_bytes'] = dict(lab_bytes); R['labelled_func_counts'] = dict(lab_funcs)
# propagate: nearest labelled function before (within 24 KB) else after (within 24 KB)
lab_starts = sorted(labels)
prop_bytes = collections.Counter(); prop_funcs = collections.Counter(); prop_label = {}
for s in starts:
    if s in labels: lab = labels[s]
    else:
        k = bisect.bisect_right(lab_starts, s) - 1
        prevl = lab_starts[k] if k >= 0 else None; nextl = lab_starts[k + 1] if k + 1 < len(lab_starts) else None
        dp = s - prevl if prevl is not None else 10 ** 9; dn = nextl - s if nextl is not None else 10 ** 9
        if min(dp, dn) > 24576: lab = 'unlabelled'
        else: lab = labels[prevl] if dp <= dn else labels[nextl]
    prop_bytes[lab] += size_of[s]; prop_funcs[lab] += 1; prop_label[s] = lab
R['propagated_bytes'] = dict(prop_bytes); R['propagated_func_counts'] = dict(prop_funcs)
cil = collections.Counter(); cil_from_game = collections.Counter()
for t, c in call_targets.items():
    f = func_of(t)
    if f is None: continue
    cil[prop_label.get(f, '?')] += c
R['call_sites_into_label'] = dict(cil)
R['simd_layout_64k'] = {('0x%x' % (TSTART + k * 0x10000)): v for k, v in sorted(simd_layout.items())}
R['prop_label_sample'] = {('0x%x' % a): l for a, l in list(prop_label.items())[:5]}
# layout per 64 KB bucket
BK = 0x10000; layout = collections.defaultdict(collections.Counter)
for s in starts:
    lab = labels.get(s)
    if lab: layout[(s - TSTART) // BK][lab] += size_of[s]
R['layout_64k'] = {('0x%x' % (TSTART + k * BK)): dict(v.most_common(4)) for k, v in sorted(layout.items())}
# RTTI helper call sites (static CRT __RTDynamicCast / __RTtypeid identified by their diagnostic strings)
helper_strings = {'Bad dynamic_cast!': 'dynamic_cast', 'Attempted a typeid of NULL pointer!': 'typeid', 'Access violation - no RTTI data!': 'rtti_check', 'Bad read pointer - no RTTI data!': 'rtti_check2'}
hs = {}
for a, st in strrefs:
    if st in helper_strings:
        f = func_of(a)
        if f is not None: hs[helper_strings[st]] = (hex(f), call_targets.get(f, 0))
R['rtti_helper_callsites'] = hs
json.dump([[s_, size_of[s_], prop_label.get(s_, '?'), s_ in labels] for s_ in starts], open(args.out + '.funcs.json', 'w', encoding='utf-8', newline=''))
R['elapsed_s'] = time.time() - t0
json.dump(R, open(args.out, 'w', encoding='utf-8', newline=''), indent=1, default=str)
print("done in %.1fs -> %s" % (R['elapsed_s'], args.out))
