//! The FX objects' particle systems (#1092; docs/re/effects.md): for now the world's fires, the
//! smoke class's variants of `FX_FIRE_L` (884 to 886) and `FX_FI_SMOKE_*` (812, 813, 876) that
//! the scripts' `CreateObj` places with their heights (`npcs`). Each runs its particles by
//! `opensacred_rules::particles` at the frame's time and draws them as the original's billboards
//! (`0x761B00`): a quad of the particle's size facing the camera, turned by its angle, coloured
//! by its layer's ramp at its life, of `PARTICLE_FIRE03`, the flames added to the frame by their
//! alpha and the smoke laid over it by its alpha, not tinted by the night; and the fire lights
//! the night by its system's radius (`+0x9C`, docs/re/effects.md, "Lights").
//!
//! The small fires of `cParticleSystem_smallfire`'s variant 2 (878, `FX_SMALLFIRE`, #1808) that
//! six pieces of set 23 leave at each of the hero's steps (`0x63918C`, `sound::HeroStep`): their
//! flames and smoke as the fires', emitted for 8 s, over a patch of burnt ground (`FX_GROUNDSTEP`'s
//! frames, added to the frame) for 9 s, a light of 150, the fires' loop at 0.6, and once a second
//! a strike at each living creature hostile to the hero within their reach of the hero's place at
//! the step, of the hero's strength and dexterity (`opensacred_rules::fight::small_fire_damages`,
//! no hit chance, through the resistances).
//!
//! Left out: the wind that the smoke class writes into its layers (`+0x68` of the renderer, a
//! **hypothesis**), the other classes, the spells' among them, for which their modules draw
//! stand-ins, and the small fire's variants 0 and 1; the torches' flames are `torches`'s.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use opensacred_rules::particles::{
    Blend, DEATH_FLAMES_EMIT, Fire, GROUND_GRID, GROUND_HALF, GROUND_TEXTURE, Geyser, Jet,
    SCORCH_HALF, SMALL_FIRE_EMITS, System, death_flames, fire, ground_frame, scorch_alpha,
    scorch_texture, small_fire,
};
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
type Material = Option<Handle<Effect>>;

/// The smoke class's variants that the game makes: the chimneys', the fire smokes', the lavas'
/// smokes and the fires (docs/re/effects.md).
const VARIANTS: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// The steps' small fires' data and materials: their flame's and smoke's layers' and their burnt
/// ground's, made as the game starts.
#[derive(Resource)]
struct StepFires {
    fire: Arc<Fire>,
    layers: Vec<Material>,
    ground: Material,
}

/// A small fire that a step of the hero leaves (#1808): its particles and dice, its layers' meshes
/// and its ground's, the level of its place, and how long until it may strike again.
#[derive(Component)]
pub struct StepFire {
    system: System,
    dice: Rng,
    meshes: Vec<Handle<Mesh>>,
    ground: Option<Handle<Mesh>>,
    level: crate::terrain::Level,
    cooldown: f32,
}

/// How far above the ground the burnt ground lies, in world units, so that the ground's mesh does
/// not cover it where the original draws it at the ground's height with its depth test
/// (docs/decisions/drawing.md, "Small fire's burnt ground lifted").
const GROUND_LIFT: f32 = 0.5;
/// The fires' loop's volume of a step's small fire (`0x690630`, 0.6).
const STEP_FIRE_VOLUME: f32 = 0.6;

/// The last burnings' flames of the variants 7 and 8 and their materials, and the scorches'
/// materials of `FX_STREAKS01` and `02` (#1262), made as the game starts.
#[derive(Resource)]
struct DeathFires {
    flames: Vec<(u8, Arc<Fire>)>,
    flame: Material,
    scorches: [Material; 2],
}

/// A last burning to make: its variant, as [`last_burning`] leaves it for [`ignite`].
#[derive(Component)]
struct Unburnt(u8);

/// A last burning of the generic system's variant 7 or 8 (#1262): its flames' data, particles and
/// dice, its flames' mesh and its scorch's.
#[derive(Component)]
struct DeathFire {
    fire: Arc<Fire>,
    system: System,
    dice: Rng,
    flames: Handle<Mesh>,
    scorch: Option<Handle<Mesh>>,
}

/// Starts the last burning of the variant `variant` (7 the fire ghoul's, 8 the magic skeletons')
/// at `at` on the level `layered` (`0x5A5AE2`: put at the event's point, following nothing).
pub fn last_burning(
    commands: &mut Commands,
    variant: u8,
    at: Vec3,
    layered: Option<crate::buildings::Layered>,
) {
    let mut fire = commands.spawn((
        Unburnt(variant),
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    if let Some(layered) = layered {
        fire.insert(layered);
    }
}

/// Gives each new last burning its particles, its flames' mesh and its scorch, of `FX_STREAKS02`
/// or `01` by a coin (`0x796150`'s end), untinted.
fn ignite(
    mut commands: Commands,
    death_fires: Option<Res<DeathFires>>,
    unburnt: Query<(Entity, &Unburnt)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(death_fires) = death_fires else {
        return;
    };
    for (entity, &Unburnt(variant)) in &unburnt {
        commands.entity(entity).remove::<Unburnt>();
        let Some((_, fire)) = death_fires.flames.iter().find(|(v, _)| *v == variant) else {
            continue;
        };
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut dice = crate::seed::dice(SEED ^ index);
        let scorch_material = death_fires.scorches[usize::from(dice.below(2) == 0)].clone();
        let flames = meshes.add(empty());
        let mut scorch = None;
        let untinted = bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED);
        commands.entity(entity).with_children(|parts| {
            if let Some(material) = scorch_material {
                let mesh = meshes.add(empty());
                scorch = Some(mesh.clone());
                parts.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::Y * GROUND_LIFT),
                    bevy::camera::visibility::NoFrustumCulling,
                    untinted.clone(),
                ));
            }
            if let Some(material) = death_fires.flame.clone() {
                parts.spawn((
                    Mesh3d(flames.clone()),
                    MeshMaterial3d(material),
                    Transform::default(),
                    bevy::camera::visibility::NoFrustumCulling,
                    untinted,
                ));
            }
        });
        commands.entity(entity).insert(DeathFire {
            fire: fire.clone(),
            system: System::new(fire.count),
            dice,
            flames,
            scorch,
        });
    }
}

/// Runs each last burning a frame on (`0x7962F0`): its flames emitted for [`DEATH_FLAMES_EMIT`]
/// and aging, its scorch whole for 10 s and fading over 2 (`scorch_alpha`, `0x796A44`); it goes
/// with its scorch.
fn burn_on(
    mut commands: Commands,
    time: Res<Time>,
    mut fires: Query<(Entity, &mut DeathFire)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut burning) in &mut fires {
        let DeathFire {
            fire,
            system,
            dice,
            flames,
            scorch,
        } = &mut *burning;
        system.update(dt, &fire.layers);
        if system.clock < DEATH_FLAMES_EMIT {
            system.emit(&fire.blocks, &fire.layers, fire.count, dice);
        }
        let Some(alpha) = scorch_alpha(system.clock) else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Some(mut mesh) = meshes.get_mut(&*flames) {
            layer_quads(fire, system, 0).into_mesh(&mut mesh);
        }
        if let Some(mut mesh) = scorch.as_ref().and_then(|handle| meshes.get_mut(handle)) {
            let half = SCORCH_HALF * iso::MODEL_SCALE;
            let mut quads = Quads::default();
            quads.push_corners(
                [
                    Vec3::new(-half, 0.0, -half),
                    Vec3::new(half, 0.0, -half),
                    Vec3::new(half, 0.0, half),
                    Vec3::new(-half, 0.0, half),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
                [1.0, 1.0, 1.0, alpha],
            );
            quads.into_mesh(&mut mesh);
        }
    }
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

/// The path of the `texture.pak` texture of `name`, its case aside, relative to the asset
/// directory (`textures/texture/<number>-<name>.<extension>`).
pub(crate) fn texture_path(assets: &std::path::Path, name: &str) -> Option<String> {
    let dir = assets.join("textures").join("texture");
    let entries = crate::files::read_dir(&dir).ok()?;
    entries.into_iter().find_map(|entry| {
        let file = entry.file_name()?.to_string_lossy().into_owned();
        let (_, rest) = file.split_once('-')?;
        let (stem, _) = rest.rsplit_once('.')?;
        stem.eq_ignore_ascii_case(name)
            .then(|| format!("textures/texture/{file}"))
    })
}

/// The particles' texture of `name` (`textures/texture/<number>-<name>.<extension>`), sampled
/// without its smaller levels, as the original's textures have one: the quads are small, and the
/// smaller levels' averaged alpha would dim what they add up to.
pub(crate) fn particle_texture(
    assets: &std::path::Path,
    server: &AssetServer,
    name: &str,
) -> Option<Handle<Image>> {
    let path = texture_path(assets, name)?;
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

/// The particles' material (#2017): Bevy's own, its colour encoded for the frame of sRGB values
/// before an added one is premultiplied by its alpha (`standard.wesl`'s `PARTICLES`), so that
/// the texels' and the colours' values are laid over the frame or added to it as they are, as
/// the original's device blends them (docs/re/effects.md, "Drawing and blending"); Bevy's own
/// writes linear light, which the frame takes for its values, its middle tones dark.
pub(crate) type Effect = ExtendedMaterial<StandardMaterial, Particles>;

/// The extension of [`Effect`]: whether an added particle takes its colour whole, its alpha
/// unused, as the original's source `ONE` does (`0x761B00`'s flag `0x10`, the glows' styles but
/// 3), where Bevy's premultiplies it by its alpha: the texels of `PARTICLE_FIRE02` and of some
/// glows carry an alpha as their brightness, which that would take twice (#1092).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
#[bind_group_data(ParticlesKey)]
pub(crate) struct Particles {
    whole: bool,
}

/// What the pipeline of an [`Effect`] depends on beyond Bevy's own material.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ParticlesKey {
    whole: bool,
}

impl From<&Particles> for ParticlesKey {
    fn from(extension: &Particles) -> Self {
        Self {
            whole: extension.whole,
        }
    }
}

impl MaterialExtension for Particles {
    fn fragment_shader() -> ShaderRef {
        crate::shaders::STANDARD.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("PARTICLES".into());
            if key.bind_group_data.whole {
                fragment.shader_defs.push("WHOLE".into());
            }
        }
        Ok(())
    }
}

/// A particles' material of the texture `image`, unlit, added to the frame by its alpha or laid
/// over it by its alpha ([`Effect`]).
pub(crate) fn particle_material(image: Handle<Image>, alpha_mode: AlphaMode) -> Effect {
    Effect {
        base: StandardMaterial {
            base_color_texture: Some(image),
            unlit: true,
            alpha_mode,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: Particles::default(),
    }
}

/// A particles' material of the texture `image` added to the frame whole, its alpha unused
/// (`ONE`, `ONE`, [`Particles`]).
pub(crate) fn whole_material(image: Handle<Image>) -> Effect {
    Effect {
        extension: Particles { whole: true },
        ..particle_material(image, AlphaMode::Add)
    }
}

/// A colour of the frame's values in linear light, as the vertices of an [`Effect`] take it: its
/// texels are decoded, multiplied by it and encoded again, so that the frame takes their values'
/// product.
fn linear([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    let linear = Color::srgba(r, g, b, a).to_linear();
    [linear.red, linear.green, linear.blue, linear.alpha]
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
/// (its corner and its side, in shares of the texture) and a colour of the frame's values, which
/// an [`Effect`] multiplies the texels' values by, as the original's device does.
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
            self.colours.push(linear(colour));
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A quad facing the camera standing on its foot `foot`, `size` across each way and twice
    /// that up, turned by `angle` about its foot in the screen's plane, the texture's bottom at the
    /// foot (`0x761B00`'s flags `0x80` and `0x04`: the Daemon's rays, `bolt_looks`).
    pub(crate) fn push_footed(
        &mut self,
        foot: Vec3,
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
        self.push_corners(
            [foot - r, foot + r, foot + r + u * 2.0, foot - r + u * 2.0],
            [
                Vec2::new(corner.x, corner.y + side),
                Vec2::new(corner.x + side, corner.y + side),
                Vec2::new(corner.x + side, corner.y),
                corner,
            ],
            colour,
        );
    }

    /// A quad of the four corners `corners`, in order about it, with the texture's points `uvs`
    /// and the colour `colour`: a beam's segment (`beams`).
    pub(crate) fn push_corners(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], colour: [f32; 4]) {
        let Ok(first) = u32::try_from(self.positions.len()) else {
            return;
        };
        self.positions
            .extend(corners.map(|corner| corner.to_array()));
        self.uvs.extend(uvs.map(|uv| uv.to_array()));
        self.colours.extend([linear(colour); 4]);
        self.indices
            .extend([0, 1, 2, 0, 2, 3].map(|index| first + index));
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
    mut materials: ResMut<Assets<Effect>>,
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
    // The steps' small fire takes the fires' two materials, and its burnt ground one added.
    let step = small_fire();
    let layers = step
        .blends
        .iter()
        .map(|&blend| {
            made.iter()
                .find(|(key, _)| *key == (step.texture, blend))
                .and_then(|(_, material)| material.clone())
        })
        .collect();
    let ground = data
        .as_ref()
        .and_then(|data| particle_texture(&data.0.assets, &assets, GROUND_TEXTURE))
        .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
    commands.insert_resource(StepFires {
        fire: Arc::new(step),
        layers,
        ground,
    });
    // The last burnings: their flames of `PARTICLE_CLOUD01` added, their scorches laid over by
    // their alpha.
    let image = |name: &str| {
        data.as_ref()
            .and_then(|data| particle_texture(&data.0.assets, &assets, name))
    };
    let flames: Vec<(u8, Arc<Fire>)> = [7, 8]
        .into_iter()
        .filter_map(|variant| Some((variant, Arc::new(death_flames(variant)?))))
        .collect();
    let flame = flames
        .first()
        .and_then(|(_, fire)| image(fire.texture))
        .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
    let scorches = [false, true].map(|even| {
        image(scorch_texture(even))
            .map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
    });
    commands.insert_resource(DeathFires {
        flames,
        flame,
        scorches,
    });
    commands.insert_resource(Fires { variants });
}

/// Leaves a small fire at each step of the hero that wears six pieces of set 23 (`0x63918C`,
/// #1808), at its place on its level, untinted by the night as the fires.
fn kindle(
    mut commands: Commands,
    mut steps: MessageReader<crate::sound::HeroStep>,
    step_fires: Option<Res<StepFires>>,
    heroes: Query<(
        &crate::hero::Body,
        Option<&crate::fight::Holding>,
        Option<&crate::buildings::Layered>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(step_fires) = step_fires else {
        steps.clear();
        return;
    };
    let Some((body, holding, layered)) = heroes.iter().next() else {
        steps.clear();
        return;
    };
    let fiery = holding.is_some_and(|holding| holding.0.sets.fiery_steps());
    for &crate::sound::HeroStep(at) in steps.read() {
        if !fiery {
            continue;
        }
        let mut handles = Vec::new();
        let mut ground = None;
        let mut fire = commands.spawn((
            Transform::from_translation(at),
            Visibility::default(),
            daynight::Light(step_fires.fire.light),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(&layered) = layered {
            fire.insert(layered);
        }
        fire.with_children(|parts| {
            let untinted = bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED);
            for material in step_fires.layers.iter().flatten() {
                let mesh = meshes.add(empty());
                handles.push(mesh.clone());
                parts.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    bevy::camera::visibility::NoFrustumCulling,
                    untinted.clone(),
                ));
            }
            if let Some(material) = step_fires.ground.clone() {
                let mesh = meshes.add(empty());
                ground = Some(mesh.clone());
                parts.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::Y * GROUND_LIFT),
                    bevy::camera::visibility::NoFrustumCulling,
                    untinted,
                ));
            }
        });
        let index = usize::try_from(fire.id().index_u32()).unwrap_or(0);
        fire.insert(StepFire {
            system: System::new(step_fires.fire.count),
            dice: crate::seed::dice(SEED ^ index),
            meshes: handles,
            ground,
            level: body.level(),
            cooldown: 0.0,
        });
    }
}

/// The quads of a fire's layer `layer` of its system `system`, facing the camera.
pub(crate) fn layer_quads(fire: &Fire, system: &System, layer: usize) -> Quads {
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
    quads
}

/// Runs each step's small fire a frame on (`0x79F6D0`): its particles, emitted while its clock is
/// below [`SMALL_FIRE_EMITS`], and its burnt ground's frame (`0x79F7D0`, the quad of a half side
/// of [`GROUND_HALF`] turned by a half turn); it goes once it emits no more and its last particle
/// and its ground are gone.
fn smoulder(
    mut commands: Commands,
    time: Res<Time>,
    step_fires: Option<Res<StepFires>>,
    mut fires: Query<(Entity, &mut StepFire)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(step_fires) = step_fires else {
        return;
    };
    let fire = &*step_fires.fire;
    let dt = time.delta_secs().min(0.1);
    for (entity, mut step) in &mut fires {
        let StepFire {
            system,
            dice,
            meshes: layers,
            ground,
            ..
        } = &mut *step;
        system.update(dt, &fire.layers);
        if system.clock < SMALL_FIRE_EMITS {
            system.emit(&fire.blocks, &fire.layers, fire.count, dice);
        }
        let frame = ground_frame(system.clock);
        if system.clock >= SMALL_FIRE_EMITS && system.particles.is_empty() && frame.is_none() {
            commands.entity(entity).despawn();
            continue;
        }
        for (layer, handle) in layers.iter().enumerate() {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                layer_quads(fire, system, layer).into_mesh(&mut mesh);
            }
        }
        if let Some(mut mesh) = ground.as_ref().and_then(|handle| meshes.get_mut(handle)) {
            let mut quads = Quads::default();
            if let Some(frame) = frame {
                let half = GROUND_HALF * iso::MODEL_SCALE;
                let grid = f32::from(GROUND_GRID);
                let corner = Vec2::new(
                    f32::from(frame % GROUND_GRID),
                    f32::from(frame / GROUND_GRID % GROUND_GRID),
                ) / grid;
                let side = 1.0 / grid;
                // Turned by a half turn: the texture's far corner at the near one.
                quads.push_corners(
                    [
                        Vec3::new(-half, 0.0, -half),
                        Vec3::new(half, 0.0, -half),
                        Vec3::new(half, 0.0, half),
                        Vec3::new(-half, 0.0, half),
                    ],
                    [
                        corner + Vec2::splat(side),
                        corner + Vec2::new(0.0, side),
                        corner,
                        corner + Vec2::new(side, 0.0),
                    ],
                    [1.0; 4],
                );
            }
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Strikes from each step's small fire, at most once a second, every living creature hostile to
/// the hero on its level within [`opensacred_rules::fight::SMALL_FIRE_REACH`] and its size of it
/// (`0x5AB490`, `0x5AE436`): the hero's strike of the fire's damages of his strength and dexterity,
/// sure, through the resistances (`0x5AE44D`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn scorch(
    time: Res<Time>,
    mut hits: MessageWriter<crate::fight::Hit>,
    mut fires: Query<(&mut StepFire, &Transform)>,
    heroes: Query<(Entity, &crate::fight::Combat), With<crate::hero::Body>>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &crate::crowd::InTheWay,
            &crate::ai::Mind,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<crate::hero::Body>, Without<crate::npcs::Dies>),
    >,
) {
    let Some((hero, combat)) = heroes.iter().next() else {
        return;
    };
    let combat = &combat.0;
    let damages = opensacred_rules::fight::small_fire_damages(
        combat.attributes[0],
        combat.attributes[2],
        [combat.multipliers[0], combat.multipliers[1]],
    );
    let dt = time.delta_secs();
    for (mut step, at) in &mut fires {
        step.cooldown = (step.cooldown - dt).max(0.0);
        if step.cooldown > 0.0 {
            continue;
        }
        let here = at.translation.xz();
        let mut struck = false;
        for (target, place, way, mind, health, size) in &creatures {
            let reach = opensacred_rules::fight::SMALL_FIRE_REACH
                + size.map_or(opensacred_rules::arrows::size(0), |size| size.0);
            if way.creature
                && way.level == step.level
                && mind.hostile()
                && health.current > 0
                && (place.translation.xz() - here).length() < reach
            {
                hits.write(crate::fight::Hit {
                    attacker: hero,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                    poison: false,
                });
                struck = true;
            }
        }
        if struck {
            step.cooldown = opensacred_rules::fight::SMALL_FIRE_PERIOD;
        }
    }
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
        let mut dice = crate::seed::dice(SEED ^ index);
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
        let fire: &Arc<Fire> = fire;
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
            layer_quads(fire, system, layer).into_mesh(&mut mesh);
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

/// The loop's volume of a fire, a world's of its variant ([`crackle_volume`]) or a step's small
/// fire's ([`STEP_FIRE_VOLUME`]).
fn volume_of(burning: Option<&Burning>) -> Option<f32> {
    match burning {
        Some(burning) => crackle_volume(burning.variant),
        None => Some(STEP_FIRE_VOLUME),
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
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn crackle(
    mut crackles: ResMut<Crackles>,
    fires: Query<(Entity, Option<&Burning>, &GlobalTransform), Or<(With<Burning>, With<StepFire>)>>,
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
            .filter(|(_, burning, at)| volume_of(*burning).is_some() && near(at))
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
        let volume = volume * volume_of(burning).unwrap_or(0.0);
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
    commands.remove_resource::<StepFires>();
    commands.remove_resource::<DeathFires>();
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
        crate::shaders::embed_standard(app);
        app.add_plugins(MaterialPlugin::<Effect>::default())
            .init_resource::<Crackles>()
            .init_resource::<GeyserLatch>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (
                    light.run_if(resource_exists::<Fires>),
                    kindle,
                    ignite,
                    burn,
                    burn_on,
                    smoulder,
                    scorch,
                    crackle,
                )
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}
