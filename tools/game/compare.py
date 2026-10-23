#!/usr/bin/env python3
"""Compares the creatures of opensacred-game and of the original about the same cells (#574, docs/03 §3.7, §3.8), or
their fight values type by type (`stats`, #587).

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

With `stats`, each type given is made at each level in both games beside their heroes (`spawn TYPE --level L
--peaceful --immortal`) and their `stats` compared: the most health, `A`, `D`, each kind's least and most damage, the
resistances and the attack speed. The original's driver prints its raw values with their multipliers, taken through
them as its attack record does (docs/re/fight.md): `A` and `D` times theirs, truncated when not 1; each damage times
the items' multiplier plus the attributes'; each resistance times its own. The creatures stay where they are made.

  python -I tools/game/compare.py stats --types 76,307,516 --levels 1,10,40
  python -I tools/game/compare.py stats --types-file <scratch>/types.txt --levels 5,25

Expected output: a block per cell, `= 3383,2520: 11 in the game, 11 in the original, 9 paired`, then a line per
difference (`level 4 / 1  type 297 at 3378,2517 / 3378,2517`, the game's value first) and per creature without a
pair (`only in the original: type 288 level 1 at 3370,2524`); with `stats`, a line per creature whose values differ
by more than the game's one decimal or two thousandths (`type 76 level 5: D 31 / 32`, the game's first) and a count at the end.
Dependencies: the standard library.
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


NUMBER = re.compile(r'-?\d+(?:\.\d+)?')


def numbers(text):
    return [float(n) for n in NUMBER.findall(text)]


def game_stats(lines):
    """The game's `stats`: its values multiplied out already."""
    values = {'most': float(re.search(r' health \d+/(\d+)', lines[0])[1])}
    for line in lines[1:]:
        if line.startswith('A '):
            values['A'], values['D'] = numbers(line)
        elif line.startswith('damage'):
            least, most = line.split(' most ')
            values['least'], values['most damage'] = numbers(least), numbers(most)
        elif line.startswith('resistances'):
            values['resistances'] = numbers(line)
        elif line.startswith('attack speed'):
            values['speed'] = numbers(line)[0]
    return values


def original_stats(lines):
    """The original's `stats`: its raw values through their multipliers, as its attack record takes them."""
    values = {'most': float(re.search(r' health \d+/(\d+)', lines[0])[1])}
    for line in lines[1:]:
        if line.startswith('A '):
            a, a_times, d, d_times = numbers(line)
            values['A'] = a if a_times == 1 else float(int(a * a_times))
            values['D'] = d if d_times == 1 else float(int(d * d_times))
        elif line.startswith('damage'):
            n = numbers(line)
            factors = [items + attributes for items, attributes in zip(n[8:12], n[12:16])]
            values['least'] = [v * f for v, f in zip(n[0:4], factors)]
            values['most damage'] = [v * f for v, f in zip(n[4:8], factors)]
        elif line.startswith('resistances'):
            n = numbers(line)
            values['resistances'] = [v * f for v, f in zip(n[0:4], n[4:8])]
        elif line.startswith('attack speed'):
            values['speed'] = numbers(line)[0]
    return values


def differences(ours, theirs):
    lines = []
    for key in ('most', 'A', 'D', 'least', 'most damage', 'resistances', 'speed'):
        a, b = ours.get(key), theirs.get(key)
        if isinstance(a, list) and isinstance(b, list):
            if any(abs(p - q) > max(0.1, abs(q) / 500) for p, q in zip(a, b)):
                lines.append(f"{key} {', '.join(f'{v:.1f}' for v in a)} / {', '.join(f'{v:.2f}' for v in b)}")
        elif a is not None and b is not None and abs(a - b) > max(0.1, abs(b) / 500):
            lines.append(f'{key} {a:g} / {b:g}')
    return lines


def stats_sweep(game, original, args):
    types = [int(t) for t in args.types.split(',')] if args.types else []
    if args.types_file:
        with open(args.types_file, encoding='utf-8') as f:
            types += [int(line.split('#')[0]) for line in f if line.split('#')[0].strip()]
    levels = [int(level) for level in args.levels.split(',')]
    differing = 0
    for creature in types:
        for level in levels:
            line = f'spawn {creature} --at {args.at} --level {level} --peaceful --immortal'
            try:
                ours = game.exec(line)[-1].rsplit(': ', 1)[-1].split(',')[0].strip()
                theirs = re.match(r'made (\d+)', original.exec(line)[-1])[1]
                found = differences(game_stats(game.exec(f'stats {ours}')),
                                    original_stats(original.exec(f'stats {theirs}')))
            except (RuntimeError, OSError, TypeError, ValueError, IndexError) as e:
                print(f'type {creature} level {level}: {e}', flush=True)
                continue
            if found:
                differing += 1
                print(f'type {creature} level {level}: ' + '; '.join(found), flush=True)
    print(f'{differing} of {len(types) * len(levels)} differ')


def main():
    if sys.argv[1:2] == ['stats']:
        parser = argparse.ArgumentParser(prog='compare.py stats', description='the fight values, type by type')
        parser.add_argument('stats')
        parser.add_argument('--types', help='creature types, comma-separated')
        parser.add_argument('--types-file', help='a file of types, one a line, # comments')
        parser.add_argument('--levels', default='1', help='levels, comma-separated (1)')
        parser.add_argument('--at', default='+3,+3', help="where to make them, from the hero's cell (+3,+3)")
        parser.add_argument('--game-port', type=int, default=15702)
        parser.add_argument('--original-port', type=int, default=15703)
        args = parser.parse_args()
        stats_sweep(Endpoint('the game', args.game_port), Endpoint('the original', args.original_port), args)
        return
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
