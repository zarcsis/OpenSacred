//! The Underworld's glyphs, their fire and the temple door's spikes (1175 to 1178, 1179, 1174;
//! #2918; docs/re/effects.md, "The Underworld's glyphs"): the scripts' FX objects of the add-on's
//! main quests 52 and 53 (`npcs`). A lock's glyph is its sign's model, turning a radian a second
//! 140 over its point, drawn twice added whole in a veined gold texture whose coordinates slide,
//! wrapped in twenty arcs of gold light swirling about it and a large dim amber glow
//! (`opensacred_rules::particles::UwGlyph`); the fire is its red sign turning in a faint cloud of
//! glows (`GlyphFire`); the spikes six spikes standing across the temple door, leaning in over its
//! middle, which grow from the ground in a second and sink back in a second once the scripts
//! signal them (`DoorSpikes`). None of them lights the world, sounds or darkens by night but the
//! spikes, lit models as the world's. The start's own spikes end at once, as the original's, and
//! are never placed (`npcs`, #3049).
//!
//! Not followed: the spikes' shadows.

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling, RenderLayers};
use bevy::math::Affine2;
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::particles::{
    DOOR_SPIKE_MODEL, DOOR_SPIKE_TEXTURE, DOOR_SPIKES, DoorSpikes, GlyphFire, SPIKE_TILT,
    UW_FIRE_LIFT, UW_FIRE_MODEL, UW_FIRE_SCALE, UW_FIRE_TEXTURES, UW_GLOW_COLOUR, UW_GLOW_HALF,
    UW_GLYPH_FIRE, UW_GLYPH_GREY, UW_GLYPH_LIFT, UW_GLYPH_MODELS, UW_GLYPH_SCALE,
    UW_GLYPH_TEXTURES, UW_GLYPHS, UW_RING_COLOUR, UW_RING_LIFT, UwGlyph, glyph_offsets,
};
use opensacred_rules::spawning::Rng;

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material, wrapped_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};

/// A model's mesh as its scene makes it, which a sign's second pass copies.
type ModelMesh = (
    Entity,
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// An object of the Underworld's quests that a script made, of its FX type, and the name by which
/// the scripts signal it.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Underworld {
    pub kind: u32,
    pub name: Option<String>,
}

impl Underworld {
    /// Whether the FX type `kind` is one of these.
    pub(crate) fn is(kind: u32) -> bool {
        kind == DOOR_SPIKES || kind == UW_GLYPH_FIRE || UW_GLYPHS.contains(&kind)
    }
}

/// An object's run.
enum Run {
    Spikes(DoorSpikes),
    Glyph(UwGlyph),
    Fire(GlyphFire),
}

/// An object that shows: its run; its models (the spikes' six, a sign's one) and whether each one's
/// meshes wear its passes yet; its passes' materials, the first on the meshes and each other on a
/// copy of them, and whether the night leaves them as they are; and the meshes of its quads (a
/// glyph's rings and glow, the fire's glows).
#[derive(Component)]
struct Shown {
    run: Run,
    models: Vec<(Entity, bool)>,
    passes: Vec<Handle<ModelMaterial>>,
    untinted: bool,
    meshes: Vec<Handle<Mesh>>,
}

/// An object's model, which stands in the world as every model does, of its object.
#[derive(Component)]
struct Of(Entity);

/// The models, relative to the asset directory, by their names; the signs' textures (the
/// veined gold, the veined red) for their passes and the spikes' bark; the glyphs' rings' and
/// glows' materials.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: std::collections::HashMap<String, String>,
    gold: Option<Handle<Image>>,
    red: Option<Handle<Image>>,
    bark: Option<Handle<Image>>,
    rings: Option<Handle<Effect>>,
    glows: Option<Handle<Effect>>,
}

/// The objects' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x497))
    }
}

/// Finds the models and the textures at the first object (`0x7C4470`, `0x7C4A00`, `0x7C54C0`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer, effects: &mut Assets<Effect>) {
    looks.made = true;
    let assets = &data.0.assets;
    looks.models = crate::npcs::model_files(assets).unwrap_or_default();
    for name in UW_GLYPH_MODELS
        .iter()
        .chain([&DOOR_SPIKE_MODEL, &UW_FIRE_MODEL])
    {
        if !looks.models.contains_key(*name) {
            warn!("underworld glyphs: no model {name}; it is not drawn");
        }
    }
    let [red, _, glow] = UW_FIRE_TEXTURES;
    let [gold, rings, _] = UW_GLYPH_TEXTURES;
    let missing = |name: &str| warn!("underworld glyphs: no texture {name}; its part is not drawn");
    looks.gold = wrapped_texture(assets, server, gold);
    looks.red = wrapped_texture(assets, server, red);
    looks.bark = crate::fx::texture_path(assets, DOOR_SPIKE_TEXTURE)
        .map(|path| crate::npcs::load_texture(server, &path));
    for (name, image) in [
        (gold, &looks.gold),
        (red, &looks.red),
        (DOOR_SPIKE_TEXTURE, &looks.bark),
    ] {
        if image.is_none() {
            missing(name);
        }
    }
    looks.rings =
        particle_texture(assets, server, rings).map(|image| effects.add(whole_material(image)));
    looks.glows =
        particle_texture(assets, server, glow).map(|image| effects.add(whole_material(image)));
    if looks.rings.is_none() || looks.glows.is_none() {
        missing("fx_demonform_demon or PARTICLE_GLOW03");
    }
}

/// A sign's pass: its texture in place of its own, of a grey, added whole, unlit, its depth tested
/// and not written (`0x40D310`).
fn pass(texture: Option<Handle<Image>>, grey: f32, anchors: &Anchors) -> ModelMaterial {
    in_order(
        StandardMaterial {
            base_color: Color::srgb(grey, grey, grey),
            base_color_texture: texture,
            unlit: true,
            ..default()
        },
        anchors,
        Pass::Added(Layer::Objects),
    )
}

/// Gives each new object its run, its models, its passes and its quads' meshes (`0x7C4780`,
/// `0x7C5100`, `0x7C58C0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    objects: Query<(Entity, &Underworld), Without<Shown>>,
    (data, server, mut looks, mut effects): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
    ),
    (mut meshes, anchors, mut materials, mut dice): (
        ResMut<Assets<Mesh>>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
        ResMut<Dice>,
    ),
) {
    for (entity, object) in &objects {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server, &mut effects);
        }
        let model = |commands: &mut Commands, name: &str| {
            looks.models.get(name).map(|file| {
                commands
                    .spawn((
                        model_looks::scene(&server, file),
                        Transform::from_scale(Vec3::ZERO),
                        model_looks::Ordered::default(),
                        Of(entity),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
        };
        let quads = |commands: &mut Commands,
                     meshes: &mut Assets<Mesh>,
                     materials: &[Option<Handle<Effect>>]| {
            materials
                .iter()
                .map(|material| {
                    let mesh = meshes.add(empty());
                    if let Some(material) = material.clone() {
                        commands.spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            Transform::IDENTITY,
                            NoFrustumCulling,
                            RenderLayers::layer(iso::UNTINTED),
                            ChildOf(entity),
                        ));
                    }
                    mesh
                })
                .collect::<Vec<_>>()
        };
        let shown = if object.kind == DOOR_SPIKES {
            let models = DoorSpikes::spikes()
                .iter()
                .filter_map(|_| model(&mut commands, DOOR_SPIKE_MODEL))
                .map(|model| (model, false))
                .collect();
            // Its bark in place of the model's own, lit, its depth written (`0x401BD0`,
            // `0x405C90`).
            let bark = in_order(
                StandardMaterial {
                    base_color_texture: looks.bark.clone(),
                    ..default()
                },
                &anchors,
                Pass::Tested,
            );
            Shown {
                run: Run::Spikes(DoorSpikes::default()),
                models,
                passes: vec![materials.add(bark)],
                untinted: false,
                meshes: Vec::new(),
            }
        } else if object.kind == UW_GLYPH_FIRE {
            let passes = [
                pass(looks.red.clone(), 1.0, &anchors),
                pass(looks.gold.clone(), UW_GLYPH_GREY, &anchors),
            ]
            .map(|material| materials.add(material));
            Shown {
                run: Run::Fire(GlyphFire::default()),
                models: model(&mut commands, UW_FIRE_MODEL)
                    .map(|model| (model, false))
                    .into_iter()
                    .collect(),
                passes: passes.to_vec(),
                untinted: true,
                meshes: quads(&mut commands, &mut meshes, &[looks.glows.clone()]),
            }
        } else {
            let variant = usize::try_from(object.kind - UW_GLYPHS.start()).unwrap_or(0);
            let name = UW_GLYPH_MODELS[variant.min(UW_GLYPH_MODELS.len() - 1)];
            let passes =
                [(); 2].map(|()| materials.add(pass(looks.gold.clone(), UW_GLYPH_GREY, &anchors)));
            Shown {
                run: Run::Glyph(UwGlyph::new(&mut dice.0)),
                models: model(&mut commands, name)
                    .map(|model| (model, false))
                    .into_iter()
                    .collect(),
                passes: passes.to_vec(),
                untinted: true,
                meshes: quads(
                    &mut commands,
                    &mut meshes,
                    &[looks.rings.clone(), looks.glows.clone()],
                ),
            }
        };
        debug!("underworld glyphs: the object {} shows", object.kind);
        commands.entity(entity).insert(shown);
    }
}

/// Dresses each model's meshes once they are there: in the object's first pass, and a copy of each
/// in each other pass (a sign's `0x40D310` twice, untinted; the spikes' bark, lit).
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes): (
        Query<&Children>,
        Query<ModelMesh, With<MeshMaterial3d<ModelMaterial>>>,
    ),
) {
    for mut object in &mut shown {
        let object = &mut *object;
        let Some((first, others)) = object.passes.split_first() else {
            continue;
        };
        for (model, dressed) in &mut object.models {
            if *dressed {
                continue;
            }
            let own: Vec<_> = children
                .iter_descendants(*model)
                .filter_map(|entity| meshes.get(entity).ok())
                .collect();
            if own.is_empty() {
                continue;
            }
            *dressed = true;
            for (entity, mesh, skinned, tag, dynamic, unculled) in own {
                let mut dressing = commands.entity(entity);
                dressing.insert(MeshMaterial3d(first.clone()));
                if object.untinted {
                    dressing.insert(RenderLayers::layer(iso::UNTINTED));
                }
                for pass in others {
                    let mut copy = commands.spawn((
                        Mesh3d(mesh.0.clone()),
                        MeshMaterial3d(pass.clone()),
                        tag.clone(),
                        ChildOf(entity),
                    ));
                    if object.untinted {
                        copy.insert(RenderLayers::layer(iso::UNTINTED));
                    }
                    if let Some(skinned) = skinned {
                        copy.insert(skinned.clone());
                    }
                    if dynamic {
                        copy.insert(DynamicSkinnedMeshBounds);
                    }
                    if unculled {
                        copy.insert(NoFrustumCulling);
                    }
                }
            }
        }
    }
}

/// Hands the scripts' signals to the spikes of their names (`SetObjState`'s `0x3C`, the case
/// `0xB0`).
fn signal(
    mut signals: MessageReader<crate::forge_elements::Signal>,
    mut shown: Query<(&Underworld, &mut Shown)>,
) {
    for crate::forge_elements::Signal(name) in signals.read() {
        for (object, mut shown) in &mut shown {
            if object.name.as_deref() == Some(name.as_str())
                && let Run::Spikes(spikes) = &mut shown.run
            {
                debug!("underworld glyphs: the door's spikes signalled");
                spikes.signalled = true;
            }
        }
    }
}

/// Runs each object a frame on and draws it (`0x7C4520`, `0x7C4B40`, `0x7C5600`; `0x7C45C0`,
/// `0x7C4C10`, `0x7C5680`); the spikes that have sunk go with their object.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    mut models: Query<&mut Transform, With<Of>>,
    (mut meshes, mut materials, orders, mut dice): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<ModelMaterial>>,
        Orders,
        ResMut<Dice>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut shown, place) in &mut shown {
        let shown = &mut *shown;
        let origin = place.translation();
        let (clock, sign) = match &mut shown.run {
            Run::Spikes(spikes) => {
                if !spikes.update(dt) {
                    debug!("underworld glyphs: the door's spikes have sunk");
                    for (model, _) in &shown.models {
                        commands.entity(*model).try_despawn();
                    }
                    commands.entity(entity).despawn();
                    continue;
                }
                for (spike, (model, _)) in DoorSpikes::spikes().iter().zip(&shown.models) {
                    let Ok(mut at) = models.get_mut(*model) else {
                        continue;
                    };
                    let [x, y] = spike.offset;
                    // Its up tilted toward its +x, then its +x turned toward the middle.
                    let toward = world([spike.turn.cos(), -spike.turn.sin(), 0.0]).xz();
                    let yaw = (-toward.y).atan2(toward.x);
                    *at = Transform::from_translation(origin + world([x, y, 0.0]))
                        .with_rotation(
                            Quat::from_rotation_y(yaw) * Quat::from_rotation_z(-SPIKE_TILT),
                        )
                        .with_scale(Vec3::splat(spike.size * spikes.growth * s));
                }
                continue;
            }
            Run::Glyph(glyph) => {
                glyph.update(dt);
                let mut rings = Quads::default();
                for k in 0..glyph.rings.len() {
                    let band = UwGlyph::band(&glyph.ring(k));
                    let lift = |[x, y, z]: [f32; 3]| world([x, y, z + UW_RING_LIFT]);
                    for j in 0..band.len() - 1 {
                        let [v0, v1] =
                            [j, j + 1].map(|j| f32::from(u8::try_from(j).unwrap_or(0)) / 8.0);
                        rings.push_corners(
                            [
                                lift(band[j][0]),
                                lift(band[j + 1][0]),
                                lift(band[j + 1][1]),
                                lift(band[j][1]),
                            ],
                            [
                                Vec2::new(0.0, v0),
                                Vec2::new(0.0, v1),
                                Vec2::new(1.0, v1),
                                Vec2::new(1.0, v0),
                            ],
                            UW_RING_COLOUR,
                        );
                    }
                }
                let mut glow = Quads::default();
                glow.push_facing(
                    world([0.0, 0.0, UW_RING_LIFT]),
                    UW_GLOW_HALF * s,
                    [UW_GLOW_COLOUR; 4],
                );
                let order = orders.at(origin);
                for (mut quads, mesh) in [rings, glow].into_iter().zip(&shown.meshes) {
                    if let Some(mut mesh) = meshes.get_mut(mesh) {
                        quads.in_order(order, origin);
                        quads.into_mesh(&mut mesh);
                    }
                }
                (glyph.clock, (glyph.phase, UW_GLYPH_LIFT, UW_GLYPH_SCALE))
            }
            Run::Fire(fire) => {
                fire.update(dt, &mut dice.0);
                let ramp = GlyphFire::ramp();
                let mut glows = Quads::default();
                for particle in &fire.system.particles {
                    let colour = ramp.at(particle.life);
                    glows.push_facing(
                        world(particle.place),
                        particle.size * s,
                        [[colour[0], colour[1], colour[2], 1.0]; 4],
                    );
                }
                if let Some(mesh) = shown.meshes.first()
                    && let Some(mut mesh) = meshes.get_mut(mesh)
                {
                    glows.in_order(orders.at(origin), origin);
                    glows.into_mesh(&mut mesh);
                }
                (fire.system.clock, (fire.phase, UW_FIRE_LIFT, UW_FIRE_SCALE))
            }
        };
        // The sign turning over its point, its two passes' textures sliding.
        let (phase, lift, scale) = sign;
        if let Some((model, _)) = shown.models.first()
            && let Ok(mut at) = models.get_mut(*model)
        {
            *at = Transform::from_translation(origin + world([0.0, 0.0, lift]))
                .with_rotation(Quat::from_rotation_y(phase))
                .with_scale(Vec3::splat(scale * s));
        }
        for (pass, offset) in shown.passes.iter().zip(glyph_offsets(clock)) {
            if let Some(mut material) = materials.get_mut(pass) {
                material.base.uv_transform = Affine2::from_translation(Vec2::from(offset));
            }
        }
    }
}

/// Takes away the models of the objects gone with their sectors or ended.
fn orphans(mut commands: Commands, models: Query<(Entity, &Of)>, shown: Query<(), With<Shown>>) {
    for (model, &Of(object)) in &models {
        if !shown.contains(object) {
            commands.entity(model).try_despawn();
        }
    }
}

pub struct UnderworldGlyphsPlugin;

impl Plugin for UnderworldGlyphsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, dress, signal, run, orphans)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
