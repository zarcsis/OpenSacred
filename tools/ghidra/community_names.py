#!/usr/bin/env python3
"""The community's names for the local Ghidra project (docs/03 §4.1, step 3): never committed.

Purpose: merges the named entry points of sacred-bild's `tools/data/functions_eng.tsv` (the GOG ENG
`Sacred.exe`; names only, the project is GPL-3.0) and the addresses of sacred-sdk's
`engine/addresses.h` and `re/reborn_extra_syms.json` (MIT; their constants prefixed `sdk_`) into one
file of `address<TAB>name` lines, which `ImportNamesScript.java` applies to the program. Ghidra's
own default names in the export (`FUN_`, `Unwind@`, `Catch@`, …) are left out.

Example
  git clone --depth 1 https://github.com/kirides/sacred-bild <dir>/sacred-bild
  git clone --depth 1 https://github.com/bssth/sacred-sdk <dir>/sacred-sdk
  uv run python -I tools/ghidra/community_names.py --community <dir> --out <dir>/community-names.tsv

Expected output (2026-10-19): "193 sacred-bild names, 76 sacred-sdk names, 266 addresses".
Dependencies: standard library.
"""
import argparse
import json
import re

DEFAULT = re.compile(r"^(FUN_|thunk_FUN_|LAB_|DAT_|Ordinal_|switchD|caseD|Unwind@|Catch@|Catch_All@)")
IMAGE = range(0x400000, 0x1900000)


def sdk_name(constant):
    return "sdk_" + (constant[4:] if constant.startswith("FUN_") else constant)


def main():
    ap = argparse.ArgumentParser(description="Merge the community's names into address/name lines.")
    ap.add_argument("--community", required=True, help="the directory holding sacred-bild/ and sacred-sdk/")
    ap.add_argument("--out", required=True, help="the file to write, outside the repository")
    args = ap.parse_args()
    names = {}
    bild = 0
    with open(f"{args.community}/sacred-bild/tools/data/functions_eng.tsv", encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            address, name = line.rstrip("\n").split("\t")[:2]
            if not DEFAULT.match(name):
                names[int(address, 16)] = name
                bild += 1
    sdk = 0
    with open(f"{args.community}/sacred-sdk/engine/addresses.h", encoding="utf-8") as f:
        for line in f:
            m = re.search(r"constexpr\s+uintptr_t\s+(\w+)\s*=\s*0x([0-9A-Fa-f]+)", line)
            if m and int(m.group(2), 16) in IMAGE:
                names.setdefault(int(m.group(2), 16), sdk_name(m.group(1)))
                sdk += 1
    with open(f"{args.community}/sacred-sdk/re/reborn_extra_syms.json", encoding="utf-8") as f:
        extra = json.load(f)
    for constant, value in extra.items():
        if constant.startswith("_") or not isinstance(value, str) or int(value, 16) not in IMAGE:
            continue
        names.setdefault(int(value, 16), sdk_name(constant))
        sdk += 1
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("# community names: sacred-bild functions_eng.tsv, sacred-sdk addresses.h and reborn_extra_syms.json\n")
        for address in sorted(names):
            f.write(f"{address:08X}\t{names[address]}\n")
    print(f"{bild} sacred-bild names, {sdk} sacred-sdk names, {len(names)} addresses")


if __name__ == "__main__":
    main()
