#!/usr/bin/env python3
"""Remote control of a running opensacred-game: the client of its Remote Protocol (docs/10 §8).

Purpose: Claude Code and scripts drive a game started with `--remote`: run its commands, read its state and log,
send the player's keys, text and clicks, take screenshots. The game listens on 127.0.0.1 only (Bevy's Remote
Protocol, JSON-RPC 2.0 over HTTP, port 15702 by default); this client calls the game's own methods
(`opensacred/exec`, `opensacred/state`, `opensacred/input`, `opensacred/screenshot`, `opensacred/logs`) or any other.
With `--port 15703` it drives the original Sacred.exe under `tools/original/host.py`, which serves the same methods
(docs/03 §3.8). Under a Claude Code session, a game started with `--remote` and no port listens on a port of the
session's own, which it records under the session and its working directory (#1397); without `--port` this client
finds it there, so that two sessions, or two worktrees of one, never drive each other's games.

Example
  cargo run --release -p opensacred-game -- --remote --hidden --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log
  uv run python -I tools/game/remote.py wait-state menu            # the game is up, on its menu
  uv run python -I tools/game/remote.py exec "new-game --class mage"
  uv run python -I tools/game/remote.py wait-ready                 # loaded, the hero animating, the sectors in place
  uv run python -I tools/game/remote.py exec "go +6,+2"
  uv run python -I tools/game/remote.py click 960 540              # as the player clicks the ground there
  uv run python -I tools/game/remote.py key Escape                 # the pause's menu
  uv run python -I tools/game/remote.py screenshot <scratch>/shot.png
  uv run python -I tools/game/remote.py state
  uv run python -I tools/game/remote.py logs --after 120
  uv run python -I tools/game/remote.py wait-log "strike: .*hit" --after 120
  uv run python -I tools/game/remote.py exec quit
  uv run python -I tools/game/remote.py --port 15703 shell         # a line a command: "cell 3296,2528", ":state", ":quit"

Expected output: `exec` prints the command's lines; `state` the overlay's lines (`--json` the snapshot); `logs` a
record a line, `sequence seconds LEVEL target: message`; the waits print the state they waited for; an error goes
to the standard error with the exit code 1. `shell` reads lines until `:quit` or the input's end: a line is a command
of the game, a line starting with `:` one of this client's own (`:help` lists them), and an error ends the line
rather than the shell. Dependencies: the standard library.
"""
import argparse, glob, json, os, re, shlex, sys, tempfile, time, urllib.error, urllib.request

HOST = '127.0.0.1'
PORT = 15702
# Where the games of Claude Code sessions record their ports (#1397), as crates/opensacred-game's remote.rs writes them:
# `<session>-<directory>.port`, the game's port, pid and working directory in JSON.
RECORDS = os.path.join(os.environ.get('LOCALAPPDATA') or tempfile.gettempdir(), 'OpenSacred', 'remote')
SESSION = re.compile(r'[A-Za-z0-9-]+')


def fnv1a(data):
    """FNV-1a of 64 bits, as the game hashes a working directory."""
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return value


def record_key(session, directory):
    """The name of a session's record for a game started in `directory`: the session, then the hash of the directory,
    in lower case with backslashes on Windows, as the game makes it."""
    if os.name == 'nt':
        directory = directory.replace('/', '\\').lower()
    return f'{session}-{fnv1a(directory.encode("utf-8")):016x}'


def read_record(path):
    """A game's record of its port, or None when it cannot be read."""
    try:
        with open(path, encoding='utf-8') as f:
            record = json.load(f)
    except (OSError, ValueError):
        return None
    return record if isinstance(record, dict) and isinstance(record.get('port'), int) else None


def game_port(session=None, directory=None, records=RECORDS):
    """The port of the game to drive when no --port is given, and where it was found: the record of the game that this
    Claude Code session started in the working directory, else this session's only record, else 15702 (outside a
    session, or for a game given its port)."""
    session = os.environ.get('CLAUDE_CODE_SESSION_ID') if session is None else session
    if not session or not SESSION.fullmatch(session):
        return PORT, None
    own = os.path.join(records, record_key(session, directory or os.getcwd()) + '.port')
    paths = [own] if os.path.exists(own) else sorted(glob.glob(os.path.join(records, f'{session}-*.port')))
    found = [(path, record) for path, record in ((p, read_record(p)) for p in paths) if record]
    if len(found) > 1:
        listed = ', '.join(f"{record['port']} ({record.get('dir')})" for _, record in found)
        raise ConnectionError(f"this session's games listen on {listed}, none started here: pass --port")
    if found:
        record = found[0][1]
        return record['port'], f"this session's game started in {record.get('dir')} (pid {record.get('pid')})"
    return PORT, None


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
    head = f"{snapshot.get('name', 'OpenSacred')} {snapshot['version']}"
    if snapshot.get('gpu'):
        head += f" | {snapshot['gpu']}"
    if snapshot.get('window'):
        width, height, mode = snapshot['window']
        head += f' | {width}x{height} {mode}'
    out = [head, f"{snapshot['fps']:6.1f} fps {snapshot['frame_ms']:6.2f} ms  slowest {snapshot['slowest_ms']:6.2f} ms  "
                 f"{snapshot['state']}"]
    process = snapshot.get('process')
    if process:
        line = (f"process {process['pid']} " + ('running' if process['running'] else f"ended ({process['exit_code']})")
                + f", {snapshot['frames']} frames, {'focused' if snapshot.get('focused') else 'not focused'} as it believes")
        if snapshot.get('rect'):
            line += ', window at {},{}'.format(*snapshot['rect'][:2]) + f" ({snapshot['place']})"
        if snapshot.get('refusals'):
            line += f", {snapshot['refusals']} calls refused"
        out.append(line)
    ui = snapshot.get('ui')
    if ui and ui.get('point'):
        # The original's interface (#1340): the mouse's point and the one before, which a click is tested at.
        line = 'ui point {},{} before {},{}'.format(*ui['point'], *ui['before'])
        if ui.get('holds'):
            kind = 'item' if ui['holds']['kind'] == 1 else f"kind {ui['holds']['kind']}"
            line += f" holds {kind} {ui['holds']['id']}"
        if ui.get('under'):
            line += ' under ' + ' > '.join(ui['under'])
        if ui.get('under_before') and ui['under_before'] != ui.get('under'):
            line += ' (before: ' + (' > '.join(ui['under_before']) or 'none') + ')'
        if ui.get('focus'):
            line += f" focus {ui['focus']}"
        out.append(line)
    hero = snapshot.get('hero')
    if hero:
        place = 'ground' if not hero.get('inside') else 'inside {},{}:{}'.format(*hero['inside'])
        line = (f"hero cell {hero['cell'][0]},{hero['cell'][1]} sector {hero['sector'][0]},{hero['sector'][1]} "
                f"{place}, {hero['gait']}, {'moving' if hero['moving'] else 'standing'}")
        if hero.get('target'):
            line += ' to {},{}'.format(*hero['target'])
            if hero.get('onto'):
                line += f" {hero['onto']}"
        out.append(line)
    world = []
    if snapshot.get('clock'):
        world.append(f"{snapshot['clock'][0]} {snapshot['clock'][1]}")
    if snapshot.get('zoom') is not None:
        world.append(f"zoom {snapshot['zoom']:.2f}")
    if snapshot.get('sectors'):
        world.append(f"sectors {snapshot['sectors'][0]} +{snapshot['sectors'][1]}")
    if snapshot.get('cursor'):
        world.append(f"cursor {snapshot['cursor']}")
    if world:
        out.append('  '.join(world))
        out.append(f"NPCs {snapshot['npcs']}  objects {snapshot['objects']}  entities {snapshot['entities']}"
                   + ('' if snapshot['ready'] else '  not ready'))
        if snapshot.get('nearest'):
            current, most, level = snapshot['nearest']
            out.append(f'nearest creature  health {current}/{most}  level {level}')
        if snapshot.get('experience'):
            level, experience = snapshot['experience']
            gold = f"  gold {snapshot['gold']}" if snapshot.get('gold') is not None else ''
            if snapshot.get('health'):
                current, most = snapshot['health']
                out.append(f'hero level {level}  health {current}/{most}  experience {experience}{gold}')
            else:
                out.append(f'hero level {level}  experience {experience}{gold}')
    return out


def polled(port):
    """The port to poll: `port`, or where it is a function, the port it finds now, as a session's game records its
    port only as it starts (#1442)."""
    return port() if callable(port) else port


def wait(port, timeout, done, what):
    """Polls the state until `done(snapshot)`, through the game's start too, for at most `timeout` seconds; `port` may
    be a function that finds the port anew at each poll (`polled`)."""
    start = time.monotonic()
    while True:
        try:
            snapshot = call(polled(port), 'opensacred/state')
            if done(snapshot):
                return snapshot
        except ConnectionError:
            snapshot = None
        if time.monotonic() - start > timeout:
            state = snapshot['state'] if snapshot else 'no game'
            raise TimeoutError(f'{what} not reached in {timeout:g} s ({state})')
        time.sleep(0.25)


def wait_log(port, pattern, after, timeout):
    """Polls the log from record `after` on until a record's `target: message` matches the regular expression; `port`
    may be a function that finds the port anew at each poll (`polled`)."""
    expression = re.compile(pattern)
    start = time.monotonic()
    while True:
        try:
            for record in call(polled(port), 'opensacred/logs', {'after': after}):
                after = record['sequence'] + 1
                if expression.search(f"{record['target']}: {record['message']}"):
                    return record
        except ConnectionError:
            pass
        if time.monotonic() - start > timeout:
            raise TimeoutError(f'no record matching {pattern!r} in {timeout:g} s')
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


def add_commands(sub):
    """The client's commands, for the command line and for the shell's lines that start with `:`."""
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
    p.add_argument('--hold', type=int, help='frames the key is held (1 by default)')
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
    p.add_argument('--after', type=int, help='from this sequence number on (0; in the shell, from the last shown)')
    p = sub.add_parser('wait-log', help="wait for a record of the log whose 'target: message' matches a regular "
                                        "expression, and print it")
    p.add_argument('pattern')
    p.add_argument('--after', type=int, help='from this sequence number on (0; in the shell, from the last shown)')
    p.add_argument('--timeout', type=float, default=60)
    p = sub.add_parser('call', help='any method of the protocol, with JSON parameters')
    p.add_argument('method')
    p.add_argument('params', nargs='?', default=None)


def record_line(record):
    return f"{record['sequence']} {record['seconds']:9.3f} {record['level']:<5} {record['target']}: {record['message']}"


def run(port, args, after=0, waited=None):
    """Does one of the client's commands; `after` is where `logs` and `wait-log` start without `--after`, and the
    waits poll `waited` where it is given, else `port`. Gives the sequence number after the last record printed, or
    `after`."""
    waited = port if waited is None else waited
    if args.what == 'exec':
        print('\n'.join(call(port, 'opensacred/exec', {'line': args.line})['output']))
    elif args.what == 'state':
        snapshot = call(port, 'opensacred/state')
        print(json.dumps(snapshot, indent=2) if args.json else '\n'.join(lines(snapshot)))
    elif args.what == 'wait-ready':
        print('\n'.join(lines(wait(waited, args.timeout, lambda s: s['ready'], 'a ready game'))))
    elif args.what == 'wait-state':
        print('\n'.join(lines(wait(waited, args.timeout, lambda s: s['state'] == args.state, args.state))))
    elif args.what == 'key':
        params = {'key': args.name}
        if args.hold is not None:
            params['hold'] = args.hold
        call(port, 'opensacred/input', params)
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
        for record in call(port, 'opensacred/logs', {'after': after if args.after is None else args.after}):
            print(record_line(record))
            after = record['sequence'] + 1
    elif args.what == 'wait-log':
        record = wait_log(waited, args.pattern, after if args.after is None else args.after, args.timeout)
        print(record_line(record))
        after = record['sequence'] + 1
    elif args.what == 'call':
        params = json.loads(args.params) if args.params else None
        print(json.dumps(call(port, args.method, params), indent=2))
    return after


class LineError(Exception):
    """A shell's line that cannot run: its message, or None once argparse has printed what it had to say."""


class LineParser(argparse.ArgumentParser):
    """Parses the shell's own commands: an error or `--help` ends the line, never the shell."""

    def error(self, message):
        raise LineError(f'{self.prog}: {message}')

    def exit(self, status=0, message=None):
        if message:
            print(message, file=sys.stderr, end='')
        raise LineError(None)


def shell_parser():
    parser = LineParser(prog=':', add_help=False, description="The shell's own commands, each after a ':'.")
    sub = parser.add_subparsers(dest='what', required=True, parser_class=LineParser, metavar='COMMAND')
    add_commands(sub)
    p = sub.add_parser('follow', help="print the log's records after each line, from now on: on or off (off at the "
                                      "start)")
    p.add_argument('switch', choices=['on', 'off'])
    sub.add_parser('help', help="these commands; a line without ':' is a command of the game, whose own help lists "
                                "them")
    sub.add_parser('quit', aliases=['exit'], help='end the shell, leaving the game running (also Ctrl+Z or Ctrl+D)')
    return parser


def words(line):
    """A line's words as the game's commands split them (#368): blanks part them, quotes keep blanks within one, and a
    backslash is a plain character, so that a Windows path takes either separator."""
    lexer = shlex.shlex(line, posix=True)
    lexer.whitespace_split = True
    lexer.escape = ''
    return list(lexer)


def complain(message):
    """Prints to the standard error after what the standard output holds, so that a piped script keeps its order."""
    sys.stdout.flush()
    print(message, file=sys.stderr, flush=True)


def last_record(port):
    """The sequence number after the log's last record, or 0 when nothing answers."""
    try:
        records = call(port, 'opensacred/logs', {'after': 0})
    except (ConnectionError, RuntimeError):
        return 0
    return records[-1]['sequence'] + 1 if records else 0


def shell(port):
    """Reads lines until `:quit` or the input's end: a line is a command of the game, one starting with `:` a command
    of this client; an error, a lost connection or Ctrl+C ends the line, not the shell."""
    parser = shell_parser()
    interactive = sys.stdin.isatty()
    try:
        name = call(port, 'opensacred/state').get('name', 'OpenSacred')
    except (ConnectionError, RuntimeError) as e:
        complain(f'error: {e}; the next line tries again')
        name = str(port)
    if interactive:
        print(f"{name} on {HOST}:{port}: a line runs a command of the game (help lists them), "
              f"':help' the shell's own, ':quit' ends the shell")
    after, follow = last_record(port), False
    while True:
        try:
            line = input(f'{name}> ' if interactive else '').strip()
        except EOFError:
            break
        except KeyboardInterrupt:
            print()
            continue
        if not line or line.startswith('#'):
            continue
        if not interactive:
            print(f'> {line}', flush=True)
        try:
            if not line.startswith(':'):
                print('\n'.join(call(port, 'opensacred/exec', {'line': line})['output']))
            else:
                args = parser.parse_args(words(line[1:]))
                if args.what in ('quit', 'exit'):
                    break
                if args.what == 'help':
                    parser.print_help()
                elif args.what == 'follow':
                    follow = args.switch == 'on'
                    if follow:
                        after = last_record(port)
                else:
                    after = run(port, args, after)
            if follow:
                # opensacred-game logs each command it runs and its output under `remote`, printed above already.
                for record in call(port, 'opensacred/logs', {'after': after}):
                    if record['target'] != 'remote':
                        print(record_line(record))
                    after = record['sequence'] + 1
        except LineError as e:
            if e.args[0]:
                complain(f'error: {e.args[0]}')
        except (ConnectionError, RuntimeError, TimeoutError, ValueError) as e:
            complain(f'error: {e}')
        except KeyboardInterrupt:
            complain('interrupted')


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--port', type=int,
                        help=f"the game's port: by default this Claude Code session's game's (#1397), else {PORT}")
    sub = parser.add_subparsers(dest='what', required=True)
    add_commands(sub)
    sub.add_parser('shell', help="read lines until ':quit': a command of the game each, or with ':' one of this "
                                 "client's own (:help)")
    args = parser.parse_args()
    found = None
    given = args.port is not None
    try:
        if not given:
            args.port, found = game_port()
        if args.what == 'shell':
            shell(args.port)
            return
        # A session's game that has not recorded its port yet, as it starts, is looked for anew at each poll of a
        # wait (#1442).
        waited = None if given or found else (lambda: game_port()[0])
        run(args.port, args, waited=waited)
    except ConnectionError as e:
        stale = f' (the record of {found}: a game ended by force leaves it)' if found else ''
        print(f'error: {e}{stale}', file=sys.stderr)
        sys.exit(1)
    except (RuntimeError, TimeoutError) as e:
        print(f'error: {e}', file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
