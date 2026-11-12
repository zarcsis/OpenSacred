#!/usr/bin/env python3
"""Two paused frames of a place for the scene's depth (#2621, research): as the game draws it, and with every statics'
sprite hidden, so that the pixels equal in both are the ground (and the models standing on it).

Purpose: the input of `scene_depth.py`, made on the user's own machine from a game started with `--remote` (the skill
`game`), never published (PLAN.md ADR-12, docs/08 §3).

  frames   the game paused at 12:00 with its overlay off; `<name>-a.png` as drawn, `<name>-b.png` with the meshes of
           `statics::SpriteMaterial` hidden over Bevy's remote protocol (`world.insert_components`, `Visibility`),
           `<name>-c.png` after their visibility is put back, and `<name>-state.json` the game's state.
  zoom     the game's own; `scene_depth.py` takes zoom 1, so leave it there.

Example
  uv run python -I tools/analysis/world/scene_shots.py --out <scratch> --name chapel
  uv run python -I tools/analysis/world/scene_shots.py --out <scratch> --name pavilion --cell 3223,2546

Dependencies: none beyond `tools/game/remote.py`.
"""
import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'game'))
import remote  # noqa: E402

SPRITES = 'bevy_pbr::mesh_material::MeshMaterial3d<opensacred_game::statics::SpriteMaterial>'
VISIBILITY = 'bevy_camera::visibility::Visibility'


def main():
    ap = argparse.ArgumentParser(description="A place's frames with and without the statics' sprites (#2621).")
    ap.add_argument('--out', required=True, help='the directory for the frames')
    ap.add_argument('--name', required=True, help="the frames' name")
    ap.add_argument('--cell', help='X,Y: put the hero there first (the console\'s `cell`)')
    ap.add_argument('--port', type=int, help="the game's port (default this session's game)")
    args = ap.parse_args()
    port = args.port or remote.game_port()[0]

    def run(line):
        return remote.call(port, 'opensacred/exec', {'line': line})['output']

    def ready(timeout=180):
        end = time.time() + timeout
        while time.time() < end:
            state = remote.call(port, 'opensacred/state')
            if state.get('state') == 'game' and state.get('ready'):
                return
            time.sleep(0.25)
        raise SystemExit('error: the game was not ready')

    if args.cell:
        run('pause off')
        print(run(f'cell {args.cell}'))
        ready()
        time.sleep(4)
    run('overlay off')
    run('time 12:00')
    run('pause on')
    time.sleep(1.0)
    with open(os.path.join(args.out, f'{args.name}-state.json'), 'w', encoding='utf-8', newline='') as fh:
        json.dump(remote.call(port, 'opensacred/state'), fh, indent=1)
    print(remote.screenshot(port, os.path.join(args.out, f'{args.name}-a.png'), 60))
    rows = remote.call(port, 'world.query', {'data': {'components': [VISIBILITY]}, 'filter': {'with': [SPRITES]}})
    for row in rows:
        remote.call(port, 'world.insert_components', {'entity': row['entity'], 'components': {VISIBILITY: 'Hidden'}})
    time.sleep(0.5)
    print(remote.screenshot(port, os.path.join(args.out, f'{args.name}-b.png'), 60))
    for row in rows:
        remote.call(port, 'world.insert_components',
                    {'entity': row['entity'], 'components': {VISIBILITY: row['components'][VISIBILITY]}})
    time.sleep(0.5)
    print(remote.screenshot(port, os.path.join(args.out, f'{args.name}-c.png'), 60))
    print(f'{len(rows)} batches of sprites hidden and shown again')


if __name__ == '__main__':
    main()
