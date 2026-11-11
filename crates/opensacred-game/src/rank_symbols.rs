//! The champions' and the bosses' symbols on the ground (#2534; docs/re/fight.md, "The health
//! rings"): under every champion (a champion's levelling, `+0xB0` bit `0x400`, which the fill's
//! types 343 and 212 have alone) and every boss (`+0x1F4` bit `0x80000`) alive and hostile to
//! the hero, with no timer, a square of `UI_DISPLAY_GROUNDSYMBOL03` (a circle of eight
//! arrowheads) or `04` (of twelve spikes) in red (`0xFFFF0000`), its half side twice half its type's size, flat at its
//! place, laid over by its alpha and its depth tested, unlit (`cCreature_render` `0x599910` at
//! `0x59A053` and `0x59A0B9`, `0x4087E0`), turning about the upright once in 16.384 s by the
//! real clock (`opensacred_rules::fight::symbol_turn`); untinted by the night, as the health
//! rings are.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::fight::{Rank, symbol_half_side, symbol_turn};

use crate::fight::{Health, Ranked};
use crate::frame::Playing;
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::iso;
use crate::npcs::Dies;

/// The symbols' textures, a champion's and a boss's (`+0xB4`, `+0xB8` of the granny's singleton).
const TEXTURES: [&str; 2] = ["UI_DISPLAY_GROUNDSYMBOL03", "UI_DISPLAY_GROUNDSYMBOL04"];
/// The symbol's vertices' colour, opaque red (`0xFFFF0000`, `0x408829`), which tints the white
/// texture.
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
/// How far over the ground the symbol lies, as the health ring does (`health_rings`).
const LIFT: f32 = 0.5;

/// A creature's symbol: its kind, 0 a champion's and 1 a boss's, and its look.
#[derive(Component)]
struct Symbol {
    kind: usize,
    look: Entity,
}

/// A symbol's look: its mesh.
#[derive(Component)]
struct Look(Handle<Mesh>);

/// The symbols' materials, made at the first symbol; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 2]>);

/// The kind of symbol that a creature of the rank `rank` shows: a champion's for a champion and
/// for a champion's levelling alone, a boss's for a boss; none for another.
fn kind(rank: Rank) -> Option<usize> {
    match rank {
        Rank::Champion | Rank::Levelled => Some(0),
        Rank::Boss => Some(1),
        Rank::Common => None,
    }
}

/// Despawns the look of a symbol that its creature loses, or loses with it.
fn gone(removed: On<Remove<Symbol>>, symbols: Query<&Symbol>, mut commands: Commands) {
    if let Ok(symbol) = symbols.get(removed.entity) {
        commands.entity(symbol.look).try_despawn();
    }
}

/// Lays each symbol under its creature while it shows, turned by the real clock, at its
/// creature's place in the order of drawing, and takes it away once it shows no more.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn show(
    mut commands: Commands,
    time: Res<Time<Real>>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Ranked,
            &Health,
            (
                Option<&crate::ai::Mind>,
                Option<&crate::arrows::Size>,
                Option<&Symbol>,
            ),
            Has<Dies>,
        ),
        Without<Body>,
    >,
    looks: Query<&Look>,
    (data, assets, mut materials): (Option<Res<Data>>, Res<AssetServer>, ResMut<Materials>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
    orders: Orders,
) {
    let turn = symbol_turn(u64::try_from(time.elapsed().as_millis()).unwrap_or(0));
    let (sin, cos) = turn.sin_cos();
    let (across, down) = iso::fx_axes();
    for (entity, at, ranked, health, (mind, size, symbol), dying) in &creatures {
        let shows = kind(ranked.0)
            .filter(|_| health.current > 0 && !dying && mind.is_some_and(|mind| mind.hostile()));
        let Some(kind) = shows else {
            if symbol.is_some() {
                commands.entity(entity).remove::<Symbol>();
            }
            continue;
        };
        let look = match symbol {
            Some(symbol) if symbol.kind == kind && looks.contains(symbol.look) => symbol.look,
            _ => {
                if let Some(symbol) = symbol {
                    commands.entity(symbol.look).try_despawn();
                }
                let made = materials.0.get_or_insert_with(|| {
                    TEXTURES.map(|name| {
                        let image = data
                            .as_deref()
                            .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                        if image.is_none() {
                            warn!("rank symbols: no texture {name}; its symbol is not drawn");
                        }
                        image.map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
                    })
                });
                let mesh = meshes.add(empty());
                let mut look = commands.spawn((
                    Look(mesh.clone()),
                    Transform::IDENTITY,
                    Visibility::default(),
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(material) = made[kind].clone() {
                    look.with_child((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
                let look = look.id();
                commands.entity(entity).insert(Symbol { kind, look });
                continue;
            }
        };
        let Ok(Look(mesh)) = looks.get(look) else {
            continue;
        };
        // Its size, 50 where its type gives none (`arrows::Size`, `0x428CE0`).
        let size = size.map_or(opensacred_rules::arrows::size(0), |size| size.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a creature's size, a small whole number"
        )]
        let half = symbol_half_side(size.max(0.0) as u32);
        let r = f32::from(u16::try_from(half).unwrap_or(u16::MAX));
        let place = at.translation() + Vec3::Y * LIFT;
        // Turned about the upright by its turn, in the models' units, as the health ring lies.
        let point = |[x, y]: [f32; 2]| {
            let (x, y) = (x * cos - y * sin, x * sin + y * cos);
            place + (across * x + down * y) * iso::MODEL_SCALE
        };
        let uv = |[x, y]: [f32; 2]| Vec2::new((x / r + 1.0) / 2.0, (1.0 - y / r) / 2.0);
        let corners = [[-r, r], [r, r], [r, -r], [-r, -r]];
        let mut quads = Quads::default();
        quads.push_corners(corners.map(point), corners.map(uv), RED);
        quads.in_order(orders.at(at.translation()), Vec3::ZERO);
        if let Some(mut mesh) = meshes.get_mut(mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The champions' and the bosses' symbols.
pub struct RankSymbolsPlugin;

impl Plugin for RankSymbolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .add_observer(gone)
            .add_systems(
                OnExit(AppState::InGame),
                |mut materials: ResMut<Materials>| {
                    materials.0 = None;
                },
            )
            .add_systems(
                Update,
                show.after(crate::frame::Step::Models).in_set(Playing),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_champion_or_a_levelling_alone_shows_a_champions_symbol_and_a_boss_a_bosss() {
        assert_eq!(kind(Rank::Champion), Some(0));
        assert_eq!(kind(Rank::Levelled), Some(0));
        assert_eq!(kind(Rank::Boss), Some(1));
        assert_eq!(kind(Rank::Common), None);
    }

    #[test]
    fn a_creature_that_loses_its_symbol_takes_its_look_with_it() {
        let mut app = App::new();
        app.add_observer(gone);
        let look = app.world_mut().spawn_empty().id();
        let boss = app.world_mut().spawn(Symbol { kind: 1, look }).id();
        app.world_mut().entity_mut(boss).remove::<Symbol>();
        app.update();
        assert!(app.world().get_entity(look).is_err());
    }
}
