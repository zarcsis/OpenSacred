#!/usr/bin/env python3
"""A conformance run of opensacred-game against the original (#600, docs/03 §3.7, §3.8).

Purpose: the checks of the game's rules against the original's, repeated with one command after a change. Both games
must run with their remote control, the game with `--remote` (127.0.0.1:15702) and the original under
`tools/original/host.py` (127.0.0.1:15703). The run starts them afresh (`new-game --class seraphim` in the game, `load
SAVE` in the original, a save of the Seraphim's new game) and goes through its suites, each check printing PASS or
FAIL:

- `experience`: a hostile goblin warrior of each of several levels made beside the hero and killed by `kill`, for a
  hero of level 1 and of level 30; the experience gained must be the same;
- `hero`: the hero's `stats` at the levels 1, 10 and 30, the original's raw values taken through their multipliers
  (`compare.py`);
- `stats`: creatures of several types at several levels, made beside the heroes (`spawn … --peaceful --immortal`),
  their `stats` compared likewise;
- `places`: the scripts' creatures of a few places whose cells and levels are fixed (the guards of 52,39 and their
  captains, the stable of 64,41), paired by type and cell: their levels, most health, hostility and rank must agree;
- `drops`: 1,000 drops of a goblin warrior of level 10 (the game's `drop`, the original's driver's `drop --id`), the
  shares of the items' categories (`items.pak +0x2E`, a derived type 2000–2999 or 6500–7999 by its root), each within
  two points or four standard deviations, whichever is wider, and the share of the derived types.

Nothing is written; the report names types, levels and categories alone, so it may go into an issue (docs/08 §3).

Example
  python -I tools/game/conformance.py
  python -I tools/game/conformance.py --suites experience,hero --save start-seraphim

Expected output: a line per check, `PASS experience: type 76 level 10, hero 1: +109` or `FAIL stats: type 32 level 10:
A 37.5 / 47`, then `N checks, M failed`; the exit code is 1 when one failed. Dependencies: the standard library.
"""
import argparse, collections, importlib.util, json, math, os, re, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
spec = importlib.util.spec_from_file_location('compare', os.path.join(HERE, 'compare.py'))
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)

# Common creatures of the Seraphim's start and of the scripts' types, with the skills 2, 8, 9, 11 and 24 among them.
TYPES = [32, 35, 38, 52, 57, 73, 76, 112, 127, 128, 131, 259, 260, 264, 272, 279, 288, 295, 297, 304, 307, 313, 319,
         513, 516, 524, 567, 577, 655, 679, 692]
# Places whose scripts' creatures stand at fixed cells and levels, visited in order from a fresh start so that no
# sector comes into the view twice: the cell to stand on, and the creatures expected there, by type and cell (each
# matched to the nearest of its type within 4 cells, as guards stroll about their posts).
PLACES = [
    ((3345, 2533), [(297, 3349, 2534), (295, 3347, 2535)]),            # hauptmann3 and a guard (tags 5, 0x13)
    ((3383, 2520), [(288, 3383, 2525), (288, 3382, 2525), (288, 3383, 2529), (288, 3384, 2529),
                    (295, 3388, 2524)]),                                # the garrison of 52,39 (tag 5)
    ((4127, 2650), [(554, 4129, 2651), (555, 4122, 2648), (551, 4120, 2639), (550, 4112, 2648),
                    (558, 4109, 2639), (552, 4101, 2643), (656, 4127, 2647)]),   # the stable of 64,41 (SetNPCState)
]
CATEGORIES = {5: 'weapon', 6: 'armour', 8: 'ring', 9: 'potion', 13: 'shield', 15: 'gold', 17: 'helmet', 18: 'shoes',
              19: 'belt', 20: 'amulet', 21: 'shoulders', 22: 'arms', 23: 'legs', 24: 'gloves', 25: 'wings',
              28: 'rune', 29: 'bridle'}


class Report:
    def __init__(self):
        self.checks, self.failed = 0, 0

    def check(self, suite, ok, text):
        self.checks += 1
        self.failed += not ok
        print(f"{'PASS' if ok else 'FAIL'} {suite}: {text}", flush=True)


def state(endpoint):
    return endpoint.call('opensacred/state')


def wait_ready(endpoint, timeout=180):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            if state(endpoint).get('ready'):
                return
        except OSError:
            pass
        time.sleep(0.5)
    raise RuntimeError(f'{endpoint.name}: not ready in {timeout} s')


def fresh(game, original, save):
    game.exec('new-game --class seraphim')
    original.exec(f'load {save}')
    time.sleep(2)
    for endpoint in (game, original):
        wait_ready(endpoint)
        endpoint.exec('god on')


def experience_suite(game, original, report):
    for hero, levels in ((None, [1, 2, 4, 7, 10, 15, 25, 40, 70]), (30, [1, 10, 20, 25, 28, 30, 33, 40])):
        if hero:
            for endpoint in (game, original):
                endpoint.exec(f'level {hero}')
        for level in levels:
            gained, heroes = [], []
            for endpoint in (game, original):
                before = state(endpoint)['experience']
                endpoint.exec(f'spawn 76 --at +2,+0 --level {level} --hostile')
                endpoint.exec('kill 4')
                after = state(endpoint)['experience']
                gained.append(after[1] - before[1])
                heroes.append(before[0])
            report.check('experience', gained[0] == gained[1] and heroes[0] == heroes[1],
                         f'type 76 level {level}, hero {heroes[0]}/{heroes[1]}: +{gained[0]} / +{gained[1]}')


def hero_suite(game, original, report):
    for level in (1, 10, 30):
        for endpoint in (game, original):
            endpoint.exec(f'level {level}')
        found = compare.differences(compare.game_stats(game.exec('stats')),
                                    compare.original_stats(original.exec('stats')))
        report.check('hero', not found, f'level {level}' + (': ' + '; '.join(found) if found else ''))


def stats_suite(game, original, report, types, levels):
    for creature in types:
        for level in levels:
            line = f'spawn {creature} --at +3,+3 --level {level} --peaceful --immortal'
            try:
                ours = game.exec(line)[-1].rsplit(': ', 1)[-1].split(',')[0].strip()
                made = original.exec(line)[-1]
                theirs = re.match(r'made (\d+) type (\d+)', made)
                if int(theirs[2]) != creature:
                    print(f'skip stats: type {creature} level {level}: the original made {theirs[2]}', flush=True)
                    continue
                found = compare.differences(compare.game_stats(game.exec(f'stats {ours}')),
                                            compare.original_stats(original.exec(f'stats {theirs[1]}')))
            except (RuntimeError, OSError, TypeError, ValueError, IndexError) as e:
                report.check('stats', False, f'type {creature} level {level}: {e}')
                continue
            report.check('stats', not found, f'type {creature} level {level}' + (': ' + '; '.join(found) if found
                                                                                    else ''))


def places_suite(game, original, report):
    def nearest(found, creature, x, y, taken):
        near = [c for c in found if c['type'] == creature and id(c) not in taken
                and max(abs(c["cell"][0] - x), abs(c["cell"][1] - y)) <= 4]
        best = min(near, key=lambda c: abs(c['cell'][0] - x) + abs(c['cell'][1] - y), default=None)
        if best is not None:
            taken.add(id(best))
        return best

    for (x, y), expected in PLACES:
        for endpoint in (game, original):
            endpoint.exec(f'cell {x},{y}')
        time.sleep(5)
        wait_ready(game)
        ours, theirs = game.creatures(30), original.creatures(30)
        taken_ours, taken_theirs = set(), set()
        for creature, cx, cy in expected:
            a, b = nearest(ours, creature, cx, cy, taken_ours), nearest(theirs, creature, cx, cy, taken_theirs)
            if a is None or b is None:
                missing = ' and '.join(name for name, c in (('the game', a), ('the original', b)) if c is None)
                report.check('places', False, f'type {creature} at {cx},{cy}: none in {missing}')
                continue
            found = [f'{key} {a[key]} / {b[key]}' for key in ('level', 'most', 'side', 'rank')
                     if b[key] is not None and a[key] != b[key]]
            report.check('places', not found, f'type {creature} at {cx},{cy}, level {b["level"]}'
                         + (': ' + '; '.join(found) if found else ''))


def item_categories(assets):
    """The category of each item type, a derived type's by its root (`Weapon.pak +0x24`)."""
    items = open(os.path.join(assets, 'tables', 'items.ron'), encoding='utf-8').read()
    category = {int(m[1]): int(m[2].split(',')[6]) >> 16
                for m in re.finditer(r'slot: (\d+),.*?words_14: \(([^)]*)\)', items, re.S)}
    weapons = open(os.path.join(assets, 'tables', 'weapons.ron'), encoding='utf-8').read()
    parent = {int(m[1]): int(m[2]) for m in re.finditer(r'\bid: (\d+),\s*parent: (\d+),', weapons)}

    def of(item):
        for _ in range(8):
            if item in category and not (2000 <= item <= 2999 or 6500 <= item <= 7999):
                return category[item]
            item = parent.get(item, 0)
            if not item:
                return -1
        return -1
    return of


def drops_suite(game, original, report, assets, times=1000):
    of = item_categories(assets)
    made = original.exec('spawn 76 --at +2,+0 --level 10 --hostile')[-1]
    oid = re.match(r'made (\d+)', made)[1]
    theirs = []
    for n in range(0, times, 200):
        for line in original.exec(f'drop {min(200, times - n)} --id {oid}')[1:]:
            theirs += [int(t) for t in re.findall(r'\d+ type (\d+)', line)]
    game.exec('spawn 76 --at +2,+0 --level 10 --hostile')
    ours = []
    for n in range(0, times, 200):
        line = game.exec(f'drop {min(200, times - n)}')[-1]
        if ': ' in line:
            ours += [int(re.match(r'\s*[a-z]+ (\d+)', item)[1]) for item in line.split(': ', 1)[1].split(', ')]
    tally = lambda found: collections.Counter(CATEGORIES.get(of(t), f'category {of(t)}') for t in found)
    a, b = tally(ours), tally(theirs)
    shares = [('any item', len(ours), len(theirs))]
    shares += [(key, a[key], b[key]) for key in sorted(set(a) | set(b), key=lambda k: -(a[k] + b[k]))]
    derived = lambda found: sum(1 for t in found if 2000 <= t <= 2999 or 6500 <= t <= 7999)
    shares.append(('derived types', derived(ours), derived(theirs)))
    for key, x, y in shares:
        p, q = x / times, y / times
        sigma = math.sqrt(max(p * (1 - p) + q * (1 - q), 1e-9) / times)
        ok = abs(p - q) <= max(0.02, 4 * sigma)
        report.check('drops', ok, f'{key}: {100 * p:.1f} % / {100 * q:.1f} %')


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--suites', default='experience,hero,stats,drops,places')
    parser.add_argument('--save', default='start-seraphim', help="the original's save of the Seraphim's new game")
    parser.add_argument('--types', help='the stats suite\'s creature types, comma-separated')
    parser.add_argument('--levels', default='1,10,25,40', help="the stats suite's levels")
    parser.add_argument('--assets', default=os.path.join(ROOT, 'assets'))
    parser.add_argument('--game-port', type=int, default=15702)
    parser.add_argument('--original-port', type=int, default=15703)
    args = parser.parse_args()
    game = compare.Endpoint('the game', args.game_port)
    original = compare.Endpoint('the original', args.original_port)
    report = Report()
    suites = args.suites.split(',')
    for suite in suites:
        fresh(game, original, args.save)
        if suite == 'experience':
            experience_suite(game, original, report)
        elif suite == 'hero':
            hero_suite(game, original, report)
        elif suite == 'stats':
            types = [int(t) for t in args.types.split(',')] if args.types else TYPES
            stats_suite(game, original, report, types, [int(level) for level in args.levels.split(',')])
        elif suite == 'drops':
            drops_suite(game, original, report, args.assets)
        elif suite == 'places':
            places_suite(game, original, report)
        else:
            parser.error(f'no suite {suite}')
    print(f'{report.checks} checks, {report.failed} failed')
    sys.exit(1 if report.failed else 0)


if __name__ == '__main__':
    main()
