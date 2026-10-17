import sys, math, datetime, collections, re
import pefile

def entropy(data):
    if not data: return 0.0
    c = collections.Counter(data); n = len(data)
    return -sum((v/n)*math.log2(v/n) for v in c.values())

for path in sys.argv[1:]:
    pe = pefile.PE(path, fast_load=False)
    fh, oh = pe.FILE_HEADER, pe.OPTIONAL_HEADER
    print("="*100); print(path)
    print(f"  machine=0x{fh.Machine:x} timestamp={datetime.datetime.fromtimestamp(fh.TimeDateStamp, datetime.timezone.utc)} UTC "
          f"linker={oh.MajorLinkerVersion}.{oh.MinorLinkerVersion} subsystem={oh.Subsystem} "
          f"imagebase=0x{oh.ImageBase:x} entry=0x{oh.AddressOfEntryPoint:x} osver={oh.MajorOperatingSystemVersion}.{oh.MinorOperatingSystemVersion} "
          f"dllchar=0x{oh.DllCharacteristics:x} chars=0x{fh.Characteristics:x}")
    print("  sections:")
    for s in pe.sections:
        d = s.get_data()
        print(f"    {s.Name.decode(errors='replace').strip(chr(0)):8s} va=0x{s.VirtualAddress:08x} vsz=0x{s.Misc_VirtualSize:08x} raw=0x{s.SizeOfRawData:08x} entropy={entropy(d):.2f} flags=0x{s.Characteristics:08x}")
    # debug dir / pdb
    if hasattr(pe, 'DIRECTORY_ENTRY_DEBUG'):
        for dbg in pe.DIRECTORY_ENTRY_DEBUG:
            e = dbg.entry
            print("  debug:", dbg.struct.Type, getattr(e, 'PdbFileName', b'')[:200] if e else '')
    # imports
    if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
        print("  imports:")
        for imp in pe.DIRECTORY_ENTRY_IMPORT:
            names = [ (i.name.decode() if i.name else f"ord{i.ordinal}") for i in imp.imports ]
            print(f"    {imp.dll.decode():20s} ({len(names)}): " + ", ".join(names[:400]))
    if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
        exps = [ (e.name.decode() if e.name else f"ord{e.ordinal}") for e in pe.DIRECTORY_ENTRY_EXPORT.symbols ]
        print(f"  exports ({len(exps)}): " + ", ".join(exps[:300]))
    # version info
    if hasattr(pe, 'FileInfo'):
        for fi in pe.FileInfo:
            for f in fi:
                if hasattr(f, 'StringTable'):
                    for st in f.StringTable:
                        for k, v in st.entries.items():
                            print(f"  verinfo {k.decode(errors='replace')}={v.decode(errors='replace')}")
    # RTTI class names
    data = pe.__data__
    rtti = sorted(set(m.group(0).decode(errors='replace') for m in re.finditer(rb'\.\?A[VU][A-Za-z0-9_@?$]{1,200}@@', data)))
    print(f"  RTTI type descriptors: {len(rtti)}")
    print("    " + " ".join(rtti[:600]))
