#!/usr/bin/env python3
"""The statics' sprites with a depth per texel (#1437, research): each sprite's relief estimated by a monocular depth
model, fitted to the scene, and written as a 16-bit map beside the sprite's image in the game's assets, which the
game's `sprites = "depth"` takes (docs/decisions/drawing.md, "Sprites' depth per texel, to compare").

Purpose: the comparison #1437 asks for, made on the user's own machine from the converted sprites, never published
(PLAN.md ADR-12, docs/08 §3). The weights are downloaded from Hugging Face on the first run and kept in its cache.

  models   dav2-small: Depth Anything V2 Small (depth-anything/Depth-Anything-V2-Small-hf, Apache-2.0), the input's long
           side 518 texels in multiples of 14; dpt-large: Intel's DPT-Large (Intel/dpt-large, Apache-2.0), 384 square,
           the sprite laid on grey to a square first, as its position embeddings take a square alone. Both give a relative inverse depth, nearer greater, up to a scale and a shift.
  input    each sprite's image as the converter assembled it (<assets-src>/world/sprites/<record>.png, #838), its
           translucent texels laid over grey, scaled to the model's size and its output scaled back bicubically.
  fit      the camera is orthographic, so a sprite's relative depth is affine in the model's output over its opaque
           texels (alpha over 64, the sprites' alpha test): the output's 2nd and 98th percentiles become 0 and 1, and
           that span becomes the relief R = scale * 0.6847 * W world units, W the opaque texels' width in pixels at
           zoom 1, as deep as a diamond of the ground W pixels across reaches back (cos 30 deg / 1.2649 pixels a
           unit); the median texel lies on the sprite's quad, at its cell's depth as in the original's order, the
           others in front of it or behind by their share of R.
  height   the same depth as each texel's height over the ground that its line of sight meets at its sprite's
           base (the camera looks down at 30 deg, so a point h units high lies 2 h nearer than that ground point,
           which lies 1.3693 units nearer for each pixel lower on screen): the texels' heights are shifted so
           that their lowest percent lie on the ground, as a sprite stands on it, the few under it raised to it.
  output   <assets>/world/sprites/<record>.depth.png: 16-bit grey over the sprite's rectangle, a texel's depth in
           front of its quad in eighths of a world unit plus 32,768 (32,768 the quad itself, as the transparent texels
           have it); <record>.height.png beside it, a texel's height in eighths of a unit (0 for the transparent
           texels); the game reads the loose files of assets/ over its archives (docs/02 §12.2), the first for
           `sprites = "relief"`, the second for `sprites = "height"`.

Example
  uv run --group depth python -I tools/analysis/world/sprite_depth.py --sprites assets-src/world/sprites --assets assets
  uv run --group depth python -I tools/analysis/world/sprite_depth.py --sprites assets-src/world/sprites --assets assets --model dpt-large --ids 401,2205 --sheet <scratch>/sheet.png

Output: a line per hundred sprites, then the time per sprite and per megatexel on the machine's GPU (or CPU) and the
time for every sprite of the directory extrapolated. --sheet writes each sprite beside its map (the nearest white) for
judging locally; the maps and the sheet are converted assets, never attached or committed.
Dependencies: numpy, Pillow, and the group `depth` of pyproject.toml (PyTorch with CUDA on Windows, transformers).
"""
import argparse
import os
import sys
import time

import numpy as np
from PIL import Image

MODELS = {
    'dav2-small': ('depth-anything/Depth-Anything-V2-Small-hf', 518, 14, False),
    'dpt-large': ('Intel/dpt-large', 384, 32, True),
}
# The sprites' alpha test: a texel of alpha 64 or less is dropped (docs/re/drawing.md).
CUTOFF = 64
# World units nearer per pixel lower on screen of the ground: sqrt 3 over 1.2649 pixels a unit.
NEARER_PER_PIXEL = 1.7320508 / 1.2649111
# World units of depth per unit of height along a line of sight: 1 / sin 30 deg.
DEPTH_PER_HEIGHT = 2.0
# World units back per pixel of a ground diamond's width: cos 30 deg over 1.2649 pixels a unit (iso::PIXELS_PER_UNIT).
DEEP_PER_PIXEL = 0.8660254 / 1.2649111
# The map's units: eighths of a world unit, about 32,768.
STEPS_PER_UNIT = 8.0
ZERO = 32768


def relative_depth(model, processor, device, rgba, long_side, multiple, square):
    """The model's relative inverse depth (nearer greater) over the sprite's texels."""
    import torch

    height, width = rgba.shape[:2]
    alpha = rgba[..., 3:4].astype(np.float32) / 255.0
    rgb = (rgba[..., :3] * alpha + 128.0 * (1.0 - alpha)).astype(np.uint8)
    if square:
        side = max(height, width)
        laid = np.full((side, side, 3), 128, np.uint8)
        laid[:height, :width] = rgb
        depth = relative_depth(model, processor, device, np.dstack([laid, np.full((side, side), 255, np.uint8)]),
                               long_side, multiple, False)
        return depth[:height, :width]
    scale = long_side / max(height, width)
    size = (max(multiple, round(width * scale / multiple) * multiple),
            max(multiple, round(height * scale / multiple) * multiple))
    image = Image.fromarray(rgb).resize(size, Image.BICUBIC)
    inputs = processor(images=image, return_tensors='pt', do_resize=False).to(device)
    with torch.no_grad():
        predicted = model(**inputs).predicted_depth
    depth = torch.nn.functional.interpolate(predicted[None], size=(height, width), mode='bicubic',
                                            align_corners=False)[0, 0]
    return depth.float().cpu().numpy()


def fitted(depth, rgba, scale):
    """The texels' depth in front of the sprite's quad in world units, 0 for the dropped ones (see `fit` above)."""
    opaque = rgba[..., 3] > CUTOFF
    out = np.zeros(depth.shape, np.float32)
    if opaque.sum() < 2:
        return out
    values = depth[opaque]
    low, high = np.percentile(values, 2), np.percentile(values, 98)
    if high - low <= 1e-6:
        return out
    share = (depth - low) / (high - low)
    columns = np.nonzero(opaque.any(axis=0))[0]
    relief = scale * DEEP_PER_PIXEL * (columns[-1] - columns[0] + 1)
    out[opaque] = relief * (share[opaque] - np.median(share[opaque]))
    return out


def heights(relief, rgba):
    """The texels' heights over the ground at their sprite's base in world units, 0 for the dropped ones (see
    `height` above)."""
    opaque = rgba[..., 3] > CUTOFF
    out = np.zeros(relief.shape, np.float32)
    if opaque.sum() < 2:
        return out
    rows = np.arange(relief.shape[0], dtype=np.float32)[:, None] * np.ones_like(relief)
    lifted = relief - NEARER_PER_PIXEL * rows
    base = np.percentile(lifted[opaque], 1)
    out[opaque] = np.maximum(lifted[opaque] - base, 0.0) / DEPTH_PER_HEIGHT
    return out


def encoded(relief):
    """The 16-bit map of `relief`, in eighths of a unit about 32,768."""
    return np.clip(np.round(relief * STEPS_PER_UNIT) + ZERO, 0, 65535).astype(np.uint16)


def main():
    ap = argparse.ArgumentParser(description="The statics' sprites' depth per texel (#1437).")
    ap.add_argument('--sprites', required=True, help='the masters of the sprites, <assets-src>/world/sprites')
    ap.add_argument('--assets', required=True, help="the game's assets, whose world/sprites/ takes the maps")
    ap.add_argument('--model', choices=sorted(MODELS), default='dav2-small')
    ap.add_argument('--ids', help='the records to make, comma-separated (default every image of --sprites)')
    ap.add_argument('--scale', type=float, default=1.0, help='the relief as a share of the footprint deep as wide')
    ap.add_argument('--sheet', help='write each sprite beside its map to this PNG (with --ids)')
    args = ap.parse_args()

    import torch
    from transformers import AutoImageProcessor, AutoModelForDepthEstimation

    name, long_side, multiple, square = MODELS[args.model]
    device = 'cuda' if torch.cuda.is_available() else 'cpu'
    processor = AutoImageProcessor.from_pretrained(name)
    model = AutoModelForDepthEstimation.from_pretrained(name).to(device).eval()
    print(f'{args.model} ({name}) on {torch.cuda.get_device_name(0) if device == "cuda" else "the CPU"}')

    every = sorted(int(f[:-4]) for f in os.listdir(args.sprites) if f.endswith('.png') and f[:-4].isdigit())
    ids = [int(v) for v in args.ids.split(',')] if args.ids else every
    out_dir = os.path.join(args.assets, 'world', 'sprites')
    os.makedirs(out_dir, exist_ok=True)

    sheet = []
    texels = 0
    took = 0.0
    warm = None
    for n, record in enumerate(ids, 1):
        rgba = np.asarray(Image.open(os.path.join(args.sprites, f'{record:05}.png')).convert('RGBA'))
        start = time.perf_counter()
        depth = relative_depth(model, processor, device, rgba, long_side, multiple, square)
        relief = fitted(depth, rgba, args.scale)
        Image.fromarray(encoded(relief)).save(os.path.join(out_dir, f'{record:05}.depth.png'))
        height = np.clip(np.round(heights(relief, rgba) * STEPS_PER_UNIT), 0, 65535).astype(np.uint16)
        Image.fromarray(height).save(os.path.join(out_dir, f'{record:05}.height.png'))
        spent = time.perf_counter() - start
        if warm is None:
            # The first run compiles the model's kernels: left out of the measure.
            warm = spent
        else:
            took += spent
            texels += rgba.shape[0] * rgba.shape[1]
        if args.sheet:
            opaque = rgba[..., 3] > CUTOFF
            span = max(float(np.abs(relief).max()), 1e-6)
            grey = np.where(opaque, np.clip(127.5 + 127.5 * relief / span, 0, 255), 0).astype(np.uint8)
            shown = np.stack([grey] * 3 + [np.full_like(grey, 255)], -1)
            sheet.append(np.concatenate([rgba, shown], 1))
        if n % 100 == 0:
            print(f'{n} of {len(ids)}', flush=True)

    measured = max(len(ids) - 1, 1)
    per_sprite = took / measured
    per_megatexel = took / max(texels / 1e6, 1e-9)
    print(f'{len(ids)} sprites, {texels / 1e6 + 0.0:.1f} Mtexels measured: {per_sprite * 1000:.1f} ms a sprite, '
          f'{per_megatexel:.2f} s a megatexel (the first, {warm * 1000:.0f} ms, left out)')
    if len(ids) < len(every):
        print(f'every sprite of the directory, {len(every)}: about {per_sprite * len(every) / 60:.1f} min')

    if args.sheet and sheet:
        width = max(t.shape[1] for t in sheet)
        canvas = np.zeros((sum(t.shape[0] for t in sheet), width, 4), np.uint8)
        y = 0
        for tile in sheet:
            canvas[y:y + tile.shape[0], :tile.shape[1]] = tile
            y += tile.shape[0]
        back = Image.new('RGBA', (width, canvas.shape[0]), (60, 60, 90, 255))
        back.alpha_composite(Image.fromarray(canvas))
        back.convert('RGB').save(args.sheet)


if __name__ == '__main__':
    sys.exit(main())
