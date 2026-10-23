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

    /// The text named `name`.
    pub fn named(&self, name: &str) -> Option<&str> {
        self.0
            .get(&text::sacred_hash(name.as_bytes()))
            .map(String::as_str)
    }
}

/// The messages `id-N = text` of a Fluent file as the converter writes it, their string
/// literals (`{ " " }`, which keep the spaces at a text's ends) undone.
fn parse(ftl: &str) -> HashMap<u32, String> {
    ftl.lines()
        .filter_map(|line| {
            let (id, value) = line.strip_prefix("id-")?.split_once(" = ")?;
            let mut text = String::new();
            let mut rest = value;
            while let Some(start) = rest.find("{ \"") {
                text.push_str(&rest[..start]);
                let after = &rest[start + 3..];
                let end = after.find("\" }")?;
                text.push_str(&after[..end]);
                rest = &after[end + 3..];
            }
            text.push_str(rest);
            Some((id.parse().ok()?, text))
        })
        .collect()
}

/// A line a creature shows over itself: the name of its text, its colour, how long it still
/// shows, and its label once it has one.
#[derive(Component)]
pub struct Says {
    name: String,
    color: Color,
    left: f32,
    label: Option<Entity>,
}

impl Says {
    /// The line of the text named `name`, in `color`.
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            name: name.into(),
            color,
            left: SHOWS,
            label: None,
        }
    }
}

/// Shows each creature's line over it, following it on the screen, until its time is out.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    time: Res<Time>,
    texts: Option<Res<Texts>>,
    assets: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_scale: Res<UiScale>,
    mut speakers: Query<(Entity, &GlobalTransform, &mut Says)>,
    mut nodes: Query<&mut Node>,
) {
    let Some((camera, view)) = cameras.iter().next() else {
        return;
    };
    for (entity, at, mut says) in &mut speakers {
        says.left -= time.delta_secs();
        let text = texts.as_ref().and_then(|texts| texts.named(&says.name));
        if says.left <= 0.0 || text.is_none() {
            if let Some(label) = says.label {
                commands.entity(label).despawn();
            }
            commands.entity(entity).remove::<Says>();
            continue;
        }
        let Ok(point) = camera.world_to_viewport(view, at.translation() + Vec3::Y * ABOVE) else {
            continue;
        };
        // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
        let point = point / ui_scale.0;
        let (left, top) = (Val::Px(point.x - WIDTH / 2.0), Val::Px(point.y));
        match says.label.map(|label| nodes.get_mut(label)) {
            Some(Ok(mut node)) => {
                node.left = left;
                node.top = top;
            }
            _ => {
                let label = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left,
                            top,
                            width: Val::Px(WIDTH),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        Text::new(text.unwrap_or_default()),
                        TextFont {
                            font: assets.load(FONT).into(),
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        TextColor(says.color),
                        TextLayout::justify(Justify::Center),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();
                debug!("lines: {entity} says {}", says.name);
                says.label = Some(label);
            }
        }
    }
}

/// The lines over the creatures.
pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.in_set(Step::Npcs));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_reads_back_with_its_spaces() {
        let texts = parse(
            "# a comment\n\nid-49 = Seraphim\nid-50 = { \" \" }two words{ \"  \" }\nnot a message\n",
        );
        assert_eq!(texts.get(&49).map(String::as_str), Some("Seraphim"));
        assert_eq!(texts.get(&50).map(String::as_str), Some(" two words  "));
        assert_eq!(texts.len(), 2);
        let texts = Texts(texts);
        assert_eq!(texts.named("1"), Some("Seraphim"));
    }
}
