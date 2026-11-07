//! The look of the Dark Elf's soulcatcher (869, `FX_SOULCATCHER`, #1092; docs/re/effects.md, "The
//! soulcatcher"): while its object hangs at him (`making`), twirls spin 40 over his neck, riding
//! with him, faint gold added and dark smudges laid over, untinted by the night; each soul it
//! absorbs (`souls`) swells them and lights a pale yellow glow that shrinks away
//! (`opensacred_rules::particles::Soulcatcher`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Soulcatcher};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The bones the twirls hang over (`0x44CB80`): the neck, else the root.
const NECK: [&str; 2] = ["Bip01 Neck", "__Root"];

/// How many runs of one layer's twirls in their slots' order it draws, each a mesh of its own
/// (`0x761B00`): the original draws both layers in one batch in their slots' order, switching
/// the blend at each change of layer, so that each black smudge dims the gold drawn before it
/// and the gold after it adds over it; the runs past these go into the last two.
const RUNS: usize = 16;

/// The soulcatcher's object at its bearer, a child of his, which takes its look ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Catcher;

/// A soul absorbed by the soulcatcher of the hero `0` (`souls`, `0x76A8F7`).
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct Absorbs(pub(crate) Entity);

/// The twirls' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x869))
    }
}

/// The materials of the twirls' runs, the gold's at the even and the black's at the odd, each
/// drawn after the one before it, and of the glow, made at the first soulcatcher; none where its
/// texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Materials>);

/// The twirls' runs' materials and the glow's.
type Materials = (Vec<Handle<Effect>>, Option<Handle<Effect>>);

/// A soulcatcher's look: its object, its bearer, its system and its meshes of the twirls' runs
/// and of the glow.
#[derive(Component)]
struct Shown {
    catcher: Entity,
    bearer: Entity,
    look: Soulcatcher,
    runs: Vec<Handle<Mesh>>,
    glow: Handle<Mesh>,
}

/// Gives each new soulcatcher its look (`0x76A6D0`, `0x76AA40`).
fn show(
    mut commands: Commands,
    catchers: Query<(Entity, &ChildOf), Added<Catcher>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (catcher, bearer) in &catchers {
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let texture = |name: &str| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("soulcatchers: no texture {name}; its part is not drawn");
                    }
                    image
                };
                // The gold added by its alpha (`SRCALPHA`, `ONE`), the black laid over, each run
                // after the one before by its order's bias (`blended`'s sense: higher later), too
                // small to move its depth's test.
                let runs = texture(particles::SOULCATCHER_TEXTURE)
                    .map(|image| {
                        (0..RUNS)
                            .map(|run| {
                                let blend = if run % 2 == 0 {
                                    AlphaMode::Add
                                } else {
                                    AlphaMode::Blend
                                };
                                let mut material = particle_material(image.clone(), blend);
                                material.base.depth_bias =
                                    f32::from(u8::try_from(run).unwrap_or(u8::MAX));
                                effects.add(material)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let glow = texture(particles::SOULCATCHER_GLOW_TEXTURE)
                    .map(|image| effects.add(whole_material(image)));
                (runs, glow)
            })
            .clone();
        let (runs, glow) = materials;
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        let mut part = |material: Handle<Effect>| {
            let mesh = meshes.add(empty());
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
            mesh
        };
        let runs: Vec<Handle<Mesh>> = runs.into_iter().map(&mut part).collect();
        let glow = match glow {
            Some(material) => part(material),
            None => meshes.add(empty()),
        };
        shown.insert(Shown {
            catcher,
            bearer: bearer.parent(),
            look: Soulcatcher::default(),
            runs,
            glow,
        });
    }
}

/// Runs each soulcatcher's look a frame on (`0x76A860`, `0x76A960`): its twirls over its bearer's
/// neck, riding with him (the flags 4 and `0x200`), its swelling at each soul, and its end with
/// its object's, its twirls cut off.
fn twirl(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    (catchers, bearers): (Query<(), With<Catcher>>, Query<&GlobalTransform>),
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut absorbed: MessageReader<Absorbs>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let absorbed: Vec<Entity> = absorbed.read().map(|soul| soul.0).collect();
    let [gold, black_ramp] = Soulcatcher::ramps();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let Some(place) = catchers
            .contains(look.catcher)
            .then(|| bearers.get(look.bearer).ok())
            .flatten()
        else {
            commands.entity(entity).despawn();
            continue;
        };
        if absorbed.contains(&look.bearer) {
            look.look.absorb();
        }
        // Its particles ride with its bearer's place on the ground at the height 0 (the flag 4).
        let at = place.translation();
        let origin = Vec3::new(at.x, 0.0, at.z);
        let neck =
            crate::making::bone(look.bearer, &NECK, &children, (&bones, &worn)).unwrap_or(at);
        let bone = models(neck - origin);
        look.look.update(dt, bone, &mut dice.0);
        // Each run of one layer in their slots' order into a mesh of its own, the gold's at the
        // even, the black's at the odd.
        let mut runs: Vec<Quads> = (0..look.runs.len()).map(|_| Quads::default()).collect();
        let mut run: Option<usize> = None;
        for twirl in &look.look.system.particles {
            let black = usize::from(twirl.layer != 0);
            let at = match run {
                Some(at) if at % 2 == black => at,
                Some(at) if at + 1 < runs.len() => at + 1,
                Some(at) => at.saturating_sub(1),
                None => black,
            };
            run = Some(at);
            let ramp = if black == 0 { &gold } else { &black_ramp };
            if let Some(quads) = runs.get_mut(at) {
                quads.push(
                    origin + world(twirl.place),
                    twirl.size * s,
                    twirl.angle,
                    (Vec2::ZERO, 1.0),
                    ramp.at(twirl.life),
                );
            }
        }
        let mut glow = Quads::default();
        if let Some(half) = look.look.glow() {
            let [x, y, z] = bone;
            glow.push(
                origin + world([x, y, z + particles::SOULCATCHER_HIGH]),
                half * s,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::SOULCATCHER_GLOW,
            );
        }
        // Just behind his model, as the original draws the system before the creature (#2188).
        let order = orders.behind(look.bearer);
        for (mut quads, mesh) in runs
            .into_iter()
            .chain([glow])
            .zip(look.runs.iter().chain([&look.glow]))
        {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, Vec3::ZERO);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The soulcatchers' looks.
pub struct SoulcatchersPlugin;

impl Plugin for SoulcatchersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Absorbs>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, twirl)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
