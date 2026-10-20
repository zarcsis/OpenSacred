#!/usr/bin/env python3
"""Remote control of a running opensacred-game: the client of its Remote Protocol (docs/10 §8).

Purpose: Claude Code and scripts drive a game started with `--remote`: run its commands, read its state and log,
send the player's keys, text and clicks, take screenshots. The game listens on 127.0.0.1 only (Bevy's Remote
Protocol, JSON-RPC 2.0 over HTTP, port 15702 by default); this client calls the game's own methods
(`opensacred/exec`, `opensacred/state`, `opensacred/input`, `opensacred/screenshot`, `opensacred/logs`) or any other.

Example
  cargo run --release -p opensacred-game -- --remote --windowed --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log
  python -I tools/game/remote.py wait-state menu            # the game is up, on its menu
  python -I tools/game/remote.py exec "new-game --class mage"
  python -I tools/game/remote.py wait-ready                 # loaded, the hero animating, the sectors in place
  python -I tools/game/remote.py exec "go +6,+2"
  python -I tools/game/remote.py click 960 540              # as the player clicks the ground there
  python -I tools/game/remote.py key Escape                 # the pause's menu
  python -I tools/game/remote.py screenshot <scratch>/shot.png
  python -I tools/game/remote.py state
  python -I tools/game/remote.py logs --after 120
  python -I tools/game/remote.py exec quit

Expected output: `exec` prints the command's lines; `state` the overlay's lines (`--json` the snapshot); `logs` a
record a line, `sequence seconds LEVEL target: message`; the waits print the state they waited for; an error goes
to the standard error with the exit code 1. Dependencies: the standard library.
"""
import argparse, json, os, sys, time, urllib.error, urllib.request

HOST = '127.0.0.1'


def call(port, method, params=None):
    """Calls `method` with `params` and returns its result; raises RuntimeError with the game's message on an error,
    ConnectionError when nothing listens."""
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
    request = urllib.request.Request(f'http://{HOST}:{port}/', data=body,
                                     headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            answer = json.load(response)
    except urllib.error.URLError as e:
        raise ConnectionError(f'no game on {HOST}:{port}: {e.reason}') from e
    if 'error' in answer:
        raise RuntimeError(answer['error'].get('message', answer['error']))
    return answer.get('result')


def lines(snapshot):
    """The overlay's lines of a snapshot, as the game's `state` command prints them."""
    head = f"OpenSacred {snapshot['version']}"
    if snapshot.get('gpu'):
        head += f" | {snapshot['gpu']}"
    if snapshot.get('window'):
        width, height, mode = snapshot['window']
        head += f' | {width}x{height} {mode}'
    out = [head, f"{snapshot['fps']:6.1f} fps {snapshot['frame_ms']:6.2f} ms  slowest {snapshot['slowest_ms']:6.2f} ms  "
                 f"{snapshot['state']}"]
    hero = snapshot.get('hero')
    if hero:
        place = 'ground' if not hero.get('inside') else 'inside {},{}:{}'.format(*hero['inside'])
        line = (f"hero cell {hero['cell'][0]},{hero['cell'][1]} sector {hero['sector'][0]},{hero['sector'][1]} "
                f"{place}, {hero['gait']}, {'moving' if hero['moving'] else 'standing'}")
        if hero.get('target'):
            line += ' to {},{}'.format(*hero['target'])
        out.append(line)
    if snapshot.get('clock'):
        out.append(f"{snapshot['clock'][0]} {snapshot['clock'][1]}  zoom {snapshot.get('zoom') or 0:.2f}  "
                   f"sectors {snapshot['sectors'][0]} +{snapshot['sectors'][1]}")
        out.append(f"NPCs {snapshot['npcs']}  objects {snapshot['objects']}  entities {snapshot['entities']}"
                   + ('' if snapshot['ready'] else '  not ready'))
    return out


def wait(port, timeout, done, what):
    """Polls the state until `done(snapshot)`, through the game's start too, for at most `timeout` seconds."""
    start = time.monotonic()
    while True:
        try:
            snapshot = call(port, 'opensacred/state')
            if done(snapshot):
                return snapshot
        except ConnectionError:
            snapshot = None
        if time.monotonic() - start > timeout:
            state = snapshot['state'] if snapshot else 'no game'
            raise TimeoutError(f'{what} not reached in {timeout:g} s ({state})')
        time.sleep(0.25)


def screenshot(port, path, timeout):
    """Asks for the next frame and waits until its file is written whole."""
    path = os.path.abspath(path)
    if os.path.exists(path):
        os.remove(path)
    call(port, 'opensacred/screenshot', {'path': path})
    start, size = time.monotonic(), -1
    while time.monotonic() - start < timeout:
        if os.path.exists(path):
            now = os.path.getsize(path)
            if now > 0 and now == size:
                return path
            size = now
        time.sleep(0.2)
    raise TimeoutError(f'{path} not written in {timeout:g} s')


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--port', type=int, default=15702, help="the game's port (15702 by default)")
    sub = parser.add_subparsers(dest='what', required=True)
    p = sub.add_parser('exec', help='run a command of the game (help lists them)')
    p.add_argument('line')
    p = sub.add_parser('state', help="the game's state: the overlay's values")
    p.add_argument('--json', action='store_true', help='the snapshot as JSON')
    p = sub.add_parser('wait-ready', help='wait for a game played, its hero animating, its sectors in place')
    p.add_argument('--timeout', type=float, default=120)
    p = sub.add_parser('wait-state', help='wait for the state: menu, loading, game or paused')
    p.add_argument('state', choices=['menu', 'loading', 'game', 'paused'])
    p.add_argument('--timeout', type=float, default=120)
    p = sub.add_parser('key', help='press and release a key: F11, Escape, Enter, R, Backquote, ArrowUp, …, '
                                   'or keys held together: Control+V')
    p.add_argument('name')
    p = sub.add_parser('type', help='type text, as into the console')
    p.add_argument('text')
    p = sub.add_parser('move', help='move the cursor to a pixel of the window')
    p.add_argument('x', type=float)
    p.add_argument('y', type=float)
    p = sub.add_parser('click', help='click a pixel of the window')
    p.add_argument('x', type=float)
    p.add_argument('y', type=float)
    p.add_argument('--button', choices=['left', 'right', 'middle'], default='left')
    p.add_argument('--hold', type=int, default=2, help='frames the button is held')
    p = sub.add_parser('wheel', help='turn the wheel by lines, up positive')
    p.add_argument('lines', type=float)
    p = sub.add_parser('screenshot', help='save the next frame to a PNG and wait for it')
    p.add_argument('path')
    p.add_argument('--timeout', type=float, default=30)
    p = sub.add_parser('logs', help="the log's records")
    p.add_argument('--after', type=int, default=0, help='from this sequence number on')
    p = sub.add_parser('call', help='any method of the protocol, with JSON parameters')
    p.add_argument('method')
    p.add_argument('params', nargs='?', default=None)
    args = parser.parse_args()
    port = args.port
    try:
        if args.what == 'exec':
            print('\n'.join(call(port, 'opensacred/exec', {'line': args.line})['output']))
        elif args.what == 'state':
            snapshot = call(port, 'opensacred/state')
            print(json.dumps(snapshot, indent=2) if args.json else '\n'.join(lines(snapshot)))
        elif args.what == 'wait-ready':
            print('\n'.join(lines(wait(port, args.timeout, lambda s: s['ready'], 'a ready game'))))
        elif args.what == 'wait-state':
            print('\n'.join(lines(wait(port, args.timeout, lambda s: s['state'] == args.state, args.state))))
        elif args.what == 'key':
            call(port, 'opensacred/input', {'key': args.name})
        elif args.what == 'type':
            call(port, 'opensacred/input', {'text': args.text})
        elif args.what == 'move':
            call(port, 'opensacred/input', {'move': [args.x, args.y]})
        elif args.what == 'click':
            call(port, 'opensacred/input', {'click': [args.x, args.y], 'button': args.button, 'hold': args.hold})
        elif args.what == 'wheel':
            call(port, 'opensacred/input', {'wheel': args.lines})
        elif args.what == 'screenshot':
            print(screenshot(port, args.path, args.timeout))
        elif args.what == 'logs':
            for record in call(port, 'opensacred/logs', {'after': args.after}):
                print(f"{record['sequence']} {record['seconds']:9.3f} {record['level']:<5} "
                      f"{record['target']}: {record['message']}")
        elif args.what == 'call':
            params = json.loads(args.params) if args.params else None
            print(json.dumps(call(port, args.method, params), indent=2))
    except (ConnectionError, RuntimeError, TimeoutError) as e:
        print(f'error: {e}', file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
