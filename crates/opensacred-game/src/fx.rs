//! The FX objects' particle systems (#1092; docs/re/effects.md): for now the world's fires, the
//! smoke class's variants of `FX_FIRE_L` (884 to 886) and `FX_FI_SMOKE_*` (812, 813, 876) that
//! the scripts' `CreateObj` places with their heights (`npcs`). Each runs its particles by
//! `opensacred_rules::particles` at the frame's time and draws them as the original's billboards
//! (`0x761B00`): a quad of the particle's size facing the camera, turned by its angle, coloured
//! by its layer's ramp at its life, of `PARTICLE_FIRE03`, the flames added to the frame by their
//! alpha and the smoke laid over it by its alpha, not tinted by the night; and the fire lights
//! the night by its system's radius (`+0x9C`, docs/re/effects.md, "Lights").
//!
//! Left out (docs/decisions/2026-10-24-bolts-look.md holds the spells' stand-ins): the wind that
//! the smoke class writes into its layers (`+0x68` of the renderer, a **hypothesis**), the other
//! classes and the torches' flames.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use opensacred_rules::particles::{Blend, Fire, Geyser, Jet, System, fire};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// The seed of the particles' `rand()` (**estimate**: any).
const SEED: usize = 0x76E;

/// A fire: its variant's data, its particles and its dice, and its layers' meshes.
#[derive(Component)]
pub struct Burning {
    variant: u8,
    fire: Arc<Fire>,
    system: System,
    dice: Rng,
    meshes: Vec<Handle<Mesh>>,
    /// A geyser's clock and its jet of its own, which the clock changes.
    geyser: Option<(Geyser, Fire)>,
}

/// The fires' and smokes' variants and each one's materials of its layers, made as the game
/// starts.
#[derive(Resource, Default)]
struct Fires {
    variants: Vec<(u8, Arc<Fire>, Vec<Material>)>,
}

/// A layer's material, none without its texture.
type Material = Option<Handle<StandardMaterial>>;

/// The smoke class's variants that the game makes: the chimneys', the fire smokes', the lavas'
/// smokes and the fires (docs/re/effects.md).
const VARIANTS: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// A fire to make: its variant, as `spawn` leaves it for [`light`].
#[derive(Component)]
struct Unlit(u8);

/// Spawns the fire of the variant `variant` at `at`, in its layer and visibility.
pub fn spawn(
    commands: &mut Commands,
    variant: u8,
    at: Vec3,
    (layered, visibility): (crate::buildings::Layered, Visibility),
) -> Entity {
    commands
        .spawn((
            Unlit(variant),
            Transform::from_translation(at),
            layered,
            visibility,
        ))
        .id()
}

/// The particles' texture of `name` (`textures/texture/<number>-<name>.<extension>`), sampled
/// without its smaller levels, as the original's textures have one: the quads are small, and the
/// smaller levels' averaged alpha would dim what they add up to.
pub(crate) fn particle_texture(
    assets: &std::path::Path,
    server: &AssetServer,
    name: &str,
) -> Option<Handle<Image>> {
    let dir = assets.join("textures").join("texture");
    let entries = crate::files::read_dir(&dir).ok()?;
    let path = entries.into_iter().find_map(|entry| {
        let file = entry.file_name()?.to_string_lossy().into_owned();
        let (_, rest) = file.split_once('-')?;
        let (stem, _) = rest.rsplit_once('.')?;
        stem.eq_ignore_ascii_case(name)
            .then(|| format!("textures/texture/{file}"))
    })?;
    Some(
        server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    lod_max_clamp: 0.0,
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load::<Image>(path),
    )
}

/// A particles' material of the texture `image`, unlit, added to the frame by its alpha or laid
/// over it by its alpha.
pub(crate) fn particle_material(image: Handle<Image>, alpha_mode: AlphaMode) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// A mesh of no quads, which a system fills each frame ([`Quads::into_mesh`]).
pub(crate) fn empty() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    Quads::default().into_mesh(&mut mesh);
    mesh
}

/// Quads facing the camera, as the original's billboards (`0x761B00`), each of a centre about
/// its entity, a half side in world units, an angle in the screen's plane, a square of the texture
/// (its corner and its side, in shares of the texture) and a colour of the frame's values: Bevy's
/// own material writes them unchanged into the frame of sRGB values (`srgb`), as the original's
/// device does.
#[derive(Default)]
pub(crate) struct Quads {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Quads {
    pub(crate) fn push(
        &mut self,
        centre: Vec3,
        size: f32,
        angle: f32,
        (corner, side): (Vec2, f32),
        colour: [f32; 4],
    ) {
        let (right, up) = (iso::right(), iso::up());
        let (sin, cos) = angle.sin_cos();
        let (r, u) = (
            (right * cos + up * sin) * size,
            (up * cos - right * sin) * size,
        );
        let base = u32::try_from(self.positions.len()).unwrap_or(0);
        for (offset, [s, t]) in [
            (-r - u, [0.0, 1.0]),
            (r - u, [1.0, 1.0]),
            (r + u, [1.0, 0.0]),
            (-r + u, [0.0, 0.0]),
        ] {
            self.positions.push((centre + offset).to_array());
            self.uvs.push([corner.x + s * side, corner.y + t * side]);
            self.colours.push(colour);
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Puts the quads into `mesh`; none put a quad of no size and no colour, which draws nothing,
    /// as the renderer warns of a mesh without vertices (#1203).
    pub(crate) fn into_mesh(mut self, mesh: &mut Mesh) {
        if self.indices.is_empty() {
            self.push(Vec3::ZERO, 0.0, 0.0, (Vec2::ZERO, 0.0), [0.0; 4]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colours);
        mesh.insert_indices(Indices::U32(self.indices));
    }
}

/// The variants and the materials of their layers, one a texture and a blend.
fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut made: Vec<((&'static str, Blend), Material)> = Vec::new();
    let mut variants = Vec::new();
    for variant in VARIANTS {
        let Some(fire) = fire(variant) else {
            continue;
        };
        let layers = fire
            .blends
            .iter()
            .map(|&blend| {
                let key = (fire.texture, blend);
                if let Some((_, material)) = made.iter().find(|(k, _)| *k == key) {
                    return material.clone();
                }
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, fire.texture));
                if image.is_none() {
                    warn!("fx: no texture {}; its effects are not drawn", fire.texture);
                }
                let alpha_mode = match blend {
                    Blend::Add => AlphaMode::Add,
                    Blend::Over => AlphaMode::Blend,
                };
                let material =
                    image.map(|image| materials.add(particle_material(image, alpha_mode)));
                made.push((key, material.clone()));
                material
            })
            .collect();
        variants.push((variant, Arc::new(fire), layers));
    }
    commands.insert_resource(Fires { variants });
}

/// Gives each new fire its particles, its layers' meshes and its light.
fn light(
    mut commands: Commands,
    fires: Res<Fires>,
    unlit: Query<(Entity, &Unlit)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, &Unlit(variant)) in &unlit {
        commands.entity(entity).remove::<Unlit>();
        let Some((_, fire, materials)) = fires.variants.iter().find(|(v, ..)| *v == variant) else {
            continue;
        };
        let mut handles = Vec::new();
        commands.entity(entity).with_children(|layers| {
            for material in materials {
                let Some(material) = material.clone() else {
                    continue;
                };
                let mesh = meshes.add(empty());
                handles.push(mesh.clone());
                // A system that the light at it does not tint keeps its colours by night: the
                // camera after the night's pass draws it (#1191).
                layers
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::default(),
                        bevy::camera::visibility::NoFrustumCulling,
                    ))
                    .insert_if(
                        bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
                        || !fire.tinted,
                    );
            }
        });
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut dice = Rng::of(0, 0, SEED ^ index);
        let geyser = (variant == GEYSER).then(|| (Geyser::new(&mut dice), (**fire).clone()));
        let mut burning = commands.entity(entity);
        burning.insert(Burning {
            variant,
            fire: fire.clone(),
            system: System::new(fire.count),
            dice,
            meshes: handles,
            geyser,
        });
        if fire.light > 0.0 {
            burning.insert(daynight::Light(fire.light));
        }
    }
}

/// The geyser's variant of the smoke class, and its bursts' two sounds (`0x76E2D0`).
const GEYSER: u8 = 7;
const GEYSER_SOUNDS: [u32; 2] = [211, 212];

/// The geysers' sound's latch, which every geyser shares (`0x17EEBC8`).
#[derive(Resource, Default)]
struct GeyserLatch(bool);

/// Runs each fire's particles a frame on, and lays its layers' quads out facing the camera; a
/// geyser runs its clock first, its burst's sound at it, one of two at a rate of 0.9 to 1.1 of the
/// sample's.
fn burn(
    time: Res<Time>,
    mut fires: Query<(Entity, &mut Burning)>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut latch, mut sounds): (ResMut<GeyserLatch>, MessageWriter<crate::sfx::Play>),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut burning) in &mut fires {
        let Burning {
            fire,
            system,
            dice,
            meshes: layers,
            geyser,
            ..
        } = &mut *burning;
        let fire: &Fire = match geyser {
            Some((geyser, own)) => {
                if let Jet::Bursts { sounds: true } = geyser.update(dt, own, &mut latch.0, dice) {
                    let pick = usize::try_from(dice.below(2)).unwrap_or(0);
                    sounds.write(crate::sfx::Play {
                        sound: crate::sfx::Of::Id(GEYSER_SOUNDS[pick]),
                        at: Some(entity),
                        rate: (0.4, crate::sfx::Roll::Share(0.2)),
                    });
                }
                own
            }
            None => fire,
        };
        system.update(dt, &fire.layers);
        system.emit(&fire.blocks, &fire.layers, fire.count, dice);
        for (layer, handle) in layers.iter().enumerate() {
            let Some(mut mesh) = meshes.get_mut(handle) else {
                continue;
            };
            let ramp = &fire.ramps[layer.min(fire.ramps.len() - 1)];
            let grid = f32::from(fire.grid.max(1));
            let mut quads = Quads::default();
            for particle in system
                .particles
                .iter()
                .filter(|p| usize::from(p.layer) == layer)
            {
                // The models' frame, its third coordinate up, at the world's scale.
                let [x, y, z] = particle.place;
                // Its frame's square of the texture's grid.
                let side = fire.grid.max(1);
                let frame = Vec2::new(
                    f32::from(particle.frame % side),
                    f32::from(particle.frame / side % side),
                );
                quads.push(
                    Vec3::new(x, z, y) * iso::MODEL_SCALE,
                    particle.size * iso::MODEL_SCALE,
                    particle.angle,
                    (frame / grid, 1.0 / grid),
                    ramp.at(particle.life),
                );
            }
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The fires' loop, `INDOORFIRE_LOOP`, how many of it play at most, and how near the hero a fire
/// sounds, in the original's screen pixels (`0x690420`).
const FIRE_LOOP: u32 = 210;
const LOOPS: usize = 4;
const NEAR: i32 = 300;

/// The loop's volume of a fire of the variant `variant`, by its type: 0.3 for `FX_FI_SMOKE_S`
/// and the smallest `FX_FIRE_L`, 0.4 for the middle ones, 0.6 for the largest; none for a smoke.
fn crackle_volume(variant: u8) -> Option<f32> {
    match variant {
        4 | 10 => Some(0.3),
        5 | 11 => Some(0.4),
        6 | 12 => Some(0.6),
        _ => None,
    }
}

/// The fires' loops that play, each at its fire.
#[derive(Resource, Default)]
struct Crackles(Vec<(Entity, Handle<bevy_kira_audio::AudioInstance>)>);

/// The fires' loop near the hero (`0x690420`): at each frame, while fewer than four play, one
/// more starts at the nearest fire within 300 pixels, looping at the sample's rate; each plays at
/// its fire's place by its volume, and stops when its fire goes or is further than 300.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn crackle(
    mut crackles: ResMut<Crackles>,
    fires: Query<(Entity, &Burning, &GlobalTransform)>,
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    mut sfx: ResMut<crate::sfx::Sfx>,
    heard: Res<crate::sfx::Listener>,
    assets: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
    mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
) {
    use bevy_kira_audio::prelude::{AudioControl, AudioTween, PlaybackState};
    let Some(hero) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let near = |at: &GlobalTransform| crate::sfx::screen_distance(at.translation(), hero) <= NEAR;
    crackles.0.retain(|(fire, handle)| {
        // A loop whose instance is not made yet is starting.
        let playing = instances.get(handle).is_none_or(|instance| {
            matches!(
                instance.state(),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        });
        let keep = playing && fires.get(*fire).is_ok_and(|(_, _, at)| near(at));
        if !keep && let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(AudioTween::default());
        }
        keep
    });
    if crackles.0.len() < LOOPS
        && let Some((fire, _)) = fires
            .iter()
            .filter(|(_, burning, at)| crackle_volume(burning.variant).is_some() && near(at))
            .map(|(fire, _, at)| (fire, crate::sfx::screen_distance(at.translation(), hero)))
            .min_by_key(|&(_, d)| d)
        && let Some((id, file)) = sfx.with_id(FIRE_LOOP)
    {
        let sample = sfx.sample(id, file, &assets);
        let handle = audio
            .play(sample)
            .looped()
            .with_volume(crate::sfx::decibels(0.0))
            .handle();
        debug!(
            "fx: the fires' loop starts at {fire}, {} playing",
            crackles.0.len() + 1
        );
        crackles.0.push((fire, handle));
    }
    for (fire, handle) in &crackles.0 {
        let (Ok((_, burning, at)), Some(mut instance)) =
            (fires.get(*fire), instances.get_mut(handle))
        else {
            continue;
        };
        let (volume, pan) = crate::sfx::placed_at(at.translation(), Some(hero), *heard);
        let volume = volume * crackle_volume(burning.variant).unwrap_or(0.0);
        instance.set_decibels(crate::sfx::decibels(volume), AudioTween::default());
        instance.set_panning(pan, AudioTween::default());
    }
}

fn end(
    mut commands: Commands,
    crackles: Option<Res<Crackles>>,
    mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
) {
    commands.remove_resource::<Fires>();
    for (_, handle) in crackles.iter().flat_map(|crackles| &crackles.0) {
        if let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(bevy_kira_audio::prelude::AudioTween::default());
        }
    }
    commands.insert_resource(Crackles::default());
}

/// The FX objects' particle systems.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Crackles>()
            .init_resource::<GeyserLatch>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (light.run_if(resource_exists::<Fires>), burn, crackle)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}
