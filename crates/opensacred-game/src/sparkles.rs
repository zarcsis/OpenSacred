//! The interface's sparkles (#1272; docs/re/interface.md, "The buttons"; docs/re/inventory.md,
//! §7): an item used up sparkles where it lay in the backpack's grid while the items' page shows
//! (`0x5D8000`, drawn by `cInventoryEntry_render`, `0x5DC430`), and a potion used up sparkles on
//! its kind's slot of the belt, emptied or not (the taskbar's message `0x10`, the button's flag
//! `0x20000`, `0x72B700`). A sparkle lasts a second, its time `t` from 0 to 1, and each frame
//! draws (`0x40B980`, the quads added onto the screen) a glow at its rectangle's centre, a square
//! of `PARTICLE_SPARK04` tinted `0xBB9360` of the half side `30 cos(πt/2)` turned `2 (1 − t)`
//! radians clockwise, with one of `PARTICLE_SPARK07` tinted `0xBBBBBB` of half that over it, and
//! 20 sparks the same of the half side `9 sin(πt)` turned a right angle, each at a point of the
//! rectangle drawn anew at random every frame. The grid's sparkles go as the page hides.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;
use opensacred_rules::spawning::Rng;

use crate::backpack::{CELL, GRID};
use crate::bar::{self, UiImages};
use crate::game::AppState;
use crate::hud;
use crate::inventory::Shown;
use crate::pickup::Pack;

const SHADER: &str = "embedded://opensacred_game/sparkle.wesl";

/// The textures of a sparkle's quads (`cGranny`'s `+0xBC` and `+0xC0`, `0x401030`).
pub(crate) const SPARK: &str = "particle_spark04";
pub(crate) const SPARK_HALF: &str = "particle_spark07";
/// Their tints (`0x40B980`).
const SPARK_TINT: [u8; 3] = [0xBB, 0x93, 0x60];
const HALF_TINT: [u8; 3] = [0xBB, 0xBB, 0xBB];
/// The glow's half side at the start, a spark's at its height, and the sparks of a frame.
const GLOW: f32 = 30.0;
const SPARK_SIDE: f32 = 9.0;
const SPARKS: u16 = 20;
/// The quads of a sparkle: the glow's two and each spark's two.
const QUADS: u16 = 2 + 2 * SPARKS;

/// A quad of a sparkle: its texture times its tint, added onto the screen.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Spark {
    #[uniform(0)]
    tint: Vec4,
    #[texture(1)]
    #[sampler(2)]
    picture: Handle<Image>,
}

impl UiMaterial for Spark {
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

/// The quads' two materials, made as the first sparkle comes.
#[derive(Resource)]
struct Materials {
    spark: Handle<Spark>,
    half: Handle<Spark>,
}

/// The sparkles' container: the original's screen centred along the window's width, over the bar
/// and the grid.
#[derive(Component)]
struct Root;

/// A sparkle: its rectangle on the original's screen, its time from 0 to 1, and whether it is the
/// grid's.
#[derive(Component)]
struct Sparkle {
    rect: Rect,
    t: f32,
    grid: bool,
}

/// A quad of a sparkle, by its place among them: 0 and 1 the glow's, then each spark's two.
#[derive(Component)]
struct Quad(u16);

/// The glow's half side at the time `t`.
fn glow_half(t: f32) -> f32 {
    GLOW * (FRAC_PI_2 * t).cos()
}

/// The glow's turn at the time `t`, clockwise in radians.
fn glow_turn(t: f32) -> f32 {
    2.0 * (1.0 - t)
}

/// A spark's half side at the time `t`.
fn spark_half(t: f32) -> f32 {
    SPARK_SIDE * (PI * t).sin()
}

/// A tint of the bytes `rgb`, as the frame's sRGB values.
fn tint([r, g, b]: [u8; 3]) -> Vec4 {
    Vec4::new(f32::from(r), f32::from(g), f32::from(b), 255.0) / 255.0
}

/// The items used up sparkle where they lay in the grid while the page shows, and the potions on
/// their kinds' slots of the belt.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn begin(
    mut commands: Commands,
    mut pack: ResMut<Pack>,
    shown: Option<Res<Shown>>,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    mut sparks: ResMut<Assets<Spark>>,
    materials: Option<Res<Materials>>,
    roots: Query<Entity, With<Root>>,
) {
    if pack.used.is_empty() {
        return;
    }
    let used = std::mem::take(&mut pack.used);
    let materials = match materials {
        Some(materials) => (materials.spark.clone(), materials.half.clone()),
        None => {
            let (Some(spark), Some(half)) = (images.file(SPARK), images.file(SPARK_HALF)) else {
                return;
            };
            let made = Materials {
                spark: sparks.add(Spark {
                    tint: tint(SPARK_TINT),
                    picture: hud::image(&assets, spark.to_owned()),
                }),
                half: sparks.add(Spark {
                    tint: tint(HALF_TINT),
                    picture: hud::image(&assets, half.to_owned()),
                }),
            };
            let pair = (made.spark.clone(), made.half.clone());
            commands.insert_resource(made);
            pair
        }
    };
    let root = roots.iter().next().unwrap_or_else(|| {
        commands
            .spawn((
                Root,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Px(0.0),
                    margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
                    width: Val::Px(bar::SCREEN.x),
                    height: Val::Px(bar::SCREEN.y),
                    ..default()
                },
                GlobalZIndex(1),
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
            ))
            .id()
    });
    let page = shown.is_some_and(|shown| shown.0);
    for (placed, item) in used {
        let mut rects = Vec::new();
        if page {
            let at = Vec2::new(f32::from(placed.at.0), f32::from(placed.at.1)) * CELL;
            let size = Vec2::new(f32::from(placed.size.0), f32::from(placed.size.1)) * CELL;
            rects.push((Rect::from_corners(GRID + at, GRID + at + size), true));
        }
        if let Some(slot) = opensacred_rules::potions::kind(item)
            .and_then(|kind| bar::BELT.get(usize::from(kind.checked_sub(1)?)))
        {
            rects.push((Rect::from_corners(*slot, *slot + bar::BELT_SIDE), false));
        }
        for (rect, grid) in rects {
            let sparkle = commands
                .spawn((
                    Sparkle { rect, t: 0.0, grid },
                    bar::placed(Vec2::ZERO, Vec2::ZERO),
                    Pickable::IGNORE,
                ))
                .with_children(|sparkle| {
                    for quad in 0..QUADS {
                        let material = if quad % 2 == 0 {
                            materials.0.clone()
                        } else {
                            materials.1.clone()
                        };
                        sparkle.spawn((
                            Quad(quad),
                            bar::placed(Vec2::ZERO, Vec2::ZERO),
                            UiTransform::IDENTITY,
                            MaterialNode(material),
                            Pickable::IGNORE,
                        ));
                    }
                })
                .id();
            commands.entity(root).add_child(sparkle);
        }
    }
}

/// Each sparkle at its time: its glow and its sparks laid out, each spark at a point of its
/// rectangle drawn anew; gone after a second, the grid's as the page hides.
fn play(
    mut commands: Commands,
    time: Res<Time<Real>>,
    shown: Option<Res<Shown>>,
    mut sparkles: Query<(Entity, &mut Sparkle, &Children)>,
    mut quads: Query<(&Quad, &mut Node, &mut UiTransform)>,
    mut dice: Local<Option<Rng>>,
) {
    let dice = dice.get_or_insert_with(|| Rng::of(0, 0, 1272));
    let page = shown.is_some_and(|shown| shown.0);
    for (entity, mut sparkle, children) in &mut sparkles {
        if sparkle.t >= 1.0 || (sparkle.grid && !page) {
            commands.entity(entity).despawn();
            continue;
        }
        let t = sparkle.t;
        let centre = sparkle.rect.center();
        let mut spark = centre;
        for &child in children {
            let Ok((&Quad(quad), mut node, mut transform)) = quads.get_mut(child) else {
                continue;
            };
            let (at, half, turn) = match quad {
                0 => (centre, glow_half(t), glow_turn(t)),
                1 => (centre, glow_half(t) / 2.0, glow_turn(t)),
                _ => {
                    if quad % 2 == 0 {
                        let mut share =
                            || f32::from(u16::try_from(dice.below(32_768)).unwrap_or(0));
                        let (x, y) = (share() / 32_767.0, share() / 32_767.0);
                        spark = sparkle.rect.min + sparkle.rect.size() * Vec2::new(x, y);
                    }
                    let half = spark_half(t) / if quad % 2 == 0 { 1.0 } else { 2.0 };
                    (spark, half, FRAC_PI_2)
                }
            };
            node.left = Val::Px(at.x - half);
            node.top = Val::Px(at.y - half);
            node.width = Val::Px(2.0 * half);
            node.height = Val::Px(2.0 * half);
            transform.rotation = Rot2::radians(turn);
        }
        sparkle.t += time.delta_secs();
    }
}

/// The interface's sparkles.
pub struct SparklesPlugin;

impl Plugin for SparklesPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "sparkle.wesl");
        app.add_plugins(UiMaterialPlugin::<Spark>::default())
            .add_systems(
                Update,
                (begin, play).chain().run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_shrinks_and_turns_back_as_the_sparks_rise_and_fall() {
        assert!((glow_half(0.0) - 30.0).abs() < 1e-5);
        assert!(glow_half(1.0).abs() < 1e-5);
        assert!((glow_turn(0.0) - 2.0).abs() < 1e-6);
        assert!(glow_turn(1.0).abs() < 1e-6);
        assert!(spark_half(0.0).abs() < 1e-5);
        assert!((spark_half(0.5) - 9.0).abs() < 1e-5);
        assert!(spark_half(1.0).abs() < 1e-5);
    }
}
