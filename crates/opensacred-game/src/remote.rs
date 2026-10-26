//! Remote control (docs/10 §8): with `--remote [PORT]`, Bevy's Remote Protocol, JSON-RPC 2.0 over
//! HTTP on the loopback alone (`127.0.0.1`, port 15702 by default), so that Claude Code and
//! scripts drive a running game. Off unless the command line asks, and never from the settings.
//!
//! The game's own methods, beside Bevy's (`rpc.discover`, `world.query`, …):
//!
//! - `opensacred/exec {"line": …}`: runs a command (`command`) and gives its output lines;
//! - `opensacred/state`: the overlay's values (`overlay::Snapshot`);
//! - `opensacred/input {…}`: the player's input as if from the system, one of `{"key": "F11"}`
//!   (or keys held together, `{"key": "Control+V"}`),
//!   `{"text": "zoom 2"}`, `{"move": [x, y]}`, `{"click": [x, y], "button": "left", "hold": 2}`
//!   or `{"wheel": 1}`, in the window's pixels: the keys and clicks go through the game's own code,
//!   the menus' and the console's too, and the system's cursor stays where it is;
//! - `opensacred/screenshot {"path": …}`: saves the next frame; the client waits for the file;
//! - `opensacred/logs {"after": n}`: the log's records numbered `n` and later.
//!
//! Bevy's `world.mutate_components` goes through the game's `mutate_components`, which answers an
//! error for an entity that is gone instead of ending the game (#456).

use std::collections::VecDeque;

use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::prelude::*;
use bevy::remote::builtin_methods::{
    BRP_MUTATE_COMPONENTS_METHOD, BrpMutateComponentsParams, parse_some,
    process_remote_mutate_components_request,
};
use bevy::remote::http::{HostPort, RemoteHttpPlugin};
use bevy::remote::{BrpError, BrpResult, RemotePlugin, error_codes};
use bevy::render::RenderApp;
use bevy::window::{PrimaryWindow, WindowEvent};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::journal::Journal;
use crate::overlay::Probe;
use crate::{command, screenshot};

/// A failure of the game's, as the protocol gives it.
fn failure(message: impl Into<String>) -> BrpError {
    BrpError {
        code: error_codes::INTERNAL_ERROR,
        message: message.into(),
        data: None,
    }
}

/// The parameters of a method.
fn parameters<T: DeserializeOwned>(params: Option<Value>) -> Result<T, BrpError> {
    serde_json::from_value(params.unwrap_or(Value::Null)).map_err(|e| BrpError {
        code: error_codes::INVALID_PARAMS,
        message: e.to_string(),
        data: None,
    })
}

#[derive(Deserialize)]
struct Line {
    line: String,
}

/// `opensacred/exec`: a command, logged under `remote`.
fn exec(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Line { line } = parameters(params)?;
    info!(target: "remote", "> {line}");
    match command::run(world, &line) {
        Ok(outcome) if outcome.wait.is_some() => Err(failure(
            "wait is for scripts: poll opensacred/state instead",
        )),
        Ok(outcome) => {
            for line in &outcome.lines {
                info!(target: "remote", "{line}");
            }
            Ok(json!({ "output": outcome.lines }))
        }
        Err(e) => Err(failure(e)),
    }
}

/// `opensacred/state`.
fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let snapshot = world
        .run_system_cached(|probe: Probe| probe.snapshot())
        .map_err(|e| failure(e.to_string()))?;
    serde_json::to_value(snapshot).map_err(|e| failure(e.to_string()))
}

#[derive(Deserialize)]
struct Path {
    path: std::path::PathBuf,
}

/// `opensacred/screenshot`.
fn save(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Path { path } = parameters(params)?;
    screenshot::save(world, path.clone());
    Ok(json!({ "path": path }))
}

#[derive(Deserialize, Default)]
struct After {
    #[serde(default)]
    after: u64,
}

/// `opensacred/logs`.
fn logs(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let After { after } = if params.is_some() {
        parameters(params)?
    } else {
        After::default()
    };
    let records = world.resource::<Journal>().since(after);
    Ok(Value::Array(
        records
            .into_iter()
            .map(|record| {
                json!({
                    "sequence": record.sequence,
                    "seconds": record.seconds,
                    "level": record.level.to_string(),
                    "target": record.target,
                    "message": record.message,
                })
            })
            .collect(),
    ))
}

/// `world.mutate_components`, Bevy's own once its entity is found: `bevy_remote` 0.20.0 takes the
/// entity with `World::entity_mut`, which panics on one despawned (a sector unloaded under the
/// client), where its other methods answer `ENTITY_NOT_FOUND`, as this does.
fn mutate_components(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let BrpMutateComponentsParams { entity, .. } = parse_some(params.clone())?;
    if world.get_entity(entity).is_err() {
        return Err(BrpError::entity_not_found(entity));
    }
    process_remote_mutate_components_request(In(params), world)
}

/// The player's input of `opensacred/input`: one of these.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Input {
    /// A key, by its place: `F11`, `Escape`, `Enter`, `R`, `Backquote`, `ArrowUp`, …, or keys held
    /// together: `Control+V`, `Shift+R`.
    key: Option<String>,
    /// Text typed a character at a time, as the console takes it.
    text: Option<String>,
    /// The cursor moved to a pixel of the window.
    #[serde(rename = "move")]
    to: Option<[f32; 2]>,
    /// The cursor moved to a pixel of the window and a button pressed there, held `hold` frames.
    click: Option<[f32; 2]>,
    /// `left` (by default), `right` or `middle`.
    button: Option<String>,
    hold: Option<u32>,
    /// The wheel turned by lines, up positive.
    wheel: Option<f32>,
}

/// A message of input waiting for its frame.
enum Message {
    Key(KeyboardInput),
    Button(MouseButtonInput),
    Moved(CursorMoved),
    Wheel(MouseWheel),
}

/// The input sent and the frames before each message, so that a key or a button is held at least
/// a frame, as the game's systems read a button held.
#[derive(Resource, Default)]
struct Pending(VecDeque<(u32, Message)>);

/// A key by its place on the keyboard, and what it means and types in an English layout.
fn key(name: &str) -> Option<(KeyCode, Key, Option<&'static str>)> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    const LOWER: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ];
    const FUNCTIONS: [(KeyCode, Key); 12] = [
        (KeyCode::F1, Key::F1),
        (KeyCode::F2, Key::F2),
        (KeyCode::F3, Key::F3),
        (KeyCode::F4, Key::F4),
        (KeyCode::F5, Key::F5),
        (KeyCode::F6, Key::F6),
        (KeyCode::F7, Key::F7),
        (KeyCode::F8, Key::F8),
        (KeyCode::F9, Key::F9),
        (KeyCode::F10, Key::F10),
        (KeyCode::F11, Key::F11),
        (KeyCode::F12, Key::F12),
    ];
    let named = match name {
        "Escape" => Some((KeyCode::Escape, Key::Escape, None)),
        "Enter" => Some((KeyCode::Enter, Key::Enter, None)),
        "Tab" => Some((KeyCode::Tab, Key::Tab, None)),
        "Space" => Some((KeyCode::Space, Key::Space, Some(" "))),
        "Backspace" => Some((KeyCode::Backspace, Key::Backspace, None)),
        "Delete" => Some((KeyCode::Delete, Key::Delete, None)),
        "Home" => Some((KeyCode::Home, Key::Home, None)),
        "End" => Some((KeyCode::End, Key::End, None)),
        "PageUp" => Some((KeyCode::PageUp, Key::PageUp, None)),
        "PageDown" => Some((KeyCode::PageDown, Key::PageDown, None)),
        "ArrowUp" => Some((KeyCode::ArrowUp, Key::ArrowUp, None)),
        "ArrowDown" => Some((KeyCode::ArrowDown, Key::ArrowDown, None)),
        "ArrowLeft" => Some((KeyCode::ArrowLeft, Key::ArrowLeft, None)),
        "ArrowRight" => Some((KeyCode::ArrowRight, Key::ArrowRight, None)),
        "Backquote" => Some((KeyCode::Backquote, Key::Character("`".into()), Some("`"))),
        "Control" | "Ctrl" => Some((KeyCode::ControlLeft, Key::Control, None)),
        "Shift" => Some((KeyCode::ShiftLeft, Key::Shift, None)),
        "Alt" => Some((KeyCode::AltLeft, Key::Alt, None)),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<usize>().ok())
        && let Some((code, key)) = n.checked_sub(1).and_then(|i| FUNCTIONS.get(i))
    {
        return Some((*code, key.clone(), None));
    }
    let letter = name.strip_prefix("Key").unwrap_or(name);
    let mut chars = letter.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => {
            let i = usize::from(u8::try_from(c.to_ascii_lowercase()).ok()? - b'a');
            Some((LETTERS[i], Key::Character(LOWER[i].into()), Some(LOWER[i])))
        }
        _ => None,
    }
}

/// `opensacred/input`.
fn input(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let input: Input = parameters(params)?;
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let window = windows
        .iter(world)
        .next()
        .ok_or_else(|| failure("no window"))?;
    let mut sent = Vec::new();
    let keyboard = |code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState| {
        Message::Key(KeyboardInput {
            key_code: code,
            logical_key: logical,
            state,
            text: (state == ButtonState::Pressed)
                .then_some(text)
                .flatten()
                .map(Into::into),
            repeat: false,
            window,
        })
    };
    if let Some(names) = &input.key {
        // `Control+V`: the keys pressed in order, and released together a frame later.
        let mut chord = Vec::new();
        for name in names.split('+') {
            chord.push(key(name).ok_or_else(|| failure(format!("no key {name}")))?);
        }
        // A key held with Control or Alt types nothing.
        let typing = chord.len() == 1
            || chord[..chord.len() - 1]
                .iter()
                .all(|k| k.0 == KeyCode::ShiftLeft);
        for (code, logical, text) in &chord {
            let text = if typing { *text } else { None };
            sent.push((
                0,
                keyboard(*code, logical.clone(), text, ButtonState::Pressed),
            ));
        }
        for (code, logical, text) in chord.into_iter().rev() {
            sent.push((1, keyboard(code, logical, text, ButtonState::Released)));
        }
    }
    if let Some(text) = &input.text {
        for c in text.chars() {
            let s = c.to_string();
            sent.push((
                0,
                keyboard(
                    KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
                    Key::Character(s.as_str().into()),
                    Some(&s),
                    ButtonState::Pressed,
                ),
            ));
        }
    }
    let moved = |[x, y]: [f32; 2]| {
        Message::Moved(CursorMoved {
            window,
            position: Vec2::new(x, y),
            delta: None,
        })
    };
    if let Some(to) = input.to {
        sent.push((0, moved(to)));
    }
    if let Some(at) = input.click {
        let button = match input.button.as_deref().unwrap_or("left") {
            "left" => MouseButton::Left,
            "right" => MouseButton::Right,
            "middle" => MouseButton::Middle,
            other => return Err(failure(format!("no button {other}"))),
        };
        let press = |state| {
            Message::Button(MouseButtonInput {
                button,
                state,
                window,
            })
        };
        // Moved a frame before the press, so that what lies under the cursor is known.
        sent.push((0, moved(at)));
        sent.push((1, press(ButtonState::Pressed)));
        sent.push((
            1 + input.hold.unwrap_or(2).max(1),
            press(ButtonState::Released),
        ));
    }
    if let Some(lines) = input.wheel {
        sent.push((
            0,
            Message::Wheel(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: lines,
                window,
                phase: TouchPhase::Moved,
            }),
        ));
    }
    if sent.is_empty() {
        return Err(failure("no input: give key, text, move, click or wheel"));
    }
    let count = sent.len();
    world.resource_mut::<Pending>().0.extend(sent);
    Ok(json!({ "messages": count }))
}

/// Sends the input whose frame has come, before Bevy reads the frame's input: the mouse's as the
/// window's events too, which Bevy's picking reads alone, so that the interface's nodes see it
/// (`bevy_picking::input::mouse_pick_events`, #1121).
fn send(
    mut pending: ResMut<Pending>,
    mut keys: MessageWriter<KeyboardInput>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut moves: MessageWriter<CursorMoved>,
    mut wheels: MessageWriter<MouseWheel>,
    mut events: MessageWriter<WindowEvent>,
) {
    let mut later = VecDeque::new();
    for (frames, message) in pending.0.drain(..) {
        if frames > 0 {
            later.push_back((frames - 1, message));
            continue;
        }
        match message {
            Message::Key(key) => {
                keys.write(key);
            }
            Message::Button(button) => {
                events.write(WindowEvent::MouseButtonInput(button));
                buttons.write(button);
            }
            Message::Moved(moved) => {
                events.write(WindowEvent::CursorMoved(moved.clone()));
                moves.write(moved);
            }
            Message::Wheel(wheel) => {
                events.write(WindowEvent::MouseWheel(wheel));
                wheels.write(wheel);
            }
        }
    }
    pending.0 = later;
}

/// Bevy's Remote Protocol on `port` of the loopback, with the game's methods.
pub struct RemoteControlPlugin {
    pub port: u16,
}

impl Plugin for RemoteControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RemotePlugin::default()
                .with_method_main("opensacred/exec", exec)
                .with_method_main("opensacred/state", state)
                .with_method_main("opensacred/input", input)
                .with_method_main("opensacred/screenshot", save)
                .with_method_main("opensacred/logs", logs)
                // Added after Bevy's methods, it takes the name's place among them.
                .with_method_main(BRP_MUTATE_COMPONENTS_METHOD, mutate_components),
            RemoteHttpPlugin::default().with_port(self.port),
        ))
        .init_resource::<Pending>()
        .add_systems(PreUpdate, send.before(InputSystems));
        // The plugin also serves the render sub-app, on 15703 unless told otherwise, which is the
        // original's driver's port (docs/03 §3.8, #528): on a port the system picks instead, as
        // nothing calls it.
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.insert_resource(HostPort(0));
        }
        info!("remote control on 127.0.0.1:{}", self.port);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::game::AppState;

    #[test]
    fn the_mouse_reaches_the_window_events_that_the_picking_reads() {
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<CursorMoved>()
            .add_message::<MouseWheel>()
            .add_message::<WindowEvent>()
            .init_resource::<Pending>()
            .add_systems(Update, send);
        let window = Entity::PLACEHOLDER;
        app.world_mut().resource_mut::<Pending>().0.extend([
            (
                0,
                Message::Moved(CursorMoved {
                    window,
                    position: Vec2::new(512.0, 287.0),
                    delta: None,
                }),
            ),
            (
                1,
                Message::Button(MouseButtonInput {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    window,
                }),
            ),
        ]);
        let events = |app: &mut App| {
            app.update();
            let messages = app.world().resource::<Messages<WindowEvent>>();
            messages
                .iter_current_update_messages()
                .map(|event| match event {
                    WindowEvent::CursorMoved(moved) => format!("moved {}", moved.position),
                    WindowEvent::MouseButtonInput(button) => format!("{:?}", button.state),
                    _ => "other".to_owned(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(events(&mut app), ["moved [512, 287]"]);
        assert_eq!(events(&mut app), ["Pressed"]);
    }

    #[test]
    fn keys_are_named_by_their_place() {
        assert_eq!(key("F11").map(|k| k.0), Some(KeyCode::F11));
        assert_eq!(
            key("R").map(|k| (k.0, k.2)),
            Some((KeyCode::KeyR, Some("r")))
        );
        assert_eq!(key("KeyQ").map(|k| k.0), Some(KeyCode::KeyQ));
        assert_eq!(key("Backquote").map(|k| k.0), Some(KeyCode::Backquote));
        assert_eq!(key("Control").map(|k| k.0), Some(KeyCode::ControlLeft));
        assert_eq!(key("F13"), None);
        assert_eq!(key("Ж"), None);
    }

    /// Posts a JSON-RPC request to the game and gives its answer.
    fn call(port: u16, method: &str, params: Value) -> Value {
        let body =
            json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
        let start = Instant::now();
        let mut stream = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => break stream,
                Err(e) if start.elapsed() < Duration::from_secs(20) => {
                    let _ = e;
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("connecting: {e}"),
            }
        };
        write!(
            stream,
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        let (head, mut rest) = answer.split_once("\r\n\r\n").unwrap();
        let mut body = String::new();
        if head.to_lowercase().contains("transfer-encoding: chunked") {
            // Chunks: a size in hexadecimal, then that many bytes, until a size of 0.
            while let Some((size, after)) = rest.split_once("\r\n") {
                let size = usize::from_str_radix(size.trim(), 16).unwrap();
                if size == 0 {
                    break;
                }
                body.push_str(&after[..size]);
                rest = &after[size + 2..];
            }
        } else {
            body.push_str(rest);
        }
        serde_json::from_str(body.trim()).unwrap_or_else(|e| panic!("{e}: {answer:?}"))
    }

    #[test]
    fn the_render_sub_app_keeps_off_the_originals_port() {
        let mut app = App::new();
        let mut render = SubApp::new();
        render.init_resource::<bevy::render::RenderScheduleOrder>();
        app.insert_sub_app(RenderApp, render);
        app.add_plugins(RemoteControlPlugin { port: 15702 });
        let render = app.sub_app(RenderApp).world();
        assert_eq!(render.resource::<HostPort>().0, 0);
        assert_eq!(app.world().resource::<HostPort>().0, 15702);
    }

    #[test]
    fn a_client_runs_commands_and_reads_the_state_over_http() {
        let port = 20_000 + u16::try_from(std::process::id() % 10_000).unwrap();
        let journal = Journal::new(None);
        journal.push(
            bevy::log::Level::INFO,
            "opensacred_game::sectors",
            "sector 50,39".into(),
        );
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            crate::overlay::OverlayPlugin,
            command::CommandPlugin,
        ))
        .insert_state(AppState::Menu)
        .insert_resource(journal)
        .add_plugins(bevy::input::InputPlugin)
        .add_message::<CursorMoved>()
        .add_message::<WindowEvent>()
        .add_plugins(RemoteControlPlugin { port });
        let (file, settings) = crate::settings::SettingsFile::read(Some(
            std::env::temp_dir().join(format!("opensacred-game-remote-{port}.toml")),
        ));
        app.insert_resource(file).insert_resource(settings);
        app.world_mut().spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection::default_3d()),
        ));
        app.register_type::<Transform>();
        let here = app.world_mut().spawn(Transform::default()).id();
        // As a sector's object after the sector has unloaded (#456).
        let gone = app.world_mut().spawn(Transform::default()).id();
        app.world_mut().despawn(gone);
        let mutate = move |entity: Entity| {
            call(
                port,
                "world.mutate_components",
                json!({
                    "entity": entity,
                    "component": "bevy_transform::components::transform::Transform",
                    "path": ".translation",
                    "value": [1.0, 2.0, 3.0],
                }),
            )
        };
        let client = std::thread::spawn(move || {
            (
                call(port, "opensacred/exec", json!({ "line": "zoom 1.5" })),
                call(port, "opensacred/state", Value::Null),
                call(port, "opensacred/exec", json!({ "line": "wait ready" })),
                call(port, "opensacred/logs", json!({ "after": 0 })),
                mutate(here),
                mutate(gone),
            )
        });
        let start = Instant::now();
        while !client.is_finished() {
            assert!(start.elapsed() < Duration::from_secs(30), "no answer");
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
        let (zoom, state, wait, logs, moved, missed) = client.join().unwrap();
        assert_eq!(zoom["result"]["output"], json!(["zoom 1.50"]));
        assert_eq!(state["result"]["state"], "menu");
        assert!(state["result"]["version"].is_string());
        assert!(
            wait["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("wait is for scripts")
        );
        let messages: Vec<&str> = logs["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|record| record["message"].as_str())
            .collect();
        assert!(messages.contains(&"sector 50,39"), "{messages:?}");
        assert_eq!(logs["result"][0]["level"], "INFO");
        assert_eq!(moved["result"], Value::Null, "{moved}");
        assert_eq!(
            app.world().get::<Transform>(here).map(|t| t.translation),
            Some(Vec3::new(1.0, 2.0, 3.0))
        );
        assert_eq!(
            missed["error"]["code"],
            json!(error_codes::ENTITY_NOT_FOUND),
            "{missed}"
        );
    }
}
