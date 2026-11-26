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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The art, the executor's type and whether a player's, as its streaks were laid.
    art: u16,
    executor: u32,
    player: bool,
}

/// An art's streaks as a save keeps them (#3774): their bearer by its name, the art, its
/// executor's type, whether a player's, how they last, and each weapon's look in the order of
/// its hands; a load lays them again on the bearer's weapons, each going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedStreaks {
    #[serde(with = "crate::names::required")]
    bearer: Entity,
    art: u16,
    executor: u32,
    player: bool,
    lasts: Lasts,
    looks: Vec<WeaponStreak>,
}

impl bevy::ecs::entity::MapEntities for SavedStreaks {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.bearer = mapper.get_mapped(self.bearer);
    }
}

/// The arts' streaks in `world` as a save keeps them (#3774), each art's of a bearer together.
pub(crate) fn saved(world: &mut World) -> Vec<SavedStreaks> {
    let mut streaks = world.query::<&Streak>();
    let mut saved: Vec<SavedStreaks> = Vec::new();
    for streak in streaks.iter(world) {
        match saved
            .iter_mut()
            .find(|kept| kept.bearer == streak.bearer && kept.art == streak.art)
        {
            Some(kept) => kept.looks.push(streak.look.clone()),
            None => saved.push(SavedStreaks {
                bearer: streak.bearer,
                art: streak.art,
                executor: streak.executor,
                player: streak.player,
                lasts: streak.lasts,
                looks: vec![streak.look.clone()],
            }),
        }
    }
    saved
}

/// Lays a loaded save's arts' streaks again (#3774); those of a bearer the load finds not are
/// left out.
pub(crate) fn again(world: &mut World, streaks: Vec<SavedStreaks>) {
    for saved in streaks {
        if crate::names::found(Some(saved.bearer)).is_none() {
            continue;
        }
        world.spawn((
            Unlaid {
                bearer: saved.bearer,
                art: saved.art,
                executor: saved.executor,
                player: saved.player,
                lasts: saved.lasts,
            },
            crate::fx::Resumes(saved.looks),
            DespawnOnExit(AppState::InGame),
        ));
    }
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
    unlaid: Query<(
        Entity,
        &Unlaid,
        Option<&crate::fx::Resumes<Vec<WeaponStreak>>>,
    )>,
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
    for (entity, unlaid, resumed) in &unlaid {
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
        for (k, (weapon, slot)) in held.into_iter().enumerate() {
            let mesh = meshes.add(empty());
            // A loaded streak goes on as it went (#3774).
            let look = resumed
                .and_then(|crate::fx::Resumes(looks)| looks.get(k).cloned())
                .unwrap_or_else(|| WeaponStreak::new(seconds, colour));
            commands.spawn((
                Transform::from_translation(at.translation()),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
                Streak {
                    bearer: unlaid.bearer,
                    weapon,
                    look,
                    bones: None,
                    lasts: unlaid.lasts,
                    mesh: mesh.clone(),
                    art: unlaid.art,
                    executor: unlaid.executor,
                    player: unlaid.player,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arts_streaks_saved_go_on_by_their_bearers_name() {
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut app = App::new();
        app.add_plugins(NamesPlugin);
        let bearer = app.world_mut().spawn(Named(Id::Hero)).id();
        let streak = |colour| Streak {
            bearer,
            weapon: Entity::PLACEHOLDER,
            look: WeaponStreak::new(1.5, colour),
            bones: None,
            lasts: Lasts::Seconds(1.5),
            mesh: Handle::default(),
            art: 1013,
            executor: 7,
            player: true,
        };
        app.world_mut().spawn(streak([1.0, 0.0, 0.0]));
        app.world_mut().spawn(streak([0.0, 1.0, 0.0]));
        let mut refs = Refs::default();
        let kept = mapped(
            saved(app.world_mut()),
            &mut Saving {
                world: app.world(),
                refs: &mut refs,
            },
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].looks.len(), 2);
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, Vec<SavedStreaks>) = postcard::from_bytes(&bytes).unwrap();
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..2).map(|_| ()));
        let bearer = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let kept = mapped(
            kept,
            &mut Loading {
                refs: &refs,
                names: loaded.world().resource::<Names>(),
                lost: 0,
            },
        );
        again(loaded.world_mut(), kept);
        let world = loaded.world_mut();
        let laid: Vec<(Entity, u16, usize)> = world
            .query::<(&Unlaid, &crate::fx::Resumes<Vec<WeaponStreak>>)>()
            .iter(world)
            .map(|(unlaid, looks)| (unlaid.bearer, unlaid.art, looks.0.len()))
            .collect();
        assert_eq!(laid, [(bearer, 1013, 2)]);
    }
}
