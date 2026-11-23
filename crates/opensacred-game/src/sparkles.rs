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
//!
//! The statistics window's close button sparkles the same as the hero rises a level (#1720,
//! `0x57F023`), and the bar's book button as the hero reads a book new to its list (`UI_TB_BOOK`,
//! `0x5CBFC0`, #3095).
//!
//! The spells page's tab flashes as a rune teaches or raises an art (`UI_TABPAGE_SPELLS`, the
//! message `0x11` of the flag `0x20000`, `0x5CD100`; docs/re/inventory.md §2,
//! #3095): 32 steps of 0.04 s, at each step `k` the row `f` of the 16 of `FX_GUI_MAIN_01` (`k`,
//! then `31 − k`) over the tab from 2 to 34 below its top and across it, tinted `0xBB9360` and
//! added onto the screen, and the sparkle at the tab at the time `k/32`; selecting the page ends
//! it, and a rune read while it flashes leaves its clock as it runs.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
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
/// The flash's texture of 16 rows, its steps a second and its steps, and where its rows lie
/// below the tab's top (`0x728800`).
const FLASH: &str = "fx_gui_main_01";
const FLASH_RATE: f32 = 25.0;
const FLASH_STEPS: f32 = 32.0;
const FLASH_ROWS: f32 = 16.0;
const FLASH_TOP: f32 = 2.0;
const FLASH_HEIGHT: f32 = 32.0;
/// The spells page, whose tab a rune flashes (`inventory::Page`).
const SPELLS_PAGE: u8 = 1;
/// The glow's half side at the start, a spark's at its height, and the sparks of a frame.
const GLOW: f32 = 30.0;
const SPARK_SIDE: f32 = 9.0;
const SPARKS: u16 = 20;
/// The quads of a sparkle: the glow's two and each spark's two.
const QUADS: u16 = 2 + 2 * SPARKS;

/// A quad of a sparkle: its texture times its tint, added onto the screen; of its texture the
/// rows from `rows.x` down `rows.y` of its height, the whole for a spark's.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Spark {
    #[uniform(0)]
    tint: Vec4,
    #[texture(1)]
    #[sampler(2)]
    picture: Handle<Image>,
    #[uniform(3)]
    rows: Vec4,
}

/// The whole texture's rows.
const WHOLE: Vec4 = Vec4::new(0.0, 1.0, 0.0, 0.0);

/// What the interface tells by the flag `0x20000`: the bar's book button a book new to the
/// hero's list (`0x5CBFC0`), the spells page's tab a rune that taught or raised an art
/// (`0x5CD100`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Told {
    BookButton,
    SpellsTab,
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

/// A sparkle: its rectangle on the original's screen, or on its tab's for a tab's flash, its
/// seconds, whether it goes as the inventory's windows hide (the grid's and the statistics
/// window's close button's), and whether it is a tab's flash.
#[derive(Component)]
struct Sparkle {
    rect: Rect,
    t: f32,
    windows: bool,
    flash: bool,
}

impl Sparkle {
    /// Its sparkle's time from 0 to 1, and whether it is over: a second's, or a flash's step of
    /// its 32 (`0x728800`).
    fn time(&self) -> (f32, bool) {
        if self.flash {
            let step = (self.t * FLASH_RATE).floor();
            (step / FLASH_STEPS, step >= FLASH_STEPS)
        } else {
            (self.t, self.t >= 1.0)
        }
    }
}

/// The row of the flash's texture at the step `step` (`0x728060`): the step for the first 16,
/// then back.
fn flash_row(step: f32) -> f32 {
    if step < FLASH_ROWS {
        step
    } else {
        2.0 * FLASH_ROWS - 1.0 - step
    }
}

/// A quad of a sparkle, by its place among them: 0 and 1 the glow's, then each spark's two.
#[derive(Component)]
struct Quad(u16);

/// A flash's quad of the rows, with its own material.
#[derive(Component)]
struct Rows(Handle<Spark>);

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

/// What a new sparkle needs: the quads' materials, made as the first sparkle comes, and the
/// sparkles' container.
#[derive(bevy::ecs::system::SystemParam)]
struct Making<'w, 's> {
    images: Res<'w, UiImages>,
    assets: Res<'w, AssetServer>,
    sparks: ResMut<'w, Assets<Spark>>,
    materials: Option<Res<'w, Materials>>,
    roots: Query<'w, 's, Entity, With<Root>>,
}

impl Making<'_, '_> {
    /// Spawns a sparkle over `rect`, which goes as the inventory's windows hide where `windows`;
    /// none where the sparks' textures are lacking.
    fn sparkle(&mut self, commands: &mut Commands, rect: Rect, windows: bool) {
        let Some(materials) = self.materials(commands) else {
            return;
        };
        let root = self.roots.iter().next().unwrap_or_else(|| {
            commands
                .spawn((
                    Root,
                    crate::cinema::Interface,
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
        let sparkle = spawn_quads(
            commands,
            Sparkle {
                rect,
                t: 0.0,
                windows,
                flash: false,
            },
            &materials,
        );
        commands.entity(root).add_child(sparkle);
    }

    /// The quads' materials, made as the first sparkle comes; none where the sparks' textures
    /// are lacking.
    fn materials(&mut self, commands: &mut Commands) -> Option<(Handle<Spark>, Handle<Spark>)> {
        if let Some(materials) = self.materials.as_deref() {
            return Some((materials.spark.clone(), materials.half.clone()));
        }
        let (spark, half) = (self.images.file(SPARK)?, self.images.file(SPARK_HALF)?);
        let made = Materials {
            spark: self.sparks.add(Spark {
                tint: tint(SPARK_TINT),
                picture: hud::image(&self.assets, spark.to_owned()),
                rows: WHOLE,
            }),
            half: self.sparks.add(Spark {
                tint: tint(HALF_TINT),
                picture: hud::image(&self.assets, half.to_owned()),
                rows: WHOLE,
            }),
        };
        let pair = (made.spark.clone(), made.half.clone());
        commands.insert_resource(made);
        Some(pair)
    }
}

/// Spawns `sparkle` with its glow's and its sparks' quads.
fn spawn_quads(
    commands: &mut Commands,
    sparkle: Sparkle,
    (spark, half): &(Handle<Spark>, Handle<Spark>),
) -> Entity {
    commands
        .spawn((
            sparkle,
            bar::placed(Vec2::ZERO, Vec2::ZERO),
            Pickable::IGNORE,
        ))
        .with_children(|sparkle| {
            for quad in 0..QUADS {
                let material = if quad % 2 == 0 {
                    spark.clone()
                } else {
                    half.clone()
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
        .id()
}

/// What the interface was told by the flag `0x20000` ([`Told`]): the bar's book button sparkles
/// a second; the spells page's tab flashes, its rows under its sparkle, unless it flashes already
/// (the control's one clock) or its page shows.
fn told(
    mut commands: Commands,
    mut told: MessageReader<Told>,
    (data, page): (
        Option<Res<crate::game::Data>>,
        Option<Res<crate::inventory::Page>>,
    ),
    tabs: Query<(Entity, &crate::inventory::Tab, &ComputedNode)>,
    flashes: Query<&Sparkle>,
    mut making: Making,
) {
    for told in told.read() {
        match told {
            Told::BookButton => {
                let (index, corner) = bar::BOOK_BUTTON;
                let rect = Rect::from_corners(corner, corner + making.images.size(index));
                making.sparkle(&mut commands, rect, false);
            }
            Told::SpellsTab => {
                if flashes.iter().any(|sparkle| sparkle.flash)
                    || page.as_deref().is_some_and(|page| page.0 == SPELLS_PAGE)
                {
                    continue;
                }
                let Some((tab, _, node)) =
                    tabs.iter().find(|(_, tab, _)| tab.page() == SPELLS_PAGE)
                else {
                    continue;
                };
                let (Some(materials), Some(data)) =
                    (making.materials(&mut commands), data.as_deref())
                else {
                    continue;
                };
                let Some(picture) =
                    crate::fx::particle_texture(&data.0.assets, &making.assets, FLASH)
                else {
                    warn!("sparkles: no texture {FLASH}; the tab does not flash");
                    continue;
                };
                let size = node.size() * node.inverse_scale_factor();
                let rows = making.sparks.add(Spark {
                    tint: tint(SPARK_TINT),
                    picture,
                    rows: Vec4::new(0.0, 1.0 / FLASH_ROWS, 0.0, 0.0),
                });
                let quad = commands
                    .spawn((
                        Rows(rows.clone()),
                        bar::placed(Vec2::new(0.0, FLASH_TOP), Vec2::new(size.x, FLASH_HEIGHT)),
                        MaterialNode(rows),
                        Pickable::IGNORE,
                    ))
                    .id();
                let sparkle = spawn_quads(
                    &mut commands,
                    Sparkle {
                        rect: Rect::from_corners(Vec2::ZERO, size),
                        t: 0.0,
                        windows: true,
                        flash: true,
                    },
                    &materials,
                );
                commands.entity(sparkle).insert_children(0, &[quad]);
                commands.entity(tab).add_child(sparkle);
            }
        }
    }
}

/// The items used up sparkle where they lay in the grid while the page shows, and the potions on
/// their kinds' slots of the belt.
fn begin(
    mut commands: Commands,
    mut pack: ResMut<Pack>,
    shown: Option<Res<Shown>>,
    mut making: Making,
) {
    if pack.used.is_empty() {
        return;
    }
    let used = std::mem::take(&mut pack.used);
    let page = shown.is_some_and(|shown| shown.0);
    for (placed, item) in used {
        if page {
            let at = Vec2::new(f32::from(placed.at.0), f32::from(placed.at.1)) * CELL;
            let size = Vec2::new(f32::from(placed.size.0), f32::from(placed.size.1)) * CELL;
            making.sparkle(
                &mut commands,
                Rect::from_corners(GRID + at, GRID + at + size),
                true,
            );
        }
        if let Some(slot) = opensacred_rules::potions::kind(item)
            .and_then(|kind| bar::BELT.get(usize::from(kind.checked_sub(1)?)))
        {
            making.sparkle(
                &mut commands,
                Rect::from_corners(*slot, *slot + bar::BELT_SIDE),
                false,
            );
        }
    }
}

/// The statistics window's close button sparkles a second as the hero rises a level while the
/// inventory's windows show (`0x57F023`, the button's draw `0x72B700`, #1720).
fn level_up(
    mut commands: Commands,
    experience: Res<crate::fight::Experience>,
    shown: Option<Res<Shown>>,
    mut making: Making,
    mut last: Local<Option<u32>>,
) {
    let level = experience.0.level;
    let risen = last.is_some_and(|was| level > was);
    *last = Some(level);
    if risen && shown.is_some_and(|shown| shown.0) {
        let rect = crate::statistics::close_button(&making.images);
        making.sparkle(&mut commands, rect, true);
    }
}

/// Each sparkle at its time: its glow and its sparks laid out, each spark at a point of its
/// rectangle drawn anew, a flash's rows at its step's; gone after a second, a flash after its 32
/// steps or as its page is selected, the grid's as the page hides.
fn play(
    mut commands: Commands,
    time: Res<Time<Real>>,
    (shown, selected): (Option<Res<Shown>>, Option<Res<crate::inventory::Page>>),
    mut sparkles: Query<(Entity, &mut Sparkle, &Children)>,
    mut quads: Query<(&Quad, &mut Node, &mut UiTransform)>,
    (rows, mut sparks): (Query<&Rows>, ResMut<Assets<Spark>>),
    mut dice: Local<Option<Rng>>,
) {
    let dice = dice.get_or_insert_with(|| crate::seed::dice(1272));
    let page = shown.is_some_and(|shown| shown.0);
    let spells = selected.is_some_and(|selected| selected.0 == SPELLS_PAGE);
    for (entity, mut sparkle, children) in &mut sparkles {
        let (t, over) = sparkle.time();
        if over || (sparkle.windows && !page) || (sparkle.flash && spells) {
            commands.entity(entity).despawn();
            continue;
        }
        if sparkle.flash {
            let row = flash_row((t * FLASH_STEPS).round());
            for &child in children {
                if let Ok(Rows(material)) = rows.get(child)
                    && let Some(mut material) = sparks.get_mut(material)
                {
                    material.rows.x = row / FLASH_ROWS;
                }
            }
        }
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
        app.add_message::<Told>()
            .add_plugins(crate::ui_layers::NativeMaterialPlugin::<Spark>::default())
            .add_systems(
                Update,
                (begin, level_up, told, play)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tabs_flash_runs_its_rows_up_and_back_in_32_steps_of_a_25th() {
        let rows = [0.0, 1.0, 15.0, 16.0, 17.0, 31.0].map(flash_row);
        assert_eq!(rows, [0.0, 1.0, 15.0, 15.0, 14.0, 0.0]);
        let at = |t, flash| {
            Sparkle {
                rect: Rect::default(),
                t,
                windows: false,
                flash,
            }
            .time()
        };
        // A sparkle's second, smooth.
        assert_eq!(at(0.5, false), (0.5, false));
        assert_eq!(at(0.99, false), (0.99, false));
        assert!(at(1.0, false).1);
        // A flash's 32 steps of 0.04 s, its sparkle at the step's time.
        assert_eq!(at(0.03, true), (0.0, false));
        assert_eq!(at(0.05, true), (1.0 / 32.0, false));
        assert_eq!(at(1.27, true), (31.0 / 32.0, false));
        assert!(at(1.3, true).1);
    }

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
