//! The info lines of the scripts' `InfoPlayer`s (#2499; docs/re/interface.md, "The info lines",
//! `cUI_TextLines`): each text a line of its own in the font 3, centred across the screen from
//! 170 pixels down, each later line under those before (`0x7237D0`). A line's age and alpha run
//! by the real clock, on through a pause (`0x723550`): for its first π/3 s it glows, its glyphs
//! drawn again 64 times about it in a crimson of `sin(3·age)` added to the frame; from π/6 s it
//! shows plain in `ARGBInfoText`'s colour of the balance, its alpha 1; past `CHAT_DELAY`, 8 s, it
//! fades over a second and goes (`0x723020`). The lines hide while the world map shows, their
//! clocks running on.

use std::f32::consts::{FRAC_PI_3, FRAC_PI_6};

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
    ShaderType,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialKey};

use crate::frame::Playing;
use crate::game::AppState;

const SHADER: &str = "embedded://opensacred_game/info_glow.wesl";

/// How far down the screen the first line's top lies, in the original's pixels (`0x723020`: its
/// rectangle at 170 of the 1024 × 768 screen, 32 high).
const TOP: f32 = 170.0;
/// How long a line holds before it fades, in seconds: `CHAT_DELAY` of `Settings.cfg`, 8 where it
/// has none (`0x81C1B0`, `0x9E4D30`).
const HOLDS: f32 = 8.0;
/// The glow's colour at its height, of 255 (`0x8971C4`, `0x890F18`, `0x890DC8`), and how far
/// about the glyphs its copies reach, in pixels (`0x7232E7`–`0x723535`).
const GLOW: [f32; 3] = [212.0, 30.0, 60.0];
const GLOW_REACH: f32 = 7.0;
/// `ARGBInfoText`'s colour where the balance lacks it: orange (`0xFFFF8000`, docs/02 §6.3).
const ORANGE: u32 = 0xFFFF_8000;

/// The glow's share of its colour at a line's age `age` in seconds: `sin(3·age)` for its first
/// π/3 s, after which it is gone.
fn glow_share(age: f32) -> Option<f32> {
    (age < FRAC_PI_3).then(|| (3.0 * age).sin().max(0.0))
}

/// Whether a line of the age `age` shows plain: past π/6 s, the glow's height.
fn plain(age: f32) -> bool {
    age > FRAC_PI_6
}

/// What the glow's shader takes: its colour, of the frame's values, and the picture's texels a
/// pixel.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
struct GlowUniform {
    colour: Vec4,
    scale: Vec4,
}

/// The glow of a line: its glyphs' coverage drawn again at ±1, 3, 5 and 7 pixels on both axes,
/// 64 times, added onto the screen.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Glow {
    #[uniform(0)]
    glow: GlowUniform,
    #[texture(1)]
    #[sampler(2)]
    picture: Handle<Image>,
}

impl UiMaterial for Glow {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        // The light added, the screen's alpha kept.
        let target = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut);
        if let Some(target) = target {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
        }
    }
}

/// The lines' column across the screen.
#[derive(Component)]
struct Lines;

/// A line: its age and its alpha in seconds of the real clock, its colour, its plain text's node,
/// and its glow's node and material.
#[derive(Component)]
struct Line {
    age: f32,
    alpha: f32,
    colour: Color,
    plain: Entity,
    glow: Entity,
    material: Handle<Glow>,
}

/// The texts that the scripts' `InfoPlayer`s show, to be laid out as lines (`talk::notices`).
#[derive(Resource, Default)]
pub(crate) struct Shown(pub Vec<String>);

/// Lays each text shown out as lines of their own under those that show, one a line of its text.
fn add(
    mut commands: Commands,
    mut shown: ResMut<Shown>,
    data: Option<Res<crate::game::Data>>,
    (mut images, mut materials): (ResMut<Assets<Image>>, ResMut<Assets<Glow>>),
    (ui_scale, windows): (Res<UiScale>, Query<&Window>),
    columns: Query<Entity, With<Lines>>,
    mut font: Local<Option<Vec<u8>>>,
) {
    if shown.0.is_empty() {
        return;
    }
    let texts = std::mem::take(&mut shown.0);
    if font.is_none() {
        *font = crate::files::read(crate::game::FONT).ok();
    }
    let Some(font) = font.as_deref() else {
        warn!("info lines: no font {}", crate::game::FONT);
        return;
    };
    let scale = ui_scale.0 * windows.single().map_or(1.0, Window::scale_factor);
    let colour = data
        .as_deref()
        .and_then(|data| data.0.tables.balance().ok())
        .and_then(|balance| balance.i32("ARGBInfoText"))
        .map_or(ORANGE, |argb| u32::from_ne_bytes(argb.to_ne_bytes()));
    let colour = crate::tooltip::argb(colour);
    let column = columns.iter().next().unwrap_or_else(|| {
        commands
            .spawn((
                Lines,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(TOP),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
                GlobalZIndex(2),
                DespawnOnExit(AppState::InGame),
            ))
            .id()
    });
    for text in texts.iter().flat_map(|text| text.lines()) {
        let Some((size, image)) = crate::place_names::line(font, text.trim(), scale) else {
            continue;
        };
        info!("info lines: {text}");
        let image = images.add(image);
        let material = materials.add(Glow {
            glow: GlowUniform {
                colour: Vec4::ZERO,
                scale: Vec4::splat(scale),
            },
            picture: image.clone(),
        });
        let line = commands
            .spawn((
                Node {
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(column),
            ))
            .id();
        let glow = commands
            .spawn((
                MaterialNode(material.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-GLOW_REACH),
                    top: Val::Px(-GLOW_REACH),
                    width: Val::Px(size.x + 2.0 * GLOW_REACH),
                    height: Val::Px(size.y + 2.0 * GLOW_REACH),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(line),
            ))
            .id();
        let plain = commands
            .spawn((
                ImageNode::new(image).with_color(colour.with_alpha(1.0)),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(line),
            ))
            .id();
        commands.entity(line).insert(Line {
            age: 0.0,
            alpha: 1.0,
            colour,
            plain,
            glow,
            material,
        });
    }
}

/// Runs each line on by the real clock (`0x723550`): its glow's colour by its age, its plain text
/// from π/6 s, its alpha falling after [`HOLDS`] s, and it goes at 0; the lines hide while the
/// world map shows.
fn run(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut lines: Query<(Entity, &mut Line)>,
    mut columns: Query<&mut Visibility, (With<Lines>, Without<ImageNode>)>,
    mut nodes: Query<(&mut Visibility, Option<&mut ImageNode>), Without<Lines>>,
    mut materials: ResMut<Assets<Glow>>,
    map: Option<Res<crate::world_map::Map>>,
) {
    let dt = real.delta_secs();
    let hidden = map.is_some_and(|map| map.shown);
    for mut visibility in &mut columns {
        visibility.set_if_neq(if hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    for (entity, mut line) in &mut lines {
        line.age += dt;
        if line.age > HOLDS {
            line.alpha -= dt;
        }
        if line.alpha <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let share = glow_share(line.age);
        if let Ok((mut visibility, _)) = nodes.get_mut(line.glow) {
            visibility.set_if_neq(if share.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        if let Some(share) = share
            && let Some(mut glow) = materials.get_mut(&line.material)
        {
            let [r, g, b] = GLOW.map(|c| (c * share).floor() / 255.0);
            glow.glow.colour = Vec4::new(r, g, b, 1.0);
        }
        if let Ok((mut visibility, image)) = nodes.get_mut(line.plain) {
            visibility.set_if_neq(if plain(line.age) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
            if let Some(mut image) = image {
                let colour = line.colour.with_alpha(line.alpha.clamp(0.0, 1.0));
                if image.color != colour {
                    image.color = colour;
                }
            }
        }
    }
}

/// The info lines.
pub struct InfoLinesPlugin;

impl Plugin for InfoLinesPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "info_glow.wesl");
        app.add_plugins(crate::ui_layers::NativeMaterialPlugin::<Glow>::default())
            .init_resource::<Shown>()
            .add_systems(OnExit(AppState::InGame), |mut shown: ResMut<Shown>| {
                shown.0.clear();
            })
            .add_systems(Update, (add, run).chain().in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_glows_then_shows_plain_and_holds() {
        // The glow rises to its height at π/6 s and is gone at π/3 s.
        assert_eq!(glow_share(0.0), Some(0.0));
        let top = glow_share(FRAC_PI_6).expect("it glows");
        assert!((top - 1.0).abs() < 1e-6);
        assert!(glow_share(FRAC_PI_3 + 0.01).is_none());
        // The plain line from the glow's height on.
        assert!(!plain(0.5) && plain(0.53));
        assert_eq!(HOLDS, 8.0);
    }
}
