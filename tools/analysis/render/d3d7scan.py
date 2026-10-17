#!/usr/bin/env python3
"""List of indirect calls through IDirect3DDevice7 vtable slots in the Sacred.exe client with recovered
push arguments: SetRenderState (state name and value), SetTextureStageState (stage, type, operation/argument),
DrawPrimitive*, SetTexture, Clear, SetTransform, etc. Slot offsets = method index × 4 in the order of the
IDirect3DDevice7 methods (wine include/d3d.h: facts of the public API, no code is copied).

Purpose (docs/05 §1): the actual D3D7 render contract is ~17 render states, 2 texture stages, ZFUNC
LESSEQUAL/GREATER, ALPHAREF 0x40, stencil, no fog. Heuristic: a site counts if it is `call [reg+disp]` with disp from
the slot table; false positives are possible on other interfaces with the same offset, so cross-check with
vt=(reg,disp).

Example
  python3 -I tools/analysis/render/d3d7scan.py --game-dir ./OriginalGame --out /tmp/analysis/geom

Output: <out>/d3d7scan.txt (sites by slot), d3d7scan.json, d3d7scan_summary.txt (state,value -> number of sites);
the summary is printed to stdout. Dependencies: pefile, capstone.
"""
import sys, os, struct, json, collections, argparse
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR, Img, hexkey
from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG, X86_INS_CALL, X86_INS_PUSH, X86_INS_RET, X86_INS_JMP, X86_INS_MOV

ap = argparse.ArgumentParser(description='IDirect3DDevice7 vtable call sites with reconstructed arguments.')
ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
ap.add_argument('--exe', help='Sacred.exe (default <game-dir>/Sacred.exe)')
ap.add_argument('--out', required=True, help='output directory')
args = ap.parse_args()
exe = args.exe or os.path.join(args.game_dir, 'Sacred.exe')
os.makedirs(args.out, exist_ok=True)
OUT = os.path.join(args.out, '')
DEV = {0x0C:('GetCaps',1),0x10:('EnumTextureFormats',2),0x14:('BeginScene',0),0x18:('EndScene',0),0x1C:('GetDirect3D',1),0x20:('SetRenderTarget',2),0x24:('GetRenderTarget',1),
 0x28:('Clear',6),0x2C:('SetTransform',2),0x30:('GetTransform',2),0x34:('SetViewport',1),0x38:('MultiplyTransform',2),0x3C:('GetViewport',1),0x40:('SetMaterial',1),0x44:('GetMaterial',1),
 0x48:('SetLight',2),0x4C:('GetLight',2),0x50:('SetRenderState',2),0x54:('GetRenderState',2),0x58:('BeginStateBlock',0),0x5C:('EndStateBlock',1),0x60:('PreLoad',1),
 0x64:('DrawPrimitive',5),0x68:('DrawIndexedPrimitive',7),0x6C:('SetClipStatus',1),0x70:('GetClipStatus',1),0x74:('DrawPrimitiveStrided',5),0x78:('DrawIndexedPrimitiveStrided',7),
 0x7C:('DrawPrimitiveVB',5),0x80:('DrawIndexedPrimitiveVB',6),0x84:('ComputeSphereVisibility',5),0x88:('GetTexture',2),0x8C:('SetTexture',2),0x90:('GetTextureStageState',3),
 0x94:('SetTextureStageState',3),0x98:('ValidateDevice',1),0x9C:('ApplyStateBlock',1),0xA0:('CaptureStateBlock',1),0xA4:('DeleteStateBlock',1),0xA8:('CreateStateBlock',2),0xAC:('Load',5),
 0xB0:('LightEnable',2),0xB4:('GetLightEnable',2),0xB8:('SetClipPlane',2),0xBC:('GetClipPlane',2),0xC0:('GetInfo',3)}
RS = {7:'ZENABLE',8:'FILLMODE',9:'SHADEMODE',14:'ZWRITEENABLE',15:'ALPHATESTENABLE',16:'LASTPIXEL',19:'SRCBLEND',20:'DESTBLEND',22:'CULLMODE',23:'ZFUNC',24:'ALPHAREF',25:'ALPHAFUNC',26:'DITHERENABLE',27:'ALPHABLENDENABLE',28:'FOGENABLE',29:'SPECULARENABLE',30:'ZVISIBLE',33:'STIPPLEDALPHA',34:'FOGCOLOR',35:'FOGTABLEMODE',36:'FOGSTART',37:'FOGEND',38:'FOGDENSITY',40:'EDGEANTIALIAS',41:'COLORKEYENABLE',47:'ZBIAS',48:'RANGEFOGENABLE',52:'STENCILENABLE',53:'STENCILFAIL',54:'STENCILZFAIL',55:'STENCILPASS',56:'STENCILFUNC',57:'STENCILREF',58:'STENCILMASK',59:'STENCILWRITEMASK',60:'TEXTUREFACTOR',136:'CLIPPING',137:'LIGHTING',138:'EXTENTS',139:'AMBIENT',140:'FOGVERTEXMODE',141:'COLORVERTEX',142:'LOCALVIEWER',143:'NORMALIZENORMALS',144:'COLORKEYBLENDENABLE',145:'DIFFUSEMATERIALSOURCE',146:'SPECULARMATERIALSOURCE',147:'AMBIENTMATERIALSOURCE',148:'EMISSIVEMATERIALSOURCE',151:'VERTEXBLEND',152:'CLIPPLANEENABLE'}
for i in range(128, 136): RS[i] = 'WRAP%d' % (i - 128)
TSS = {1:'COLOROP',2:'COLORARG1',3:'COLORARG2',4:'ALPHAOP',5:'ALPHAARG1',6:'ALPHAARG2',7:'BUMPENVMAT00',8:'BUMPENVMAT01',9:'BUMPENVMAT10',10:'BUMPENVMAT11',11:'TEXCOORDINDEX',12:'ADDRESS',13:'ADDRESSU',14:'ADDRESSV',15:'BORDERCOLOR',16:'MAGFILTER',17:'MINFILTER',18:'MIPFILTER',19:'MIPMAPLODBIAS',20:'MAXMIPLEVEL',21:'MAXANISOTROPY',22:'BUMPENVLSCALE',23:'BUMPENVLOFFSET',24:'TEXTURETRANSFORMFLAGS'}
TOP = {1:'DISABLE',2:'SELECTARG1',3:'SELECTARG2',4:'MODULATE',5:'MODULATE2X',6:'MODULATE4X',7:'ADD',8:'ADDSIGNED',9:'ADDSIGNED2X',10:'SUBTRACT',11:'ADDSMOOTH',12:'BLENDDIFFUSEALPHA',13:'BLENDTEXTUREALPHA',14:'BLENDFACTORALPHA',15:'BLENDTEXTUREALPHAPM',16:'BLENDCURRENTALPHA'}
TA = {0:'DIFFUSE',1:'CURRENT',2:'TEXTURE',3:'TFACTOR',4:'SPECULAR'}

im = Img(exe)
code = bytes(im.img[im.TS - im.base: im.TE - im.base])
sites = []
win = collections.deque(maxlen=40)
n = 0
for ins in im.md.disasm(code, im.TS):
    n += 1
    win.append(ins)
    if ins.id != X86_INS_CALL: continue
    op = ins.operands[0]
    if op.type != X86_OP_MEM or op.mem.base == 0 or op.mem.index != 0 or op.mem.disp not in DEV: continue
    disp = op.mem.disp; name, nargs = DEV[disp]
    callreg = op.mem.base
    args_ = []; vt = None
    lst = list(win)[:-1]
    for prev in reversed(lst):
        if prev.id in (X86_INS_RET, X86_INS_JMP, X86_INS_CALL) or prev.mnemonic.startswith('j'): break
        if prev.id == X86_INS_PUSH:
            o = prev.operands[0]
            if o.type == X86_OP_IMM: args_.append(('imm', o.imm))
            elif o.type == X86_OP_REG: args_.append(('reg', prev.reg_name(o.reg)))
            else: args_.append(('mem', prev.op_str))
            if len(args_) >= nargs + 1: break
        if prev.id == X86_INS_MOV and prev.operands[0].type == X86_OP_REG and prev.operands[0].reg == callreg and prev.operands[1].type == X86_OP_MEM:
            m = prev.operands[1].mem
            vt = (prev.reg_name(m.base) if m.base else None, m.disp)
    # args is in walk-back order: [this, arg1, arg2, ...] (stdcall pushes right-to-left, 'this' pushed last)
    sites.append({'va': ins.address, 'slot': disp, 'name': name, 'nargs': nargs, 'args': args_, 'vt': vt})
print('instructions', n, 'sites', len(sites))

def fmt(a):
    k, v = a
    if k == 'imm':
        u = v & 0xffffffff
        s = '0x%x' % u
        if 0x3C000000 <= u <= 0x47000000 or 0xBC000000 <= u <= 0xC7000000:
            s += '(f=%g)' % struct.unpack('<f', struct.pack('<I', u))[0]
        return s
    return '%s:%s' % (k, v)

def real_args(s):
    a = s['args']
    return a[1:1 + s['nargs']]

def pretty(s):
    nm = s['name']; real = real_args(s)
    txt = ', '.join(fmt(x) for x in real)
    note = ''
    if nm == 'SetRenderState' and len(real) == 2 and real[0][0] == 'imm':
        st = real[0][1]; note = RS.get(st, 'NOT-RS(%d)' % st)
        if st in RS and real[1][0] == 'imm': note += '=%d' % real[1][1]
    if nm == 'SetTextureStageState' and len(real) == 3 and real[1][0] == 'imm':
        t = real[1][1]
        if t in TSS:
            note = 'stage %s %s' % (fmt(real[0]), TSS[t])
            if real[2][0] == 'imm':
                v = real[2][1]
                if t in (1, 4): note += '=%s' % TOP.get(v, v)
                elif t in (2, 3, 5, 6): note += '=%s' % TA.get(v & 0xf, v) + ('|COMPLEMENT' if v & 0x10 else '') + ('|ALPHAREPLICATE' if v & 0x20 else '')
                else: note += '=%d' % v
        else: note = 'NOT-TSS(%d)' % t
    return '%08x %-26s (%s) vt=%s %s' % (s['va'], nm, txt, s['vt'], note)

with open(OUT + 'd3d7scan.txt', 'w') as f:
    f.write('instructions %d sites %d\n' % (n, len(sites)))
    for slot in sorted(DEV):
        ss = [s for s in sites if s['slot'] == slot]
        if not ss: continue
        f.write('\n## slot 0x%02x %s : %d sites\n' % (slot, DEV[slot][0], len(ss)))
        for s in ss: f.write(pretty(s) + '\n')
json.dump(sites, open(OUT + 'd3d7scan.json', 'w'))
rs = collections.Counter(); tss = collections.Counter()
for s in sites:
    real = real_args(s)
    if s['name'] == 'SetRenderState' and len(real) == 2 and real[0][0] == 'imm' and real[0][1] in RS:
        rs[(RS[real[0][1]], fmt(real[1]))] += 1
    if s['name'] == 'SetTextureStageState' and len(real) == 3 and real[1][0] == 'imm' and real[1][1] in TSS:
        tss[(fmt(real[0]), TSS[real[1][1]], fmt(real[2]))] += 1
with open(OUT + 'd3d7scan_summary.txt', 'w') as f:
    f.write('# SetRenderState (state, value) -> number of call sites\n')
    for k, v in sorted(rs.items()): f.write('%-24s %-16s %d\n' % (k[0], k[1], v))
    f.write('\n# SetTextureStageState (stage, type, value) -> number of call sites\n')
    for k, v in sorted(tss.items()): f.write('%-6s %-24s %-16s %d\n' % (k[0], k[1], k[2], v))
print(open(OUT + 'd3d7scan_summary.txt').read())
