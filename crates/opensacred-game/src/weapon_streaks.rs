//! The weapons' streaks in the arts (797, `cParticleSystem_magicfade`; #3454; docs/re/effects.md,
//! "The arts' streaks"): as an art that its executor streaks starts its clip, a ribbon of
//! `FADESTREAK` along each weapon in the hands it streaks, between the weapon's bones
//! `weapon_fx01` and `weapon_fx02`, a pair of points laid at each frame for the art's clip and
//! left where they lie, each fading within 0.255 s, so that the blade sweeps a band of light that
//! fades behind it; pale blue for most, green for the Wood Elf's, red for the Vampiress's, a
//! creature Gladiator's by its weapon (`opensacred_rules::particles::{WeaponStreak, streak_hands,
//! streak_colour}`). Added by its alpha, untinted by the night, drawn just behind its bearer's
//! model, as the original draws the system before the creature it follows (`fx::Orders`). A shield,
//! a cosmetic item and a weapon whose model lacks the bones show none.
//!
//! Not followed: the streak of an art that plays no clip of its own (a jump that fails, the
//! Seraphim's 1013, 1020 and 1054 refused with a bow), which the original lays on the clip it
//! finds playing.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    STREAK_BONES, STREAK_TEXTURE, StreakHands, WeaponStreak, streak_colour, streak_hands,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::npcs::{Npcs, Piece, Wielded};

/// How long an art's streak lasts: its clip's seconds at its speed, the hero's; or while its
/// creature makes the art (`creature_arts::Makes`), whose clip plays as long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Lasts {
    Seconds(f32),
    WhileMaking,
}

/// An art's streaks to lay at its bearer (`lay`).
#[derive(Component)]
struct Unlaid {
    bearer: Entity,
    art: u16,
    executor: u32,
    player: bool,
    lasts: Lasts,
}

/// Lays the streaks of the art `art` that `bearer` starts, made through the executor of the hero's
/// type `executor` (`opensacred_rules::casts::executor`), a player's or not, for `lasts`; none for
/// an art that its executor does not streak.
pub(crate) fn lay(
    commands: &mut Commands,
    bearer: Entity,
    (art, executor, player): (u16, u32, bool),
    lasts: Lasts,
) {
    if streak_hands(executor, art).is_some() {
        commands.spawn((
            Unlaid {
                bearer,
                art,
                executor,
                player,
                lasts,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The streaks' material, `FADESTREAK` added by its alpha, made as the first asks; `None` where
/// the texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A weapon's streak: its bearer, its weapon, its look, its weapon's bones once found, how it
/// lasts and its mesh.
#[derive(Component)]
struct Streak {
    bearer: Entity,
    weapon: Entity,
    look: WeaponStreak,
    bones: Option<[Entity; 2]>,
    lasts: Lasts,
    mesh: Handle<Mesh>,
}

/// Gives each art's streaks a look for each weapon in the hands it streaks: the right hand's
/// (the slot 13), else the left's, for the Gladiator's executor, each hand's for the others'; a
/// shield, a cosmetic item or an empty hand none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    unlaid: Query<(Entity, &Unlaid)>,
    weapons: Query<(Entity, &crate::models::OrderedWith, &Wielded, &Piece)>,
    (places, holding): (Query<&GlobalTransform>, Query<&crate::fight::Holding>),
    (npcs, data, assets, mut looks): (
        Option<Res<Npcs>>,
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unlaid) in &unlaid {
        commands.entity(entity).despawn();
        let (Ok(at), Some(hands), Some(npcs)) = (
            places.get(unlaid.bearer),
            streak_hands(unlaid.executor, unlaid.art),
            npcs.as_deref(),
        ) else {
            continue;
        };
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, STREAK_TEXTURE));
                if image.is_none() {
                    warn!("weapon streaks: no texture {STREAK_TEXTURE}; they are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let Some(material) = material else {
            continue;
        };
        let mut held: Vec<(Entity, u8)> = weapons
            .iter()
            .filter(|(_, with, _, piece)| with.0 == unlaid.bearer && npcs.streaked(piece.item()))
            .map(|(weapon, _, wielded, _)| (weapon, wielded.slot()))
            .collect();
        held.sort_by_key(|&(_, slot)| std::cmp::Reverse(slot));
        if hands == StreakHands::One {
            held.truncate(1);
        }
        // A creature Gladiator's colour by its weapon's most damages (`0x5BEB30(−1)`).
        let most = holding
            .get(unlaid.bearer)
            .ok()
            .and_then(|holding| {
                holding
                    .items
                    .iter()
                    .find(|item| opensacred_rules::particles::streaks_on(item.class))
            })
            .map_or([0.0; 4], |item| item.damage.map(|[_, most]| most));
        let colour = streak_colour(unlaid.executor, unlaid.player, most);
        let seconds = match unlaid.lasts {
            Lasts::Seconds(seconds) => seconds,
            Lasts::WhileMaking => f32::INFINITY,
        };
        for (weapon, slot) in held {
            let mesh = meshes.add(empty());
            commands.spawn((
                Transform::from_translation(at.translation()),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
                Streak {
                    bearer: unlaid.bearer,
                    weapon,
                    look: WeaponStreak::new(seconds, colour),
                    bones: None,
                    lasts: unlaid.lasts,
                    mesh: mesh.clone(),
                },
                children![(
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                )],
            ));
            debug!(
                "weapon streaks: {} streaks {weapon} of its slot {slot} in {}",
                unlaid.bearer, unlaid.art
            );
        }
    }
}

/// Runs each streak a frame on (`0x77CA10`) and draws it (`0x77CA90`): a pair at its weapon's
/// bones while it lasts, its quads at its bearer's place; it goes with its last pair, or with its
/// bearer or its weapon.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut streaks: Query<(Entity, &mut Streak, &mut Transform)>,
    (places, children, bones, worn, making): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
        Query<(), With<crate::creature_arts::Makes>>,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut streak, mut transform) in &mut streaks {
        let streak = &mut *streak;
        let Ok(origin) = places.get(streak.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = origin;
        if streak.lasts == Lasts::WhileMaking && !making.contains(streak.bearer) {
            streak.look.stop();
        }
        if streak.bones.is_none() {
            let [first, second] = STREAK_BONES.map(|bone| {
                crate::making::bone_entity(streak.weapon, &[bone], &children, (&bones, &worn))
            });
            streak.bones = first.zip(second).map(|(a, b)| [a, b]);
        }
        // The pairs lie in the world where they are born, kept in the models' frame.
        let points = streak.bones.and_then(|[a, b]| {
            let (a, b) = (places.get(a).ok()?, places.get(b).ok()?);
            Some([models(a.translation()), models(b.translation())])
        });
        streak.look.update(dt, points);
        if streak.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let [r, g, b] = streak.look.colour;
        let mut quads = Quads::default();
        for quad in streak.look.quads() {
            quads.push_corners_shaded(
                quad.corners.map(|corner| world(corner) - origin),
                quad.uvs.map(Vec2::from),
                quad.alphas.map(|alpha| [r, g, b, alpha]),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&streak.mesh) {
            quads.in_order(orders.borne_by(streak.bearer), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The weapons' streaks in the arts.
pub struct WeaponStreaksPlugin;

impl Plugin for WeaponStreaksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
