#!/usr/bin/env python3
"""The host of the original's driver (#448, docs/03 §3.8): starts the developer's own Sacred.exe under Frida in the
background and serves its remote control, as opensacred-game serves its own (docs/10 §8).

Purpose: Claude Code sees how the running original behaves without disturbing the maintainer. The host checks
`Sacred.exe` against the manifest, sets the keys of `Settings.cfg` it needs, spawns the game suspended with its
working directory the installation, loads `agent.js` and resumes the game once every guard of the maintainer's desktop
is in place (else it kills it). The game lives in a job object that kills it when the host's handle closes, and the
agent ends it when the host's heartbeat stops. The host serves JSON-RPC 2.0 over HTTP on 127.0.0.1 with the game's
own methods: `opensacred/state`, `opensacred/screenshot {path}`, `opensacred/logs {after}`, `opensacred/exec {line}`.
`tools/game/remote.py --port 15703` is its client. Frames, logs and traces stay on this machine (docs/08 §3).

Example
  python -I tools/original/host.py --log-file <scratch>/original.log          # run_in_background
  python -I tools/game/remote.py --port 15703 state
  python -I tools/game/remote.py --port 15703 screenshot <scratch>/menu.png
  python -I tools/game/remote.py --port 15703 exec quit

Expected output: the log's records, a line each, `sequence seconds LEVEL target: message`; the exit code is the
game's. Dependencies: frida, Pillow (tools/requirements.txt); Windows.
"""
import argparse, ctypes, hashlib, http.server, json, os, queue, re, signal, sys, tempfile, threading, time
from ctypes import wintypes

import frida
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MANIFEST = os.path.join(ROOT, 'docs', 'manifest-gog-2.28.txt')
AGENT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'agent.js')
WIDTH, HEIGHT = 1024, 768

# The original's menus, in the window's pixels (1024 x 768), recorded on 2026-10-21 (#450).
MAIN_ANCARIA = (512, 287)       # the main menu's "Ancaria Campaign"
HEROES = {                      # the figures of the hero's choice, left to right as they stand
    'gladiator': (95, 480), 'seraphim': (215, 380), 'daemon': (365, 380), 'dwarf': (530, 430),
    'wood-elf': (620, 330), 'mage': (735, 400), 'dark-elf': (805, 330), 'vampiress': (890, 480),
}
HERO_NAME = (510, 586)          # the name's field, under the figures
DIFFICULTY_NEXT = (593, 621)    # the difficulty's right arrow; the original offers the levels unlocked alone
DIFFICULTIES = ['bronze', 'silver', 'gold', 'platinum', 'niobium']
START_GAME = (607, 714)
# The first window of a new game, of a height that depends on its text: Enter is its OK, and its Skip Tutorial and the
# tutorial's first tip's No more Tips are green words found on a frame, in these columns.
DREAM_SKIP_TUTORIAL = (540, 680)
TIP_NO_MORE = (560, 700)
MAIN_LOAD = (512, 415)          # the main menu's "Load Game"
MENU_SAVE = (512, 346)          # the menu over a game (Escape): "Save Game", which saves and loads
SAVES_LOAD, SAVES_SAVE = (866, 302), (866, 367)
SAVES_CONFIRM = (422, 440)      # OK of "Load game?" and of "Overwrite the old savegame?"
SAVES_UP, SAVES_DOWN = (734, 347), (734, 655)   # the list's arrows, a row a click; it keeps where it was scrolled
SAVES_ROWS = 3                  # rows shown: the saves by their number, then one empty
SAVES_NAME = lambda row: (455, 324 + 137 * row)    # a row's name field
SAVES_PICTURE = lambda row: (135, 360 + 137 * row)  # a row's picture, which picks it
SAVE_NAME_OFFSET = 0x500        # the save's name in save/GAMEnn.PAK, a C string
SAVE_NAME = re.compile(r'[A-Za-z0-9 _.-]{1,24}')

# The world's commands (#451). A cell that the scripts' Teleport takes for the end of the game (sdk_ENGINE_TP) is
# refused; the interface's bar covers the window's bottom, where a click would not reach the ground.
FINAL_CELL = (0x189C, 0x591)
GROUND_BOTTOM = 650
CELL_LINE = re.compile(r'^\s*(\+|-)?(\d+)\s*,\s*(\+|-)?(\d+)\s*$')

# The keys of Settings.cfg the host sets before each start, and nothing else (#448, decision 2).
SETTINGS = {'FULLSCREEN': '0', 'SHOWMOVIE': '0'}
VOLUMES = {'MUSICVOLUME': '0', 'SFXVOLUME': '0', 'VOICEVOLUME': '0'}
# With --muted the game plays its sounds at the installation's volumes, for the sound traces, and the agent silences
# mss32.dll's output.
MUTED_VOLUMES = {'MUSICVOLUME': '64', 'SFXVOLUME': '64', 'VOICEVOLUME': '64'}

kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.CreateJobObjectW.restype = wintypes.HANDLE
kernel32.CreateJobObjectW.argtypes = [wintypes.LPVOID, wintypes.LPCWSTR]
kernel32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel32.WaitForSingleObject.restype = wintypes.DWORD
kernel32.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
kernel32.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]

PROCESS_TERMINATE, PROCESS_SET_QUOTA, PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZE = 0x1, 0x100, 0x1000, 0x100000
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000
JOB_OBJECT_EXTENDED_LIMIT_INFORMATION = 9
WAIT_OBJECT_0 = 0


class BasicLimits(ctypes.Structure):
    _fields_ = [('PerProcessUserTimeLimit', ctypes.c_int64), ('PerJobUserTimeLimit', ctypes.c_int64),
                ('LimitFlags', wintypes.DWORD), ('MinimumWorkingSetSize', ctypes.c_size_t),
                ('MaximumWorkingSetSize', ctypes.c_size_t), ('ActiveProcessLimit', wintypes.DWORD),
                ('Affinity', ctypes.c_size_t), ('PriorityClass', wintypes.DWORD), ('SchedulingClass', wintypes.DWORD)]


class ExtendedLimits(ctypes.Structure):
    _fields_ = [('BasicLimitInformation', BasicLimits), ('IoInfo', ctypes.c_uint64 * 6),
                ('ProcessMemoryLimit', ctypes.c_size_t), ('JobMemoryLimit', ctypes.c_size_t),
                ('PeakProcessMemoryUsed', ctypes.c_size_t), ('PeakJobMemoryUsed', ctypes.c_size_t)]


class Journal:
    """The host's and the agent's records, numbered from 0, also printed and written to the log file."""

    def __init__(self, path):
        self.start = time.monotonic()
        self.records = []
        self.lock = threading.Lock()
        self.file = open(path, 'a', encoding='utf-8', newline='') if path else None

    def push(self, level, target, message):
        with self.lock:
            record = {'sequence': len(self.records), 'seconds': time.monotonic() - self.start, 'level': level,
                      'target': target, 'message': message}
            self.records.append(record)
            line = f"{record['sequence']} {record['seconds']:9.3f} {level:<5} {target}: {message}"
            print(line, flush=True)
            if self.file:
                self.file.write(line + '\n')
                self.file.flush()

    def since(self, after):
        with self.lock:
            return self.records[max(0, after):]


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def check_executable(game_dir):
    """Sacred.exe must be the manifest's, as for the oracle (docs/03 §3.4)."""
    expected = None
    with open(MANIFEST, encoding='utf-8') as f:
        for line in f:
            digest, _, name = line.strip().partition('  ')
            if name == 'Sacred.exe':
                expected = digest
    exe = os.path.join(game_dir, 'Sacred.exe')
    if not os.path.isfile(exe):
        raise SystemExit(f'error: {exe} not found')
    actual = sha256(exe)
    if actual != expected:
        raise SystemExit(f'error: {exe} is not the GOG 2.28 build of the manifest (SHA-256 {actual})')
    return exe


def set_settings(game_dir, sound, muted, journal):
    """Sets FULLSCREEN, SHOWMOVIE and the three volumes in Settings.cfg (0; the game's 64 when `muted`; kept with
    `sound`); keeps every other line."""
    path = os.path.join(game_dir, 'Settings.cfg')
    wanted = dict(SETTINGS, **(MUTED_VOLUMES if muted else {} if sound else VOLUMES))
    with open(path, 'rb') as f:
        text = f.read().decode('latin-1')
    lines = text.split('\r\n')
    changed = []
    for i, line in enumerate(lines):
        key, sep, value = line.partition(':')
        key = key.strip()
        if sep and key in wanted:
            if value.strip() != wanted[key]:
                lines[i] = f'{key} : {wanted[key]}'
                changed.append(f'{key} {value.strip()} -> {wanted[key]}')
            del wanted[key]
    if wanted:
        at = len(lines) - 1 if lines and lines[-1] == '' else len(lines)
        for key, value in wanted.items():
            lines.insert(at, f'{key} : {value}')
            at += 1
            changed.append(f'{key} -> {value} (added)')
    if changed:
        with open(path, 'wb') as f:
            f.write('\r\n'.join(lines).encode('latin-1'))
        journal.push('INFO', 'host', 'Settings.cfg: ' + ', '.join(changed))


class Host:
    def __init__(self, args, journal):
        self.args = args
        self.journal = journal
        self.game_dir = os.path.realpath(args.game_dir)
        self.pid = None
        self.process = None
        self.job = None
        self.session = None
        self.script = None
        self.agent = None
        self.shots = queue.Queue()  # the paths asked for, in order
        self.ended = threading.Event()
        self.exit_code = None
        self.fatal = None
        self.scripted = None  # what the host clicks through now, as `new-game`

    # --- Start ---------------------------------------------------------------------------------------------------

    def start(self):
        exe = check_executable(self.game_dir)
        self.journal.push('INFO', 'host', 'Sacred.exe matches the manifest')
        device = frida.get_local_device()
        running = [p.pid for p in device.enumerate_processes() if p.name.lower() == 'sacred.exe']
        if running:
            raise SystemExit(f'error: a Sacred.exe runs already (pid {running[0]}); one original runs at a time')
        set_settings(self.game_dir, self.args.sound, self.args.muted, self.journal)
        with open(self.args.agent, encoding='utf-8') as f:
            source = f.read()
        self.pid = frida.spawn([exe] + self.args.arg, cwd=self.game_dir)
        self.journal.push('INFO', 'host', f'spawned Sacred.exe suspended, pid {self.pid}')
        try:
            self.enter_job()
            self.session = frida.attach(self.pid)
            self.session.on('detached', self.on_detached)
            self.script = self.session.create_script(source, name='agent')
            self.script.on('message', self.on_message)
            self.script.load()
            self.agent = self.script.exports_sync
            report = self.agent.init({'place': self.args.place, 'heartbeatTimeout': 5000,
                                      'gameLog': not self.args.no_game_log, 'muted': self.args.muted})
            missing = [name for name, ok in report['guards'].items() if not ok]
            if missing:
                raise RuntimeError(f'guards not in place: {", ".join(missing)}')
            self.journal.push('INFO', 'host', f"guards in place ({report['arch']}): {', '.join(report['guards'])}")
        except Exception as e:
            self.journal.push('ERROR', 'host', f'the agent failed, the game is killed: {e}')
            self.kill(1)
            raise SystemExit(1)
        threading.Thread(target=self.heartbeat, daemon=True).start()
        frida.resume(self.pid)
        self.journal.push('INFO', 'host', 'resumed')
        threading.Thread(target=self.watch, daemon=True).start()

    def enter_job(self):
        """The game never outlives the host: its job kills it when the host's handle closes."""
        access = PROCESS_TERMINATE | PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE
        self.process = kernel32.OpenProcess(access, False, self.pid)
        if not self.process:
            raise RuntimeError(f'OpenProcess failed: {ctypes.get_last_error()}')
        self.job = kernel32.CreateJobObjectW(None, None)
        if not self.job:
            raise RuntimeError(f'CreateJobObjectW failed: {ctypes.get_last_error()}')
        limits = ExtendedLimits()
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not kernel32.SetInformationJobObject(self.job, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                                                ctypes.byref(limits), ctypes.sizeof(limits)):
            raise RuntimeError(f'SetInformationJobObject failed: {ctypes.get_last_error()}')
        if not kernel32.AssignProcessToJobObject(self.job, self.process):
            raise RuntimeError(f'AssignProcessToJobObject failed: {ctypes.get_last_error()}')
        self.journal.push('INFO', 'host', 'the game is in a job that ends with the host')

    def heartbeat(self):
        while not self.ended.is_set():
            try:
                self.agent.heartbeat()
            except Exception:
                pass
            self.ended.wait(1)

    def watch(self):
        while kernel32.WaitForSingleObject(self.process, 500) != WAIT_OBJECT_0:
            pass
        code = wintypes.DWORD()
        kernel32.GetExitCodeProcess(self.process, ctypes.byref(code))
        self.exit_code = code.value
        self.journal.push('INFO', 'host', f'the game exited with code {code.value}')
        self.ended.set()

    def kill(self, code):
        if self.process:
            kernel32.TerminateProcess(self.process, code)
        elif self.pid:
            try:
                frida.kill(self.pid)
            except Exception:
                pass

    # --- The agent's messages ------------------------------------------------------------------------------------

    def on_message(self, message, data):
        if message['type'] == 'error':
            self.journal.push('ERROR', 'agent', message.get('stack') or message.get('description'))
            return
        payload = message['payload']
        kind = payload.get('type')
        if kind == 'log':
            self.journal.push(payload['level'], 'agent', payload['message'])
        elif kind == 'game':
            self.journal.push('INFO', 'game', payload['message'])
        elif kind == 'trace':
            self.journal.push('INFO', 'trace', f"{payload['name']}: {payload['message']}")
        elif kind == 'fatal':
            self.fatal = payload['message']
            self.journal.push('ERROR', 'agent', f"the game is ended: {payload['message']}")
        elif kind == 'shot':
            threading.Thread(target=self.convert_shot, args=(payload['file'], payload['frame']), daemon=True).start()

    def on_detached(self, reason, crash):
        self.journal.push('WARN' if reason != 'process-terminated' else 'INFO', 'host', f'detached: {reason}')
        if crash is not None:
            self.journal.push('ERROR', 'host', f'crash: {crash.summary}')

    def convert_shot(self, file, frame):
        try:
            path = self.shots.get_nowait()
        except queue.Empty:
            self.journal.push('WARN', 'host', f'a screenshot nobody asked for: {file}')
            return
        tga = os.path.join(self.game_dir, file.replace('\\', os.sep).lstrip('.' + os.sep))
        try:
            with Image.open(tga) as image:
                rgb = image.convert('RGB')
            partial = path + '.partial'
            rgb.save(partial, 'PNG')
            os.replace(partial, path)
            self.journal.push('INFO', 'host', f'screenshot of frame {frame}: {path}')
        except Exception as e:
            self.journal.push('ERROR', 'host', f'the screenshot {tga} could not be converted: {e}')
            return
        if not self.args.keep_captures:
            self.remove_capture(tga)

    def remove_capture(self, tga):
        """Removes the capture the host asked for once the game's own thread has written its JPEG beside it."""
        jpeg = os.path.splitext(tga)[0] + '.jpg'
        size, deadline = -1, time.monotonic() + 15
        while time.monotonic() < deadline:
            if os.path.exists(jpeg):
                now = os.path.getsize(jpeg)
                if now > 0 and now == size:
                    break
                size = now
            time.sleep(0.5)
        for path in (tga, jpeg):
            try:
                os.remove(path)
            except OSError:
                pass

    # --- The methods ---------------------------------------------------------------------------------------------

    def state(self):
        running = not self.ended.is_set()
        agent = {}
        if running and self.agent is not None:
            try:
                agent = self.agent.state()
            except Exception as e:
                agent = {'error': str(e)}
        rect = agent.get('rect')
        snapshot = {
            'name': 'Sacred', 'version': '2.28 (GOG)',
            'state': 'ended' if not running else agent.get('screen', 'starting') if agent.get('frames') else 'starting',
            'ready': running and agent.get('screen') == 'game' and self.scripted is None,
            'scripted': self.scripted,
            'fps': agent.get('fps', 0), 'frame_ms': agent.get('frame_ms', 0), 'slowest_ms': agent.get('slowest_ms', 0),
            'window': [rect[2] - rect[0], rect[3] - rect[1], 'windowed'] if rect else None,
            'process': {'pid': self.pid, 'running': running, 'exit_code': self.exit_code},
            'frames': agent.get('frames', 0),
            'focused': agent.get('focused'),
            'rect': rect,
            'place': self.args.place,
            'refusals': agent.get('refusals', 0),
            'flip_error': agent.get('flip_error', 0),
            'muted': agent.get('muted'),
            'cursor': agent.get('cursor'),
            'npcs': 0, 'objects': 0, 'entities': 0,
        }
        if running and agent.get('screen') in ('game', 'paused'):
            try:
                world = self.agent.world('snapshot', {})
            except Exception as e:
                world = None
                snapshot['world_error'] = str(e)
            if world:
                snapshot.update(self.world_fields(world))
        return snapshot

    @staticmethod
    def world_fields(world):
        """The world's snapshot of the agent in the fields of opensacred-game's state."""
        fields = {'npcs': world['creatures'], 'objects': world['objects'], 'entities': world['objects'],
                  'zoom': world['zoom']}
        if world.get('clock'):
            fields['clock'] = [world['clock']['time'], world['clock']['phase']]
            fields['day_rates'] = world['clock']['rates']
        hero = world.get('hero')
        if hero:
            x, y = hero['cell']
            fields['hero'] = {
                'cell': [x, y], 'sector': [x >> 6, y >> 6],
                'inside': [x >> 6, y >> 6, hero['floor']] if hero['inside'] else None,
                'target': None, 'onto': None,
                'gait': 'run' if hero['gait'] == 2 else 'walk', 'moving': hero['action'][0] == 1,
            }
            fields['health'] = [hero['health'], hero['most']]
            fields['experience'] = [hero['level'], hero['experience']]
            fields['hero_state'] = {'id': hero['id'], 'type': hero['type'], 'ai': hero['ai'], 'action': hero['action']}
        nearest = world.get('nearest')
        if nearest:
            fields['nearest'] = [nearest['health'], nearest['most'], nearest['level']]
            fields['nearest_creature'] = nearest
        return fields

    def screenshot(self, params):
        path = os.path.abspath(params['path'])
        if self.ended.is_set():
            raise RuntimeError('the game has ended')
        self.shots.put(path)
        self.agent.screenshot()
        return None

    def input(self, params):
        """The player's input, as opensacred-game's `opensacred/input` takes it (the window's pixels, 1024 x 768)."""
        if self.ended.is_set():
            raise RuntimeError('the game has ended')
        allowed = {'key', 'text', 'move', 'click', 'button', 'hold', 'wheel'}
        unknown = set(params or {}) - allowed
        if unknown:
            raise ValueError(f"unknown fields: {', '.join(sorted(unknown))}")
        self.agent.input(params)
        return None

    def logs(self, params):
        return self.journal.since(int((params or {}).get('after', 0)))

    def exec(self, params):
        line = params['line'].strip()
        word, _, rest = line.partition(' ')
        if word == 'help':
            return {'output': [
                'help: these lines',
                'new-game [--class CLASS] [--difficulty LEVEL] [--name NAME] [--skip-tutorial]: from the main menu, '
                'a new game of the Ancaria campaign (classes: ' + ', '.join(HEROES) + '; the Seraphim by default; '
                'levels: ' + ', '.join(DIFFICULTIES) + ', as far as the hero has unlocked them); then wait-ready',
                "save NAME: in a game, saves through the original's menu into the save of that name, or a new one",
                'load NAME: from the main menu or a game, loads the save of that name; then wait-ready',
                'saves: the saves of save/, their files and names',
                'creatures [RADIUS]: the creatures within RADIUS cells of the hero (20 by default), the nearest first',
                'cell X,Y [FLOOR]: the hero onto that cell (on the ground, or a building\'s floor), or the nearest one '
                'it may stand on, as the scripts teleport a player',
                'sector X,Y: the hero into that sector, near its middle',
                'time HH:MM: the clock; day-speed DAY [NIGHT]: its rates (1 by default, 0 stops it)',
                'zoom Z: the view\'s zoom, 0.5 to 2 (the game settles on 0.5, 1 or 2)',
                'point X,Y: the window\'s pixel of a cell, for click',
                'go X,Y or go +DX,+DY: the hero walks there, by a click on the cell\'s pixel',
                'trace PRESET|GROUP|ADDRESS [--name N] [--abi cdecl|stdcall|thiscall|fastcall] [--args K,K,...] '
                '[--ret K] [--ids ID,ID] [--limit N]: log every call (kinds: int uint hex float ptr object id cstr '
                'out-float; the first argument is ECX for thiscall and fastcall); traces lists those running and the '
                'presets; untrace NAME|GROUP|all',
                'quit: closes the game as its window is closed, ends it after 15 s if it is still running']}
        if word == 'new-game':
            return {'output': [self.start_script('new-game', self.new_game, self.new_game_options(rest))]}
        if word in ('trace', 'untrace', 'traces'):
            return {'output': self.command_trace(word, rest.strip())}
        if word in WORLD_COMMANDS:
            return {'output': WORLD_COMMANDS[word](self, rest.strip())}
        if word == 'saves':
            return {'output': [f'{file} {name}' for file, name in self.saves()] or ['no saves']}
        if word in ('save', 'load'):
            name = rest.strip()
            if not SAVE_NAME.fullmatch(name):
                raise ValueError(f'{word} NAME: 1 to 24 letters, digits, spaces, _ . -')
            screen = self.agent_state().get('screen')
            if word == 'save' and screen != 'game':
                raise RuntimeError('save works in a game, not on the screen ' + str(screen))
            if word == 'load':
                if name not in [n for _, n in self.saves()]:
                    raise RuntimeError(f'no save {name} (exec saves lists them)')
                if screen not in ('menu', 'game'):
                    raise RuntimeError('load works from the main menu or a game, not on the screen ' + str(screen))
            run = self.save if word == 'save' else self.load
            return {'output': [self.start_script(f'{word} {name}', run, name)]}
        if word == 'quit':
            if self.ended.is_set():
                return {'output': ['the game has ended']}
            closed = self.agent.close()
            threading.Thread(target=self.quit_later, daemon=True).start()
            return {'output': ['closing the game' if closed else 'no window yet: ending the game']}
        raise RuntimeError(f'unknown command: {word} (help lists them)')

    # --- The menus ---------------------------------------------------------------------------------------------

    def new_game_options(self, rest):
        parser = argparse.ArgumentParser(prog='new-game', add_help=False, exit_on_error=False)
        parser.add_argument('--class', dest='hero', choices=list(HEROES), default='seraphim')
        parser.add_argument('--difficulty', choices=DIFFICULTIES, default='bronze')
        parser.add_argument('--name')
        parser.add_argument('--skip-tutorial', action='store_true')
        try:
            options, unknown = parser.parse_known_args(rest.split())
        except (argparse.ArgumentError, SystemExit) as e:
            raise ValueError(f'new-game: {e}') from None
        if unknown:
            raise ValueError(f"new-game: unknown {' '.join(unknown)}")
        if self.agent_state().get('screen') != 'menu':
            raise RuntimeError('new-game starts from the main menu')
        return options

    def start_script(self, name, run, options):
        if self.scripted is not None:
            raise RuntimeError(f'{self.scripted} is running')
        self.scripted = name

        def body():
            try:
                run(options)
                self.journal.push('INFO', 'host', f'{name}: done')
            except Exception as e:
                self.journal.push('ERROR', 'host', f'{name}: {e}')
            finally:
                self.scripted = None
        threading.Thread(target=body, daemon=True).start()
        return f'{name}: running (wait-ready)'

    def agent_state(self):
        try:
            return self.agent.state()
        except Exception:
            return {}

    def click(self, point, pause=1.0):
        self.agent.input({'click': list(point)})
        time.sleep(pause)

    def wait_screen(self, screens, timeout):
        deadline = time.monotonic() + timeout
        while self.agent_state().get('screen') not in screens:
            if self.ended.is_set() or time.monotonic() > deadline:
                raise TimeoutError(f"no {' or '.join(screens)} in {timeout:g} s")
            time.sleep(0.25)

    def green_words(self, columns):
        """The middle of the green words (a window's buttons) of the next frame within `columns`, above the bar of the
        game's interface, or None."""
        path = os.path.join(tempfile.gettempdir(), f'opensacred-original-{os.getpid()}.png')
        if os.path.exists(path):
            os.remove(path)
        self.screenshot({'path': path})
        deadline = time.monotonic() + 30
        while not os.path.exists(path):
            if time.monotonic() > deadline:
                raise TimeoutError('no frame in 30 s')
            time.sleep(0.2)
        with Image.open(path) as image:
            pixels = image.convert('RGB').load()
        os.remove(path)
        x0, x1 = columns
        rows = [y for y in range(150, 690)
                if sum(1 for x in range(x0, x1) if pixels[x, y][1] > 170 and pixels[x, y][0] < 90
                       and pixels[x, y][2] < 90) >= 3]
        if not rows:
            return None
        return ((x0 + x1) // 2, (rows[0] + rows[-1]) // 2)

    def wait_main_menu(self):
        """The main menu takes clicks a few seconds after it first appears."""
        while self.agent_state().get('frames', 0) < 200:
            time.sleep(0.25)
        time.sleep(2)

    def new_game(self, options):
        """Clicks through the original's menus: the Ancaria campaign, the hero, the name, the difficulty, Start Game,
        then the first window of the game and the tutorial's first tip."""
        step = lambda message: self.journal.push('INFO', 'host', f'new-game: {message}')
        self.wait_main_menu()
        step('Ancaria Campaign')
        self.click(MAIN_ANCARIA, 2.5)
        step(options.hero)
        self.click(HEROES[options.hero], 1.5)
        if options.name:
            self.click(HERO_NAME, 0.5)
            for _ in range(24):
                self.agent.input({'key': 'Backspace'})
                time.sleep(0.05)
            self.agent.input({'text': options.name})
            time.sleep(0.5)
        for _ in range(DIFFICULTIES.index(options.difficulty)):
            self.click(DIFFICULTY_NEXT, 0.5)
        step('Start Game')
        self.click(START_GAME, 0.5)
        self.wait_screen({'loading', 'game', 'paused'}, 120)
        self.wait_screen({'game', 'paused'}, 300)
        time.sleep(2)
        if options.skip_tutorial:
            step('Skip Tutorial')
            button = self.green_words(DREAM_SKIP_TUTORIAL)
            if button is None:
                raise RuntimeError('no Skip Tutorial on the first window')
            self.click(button, 2)
        else:
            step('OK, then No more Tips')
            self.agent.input({'key': 'Enter'})
            time.sleep(2)
            button = self.green_words(TIP_NO_MORE)
            if button is None:
                raise RuntimeError("no tutorial's tip after the first window")
            self.click(button, 1.5)
        self.wait_screen({'game'}, 30)

    def saves(self):
        """The saves of save/, in the order of the original's list (by their number), with their names."""
        folder = os.path.join(self.game_dir, 'save')
        found = []
        for file in os.listdir(folder) if os.path.isdir(folder) else []:
            match = re.fullmatch(r'GAME(\d+)\.PAK', file, re.IGNORECASE)
            if match:
                with open(os.path.join(folder, file), 'rb') as f:
                    f.seek(SAVE_NAME_OFFSET)
                    name = f.read(64).split(b'\0')[0].decode('latin-1')
                found.append((int(match.group(1)), file, name))
        return [(file, name) for _, file, name in sorted(found)]

    def open_saves(self):
        """Opens the list of saves: from a game through its menu, from the main menu through Load Game."""
        if self.agent_state().get('screen') == 'menu':
            self.wait_main_menu()
            self.click(MAIN_LOAD, 2.5)
        else:
            self.agent.input({'key': 'Escape'})
            time.sleep(1)
            self.click(MENU_SAVE, 1.5)

    def show_row(self, row):
        """Scrolls the list to its top, then until `row` shows, and gives its place among the rows shown."""
        for _ in range(len(self.saves())):
            self.click(SAVES_UP, 0.3)
        for _ in range(max(0, row - (SAVES_ROWS - 1))):
            self.click(SAVES_DOWN, 0.4)
        return min(row, SAVES_ROWS - 1)

    def save(self, name):
        saves = self.saves()
        names = [n for _, n in saves]
        row = names.index(name) if name in names else len(saves)
        before = {file: os.path.getmtime(os.path.join(self.game_dir, 'save', file)) for file, _ in saves}
        self.open_saves()
        shown = self.show_row(row)
        self.click(SAVES_NAME(shown), 0.4)
        self.agent.input({'key': 'End'})
        for _ in range(32):
            self.agent.input({'key': 'Backspace'})
            time.sleep(0.03)
        self.agent.input({'text': name})
        time.sleep(0.4)
        self.click(SAVES_SAVE, 1.5)
        if row < len(saves):
            self.click(SAVES_CONFIRM, 0.5)
        deadline = time.monotonic() + 60
        while True:
            now = self.saves()
            written = [file for file, n in now if n == name and before.get(file) != os.path.getmtime(
                os.path.join(self.game_dir, 'save', file))]
            if written:
                break
            if time.monotonic() > deadline:
                raise TimeoutError('the save was not written in 60 s')
            time.sleep(0.5)
        self.wait_screen({'game'}, 30)
        self.journal.push('INFO', 'host', f'save {name}: {written[0]}')

    def load(self, name):
        row = [n for _, n in self.saves()].index(name)
        self.open_saves()
        shown = self.show_row(row)
        self.click(SAVES_PICTURE(shown), 0.5)
        self.click(SAVES_LOAD, 1)
        self.click(SAVES_CONFIRM, 1)
        self.wait_screen({'game'}, 180)

    # --- Traces ------------------------------------------------------------------------------------------------

    def trace(self, params):
        """`original/trace {target, name, abi, args, ret, ids, limit}`: a preset's or a group's name, or an address."""
        if self.ended.is_set():
            raise RuntimeError('the game has ended')
        allowed = {'target', 'name', 'abi', 'args', 'ret', 'ids', 'limit'}
        unknown = set(params or {}) - allowed
        if unknown or 'target' not in (params or {}):
            raise ValueError(f"original/trace takes {', '.join(sorted(allowed))}, target required")
        started = self.agent.trace(params)
        for t in started:
            self.journal.push('INFO', 'host', f"trace {t['name']} at {t['target']}")
        return started

    def untrace(self, params):
        stopped = self.agent.untrace((params or {}).get('name', 'all'))
        for name in stopped:
            self.journal.push('INFO', 'host', f'untrace {name}')
        return stopped

    def command_trace(self, word, rest):
        if word == 'traces':
            running = self.agent.traces()
            presets = self.agent.presets()
            lines = [f"{t['name']} at {t['target']}: {t['calls']} calls" for t in running] or ['no traces']
            lines.append('presets: ' + ', '.join(presets['presets']))
            lines.append('groups: ' + '; '.join(f'{g} ({", ".join(m)})' for g, m in presets['groups'].items()))
            return lines
        if word == 'untrace':
            stopped = self.untrace({'name': rest or 'all'})
            return [f'untraced {", ".join(stopped)}' if stopped else 'nothing to untrace']
        parser = argparse.ArgumentParser(prog='trace', add_help=False, exit_on_error=False)
        parser.add_argument('target')
        parser.add_argument('--name')
        parser.add_argument('--abi', choices=['cdecl', 'stdcall', 'thiscall', 'fastcall'])
        parser.add_argument('--args')
        parser.add_argument('--ret')
        parser.add_argument('--ids')
        parser.add_argument('--limit', type=int)
        try:
            options, unknown = parser.parse_known_args(rest.split())
        except (argparse.ArgumentError, SystemExit) as e:
            raise ValueError(f'trace: {e}') from None
        if unknown:
            raise ValueError(f"trace: unknown {' '.join(unknown)}")
        params = {'target': options.target}
        if options.name:
            params['name'] = options.name
        if options.abi:
            params['abi'] = options.abi
        if options.args:
            params['args'] = options.args.split(',')
        if options.ret:
            params['ret'] = options.ret
        if options.ids:
            params['ids'] = [int(i) for i in options.ids.split(',')]
        if options.limit:
            params['limit'] = options.limit
        return [f"tracing {t['name']} at {t['target']}" for t in self.trace(params)]

    # --- The world ---------------------------------------------------------------------------------------------

    def world(self, name, args):
        if self.ended.is_set():
            raise RuntimeError('the game has ended')
        return self.agent.world(name, args)

    def hero_cell(self):
        hero = self.world('snapshot', {}).get('hero')
        if not hero:
            raise RuntimeError('no hero')
        return hero['cell']

    def target_cell(self, text, relative_to_hero):
        match = CELL_LINE.match(text)
        if not match:
            raise ValueError(f'a cell: X,Y or +DX,+DY, not {text!r}')
        sx, x, sy, y = match.groups()
        x, y = int(x) * (-1 if sx == '-' else 1), int(y) * (-1 if sy == '-' else 1)
        if relative_to_hero and (sx or sy):
            hx, hy = self.hero_cell()
            x, y = hx + x, hy + y
        return x, y

    def command_creatures(self, rest):
        radius = int(rest) if rest else 20
        found = self.world('creatures', {'radius': radius})
        lines = [f'{len(found)} creatures within {radius} cells of the hero']
        for c in found:
            lines.append(f"{c['id']} type {c['type']} cell {c['cell'][0]},{c['cell'][1]}"
                         + (f" floor {c['floor']}" if c['inside'] else '')
                         + f" health {c['health']}/{c['most']} level {c['level']} ai {c['ai'][0]}.{c['ai'][1]}"
                         f" action {c['action'][0]}.{c['action'][1]}"
                         + (f" target {c['target']}" if c['target'] else ''))
        return lines

    def command_cell(self, rest, search=8):
        where, _, floor = rest.partition(' ')
        if floor.strip() and not floor.strip().isdigit():
            raise ValueError('cell X,Y [FLOOR]')
        x, y = self.target_cell(where, relative_to_hero=True)
        if (x, y) == FINAL_CELL:
            raise ValueError('that cell is the one the scripts take for the end of the game')
        placed = self.world('cell', {'x': x, 'y': y, 'floor': int(floor or 0), 'search': search})
        return [f"hero at {placed['cell'][0]},{placed['cell'][1]} (asked {x},{y})"]

    def command_sector(self, rest):
        x, y = self.target_cell(rest, relative_to_hero=False)
        return self.command_cell(f'{x * 64 + 32},{y * 64 + 32}', search=24)

    def command_time(self, rest):
        match = re.fullmatch(r'(\d{1,2}):(\d{2})', rest)
        if not match or int(match.group(1)) > 23 or int(match.group(2)) > 59:
            raise ValueError('time HH:MM')
        clock = self.world('time', {'hours': int(match.group(1)), 'minutes': int(match.group(2))})
        return [f"{clock['time']} {clock['phase']}"]

    def command_day_speed(self, rest):
        words = rest.split()
        if not 1 <= len(words) <= 2:
            raise ValueError('day-speed DAY [NIGHT]')
        day = float(words[0])
        night = float(words[1]) if len(words) > 1 else day
        clock = self.world('day-speed', {'day': day, 'night': night})
        return [f"{clock['time']} {clock['phase']}, rates {clock['rates'][0]:g} by day, {clock['rates'][1]:g} by night"]

    def command_zoom(self, rest):
        zoom = float(rest)
        if not 0.5 <= zoom <= 2:
            raise ValueError('zoom: 0.5 to 2')
        return [f"zoom {self.world('zoom', {'zoom': zoom}):.2f}"]

    def command_point(self, rest):
        x, y = self.target_cell(rest, relative_to_hero=True)
        px, py = self.world('point', {'x': x, 'y': y})
        return [f'{px} {py}']

    def command_go(self, rest):
        x, y = self.target_cell(rest, relative_to_hero=True)
        px, py = self.world('point', {'x': x, 'y': y})
        if not (0 <= px < WIDTH and 0 <= py < GROUND_BOTTOM):
            raise RuntimeError(f'{x},{y} is off the screen ({px},{py}): go nearer first')
        self.agent.input({'click': [px, py]})
        return [f'to {x},{y} by a click at {px},{py}']

    def quit_later(self):
        if not self.ended.wait(15):
            self.journal.push('WARN', 'host', 'the game is still running 15 s after its window was closed: ended')
            self.kill(1)

    METHODS = {'opensacred/state': lambda self, params: self.state(),
               'opensacred/screenshot': screenshot, 'opensacred/logs': logs, 'opensacred/exec': exec,
               'opensacred/input': input, 'original/trace': trace, 'original/untrace': untrace}


WORLD_COMMANDS = {
    'creatures': Host.command_creatures, 'cell': Host.command_cell, 'sector': Host.command_sector,
    'time': Host.command_time, 'day-speed': Host.command_day_speed, 'zoom': Host.command_zoom,
    'point': Host.command_point, 'go': Host.command_go,
}


def serve(host, port):
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            try:
                request = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
            except ValueError as e:
                return self.answer({'jsonrpc': '2.0', 'id': None, 'error': {'code': -32700, 'message': str(e)}})
            method = Host.METHODS.get(request.get('method'))
            ident = request.get('id')
            if method is None:
                return self.answer({'jsonrpc': '2.0', 'id': ident,
                                    'error': {'code': -32601, 'message': f"no method {request.get('method')}"}})
            try:
                result = method(host, request.get('params'))
                self.answer({'jsonrpc': '2.0', 'id': ident, 'result': result})
            except (KeyError, TypeError, ValueError) as e:
                self.answer({'jsonrpc': '2.0', 'id': ident, 'error': {'code': -32602, 'message': repr(e)}})
            except Exception as e:
                # An error of the agent carries its stack after its message.
                message = str(e).splitlines()[0] if str(e) else repr(e)
                self.answer({'jsonrpc': '2.0', 'id': ident, 'error': {'code': -32603, 'message': message}})

        def answer(self, body):
            data = json.dumps(body).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, format, *args):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', port), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--game-dir', default=os.path.join(ROOT, 'OriginalGame'),
                        help='the GOG installation (OriginalGame/ by default)')
    parser.add_argument('--port', type=int, default=15703, help='the port of the remote control (15703 by default)')
    parser.add_argument('--place', choices=['offscreen', 'behind'], default='offscreen',
                        help='the window to the right of every monitor (by default) or behind the other windows')
    parser.add_argument('--sound', action='store_true', help="keep Settings.cfg's volumes, audible")
    parser.add_argument('--muted', action='store_true',
                        help="the game's volumes 64, so that it plays its sounds for the sound traces, while the agent "
                             "silences mss32.dll's output")
    parser.add_argument('--log-file', help='also append the records to this file')
    parser.add_argument('--keep-captures', action='store_true',
                        help="keep the game's Capture/shotNNNN.tga and .jpg of each screenshot")
    parser.add_argument('--no-game-log', action='store_true', help="leave out the game's own log (logPrintf)")
    parser.add_argument('--agent', default=AGENT, help='the agent (tools/original/agent.js by default)')
    parser.add_argument('--arg', action='append', default=[],
                        help="a word of Sacred.exe's command line (repeatable), as `--arg tabblocker`")
    args = parser.parse_args()
    journal = Journal(args.log_file)
    host = Host(args, journal)
    try:
        server = serve(host, args.port)
    except OSError as e:
        raise SystemExit(f'error: cannot listen on 127.0.0.1:{args.port}: {e}')

    def stop(signum, frame):
        journal.push('INFO', 'host', f'signal {signum}: the game is ended')
        host.kill(1)
        os._exit(1)

    for name in ('SIGINT', 'SIGTERM', 'SIGBREAK'):
        if hasattr(signal, name):
            signal.signal(getattr(signal, name), stop)
    host.start()
    journal.push('INFO', 'host', f'remote control on 127.0.0.1:{args.port}')
    while not host.ended.wait(0.5):
        pass
    server.shutdown()
    sys.exit(host.exit_code if host.exit_code is not None else 1)


if __name__ == '__main__':
    main()
