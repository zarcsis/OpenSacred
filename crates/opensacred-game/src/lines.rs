//! The lines over the creatures (#270): a line of the game's texts that a creature shows over
//! itself for a while, in a colour, as the original's children and the Daemon's onlookers do
//! (the engine's event of type 6; docs/re/ai.md, "The townsfolk"). The texts come from
//! `text/en-US/global.ftl` by name (`opensacred_data::text`).

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use opensacred_data::text;

use crate::files;
use crate::frame::Step;
use crate::game::{AppState, FONT};

/// How long a line shows, in seconds, and how high over a creature's feet, in world units
/// (**hypothesis**: the event 6's time and place are not read).
const SHOWS: f32 = 3.0;
const ABOVE: f32 = 150.0;
const FONT_SIZE: f32 = 14.0;
/// How wide a line's box is, in logical pixels, centred over the creature.
const WIDTH: f32 = 320.0;

/// The game's texts, by their ids.
#[derive(Resource, Default)]
pub struct Texts(HashMap<u32, String>);

impl Texts {
    /// The texts of `assets`, none where they cannot be read.
    pub fn read(assets: &Path) -> Self {
        let path = assets.join(text::GLOBAL);
        match files::read_to_string(&path) {
            Ok(ftl) => Self(parse(&ftl)),
            Err(e) => {
                warn!("no lines over the creatures: {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// The texts of a Fluent file as the converter writes it.
    #[cfg(test)]
    pub fn of(ftl: &str) -> Self {
        Self(parse(ftl))
    }

    /// The text named `name`.
    pub fn named(&self, name: &str) -> Option<&str> {
        self.0
            .get(&text::sacred_hash(name.as_bytes()))
            .map(String::as_str)
    }
}

/// The messages `id-N = text` of a Fluent file as the converter writes it
/// (`opensacred_convert::text::pattern`), their string literals (`{" "}`, which keep the spaces
/// at a line's ends, the braces and an empty line) undone, and a message's continued lines
/// (`\n    `) joined by their line breaks (#1030).
fn parse(ftl: &str) -> HashMap<u32, String> {
    let mut texts = HashMap::new();
    let mut current: Option<(u32, String)> = None;
    for line in ftl.lines() {
        if let Some(continued) = line.strip_prefix("    ")
            && let Some((_, text)) = current.as_mut()
        {
            text.push('\n');
            text.push_str(&unquote(continued));
            continue;
        }
        texts.extend(current.take());
        current = line
            .strip_prefix("id-")
            .and_then(|rest| rest.split_once(" = "))
            .and_then(|(id, value)| Some((id.parse().ok()?, unquote(value))));
    }
    texts.extend(current);
    texts
}

/// A line of a Fluent pattern with its string literals (`{"…"}`) undone.
fn unquote(value: &str) -> String {
    let mut text = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("{\"") {
        text.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("\"}") else {
            break;
        };
        text.push_str(&after[..end]);
        rest = &after[end + 2..];
    }
    text.push_str(rest);
    text
}

/// The lines a creature shows over itself, one under another in the order they came (the gold's
/// and the experience's of a reward, seen 2026-10-28, #1386): each its text, its colour, how long
/// it still shows, and its label once it has one. Lines in their place take their labels away
/// ([`replaced`]); [`Says::push`] adds one under them.
#[derive(Component)]
pub struct Says {
    lines: Vec<Said>,
}

/// One of a creature's lines.
struct Said {
    line: Line,
    color: Color,
    left: f32,
    label: Option<Entity>,
}

/// A line's text: one of the game's texts by its name, or a text of its own (a number).
enum Line {
    Named(String),
    Own(String),
}

impl Says {
    /// The line of the text named `name`, in `color`.
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self::of(Line::Named(name.into()), color)
    }

    /// The line of `text` itself, in `color`: the Daemon's form's countdown (`0x5635BB`).
    pub fn own(text: impl Into<String>, color: Color) -> Self {
        Self::of(Line::Own(text.into()), color)
    }

    fn of(line: Line, color: Color) -> Self {
        Self {
            lines: vec![Said {
                line,
                color,
                left: SHOWS,
                label: None,
            }],
        }
    }

    /// Adds the lines of `under` under those it shows.
    pub fn push(&mut self, under: Says) {
        self.lines.extend(under.lines);
    }

    /// Shows `says` over `entity` under the lines it shows already, or alone.
    pub fn add(commands: &mut Commands, entity: Entity, says: Says) {
        commands
            .entity(entity)
            .queue(
                move |mut entity: EntityWorldMut| match entity.get_mut::<Says>() {
                    Some(mut shown) => shown.push(says),
                    None => {
                        entity.insert(says);
                    }
                },
            );
    }
}

/// A line's label goes with it, at its end or as others take its place.
fn replaced(replaced: On<Discard<Says>>, mut commands: Commands, speakers: Query<&Says>) {
    if let Ok(says) = speakers.get(replaced.entity) {
        for label in says.lines.iter().filter_map(|said| said.label) {
            commands.entity(label).try_despawn();
        }
    }
}

impl Line {
    /// The text's name, or the text itself.
    fn name(&self) -> &str {
        match self {
            Self::Named(text) | Self::Own(text) => text,
        }
    }
}

/// How far each line shows under the one before, in logical pixels.
const LINE_HEIGHT: f32 = 16.0;

/// Shows each creature's lines over it, following it on the screen, each until its time is out.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    time: Res<Time>,
    texts: Option<Res<Texts>>,
    assets: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), crate::iso::Eye>,
    ui_scale: Res<UiScale>,
    mut speakers: Query<(Entity, &GlobalTransform, &mut Says)>,
    mut nodes: Query<&mut Node>,
) {
    let Some((camera, view)) = cameras.iter().next() else {
        return;
    };
    for (entity, at, mut says) in &mut speakers {
        let text = |said: &Said| match &said.line {
            Line::Named(name) => texts
                .as_ref()
                .and_then(|texts| texts.named(name))
                .map(str::to_owned),
            Line::Own(text) => Some(text.clone()),
        };
        let delta = time.delta_secs();
        says.lines.retain_mut(|said| {
            said.left -= delta;
            let goes = said.left <= 0.0 || text(said).is_none();
            if goes && let Some(label) = said.label {
                commands.entity(label).try_despawn();
            }
            !goes
        });
        if says.lines.is_empty() {
            commands.entity(entity).remove::<Says>();
            continue;
        }
        let Ok(point) = camera.world_to_viewport(view, at.translation() + Vec3::Y * ABOVE) else {
            continue;
        };
        // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
        let point = point / ui_scale.0;
        let mut top = point.y;
        for said in &mut says.lines {
            let (left, at_top) = (Val::Px(point.x - WIDTH / 2.0), Val::Px(top));
            top += LINE_HEIGHT;
            match said.label.map(|label| nodes.get_mut(label)) {
                Some(Ok(mut node)) => {
                    node.left = left;
                    node.top = at_top;
                }
                _ => {
                    let label = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left,
                                top: at_top,
                                width: Val::Px(WIDTH),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            Text::new(text(said).unwrap_or_default()),
                            TextFont {
                                font: assets.load(FONT).into(),
                                font_size: FontSize::Px(FONT_SIZE),
                                ..default()
                            },
                            TextColor(said.color),
                            TextLayout::justify(Justify::Center),
                            DespawnOnExit(AppState::InGame),
                        ))
                        .id();
                    debug!("lines: {entity} says {}", said.line.name());
                    said.label = Some(label);
                }
            }
        }
    }
}

/// The lines over the creatures.
pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.in_set(Step::Npcs))
            .add_observer(replaced);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_reads_back_with_its_spaces() {
        let texts = parse(
            "# a comment\n\nid-49 = Seraphim\nid-50 = {\" \"}two {\"{\"}words{\"  \"}\nnot a message\n\
             id-51 = first\n    {\"\"}\n    {\" \"}third\n",
        );
        assert_eq!(texts.get(&49).map(String::as_str), Some("Seraphim"));
        assert_eq!(texts.get(&50).map(String::as_str), Some(" two {words  "));
        assert_eq!(texts.get(&51).map(String::as_str), Some("first\n\n third"));
        assert_eq!(texts.len(), 3);
        let texts = Texts(texts);
        assert_eq!(texts.named("1"), Some("Seraphim"));
    }
}
