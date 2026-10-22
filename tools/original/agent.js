// The agent of the original's driver (#448, docs/03 §3.8): Frida loads it into the developer's own running
// Sacred.exe (GOG 2.28, 32-bit), where it guards the maintainer's desktop, keeps the game drawing without the
// focus, counts its frames and takes its screenshots. `tools/original/host.py` loads it and talks to it through
// `rpc.exports` and `send`.
//
// It carries only addresses, offsets and the original's names (docs/re/window.md), and changes nothing on disk:
// it hooks functions of the running process alone.

'use strict';

// Sacred.exe's own functions and data (docs/re/window.md); the GOG build loads at its preferred base 0x400000.
const SACRED_BASE = ptr('0x400000');
const ADDRESSES = {
  WinMain: ptr('0x817b30'),
  sacredWndProc: ptr('0x812bf0'),     // the window procedure of the class "Sacred", the menus'
  playWndProc: ptr('0x811a20'),       // the window procedure while a game is played, from the message 0x8002 on
  engine: ptr('0xad6ef8'),            // cEngine, made when a game starts (0x60d6c0); +0x54 its state's flags
  timer: ptr('0xcd7a4c'),             // the game's timer (0x6413e0); its first byte: the game's time stands
  dxDriver7_flip: ptr('0x645e40'),    // dxDriver7::flip, once a frame, ECX the driver
  logPrintf: ptr('0x66ef40'),         // the game's log, printf-like, cdecl
  engineFlags: ptr('0x182ee38'),      // a flags word; bit 0x20000: the application is inactive
  screenshotName: ptr('0xcdc918'),    // the name captureScreenshot formats, ".\Capture\shotNNNN.tga"
};
const DRIVER_SHOTS = 0xa8;            // dxDriver7: screenshots asked for, taken at the next flip
const DRIVER_FRAMES_CAPTURED = 0xac;  // dxDriver7: frames captured so far, 0 for a single shot
const INACTIVE = 0x20000;
const ENGINE_STATE = 0x54;
const ENGINE_LOADING = 0x10000;         // the render thread draws the loading screen

// Window messages.
const WM = {
  NULL: 0x0000, ACTIVATE: 0x0006, KILLFOCUS: 0x0008, CLOSE: 0x0010, ACTIVATEAPP: 0x001c,
  SETCURSOR: 0x0020, MOUSEACTIVATE: 0x0021, NCACTIVATE: 0x0086,
};
const MA_NOACTIVATEANDEAT = 4;
// The agent's own message: its wParam names the input the window procedure turns it into.
const AGENT_MESSAGE = 0x83a5;
const SW = { SHOWNORMAL: 1, SHOWMAXIMIZED: 3, SHOWNOACTIVATE: 4, SHOW: 5, SHOWNA: 8, RESTORE: 9, SHOWDEFAULT: 10 };
const SWP = { NOZORDER: 0x4, NOACTIVATE: 0x10, NOMOVE: 0x2 };
const HWND_TOPMOST = -1;
const HWND_BOTTOM = 1;
const WS_VISIBLE = 0x10000000;
const WS_EX_TOPMOST = 0x8;
const WS_EX_NOACTIVATE = 0x08000000;
const WH_KEYBOARD_LL = 13;
const WH_MOUSE_LL = 14;
const DDSCL_FULLSCREEN = 0x1;
const DDSCL_EXCLUSIVE = 0x10;

// The mouse and keyboard messages the window procedure drops unless the agent sent them: the keys and characters,
// the client's and the frame's mouse, hover and leave, raw input and the input method's.
function isInput(message) {
  return (message >= 0x100 && message <= 0x10f) || (message >= 0x200 && message <= 0x20e) ||
    (message >= 0xa0 && message <= 0xad) || (message >= 0x2a0 && message <= 0x2a3) ||
    (message >= 0x281 && message <= 0x291) || message === 0xff || message === WM.SETCURSOR;
}

const config = { place: 'offscreen', heartbeatTimeout: 5000 };
const guards = {};
const state = {
  window: null,          // the game's main window
  place: null,           // where the agent puts it: {x, y, behind}
  frames: 0,
  flipError: 0,          // the last flip's failure, 0 when it succeeded
  flipTimes: [],         // the last flips' times, ms
  shotsWanted: 0,
  lastBeat: Date.now(),
  ending: false,
  exiting: false,        // the game ends itself: WinMain returned, or the C runtime's exit was called
};
let refusals = 0;
let fakeHooks = 0x0bad0000;
const ownHooks = new Set();
let logSuppressed = 0;
let logWindowStart = 0;
let logInWindow = 0;

function log(level, message) {
  send({ type: 'log', level, message });
}

function refuse(what) {
  refusals += 1;
  log('WARN', `refused: ${what}`);
}

// --- System functions, called past their guards -------------------------------------------------------------------

const user32 = Module.load('user32.dll');
const kernel32 = Module.load('kernel32.dll');
const modules = { 'user32.dll': user32 };

function exportOf(module, name) {
  return modules[module].getExportByName(name);
}
const native = {
  terminateProcess: new NativeFunction(kernel32.getExportByName('TerminateProcess'), 'int', ['pointer', 'uint'],
    'stdcall'),
  queryPerformanceCounter: new NativeFunction(kernel32.getExportByName('QueryPerformanceCounter'), 'int',
    ['pointer'], 'stdcall'),
  queryPerformanceFrequency: new NativeFunction(kernel32.getExportByName('QueryPerformanceFrequency'), 'int',
    ['pointer'], 'stdcall'),
  getSystemMetrics: new NativeFunction(user32.getExportByName('GetSystemMetrics'), 'int', ['int'], 'stdcall'),
  clientToScreen: new NativeFunction(user32.getExportByName('ClientToScreen'), 'int', ['pointer', 'pointer'],
    'stdcall'),
  getWindowRect: new NativeFunction(user32.getExportByName('GetWindowRect'), 'int', ['pointer', 'pointer'],
    'stdcall'),
  postMessageA: new NativeFunction(user32.getExportByName('PostMessageA'), 'int',
    ['pointer', 'uint', 'pointer', 'pointer'], 'stdcall'),
  setWindowPos: new NativeFunction(user32.getExportByName('SetWindowPos'), 'int',
    ['pointer', 'pointer', 'int', 'int', 'int', 'int', 'uint'], 'stdcall'),
  isWindow: new NativeFunction(user32.getExportByName('IsWindow'), 'int', ['pointer'], 'stdcall'),
};

const counter = Memory.alloc(8);
native.queryPerformanceFrequency(counter);
const ticksPerMs = counter.readU64().toNumber() / 1000;

function now() {
  native.queryPerformanceCounter(counter);
  return counter.readU64().toNumber() / ticksPerMs;
}

function end(code, why) {
  if (state.ending) {
    return;
  }
  state.ending = true;
  send({ type: 'fatal', message: why });
  native.terminateProcess(ptr(-1), code);
}

function isGameWindow(hwnd) {
  return state.window !== null && hwnd.equals(state.window);
}

// Where the window goes: to the right of every monitor, or behind the other windows where the system put it.
function placement() {
  if (config.place === 'behind') {
    return { x: 0, y: 0, behind: true };
  }
  const left = native.getSystemMetrics(76);   // SM_XVIRTUALSCREEN
  const top = native.getSystemMetrics(77);    // SM_YVIRTUALSCREEN
  const width = native.getSystemMetrics(78);  // SM_CXVIRTUALSCREEN
  return { x: left + width + 64, y: top, behind: false };
}

// --- Guards ---------------------------------------------------------------------------------------------------------

function replace(module, name, ret, args, implementation) {
  const target = exportOf(module, name);
  const original = new NativeFunction(target, ret, args, 'stdcall');
  Interceptor.replace(target, new NativeCallback(function (...values) {
    return implementation(original, ...values);
  }, ret, args, 'stdcall'));
  guards[name] = true;
}

function attach(module, name, callbacks) {
  Interceptor.attach(exportOf(module, name), callbacks);
  guards[name] = true;
}

function installHookGuards() {
  // The "tabblocker" (docs/09 §2.1, docs/re/window.md): a low-level keyboard or mouse hook installs nothing.
  for (const name of ['SetWindowsHookExA', 'SetWindowsHookExW']) {
    replace('user32.dll', name, 'pointer', ['int', 'pointer', 'pointer', 'uint'], (original, id, proc, module, thread) => {
      if (id === WH_KEYBOARD_LL || id === WH_MOUSE_LL) {
        refuse(`${name}(${id === WH_KEYBOARD_LL ? 'WH_KEYBOARD_LL' : 'WH_MOUSE_LL'})`);
        fakeHooks += 1;
        ownHooks.add(fakeHooks);
        return ptr(fakeHooks);
      }
      return original(id, proc, module, thread);
    });
  }
  replace('user32.dll', 'UnhookWindowsHookEx', 'int', ['pointer'], (original, hook) => {
    if (ownHooks.delete(hook.toUInt32())) {
      return 1;
    }
    return original(hook);
  });
}

function installFocusGuards() {
  // The calls that would take the focus or raise a window do nothing.
  replace('user32.dll', 'SetForegroundWindow', 'int', ['pointer'], () => 1);
  replace('user32.dll', 'BringWindowToTop', 'int', ['pointer'], () => 1);
  replace('user32.dll', 'SetActiveWindow', 'pointer', ['pointer'], () => state.window ?? ptr(0));
  replace('user32.dll', 'SetFocus', 'pointer', ['pointer'], () => state.window ?? ptr(0));
  // The focus queries answer with the game's window, so that sdk_FORCE_FG_WAIT ends at once (docs/re/window.md).
  for (const name of ['GetForegroundWindow', 'GetActiveWindow', 'GetFocus']) {
    replace('user32.dll', name, 'pointer', [], (original) => state.window ?? original());
  }
  // A message box would take the focus and stop its thread: it is logged and answered OK.
  for (const name of ['MessageBoxA', 'MessageBoxW']) {
    const read = (text) => (text.isNull() ? '' : name.endsWith('W') ? text.readUtf16String() : text.readCString());
    replace('user32.dll', name, 'int', ['pointer', 'pointer', 'pointer', 'uint'], (original, owner, text, caption) => {
      refuse(`${name} "${read(caption)}": ${read(text)}`);
      return 1;  // IDOK
    });
  }
}

function installWindowGuards() {
  attach('user32.dll', 'CreateWindowExA', {
    onEnter(args) {
      const parent = args[8];
      const style = args[3].toUInt32();
      // A top-level window (not a child, WS_CHILD 0x40000000) opens without activation, never topmost, where the
      // agent puts it.
      this.topLevel = parent.isNull() || (style & 0x40000000) === 0;
      if (!this.topLevel) {
        return;
      }
      const exStyle = (args[0].toUInt32() | WS_EX_NOACTIVATE) & ~WS_EX_TOPMOST;
      args[0] = ptr(exStyle >>> 0);
      state.place = placement();
      args[4] = ptr(state.place.x);
      args[5] = ptr(state.place.y);
      this.className = args[1].compare(ptr(0x10000)) < 0 ? `#${args[1].toUInt32()}` : args[1].readCString();
      // The game's window opens hidden: its ShowWindow, right after, shows it without activation.
      if (this.className === 'Sacred') {
        args[3] = ptr((style & ~WS_VISIBLE) >>> 0);
      }
    },
    onLeave(result) {
      if (this.topLevel && this.className === 'Sacred' && !result.isNull()) {
        // A copy: the return value's object lives as long as the call.
        state.window = ptr(result.toString());
        log('INFO', `window ${result} of the class Sacred at ${state.place.x},${state.place.y}`);
      }
    },
  });
  attach('user32.dll', 'ShowWindow', {
    onEnter(args) {
      this.window = false;
      if (!isGameWindow(args[0])) {
        return;
      }
      const command = args[1].toInt32();
      if (command === SW.SHOW) {
        args[1] = ptr(SW.SHOWNA);
      } else if ([SW.SHOWNORMAL, SW.SHOWMAXIMIZED, SW.RESTORE, SW.SHOWDEFAULT].includes(command)) {
        args[1] = ptr(SW.SHOWNOACTIVATE);
      }
      this.window = isGameWindow(args[0]);
    },
    onLeave() {
      // Behind the other windows: a window shown goes to the top of the order, without activation or not.
      if (this.window && state.place?.behind) {
        native.setWindowPos(state.window, ptr(HWND_BOTTOM), 0, 0, 0, 0, 0x1 | SWP.NOMOVE | SWP.NOACTIVATE);
      }
    },
  });
  attach('user32.dll', 'SetWindowPos', {
    onEnter(args) {
      if (!isGameWindow(args[0])) {
        return;
      }
      let flags = args[6].toUInt32() | SWP.NOACTIVATE;
      const after = args[1].toInt32();
      if (state.place?.behind) {
        args[1] = ptr(HWND_BOTTOM);
        flags &= ~SWP.NOZORDER;
      } else if (after === HWND_TOPMOST || after === 0) {
        flags |= SWP.NOZORDER;
      }
      if ((flags & SWP.NOMOVE) === 0 && state.place !== null) {
        args[2] = ptr(state.place.x);
        args[3] = ptr(state.place.y);
      }
      args[6] = ptr(flags >>> 0);
    },
  });
  attach('user32.dll', 'MoveWindow', {
    onEnter(args) {
      if (isGameWindow(args[0]) && state.place !== null) {
        args[1] = ptr(state.place.x);
        args[2] = ptr(state.place.y);
      }
    },
  });
}

function installCursorAndDisplayGuards() {
  replace('user32.dll', 'ClipCursor', 'int', ['pointer'], () => 1);
  replace('user32.dll', 'SetCursorPos', 'int', ['int', 'int'], () => 1);
  for (const name of ['ChangeDisplaySettingsA', 'ChangeDisplaySettingsW']) {
    replace('user32.dll', name, 'int', ['pointer', 'uint'], (original, mode) => {
      if (!mode.isNull()) {
        refuse(`${name}: a display mode`);
      }
      return 0;  // DISP_CHANGE_SUCCESSFUL
    });
  }
  for (const name of ['ChangeDisplaySettingsExA', 'ChangeDisplaySettingsExW']) {
    replace('user32.dll', name, 'int', ['pointer', 'pointer', 'pointer', 'uint', 'pointer'], (original, device, mode) => {
      if (!mode.isNull()) {
        refuse(`${name}: a display mode`);
      }
      return 0;
    });
  }
  // Input made for the whole system never leaves the process.
  replace('user32.dll', 'SendInput', 'uint', ['uint', 'pointer', 'int'], () => {
    refuse('SendInput');
    return 0;
  });
  for (const name of ['keybd_event', 'mouse_event']) {
    const args = name === 'keybd_event' ? ['uint', 'uint', 'uint', 'pointer'] : ['uint', 'uint', 'uint', 'uint', 'pointer'];
    replace('user32.dll', name, 'void', args, () => {
      refuse(name);
    });
  }
}

// The maintainer's own keys and cursor never reach the game: the queries answer from the agent's virtual keyboard and
// cursor, all keys up and the cursor at the window's corner until the agent's input moves them (#450).
const virtualInput = { cursor: { x: 0, y: 0 }, keys: new Uint8Array(256), pressed: new Uint8Array(256) };

function installInputGuards() {
  replace('user32.dll', 'GetAsyncKeyState', 'int16', ['int'], (original, key) => {
    const down = (virtualInput.keys[key & 0xff] & 0x80) !== 0;
    const pressed = virtualInput.pressed[key & 0xff];
    virtualInput.pressed[key & 0xff] = 0;
    return (down ? -32768 : 0) | pressed;
  });
  replace('user32.dll', 'GetKeyState', 'int16', ['int'], (original, key) => {
    const value = virtualInput.keys[key & 0xff];
    return ((value & 0x80) !== 0 ? -32768 : 0) | (value & 1);
  });
  replace('user32.dll', 'GetKeyboardState', 'int', ['pointer'], (original, buffer) => {
    buffer.writeByteArray(virtualInput.keys.buffer);
    return 1;
  });
  const point = Memory.alloc(8);
  replace('user32.dll', 'GetCursorPos', 'int', ['pointer'], (original, out) => {
    point.writeS32(virtualInput.cursor.x);
    point.add(4).writeS32(virtualInput.cursor.y);
    if (state.window !== null) {
      native.clientToScreen(state.window, point);
    }
    out.writeS32(point.readS32());
    out.add(4).writeS32(point.add(4).readS32());
    return 1;
  });
}

// DirectDraw's exclusive full-screen level, which would change the display mode, ends the run; so does a display
// mode set through it. IDirectDraw and IDirectDraw7 share the vtable's first entries: SetCooperativeLevel is the
// 21st, SetDisplayMode the 22nd.
function installDirectDrawGuards() {
  const ddraw = Module.load('ddraw.dll');
  const hooked = new Set();
  function guardInterface(object) {
    const vtable = object.readPointer();
    const setCooperativeLevel = vtable.add(20 * 4).readPointer();
    const setDisplayMode = vtable.add(21 * 4).readPointer();
    if (!hooked.has(setCooperativeLevel.toString())) {
      hooked.add(setCooperativeLevel.toString());
      Interceptor.attach(setCooperativeLevel, {
        onEnter(args) {
          const flags = args[2].toUInt32();
          if ((flags & (DDSCL_EXCLUSIVE | DDSCL_FULLSCREEN)) !== 0) {
            end(3, `DirectDraw's exclusive full-screen level (0x${flags.toString(16)}): FULLSCREEN is not 0?`);
          }
        },
      });
    }
    if (!hooked.has(setDisplayMode.toString())) {
      hooked.add(setDisplayMode.toString());
      Interceptor.attach(setDisplayMode, {
        onEnter(args) {
          end(3, `DirectDraw's SetDisplayMode ${args[1].toUInt32()}x${args[2].toUInt32()}`);
        },
      });
    }
  }
  for (const name of ['DirectDrawCreate', 'DirectDrawCreateEx']) {
    Interceptor.attach(ddraw.getExportByName(name), {
      onEnter(args) {
        this.out = args[1];
      },
      onLeave(result) {
        if (result.toInt32() >= 0 && !this.out.isNull() && !this.out.readPointer().isNull()) {
          guardInterface(this.out.readPointer());
        }
      },
    });
    guards[name] = true;
  }
}

// --- The agent's input ---------------------------------------------------------------------------------------------

// Virtual-key codes by the names of opensacred-game's `opensacred/input` (its keys by their place), and the
// character each types in an English layout.
const KEYS = {
  Escape: [0x1b, '\x1b'], Enter: [0x0d, '\r'], Tab: [0x09, '\t'], Space: [0x20, ' '], Backspace: [0x08, '\b'],
  Delete: [0x2e], Home: [0x24], End: [0x23], PageUp: [0x21], PageDown: [0x22], Insert: [0x2d],
  ArrowUp: [0x26], ArrowDown: [0x28], ArrowLeft: [0x25], ArrowRight: [0x27], Backquote: [0xc0, '`'],
  Control: [0x11], Ctrl: [0x11], Shift: [0x10], Alt: [0x12], PrintScreen: [0x2c], Pause: [0x13],
  NumpadAdd: [0x6b, '+'], NumpadSubtract: [0x6d, '-'], Minus: [0xbd, '-'], Equal: [0xbb, '='],
};
for (let n = 1; n <= 12; n += 1) {
  KEYS[`F${n}`] = [0x6f + n];
}
for (let c = 0; c < 26; c += 1) {
  const letter = String.fromCharCode(65 + c);
  KEYS[letter] = [65 + c, letter.toLowerCase()];
  KEYS[`Key${letter}`] = KEYS[letter];
}
for (let d = 0; d < 10; d += 1) {
  KEYS[String(d)] = [48 + d, String(d)];
  KEYS[`Digit${d}`] = KEYS[String(d)];
  KEYS[`Numpad${d}`] = [0x60 + d, String(d)];
}
const MAPVK_VK_TO_VSC = 0;
const mapVirtualKey = new NativeFunction(user32.getExportByName('MapVirtualKeyA'), 'uint', ['uint', 'uint'], 'stdcall');

const BUTTONS = {
  left: { down: 0x201, up: 0x202, key: 0x01, flag: 0x0001 },
  right: { down: 0x204, up: 0x205, key: 0x02, flag: 0x0002 },
  middle: { down: 0x207, up: 0x208, key: 0x04, flag: 0x0010 },
};

const input = {
  pending: new Map(),  // the agent's messages posted, by their number
  next: 1,
  schedule: [],        // {frame, deadline, run}: an input waiting for its frame, or its time if no frame comes
};

// Posts one message of the agent's to the game's window; the window procedure turns it into `message`.
function post(message, wParam, lParam) {
  if (state.window === null) {
    throw new Error('no window yet');
  }
  const id = input.next++;
  input.pending.set(id, { message, wParam: wParam >>> 0, lParam: lParam >>> 0 });
  native.postMessageA(state.window, AGENT_MESSAGE, ptr(id), ptr(0));
}

function later(frames, run) {
  input.schedule.push({ frame: state.frames + frames, deadline: Date.now() + frames * 50 + 20, run });
}

function runSchedule() {
  if (input.schedule.length === 0) {
    return;
  }
  const time = Date.now();
  const due = input.schedule.filter((a) => state.frames >= a.frame || time >= a.deadline);
  input.schedule = input.schedule.filter((a) => !due.includes(a));
  for (const action of due) {
    action.run();
  }
}

// The state that a message of the agent's sets as the window procedure takes it, as the system's would.
function applyInput(entry) {
  const { message, wParam, lParam } = entry;
  if (message === 0x100) {        // WM_KEYDOWN
    if ((virtualInput.keys[wParam & 0xff] & 0x80) === 0) {
      virtualInput.keys[wParam & 0xff] ^= 1;  // the toggle, as the system keeps it
    }
    virtualInput.keys[wParam & 0xff] |= 0x80;
    virtualInput.pressed[wParam & 0xff] = 1;
  } else if (message === 0x101) { // WM_KEYUP
    virtualInput.keys[wParam & 0xff] &= ~0x80;
  } else if (message >= 0x200 && message <= 0x209) {
    virtualInput.cursor = { x: lParam << 16 >> 16, y: lParam >> 16 };
    for (const button of Object.values(BUTTONS)) {
      if (message === button.down) {
        virtualInput.keys[button.key] |= 0x80;
        virtualInput.pressed[button.key] = 1;
      } else if (message === button.up) {
        virtualInput.keys[button.key] &= ~0x80;
      }
    }
  }
}

function keyLParam(vk, up) {
  const scan = mapVirtualKey(vk, MAPVK_VK_TO_VSC) & 0xff;
  return (1 | (scan << 16) | (up ? 0xc0000000 : 0)) >>> 0;
}

function buttonsHeld() {
  let flags = 0;
  for (const button of Object.values(BUTTONS)) {
    if ((virtualInput.keys[button.key] & 0x80) !== 0) {
      flags |= button.flag;
    }
  }
  if ((virtualInput.keys[0x10] & 0x80) !== 0) {
    flags |= 0x4;  // MK_SHIFT
  }
  if ((virtualInput.keys[0x11] & 0x80) !== 0) {
    flags |= 0x8;  // MK_CONTROL
  }
  return flags;
}

function point(x, y) {
  const cx = Math.round(x);
  const cy = Math.round(y);
  if (cx < 0 || cy < 0 || cx >= 1024 || cy >= 768) {
    throw new Error(`${x},${y} is outside the window's 1024 x 768`);
  }
  return [cx, cy, ((cy & 0xffff) << 16 | (cx & 0xffff)) >>> 0];
}

// The player's input of `opensacred/input`, as opensacred-game takes it: a key (or keys held together), text, a
// move, a click held `hold` frames, the wheel by lines.
function sendInput(params) {
  if (params.key !== undefined) {
    const chord = params.key.split('+').map((name) => {
      const key = KEYS[name];
      if (key === undefined) {
        throw new Error(`no key ${name}`);
      }
      return key;
    });
    const typing = chord.slice(0, -1).every(([vk]) => vk === 0x10);
    for (const [vk, character] of chord) {
      post(0x100, vk, keyLParam(vk, false));
      if (typing && character !== undefined) {
        const shifted = chord.length > 1 && character.length === 1 ? character.toUpperCase() : character;
        post(0x102, shifted.charCodeAt(0), keyLParam(vk, false));
      }
    }
    later(Math.max(1, params.hold ?? 2), () => {
      for (const [vk] of chord.slice().reverse()) {
        post(0x101, vk, keyLParam(vk, true));
      }
    });
  } else if (params.text !== undefined) {
    for (const character of params.text) {
      const code = character.charCodeAt(0);
      if (code > 0xff) {
        throw new Error(`the character ${character} is not in the game's code page`);
      }
      post(0x102, code, 1);
    }
  } else if (params.move !== undefined) {
    const [, , lParam] = point(params.move[0], params.move[1]);
    post(0x200, buttonsHeld(), lParam);
  } else if (params.click !== undefined) {
    const button = BUTTONS[params.button ?? 'left'];
    if (button === undefined) {
      throw new Error(`no button ${params.button}`);
    }
    const [, , lParam] = point(params.click[0], params.click[1]);
    post(0x200, buttonsHeld(), lParam);
    later(1, () => {
      post(button.down, buttonsHeld() | button.flag, lParam);
      later(Math.max(1, params.hold ?? 2), () => post(button.up, buttonsHeld() & ~button.flag, lParam));
    });
  } else if (params.wheel !== undefined) {
    const delta = Math.round(params.wheel * 120);
    const screen = Memory.alloc(8);
    screen.writeS32(virtualInput.cursor.x);
    screen.add(4).writeS32(virtualInput.cursor.y);
    native.clientToScreen(state.window, screen);
    post(0x20a, ((delta & 0xffff) << 16) | buttonsHeld(),
      ((screen.add(4).readS32() & 0xffff) << 16) | (screen.readS32() & 0xffff));
  } else {
    throw new Error('no input: key, text, move, click or wheel');
  }
}

// The program's screen: the menus before a game (no engine yet), the loading screen, the game, or the game's time
// standing (the original's menu over a game).
function screen() {
  const engine = ADDRESSES.engine.readPointer();
  if (engine.isNull()) {
    return 'menu';
  }
  if ((engine.add(ENGINE_STATE).readU32() & ENGINE_LOADING) !== 0) {
    return 'loading';
  }
  const timer = ADDRESSES.timer.readPointer();
  if (!timer.isNull() && timer.readU8() !== 0) {
    return 'paused';
  }
  return 'game';
}

// --- The world -----------------------------------------------------------------------------------------------------

// The world's objects, the hero, the clock and the view (docs/re/objects.md).
const WORLD = {
  objects: ptr('0xad5c40'),          // sdk_OM: the object manager; its +4 and +8 the vector of the objects by id
  context: ptr('0x182ebe8'),         // sdk_CTX: the game's context; its +0x14 the local player's id
  clock: ptr('0xaa4540'),            // the clock (0x417560)
  creature: ptr('0x8eb660'),         // cCreature's RTTI type descriptor
  teleport: ptr('0x54d9d0'),         // sdk_ENGINE_TP: thiscall (creature, cell x, cell y, floor, effect), 1 if placed
  setZoom: ptr('0x628130'),          // cWorldView_setZoom: thiscall (view, zoom)
  tick: ptr('0x60dd20'),             // cEngine_handleZoomInput: once a frame on the render thread, the engine in ECX
};
const CELL = 53.66563;               // a cell's side in world units
const CREATURE = {
  id: 0x0c, type: 0x10, x: 0x1c, y: 0x20, floor: 0x24, layer: 0x2c, ai: 0xfc, aiSub: 0xfe, target: 0x104,
  action: 0x150, actionSub: 0x152, gait: 0x1f8, experience: 0x3b4, level: 0x3fe, most: 0x4d4, health: 0x4d8,
};
const CLOCK = { dayRate: 0x10, nightRate: 0x14, hours: 0x20, minutes: 0x22, seconds: 0x24, fraction: 0x28 };
const VIEW_ZOOM = 0xb60;
const VIEW_SCREEN_TO_WORLD = 6;      // the view's virtual function: thiscall (view, x, y, position*)

const teleport = new NativeFunction(WORLD.teleport, 'uint32', ['pointer', 'int', 'int', 'int', 'int'], 'thiscall');
const setZoom = new NativeFunction(WORLD.setZoom, 'uint32', ['pointer', 'float'], 'thiscall');

const world = {
  jobs: [],        // {run, resolve, reject, deadline}: run on the render thread's next frame
  engine: null,    // the engine of the last tick
  creatureVtables: new Map(),
};

function objectCount() {
  const manager = WORLD.objects.readPointer();
  if (manager.isNull()) {
    return 0;
  }
  return manager.add(8).readPointer().sub(manager.add(4).readPointer()).toInt32() / 4;
}

function objectById(id) {
  const manager = WORLD.objects.readPointer();
  if (manager.isNull() || id <= 0 || id >= objectCount()) {
    return null;
  }
  const object = manager.add(4).readPointer().add(id * 4).readPointer();
  return object.isNull() ? null : object;
}

// A creature is an object whose class derives from cCreature, by the class hierarchy of its vtable's RTTI.
function isCreature(object) {
  const vtable = object.readPointer();
  const key = vtable.toString();
  let known = world.creatureVtables.get(key);
  if (known === undefined) {
    known = false;
    try {
      const locator = vtable.sub(4).readPointer();
      const hierarchy = locator.add(0x10).readPointer();
      const bases = hierarchy.add(8).readU32();
      const array = hierarchy.add(0xc).readPointer();
      for (let i = 0; i < bases && i < 32; i += 1) {
        if (array.add(4 * i).readPointer().readPointer().equals(WORLD.creature)) {
          known = true;
          break;
        }
      }
    } catch (e) {
      known = false;
    }
    world.creatureVtables.set(key, known);
  }
  return known;
}

function hero() {
  const context = WORLD.context.readPointer();
  return context.isNull() ? null : objectById(context.add(0x14).readU32());
}

function cellOf(object) {
  return [Math.floor(object.add(CREATURE.x).readS32() / CELL), Math.floor(object.add(CREATURE.y).readS32() / CELL)];
}

function readCreature(object) {
  const target = object.add(CREATURE.target).readU32();
  return {
    id: object.add(CREATURE.id).readU32(),
    type: object.add(CREATURE.type).readU32(),
    cell: cellOf(object),
    floor: object.add(CREATURE.floor).readU8(),
    inside: object.add(CREATURE.layer).readU32() !== 0,
    health: object.add(CREATURE.health).readS32(),
    most: object.add(CREATURE.most).readS32(),
    level: object.add(CREATURE.level).readU16(),
    experience: object.add(CREATURE.experience).readU32(),
    ai: [object.add(CREATURE.ai).readU16(), object.add(CREATURE.aiSub).readU16()],
    action: [object.add(CREATURE.action).readU16(), object.add(CREATURE.actionSub).readU16()],
    gait: object.add(CREATURE.gait).readU8(),
    target: target > 0 && target < objectCount() && objectById(target) !== null ? target : null,
  };
}

function creatures() {
  const found = [];
  const count = objectCount();
  for (let id = 1; id < count; id += 1) {
    const object = objectById(id);
    if (object !== null && isCreature(object)) {
      found.push(object);
    }
  }
  return found;
}

function view(engine) {
  return engine.add(8 + engine.add(0x48).readU16() * 4).readPointer();
}

function phase(minutes) {
  // docs/re/daynight.md, "The phases"
  if (minutes > 320 && minutes < 1220) {
    return 'day';
  }
  if (minutes >= 1220 && minutes <= 1340) {
    return 'dusk';
  }
  if (minutes >= 200 && minutes <= 320) {
    return 'dawn';
  }
  return 'night';
}

function readClock() {
  const clock = WORLD.clock.readPointer();
  if (clock.isNull()) {
    return null;
  }
  const hours = clock.add(CLOCK.hours).readU16();
  const minutes = clock.add(CLOCK.minutes).readU16();
  return {
    time: `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`,
    phase: phase(hours * 60 + minutes),
    rates: [clock.add(CLOCK.dayRate).readFloat(), clock.add(CLOCK.nightRate).readFloat()],
  };
}

// The window's pixel of a world point: the view's own screen-to-world function is affine on the ground's plane, so
// three of its answers give its inverse. Heights are not taken into account; the game's pick of a click is.
function pixelOf(engine, worldX, worldY) {
  const v = view(engine);
  const toWorld = new NativeFunction(v.readPointer().add(VIEW_SCREEN_TO_WORLD * 4).readPointer(), 'void',
    ['pointer', 'int', 'int', 'pointer'], 'thiscall');
  const position = Memory.alloc(16);
  const at = (px, py) => {
    position.writeByteArray(new Array(16).fill(0));
    toWorld(v, px, py, position);
    return [position.add(4).readS32(), position.add(8).readS32()];
  };
  const [x0, y0] = at(512, 384);
  const [xa, ya] = at(612, 384);
  const [xb, yb] = at(512, 484);
  const a = (xa - x0) / 100, c = (ya - y0) / 100, b = (xb - x0) / 100, d = (yb - y0) / 100;
  const det = a * d - b * c;
  if (Math.abs(det) < 1e-9) {
    throw new Error('the view gives no projection');
  }
  const dx = worldX - x0, dy = worldY - y0;
  return [Math.round(512 + (d * dx - b * dy) / det), Math.round(384 + (-c * dx + a * dy) / det)];
}

function snapshot(engine) {
  const h = hero();
  const all = creatures();
  const out = { objects: objectCount(), creatures: all.length, clock: readClock(), zoom: view(engine).add(VIEW_ZOOM).readFloat() };
  if (h !== null) {
    const me = readCreature(h);
    out.hero = me;
    let nearest = null;
    let best = Infinity;
    for (const object of all) {
      if (object.equals(h)) {
        continue;
      }
      const [x, y] = cellOf(object);
      const distance = Math.max(Math.abs(x - me.cell[0]), Math.abs(y - me.cell[1]));
      if (distance < best && object.add(CREATURE.health).readS32() > 0) {
        best = distance;
        nearest = object;
      }
    }
    out.nearest = nearest === null ? null : { ...readCreature(nearest), distance: best };
  }
  return out;
}

const COMMANDS = {
  snapshot: (engine) => snapshot(engine),
  creatures: (engine, { radius }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const [hx, hy] = cellOf(h);
    return creatures().filter((o) => !o.equals(h)).map(readCreature)
      .filter((c) => Math.max(Math.abs(c.cell[0] - hx), Math.abs(c.cell[1] - hy)) <= radius)
      .sort((p, q) => Math.hypot(p.cell[0] - hx, p.cell[1] - hy) - Math.hypot(q.cell[0] - hx, q.cell[1] - hy));
  },
  // The hero onto a cell as the scripts' Teleport puts a player (docs/re/objects.md), or onto the first cell it may
  // stand on in rings about it.
  cell: (engine, { x, y, floor, search }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    for (let r = 0; r <= search; r += 1) {
      for (let dx = -r; dx <= r; dx += 1) {
        for (let dy = -r; dy <= r; dy += 1) {
          if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) {
            continue;
          }
          if ((teleport(h, x + dx, y + dy, floor, 0) & 0xff) !== 0) {
            return { cell: cellOf(h), asked: [x, y] };
          }
        }
      }
    }
    throw new Error(`no cell to stand on within ${search} of ${x},${y}`);
  },
  time: (engine, { hours, minutes }) => {
    const clock = WORLD.clock.readPointer();
    clock.add(CLOCK.hours).writeU16(hours);
    clock.add(CLOCK.minutes).writeU16(minutes);
    clock.add(CLOCK.seconds).writeU16(0);
    clock.add(CLOCK.fraction).writeFloat(0);
    return readClock();
  },
  'day-speed': (engine, { day, night }) => {
    const clock = WORLD.clock.readPointer();
    clock.add(CLOCK.dayRate).writeFloat(day);
    clock.add(CLOCK.nightRate).writeFloat(night);
    return readClock();
  },
  zoom: (engine, { zoom }) => {
    setZoom(view(engine), zoom);
    return view(engine).add(VIEW_ZOOM).readFloat();
  },
  point: (engine, { x, y }) => pixelOf(engine, (x + 0.5) * CELL, (y + 0.5) * CELL),
};

// Runs a command on the render thread's next frame, where the game updates its world, so that a read never sees an
// object halfway through its update and a change is made where the game makes its own.
function worldCommand(name, args) {
  const command = COMMANDS[name];
  if (command === undefined) {
    return Promise.reject(new Error(`no world command ${name}`));
  }
  if (ADDRESSES.engine.readPointer().isNull()) {
    return Promise.reject(new Error('no game: the engine is made when a game starts'));
  }
  return new Promise((resolve, reject) => {
    world.jobs.push({ run: (engine) => command(engine, args ?? {}), resolve, reject, deadline: Date.now() + 2000 });
  });
}

function runWorldJobs(engine) {
  const jobs = world.jobs;
  world.jobs = [];
  for (const job of jobs) {
    try {
      job.resolve(job.run(engine));
    } catch (e) {
      job.reject(e);
    }
  }
}

function installWorld() {
  Interceptor.attach(WORLD.tick, {
    onEnter() {
      world.engine = ptr(this.context.ecx.toString());
      if (world.jobs.length > 0) {
        runWorldJobs(world.engine);
      }
    },
  });
  setInterval(() => {
    const now = Date.now();
    const late = world.jobs.filter((job) => now > job.deadline);
    world.jobs = world.jobs.filter((job) => now <= job.deadline);
    for (const job of late) {
      job.reject(new Error('the render thread ran no frame of the world in 2 s'));
    }
  }, 250);
  guards.worldTick = true;
}

// --- The game's own functions ---------------------------------------------------------------------------------------

// The window procedure sees itself active and never loses the focus (the render thread skips its frames while
// inactive, docs/re/window.md), and drops the input the agent did not send.
function installWindowProcedure() {
  const procedure = {
    onEnter(args) {
      let message = args[1].toUInt32();
      if (message === AGENT_MESSAGE) {
        const entry = input.pending.get(args[2].toUInt32());
        input.pending.delete(args[2].toUInt32());
        if (entry === undefined) {
          args[1] = ptr(WM.NULL);
          this.message = WM.NULL;
          return;
        }
        applyInput(entry);
        args[1] = ptr(entry.message);
        args[2] = ptr(entry.wParam);
        args[3] = ptr(entry.lParam);
        this.message = entry.message;
        return;
      }
      this.message = message;
      if (FOCUS_MESSAGES[message] !== undefined && focusSeen < 50) {
        focusSeen += 1;
        log('DEBUG', `window procedure: ${FOCUS_MESSAGES[message]}(${args[2]}, ${args[3]}) as the system sent it`);
      }
      switch (message) {
        case WM.ACTIVATEAPP:
        case WM.NCACTIVATE:
          args[2] = ptr(1);
          break;
        case WM.ACTIVATE:
          args[2] = ptr(1);  // WA_ACTIVE, not minimized
          break;
        case WM.KILLFOCUS:
          args[1] = ptr(WM.NULL);
          break;
        case WM.MOUSEACTIVATE:
          args[1] = ptr(WM.NULL);
          break;
        default:
          if (isInput(message)) {
            args[1] = ptr(WM.NULL);
          }
      }
    },
    onLeave(result) {
      if (this.message === WM.MOUSEACTIVATE) {
        result.replace(ptr(MA_NOACTIVATEANDEAT));
      }
    },
  };
  Interceptor.attach(ADDRESSES.sacredWndProc, procedure);
  Interceptor.attach(ADDRESSES.playWndProc, procedure);
  guards.sacredWndProc = true;
  guards.playWndProc = true;
}

const FOCUS_MESSAGES = { 0x6: 'WM_ACTIVATE', 0x7: 'WM_SETFOCUS', 0x8: 'WM_KILLFOCUS', 0x1c: 'WM_ACTIVATEAPP',
  0x21: 'WM_MOUSEACTIVATE', 0x86: 'WM_NCACTIVATE' };
let focusSeen = 0;

// Each flip is a frame; a screenshot asked for is taken by the flip itself, from the back buffer (captureScreenshot,
// docs/re/window.md).
function installFlip() {
  Interceptor.attach(ADDRESSES.dxDriver7_flip, {
    onEnter() {
      this.driver = this.context.ecx;
      this.shot = false;
      if (state.shotsWanted > 0 && this.driver.add(DRIVER_SHOTS).readU32() === 0) {
        state.shotsWanted -= 1;
        this.shot = true;
        this.captured = this.driver.add(DRIVER_FRAMES_CAPTURED).readU32();
        this.driver.add(DRIVER_FRAMES_CAPTURED).writeU32(0);
        this.driver.add(DRIVER_SHOTS).writeU32(1);
        ADDRESSES.screenshotName.writeU8(0);
      }
    },
    onLeave(result) {
      state.frames += 1;
      runSchedule();
      state.flipError = result.toInt32();
      const time = now();
      state.flipTimes.push(time);
      if (state.flipTimes.length > 240) {
        state.flipTimes.splice(0, state.flipTimes.length - 240);
      }
      if (this.shot) {
        this.driver.add(DRIVER_FRAMES_CAPTURED).writeU32(this.captured);
        const file = ADDRESSES.screenshotName.readCString();
        send({ type: 'shot', file, frame: state.frames });
      }
    },
  });
  guards.dxDriver7_flip = true;
}

// The game's own end. Frida unloads the script in its own hook of ExitProcess, before any of the agent's, so the
// agent marks the end earlier: when WinMain returns, or when the C runtime's exit is called.
function installExit() {
  Interceptor.attach(ADDRESSES.WinMain, {
    onLeave(result) {
      state.exiting = true;
      log('INFO', `WinMain returned ${result.toInt32()}`);
    },
  });
  Process.attachModuleObserver({
    onAdded(module) {
      if (module.name.toLowerCase() !== 'msvcrt.dll') {
        return;
      }
      for (const name of ['exit', '_exit']) {
        Interceptor.attach(module.getExportByName(name), {
          onEnter() {
            state.exiting = true;
          },
        });
      }
    },
  });
  guards.exit = true;
}

// The game's log (logPrintf), formatted here, whatever its own LOG key says.
function format(text, stack) {
  let slot = 0;
  const word = () => stack.add(4 * slot++).readU32();
  return text.replace(/%([-+ #0]*)(\d+|\*)?(?:\.(\d+|\*))?(hh|h|ll|l|L|I64)?([diouxXcsSfFeEgGp%])/g,
    (whole, flags, width, precision, size, kind) => {
      if (kind === '%') {
        return '%';
      }
      if (width === '*') {
        width = String(word() | 0);
      }
      if (precision === '*') {
        precision = String(word() | 0);
      }
      let value;
      switch (kind) {
        case 'd': case 'i': {
          value = size === 'll' || size === 'I64' ? stack.add(4 * (slot += 2) - 8).readS64().toString()
            : String(word() | 0);
          if (precision !== undefined) {
            const negative = value.startsWith('-');
            value = (negative ? '-' : '') + value.replace('-', '').padStart(Number(precision), '0');
          }
          break;
        }
        case 'u': value = String(word() >>> 0); break;
        case 'o': value = (word() >>> 0).toString(8); break;
        case 'x': value = (word() >>> 0).toString(16); break;
        case 'X': value = (word() >>> 0).toString(16).toUpperCase(); break;
        case 'p': value = (word() >>> 0).toString(16).toUpperCase().padStart(8, '0'); break;
        case 'c': value = String.fromCharCode(word() & 0xff); break;
        case 's': case 'S': {
          const pointer = ptr(word());
          value = pointer.isNull() ? '(null)' : (kind === 'S' || size === 'l' ? pointer.readUtf16String()
            : pointer.readCString());
          if (precision !== undefined) {
            value = value.slice(0, Number(precision));
          }
          break;
        }
        default: {
          const number = stack.add(4 * slot).readDouble();
          slot += 2;
          const digits = precision === undefined ? 6 : Number(precision);
          value = kind === 'e' || kind === 'E' ? number.toExponential(digits)
            : kind === 'g' || kind === 'G' ? String(Number(number.toPrecision(Math.max(digits, 1))))
              : number.toFixed(digits);
        }
      }
      const pad = flags.includes('0') && !flags.includes('-') && kind !== 's' ? '0' : ' ';
      if (width !== undefined && value.length < Number(width)) {
        value = flags.includes('-') ? value.padEnd(Number(width)) : value.padStart(Number(width), pad);
      }
      return value;
    });
}

function installGameLog() {
  Interceptor.attach(ADDRESSES.logPrintf, {
    onEnter() {
      const time = Date.now();
      if (time - logWindowStart > 1000) {
        if (logSuppressed > 0) {
          log('WARN', `game log: ${logSuppressed} records dropped (over 2,000 a second)`);
        }
        logWindowStart = time;
        logInWindow = 0;
        logSuppressed = 0;
      }
      logInWindow += 1;
      if (logInWindow > 2000) {
        logSuppressed += 1;
        return;
      }
      try {
        const stack = this.context.esp;
        const text = stack.add(4).readPointer().readCString();
        const line = format(text, stack.add(8)).replace(/[\r\n]+$/, '');
        if (line.length > 0) {
          send({ type: 'game', message: line });
        }
      } catch (e) {
        log('WARN', `game log: ${e.message}`);
      }
    },
  });
  guards.logPrintf = true;
}

// --- The host's calls -----------------------------------------------------------------------------------------------

function frameStats() {
  const times = state.flipTimes;
  const last = times.length > 0 ? times[times.length - 1] : 0;
  const recent = times.filter((t) => last - t < 1000);
  let slowest = 0;
  for (let i = 1; i < recent.length; i += 1) {
    slowest = Math.max(slowest, recent[i] - recent[i - 1]);
  }
  const stale = times.length === 0 || now() - last > 1000;
  const fps = stale ? 0 : recent.length;
  return { fps, frame_ms: fps > 0 ? 1000 / fps : 0, slowest_ms: slowest };
}

function windowRect() {
  if (state.window === null || native.isWindow(state.window) === 0) {
    return null;
  }
  const rect = Memory.alloc(16);
  native.getWindowRect(state.window, rect);
  return [rect.readS32(), rect.add(4).readS32(), rect.add(8).readS32(), rect.add(12).readS32()];
}

rpc.exports = {
  // Installs every guard, before the game's first instruction; the host resumes the game only if all are in place.
  init(options) {
    Object.assign(config, options);
    const sacred = Process.findModuleByName('Sacred.exe');
    if (sacred === null || !sacred.base.equals(SACRED_BASE)) {
      throw new Error(`Sacred.exe is not at ${SACRED_BASE}: ${sacred === null ? 'not found' : sacred.base}`);
    }
    installHookGuards();
    installFocusGuards();
    installWindowGuards();
    installCursorAndDisplayGuards();
    installInputGuards();
    installDirectDrawGuards();
    installWindowProcedure();
    installFlip();
    installWorld();
    installExit();
    if (config.gameLog !== false) {
      installGameLog();
    }
    setInterval(runSchedule, 10);
    state.lastBeat = Date.now();
    setInterval(() => {
      if (Date.now() - state.lastBeat > config.heartbeatTimeout) {
        end(4, `no heartbeat from the host for ${config.heartbeatTimeout / 1000} s`);
      }
    }, 500);
    return { guards, arch: Process.arch };
  },
  heartbeat() {
    state.lastBeat = Date.now();
  },
  state() {
    return {
      window: state.window === null ? null : state.window.toString(),
      rect: windowRect(),
      frames: state.frames,
      flip_error: state.flipError,
      focused: (ADDRESSES.engineFlags.readU32() & INACTIVE) === 0,
      screen: screen(),
      cursor: [virtualInput.cursor.x, virtualInput.cursor.y],
      refusals,
      ...frameStats(),
    };
  },
  screenshot() {
    state.shotsWanted += 1;
  },
  input(params) {
    sendInput(params);
  },
  world(name, args) {
    return worldCommand(name, args);
  },
  close() {
    if (state.window === null) {
      return false;
    }
    native.postMessageA(state.window, WM.CLOSE, ptr(0), ptr(0));
    return true;
  },
  // The script is unloaded: the host is gone, and the game goes with it, unless it is ending itself.
  dispose() {
    if (!state.exiting) {
      end(5, 'the agent is unloaded');
    }
  },
};
