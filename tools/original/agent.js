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

// `interactive` (#551): the maintainer plays the original on the monitor `monitor` (0 the primary, else the n-th that
// the system lists), its window scaled by dgVoodoo2; the player's input reaches it while its window has the foreground.
const config = { place: 'offscreen', heartbeatTimeout: 5000, interactive: false, monitor: 0 };
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
  clipCursor: new NativeFunction(user32.getExportByName('ClipCursor'), 'int', ['pointer'], 'stdcall'),
  getClipCursor: new NativeFunction(user32.getExportByName('GetClipCursor'), 'int', ['pointer'], 'stdcall'),
  getWindowRect: new NativeFunction(user32.getExportByName('GetWindowRect'), 'int', ['pointer', 'pointer'],
    'stdcall'),
  postMessageA: new NativeFunction(user32.getExportByName('PostMessageA'), 'int',
    ['pointer', 'uint', 'pointer', 'pointer'], 'stdcall'),
  setWindowPos: new NativeFunction(user32.getExportByName('SetWindowPos'), 'int',
    ['pointer', 'pointer', 'int', 'int', 'int', 'int', 'uint'], 'stdcall'),
  isWindow: new NativeFunction(user32.getExportByName('IsWindow'), 'int', ['pointer'], 'stdcall'),
  getForegroundWindow: new NativeFunction(user32.getExportByName('GetForegroundWindow'), 'pointer', [], 'stdcall'),
  enumDisplayMonitors: new NativeFunction(user32.getExportByName('EnumDisplayMonitors'), 'int',
    ['pointer', 'pointer', 'pointer', 'pointer'], 'stdcall'),
  getMonitorInfoW: new NativeFunction(user32.getExportByName('GetMonitorInfoW'), 'int', ['pointer', 'pointer'],
    'stdcall'),
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
  if (clip.held !== null) {
    clip.wanted = null;
    holdCursor();
  }
  send({ type: 'fatal', message: why });
  native.terminateProcess(ptr(-1), code);
}

function isGameWindow(hwnd) {
  return state.window !== null && hwnd.equals(state.window);
}

// The monitors as the system lists them: their rectangles, and which is the primary (MONITORINFOF_PRIMARY).
function monitors() {
  const found = [];
  const info = Memory.alloc(40);
  const each = new NativeCallback((monitor) => {
    info.writeU32(40);
    if (native.getMonitorInfoW(monitor, info) !== 0) {
      const rect = [0, 4, 8, 12].map((offset) => info.add(4 + offset).readS32());
      found.push({ rect, primary: (info.add(36).readU32() & 1) !== 0 });
    }
    return 1;
  }, 'int', ['pointer', 'pointer', 'pointer', 'pointer'], 'stdcall');
  native.enumDisplayMonitors(ptr(0), ptr(0), each, ptr(0));
  return found;
}

// Where the window goes: to the right of every monitor, or behind the other windows where the system put it; for the
// maintainer's play, at the corner of the monitor asked for, where dgVoodoo2 makes it the monitor's size.
function placement() {
  if (config.interactive) {
    const all = monitors();
    all.forEach((m, i) => log('INFO', `monitor ${i + 1}: ${m.rect[2] - m.rect[0]}x${m.rect[3] - m.rect[1]} at ` +
      `${m.rect[0]},${m.rect[1]}${m.primary ? ', the primary' : ''}`));
    const chosen = config.monitor > 0 ? all[config.monitor - 1] : all.find((m) => m.primary);
    if (chosen === undefined) {
      end(6, `no monitor ${config.monitor}: ${all.length} found`);
      return { x: 0, y: 0, behind: false };
    }
    return { x: chosen.rect[0], y: chosen.rect[1], behind: false };
  }
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

// The window that has the foreground in truth, past the guard below that answers the game's window; looked up at
// most every 50 ms, as the key queries ask it many times a frame.
let askingTruth = false;
const foreground = { window: ptr(0), at: 0 };

function trueForeground() {
  const time = Date.now();
  if (time - foreground.at > 50) {
    askingTruth = true;
    try {
      foreground.window = native.getForegroundWindow();
    } finally {
      askingTruth = false;
    }
    foreground.at = time;
  }
  return foreground.window;
}

// In the maintainer's play, whether the player's input is the game's: its window has the foreground.
function playerHasWindow() {
  return config.interactive && state.window !== null && trueForeground().equals(state.window);
}

function installFocusGuards() {
  // The calls that would take the focus or raise a window do nothing, but in the maintainer's play.
  replace('user32.dll', 'SetForegroundWindow', 'int', ['pointer'], (original, hwnd) =>
    config.interactive ? original(hwnd) : 1);
  replace('user32.dll', 'BringWindowToTop', 'int', ['pointer'], (original, hwnd) =>
    config.interactive ? original(hwnd) : 1);
  replace('user32.dll', 'SetActiveWindow', 'pointer', ['pointer'], (original, hwnd) =>
    config.interactive ? original(hwnd) : state.window ?? ptr(0));
  replace('user32.dll', 'SetFocus', 'pointer', ['pointer'], (original, hwnd) =>
    config.interactive ? original(hwnd) : state.window ?? ptr(0));
  // The focus queries answer with the game's window, so that sdk_FORCE_FG_WAIT ends at once (docs/re/window.md).
  for (const name of ['GetForegroundWindow', 'GetActiveWindow', 'GetFocus']) {
    replace('user32.dll', name, 'pointer', [], (original) => (askingTruth ? original() : state.window ?? original()));
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
      this.className = args[1].compare(ptr(0x10000)) < 0 ? `#${args[1].toUInt32()}` : args[1].readCString();
      // In the maintainer's play the game's window opens shown and activated on the monitor asked for, and nothing
      // moves it after: dgVoodoo2 makes it the monitor's size.
      if (config.interactive) {
        if (this.className === 'Sacred') {
          state.place = placement();
          args[4] = ptr(state.place.x);
          args[5] = ptr(state.place.y);
        }
        return;
      }
      const exStyle = (args[0].toUInt32() | WS_EX_NOACTIVATE) & ~WS_EX_TOPMOST;
      args[0] = ptr(exStyle >>> 0);
      state.place = placement();
      args[4] = ptr(state.place.x);
      args[5] = ptr(state.place.y);
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
      if (!isGameWindow(args[0]) || config.interactive) {
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
      // In the maintainer's play the window keeps the monitor's corner, where the game would centre its 1024 x 768 and
      // dgVoodoo2 then grows it to the monitor's size from there.
      if (config.interactive) {
        if ((args[6].toUInt32() & SWP.NOMOVE) === 0 && state.place !== null) {
          args[2] = ptr(state.place.x);
          args[3] = ptr(state.place.y);
        }
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

// The calls of ClipCursor and SetCursorPos in the maintainer's play, logged the first times.
let cursorCalls = 0;
// The game's clip of the cursor in the maintainer's play, in its 1024 x 768, held while its window has the foreground
// and let go when another window takes it (the game believes itself active throughout). dgVoodoo2 maps the game's
// rectangle and points onto the picture it scales into the window, below these guards: they pass them as they are.
const clip = { wanted: null, held: null, heldAt: 0, passing: false };
const clipRect = Memory.alloc(16);

// Holds the cursor in the game's clip while its window has the foreground, again every second in case the system let
// it go (a locked desktop does), and lets it go otherwise.
function holdCursor() {
  const wanted = clip.wanted !== null && playerHasWindow() ? clip.wanted : null;
  if (wanted === null && clip.held === null) {
    return;
  }
  if (wanted !== null && String(wanted) === String(clip.held) && Date.now() - clip.heldAt < 1000) {
    return;
  }
  if (wanted !== null) {
    wanted.forEach((value, i) => clipRect.add(4 * i).writeS32(value));
  }
  clip.passing = true;
  try {
    native.clipCursor(wanted === null ? ptr(0) : clipRect);
  } finally {
    clip.passing = false;
  }
  clip.held = wanted;
  clip.heldAt = Date.now();
}

function installCursorAndDisplayGuards() {
  // The cursor is never held in the window. In the maintainer's play the game holds it in its screen and may move it
  // while its window has the foreground.
  replace('user32.dll', 'ClipCursor', 'int', ['pointer'], (original, rect) => {
    if (clip.passing) {
      return original(rect);
    }
    if (config.interactive) {
      clip.wanted = rect.isNull() ? null : [0, 4, 8, 12].map((o) => rect.add(o).readS32());
      if (cursorCalls < 20) {
        cursorCalls += 1;
        log('DEBUG', `ClipCursor(${clip.wanted ?? 'NULL'})`);
      }
      holdCursor();
    }
    return 1;
  });
  replace('user32.dll', 'SetCursorPos', 'int', ['int', 'int'], (original, x, y) => {
    if (!playerHasWindow()) {
      return 1;
    }
    if (cursorCalls < 20) {
      cursorCalls += 1;
      log('DEBUG', `SetCursorPos(${x}, ${y})`);
    }
    return original(x, y);
  });
  if (config.interactive) {
    setInterval(holdCursor, 100);
  }
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
// cursor, all keys up and the cursor at the window's corner until the agent's input moves them (#450). In the
// maintainer's play they answer from the system while the game's window has the foreground (#551), so that the keys
// typed into a terminal never reach the game.
const virtualInput = { cursor: { x: 0, y: 0 }, keys: new Uint8Array(256), pressed: new Uint8Array(256) };

function installInputGuards() {
  replace('user32.dll', 'GetAsyncKeyState', 'int16', ['int'], (original, key) => {
    if (playerHasWindow()) {
      return original(key);
    }
    const down = (virtualInput.keys[key & 0xff] & 0x80) !== 0;
    const pressed = virtualInput.pressed[key & 0xff];
    virtualInput.pressed[key & 0xff] = 0;
    return (down ? -32768 : 0) | pressed;
  });
  replace('user32.dll', 'GetKeyState', 'int16', ['int'], (original, key) => {
    if (playerHasWindow()) {
      return original(key);
    }
    const value = virtualInput.keys[key & 0xff];
    return ((value & 0x80) !== 0 ? -32768 : 0) | (value & 1);
  });
  replace('user32.dll', 'GetKeyboardState', 'int', ['pointer'], (original, buffer) => {
    if (playerHasWindow()) {
      return original(buffer);
    }
    buffer.writeByteArray(virtualInput.keys.buffer);
    return 1;
  });
  const point = Memory.alloc(8);
  replace('user32.dll', 'GetCursorPos', 'int', ['pointer'], (original, out) => {
    if (playerHasWindow()) {
      return original(out);
    }
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
// 21st, SetDisplayMode the 22nd. In the maintainer's play the game runs full screen through dgVoodoo2's DirectDraw
// beside Sacred.exe, whose full screen is a window of the monitor's size and changes no display mode (#551): there
// both pass, and through any other DirectDraw they end the run all the same.
function installDirectDrawGuards() {
  const ddraw = Module.load('ddraw.dll');
  const directory = (path) => path.replace(/[^\\/]*$/, '').toLowerCase();
  const sacred = Process.findModuleByName('Sacred.exe').path;
  const wrapped = directory(ddraw.path) === directory(sacred);
  log('INFO', `DirectDraw: ${ddraw.path}${wrapped ? ' (dgVoodoo2 beside Sacred.exe)' : `, Sacred.exe ${sacred}`}`);
  if (config.interactive && !wrapped) {
    throw new Error(`the maintainer's play needs dgVoodoo2's DirectDraw beside Sacred.exe, not ${ddraw.path}`);
  }
  const fullScreenPasses = config.interactive && wrapped;
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
            if (fullScreenPasses) {
              log('INFO', `DirectDraw's exclusive full-screen level (0x${flags.toString(16)}), dgVoodoo2's`);
            } else {
              end(3, `DirectDraw's exclusive full-screen level (0x${flags.toString(16)}): FULLSCREEN is not 0?`);
            }
          }
        },
      });
    }
    if (!hooked.has(setDisplayMode.toString())) {
      hooked.add(setDisplayMode.toString());
      Interceptor.attach(setDisplayMode, {
        onEnter(args) {
          const mode = `${args[1].toUInt32()}x${args[2].toUInt32()}`;
          if (fullScreenPasses) {
            log('INFO', `DirectDraw's SetDisplayMode ${mode}, dgVoodoo2's: no display mode changes`);
          } else {
            end(3, `DirectDraw's SetDisplayMode ${mode}`);
          }
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
  setLevel: ptr('0x549500'),         // cCreature: thiscall (creature, level, flag), its stats' level setter 0x564D60
  statsLevel: ptr('0x564d60'),       // the stats' level setter: thiscall (stats, level, a, b); a 0 scales the level by
                                     // the difficulty, b 0 holds an NPC's at its sector's highest, b 1 takes it as
                                     // asked, at most 250 (docs/re/objects.md)
  addExperience: ptr('0x549520'),    // cCreature: thiscall (creature, points), its stats' addExperience 0x57E160
  strike: ptr('0x56b130'),           // a strike's resolution: thiscall (the defender's stats, the event 5, ...)
  cheats: ptr('0x182ee3c'),          // the console's cheat flags (0x615550): bit 1 its god mode, which `lord` toggles
  makeCreature: ptr('0x480d40'),     // FillSector's creature maker: thiscall (interpreter, cell x, cell y, 0, 0, type,
                                     // level, three items, 0, flags into +0x1F4, hostility), the creature made
  hostile: ptr('0x549000'),          // cCreature: thiscall (creature, id), whether it is hostile to that living one
  drop: ptr('0x559bf0'),             // cCreature: thiscall (creature, type, bonus, value, killer's id), one drop
  sectorExists: ptr('0x634840'),     // the world: thiscall (world, sector), whether the world has it
  sectorInfo: ptr('0x63cfc0'),       // the world: thiscall (world, sector), its information (SpawnValues')
  difficulty: ptr('0x182ee44'),      // the difficulty, 0 bronze to 4 niobium
  lowestRaise: ptr('0xad4c18'),      // by the difficulty's column: what CreateNPC adds to a sector's lowest level
  highestRaise: ptr('0xad4e1c'),     // and to its highest
  strikeEvent: ptr('0x89095c'),      // the vtable of the event that 0x4EA3F0 sends a creature it hits
  addArt: ptr('0x580750'),           // the stats: thiscall (stats, an art's entry of 22 bytes on the stack), adds it
  artIndex: ptr('0x581d40'),         // the stats: thiscall (stats, art), the index of its entry
  raiseArt: ptr('0x581f80'),         // the stats: thiscall (stats, index, levels, for good, no linked arts)
};
const CELL = 53.66563;               // a cell's side in world units
const CREATURE = {
  id: 0x0c, type: 0x10, x: 0x1c, y: 0x20, floor: 0x24, layer: 0x2c, ai: 0xfc, aiSub: 0xfe, target: 0x104,
  action: 0x150, actionSub: 0x152, gait: 0x1f8, experience: 0x3b4, level: 0x3fe, most: 0x4d4, health: 0x4d8,
};
const CLOCK = { dayRate: 0x10, nightRate: 0x14, hours: 0x20, minutes: 0x22, seconds: 0x24, fraction: 0x28 };
// A creature's stats at +0x3A8 (docs/re/fight.md, docs/re/items.md, "The bonus slots in the values").
const STATS = 0x3a8;
const STAT = {
  speeds: 0x3c, attributes: 0x10, base: 0x4a, level: 0x56, attack: 0x5a, defence: 0x5e, resistances: 0x66,
  most: 0x76, least: 0x86, itemMultipliers: 0x96, multipliers: 0xa6, other: 0xb6, resistanceMultipliers: 0xd6,
  attackMultiplier: 0xe6, defenceMultiplier: 0xea, mostHealth: 0x12c, health: 0x130,
};
const VIEW_ZOOM = 0xb60;
const VIEW_SCREEN_TO_WORLD = 6;      // the view's virtual function: thiscall (view, x, y, position*)

const teleport = new NativeFunction(WORLD.teleport, 'uint32', ['pointer', 'int', 'int', 'int', 'int'], 'thiscall');
const setZoom = new NativeFunction(WORLD.setZoom, 'uint32', ['pointer', 'float'], 'thiscall');
const setLevel = new NativeFunction(WORLD.setLevel, 'void', ['pointer', 'int', 'int'], 'thiscall');
const statsLevel = new NativeFunction(WORLD.statsLevel, 'void', ['pointer', 'int', 'int', 'int'], 'thiscall');
const addExperience = new NativeFunction(WORLD.addExperience, 'void', ['pointer', 'int'], 'thiscall');
const makeCreature = new NativeFunction(WORLD.makeCreature, 'pointer',
  ['pointer', 'int', 'int', 'int', 'int', 'int', 'int', 'int', 'int', 'int', 'int', 'uint32', 'int'], 'thiscall');
const addArt = new NativeFunction(WORLD.addArt, 'void',
  ['pointer', 'uint32', 'uint32', 'uint32', 'uint32', 'uint32', 'uint32'], 'thiscall');
const artIndex = new NativeFunction(WORLD.artIndex, 'uint32', ['pointer', 'uint32'], 'thiscall');
const raiseArt = new NativeFunction(WORLD.raiseArt, 'uint32', ['pointer', 'uint32', 'uint32', 'uint32', 'uint32'],
  'thiscall');
// A creature's combat arts (docs/re/arts.md, "A creature's arts"): entries of 22 bytes between the pointers at
// +0x102 and +0x106 of its stats.
const ART = { list: 0x102, end: 0x106, size: 22, table: 0, id: 4, level: 6, bonus: 7, flags: 8, time: 10, factor: 14,
  remaining: 18 };
const isHostile = new NativeFunction(WORLD.hostile, 'uint8', ['pointer', 'uint32'], 'thiscall');
const dropOf = new NativeFunction(WORLD.drop, 'uint32', ['pointer', 'uint32', 'uint32', 'uint32', 'uint32'], 'thiscall');
const sectorExists = new NativeFunction(WORLD.sectorExists, 'uint32', ['pointer', 'uint32'], 'thiscall');
const sectorInfo = new NativeFunction(WORLD.sectorInfo, 'pointer', ['pointer', 'uint32'], 'thiscall');
// The creature maker's `this`, the script interpreter: of it, the maker reads only the byte +0x12, set while a sector's
// `Enter` runs (docs/re/spawning.md), so a creature made with it clear stays when its sector leaves the view.
const interpreter = Memory.alloc(0x100);
const GOD = 1;                       // the bit of the god mode in the console's cheat flags
const SPARED_HEALTH = 0x3fffffff;    // the health a spared defender shows its strike's resolution
const BOSS = 0x80000;                // a boss: +0x1F4 (docs/re/fight.md, "Bosses and champions")
const CHAMPION = 0x40;               // a champion: +0x2B8, with +0xB0 bit 0x400, a champion's levelling
const CHAMPION_LEVELLING = 0x400;
const KILL_TRIES = 30;               // the strikes `kill` sends a creature before it gives up
const MOST_LEVEL = 216;              // the highest level of an experience (0x57F8D0, docs/re/fight.md)

const world = {
  jobs: [],        // {run, resolve, reject, deadline}: run on the render thread's next frame
  engine: null,    // the engine of the last tick
  creatureVtables: new Map(),
  god: null,       // with `god on`: {id, health}, the hero and the health it keeps
  immortal: new Set(),   // the ids of the creatures `spawn --immortal` made, whose health strikes leave as it was
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

// A creature's stats: the attributes (with the items' bonuses) and the base ones, the level, A and D before their
// multipliers, each kind's resistance, most and least damage before theirs, the multipliers, the attack speed and the
// words after it, and the health.
function readStats(object) {
  const s = object.add(STATS);
  const words = (at, n) => Array.from({ length: n }, (_, i) => s.add(at + 2 * i).readU16());
  const floats = (at, n) => Array.from({ length: n }, (_, i) => Math.round(s.add(at + 4 * i).readFloat() * 1000) / 1000);
  return {
    id: object.add(CREATURE.id).readU32(), type: object.add(CREATURE.type).readU16(),
    attributes: words(STAT.attributes, 6), base: words(STAT.base, 6), level: s.add(STAT.level).readU16(),
    attack: floats(STAT.attack, 1)[0], defence: floats(STAT.defence, 1)[0],
    resistances: floats(STAT.resistances, 4), most: floats(STAT.most, 4), least: floats(STAT.least, 4),
    itemMultipliers: floats(STAT.itemMultipliers, 4), multipliers: floats(STAT.multipliers, 4),
    other: floats(STAT.other, 4), resistanceMultipliers: floats(STAT.resistanceMultipliers, 4),
    attackMultiplier: floats(STAT.attackMultiplier, 1)[0], defenceMultiplier: floats(STAT.defenceMultiplier, 1)[0],
    speeds: words(STAT.speeds, 3), health: [s.add(STAT.health).readS32(), s.add(STAT.mostHealth).readS32()],
  };
}

const COMMANDS = {
  snapshot: (engine) => snapshot(engine),
  stats: (engine, { id }) => {
    const object = id ? objectById(id) : hero();
    if (object === null || !isCreature(object)) {
      throw new Error(id ? `no creature ${id}` : 'no hero');
    }
    return readStats(object);
  },
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
  // The hero takes no damage (#558): the console's own god mode, which spares a player a strike's knock-down, and its
  // health kept, by the strike's resolution and at each frame (installWorld).
  god: (engine, { on }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const want = on ?? world.god === null;
    const flags = WORLD.cheats.readU32();
    WORLD.cheats.writeU32(want ? (flags | GOD) >>> 0 : (flags & ~GOD) >>> 0);
    world.god = want ? { id: h.add(CREATURE.id).readU32(), hero: h, health: h.add(CREATURE.health).readS32() } : null;
    return { god: want, hero: readCreature(h) };
  },
  // The world's time stands, as under the menu over a game, without the menu (#567): the first byte of the timer,
  // which the state reads as the screen `paused`; the frames go on.
  pause: (engine, { on }) => {
    const timer = ADDRESSES.timer.readPointer();
    if (timer.isNull()) {
      throw new Error('no timer');
    }
    const want = on ?? timer.readU8() === 0;
    timer.writeU8(want ? 1 : 0);
    return { paused: want };
  },
  // A creature's combat arts, the hero's by default (#569); `learn` raises the hero's art by levels as runes do,
  // through the original's own functions, and `ready` ends every art's regeneration.
  arts: (engine, { id, learn, levels, ready }) => {
    const object = id ? objectById(id) : hero();
    if (object === null || !isCreature(object)) {
      throw new Error(id ? `no creature ${id}` : 'no hero');
    }
    const stats = object.add(STATS);
    if (learn) {
      // The rune's way (0x5CD100): the entry of the table at level 0 if the list lacks it, then a level for good
      // a rune at a time; an id below 1000 is of the first table, the spells, from 1000 of the second.
      addArt(stats, learn < 1000 ? 1 : 2, learn, 0, 0, 0, 0);
      for (let i = 0; i < levels; i += 1) {
        if ((raiseArt(stats, artIndex(stats, learn), 1, 1, 0) & 0xff) === 0) {
          throw new Error(`the art ${learn} rises no further`);
        }
      }
    }
    if (ready) {
      for (const entry of artEntries(stats)) {
        entry.add(ART.remaining).writeFloat(0);
      }
    }
    return artEntries(stats).map((e) => ({
      table: e.add(ART.table).readU32(), id: e.add(ART.id).readU16(), level: e.add(ART.level).readU8(),
      bonus: e.add(ART.bonus).readU8(), flags: e.add(ART.flags).readU16(),
      time: e.add(ART.time).readFloat(), factor: e.add(ART.factor).readFloat(),
      remaining: e.add(ART.remaining).readFloat(),
    }));
  },
  // A creature's health to its most, the hero's by default.
  heal: (engine, { id }) => {
    const object = id ? objectById(id) : hero();
    if (object === null || !isCreature(object)) {
      throw new Error(id ? `no creature ${id}` : 'no hero');
    }
    const most = object.add(CREATURE.most).readS32();
    object.add(CREATURE.health).writeS32(most);
    if (world.god !== null && world.god.id === object.add(CREATURE.id).readU32()) {
      world.god.health = most;
    }
    return readCreature(object);
  },
  // The hero's level: the experience the level starts at added as the kills add it, so that each level between
  // gives what a level-up gives. The original's levels only rise.
  level: (engine, { level }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const current = h.add(CREATURE.level).readU16();
    if (level < current) {
      throw new Error(`the hero is at level ${current}: the original's levels only rise (a new game starts at 1)`);
    }
    const points = levelStart(level) - h.add(CREATURE.experience).readU32();
    if (points > 0) {
      addExperience(h, points);
    }
    return readCreature(h);
  },
  // The hero's experience set (upward only) or added, as a kill adds it (0x57E160): its level follows.
  experience: (engine, { points, add }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const current = h.add(CREATURE.experience).readU32();
    const more = add ? points : points - current;
    if (more < 0) {
      throw new Error(`the hero has ${current} experience: the original's experience only rises`);
    }
    if (more > 0) {
      addExperience(h, more);
    }
    return readCreature(h);
  },
  // Creatures made as FillSector makes them (#559): its creature maker on cells about a cell, from it outward, then
  // the level setter: at the level asked as it is (as a horse's), or by CreateNPC's rule held at the sector's highest,
  // or with a boss's or a champion's mark at the marker level 1 as FillSector gives them (docs/re/fight.md, "Bosses
  // and champions").
  spawn: (engine, { type, x, y, count, level, rank, hostility, immortal }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const made = [];
    for (const [cx, cy] of cellsAbout(x, y, count)) {
      const creature = makeCreature(interpreter, cx, cy, 0, 0, type, 0, 0, 0, 0, 0,
        rank === 'boss' ? BOSS : 0xffffffff, hostility);
      if (creature.isNull()) {
        throw new Error(`the creature maker made no creature of type ${type}`);
      }
      if (level !== null && level !== undefined) {
        statsLevel(creature.add(STATS), level, 1, 1);
      } else {
        setLevel(creature, rank ? 1 : scriptedLevel(creature, h), 0);
      }
      if (rank === 'champion') {
        creature.add(0x2b8).writeU8(creature.add(0x2b8).readU8() | CHAMPION);
        creature.add(0xb0).writeU32((creature.add(0xb0).readU32() | CHAMPION_LEVELLING) >>> 0);
      }
      const id = creature.add(CREATURE.id).readU32();
      if (immortal) {
        world.immortal.add(id);
      }
      made.push({ ...readCreature(creature), hostile: isHostile(creature, h.add(CREATURE.id).readU32()) !== 0 });
    }
    return made;
  },
  // The hostile creatures within a radius die as the hero's kills (#559): the hero's strike's event sent to each
  // (0x4EA3F0's) at a health of 1, again while the strikes miss, so that the creature's own handler makes its death,
  // its drops and the hero's experience; one still alive after KILL_TRIES strikes gets its health back.
  // (A function called from the agent's own hook of the frame runs past the agent's other hooks, so the strike's
  // resolution cannot be changed from inside it for these strikes.)
  kill: (engine, { radius }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const heroId = h.add(CREATURE.id).readU32();
    const [hx, hy] = cellOf(h);
    const victims = creatures().filter((o) => {
      if (o.equals(h) || isHostile(o, heroId) === 0 || o.add(CREATURE.health).readS32() <= 0) {
        return false;
      }
      const [x, y] = cellOf(o);
      return Math.max(Math.abs(x - hx), Math.abs(y - hy)) <= radius;
    });
    return victims.map((victim) => {
      const health = victim.add(CREATURE.health).readS32();
      let strikes = 0;
      while (strikes < KILL_TRIES && !isDying(victim)) {
        victim.add(CREATURE.health).writeS32(1);
        strike(h, victim);
        strikes += 1;
      }
      if (!isDying(victim)) {
        victim.add(CREATURE.health).writeS32(health);
      }
      return { ...readCreature(victim), strikes };
    });
  },
  // The drop of a creature as its death makes it, the hero its killer (#559): the nearest hostile creature, or ID.
  drop: (engine, { id, times }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const heroId = h.add(CREATURE.id).readU32();
    const creature = id ? objectById(id) : nearestHostile(h);
    if (creature === null || !isCreature(creature)) {
      throw new Error(id ? `no creature ${id}` : 'no hostile creature alive near the hero');
    }
    const made = [];
    for (let i = 0; i < times; i += 1) {
      made.push(dropOf(creature, 0, 0, 0, heroId));
    }
    return { creature: readCreature(creature), made: made.map(describeMade) };
  },
  // An item of a type, as a drop forced to it makes it (#559): the hero's drop of the type, at the hero's place and
  // level, thrown as a death's drop is.
  item: (engine, { type }) => {
    const h = hero();
    if (h === null) {
      throw new Error('no hero');
    }
    const made = dropOf(h, type, 0, 0, h.add(CREATURE.id).readU32());
    return { made: describeMade(made) };
  },
};

// The entries of a creature's arts.
function artEntries(stats) {
  const begin = stats.add(ART.list).readPointer();
  const end = stats.add(ART.end).readPointer();
  if (begin.isNull() || end.compare(begin) <= 0) {
    return [];
  }
  const count = Math.floor(end.sub(begin).toInt32() / ART.size);
  return Array.from({ length: Math.min(count, 256) }, (_, i) => begin.add(ART.size * i));
}

// The cells of `count` creatures about a cell: the cell, then the rings about it.
function cellsAbout(x, y, count) {
  const cells = [];
  for (let r = 0; cells.length < count; r += 1) {
    for (let dy = -r; dy <= r && cells.length < count; dy += 1) {
      for (let dx = -r; dx <= r && cells.length < count; dx += 1) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) === r) {
          cells.push([x + dx, y + dy]);
        }
      }
    }
  }
  return cells;
}

// The level CreateNPC gives a creature without a level of its own (docs/re/spawning.md, "`CreateNPC`'s level"): the
// hero's, held within its sector's lowest and highest raised by the difficulty's additions, or one more between them;
// the difficulty's scaling of the levels (0x552230) and a horse's riding are left out, which bronze's first quarter
// hour of play does too.
function scriptedLevel(creature, h) {
  let level = h.add(CREATURE.level).readU16();
  const world_ = WORLD.objects.readPointer().readPointer();
  const sector = creature.add(0x18).readU16();
  if (world_.isNull() || sectorExists(world_, sector) === 0) {
    return level;
  }
  const info = sectorInfo(world_, sector);
  if (info.isNull() || info.add(0xd3).readU16() === 0 || info.add(0xd5).readU16() === 0) {
    return level;
  }
  // The column of the additions by the difficulty (CreateNPC at 0x48440E): 3, 2, 1 and 0 for 4 down to 1, else 5.
  const difficulty = WORLD.difficulty.readS32();
  const column = { 4: 3, 3: 2, 2: 1, 1: 0 }[difficulty] ?? 5;
  const lowest = info.add(0xd3).readU16() + WORLD.lowestRaise.add(4 * column).readS32();
  const highest = info.add(0xd5).readU16() + WORLD.highestRaise.add(4 * column).readS32();
  if (level < lowest) {
    level = lowest;
  } else if (level > highest) {
    level = highest;
  } else {
    level += Math.floor(Math.random() * 2);
  }
  return Math.max(1, level);
}

// Whether a creature is dead or dying: its AI's state 9, its action 6 or its health 0 (0x549080's test).
function isDying(creature) {
  return creature.add(CREATURE.ai).readU16() === 9 || creature.add(CREATURE.action).readU16() === 6 ||
    creature.add(CREATURE.health).readS32() <= 0;
}

// The hostile creature alive nearest the hero.
function nearestHostile(h) {
  const heroId = h.add(CREATURE.id).readU32();
  const [hx, hy] = cellOf(h);
  let nearest = null;
  let best = Infinity;
  for (const o of creatures()) {
    if (o.equals(h) || o.add(CREATURE.health).readS32() <= 0 || isHostile(o, heroId) === 0) {
      continue;
    }
    const [x, y] = cellOf(o);
    const distance = Math.hypot(x - hx, y - hy);
    if (distance < best) {
      best = distance;
      nearest = o;
    }
  }
  return nearest;
}

// The hero's strike at a creature as 0x4EA3F0 sends it, unarmed: the event 5 from the attacker to the target, the
// strike's record of no weapon and a factor of 1, through the target's handler (+0x18 of its vtable).
function strike(attacker, target) {
  const event = Memory.alloc(0x40);
  const attackerId = attacker.add(CREATURE.id).readU32();
  const targetId = target.add(CREATURE.id).readU32();
  event.writePointer(WORLD.strikeEvent);
  event.add(0x04).writeU32(5);
  event.add(0x08).writeU32(attackerId);
  event.add(0x0c).writeU32(targetId);
  event.add(0x10).writeU32(targetId);
  event.add(0x1c).writeU32(attackerId);   // the record: the weapon's type and id (none), the attacker, ...
  event.add(0x30).writeFloat(1.0);        // ... and the factor
  const handler = new NativeFunction(target.readPointer().add(0x18).readPointer(), 'uint32', ['pointer', 'pointer'],
    'thiscall');
  handler(target, event);
}

// An object a drop made: its id, type and, for gold, its amount.
function describeMade(id) {
  const object = id > 0 ? objectById(id) : null;
  if (object === null) {
    return null;
  }
  const type = object.add(CREATURE.type).readU32();
  return { id, type, cell: cellOf(object), gold: type >= 5132 && type <= 5135 ? object.add(0x120).readU32() : null };
}

// The experience a level starts at: the end of the level before, `X(L) = ⌊(L⁴ + 40L³ + 150L² + 200L − 90)/100⌋·100`
// (0x57F6E0, docs/re/fight.md, "The hero's levels").
function levelStart(level) {
  if (level < 1 || level > MOST_LEVEL) {
    throw new Error(`a level from 1 to ${MOST_LEVEL}`);
  }
  const l = level - 1;
  return l === 0 ? 0 : Math.floor((l ** 4 + 40 * l ** 3 + 150 * l ** 2 + 200 * l - 90) / 100) * 100;
}

// The strikes' resolution as the commands want it:
// - the god mode's hero: a strike leaves its health as it was (the resolution sees a health no damage reaches, so that
//   it takes no death's branch), and any other loss is given back at the next frame, the regeneration's rise kept;
// - an immortal creature (`spawn --immortal`): the strike takes its damage, which the log tells, and its health is
//   put back as the resolution returns, before anything sees it fall.
function installStrikes() {
  Interceptor.attach(WORLD.strike, {
    onEnter() {
      if (world.god === null && world.immortal.size === 0) {
        return;
      }
      const defender = this.context.ecx.add(4).readPointer();
      if (defender.isNull()) {
        return;
      }
      const id = defender.add(CREATURE.id).readU32();
      if (world.god !== null && id === world.god.id) {
        this.kept = defender;
        this.health = defender.add(CREATURE.health).readS32();
        defender.add(CREATURE.health).writeS32(SPARED_HEALTH);
      } else if (world.immortal.has(id)) {
        this.immortal = defender;
        this.health = defender.add(CREATURE.health).readS32();
      }
    },
    onLeave() {
      if (this.kept !== undefined) {
        this.kept.add(CREATURE.health).writeS32(this.health);
      } else if (this.immortal !== undefined) {
        const after = this.immortal.add(CREATURE.health).readS32();
        if (after < this.health) {
          log('INFO', `immortal ${describeObject(this.immortal)}: hit ${this.health - after}` +
            (after === 0 ? ' or more (its health reached 0)' : ''));
        }
        this.immortal.add(CREATURE.health).writeS32(this.health);
      }
    },
  });
}

// A load or a new game makes a new hero of the same id (#565): the health kept is then the new one's own.
function keepGodHealth() {
  const god = world.god;
  const h = hero();
  if (god === null || h === null) {
    return;
  }
  const health = h.add(CREATURE.health).readS32();
  if (!h.equals(god.hero) || h.add(CREATURE.id).readU32() !== god.id) {
    Object.assign(god, { id: h.add(CREATURE.id).readU32(), hero: h, health });
    return;
  }
  if (health < god.health) {
    h.add(CREATURE.health).writeS32(Math.min(god.health, h.add(CREATURE.most).readS32()));
  } else {
    god.health = health;
  }
}

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

// The frame's function is replaced rather than hooked, the original called at the end: a function the jobs call from
// within a hook's callback would run past every hook of the agent (Frida's guard against recursion), the traces' and
// the strikes' too, while from a replacement it runs through them (`kill`'s drops and experience in the traces).
function installWorld() {
  const tick = new NativeFunction(WORLD.tick, 'uint32', ['pointer', 'uint32'], 'thiscall');
  world.tick = new NativeCallback((engine, argument) => {
    world.engine = engine;
    try {
      if (world.jobs.length > 0) {
        runWorldJobs(engine);
      }
      if (world.god !== null) {
        keepGodHealth();
      }
    } catch (e) {
      log('ERROR', `the frame's jobs: ${e.stack ?? e}`);
    }
    return tick(engine, argument);
  }, 'uint32', ['pointer', 'uint32'], 'thiscall');
  Interceptor.replace(WORLD.tick, world.tick);
  installStrikes();
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

// --- Traces --------------------------------------------------------------------------------------------------------

// A trace logs every call of a function of the original (or of mss32.dll), with the wall clock's time, the frame, the
// game's clock and the thread, its arguments read as their kinds say and its return value (docs/re/traces.md).
const traces = new Map();   // name -> {listeners, count, dropped, second, limit}
const scratch = Memory.alloc(8);

function asFloat(value) {
  scratch.writeS32(value.toInt32());
  return scratch.readFloat();
}

function describeObject(object) {
  if (object === null || object.isNull()) {
    return 'none';
  }
  try {
    const id = object.add(CREATURE.id).readU32();
    const [x, y] = cellOf(object);
    let text = `${id} type ${object.add(CREATURE.type).readU32()} cell ${x},${y}`;
    if (isCreature(object)) {
      text += ` health ${object.add(CREATURE.health).readS32()}/${object.add(CREATURE.most).readS32()}`;
    }
    return text;
  } catch (e) {
    return `${object}`;
  }
}

function describeId(id) {
  const object = id > 0 && id < objectCount() ? objectById(id) : null;
  return object === null ? `${id}` : describeObject(object);
}

// An argument by its kind: int, uint, hex, u16, i16, float, ptr, object (a pointer to an object), id (an object's id),
// cstr (a C string), out-float (a pointer whose float is read as the call returns).
function readArgument(kind, value) {
  switch (kind) {
    case 'int': return String(value.toInt32());
    case 'uint': return String(value.toUInt32());
    case 'hex': return `0x${value.toUInt32().toString(16)}`;
    case 'u16': return String(value.toUInt32() & 0xffff);
    case 'i16': return String((value.toInt32() << 16) >> 16);
    case 'float': return String(asFloat(value));
    case 'object': return describeObject(value);
    case 'id': return describeId(value.toUInt32());
    case 'cstr': return value.isNull() ? 'null' : JSON.stringify(value.readCString());
    default: return value.toString();
  }
}

function clockText() {
  try {
    const clock = WORLD.clock.readPointer();
    if (clock.isNull()) {
      return '';
    }
    const two = (n) => String(n).padStart(2, '0');
    return `${two(clock.add(CLOCK.hours).readU16())}:${two(clock.add(CLOCK.minutes).readU16())}:` +
      `${two(clock.add(CLOCK.seconds).readU16())}`;
  } catch (e) {
    return '';
  }
}

function record(trace, text) {
  const second = Math.floor(Date.now() / 1000);
  if (second !== trace.second) {
    if (trace.dropped > 0) {
      send({ type: 'trace', name: trace.name, message: `${trace.dropped} calls not logged (over ${trace.limit} a second)` });
    }
    trace.second = second;
    trace.inSecond = 0;
    trace.dropped = 0;
  }
  trace.inSecond += 1;
  trace.count += 1;
  if (trace.inSecond > trace.limit) {
    trace.dropped += 1;
    return;
  }
  send({
    type: 'trace', name: trace.name,
    message: `${text} | t ${now().toFixed(1)} ms, frame ${state.frames}, ${clockText()}, thread ${Process.getCurrentThreadId()}`,
  });
}

// The `this` (ECX) and the stack's arguments of a call by its ABI.
function callArguments(abi, context, args, count) {
  const out = [];
  if (abi === 'thiscall') {
    out.push(context.ecx);
    for (let i = 0; i < count - 1; i += 1) {
      out.push(args[i]);
    }
  } else if (abi === 'fastcall') {
    out.push(context.ecx, context.edx);
    for (let i = 0; i < count - 2; i += 1) {
      out.push(args[i]);
    }
    out.length = count;
  } else {
    for (let i = 0; i < count; i += 1) {
      out.push(args[i]);
    }
  }
  return out;
}

function objectOfFirst(abi, context, args) {
  return abi === 'cdecl' || abi === 'stdcall' ? args[0] : context.ecx;
}

// A trace of a function by its address: the kinds of its arguments (the first is ECX for thiscall and fastcall), of
// its return, and the ids of the objects whose calls are logged (the first argument's object), all if none.
function genericTrace(spec) {
  const kinds = spec.args ?? [];
  const ids = spec.ids?.length ? new Set(spec.ids) : null;
  return (trace) => ({
    onEnter(args) {
      this.skip = false;
      if (ids !== null) {
        try {
          const object = objectOfFirst(spec.abi, this.context, args);
          this.skip = !ids.has(object.add(CREATURE.id).readU32());
        } catch (e) {
          this.skip = true;
        }
      }
      if (this.skip) {
        return;
      }
      this.values = callArguments(spec.abi, this.context, args, kinds.length).map((v) => ptr(v.toString()));
      this.text = kinds.map((kind, i) => (kind === 'out-float' ? null : readArgument(kind, this.values[i])));
    },
    onLeave(result) {
      if (this.skip) {
        return;
      }
      const shown = kinds.map((kind, i) => {
        if (kind !== 'out-float') {
          return this.text[i];
        }
        return this.values[i].isNull() ? 'null' : `->${this.values[i].readFloat()}`;
      });
      const ret = spec.ret && spec.ret !== 'void' ? ` = ${readArgument(spec.ret, result)}` : '';
      record(trace, `(${shown.join(', ')})${ret}`);
    },
  });
}

// The health of a creature before and after a call whose ECX (or, through `via`, a pointer of it) is the creature.
function healthAround(object) {
  return object.add(CREATURE.health).readS32();
}

// The presets, by the functions of docs/re (docs/re/traces.md lists them with their docs).
const PRESETS = {
  // A strike's resolution (docs/re/fight.md): the defender is the creature of `this`'s +4, the attacker the id at the
  // event's +8; the health before and after gives the damage, 0 a miss or a strike that did nothing.
  strike: { address: '0x56b130', make: (trace) => ({
    onEnter(args) {
      this.defender = this.context.ecx.add(4).readPointer();
      this.attacker = args[0].add(8).readU32();
      this.before = healthAround(this.defender);
      this.spared = this.before === SPARED_HEALTH;
    },
    onLeave() {
      if (this.spared) {
        record(trace, `attacker ${describeId(this.attacker)} -> defender ${describeObject(this.defender)}: ` +
          'spared by the god mode');
        return;
      }
      const after = healthAround(this.defender);
      record(trace, `attacker ${describeId(this.attacker)} -> defender ${describeObject(this.defender)}: health ` +
        `${this.before} -> ${after}, ${this.before > after ? `hit ${this.before - after}` : 'no damage'}`);
    },
  }) },
  // The hit chance and a kind's damage through a resistance (0x43EEE0, cdecl): x, y, the base c, p, the share w out.
  chance: { address: '0x43eee0', abi: 'cdecl', args: ['float', 'float', 'float', 'float', 'out-float'] },
  // The dead creature's state (the AI's 9, every frame while dead): its first call for each creature.
  death: { address: '0x53e7c0', make: (trace) => {
    const seen = new Set();
    return {
      onEnter() {
        const creature = this.context.ecx;
        const id = creature.add(CREATURE.id).readU32();
        if (!seen.has(id)) {
          seen.add(id);
          record(trace, `dead ${describeObject(creature)} level ${creature.add(CREATURE.level).readU16()}`);
        }
      },
    };
  } },
  // A kill's experience (0x57D860), its return the experience earned.
  experience: { address: '0x57d860', abi: 'thiscall', args: ['ptr', 'u16', 'u16', 'int'], ret: 'int' },
  // One call of a killed creature's drop (docs/re/items.md): the creature (ECX), then on the stack the type asked (0
  // any), the bonus, the value and the killer's id; the objects it makes (sdk_CREATE) with their types and, for gold,
  // its amount (+0x120).
  drop: { address: '0x559bf0', make: (trace) => {
    const making = new Map();   // thread -> ids made
    const create = Interceptor.attach(ptr('0x5fba40'), {
      onLeave(result) {
        const made = making.get(Process.getCurrentThreadId());
        if (made !== undefined && result.toInt32() > 0) {
          made.push(result.toUInt32());
        }
      },
    });
    trace.listeners.push(create);
    return {
      onEnter(args) {
        this.creature = ptr(this.context.ecx.toString());
        this.text = `${describeObject(this.creature)}: type ${args[0].toUInt32()}, bonus ${args[1].toUInt32()}, ` +
          `value ${args[2].toUInt32()}, killer ${args[3].toUInt32()}`;
        making.set(Process.getCurrentThreadId(), []);
      },
      onLeave() {
        const made = making.get(Process.getCurrentThreadId()) ?? [];
        making.delete(Process.getCurrentThreadId());
        const items = made.map((id) => {
          const object = objectById(id);
          if (object === null) {
            return `${id}`;
          }
          const type = object.add(CREATURE.type).readU32();
          return type >= 5132 && type <= 5135 ? `${id} gold ${object.add(0x120).readU32()} (type ${type})`
            : `${id} type ${type}`;
        });
        record(trace, `${this.text} -> ${items.length ? items.join('; ') : 'nothing'}`);
      },
    };
  } },
  // The item maker (docs/re/items.md, 0x5D5080, cdecl): its type and level first.
  item: { address: '0x5d5080', abi: 'cdecl', args: ['uint', 'uint', 'int', 'hex'] },
  // The regeneration (docs/re/fight.md, "Health", every frame of every creature): a creature's health as it changes.
  regen: { address: '0x562730', make: (trace, spec) => {
    const ids = new Set(spec.ids?.length ? spec.ids : [1]);
    return {
      onEnter() {
        const creature = this.context.ecx;
        this.watch = ids.has(creature.add(CREATURE.id).readU32());
        if (this.watch) {
          this.creature = ptr(creature.toString());
          this.before = healthAround(creature);
        }
      },
      onLeave() {
        if (this.watch) {
          const after = healthAround(this.creature);
          if (after !== this.before) {
            record(trace, `${describeObject(this.creature)}: ${this.before} -> ${after}`);
          }
        }
      },
    };
  } },
  // The creatures' decisions (docs/re/ai.md): woken by sdk_WAKE, the hero's look about, the idle action's roll.
  wake: { address: '0x59f580', abi: 'fastcall', args: ['object'] },
  look: { address: '0x547070', abi: 'thiscall', args: ['object', 'hex'], ret: 'hex' },
  idle: { address: '0x54ea30', abi: 'fastcall', args: ['object'] },
  // A creature's search for a door in its way (docs/re/fight.md, "Doors in the way"), and a door's turn by an event:
  // the event's type (+4) and the door's trigger's state (+0x34, its word +0x0A) before and after.
  'door-search': { address: '0x4d4800', abi: 'fastcall', args: ['object'], ret: 'int' },
  door: { address: '0x44bcd0', make: (trace) => ({
    onEnter(args) {
      // The objects' event handler: the doors alone, items.pak's 5650 to 5766 (docs/re/ai.md, "Houses and doors").
      const type = this.context.ecx.add(CREATURE.type).readU32();
      this.skip = type < 5650 || type > 5766;
      if (this.skip) {
        return;
      }
      this.door = ptr(this.context.ecx.toString());
      this.event = args[0].add(4).readU32();
      const trigger = this.door.add(0x34).readPointer();
      this.trigger = trigger;
      this.before = trigger.isNull() ? null : trigger.add(0x0a).readU16();
    },
    onLeave() {
      if (this.skip) {
        return;
      }
      const after = this.trigger.isNull() ? null : this.trigger.add(0x0a).readU16();
      record(trace, `door ${describeObject(this.door)} event 0x${this.event.toString(16)}: state ` +
        `${this.before === null ? 'none' : `0x${this.before.toString(16)}`} -> ${after === null ? 'none' : `0x${after.toString(16)}`}`);
    },
  }) },
  // A trigger's state set or reset (cTrigger's 0x415F00 and 0x416280, thiscall: the bits, whether to send it on),
  // which turns a door whatever turns it: the objects' events (0x44BCD0) and the engine's (cEngine_receiveEvent,
  // the hero's click among them). The trigger's id (+0), the object it serves (+6), the bits, its word (+0x0A)
  // before and after.
  'trigger-set': { address: '0x415f00', make: (trace) => triggerState(trace, 'sets') },
  'trigger-reset': { address: '0x416280', make: (trace) => triggerState(trace, 'resets') },
  // The sounds (docs/re/sound.md): a variant of a profile's group, its id returned; the sound manager's events; the
  // samples and streams mss32.dll opens and starts.
  'sound-pick': { address: '0x4261d0', abi: 'thiscall', args: ['ptr', 'int', 'int'], ret: 'uint' },
  'sound-event': { address: '0x67a7a0', abi: 'thiscall', args: ['ptr', 'int', 'int', 'int', 'int'], limit: 20 },
  'sample-file': { module: 'mss32.dll', export: '_AIL_set_named_sample_file@20', abi: 'stdcall', args: ['ptr', 'cstr', 'ptr', 'uint'] },
  'sample-3d-file': { module: 'mss32.dll', export: '_AIL_set_3D_sample_file@8', abi: 'stdcall', args: ['ptr', 'ptr'] },
  'sample-start': { module: 'mss32.dll', export: '_AIL_start_sample@4', abi: 'stdcall', args: ['ptr'] },
  'sample-3d-start': { module: 'mss32.dll', export: '_AIL_start_3D_sample@4', abi: 'stdcall', args: ['ptr'] },
  stream: { module: 'mss32.dll', export: '_AIL_open_stream@12', abi: 'stdcall', args: ['ptr', 'cstr', 'int'], ret: 'ptr' },
};
// The listeners of a trigger's state set or reset (`verb`): the trigger, the object it serves, the bits and the
// state's word before and after.
function triggerState(trace, verb) {
  return {
    onEnter(args) {
      this.trigger = ptr(this.context.ecx.toString());
      this.bits = args[0].toUInt32() & 0xffff;
      this.before = this.trigger.add(0x0a).readU16();
    },
    onLeave() {
      const id = this.trigger.readU32();
      const object = this.trigger.add(6).readU32();
      const after = this.trigger.add(0x0a).readU16();
      record(trace, `trigger ${id} of object ${object} ${verb} 0x${this.bits.toString(16)}: state ` +
        `0x${this.before.toString(16)} -> 0x${after.toString(16)}`);
    },
  };
}

const GROUPS = {
  fight: ['strike', 'chance', 'death', 'experience'],
  drops: ['drop', 'item'],
  ai: ['wake', 'look', 'idle'],
  doors: ['door-search', 'door', 'trigger-set', 'trigger-reset'],
  sound: ['sound-pick', 'sample-file', 'sample-3d-file', 'sample-start', 'sample-3d-start', 'stream'],
};

function startTrace(spec) {
  const preset = PRESETS[spec.target];
  const name = spec.name ?? (preset ? spec.target : String(spec.target));
  if (traces.has(name)) {
    throw new Error(`a trace ${name} runs already`);
  }
  let target;
  let make;
  if (preset !== undefined) {
    target = preset.module !== undefined ? Process.getModuleByName(preset.module).getExportByName(preset.export)
      : ptr(preset.address);
    make = preset.make ?? genericTrace({ ...preset, ids: spec.ids });
  } else {
    const address = ptr(String(spec.target));
    if (Process.findModuleByAddress(address) === null) {
      throw new Error(`${spec.target} is neither a preset nor an address of a module`);
    }
    target = address;
    make = genericTrace({ abi: spec.abi ?? 'cdecl', args: spec.args ?? [], ret: spec.ret, ids: spec.ids });
  }
  const limit = spec.limit ?? preset?.limit ?? 200;
  const trace = { name, target, listeners: [], count: 0, dropped: 0, second: 0, inSecond: 0, limit };
  trace.listeners.push(Interceptor.attach(target, make(trace, spec)));
  traces.set(name, trace);
  return { name, target: target.toString() };
}

function stopTrace(name) {
  const trace = traces.get(name);
  if (trace === undefined) {
    return false;
  }
  for (const listener of trace.listeners) {
    listener.detach();
  }
  traces.delete(name);
  return true;
}

// --- The mute ------------------------------------------------------------------------------------------------------

// With the host's --muted, the game keeps its volumes and so plays its sounds, for their traces, while mss32.dll
// outputs nothing: every volume the game sets on a sample, a 3D sample or a stream is 0, each is set to 0 again as it
// starts, and so is the digital driver's master volume as it opens.
const mute = { set: 0 };

function installMute() {
  const mss = Module.load('mss32.dll');
  const exp = (name) => mss.getExportByName(name);
  const zero = (indices) => ({
    onEnter(args) {
      for (const i of indices) {
        args[i] = ptr(0);   // a float's 0.0
      }
      mute.set += 1;
    },
  });
  const volumes = {
    '_AIL_set_sample_volume_pan@12': [1], '_AIL_set_sample_volume_levels@12': [1, 2],
    '_AIL_set_3D_sample_volume@8': [1], '_AIL_set_stream_volume_pan@12': [1],
    '_AIL_set_stream_volume_levels@12': [1, 2], '_AIL_set_digital_master_volume_level@8': [1],
    '_AIL_redbook_set_volume_level@8': [1], '_AIL_set_XMIDI_master_volume@8': [1],
  };
  for (const [name, indices] of Object.entries(volumes)) {
    Interceptor.attach(exp(name), zero(indices));
  }
  const setSample = new NativeFunction(exp('_AIL_set_sample_volume_pan@12'), 'void', ['pointer', 'float', 'float'], 'stdcall');
  const set3D = new NativeFunction(exp('_AIL_set_3D_sample_volume@8'), 'void', ['pointer', 'float'], 'stdcall');
  const setStream = new NativeFunction(exp('_AIL_set_stream_volume_pan@12'), 'void', ['pointer', 'float', 'float'], 'stdcall');
  const setMaster = new NativeFunction(exp('_AIL_set_digital_master_volume_level@8'), 'void', ['pointer', 'float'], 'stdcall');
  const before = (name, set) => Interceptor.attach(exp(name), {
    onEnter(args) {
      if (!args[0].isNull()) {
        set(args[0]);
      }
    },
  });
  before('_AIL_start_sample@4', (s) => setSample(s, 0, 0.5));
  before('_AIL_resume_sample@4', (s) => setSample(s, 0, 0.5));
  before('_AIL_start_3D_sample@4', (s) => set3D(s, 0));
  before('_AIL_resume_3D_sample@4', (s) => set3D(s, 0));
  before('_AIL_start_stream@4', (s) => setStream(s, 0, 0.5));
  Interceptor.attach(exp('_AIL_open_digital_driver@16'), {
    onLeave(driver) {
      if (!driver.isNull()) {
        setMaster(driver, 0);
        log('INFO', 'mute: the digital driver opened, its master volume 0');
      }
    },
  });
  Interceptor.attach(exp('_AIL_waveOutOpen@16'), {
    onEnter(args) {
      this.out = args[0];
    },
    onLeave() {
      const driver = this.out.isNull() ? ptr(0) : this.out.readPointer();
      if (!driver.isNull()) {
        setMaster(driver, 0);
        log('INFO', 'mute: the wave-out driver opened, its master volume 0');
      }
    },
  });
  guards.mute = true;
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
      // In the maintainer's play the game takes its first activation and no other: full screen, it answers each with
      // its surfaces restored, the display mode set again and its textures and models flushed, which took the
      // foreground back from the first Alt-Tab and ended the game after a few (#551); dgVoodoo2 loses no surface.
      // The system's input passes, as the keys come to the window with the foreground alone.
      if (config.interactive) {
        if (message === WM.ACTIVATEAPP) {
          if (activated || args[2].toUInt32() === 0) {
            args[1] = ptr(WM.NULL);
          } else {
            activated = true;
          }
        } else if (message === WM.KILLFOCUS) {
          args[1] = ptr(WM.NULL);
        }
        return;
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
      if (this.message === WM.MOUSEACTIVATE && !config.interactive) {
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
let activated = false;  // in the maintainer's play, whether the game has taken its one activation

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
  // The process's exceptions, the first ones logged with where they happened, so that a crash shows in the log before
  // the process goes; the game's own handlers still take them. Those of no kind Frida names (`system`) are raised on
  // purpose, at each start too (KERNELBASE's RaiseException).
  let exceptions = 0;
  Process.setExceptionHandler((details) => {
    if (exceptions < 20) {
      exceptions += 1;
      const module = Process.findModuleByAddress(details.address);
      const where = module === null ? `${details.address}` :
        `${module.name}+0x${details.address.sub(module.base).toString(16)}`;
      const memory = details.memory ? `, ${details.memory.operation} at ${details.memory.address}` : '';
      log(details.type === 'system' ? 'DEBUG' : 'WARN', `exception ${details.type} at ${where}${memory}`);
    }
    return false;
  });
  // The game or a library of its may end the process past the C runtime, as the full screen's way out does.
  Interceptor.attach(kernel32.getExportByName('ExitProcess'), {
    onEnter(args) {
      if (!state.ending) {
        state.exiting = true;
        log('INFO', `ExitProcess(${args[0].toUInt32()})`);
      }
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
    if (config.muted) {
      installMute();
    }
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
      interactive: config.interactive,
      player_has_window: playerHasWindow(),
      refusals,
      muted: config.muted ? mute.set : null,
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
  trace(spec) {
    const group = GROUPS[spec.target];
    if (group !== undefined && spec.name === undefined) {
      return group.filter((name) => !traces.has(name)).map((name) => startTrace({ ...spec, target: name }));
    }
    return [startTrace(spec)];
  },
  untrace(name) {
    if (name === 'all') {
      const names = [...traces.keys()];
      names.forEach(stopTrace);
      return names;
    }
    const group = GROUPS[name];
    if (group !== undefined) {
      return group.filter(stopTrace);
    }
    return stopTrace(name) ? [name] : [];
  },
  traces() {
    return [...traces.values()].map((t) => ({ name: t.name, target: t.target.toString(), calls: t.count }));
  },
  presets() {
    return { presets: Object.keys(PRESETS), groups: GROUPS };
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
