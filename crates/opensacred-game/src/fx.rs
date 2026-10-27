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
use opensacred_rules::particles::{Blend, Fire, System, fire};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// The seed of the particles' `rand()` (**estimate**: any).
const SEED: usize = 0x76E;

/// A fire: its variant's data, its particles and its dice, and its layers' meshes.
#[derive(Component)]
pub struct Burning {
    fire: Arc<Fire>,
    system: System,
    dice: Rng,
    meshes: Vec<Handle<Mesh>>,
}

/// The fires' variants and their materials, made as the game starts.
#[derive(Resource, Default)]
struct Fires {
    variants: Vec<(u8, Arc<Fire>)>,
    /// A material of the particles' texture a blend.
    add: Option<Handle<StandardMaterial>>,
    over: Option<Handle<StandardMaterial>>,
}

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

/// A mesh of no quads, which a system fills each frame.
pub(crate) fn empty() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
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

    /// Puts the quads into `mesh`.
    pub(crate) fn into_mesh(self, mesh: &mut Mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colours);
        mesh.insert_indices(Indices::U32(self.indices));
    }
}

/// The fires' variants and their two materials, of the texture `PARTICLE_FIRE03`.
fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let variants: Vec<(u8, Arc<Fire>)> = [4, 5, 6, 10, 11, 12]
        .into_iter()
        .filter_map(|variant| Some((variant, Arc::new(fire(variant)?))))
        .collect();
    let image = data.and_then(|data| {
        let name = variants.first()?.1.texture;
        particle_texture(&data.0.assets, &assets, name)
    });
    if image.is_none() {
        warn!("fx: no particle texture; the fires are not drawn");
    }
    let mut material = |alpha_mode| {
        image
            .clone()
            .map(|image| materials.add(particle_material(image, alpha_mode)))
    };
    let (add, over) = (material(AlphaMode::Add), material(AlphaMode::Blend));
    commands.insert_resource(Fires {
        variants,
        add,
        over,
    });
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
        let Some((_, fire)) = fires.variants.iter().find(|(v, _)| *v == variant) else {
            continue;
        };
        let mut handles = Vec::new();
        commands.entity(entity).with_children(|layers| {
            for blend in &fire.blends {
                let material = match blend {
                    Blend::Add => fires.add.clone(),
                    Blend::Over => fires.over.clone(),
                };
                let Some(material) = material else {
                    continue;
                };
                let mesh = meshes.add(empty());
                handles.push(mesh.clone());
                layers.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::default(),
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        });
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        commands.entity(entity).insert((
            Burning {
                fire: fire.clone(),
                system: System::new(fire.count),
                dice: Rng::of(0, 0, SEED ^ index),
                meshes: handles,
            },
            daynight::Light(fire.light),
        ));
    }
}

/// Runs each fire's particles a frame on, and lays its layers' quads out facing the camera.
fn burn(time: Res<Time>, mut fires: Query<&mut Burning>, mut meshes: ResMut<Assets<Mesh>>) {
    let dt = time.delta_secs().min(0.1);
    for mut burning in &mut fires {
        let burning = &mut *burning;
        let fire = burning.fire.clone();
        burning.system.update(dt, &fire.layers);
        burning
            .system
            .emit(&fire.blocks, &fire.layers, fire.count, &mut burning.dice);
        for (layer, handle) in burning.meshes.iter().enumerate() {
            let Some(mut mesh) = meshes.get_mut(handle) else {
                continue;
            };
            let ramp = &fire.ramps[layer.min(fire.ramps.len() - 1)];
            let mut quads = Quads::default();
            for particle in burning
                .system
                .particles
                .iter()
                .filter(|p| usize::from(p.layer) == layer)
            {
                // The models' frame, its third coordinate up, at the world's scale.
                let [x, y, z] = particle.place;
                quads.push(
                    Vec3::new(x, z, y) * iso::MODEL_SCALE,
                    particle.size * iso::MODEL_SCALE,
                    particle.angle,
                    (Vec2::ZERO, 1.0),
                    ramp.at(particle.life),
                );
            }
            quads.into_mesh(&mut mesh);
        }
    }
}

fn end(mut commands: Commands) {
    commands.remove_resource::<Fires>();
}

/// The FX objects' particle systems.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (light.run_if(resource_exists::<Fires>), burn)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}
