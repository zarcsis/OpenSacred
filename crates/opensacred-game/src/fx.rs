//! The FX objects' particle systems (#1092; docs/re/effects.md): the world's fires and smokes, the
//! smoke class's variants 1 to 13 (the chimneys' smokes 771, 810 and 811, the fire smokes
//! `FX_FI_SMOKE_*` 812, 813 and 876, the geyser 879, the lavas' smokes 882 and 881, the fires
//! `FX_FIRE_L` 884 to 886 and the magic fire 904) that the scripts' `CreateObj` places with their
//! heights (`npcs`). Each runs its particles by `opensacred_rules::particles` at the frame's time
//! and draws them as the original's billboards (`0x761B00`): a quad of the particle's size facing
//! the camera, turned by its angle, coloured by its layer's ramp at its life, of its variant's
//! texture, each layer added to the frame or laid over it, the chimneys' smokes and the geyser
//! tinted by the night and the rest not; and a fire lights the night by its system's radius
//! (`+0x9C`, docs/re/effects.md, "Lights"). A geyser bursts by its clock, with its sound; the two
//! larger fire smokes and a bursting geyser strike the creatures in them once a second (`scald`,
//! #2518); and the fires' loop plays at the fires near the hero (`crackle`, `0x690420`).
//!
//! The small fires of `cParticleSystem_smallfire`'s variant 2 (878, `FX_SMALLFIRE`, #1808) that
//! six pieces of set 23 leave at each of the hero's steps (`0x63918C`, `sound::HeroStep`): their
//! flames and smoke as the fires', emitted for 8 s, over a patch of burnt ground (`FX_GROUNDSTEP`'s
//! frames, added to the frame) for 9 s, a light of 150, the fires' loop at 0.6, and once a second
//! a strike at each living creature hostile to the hero within their reach of the hero's place at
//! the step, of the hero's strength and dexterity (`opensacred_rules::fight::small_fire_damages`,
//! no hit chance, through the resistances); and those that a Big Demon's steps leave, its strikes
//! at the living hostile to it, the hero among them (#3028).
//!
//! The last burnings (#1262), the generic system's variants 7 and 8 that `undead` starts where a
//! fire ghoul or a magic skeleton dies its last death: their flames, emitted for a second, over a
//! scorch on the ground that stays for 10 s and fades.
//!
//! The other effects' modules draw with this one's parts: the particles' material ([`Effect`],
//! #2017) and its makers, the quads of a mesh ([`Quads`]), the particles' textures, and the depth
//! of a system's place in the original's order of drawing ([`Orders`], [`InOrder`], #2178).
//!
//! The smoke class's layers take the renderer's wind at each frame (`0x76E2D0`,
//! `daynight::Clock::wind`, #3154), which carries its rising flames and smokes against it. A fire
//! of flames added and smoke laid over, the steps' small fires among them, draws both as one mesh
//! in their particles' slots' order, as the original switches the blend at each change of layer
//! (`0x761B00`, [`mixed_material`], #3661), so that a flame born after the smoke about it lies
//! over it whichever way the wind lays them.
//!
//! Left out: the small fire's variant 1. The other classes are their own modules', the small
//! fire's variant 0 `lava_bursts`'s and the torches' flames `torches`'s.

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
    Particle, SCORCH_HALF, SMALL_FIRE_EMITS, System, breath_small_fire, death_flames, fire,
    ground_frame, scorch_alpha, scorch_texture, small_fire,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// The seed of the particles' `rand()` (**estimate**: any).
const SEED: usize = 0x76E;

/// A loaded look's state, which its module's making takes in place of a new one (#3773, the
/// epic #3753): a look made of a kept state, or a passing sight made again by a load.
#[derive(Component)]
pub struct Resumes<S: Send + Sync + 'static>(pub S);

/// How many steps a load's first frame played runs the fixed systems on by ([`Preroll`]), and
/// each step's seconds: 6 s.
const PREROLL_STEPS: usize = 120;
const PREROLL_STEP: f32 = 0.05;

/// Whether the fixed systems pre-roll, from a load's resume (`saves`) to the end of its first
/// frame played (#3805; docs/decisions/drawing.md, "The sectors' fixed systems come back burning,
/// not particle for particle"): the steady systems that the sectors, the scripts and the worn
/// items make anew as they come in, which a save does not keep, run on by [`Preroll::steps`]
/// before they are first drawn, so that a fire in view stands burning rather than building up.
#[derive(Resource, Default)]
pub struct Preroll(pub bool);

impl Preroll {
    /// The steps that a frame of `dt` seconds runs a fixed system on by, each with whether it is
    /// the frame's own: the frame's alone, and as a load's first frame is played the pre-roll's
    /// before it.
    pub fn steps(&self, dt: f32) -> impl Iterator<Item = (f32, bool)> {
        let count = if self.0 { PREROLL_STEPS } else { 0 };
        std::iter::repeat_n((PREROLL_STEP, false), count).chain(std::iter::once((dt, true)))
    }
}

/// Ends a load's pre-roll once its first frame is played.
fn end_preroll(mut preroll: ResMut<Preroll>) {
    if preroll.0 {
        preroll.0 = false;
    }
}

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
    /// The level of its place, and how long until it may strike again ([`scald`]).
    level: crate::terrain::Level,
    cooldown: f32,
    /// Whether its type puts it in list 4 ([`over_all`]).
    over: bool,
}

/// The fires' and smokes' variants and each one's materials of its layers, made as the game
/// starts, and the same of list 4 ([`over_all`]).
#[derive(Resource, Default)]
struct Fires {
    variants: Vec<(u8, Arc<Fire>, Layers)>,
}

/// A variant's materials of its layers, and the same of list 4 ([`over_all`]).
type Layers = [Vec<Material>; 2];

/// A layer's material, none without its texture.
type Material = Option<Handle<Effect>>;

/// What a fire's material is made of: its texture, its blend, whether it is of list 4 and whether
/// the night tints it.
type MaterialKey = (&'static str, Blend, bool, bool);

/// The smoke class's variants that the game makes: the chimneys', the fire smokes', the geyser,
/// the lavas' smokes, the fires and the magic fire (docs/re/effects.md).
const VARIANTS: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

/// The steps' small fires' data and materials: their flame's and smoke's layers' and their burnt
/// ground's, made as the game starts.
#[derive(Resource)]
struct StepFires {
    fire: Arc<Fire>,
    breath: Arc<Fire>,
    layers: Vec<Material>,
    ground: Material,
}

/// A small fire that a step of the hero leaves (#1808), a step of a Big Demon's (#3028), a
/// creature's cast (#2898) or a bounce of the Daemon's breath (#3301): its variant's data, its
/// particles and dice, its layers' meshes and its ground's, the level of its place, the creature
/// whose strike its strikes are, and how long until it may strike again.
#[derive(Component)]
pub struct StepFire {
    fire: Arc<Fire>,
    system: System,
    dice: Rng,
    meshes: Vec<Handle<Mesh>>,
    ground: Option<Handle<Mesh>>,
    level: crate::terrain::Level,
    caster: Option<Entity>,
    cooldown: f32,
}

/// A small fire of the variant 2 at `at` on `level`, of the caster `caster`, whose strike its
/// strikes are: a Big Demon's at each of its steps (`0x639532`, the demon its caster; #3028), or
/// the one that a big demon's fire, the spell 50, leaves at its feet (`0x527669`: 878 by the FX
/// event `0x65`), whose caster is the object's own id, for which its strikes gather no target, so
/// that it never strikes: none here (docs/re/arts.md, "The creatures' own spells"; #2898).
///
/// The Daemon's breath leaves one of the variant 1 at each bounce, narrower and shorter, its
/// caster the breath's (`0x5AA5A9`, `breath`; #3301): `breath`.
#[derive(Message, Clone, Copy, Debug)]
pub struct SmallFire {
    pub at: Vec3,
    pub level: crate::terrain::Level,
    pub caster: Option<Entity>,
    pub breath: bool,
}

/// How far above the ground the burnt ground lies, in world units, so that the ground's mesh does
/// not cover it where the original draws it at the ground's height with its depth test
/// (docs/decisions/drawing.md, "Small fire's burnt ground lifted").
const GROUND_LIFT: f32 = 0.5;
/// The fires' loop's place's factor of a step's small fire (`0x690630`, 0.6).
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
            let half = SCORCH_HALF;
            let mut quads = Quads::default();
            quads.push_corners(
                [
                    world([-half, -half, 0.0]),
                    world([half, -half, 0.0]),
                    world([half, half, 0.0]),
                    world([-half, half, 0.0]),
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

/// A fire to make: its variant, the level of its place and whether it is of list 4, as `spawn`
/// leaves it for [`light`].
#[derive(Component)]
struct Unlit(u8, crate::terrain::Level, bool);

/// Spawns the fire of the variant `variant` at `at` on `level`, in its layer and visibility, of
/// list 4 where `over` (its type's flag `0x800000`, #3286).
pub fn spawn(
    commands: &mut Commands,
    (variant, level, over): (u8, crate::terrain::Level, bool),
    at: Vec3,
    (layered, visibility): (crate::buildings::Layered, Visibility),
) -> Entity {
    commands
        .spawn((
            Unlit(variant, level, over),
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

/// [`particle_texture`], repeated along its width: the lightning's strips, whose `u` runs on past
/// 1 (`lightning_strikes`).
pub(crate) fn repeated_texture(
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
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load::<Image>(path),
    )
}

/// [`particle_texture`], repeated along both its axes: a model's texture whose coordinates
/// scroll (`dwarf_burrows`).
pub(crate) fn wrapped_texture(
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
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
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
/// glows carry an alpha as their brightness, which that would take twice (#1092); whether its
/// `depth_bias` is a blended layer's, for its order alone ([`layered`]); whether it is of list
/// 4 ([`over_all`]); and whether it draws a system's added and laid layers in one mesh
/// ([`mixed_material`]).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
#[bind_group_data(ParticlesKey)]
pub(crate) struct Particles {
    whole: bool,
    layered: bool,
    over: bool,
    mixed: bool,
}

/// What the pipeline of an [`Effect`] depends on beyond Bevy's own material.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ParticlesKey {
    whole: bool,
    layered: bool,
    over: bool,
    mixed: bool,
}

impl From<&Particles> for ParticlesKey {
    fn from(extension: &Particles) -> Self {
        Self {
            whole: extension.whole,
            layered: extension.layered,
            over: extension.over,
            mixed: extension.mixed,
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
            if key.bind_group_data.mixed {
                fragment.shader_defs.push("MIXED".into());
            }
            if key.bind_group_data.over {
                // A model's texel nearer than its true point hides it (`standard.wesl`).
                fragment.shader_defs.push("NEAREST".into());
                fragment.shader_defs.push("LIST_4".into());
            }
        }
        if key.bind_group_data.layered {
            crate::blended::unbias(descriptor);
        }
        Ok(())
    }
}

/// `material` in the blended layer `layer`: drawn after every mesh of the layers before it and
/// before every mesh of those after, its depth as it is (`blended`, [`Particles`]).
pub(crate) fn layered(material: Effect, layer: crate::blended::Layer) -> Effect {
    Effect {
        base: StandardMaterial {
            depth_bias: layer.bias(),
            ..material.base
        },
        extension: Particles {
            layered: true,
            ..material.extension
        },
    }
}

/// `material` among the objects of list 3 ([`crate::blended::Layer::Objects`]): an effect that the
/// world's camera draws, which the original draws in list 3 after its cell's other objects, over
/// the underlays of lists 0 and 2 and the statics' shadows of list 1 whatever their rows, and among
/// list 3's sprites and models by its depth (docs/re/drawing.md, "What the game does with it",
/// #3459).
pub(crate) fn among_objects(material: Effect) -> Effect {
    layered(material, crate::blended::Layer::Objects)
}

/// `material` of an FX object of list 4 (#3286): in the last of the blended layers, over every
/// sprite's texels, its quads [`crate::statics::LIST_4_FORWARD`] nearer the camera than they lie
/// ([`Quads::ahead`]), so that every sprite and model lies behind them, and each of its fragments
/// dropped where a model's texel lies nearer than the fragment's true point (`standard.wesl`'s
/// `LIST_4`, the models' prepass): as the original draws list 4 last, the sprites without the
/// depth test, and its particles tested against the models' depths (`0x62AE90`, `0x761B00`).
pub(crate) fn over_all(material: Effect) -> Effect {
    let material = layered(material, crate::blended::Layer::Overlays);
    Effect {
        extension: Particles {
            over: true,
            ..material.extension
        },
        ..material
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
        extension: Particles {
            whole: true,
            ..Particles::default()
        },
        ..particle_material(image, AlphaMode::Add)
    }
}

/// A particles' material of the texture `image` for a system of two layers, one added to the frame
/// by its alpha and one laid over it (`SRCALPHA` with `ONE` or `INVSRCALPHA`), drawn as one mesh
/// in their particles' slots' order, as the original switches the blend at each change of layer
/// (`0x761B00`, docs/re/effects.md, "The billboards"): premultiplied, an added particle's colour
/// taking nothing of the frame away, a laid one's covering it by its alpha; its added particles'
/// vertices carry their alpha negative ([`mixed_quads`], `standard.wesl`'s `MIXED`).
pub(crate) fn mixed_material(image: Handle<Image>) -> Effect {
    Effect {
        extension: Particles {
            mixed: true,
            ..Particles::default()
        },
        ..particle_material(image, AlphaMode::Premultiplied)
    }
}

/// A material of no texture, of its vertices' colours alone, added to the frame by their alpha
/// (`SRCALPHA`, `ONE`): the geometry that a system draws with its texture stage emptied, as the
/// holy light's column (`holy_lights`).
pub(crate) fn plain_material() -> Effect {
    Effect {
        base: StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: Particles::default(),
    }
}

/// A material of no texture, of its vertices' colours alone, laid over the frame by their alpha
/// (`SRCALPHA`, `INVSRCALPHA`), its depth tested: the parade's streaks (`blow_sights`), and the
/// summoning's triangles and bolts, which the original draws with its depth not tested, laid at
/// their place's depth ([`Quads::at_order`]).
pub(crate) fn plain_blend_material() -> Effect {
    Effect {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            ..plain_material().base
        },
        extension: Particles::default(),
    }
}

/// A colour of the frame's values in linear light, as the vertices of an [`Effect`] take it: its
/// texels are decoded, multiplied by it and encoded again, so that the frame takes their values'
/// product.
pub(crate) fn linear([r, g, b, a]: [f32; 4]) -> [f32; 4] {
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

    /// A ray facing the camera from its foot `foot`, `across` to each side and `length` up, turned
    /// by `angle` about its foot in the screen's plane, the whole texture, its bottom at the foot
    /// (`0x761B00`'s flags `0x80` and `0x04` of a factor below 1: the demon fire's rays,
    /// `demon_fires`), but its first `hidden` up from the foot, which something that writes the
    /// depth hides there (a ball about the foot).
    pub(crate) fn push_ray(
        &mut self,
        foot: Vec3,
        (across, length, hidden): (f32, f32, f32),
        angle: f32,
        colour: [f32; 4],
    ) {
        if hidden >= length {
            return;
        }
        let (right, up) = (iso::right(), iso::up());
        let (sin, cos) = angle.sin_cos();
        let (r, along) = ((right * cos + up * sin) * across, up * cos - right * sin);
        let (from, to) = (foot + along * hidden.max(0.0), foot + along * length);
        let v = 1.0 - hidden.max(0.0) / length;
        self.push_corners(
            [from - r, from + r, to + r, to - r],
            [Vec2::new(0.0, v), Vec2::new(1.0, v), Vec2::X, Vec2::ZERO],
            colour,
        );
    }

    /// A quad facing the camera about `centre`, of the half side `size`, the whole texture, its
    /// corners' colours `colours` from the bottom left, the top left, the bottom right and the top
    /// right (the blades of light's dots, `light_blades`).
    pub(crate) fn push_facing(&mut self, centre: Vec3, size: f32, colours: [[f32; 4]; 4]) {
        self.push_facing_turned(centre, size, 0.0, colours);
    }

    /// [`Self::push_facing`] turned by `angle` in the screen's plane (the flags `0x04`: the
    /// healing's sparks, `healings`).
    pub(crate) fn push_facing_turned(
        &mut self,
        centre: Vec3,
        size: f32,
        angle: f32,
        colours: [[f32; 4]; 4],
    ) {
        let (right, up) = (iso::right(), iso::up());
        let (sin, cos) = angle.sin_cos();
        let (r, u) = (
            (right * cos + up * sin) * size,
            (up * cos - right * sin) * size,
        );
        let Ok(first) = u32::try_from(self.positions.len()) else {
            return;
        };
        let [bottom_left, top_left, bottom_right, top_right] = colours;
        for (offset, uv, colour) in [
            (-r - u, [0.0, 1.0], bottom_left),
            (r - u, [1.0, 1.0], bottom_right),
            (r + u, [1.0, 0.0], top_right),
            (-r + u, [0.0, 0.0], top_left),
        ] {
            self.positions.push((centre + offset).to_array());
            self.uvs.push(uv);
            self.colours.push(linear(colour));
        }
        // Split from its top left to its bottom right, as the original's strip of the bottom
        // left, the top left, the bottom right and the top right is: the colour at its middle,
        // where its texture is brightest, is the mean of those two corners'.
        self.indices
            .extend([0, 3, 1, 3, 2, 1].map(|index| first + index));
    }

    /// A quad of the four corners `corners`, in order about it, each its own colour of `colours`
    /// (the holy light's column, `holy_lights`), split from its second corner to its fourth, as
    /// the original's strips are.
    pub(crate) fn push_shaded(&mut self, corners: [Vec3; 4], colours: [[f32; 4]; 4]) {
        let Ok(first) = u32::try_from(self.positions.len()) else {
            return;
        };
        self.positions
            .extend(corners.map(|corner| corner.to_array()));
        self.uvs.extend([[0.0, 0.0]; 4]);
        self.colours.extend(colours.map(linear));
        self.indices
            .extend([0, 1, 3, 1, 2, 3].map(|index| first + index));
    }

    /// A quad of the four corners `corners`, in order about it, with the texture's points `uvs`
    /// and the colour `colour`: a beam's segment (`beams`).
    pub(crate) fn push_corners(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], colour: [f32; 4]) {
        self.push_corners_shaded(corners, uvs, [colour; 4]);
    }

    /// [`Self::push_corners`] with each corner its own colour of `colours` (the light strikes'
    /// fade to their top, `light_strikes`).
    pub(crate) fn push_corners_shaded(
        &mut self,
        corners: [Vec3; 4],
        uvs: [Vec2; 4],
        colours: [[f32; 4]; 4],
    ) {
        let Ok(first) = u32::try_from(self.positions.len()) else {
            return;
        };
        self.positions
            .extend(corners.map(|corner| corner.to_array()));
        self.uvs.extend(uvs.map(|uv| uv.to_array()));
        self.colours.extend(colours.map(linear));
        self.indices
            .extend([0, 1, 2, 0, 2, 3].map(|index| first + index));
    }

    /// A flat ring about `centre` between the radii `inner` and `outer` in world units, of
    /// `segments` quads in `colour`, each the texture's width along it and its height from the
    /// inner radius out (`0x763530`: the rings of poison and of the fear).
    pub(crate) fn push_ring(
        &mut self,
        centre: Vec3,
        (inner, outer): (f32, f32),
        segments: u16,
        colour: [f32; 4],
    ) {
        let (across, down) = iso::fx_axes();
        let edge = |k: u16, radius: f32| {
            let angle = std::f32::consts::TAU * f32::from(k) / f32::from(segments.max(1));
            centre + (across * angle.sin() + down * angle.cos()) * radius
        };
        for k in 0..segments {
            self.push_corners(
                [
                    edge(k, inner),
                    edge(k, outer),
                    edge(k + 1, outer),
                    edge(k + 1, inner),
                ],
                [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X],
                colour,
            );
        }
    }

    /// Lays every vertex nearer the camera than the depth of `order` back to it along the line of
    /// sight, where one is given, the mesh's origin at `origin` (its entity's place, of no turn and
    /// no scale): in the camera's orthographic view each vertex shows where it did, none nearer
    /// than its system's place in the original's order of drawing ([`Orders`], #2178), so that a
    /// creature drawn after the system covers it; a vertex further back keeps its depth, hidden
    /// by a creature drawn before the system in front of it, as the original's depth test hides
    /// it (#2188).
    /// A system that its bearer carries ([`Orders::borne_by`]) lays each vertex as
    /// [`Self::as_drawn`] does.
    pub(crate) fn in_order(&mut self, order: Option<InOrder>, origin: Vec3) {
        let Some(order) = order else {
            return;
        };
        for position in &mut self.positions {
            let vertex = Vec3::from_array(*position);
            *position = if order.drawn {
                order.drawn(vertex, origin)
            } else {
                order.no_nearer(vertex, origin)
            }
            .to_array();
        }
    }

    /// Brings every vertex `by` world units nearer the camera along the line of sight, where it
    /// shows still in the camera's orthographic view: the quads of list 4 ([`over_all`]).
    pub(crate) fn ahead(&mut self, by: f32) {
        let back = -iso::forward() * by;
        for position in &mut self.positions {
            *position = (Vec3::from_array(*position) + back).to_array();
        }
    }

    /// Lays every vertex at the depth of `order` along the line of sight, where one is given, the
    /// mesh's origin at `origin`: the quads of a system that the original draws with its depth
    /// not tested, at its place in the order (the stargates' whirls, the summonings' triangles
    /// and bolts, #2220), tested here at that one depth, so that they lie over what the order
    /// draws before them and under what it draws after, as the original's lie.
    pub(crate) fn at_order(&mut self, order: Option<InOrder>, origin: Vec3) {
        let Some(order) = order else {
            return;
        };
        for position in &mut self.positions {
            *position = order.on(Vec3::from_array(*position), origin).to_array();
        }
    }

    /// Lays every vertex where a texel of the model of `order` at its place lies, where one is
    /// given ([`Orders::over`]), the mesh's origin at `origin`, the model's root: a system that
    /// the original draws after the model, depth tested against it, so that what lies before the
    /// model's surface covers it and what lies behind it stays hidden (the level-up's sparks,
    /// #2762).
    pub(crate) fn as_drawn(&mut self, order: Option<InOrder>, origin: Vec3) {
        let Some(order) = order else {
            return;
        };
        for position in &mut self.positions {
            *position = order.drawn(Vec3::from_array(*position), origin).to_array();
        }
    }

    /// Appends the quads of `other` to its own: the systems of many objects in one mesh, each
    /// laid in its own order first ([`Self::in_order`]; the items' glitter, #2220).
    pub(crate) fn append(&mut self, other: Quads) {
        let Ok(first) = u32::try_from(self.positions.len()) else {
            return;
        };
        self.positions.extend(other.positions);
        self.uvs.extend(other.uvs);
        self.colours.extend(other.colours);
        self.indices
            .extend(other.indices.into_iter().map(|index| first + index));
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

/// How far forward a place comes per world unit that its cell lies right of the point the camera
/// looks at (`ACROSS` of `order.wesl`).
const ACROSS: f32 = 0.002;
/// How far forward per place in a cell (`CHAIN` of `order.wesl`).
const CHAIN: f32 = 0.0045;
/// The part of a model's vertex's own depth from its anchor that it keeps (`SQUEEZE` of
/// `models.wesl`).
const SQUEEZE: f32 = 0.01;
/// How far behind its root along the line of sight the back of a creature's body lies, in world
/// units (**estimate**: about half a man's depth): a system at its bearer's place lies that far
/// behind it, so that the bearer's model, whose texels lie at their own depths from its root,
/// covers it.
const BODY: f32 = 20.0;
/// How far nearer the camera than a texel of a model at its place a system drawn over the model
/// lays its vertices, in world units of its own depth ([`Orders::over`]): a quad at a vertex of
/// the model's surface covers the surface about it rather than meeting it, as the original's
/// sparks there show whole.
const OVER: f32 = 2.0;

/// The depth of a system's place in the original's order of drawing, where it lays its quads
/// ([`Quads::in_order`]): the place of its cell at height 0, and how far forward of that place
/// along the line of sight the system lies (`order.wesl`'s order beyond the place's own depth).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InOrder {
    place: Vec3,
    beyond: f32,
    /// The upper storey's floor that the system stands on, if any, whose plane the order lies
    /// no further back than.
    floor: Option<OnFloor>,
    /// Whether its system is drawn as its bearer's model's texels, each vertex at its own depth
    /// from the root ([`Orders::over`], [`Self::drawn`]), rather than no nearer than its place.
    drawn: bool,
    /// The bearer's model's root, from which [`Self::drawn`] takes a vertex's own depth.
    root: Vec3,
}

/// An upper storey's floor under a system, as `models.wesl`'s `on_floor` lays a model standing
/// on it: the height of its plane, how far behind the ground point under a point of the plane
/// what stands there lies at most, in its order among the quads standing on the floor
/// ([`crate::statics::Floor::under`]), the point the camera looks at, and the part of the
/// system's own depth that it keeps (`ON_FLOOR_SQUEEZE`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct OnFloor {
    height: f32,
    behind: f32,
    looked: Vec3,
    own: f32,
}

impl OnFloor {
    /// The floor `floor` of [`crate::statics::Floor::under`] (its plane's height and how far
    /// behind), the camera looking at `looked`, a system `behind` units of its own depth
    /// behind a model of the anchor `anchor` rooted at `root`.
    fn new(floor: Vec4, anchor: Vec4, root: Vec3, looked: Vec3, behind: f32) -> Self {
        let own = (root - anchor.xyz()).with_y(0.0).dot(-iso::forward());
        Self {
            height: floor.x,
            behind: floor.y,
            looked,
            own: ON_FLOOR_SQUEEZE * (own - behind),
        }
    }
}

/// The part of its own depth that a model's vertex keeps on a floor's plane (`ON_FLOOR_SQUEEZE`
/// of `models.wesl`).
const ON_FLOOR_SQUEEZE: f32 = 0.002;

/// Where the original draws a particle system in its draw lists (docs/re/drawing.md, "The models",
/// #2047): its FX object's whole system at the object's place, after the
/// statics of its cell and before the creature it follows, or one in its cell, which is drawn
/// after it and covers it; its billboards write no depth, so the model covers every quad of its
/// system that it lies over. The game draws such a system's quads at the depth of that place
/// ([`Quads::in_order`]), just behind its bearer's model, whose texels write their depths
/// (`models.wesl`); the systems drawn by the [`iso::Untinted`] camera test the world's depth too.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Orders<'w, 's> {
    anchors: Res<'w, crate::models::Anchors>,
    models: Query<'w, 's, (&'static crate::models::Ordered, &'static GlobalTransform)>,
    eye: Query<'w, 's, &'static GlobalTransform, iso::Eye>,
    heights: Option<Res<'w, crate::terrain::Heights>>,
    floors: Query<'w, 's, &'static crate::statics::Floor>,
}

impl Orders<'_, '_> {
    /// The depth of a system that `bearer` carries, as the original draws it after its bearer's
    /// model, each of its quads at its particle's own depth, so that what lies before the model's
    /// surface covers it and what lies behind it stays hidden ([`Self::over`], laid by
    /// [`Quads::in_order`]; docs/re/drawing.md, "The models", #3458); on an upper storey's floor,
    /// no further back than the floor's plane where each of its vertices shows, as the model's
    /// (`models.wesl`'s `on_floor`, #2220). `None`, its quads at their own depths, where `bearer`
    /// has no model in the order or the world has no camera.
    pub(crate) fn borne_by(&self, bearer: Entity) -> Option<InOrder> {
        self.over(bearer)
    }

    /// The depth of a system that the original draws after the model of `bearer` standing in its
    /// cell, its billboards depth tested against the model's (the level-up's sparks, #2762): at
    /// the model's root and its place, each of the system's vertices keeping its own depth from
    /// the root as the model's texels keep theirs ([`Quads::as_drawn`]), [`OVER`] nearer, so that
    /// a quad before the model's surface covers it and one behind it stays hidden. `None` as
    /// [`Self::borne_by`].
    pub(crate) fn over(&self, bearer: Entity) -> Option<InOrder> {
        // The root's own depth from its anchor whole, as `models.wesl` takes a texel's, where
        // `InOrder::of` takes it along the ground.
        self.borne(bearer, |rise| -OVER - rise)
            .map(|order| InOrder {
                drawn: true,
                ..order
            })
    }

    /// The depth of a system that `bearer` carries, `behind` units of its own depth behind its
    /// root, given how far the root's own depth lies before its own depth along the ground.
    fn borne(&self, bearer: Entity, behind: impl Fn(f32) -> f32) -> Option<InOrder> {
        let (ordered, at) = self.models.get(bearer).ok()?;
        let (anchor, floor) = self.anchors.of(ordered.slot()?)?;
        let root = at.translation();
        let behind = behind((root.y - anchor.y) * -iso::forward().y);
        let looked = self.looked()?;
        let mut order = InOrder::of(anchor, floor.z, root, looked, behind);
        if floor.w > 0.5 {
            order.floor = Some(OnFloor::new(floor, anchor, root, looked, behind));
        }
        Some(order)
    }

    /// The depth of a system whose object stands at `position`, carried by no model: at the
    /// place of its cell, as just behind a creature standing there.
    pub(crate) fn at(&self, position: Vec3) -> Option<InOrder> {
        self.order(crate::models::place_of(position), 0.0, position)
    }

    /// The depth of a system whose object has stood at `position` from the world's start, a
    /// fire of the world's: after the statics of its cell, at the last place of its chain short of
    /// the next cell of its diagonal (33.8 places on, see `models::PLACE`), as the row walk draws a
    /// cell's chain and then its objects (`0x62AE90`); a creature that came into the cell later is
    /// chained before it and drawn first (`0x5FCCB0`, docs/re/drawing.md, "The models"), but for
    /// the texels of its model's own depth before its place.
    pub(crate) fn after_statics(&self, position: Vec3) -> Option<InOrder> {
        let place = crate::models::place_of(position);
        let anchor = Vec4::new(place.x, 0.0, place.z, LAST_PLACE);
        Some(InOrder::of(anchor, 0.0, anchor.xyz(), self.looked()?, 0.0))
    }

    /// The depth of an FX object of list 0 on the ground at `position`, a type of the flag `0x4`
    /// (the marks, the blood's splashes and puddles, the summonings' pentagrams; #3285): in list
    /// 0, [`crate::statics::LIST_0_BACK`] behind its cell's place, after its cell's statics there,
    /// as the walk appends it after the cell's other objects (`0x62AE90`, docs/re/drawing.md, "The
    /// models"); under the statics' shadows of list 1, the underlays of list 2 and every sprite
    /// and model of lists 3 and 4, its quads laid at that one depth ([`Quads::at_order`]).
    pub(crate) fn in_list_0(&self, position: Vec3) -> Option<InOrder> {
        Some(InOrder::in_list_0(position, self.looked()?))
    }

    /// [`Self::after_statics`] for a system of an object `layered` in a building: on an upper
    /// storey's floor, no further back than the floor's plane where each of its vertices shows,
    /// in its order among the quads standing on the floor, as a model standing there
    /// (`models.wesl`'s `on_floor`, #2220); on the ground and a building's ground floor, as on
    /// the ground.
    pub(crate) fn after_statics_in(
        &self,
        position: Vec3,
        layered: Option<&crate::buildings::Layered>,
    ) -> Option<InOrder> {
        let order = self.after_statics(position)?;
        let part = self
            .heights
            .as_deref()
            .and_then(|heights| crate::models::storey(heights, layered, None));
        let place = crate::models::place_of(position);
        self.on_storey(order, part, (place, place.xyz()), 0.0)
    }

    /// [`Self::at`] for a system of an object on `level`: on an upper storey's floor, no further
    /// back than the floor's plane where each of its vertices shows, as a creature standing
    /// there (#2220).
    pub(crate) fn at_level(&self, position: Vec3, level: crate::terrain::Level) -> Option<InOrder> {
        let order = self.at(position)?;
        let part = self.heights.as_deref().and_then(|heights| {
            heights
                .floor(level)
                .and_then(|floor| crate::models::upper(heights, floor))
        });
        let place = crate::models::place_of(position);
        self.on_storey(order, part, (place, position), BODY)
    }

    /// `order` on the upper storey's floor `part`, if any, for a system `behind` units of its own
    /// depth behind a model of the anchor and root `at`.
    fn on_storey(
        &self,
        mut order: InOrder,
        part: Option<(crate::buildings::Anchor, u8)>,
        (anchor, root): (Vec4, Vec3),
        behind: f32,
    ) -> Option<InOrder> {
        if let Some(floor) = part.and_then(|part| self.floors.iter().find(|f| f.part == part)) {
            let on = floor.under(anchor);
            order.floor = Some(OnFloor::new(on, anchor, root, self.looked()?, behind));
        }
        Some(order)
    }

    /// The point the world's camera looks at, if there is one.
    fn looked(&self) -> Option<Vec3> {
        let eye = self.eye.single().ok()?;
        Some(eye.translation() + iso::forward() * iso::DISTANCE)
    }

    /// [`InOrder::of`], the camera looking at the point that the world's camera does, just behind
    /// the body of a creature rooted at `root`.
    fn order(&self, anchor: Vec4, sunk: f32, root: Vec3) -> Option<InOrder> {
        Some(InOrder::of(anchor, sunk, root, self.looked()?, BODY))
    }
}

/// The last place of a cell's chain short of the next cell of its diagonal, which lies 33.8
/// places on (see `models::PLACE`).
const LAST_PLACE: f32 = 33.0;

// In front of the statics' places of its cell's chain (to 32) and of a model's own place, behind
// the next cell of its diagonal, a cell's diagonal along the screen's right.
const _: () = assert!(32.0 < LAST_PLACE && crate::models::PLACE < LAST_PLACE);
const _: () = assert!(CHAIN * LAST_PLACE < ACROSS * iso::CELL * std::f32::consts::SQRT_2);

impl InOrder {
    /// The order of an FX object of list 0 on the ground at `position`, the camera looking at
    /// `looked` ([`Orders::in_list_0`]).
    fn in_list_0(position: Vec3, looked: Vec3) -> Self {
        let place = crate::models::place_of(position);
        let anchor = Vec4::new(place.x, 0.0, place.z, LAST_PLACE);
        Self::of(
            anchor,
            crate::statics::LIST_0_BACK,
            anchor.xyz(),
            looked,
            0.0,
        )
    }

    /// How far `vertex`, from a mesh's origin `origin`, lies behind the order's depth along the
    /// line of sight toward the camera: below 0 where it lies nearer the camera. On a floor, the
    /// nearer of its place's depth and the floor's where the vertex shows, as `models.wesl`
    /// takes the nearer of the two for a model's vertex: the ground point under the plane's
    /// point on its line of sight, as far behind as what stands on the floor lies, ordered as
    /// `order.wesl` orders it.
    fn shift(&self, vertex: Vec3, origin: Vec3) -> f32 {
        let placed = self.placed(vertex, origin);
        self.on_floor(vertex, origin)
            .map_or(placed, |on| placed.max(on))
    }

    /// How far `vertex`, from a mesh's origin `origin`, lies behind its place's depth along the
    /// line of sight toward the camera ([`Self::shift`]).
    fn placed(&self, vertex: Vec3, origin: Vec3) -> f32 {
        (self.place - origin - vertex).dot(-iso::forward()) + self.beyond
    }

    /// How far `vertex`, from a mesh's origin `origin`, lies behind its floor's depth where it
    /// shows ([`Self::shift`]), if the order stands on a floor.
    fn on_floor(&self, vertex: Vec3, origin: Vec3) -> Option<f32> {
        let back = -iso::forward();
        let floor = self.floor?;
        let world = origin + vertex;
        let plane = world + back * ((floor.height - world.y) / back.y);
        let under = Vec3::new(plane.x, 0.0, plane.z) - back * floor.behind;
        Some(
            (under - world).dot(back)
                + ACROSS * (under - floor.looked).dot(iso::right())
                + floor.own,
        )
    }

    /// `vertex`, from a mesh's origin `origin`, laid along the line of sight where a texel of the
    /// model of the order ([`Orders::over`]) at its place lies ([`Quads::as_drawn`]): at the
    /// order's depth plus its own depth from the model's root, squeezed as `models.wesl` squeezes
    /// a model's, on a floor no further back than the floor's plane brought forward by its own
    /// depth squeezed as there. The own depth is taken from the root whatever the mesh's origin,
    /// the root or the world's start (#3842): from the world's start it was the vertex's whole
    /// depth in the world, which brought the quads some thousands of units nearer.
    pub(crate) fn drawn(&self, vertex: Vec3, origin: Vec3) -> Vec3 {
        let own = (origin + vertex - self.root).dot(-iso::forward());
        let placed = self.placed(vertex, origin) + SQUEEZE * own;
        let shift = self
            .on_floor(vertex, origin)
            .map_or(placed, |on| placed.max(on + ON_FLOOR_SQUEEZE * own));
        vertex - iso::forward() * shift
    }

    /// `vertex`, from a mesh's origin `origin`, laid back along the line of sight to the order's
    /// depth where it lies nearer the camera ([`Quads::in_order`]).
    pub(crate) fn no_nearer(&self, vertex: Vec3, origin: Vec3) -> Vec3 {
        vertex - iso::forward() * self.shift(vertex, origin).min(0.0)
    }

    /// `vertex`, from a mesh's origin `origin`, laid back along the line of sight to `by` units
    /// behind the order's depth where it lies nearer than that ([`Self::no_nearer`] with a
    /// margin): what the original draws before a quad of its own system at the order's depth,
    /// that quad's depth not tested, so that it never covers it (a stargate's window under its
    /// whirls, #2539).
    pub(crate) fn behind(&self, vertex: Vec3, origin: Vec3, by: f32) -> Vec3 {
        vertex - iso::forward() * (self.shift(vertex, origin) - by).min(0.0)
    }

    /// `vertex`, from a mesh's origin `origin`, laid along the line of sight at the order's depth
    /// ([`Quads::at_order`]).
    pub(crate) fn on(&self, vertex: Vec3, origin: Vec3) -> Vec3 {
        vertex - iso::forward() * self.shift(vertex, origin)
    }

    /// The depth `behind` units of a model's own depth behind a model rooted at `root` of the
    /// anchor `anchor` (its cell's centre and its place in the cell), its place `sunk` units
    /// further back (under a liquid's surface, `models::Sunk`), the camera looking at `looked`, as
    /// `models.wesl` lays the model's texels: its place's depth and the root's own depth from the
    /// anchor, less `behind`, squeezed.
    fn of(anchor: Vec4, sunk: f32, root: Vec3, looked: Vec3, behind: f32) -> Self {
        let back = -iso::forward();
        let place = Vec3::new(anchor.x, 0.0, anchor.z) - back * sunk;
        let own = (root - anchor.xyz()).with_y(0.0).dot(back);
        Self {
            place,
            beyond: ACROSS * (place - looked).dot(iso::right())
                + CHAIN * anchor.w
                + SQUEEZE * (own - behind),
            floor: None,
            drawn: false,
            root,
        }
    }
}

/// The variants and the materials of their layers, one a texture and a blend.
fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<Effect>>,
) {
    let mut made: Vec<(MaterialKey, Material)> = Vec::new();
    let mut mixed: Vec<((&'static str, bool, bool), Material)> = Vec::new();
    // A fire's material of its layers drawn as one ([`mixes`]), of list 4 or not.
    let mut mixed_of = |fire: &Fire, over: bool, materials: &mut Assets<Effect>| -> Material {
        let key = (fire.texture, over, fire.tinted);
        if let Some((_, material)) = mixed.iter().find(|(k, _)| *k == key) {
            return material.clone();
        }
        let image = data
            .as_ref()
            .and_then(|data| particle_texture(&data.0.assets, &assets, fire.texture));
        let material = image.map(|image| {
            let material = mixed_material(image);
            materials.add(match (over, fire.tinted) {
                (true, _) => over_all(material),
                (false, true) => among_objects(material),
                (false, false) => material,
            })
        });
        mixed.push((key, material.clone()));
        material
    };
    let mut variants = Vec::new();
    for variant in VARIANTS {
        let Some(fire) = fire(variant) else {
            continue;
        };
        if mixes(&fire) {
            let layers = [false, true].map(|over| vec![mixed_of(&fire, over, &mut materials)]);
            variants.push((variant, Arc::new(fire), layers));
            continue;
        }
        let [layers, over] = [false, true].map(|over| {
            fire.blends
                .iter()
                .map(|&blend| {
                    let key = (fire.texture, blend, over, fire.tinted);
                    if let Some((_, material)) = made.iter().find(|(k, _)| *k == key) {
                        return material.clone();
                    }
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, fire.texture));
                    if image.is_none() {
                        warn!("fx: no texture {}; its effects are not drawn", fire.texture);
                    }
                    let material = image.map(|image| {
                        let material = match blend {
                            Blend::Add => particle_material(image, AlphaMode::Add),
                            Blend::Whole => whole_material(image),
                            Blend::Over => particle_material(image, AlphaMode::Blend),
                        };
                        // A tinted one among the objects, as the world's camera draws it
                        // (#3459).
                        materials.add(match (over, fire.tinted) {
                            (true, _) => over_all(material),
                            (false, true) => among_objects(material),
                            (false, false) => material,
                        })
                    });
                    made.push((key, material.clone()));
                    material
                })
                .collect::<Vec<_>>()
        });
        variants.push((variant, Arc::new(fire), [layers, over]));
    }
    // The steps' small fire takes the fires' materials, and its burnt ground one added.
    let step = small_fire();
    let layers = if mixes(&step) {
        vec![mixed_of(&step, false, &mut materials)]
    } else {
        step.blends
            .iter()
            .map(|&blend| {
                made.iter()
                    .find(|(key, _)| *key == (step.texture, blend, false, step.tinted))
                    .or_else(|| {
                        made.iter().find(|((texture, of, over, _), _)| {
                            (*texture, *of, *over) == (step.texture, blend, false)
                        })
                    })
                    .and_then(|(_, material)| material.clone())
            })
            .collect()
    };
    let ground = data
        .as_ref()
        .and_then(|data| particle_texture(&data.0.assets, &assets, GROUND_TEXTURE))
        .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
    commands.insert_resource(StepFires {
        fire: Arc::new(step),
        breath: Arc::new(breath_small_fire()),
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
/// #1808), at its place on its level, untinted by the night as the fires; and one where a Big
/// Demon's step or a creature's cast asks for one ([`SmallFire`], #2898, #3028).
fn kindle(
    mut commands: Commands,
    mut steps: MessageReader<crate::sound::HeroStep>,
    mut cast: MessageReader<SmallFire>,
    step_fires: Option<Res<StepFires>>,
    heroes: Query<(
        Entity,
        &crate::hero::Body,
        Option<&crate::fight::Holding>,
        Option<&crate::buildings::Layered>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(step_fires) = step_fires else {
        steps.clear();
        cast.clear();
        return;
    };
    for &SmallFire {
        at,
        level,
        caster,
        breath,
    } in cast.read()
    {
        let fire = if breath {
            &step_fires.breath
        } else {
            &step_fires.fire
        };
        light_small_fire(
            &mut commands,
            (&step_fires, fire, &mut meshes),
            (at, level, None),
            caster,
        );
    }
    let Some((hero, body, holding, layered)) = heroes.iter().next() else {
        steps.clear();
        return;
    };
    let fiery = holding.is_some_and(|holding| holding.0.sets.fiery_steps());
    for &crate::sound::HeroStep(at) in steps.read() {
        if !fiery {
            continue;
        }
        light_small_fire(
            &mut commands,
            (&step_fires, &step_fires.fire, &mut meshes),
            (at, body.level(), layered.copied()),
            Some(hero),
        );
    }
}

/// A small fire at `at` on `level`, in the layer of a building's of `layered`, whose strikes are
/// those of `caster`, none for none.
fn light_small_fire(
    commands: &mut Commands,
    (step_fires, variant, meshes): (&StepFires, &Arc<Fire>, &mut Assets<Mesh>),
    (at, level, layered): (
        Vec3,
        crate::terrain::Level,
        Option<crate::buildings::Layered>,
    ),
    caster: Option<Entity>,
) {
    {
        let mut handles = Vec::new();
        let mut ground = None;
        let mut fire = commands.spawn((
            Transform::from_translation(at),
            Visibility::default(),
            daynight::Light(variant.light),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(layered) = layered {
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
            fire: variant.clone(),
            system: System::new(variant.count),
            dice: crate::seed::dice(SEED ^ index),
            meshes: handles,
            ground,
            level,
            caster,
            cooldown: 0.0,
        });
    }
}

/// The quads of a fire's layer `layer` of its system `system`, facing the camera.
pub(crate) fn layer_quads(fire: &Fire, system: &System, layer: usize) -> Quads {
    let mut quads = Quads::default();
    for particle in system
        .particles
        .iter()
        .filter(|p| usize::from(p.layer) == layer)
    {
        push_particle(&mut quads, fire, particle, 1.0);
    }
    quads
}

/// Whether a fire's layers are drawn as one mesh in their particles' slots' order
/// ([`mixed_material`]): two or more, some added by their alpha and some laid over, none added
/// whole.
pub(crate) fn mixes(fire: &Fire) -> bool {
    fire.layers.len() > 1
        && fire.blends.contains(&Blend::Add)
        && fire.blends.contains(&Blend::Over)
        && !fire.blends.contains(&Blend::Whole)
}

/// The quads of every layer of a fire's system `system` in its particles' slots' order, facing the
/// camera, an added layer's alpha negative ([`mixed_material`]).
pub(crate) fn mixed_quads(fire: &Fire, system: &System) -> Quads {
    let mut quads = Quads::default();
    for particle in &system.particles {
        let added = fire.blends.get(usize::from(particle.layer)) == Some(&Blend::Add);
        push_particle(&mut quads, fire, particle, if added { -1.0 } else { 1.0 });
    }
    quads
}

/// The quads of a fire's system for its `meshes` meshes: one of its layers in their particles'
/// slots' order where it [`mixes`] them, else one a layer.
pub(crate) fn drawn_quads(fire: &Fire, system: &System, meshes: usize) -> Vec<Quads> {
    if mixes(fire) && meshes == 1 {
        return vec![mixed_quads(fire, system)];
    }
    (0..meshes)
        .map(|layer| layer_quads(fire, system, layer))
        .collect()
}

/// A particle's quad of its layer's ramp at its life, its texture's frame of the grid, its alpha
/// times `sign`.
fn push_particle(quads: &mut Quads, fire: &Fire, particle: &Particle, sign: f32) {
    let layer = usize::from(particle.layer);
    let ramp = &fire.ramps[layer.min(fire.ramps.len() - 1)];
    let grid = f32::from(fire.grid.max(1));
    // Its frame's square of the texture's grid.
    let side = fire.grid.max(1);
    let frame = Vec2::new(
        f32::from(particle.frame % side),
        f32::from(particle.frame / side % side),
    );
    let [r, g, b, a] = ramp.at(particle.life);
    quads.push(
        world(particle.place),
        particle.size * iso::MODEL_SCALE,
        particle.angle,
        (frame / grid, 1.0 / grid),
        [r, g, b, a * sign],
    );
}

/// Runs each step's small fire a frame on (`0x79F6D0`): its particles, emitted while its clock is
/// below [`SMALL_FIRE_EMITS`], and its burnt ground's frame (`0x79F7D0`, the quad of a half side
/// of [`GROUND_HALF`] turned by a half turn); it goes once it emits no more and its last particle
/// and its ground are gone.
fn smoulder(
    mut commands: Commands,
    time: Res<Time>,
    mut fires: Query<(Entity, &mut StepFire)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut step) in &mut fires {
        let StepFire {
            fire,
            system,
            dice,
            meshes: layers,
            ground,
            ..
        } = &mut *step;
        let fire: &Fire = fire;
        system.update(dt, &fire.layers);
        if system.clock < SMALL_FIRE_EMITS {
            system.emit(&fire.blocks, &fire.layers, fire.count, dice);
        }
        let frame = ground_frame(system.clock);
        if system.clock >= SMALL_FIRE_EMITS && system.particles.is_empty() && frame.is_none() {
            commands.entity(entity).despawn();
            continue;
        }
        for (quads, handle) in drawn_quads(fire, system, layers.len())
            .into_iter()
            .zip(layers)
        {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
        if let Some(mut mesh) = ground.as_ref().and_then(|handle| meshes.get_mut(handle)) {
            let mut quads = Quads::default();
            if let Some(frame) = frame {
                let half = GROUND_HALF;
                let grid = f32::from(GROUND_GRID);
                let corner = Vec2::new(
                    f32::from(frame % GROUND_GRID),
                    f32::from(frame / GROUND_GRID % GROUND_GRID),
                ) / grid;
                let side = 1.0 / grid;
                // Turned by a half turn: the texture's far corner at the near one.
                quads.push_corners(
                    [
                        world([-half, -half, 0.0]),
                        world([half, -half, 0.0]),
                        world([half, half, 0.0]),
                        world([-half, half, 0.0]),
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

/// Strikes from each fire smoke, and from a geyser while it bursts, at most once a second, every
/// living creature on its level within its size and 50 of its point, the hero, the peaceful and
/// the hostile alike, as nobody's (`0x5A6680`, `0x5AB490`, `0x5FF7C0`; `0x5ABD80`'s cases `0x32D`,
/// `0x36C` and `0x36F`; #2518): the fire's physical and fire damage, sure, through their
/// resistances (`fight::Burn`, `opensacred_rules::fight::scald`). Its cooldown runs down at every
/// frame and starts again only at a strike, so that one who steps into it is struck at once.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn scald(
    time: Res<Time>,
    mut burns: MessageWriter<crate::fight::Burn>,
    mut fires: Query<(&mut Burning, &GlobalTransform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &crate::crowd::InTheWay,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<crate::hero::Body>, Without<crate::npcs::Dies>),
    >,
    heroes: Query<
        (
            Entity,
            &Transform,
            &crate::hero::Body,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        Without<crate::npcs::Dies>,
    >,
) {
    let dt = time.delta_secs();
    for (mut fire, at) in &mut fires {
        fire.cooldown = (fire.cooldown - dt).max(0.0);
        let bursting = fire
            .geyser
            .as_ref()
            .is_none_or(|(geyser, _)| geyser.bursting());
        let Some(damages) = opensacred_rules::fight::scald(fire.variant).filter(|_| bursting)
        else {
            continue;
        };
        if fire.cooldown > 0.0 {
            continue;
        }
        let here = at.translation().xz();
        // Its own size, 0 in its record, is 50 (`0x428CE0`).
        let own = opensacred_rules::arrows::size(0);
        let within = |place: &Transform, size: Option<&crate::arrows::Size>| {
            let reach = own + size.map_or(own, |size| size.0);
            (place.translation.xz() - here).length() < reach
        };
        let creatures = creatures
            .iter()
            .filter(|(_, place, way, health, size)| {
                way.creature
                    && way.level == fire.level
                    && health.current > 0
                    && within(place, *size)
            })
            .map(|(target, ..)| target);
        let heroes = heroes
            .iter()
            .filter(|(_, place, body, health, size)| {
                body.level() == fire.level && health.current > 0 && within(place, *size)
            })
            .map(|(target, ..)| target);
        let mut struck = false;
        for target in creatures.chain(heroes) {
            burns.write(crate::fight::Burn { target, damages });
            struck = true;
        }
        if struck {
            fire.cooldown = opensacred_rules::fight::SCALD_PERIOD;
        }
    }
}

/// Strikes from each small fire with a caster, at most once a second, every living creature
/// hostile to its caster on its level within [`opensacred_rules::fight::SMALL_FIRE_REACH`] and its
/// size of it (`0x5AB490`, `0x5AE436`): the caster's strike of the fire's damages of its strength
/// and dexterity, sure, through the resistances (`0x5AE44D`). Of a hero's, the creatures hostile
/// to the hero, never the hero; of a creature's, the living hostile to it (`0x548F60`): the hero
/// where it is hostile to the hero, and the creatures hostile to it by their sides, its fellows
/// left out (#3028).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn scorch(
    time: Res<Time>,
    mut hits: MessageWriter<crate::fight::Hit>,
    mut fires: Query<(&mut StepFire, &Transform)>,
    casters: Query<(&crate::fight::Combat, Option<&crate::ai::Mind>)>,
    heroes: Query<
        (
            Entity,
            &Transform,
            &crate::hero::Body,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        Without<crate::npcs::Dies>,
    >,
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
    let dt = time.delta_secs();
    for (mut step, at) in &mut fires {
        step.cooldown = (step.cooldown - dt).max(0.0);
        let Some(caster) = step.caster else {
            continue;
        };
        if step.cooldown > 0.0 {
            continue;
        }
        let Ok((combat, caster_mind)) = casters.get(caster) else {
            continue;
        };
        let combat = &combat.0;
        let damages = opensacred_rules::fight::small_fire_damages(
            combat.attributes[0],
            combat.attributes[2],
            [combat.multipliers[0], combat.multipliers[1]],
        );
        let here = at.translation.xz();
        let within = |place: &Transform, size: Option<&crate::arrows::Size>| {
            let reach = opensacred_rules::fight::SMALL_FIRE_REACH
                + size.map_or(opensacred_rules::arrows::size(0), |size| size.0);
            (place.translation.xz() - here).length() < reach
        };
        let hostile = |mind: &crate::ai::Mind| match caster_mind {
            Some(caster) => caster.hostile_to(mind),
            None => mind.hostile(),
        };
        let creatures = creatures
            .iter()
            .filter(|&(target, place, way, mind, health, size)| {
                target != caster
                    && way.creature
                    && way.level == step.level
                    && health.current > 0
                    && hostile(mind)
                    && within(place, size)
            })
            .map(|(target, ..)| target);
        // A creature's fire takes the hero where the creature is hostile to the hero.
        let heroes = heroes
            .iter()
            .filter(|&(target, place, body, health, size)| {
                caster_mind.is_some_and(|caster| caster.hostile())
                    && target != caster
                    && body.level() == step.level
                    && health.current > 0
                    && within(place, size)
            })
            .map(|(target, ..)| target);
        let mut struck = false;
        for target in creatures.chain(heroes) {
            hits.write(crate::fight::Hit {
                attacker: caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(damages),
                sure: true,
                poison: false,
                ward: false,
                hurled: false,
                object: true,
                mount: false,
                marks: false,
            });
            struck = true;
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
    unlit: Query<(Entity, &Unlit, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, &Unlit(variant, level, over), at) in &unlit {
        commands.entity(entity).remove::<Unlit>();
        let (x, z) = (at.translation.x, at.translation.z);
        debug!(
            "fx: a fire of the variant {variant} at {:?}",
            crate::paths::cell_of(x, z)
        );
        let Some((_, fire, layers)) = fires.variants.iter().find(|(v, ..)| *v == variant) else {
            continue;
        };
        let materials = &layers[usize::from(over)];
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
            level,
            cooldown: 0.0,
            over,
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

/// Runs each fire's particles a frame on, carried by the renderer's wind (`0x76E2D0`), and lays
/// its layers' quads out facing the camera; a geyser runs its clock first, its burst's sound at
/// it, one of two at a rate of 0.9 to 1.1 of the sample's.
fn burn(
    (time, clock, preroll): (Res<Time>, Option<Res<crate::daynight::Clock>>, Res<Preroll>),
    mut fires: Query<(
        Entity,
        &mut Burning,
        &GlobalTransform,
        Option<&crate::buildings::Layered>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut latch, mut sounds): (ResMut<GeyserLatch>, MessageWriter<crate::sfx::Play>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let wind = clock
        .as_deref()
        .map_or([0.0; 3], crate::daynight::Clock::wind);
    for (entity, mut burning, at, layered) in &mut fires {
        // At its object's place in the order, after its cell's statics and under what a row
        // nearer draws (#2188); on an upper storey's floor, no further back than its plane
        // (#2220); one of list 4 over every sprite (#3286).
        let place = at.translation();
        let order = orders.after_statics_in(place, layered);
        let over = burning.over;
        let Burning {
            fire,
            system,
            dice,
            meshes: layers,
            geyser,
            ..
        } = &mut *burning;
        let fire: &Arc<Fire> = fire;
        // A load's first frame runs it on first (#3805), a geyser's sounds at the frame's own
        // step alone.
        for (dt, own_step) in preroll.steps(dt) {
            let fire: &Fire = match geyser.as_mut() {
                Some((geyser, own)) => {
                    if let Jet::Bursts { sounds: true } = geyser.update(dt, own, &mut latch.0, dice)
                        && own_step
                    {
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
            let blown: Vec<_> = fire.layers.iter().map(|layer| layer.blown(wind)).collect();
            system.update(dt, &blown);
            system.emit(&fire.blocks, &fire.layers, fire.count, dice);
        }
        let fire: &Fire = geyser.as_ref().map_or(fire.as_ref(), |(_, own)| own);
        for (mut quads, handle) in drawn_quads(fire, system, layers.len())
            .into_iter()
            .zip(layers)
        {
            let Some(mut mesh) = meshes.get_mut(handle) else {
                continue;
            };
            if over {
                quads.ahead(crate::statics::LIST_4_FORWARD);
            } else {
                quads.in_order(order, place);
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

/// The loop's place's factor of a fire of the variant `variant` (`0x690587`, `playSFX`'s seventh
/// argument, `sfx::placed_by`, #3331), by its type: 0.3 for `FX_FI_SMOKE_S`
/// and the smallest `FX_FIRE_L`, 0.4 for the middle ones and the magic fire 904 (the variant 13,
/// #2830), 0.6 for the largest; none for a smoke.
fn crackle_volume(variant: u8) -> Option<f32> {
    match variant {
        4 | 10 => Some(0.3),
        5 | 11 | 13 => Some(0.4),
        6 | 12 => Some(0.6),
        _ => None,
    }
}

/// The loop's place's factor of a fire, a world's of its variant ([`crackle_volume`]) or a step's
/// small fire's ([`STEP_FIRE_VOLUME`]): as loud as any sample at the fire, silent sooner.
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
/// more starts at the nearest fire within 300 pixels, looping at the sample's rate, without
/// asking whether that fire has one already, so that four play at one fire as the original's do;
/// each plays at its fire's place by its factor ([`volume_of`]), and stops when its fire goes or is
/// further than 300.
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
    settings: Option<Res<crate::settings::Settings>>,
) {
    use bevy_kira_audio::prelude::{AudioControl, AudioTween, PlaybackState};
    let Some(hero) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    // None at the lowest sound quality (`0x690451`).
    let quiet = crate::settings::low_sound(settings.as_deref());
    let near = |at: &GlobalTransform| {
        !quiet && crate::sfx::screen_distance(at.translation(), hero) <= NEAR
    };
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
        let factor = volume_of(burning).unwrap_or(0.0);
        let (volume, pan) = crate::sfx::placed_at_by(at.translation(), Some(hero), *heard, factor);
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
            .init_resource::<Preroll>()
            .add_systems(
                Last,
                end_preroll.run_if(in_state(crate::pause::Play::Running)),
            )
            .add_message::<SmallFire>()
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
                    scald,
                    crackle,
                )
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statics::tests::{ORDER, shader_constant};

    #[test]
    fn a_loads_first_frame_runs_the_fixed_systems_on_for_six_seconds_first() {
        let own: Vec<(f32, bool)> = Preroll(false).steps(0.02).collect();
        assert_eq!(own, [(0.02, true)]);
        let steps: Vec<(f32, bool)> = Preroll(true).steps(0.02).collect();
        assert_eq!(steps.last(), Some(&(0.02, true)));
        let before: Vec<f32> = steps[..steps.len() - 1]
            .iter()
            .map(|&(dt, own)| {
                assert!(!own);
                dt
            })
            .collect();
        assert!((before.iter().sum::<f32>() - 6.0).abs() < 1e-3);
        let mut world = World::new();
        world.insert_resource(Preroll(true));
        bevy::ecs::system::RunSystemOnce::run_system_once(&mut world, end_preroll).unwrap();
        assert!(!world.resource::<Preroll>().0);
    }

    #[test]
    fn a_fires_flames_and_smoke_go_into_one_mesh_in_their_slots_order() {
        let flame = |layer: u8, life: f32| Particle {
            place: [0.0; 3],
            velocity: [0.0; 3],
            buoyancy: 0.0,
            size: 4.0,
            life,
            angle: 0.0,
            spin: 0.0,
            layer,
            frame: 0,
            colour: [1.0; 3],
        };
        let Some(fire) = fire(10) else {
            panic!("the fire 10 is made");
        };
        // The fires' flames added, their smoke laid over: one mesh.
        assert!(mixes(&fire));
        assert!(!mixes(&Fire {
            blends: vec![Blend::Over],
            layers: fire.layers[..1].to_vec(),
            ..fire.clone()
        }));
        assert!(!mixes(&Fire {
            blends: vec![Blend::Whole, Blend::Over],
            ..fire.clone()
        }));
        let mut system = System::new(3);
        system.particles = vec![flame(1, 200.0), flame(0, 200.0), flame(1, 100.0)];
        let quads = mixed_quads(&fire, &system);
        // Four corners a particle, in the slots' order: the smoke laid by its alpha, the flame's
        // alpha negative as an added one's.
        let alphas: Vec<f32> = quads.colours.iter().step_by(4).map(|c| c[3]).collect();
        assert_eq!(alphas.len(), 3);
        assert!(
            alphas[0] > 0.0 && alphas[1] < 0.0 && alphas[2] > 0.0,
            "{alphas:?}"
        );
        assert_eq!(alphas[1], -fire.ramps[0].at(200.0)[3]);
        assert_eq!(drawn_quads(&fire, &system, 1).len(), 1);
        // Two meshes take a layer each.
        let apart = drawn_quads(&fire, &system, 2);
        assert_eq!(
            apart.iter().map(|q| q.colours.len()).collect::<Vec<_>>(),
            [4, 8]
        );
    }

    /// The targets that a small fire of the caster `caster` of the world `world` strikes at its
    /// first second.
    fn struck_by(world: &mut World, caster: Option<Entity>) -> Vec<Entity> {
        world.spawn((
            StepFire {
                fire: Arc::new(small_fire()),
                system: System::new(1),
                dice: Rng::of(1, 2, 3),
                meshes: Vec::new(),
                ground: None,
                level: None,
                caster,
                cooldown: 0.0,
            },
            Transform::default(),
        ));
        world.run_system_cached(scorch).unwrap();
        let mut hits = world.resource_mut::<Messages<crate::fight::Hit>>();
        let mut struck: Vec<Entity> = hits.drain().map(|hit| hit.target).collect();
        struck.sort();
        struck
    }

    #[test]
    fn a_creatures_small_fire_strikes_those_hostile_to_it() {
        let combat = crate::fight::Combat(opensacred_rules::fight::Combat {
            attack: 100.0,
            defence: 10.0,
            damage: [[10.0, 20.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [100.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let health = crate::fight::Health {
            current: 50,
            most: 50,
            level: 1,
        };
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Messages<crate::fight::Hit>>();
        let creature = |world: &mut World, kind: u32, x: f32| {
            let mind = crate::ai::Mind::new(
                Vec3::ZERO,
                opensacred_rules::ways::HOSTILE,
                (false, true),
                None,
                [70.0, 180.0],
                (kind, 1),
            );
            world
                .spawn((
                    Transform::from_xyz(x, 0.0, 0.0),
                    health,
                    mind,
                    combat,
                    crate::crowd::InTheWay::of(0, 0, None),
                ))
                .id()
        };
        // A Big Demon, a goblin beside it and the hero, all within the fire's reach.
        let demon = creature(&mut world, 53, 0.0);
        let goblin = creature(&mut world, 76, 10.0);
        let hero = world
            .spawn((
                Transform::from_xyz(-10.0, 0.0, 0.0),
                crate::hero::Body::standing(),
                health,
                combat,
            ))
            .id();
        // The demon's strikes the hero alone, its fellow and itself spared; the hero's the goblin
        // and the demon, never the hero; a fire of no caster strikes nothing.
        assert_eq!(struck_by(&mut world, Some(demon)), vec![hero]);
        let mut foes = vec![demon, goblin];
        foes.sort();
        assert_eq!(struck_by(&mut world, Some(hero)), foes);
        assert!(struck_by(&mut world, None).is_empty());
    }

    #[test]
    fn a_demons_small_fire_burns_where_it_is_asked_and_strikes_nothing() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .add_message::<crate::sound::HeroStep>()
            .add_message::<SmallFire>()
            .insert_resource(StepFires {
                fire: Arc::new(small_fire()),
                breath: Arc::new(breath_small_fire()),
                layers: Vec::new(),
                ground: None,
            })
            .add_systems(Update, kindle);
        let at = Vec3::new(100.0, 0.0, 200.0);
        app.world_mut().write_message(SmallFire {
            at,
            level: None,
            caster: None,
            breath: false,
        });
        app.update();
        let mut fires = app.world_mut().query::<(&StepFire, &Transform)>();
        let (fire, place) = fires.single(app.world()).unwrap();
        assert_eq!(place.translation, at);
        assert!(fire.caster.is_none() && fire.level.is_none());
        assert_eq!(fire.fire.light, small_fire().light);
        // No hero, no steps' fire.
        app.world_mut()
            .write_message(crate::sound::HeroStep(Vec3::ZERO));
        app.update();
        assert_eq!(fires.iter(app.world()).count(), 1);
        // A breath's bounce's of the variant 1, its light 100 (#3301).
        app.world_mut().write_message(SmallFire {
            at: Vec3::ZERO,
            level: None,
            caster: None,
            breath: true,
        });
        app.update();
        let mut lights = app
            .world_mut()
            .query::<(&StepFire, &daynight::Light, &Transform)>();
        let (bounce, light, _) = lights
            .iter(app.world())
            .find(|(.., place)| place.translation == Vec3::ZERO)
            .unwrap();
        assert_eq!(bounce.fire.light, 100.0);
        assert_eq!(light.0, 100.0);
    }

    #[test]
    fn the_fires_loop_by_their_types_the_magic_fire_too() {
        assert_eq!(crackle_volume(4), Some(0.3));
        assert_eq!(crackle_volume(11), Some(0.4));
        assert_eq!(crackle_volume(13), Some(0.4));
        assert_eq!(crackle_volume(12), Some(0.6));
        assert_eq!(crackle_volume(0), None);
    }

    #[test]
    fn the_order_of_drawing_is_the_shaders() {
        assert_eq!(ACROSS, shader_constant(ORDER, "ACROSS"));
        assert_eq!(CHAIN, shader_constant(ORDER, "CHAIN"));
        assert_eq!(
            SQUEEZE,
            shader_constant(include_str!("models.wesl"), "SQUEEZE")
        );
        assert_eq!(
            ON_FLOOR_SQUEEZE,
            shader_constant(include_str!("models.wesl"), "ON_FLOOR_SQUEEZE")
        );
    }

    #[test]
    fn a_system_on_a_floor_lies_no_further_back_than_its_plane() {
        let back = -iso::forward();
        let looked = iso::ground(10.0, 10.0);
        // A bearer on a floor 120 units high, at its cell's centre; what stands on the floor
        // lies 40 units behind the ground point under the plane's point.
        let anchor = iso::ground(12.5, 12.5).extend(crate::models::PLACE);
        let root = anchor.xyz() + Vec3::Y * 120.0;
        let mut order = InOrder::of(anchor, 0.0, root, looked, BODY);
        let ground = order;
        order.floor = Some(OnFloor::new(
            Vec4::new(120.0, 40.0, 0.0, 1.0),
            anchor,
            root,
            looked,
            BODY,
        ));
        // A flame on the floor 60 units in front of it, where the plane lies nearer than its
        // place, brought far nearer the camera than either depth.
        let flame = Vec3::new(back.x, 0.0, back.z).normalize() * 60.0 + back * 500.0;
        let laid = root + order.no_nearer(flame, root);
        let on_ground = root + ground.no_nearer(flame, root);
        // Along its line of sight, where it showed.
        let moved = laid - (root + flame);
        assert!(moved.cross(back).length() < 1e-2, "{moved}");
        // At the floor's depth where it shows, the shader's `on_floor`: nearer than its
        // place's on the ground, which the floor's plane would hide.
        let plane = laid + back * ((120.0 - laid.y) / back.y);
        let under = Vec3::new(plane.x, 0.0, plane.z) - back * 40.0;
        let own = (root - anchor.xyz()).with_y(0.0).dot(back);
        let expected = (under - looked).dot(back)
            + ACROSS * (under - looked).dot(iso::right())
            + ON_FLOOR_SQUEEZE * (own - BODY);
        assert!(((laid - looked).dot(back) - expected).abs() < 1e-2);
        assert!((laid - looked).dot(back) > (on_ground - looked).dot(back));
    }

    #[test]
    fn a_system_in_order_shows_where_it_did_just_behind_its_bearers_model() {
        let back = -iso::forward();
        // `order.wesl`'s order beyond the point looked at, in world units toward the camera: of a
        // place `anchor` and a part `own` of its own depth, as a model's vertex takes it, or of a
        // point of the world at its true depth.
        let looked = iso::ground(10.0, 10.0);
        let in_order = |anchor: Vec4, own: f32| {
            let place = Vec3::new(anchor.x, 0.0, anchor.z);
            (place - looked).dot(back)
                + ACROSS * (place - looked).dot(iso::right())
                + CHAIN * anchor.w
                + SQUEEZE * own
        };
        let true_depth = |point: Vec3| (point - looked).dot(back);
        // A bearer standing off its cell's centre, a little toward the camera, its anchor its
        // cell's centre at the cell's height 12; near the world's origin, where a point's
        // rounding is a thousandth of a unit and less.
        let root = iso::ground(22.7, 13.6) + Vec3::Y * 12.0;
        let anchor = iso::ground(22.5, 13.5)
            .with_y(12.0)
            .extend(crate::models::PLACE);
        let order = InOrder::of(anchor, 0.0, root, looked, BODY);
        // Flames about it, up to its head, laid out about a mesh's origin at its root.
        let mut quads = Quads::default();
        for (x, y, z) in [(-20.0, 0.0, 5.0), (15.0, 60.0, -10.0), (0.0, 110.0, 25.0)] {
            quads.push(Vec3::new(x, y, z), 8.0, 0.3, (Vec2::ZERO, 1.0), [1.0; 4]);
        }
        let before: Vec<Vec3> = quads.positions.iter().copied().map(Vec3::from).collect();
        quads.in_order(Some(order), root);
        // The model's texels, from 15 units behind its root to its head before it.
        let own = |point: Vec3| (point - anchor.xyz()).dot(back);
        let nearest_behind = in_order(anchor, own(root - back * 15.0));
        for (was, &now) in before.iter().zip(&quads.positions) {
            let (was, now) = (root + *was, root + Vec3::from(now));
            // Where it shows on screen, it shows still.
            assert!((now - was).dot(iso::right()).abs() < 1e-3, "{was} {now}");
            assert!((now - was).dot(iso::up()).abs() < 1e-3, "{was} {now}");
            // At its system's depth: just behind the back of the bearer's body, a few hundredths
            // of a unit, and not a tenth of a unit further.
            let depth = true_depth(now);
            assert!(
                (depth - in_order(anchor, own(root) - BODY)).abs() < 0.02,
                "{depth}"
            );
            assert!(depth < nearest_behind && depth > nearest_behind - 0.1);
        }
        // A model a row of cells further back, drawn before the bearer, lies behind the system;
        // one a row nearer, drawn after it, in front of it.
        let row = iso::CELL * Vec3::X.dot(back);
        let farther = in_order(anchor - Vec4::new(iso::CELL, 0.0, 0.0, 0.0), 40.0);
        let nearer = in_order(anchor + Vec4::new(iso::CELL, 0.0, 0.0, 0.0), -40.0);
        let system = true_depth(root + Vec3::from(quads.positions[0]));
        assert!(row > 1.0 && farther < system && system < nearer);
        // A part of the system far behind its place, a ring's on the ground rows back, keeps its
        // own depth, which a creature drawn before it in front of it hides (#2188).
        let mut ring = Quads::default();
        ring.push(-back * 100.0, 8.0, 0.0, (Vec2::ZERO, 1.0), [1.0; 4]);
        let was = ring.positions.clone();
        ring.in_order(Some(order), root);
        assert_eq!(ring.positions, was);
    }

    #[test]
    fn a_quad_of_list_4_comes_ahead_where_it_shows() {
        use crate::statics::LIST_4_FORWARD;
        let mut quads = Quads::default();
        quads.push(
            Vec3::new(5.0, 80.0, -3.0),
            8.0,
            0.3,
            (Vec2::ZERO, 1.0),
            [1.0; 4],
        );
        let before = quads.positions.clone();
        quads.ahead(LIST_4_FORWARD);
        for (was, now) in before.iter().zip(&quads.positions) {
            let (was, now) = (Vec3::from(*was), Vec3::from(*now));
            // Where it shows on screen, it shows still, that much nearer the camera.
            assert!((now - was).dot(iso::right()).abs() < 1e-3, "{was} {now}");
            assert!((now - was).dot(iso::up()).abs() < 1e-3, "{was} {now}");
            assert!(((now - was).dot(-iso::forward()) - LIST_4_FORWARD).abs() < 1e-2);
        }
    }

    #[test]
    fn a_mark_of_list_0_lies_among_its_sprites_under_the_other_lists() {
        use crate::statics::{LIST_0_BACK, LIST_1_BACK, LIST_2_BACK};
        let back = -iso::forward();
        let looked = iso::ground(10.0, 10.0);
        // `order.wesl`'s order of a sprite of the cell `(cx, cy)`, `behind` units back along the
        // line of sight, at the place `chain` of its cell's chain, as `statics::Spot` lays it.
        let sprite = |cx: f32, cy: f32, behind: f32, chain: f32| {
            let at = iso::ground(cx, cy) + iso::forward() * behind;
            (at - looked).dot(back) + ACROSS * (at - looked).dot(iso::right()) + CHAIN * chain
        };
        let true_depth = |point: Vec3| (point - looked).dot(back);
        // A puddle in the cell (22, 13), off its centre, flat on the ground at height 12.
        let origin = iso::ground(22.3, 13.8) + Vec3::Y * 12.0;
        let (across, depth) = iso::fx_axes();
        let mut quads = Quads::default();
        quads.push_corners(
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(x, y)| (across * x + depth * y) * 30.0),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            [1.0; 4],
        );
        let before = quads.positions.clone();
        quads.at_order(Some(InOrder::in_list_0(origin, looked)), origin);
        for (was, now) in before.iter().zip(&quads.positions) {
            let (was, now) = (origin + Vec3::from(*was), origin + Vec3::from(*now));
            // Where it shows on screen, it shows still.
            assert!((now - was).dot(iso::right()).abs() < 1e-3, "{was} {now}");
            assert!((now - was).dot(iso::up()).abs() < 1e-3, "{was} {now}");
            let depth = true_depth(now);
            // In list 0 after its cell's sprites there, before those of the next row.
            assert!(depth > sprite(22.5, 13.5, LIST_0_BACK, 32.0), "{depth}");
            assert!(depth < sprite(23.5, 13.5, LIST_0_BACK, 0.0), "{depth}");
            // Under its cell's shadows of list 1 and underlays of list 2, and under the sprites
            // of list 3 of its cell and of rows behind it.
            assert!(depth < sprite(22.5, 13.5, LIST_1_BACK, 0.0));
            assert!(depth < sprite(22.5, 13.5, LIST_2_BACK, 0.0));
            assert!(depth < sprite(22.5, 13.5, 0.0, 0.0));
            assert!(depth < sprite(20.5, 12.5, 0.0, 0.0));
        }
    }

    #[test]
    fn a_system_drawn_over_its_bearer_lies_just_before_its_models_texels() {
        let back = -iso::forward();
        let looked = iso::ground(10.0, 10.0);
        // `order.wesl`'s order of a model's texel of the place `anchor` and its own depth `own`,
        // and a point's true depth, as above.
        let texel = |anchor: Vec4, own: f32| {
            let place = Vec3::new(anchor.x, 0.0, anchor.z);
            (place - looked).dot(back)
                + ACROSS * (place - looked).dot(iso::right())
                + CHAIN * anchor.w
                + SQUEEZE * own
        };
        let true_depth = |point: Vec3| (point - looked).dot(back);
        // A bearer off its cell's centre, standing 3 units above the cell's height 12.
        let root = iso::ground(22.7, 13.6) + Vec3::Y * 15.0;
        let anchor = iso::ground(22.5, 13.5)
            .with_y(12.0)
            .extend(crate::models::PLACE);
        // `Orders::over`'s order.
        let rise = (root.y - anchor.y) * back.y;
        let order = InOrder::of(anchor, 0.0, root, looked, -OVER - rise);
        let own = |point: Vec3| (point - anchor.xyz()).dot(back);
        // Sparks at points of its model before its root, behind it and up at its head.
        for point in [
            back * 10.0 + Vec3::Y * 50.0,
            -back * 12.0 + Vec3::Y * 30.0,
            Vec3::Y * 110.0,
        ] {
            let spark = root + order.drawn(point, root);
            let was = root + point;
            // The same from a mesh whose vertices lie in the world, its origin the world's start.
            assert!(
                (order.drawn(was, Vec3::ZERO) - spark).length() < 1e-2,
                "{spark} {}",
                order.drawn(was, Vec3::ZERO)
            );
            // Where it shows on screen, it shows still.
            assert!(
                (spark - was).dot(iso::right()).abs() < 1e-3,
                "{was} {spark}"
            );
            assert!((spark - was).dot(iso::up()).abs() < 1e-3, "{was} {spark}");
            // Just before the model's texel there, by `OVER` squeezed: over the texels behind
            // it, under those before it.
            let depth = true_depth(spark) - texel(anchor, own(was));
            assert!((depth - SQUEEZE * OVER).abs() < 2e-3, "{depth}");
        }
    }

    #[test]
    fn a_layered_effect_takes_its_layers_order_and_keeps_the_rest() {
        let added = whole_material(Handle::default());
        let rings = layered(added.clone(), crate::blended::Layer::Rings);
        assert_eq!(rings.base.depth_bias, crate::blended::Layer::Rings.bias());
        assert!(rings.extension.layered && rings.extension.whole);
        assert_eq!(rings.base.alpha_mode, added.base.alpha_mode);
        assert!(!added.extension.layered);
    }
}
