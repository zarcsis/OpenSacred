//! The blows' sights (#1092; docs/re/effects.md, "The bleed and the parade"): at each strike that
//! does a fiftieth of its target's most health at least, `cParticleSystem_bleed` (772) sprays
//! fifty droplets of the target's blood sideways from about its neck, laid over by their alpha;
//! at each strike that does no damage, a miss or a damage resisted whole,
//! `cParticleSystem_parade` (773) bursts ten thin golden streaks from its neck, gone in a quarter
//! of a second. Both untinted by the night and drawn just behind the struck creature's model, as
//! the original draws the system before the creature it follows (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::{Bleed, Parade}`).
//!
//! Under `[gameplay] violence` a bleed leaves its blood's splashes on the ground about its
//! creature as it ends (903, `marks::splashes`, #2773). Not followed: the
//! parade's frame, taken as the camera's (**hypothesis**).

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BLEED_TEXTURE, Bleed, PARADE_COLOURS, Parade, bleed_splashes, blood_colour, blow_bleeds,
    splash_offset,
};

use crate::bolt_looks::world;
use crate::fight::Struck;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, plain_blend_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The bone the sights stand at, else the root (the handler's cases 8 and `0xA`: the bone 3).
const NECK: [&str; 1] = ["Bip01 Neck"];

/// How wide a parade's streak is, in world units: a line of a pixel at the zoom 1.
const STREAK_HALF: f32 = 0.4;

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x772))
    }
}

/// Each creature type's blood and gib set (`items.pak`'s category and gib set), read at the
/// start; one without, the heroes', bleeds red, of the set 0.
#[derive(Resource, Default)]
struct Bloods(HashMap<u32, ([f32; 3], u8)>);

/// The sights' materials, made as the first asks: the droplets laid over by their alpha, the
/// streaks of their vertices' colours laid over; none for the droplets where their texture is
/// missing.
#[derive(Resource, Default)]
struct Looks(Option<(Option<Handle<Effect>>, Handle<Effect>)>);

/// A bleed that shows: its creature, its look, its blood, its gib set and its mesh.
#[derive(Component)]
struct Bleeding {
    target: Entity,
    look: Bleed,
    colour: [f32; 3],
    gib_set: u8,
    mesh: Handle<Mesh>,
}

/// A parade that shows: its creature, its look and its mesh.
#[derive(Component)]
struct Parrying {
    target: Entity,
    look: Parade,
    mesh: Handle<Mesh>,
}

/// Reads each creature type's blood off the assets' items at the start.
fn read(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    match data.0.tables.items() {
        Ok(items) => commands.insert_resource(Bloods(
            items
                .iter()
                .map(|item| {
                    let blood = blood_colour(item.category(), item.gib_set());
                    (item.slot, (blood, item.gib_set()))
                })
                .collect(),
        )),
        Err(e) => warn!("blow sights: no bloods: {e:#}"),
    }
}

/// Makes the sight of each strike resolved (`0x532635`, `0x5329FB`): a bleed where it did a
/// fiftieth of its target's most health, a parade where it did none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    mut blows: MessageReader<Struck>,
    (places, minds, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&crate::ai::Mind>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets, bloods, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        Option<Res<Bloods>>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for blow in blows.read() {
        let parade = blow.damage == 0;
        if !parade && !blow_bleeds(blow.damage, blow.most) {
            continue;
        }
        let Ok(root) = places.get(blow.target).map(GlobalTransform::translation) else {
            continue;
        };
        let neck =
            crate::making::bone(blow.target, &NECK, &children, (&bones, &worn)).unwrap_or(root);
        let (droplets, streaks) = materials(&mut looks, data.as_deref(), &assets, &mut effects);
        if parade {
            spawn_sight(
                &mut commands,
                (Some(streaks), &mut meshes, neck),
                Kept::Parade {
                    target: blow.target,
                    at: neck,
                    look: Parade::new(&mut dice.0),
                },
            );
        } else {
            let (colour, gib_set) = minds
                .get(blow.target)
                .ok()
                .and_then(|mind| bloods.as_ref()?.0.get(&mind.creature).copied())
                .unwrap_or_else(|| (blood_colour(2, 0), 0));
            spawn_sight(
                &mut commands,
                (droplets, &mut meshes, neck),
                Kept::Bleed {
                    target: blow.target,
                    at: neck,
                    look: Bleed::new(&mut dice.0),
                    colour,
                    gib_set,
                },
            );
        }
    }
}

/// The bleeds' and the parades' materials, made as first asked.
fn materials(
    looks: &mut Looks,
    data: Option<&Data>,
    assets: &AssetServer,
    effects: &mut Assets<Effect>,
) -> (Option<Handle<Effect>>, Handle<Effect>) {
    looks
        .0
        .get_or_insert_with(|| {
            let image =
                data.and_then(|data| particle_texture(&data.0.assets, assets, BLEED_TEXTURE));
            if image.is_none() {
                warn!("blow sights: no texture {BLEED_TEXTURE}; the bleeds are not drawn");
            }
            (
                image.map(|image| effects.add(particle_material(image, AlphaMode::Blend))),
                effects.add(plain_blend_material()),
            )
        })
        .clone()
}

/// Makes the sight `kept` at its creature's neck, `at`, in `material`; none without it.
fn spawn_sight(
    commands: &mut Commands,
    (material, meshes, at): (Option<Handle<Effect>>, &mut Assets<Mesh>, Vec3),
    kept: Kept,
) {
    let Some(material) = material else {
        return;
    };
    let mesh = meshes.add(empty());
    let mut sight = commands.spawn((
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    sight.with_child((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(iso::UNTINTED),
    ));
    match kept {
        Kept::Parade { target, look, .. } => {
            sight.insert(Parrying { target, look, mesh });
        }
        Kept::Bleed {
            target,
            look,
            colour,
            gib_set,
            ..
        } => {
            sight.insert(Bleeding {
                target,
                look,
                colour,
                gib_set,
                mesh,
            });
        }
    }
}

/// A blow's sight as a save keeps it (#3812, the epic #3753): its creature by its name, where it
/// shows, its look, and a bleed's blood and gib set; a load makes it again there, going on as it
/// went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedBlowSight")]
pub enum Kept {
    Bleed {
        #[serde(with = "crate::names::required")]
        target: Entity,
        at: Vec3,
        look: Bleed,
        colour: [f32; 3],
        gib_set: u8,
    },
    Parade {
        #[serde(with = "crate::names::required")]
        target: Entity,
        at: Vec3,
        look: Parade,
    },
}

impl bevy::ecs::entity::MapEntities for Kept {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        match self {
            Self::Bleed { target, .. } | Self::Parade { target, .. } => {
                *target = mapper.get_mapped(*target);
            }
        }
    }
}

/// The blows' sights in `world` as a save keeps them (#3812).
pub(crate) fn saved(world: &mut World) -> Vec<Kept> {
    let mut bleeds = world.query::<(&Bleeding, &Transform)>();
    let mut saved: Vec<Kept> = bleeds
        .iter(world)
        .map(|(bleed, at)| Kept::Bleed {
            target: bleed.target,
            at: at.translation,
            look: bleed.look.clone(),
            colour: bleed.colour,
            gib_set: bleed.gib_set,
        })
        .collect();
    let mut parades = world.query::<(&Parrying, &Transform)>();
    saved.extend(parades.iter(world).map(|(parade, at)| Kept::Parade {
        target: parade.target,
        at: at.translation,
        look: parade.look.clone(),
    }));
    saved
}

/// Makes a loaded save's blows' sights again (#3812); one whose creature the load finds not is
/// left out.
pub(crate) fn again(world: &mut World, sights: Vec<Kept>) {
    if sights.is_empty() {
        return;
    }
    if let Err(e) = world.run_system_cached_with(sights_again, sights) {
        warn!("blow sights: the saved sights are not made: {e}");
    }
}

/// Makes the saved sights `sights` again ([`again`]).
fn sights_again(
    In(sights): In<Vec<Kept>>,
    mut commands: Commands,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    let (droplets, streaks) = materials(&mut looks, data.as_deref(), &assets, &mut effects);
    for kept in sights {
        let (target, at, material) = match &kept {
            Kept::Bleed { target, at, .. } => (*target, *at, droplets.clone()),
            Kept::Parade { target, at, .. } => (*target, *at, Some(streaks.clone())),
        };
        if crate::names::found(Some(target)).is_none() {
            continue;
        }
        spawn_sight(&mut commands, (material, &mut meshes, at), kept);
    }
}

/// The level each creature stands or lies on: the hero's its body's, another's its place's in
/// the others' way, a body's where it fell.
#[derive(bevy::ecs::system::SystemParam)]
struct Levels<'w, 's> {
    heroes: Query<'w, 's, &'static crate::hero::Body>,
    ways: Query<'w, 's, &'static crate::crowd::InTheWay>,
    corpses: Query<'w, 's, &'static crate::npcs::Corpse>,
}

impl Levels<'_, '_> {
    /// The level of `creature`, the ground's where it has none.
    fn of(&self, creature: Entity) -> crate::terrain::Level {
        self.heroes
            .get(creature)
            .map(crate::hero::Body::level)
            .or_else(|_| self.ways.get(creature).map(|way| way.level))
            .or_else(|_| self.corpses.get(creature).map(|corpse| corpse.level))
            .unwrap_or(None)
    }
}

/// Runs each bleed a frame on (`0x76AEF0`) and draws it (`0x76AFF0`): its droplets facing the
/// camera, turned by their angles, in its blood at the alpha of their life, where they flew from
/// its creature's neck; it goes with its last droplet, and then, with the violence on and but
/// for the gib sets 3 and 4, leaves its blood's splashes on the ground within 15 world units of
/// its creature along each of the cells' axes (`0x76AEF0`, `0x5B34A8`, #2777; the creature's
/// place for the bleed's object's, at the creature's height for the ground's there).
fn bleed(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Bleeding, &Transform)>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
    (orders, settings, places): (
        Orders,
        Option<Res<crate::settings::Settings>>,
        Query<&GlobalTransform>,
    ),
    levels: Levels,
) {
    let dt = time.delta_secs().min(0.1);
    let violence = settings.is_some_and(|settings| settings.gameplay.violence);
    for (entity, mut sight, at) in &mut sights {
        let sight = &mut *sight;
        sight.look.update(dt, &mut dice.0);
        if sight.look.done() {
            commands.entity(entity).despawn();
            if violence && bleed_splashes(sight.gib_set) {
                let [x, y] = splash_offset(&mut dice.0);
                let ground = places
                    .get(sight.target)
                    .map_or(at.translation, GlobalTransform::translation);
                let at = (ground + Vec3::new(x, 0.0, y), levels.of(sight.target));
                crate::marks::splashes(&mut commands, at, sight.colour);
            }
            continue;
        }
        let [r, g, b] = sight.colour;
        let mut quads = Quads::default();
        for droplet in &sight.look.system.particles {
            quads.push(
                world(droplet.place),
                droplet.size * iso::MODEL_SCALE,
                droplet.angle,
                (Vec2::ZERO, 1.0),
                [r, g, b, droplet.life.clamp(0.0, 255.0) / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&sight.mesh) {
            quads.in_order(orders.borne_by(sight.target), at.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each parade a frame on (`0x76C2F0`) and draws it (`0x76C630`): at its creature's neck,
/// each streak a line through its five points in the camera's plane (**hypothesis**: the draw's
/// matrix the camera's), from its bright head to its faint tail; it goes once they are gone.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn parry(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Parrying, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let (right, up) = (iso::right(), iso::up());
    let s = iso::MODEL_SCALE;
    for (entity, mut sight, mut transform) in &mut sights {
        let sight = &mut *sight;
        sight.look.update(dt);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(root) = places.get(sight.target).map(GlobalTransform::translation) {
            transform.translation =
                crate::making::bone(sight.target, &NECK, &children, (&bones, &worn))
                    .unwrap_or(root);
        }
        let on_screen = |[x, y, _]: [f32; 3]| (right * x + up * y) * s;
        let mut quads = Quads::default();
        for streak in &sight.look.streaks {
            for (k, pair) in streak.windows(2).enumerate() {
                let (a, b) = (on_screen(pair[0]), on_screen(pair[1]));
                let across = (b - a).cross(right.cross(up)).normalize_or_zero() * STREAK_HALF;
                let (head, tail) = (PARADE_COLOURS[k], PARADE_COLOURS[k + 1]);
                quads.push_corners_shaded(
                    [a - across, b - across, b + across, a + across],
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                    [head, tail, tail, head],
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&sight.mesh) {
            quads.in_order(orders.borne_by(sight.target), transform.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The blows' sights.
pub struct BlowSightsPlugin;

impl Plugin for BlowSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(OnEnter(AppState::InGame), read)
            .add_systems(
                Update,
                (make, bleed, parry)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
