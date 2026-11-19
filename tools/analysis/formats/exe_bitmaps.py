#!/usr/bin/env python3
"""The bitmaps of Sacred.exe's resources (RT_BITMAP) and their decoded pixels: the reference for opensacred-formats' `pe::Image::resource` and `bmp::decode_dib` (#2891).

What the script does
  Lists each bitmap resource of the executable by its number: its language, its size in bytes, the DIB header's
  width, height, bits per pixel and compression, and the CRC-32 of its pixels decoded to RGBA rows from the top,
  as Pillow decodes the DIB once a BITMAPFILEHEADER is put before it. With --out, writes each as <number>.png there.

Examples
  uv run python -I tools/analysis/formats/exe_bitmaps.py
  uv run python -I tools/analysis/formats/exe_bitmaps.py --game-dir ./OriginalGame --json
  uv run python -I tools/analysis/formats/exe_bitmaps.py --out <scratch>/bitmaps

Expected output: a line per bitmap resource, as "124 lang 1031 bytes 787486 1024x768 8-bit compression 0
rgba <crc32>"; with --json one JSON object per line. Prints no picture's content. Dependencies: pefile and
Pillow (pyproject.toml).
"""
import argparse
import io
import json
import os
import struct
import sys
import zlib

import pefile
from PIL import Image

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
from sacred_pe import DEFAULT_GAME_DIR

RT_BITMAP = 2


def bitmaps(pe):
    """Each bitmap resource: (its number, its first language, its bytes)."""
    for kind in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if kind.id != RT_BITMAP:
            continue
        for entry in kind.directory.entries:
            language = entry.directory.entries[0]
            data = language.data.struct
            raw = pe.get_data(data.OffsetToData, data.Size)
            yield entry.id, language.id, raw


def decode(dib):
    """The DIB as a file Pillow reads: a BITMAPFILEHEADER whose pixels follow the header and the palette."""
    header_size, _width, _height, _planes, bits, _compression, _image, _xppm, _yppm, used = struct.unpack_from('<IiiHHIIiiI', dib)
    colours = used or (1 << bits if bits <= 8 else 0)
    pixels_at = 14 + header_size + 4 * colours
    file = b'BM' + struct.pack('<IHHI', 14 + len(dib), 0, 0, pixels_at) + dib
    return Image.open(io.BytesIO(file)).convert('RGBA')


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    parser.add_argument('--json', action='store_true')
    parser.add_argument('--out')
    args = parser.parse_args()
    pe = pefile.PE(os.path.join(args.game_dir, 'Sacred.exe'), fast_load=True)
    pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_RESOURCE']])
    if args.out:
        os.makedirs(args.out, exist_ok=True)
    for number, language, raw in bitmaps(pe):
        _size, width, height, _planes, bits, compression = struct.unpack_from('<IiiHHI', raw)
        image = decode(raw)
        rgba = f"{zlib.crc32(image.tobytes()):08x}"
        if args.out:
            image.save(os.path.join(args.out, f'{number}.png'))
        if args.json:
            print(json.dumps({'number': number, 'language': language, 'bytes': len(raw), 'width': width,
                              'height': height, 'bits': bits, 'compression': compression, 'rgba': rgba}))
        else:
            print(f'{number} lang {language} bytes {len(raw)} {width}x{height} {bits}-bit compression {compression} rgba {rgba}')


if __name__ == '__main__':
    main()
