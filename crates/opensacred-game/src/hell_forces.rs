//! The look of the Daemon's force of hell (63; #1092; docs/re/effects.md, "The force of hell"):
//! two `cParticleSystem_forceofhell` (1173) at its bearer, one a hand, each along the blade that
//! hand holds, between its bones `weapon_fx01` and `weapon_fx02`, for the spell's seconds: a chain
//! of soft orange beads every 2 units along it (`0x40DA80`) and tiny orange sparks raining from
//! random points along its sweep, falling, fading over 0.85 s, riding with the bearer; added by
//! their alpha, untinted by the night, drawn just behind the bearer's model, as the original draws
//! the system before the creature it follows (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::ForceOfHell`). A hand that holds nothing shows none.
//!
//! Not followed: the sparks' trails of the top detail (`0x761B00`'s trail layer).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{FORCE_COLOUR, FORCE_LINE, FORCE_TEXTURE, ForceOfHell};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A blade's bones, its glow line's ends (`+0x72` 15, `+0x74` 16, the handler's case `0x93`).
const BLADE: [&str; 2] = ["weapon_fx01", "weapon_fx02"];

/// A force to show at its bearer for its seconds, made with its spell (`spells`).
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    seconds: f32,
}

/// Starts the force of hell's look at `bearer` for `seconds`, the spell's value 0 at its level
/// (`0x5283E9`, `0x5284B2`: an object a hand, the FX event `0x93`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32) {
    commands.spawn((Unmade { bearer, seconds }, DespawnOnExit(AppState::InGame)));
}

/// The forces' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1173))
    }
}

/// The forces' material, `PARTICLE_GLOW01` added by its alpha, made as the first asks; `None`
/// where the texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A blade's force that shows: its bearer, its weapon, its look, its blade's bones once found,
/// where the blade lay a frame ago in the frame of the bearer's place, and the meshes of its
/// sparks and its line.
#[derive(Component)]
struct Blade {
    bearer: Entity,
    weapon: Entity,
    look: ForceOfHell,
    bones: Option<[Entity; 2]>,
    last: Option<[[f32; 3]; 2]>,
    meshes: [Handle<Mesh>; 2],
}

/// Gives each new force a look for each weapon its bearer holds.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    weapons: Query<(Entity, &crate::models::OrderedWith), With<crate::npcs::Wielded>>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        commands.entity(entity).despawn();
        let Ok(at) = places.get(unmade.bearer) else {
            continue;
        };
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, FORCE_TEXTURE));
                if image.is_none() {
                    warn!("hell forces: no texture {FORCE_TEXTURE}; they are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let Some(material) = material else {
            continue;
        };
        for (weapon, _) in weapons.iter().filter(|(_, with)| with.0 == unmade.bearer) {
            let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
            let mut blade = commands.spawn((
                Transform::from_translation(at.translation()),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            for mesh in &parts {
                blade.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
            blade.insert(Blade {
                bearer: unmade.bearer,
                weapon,
                look: ForceOfHell::new(unmade.seconds),
                bones: None,
                last: None,
                meshes: parts,
            });
            debug!("hell forces: {} glows along {weapon}", unmade.bearer);
        }
    }
}

/// Runs each blade's force a frame on (`0x7C3F80`) and draws it (`0x7C4000`): at its bearer's
/// place, its sparks along its blade's sweep and, within its seconds, its beads along the blade;
/// it goes with its last spark, or with its bearer.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut blades: Query<(Entity, &mut Blade, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut blade, mut transform) in &mut blades {
        let blade = &mut *blade;
        let Ok(origin) = places.get(blade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = origin;
        if blade.bones.is_none() {
            let [first, second] = BLADE.map(|bone| {
                crate::making::bone_entity(blade.weapon, &[bone], &children, (&bones, &worn))
            });
            blade.bones = first.zip(second).map(|(a, b)| [a, b]);
        }
        let points = blade.bones.and_then(|[a, b]| {
            let (a, b) = (places.get(a).ok()?, places.get(b).ok()?);
            Some([
                models(a.translation() - origin),
                models(b.translation() - origin),
            ])
        });
        let sweep = points.map(|now| (blade.last.unwrap_or(now), now));
        blade.look.update(dt, sweep, &mut dice.0);
        blade.last = points;
        if blade.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let whole = (Vec2::ZERO, 1.0);
        let mut sparks = Quads::default();
        for spark in &blade.look.system.particles {
            sparks.push(
                world(spark.place),
                spark.size * s,
                0.0,
                whole,
                ForceOfHell::colour(spark.life),
            );
        }
        let mut line = Quads::default();
        if let Some([a, b]) = points.filter(|_| blade.look.lit()) {
            let [r, g, bl] = FORCE_COLOUR;
            for bead in FORCE_LINE.points(a, b) {
                line.push(
                    world(bead),
                    ForceOfHell::bead_half(&mut dice.0) * s,
                    0.0,
                    whole,
                    [r, g, bl, 1.0],
                );
            }
        }
        let order = orders.behind(blade.bearer);
        for (handle, mut quads) in blade.meshes.iter().zip([sparks, line]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The force of hell's looks.
pub struct HellForcesPlugin;

impl Plugin for HellForcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
