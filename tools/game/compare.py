#!/usr/bin/env python3
"""Compares the creatures of opensacred-game and of the original about the same cells (#574, docs/03 §3.7, §3.8).

Purpose: a sweep of the levels, the health and the hostility of the creatures that both games make. For each cell
given, the hero of each game goes onto it (`cell X,Y`), optionally at a level and an hour, and after the sectors have
come in each game lists its creatures (`creatures RADIUS`). The creatures are paired by type, each with the nearest
of the other game's within --pair cells, and the report prints the pairs whose level, most health, hostility or rank
differ, then the creatures of one game that found no pair, by type, with their levels. The game's lines and the
original's driver's carry the same fields (`ID type T cell X,Y [floor F] health H/M level L hostile|peaceful
[champion|boss]`). Both games must run with their remote control: the game with `--remote` (127.0.0.1:15702) and the
original under `tools/original/host.py` (127.0.0.1:15703), each in a game. Nothing is written: the report goes to the
standard output, and it names creatures by their types and cells alone, so it may go into an issue (docs/08 §3).

Example
  python -I tools/game/compare.py --level 4 --time 12:00 3383,2520 3250,2604 3306,2519
  python -I tools/game/compare.py --radius 25 --settle 6 --cells-file <scratch>/cells.txt

Expected output: a block per cell, `= 3383,2520: 11 in the game, 11 in the original, 9 paired`, then a line per
difference (`level 4 / 1  type 297 at 3378,2517 / 3378,2517`, the game's value first) and per creature without a
pair (`only in the original: type 288 level 1 at 3370,2524`). Dependencies: the standard library.
"""
import argparse, json, re, sys, time, urllib.request

LINE = re.compile(r'^(\S+) type (\d+) cell (-?\d+),(-?\d+)(?: floor (\d+))?.*? health (\d+)/(\d+) level (\d+)'
                  r'(?: (hostile|peaceful))?(?: (champion|boss))?')
CELL = re.compile(r'^\s*(\d+)\s*,\s*(\d+)\s*$')


class Endpoint:
    def __init__(self, name, port):
        self.name, self.port = name, port

    def call(self, method, params=None):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or {}}).encode()
        request = urllib.request.Request(f'http://127.0.0.1:{self.port}', data=body,
                                         headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=60) as reply:
            answer = json.loads(reply.read())
        if 'error' in answer:
            raise RuntimeError(f"{self.name}: {answer['error'].get('message')}")
        return answer['result']

    def exec(self, line):
        return self.call('opensacred/exec', {'line': line}).get('output', [])

    def ready(self, timeout):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if self.call('opensacred/state').get('ready'):
                return
            time.sleep(0.25)
        raise RuntimeError(f'{self.name}: not ready in {timeout} s')

    def creatures(self, radius):
        found = []
        for line in self.exec(f'creatures {radius}')[1:]:
            match = LINE.match(line)
            if match:
                found.append({'type': int(match[2]), 'cell': (int(match[3]), int(match[4])),
                              'floor': int(match[5]) if match[5] else None, 'health': int(match[6]),
                              'most': int(match[7]), 'level': int(match[8]), 'side': match[9], 'rank': match[10]})
        return found


def pair(ours, theirs, within):
    """Greedy pairs of one type: the nearest pair first, at most `within` cells apart (the larger axis)."""
    candidates = sorted(
        (max(abs(a['cell'][0] - b['cell'][0]), abs(a['cell'][1] - b['cell'][1])), i, j)
        for i, a in enumerate(ours) for j, b in enumerate(theirs) if a['type'] == b['type'])
    used_a, used_b, pairs = set(), set(), []
    for distance, i, j in candidates:
        if distance <= within and i not in used_a and j not in used_b:
            used_a.add(i)
            used_b.add(j)
            pairs.append((ours[i], theirs[j]))
    return (pairs, [a for i, a in enumerate(ours) if i not in used_a],
            [b for j, b in enumerate(theirs) if j not in used_b])


def at(c):
    return f"{c['cell'][0]},{c['cell'][1]}" + (f" floor {c['floor']}" if c['floor'] else '')


def compare(game, original, cell, args):
    for endpoint in (game, original):
        endpoint.exec(f'cell {cell[0]},{cell[1]}')
    time.sleep(args.settle)
    game.ready(args.timeout)
    ours, theirs = game.creatures(args.radius), original.creatures(args.radius)
    pairs, only_ours, only_theirs = pair(ours, theirs, args.pair)
    print(f'= {cell[0]},{cell[1]}: {len(ours)} in the game, {len(theirs)} in the original, {len(pairs)} paired')
    for a, b in pairs:
        for field in ('level', 'most', 'side', 'rank'):
            if b[field] is not None and a[field] != b[field]:
                print(f"  {field} {a[field]} / {b[field]}  type {a['type']} at {at(a)} / {at(b)}")
    for name, rest in (('the game', only_ours), ('the original', only_theirs)):
        for c in sorted(rest, key=lambda c: (c['type'], c['cell'])):
            print(f"  only in {name}: type {c['type']} level {c['level']} health {c['most']} at {at(c)}"
                  + (f" {c['side']}" if c['side'] else '') + (f" {c['rank']}" if c['rank'] else ''))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('cells', nargs='*', help='X,Y global cells')
    parser.add_argument('--cells-file', help='a file of cells, X,Y a line, # comments')
    parser.add_argument('--radius', type=int, default=15, help='the creatures within this many cells (15)')
    parser.add_argument('--pair', type=int, default=3, help='pair creatures of a type at most this far apart (3)')
    parser.add_argument('--settle', type=float, default=4, help='seconds for the sectors to come in (4)')
    parser.add_argument('--timeout', type=float, default=60, help='seconds to wait for the game to be ready')
    parser.add_argument('--level', type=int, help="the heroes' level first (each game's `level`)")
    parser.add_argument('--time', help='the clock first, HH:MM')
    parser.add_argument('--game-port', type=int, default=15702)
    parser.add_argument('--original-port', type=int, default=15703)
    args = parser.parse_args()
    cells = list(args.cells)
    if args.cells_file:
        with open(args.cells_file, encoding='utf-8') as f:
            cells += [line.split('#')[0].strip() for line in f if line.split('#')[0].strip()]
    parsed = []
    for text in cells:
        match = CELL.match(text)
        if not match:
            parser.error(f'a cell is X,Y, not {text!r}')
        parsed.append((int(match[1]), int(match[2])))
    if not parsed:
        parser.error('no cells given')
    game, original = Endpoint('the game', args.game_port), Endpoint('the original', args.original_port)
    for endpoint in (game, original):
        if args.level:
            endpoint.exec(f'level {args.level}')
        if args.time:
            endpoint.exec(f'time {args.time}')
    for cell in parsed:
        try:
            compare(game, original, cell, args)
        except (RuntimeError, OSError) as e:
            print(f'= {cell[0]},{cell[1]}: {e}', file=sys.stderr)


if __name__ == '__main__':
    main()
