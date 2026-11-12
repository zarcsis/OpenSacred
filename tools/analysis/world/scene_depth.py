#!/usr/bin/env python3
"""A depth model's view of the whole scene (#2621, research): each frame of `scene_shots.py` through a monocular depth
model, its output fitted to the ground the frame shows, and each pixel's height over the ground that its ray meets.

Purpose: the experiment of #1437's comment on the scene's depth, made on the user's own machine from the game's
frames, never published (PLAN.md ADR-12, docs/08 §3). The weights are downloaded from Hugging Face on the first run.

  ground   <name>-a.png is the frame as drawn, <name>-b.png with the statics' sprites hidden: the pixels equal in both
           (by more than 24 in no channel), grown away from the sprites by 3 pixels and outside the bar and the
           portrait, are the ground, the models standing on it left out by the fit's rounds.
  models   Depth Anything V2 Small (Apache-2.0) at a long side of 518 (its training size), 1022 and 1918; DPT-Large
           (Apache-2.0) laid on a square at 384 and 768. At 518 the frame's context holds; the larger ones and
           DPT-Large lose it.
  fit      the camera is orthographic at zoom 1: on flat ground a pixel lower on screen is 1.3693 units nearer, so the
           model's output is fitted to the ground's nearness, affinely, over the ground's pixels, the worst fifth
           left out in each of six rounds; a pixel's nearness is then its output through the fit, and its height
           over the ground its ray meets (nearness − the ground's) / 2, as a point h units high lies 2 h nearer.
  output   <name>-<variant>-height.npy and -near.npy (float32, the frame's size), and coloured PNGs of both
           (heights from −20 blue to 160 red, nearness over its 1st to 99th percentiles); --probe prints the heights
           and nearness at points `X,Y` or along a column `X,Y0,Y1,STEP`.

Example
  uv run --group depth python -I tools/analysis/world/scene_depth.py --frames <scratch> chapel pavilion
  uv run --group depth python -I tools/analysis/world/scene_depth.py --frames <scratch> chapel --variants dav2-518 \
      --probe 1322,290,540,10 --probe 700,470

Output: per frame and variant, the share of the frame that is ground and the ground's own heights' percentiles (how
flat the fit leaves it). The maps and the frames are converted assets, never attached or committed; cropped frames of
a finding alone go to its issue.
Dependencies: numpy, Pillow, and the group `depth` of pyproject.toml (PyTorch with CUDA on Windows, transformers).
"""
import argparse
import os

import numpy as np
from PIL import Image

# World units nearer per pixel lower on screen of the ground: sqrt 3 over 1.2649 pixels a unit.
NEARER_PER_PIXEL = 1.7320508 / 1.2649111
# World units of depth per unit of height along a line of sight: 1 / sin 30 deg.
DEPTH_PER_HEIGHT = 2.0
VARIANTS = {
    'dav2-518': ('depth-anything/Depth-Anything-V2-Small-hf', 518, 14, False),
    'dav2-1022': ('depth-anything/Depth-Anything-V2-Small-hf', 1022, 14, False),
    'dav2-1918': ('depth-anything/Depth-Anything-V2-Small-hf', 1918, 14, False),
    'dpt-384': ('Intel/dpt-large', 384, 32, True),
    'dpt-768': ('Intel/dpt-large', 768, 32, True),
}


def ground_mask(drawn, bare):
    """The ground's pixels: equal in both frames, away from the sprites, outside the bar and the portrait."""
    statics = np.abs(drawn.astype(np.int16) - bare.astype(np.int16)).max(axis=2) > 24
    grown = statics.copy()
    for dy in range(-3, 4):
        for dx in range(-3, 4):
            grown |= np.roll(np.roll(statics, dy, 0), dx, 1)
    ground = ~grown
    height, width = ground.shape
    ground[int(height * 0.83):, int(width * 0.36):int(width * 0.66)] = False
    ground[:int(height * 0.19), int(width * 0.92):] = False
    return ground


def predict(model, processor, device, rgb, long_side, multiple, square):
    """The model's relative inverse depth (nearer greater) over the frame."""
    import torch

    height, width = rgb.shape[:2]
    if square:
        side = max(height, width)
        laid = np.full((side, side, 3), 128, np.uint8)
        laid[:height, :width] = rgb
        return predict(model, processor, device, laid, long_side, multiple, False)[:height, :width]
    scale = long_side / max(height, width)
    size = (max(multiple, round(width * scale / multiple) * multiple),
            max(multiple, round(height * scale / multiple) * multiple))
    image = Image.fromarray(rgb).resize(size, Image.BICUBIC)
    inputs = processor(images=image, return_tensors='pt', do_resize=False).to(device)
    with torch.no_grad():
        predicted = model(**inputs).predicted_depth
    out = torch.nn.functional.interpolate(predicted[None], size=(height, width), mode='bicubic', align_corners=False)
    return out[0, 0].float().cpu().numpy()


def robust_fit(x, y, rounds=6, keep=0.8):
    """`y ≈ a·x + c` over the points, the worst fifth left out in each round."""
    chosen = np.ones(x.shape, bool)
    for _ in range(rounds):
        a, c = np.polyfit(x[chosen], y[chosen], 1)
        error = np.abs(y - (a * x + c))
        chosen = error <= np.quantile(error[chosen], keep)
    return np.polyfit(x[chosen], y[chosen], 1)


def colour(values, low, high):
    """A ramp from dark blue through green and yellow to red."""
    t = np.clip((values - low) / (high - low), 0, 1)
    stops = np.array([[20, 20, 90], [30, 110, 200], [40, 180, 90], [240, 220, 40], [220, 40, 30]], np.float32)
    position = t * (len(stops) - 1)
    index = np.minimum(position.astype(int), len(stops) - 2)
    share = (position - index)[..., None]
    return (stops[index] * (1 - share) + stops[index + 1] * share).astype(np.uint8)


def probe(frames, name, variant, specs):
    heights = np.load(os.path.join(frames, f'{name}-{variant}-height.npy'))
    nearness = np.load(os.path.join(frames, f'{name}-{variant}-near.npy'))
    for spec in specs:
        numbers = [int(n) for n in spec.split(',')]
        if len(numbers) == 2:
            x, y = numbers
            print(f'  {x},{y}: height {heights[y - 2:y + 3, x - 2:x + 3].mean():7.1f}  '
                  f'nearness {nearness[y - 2:y + 3, x - 2:x + 3].mean():7.1f}')
        else:
            x, top, bottom, step = numbers
            for y in range(top, bottom + 1, step):
                print(f'  {x},{y}: height {heights[y, x - 1:x + 2].mean():7.1f}  '
                      f'nearness {nearness[y, x - 1:x + 2].mean():7.1f}')


def main():
    ap = argparse.ArgumentParser(description="A depth model's view of the whole scene (#2621).")
    ap.add_argument('--frames', required=True, help="the directory of scene_shots.py's frames, and of the maps")
    ap.add_argument('names', nargs='+', help="the frames' names")
    ap.add_argument('--variants', default=','.join(VARIANTS), help='comma-separated, of ' + ', '.join(VARIANTS))
    ap.add_argument('--probe', action='append', default=[], help='X,Y or X,Y0,Y1,STEP, after the maps are made')
    args = ap.parse_args()
    import torch
    from transformers import AutoImageProcessor, AutoModelForDepthEstimation

    device = 'cuda' if torch.cuda.is_available() else 'cpu'
    frames = {}
    for name in args.names:
        drawn = np.asarray(Image.open(os.path.join(args.frames, f'{name}-a.png')).convert('RGB'))
        bare = np.asarray(Image.open(os.path.join(args.frames, f'{name}-b.png')).convert('RGB'))
        frames[name] = (drawn, ground_mask(drawn, bare))
        print(f'{name}: the ground {frames[name][1].mean() * 100:.1f} % of the frame')
    loaded = {}
    for variant in args.variants.split(','):
        repo, long_side, multiple, square = VARIANTS[variant]
        if repo not in loaded:
            loaded = {repo: (AutoImageProcessor.from_pretrained(repo),
                             AutoModelForDepthEstimation.from_pretrained(repo).to(device).eval())}
        processor, model = loaded[repo]
        for name, (drawn, ground) in frames.items():
            output = predict(model, processor, device, drawn, long_side, multiple, square)
            height, width = output.shape
            rows = np.arange(height, dtype=np.float32)[:, None] * np.ones((1, width), np.float32)
            ground_near = NEARER_PER_PIXEL * rows
            ys, xs = np.nonzero(ground)
            step = max(1, len(ys) // 200_000)
            ys, xs = ys[::step], xs[::step]
            a, c = robust_fit(ground_near[ys, xs], output[ys, xs])
            near = (output - c) / a
            heights = (near - ground_near) / DEPTH_PER_HEIGHT
            stem = os.path.join(args.frames, f'{name}-{variant}')
            np.save(f'{stem}-height.npy', heights.astype(np.float32))
            np.save(f'{stem}-near.npy', near.astype(np.float32))
            Image.fromarray(colour(heights, -20, 160)).save(f'{stem}-height.png')
            Image.fromarray(colour(near, *np.percentile(near, [1, 99]))).save(f'{stem}-near.png')
            own = heights[ys, xs]
            print(f'{name} {variant}: the ground\'s heights p10/p50/p90 {np.percentile(own, 10):.1f} / '
                  f'{np.percentile(own, 50):.1f} / {np.percentile(own, 90):.1f} units')
            if args.probe:
                probe(args.frames, name, variant, args.probe)


if __name__ == '__main__':
    main()
