//! The forge's ritual (911 to 916, #2098; docs/re/effects.md, "The forge's elements"): the five
//! elements that a script's `CreateObj` makes about the forge (`npcs`), each its model floating
//! and turning over a column of light with a bright edge sweeping about it and a ring and a glow
//! flat under it (`opensacred_rules::particles::ForgeElement`); the scripts' signal
//! (`SetObjState`'s `0x3C`, [`Signal`]) shrinks and fades them, and the air element then shakes
//! the screen ever harder and makes the beam at the forge's middle
//! (`opensacred_rules::particles::ForgeBeam`): a column of light and a streak of the horizon
//! across it, upright and facing the viewer, a glow at its foot and rings rising from it, with
//! `SPELL_BIGSUMMON` and a light of 500. The columns, rings, glows and the beam are added whole
//! and untinted by the night; the elements' models are unlit, at their places in the order.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    FORGE_BEAM_AT, FORGE_BEAM_CORE, FORGE_BEAM_GLOW_HALF, FORGE_BEAM_HORIZON,
    FORGE_BEAM_HORIZON_COLOUR, FORGE_BEAM_LIFT, FORGE_BEAM_LIGHT, FORGE_BEAM_SOUND,
    FORGE_BEAM_TEXTURES, FORGE_COLUMN_HEIGHT, FORGE_COLUMN_PAIRS, FORGE_COLUMN_RADIUS,
    FORGE_ELEMENT_LOOKS, FORGE_LIFT, FORGE_RING_HALF, FORGE_TEXTURES, ForgeBeam, ForgeElement,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_texture, texture_path, whole_material, wrapped_texture,
};
use crate::game::{AppState, Data};
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::spawning::Rng;
use crate::{daynight, iso};

/// An element of the forge's ritual, of its variant (its type less 911) and its name in the
/// scripts (its `CreateObj`'s `0x01`, lower-cased), which `npcs` makes for a script's `CreateObj`.
#[derive(Component)]
pub(crate) struct Element {
    pub variant: u8,
    pub name: Option<String>,
}

/// The scripts' signal to the FX objects of a name, lower-cased (`SetObjState`'s `0x3C`,
/// `0x48B8C4`), which `npcs` sends as the scripts run it.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Signal(pub String);

/// The ritual's looks, made as the first element shows: each variant's model, relative to the
/// asset directory, and its texture; the elements' column's, ring's and glow's materials; and the
/// beam's core's, horizon's, rings' and glow's. None where a file is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: [Option<(String, Handle<Image>)>; 5],
    parts: [Option<Handle<Effect>>; 3],
    beam: [Option<Handle<Effect>>; 4],
}

/// The beams' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x394))
    }
}

/// An element that shows: its run, its model, its model's material and whether its meshes wear
/// it, and the meshes of its column, its ring and its glow.
#[derive(Component)]
struct Shown {
    element: ForgeElement,
    model: Option<(Entity, Handle<ModelMaterial>, bool)>,
    meshes: [Handle<Mesh>; 3],
    parts: Vec<Entity>,
}

/// A beam that shows: its run and the meshes of its core and horizon, its rings and its glow.
#[derive(Component)]
struct Beam {
    beam: ForgeBeam,
    meshes: [Handle<Mesh>; 3],
}

/// Meshes added whole under `parent`, untinted by the night, of the materials `materials`, each
/// its own mesh; the entities made.
fn parts<const N: usize>(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    materials: [Option<&Handle<Effect>>; N],
) -> ([Handle<Mesh>; N], Vec<Entity>) {
    let mut made = Vec::new();
    let handles = materials.map(|material| {
        let mesh = meshes.add(empty());
        if let Some(material) = material {
            made.push(
                commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                        ChildOf(parent),
                    ))
                    .id(),
            );
        }
        mesh
    });
    (handles, made)
}

/// Finds the ritual's files at the first element (`0x7A72E0`, `0x7A7C80`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer, effects: &mut Assets<Effect>) {
    looks.made = true;
    let assets = &data.0.assets;
    let files = crate::npcs::model_files(assets).ok();
    looks.models = FORGE_ELEMENT_LOOKS.map(|(model_name, texture_name)| {
        let model = files
            .as_ref()
            .and_then(|files| files.get(model_name).cloned());
        let texture =
            texture_path(assets, texture_name).map(|path| crate::npcs::load_texture(server, &path));
        if model.is_none() || texture.is_none() {
            warn!("forge elements: no {model_name} or {texture_name}; its model is not drawn");
        }
        model.zip(texture)
    });
    let load = |name: &str, wrapped: bool| {
        let image = if wrapped {
            wrapped_texture(assets, server, name)
        } else {
            particle_texture(assets, server, name)
        };
        if image.is_none() {
            warn!("forge elements: no texture {name}; its part is not drawn");
        }
        image
    };
    let [column, ring, glow] = FORGE_TEXTURES;
    looks.parts = [load(column, true), load(ring, false), load(glow, false)]
        .map(|image| image.map(|image| effects.add(whole_material(image))));
    looks.beam = FORGE_BEAM_TEXTURES
        .map(|name| load(name, false).map(|image| effects.add(whole_material(image))));
}

/// Gives each new element its look (`0x7A7C30`): its model, unlit and alpha tested as every
/// model is, and its column, ring and glow.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    elements: Query<(Entity, &Element), Without<Shown>>,
    (data, server, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, anchors, mut materials): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    for (entity, element) in &elements {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server, &mut effects);
        }
        let model = looks
            .models
            .get(usize::from(element.variant))
            .cloned()
            .flatten()
            .map(|(model, texture)| {
                let root = commands
                    .spawn((
                        model_looks::scene(&server, &model),
                        Transform::from_scale(Vec3::ZERO),
                        model_looks::Ordered::default(),
                        Of(entity),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();
                let material = materials.add(in_order(
                    StandardMaterial {
                        base_color_texture: Some(texture),
                        unlit: true,
                        ..default()
                    },
                    &anchors,
                    Pass::Tested,
                ));
                (root, material, false)
            });
        let [column, ring, glow] = looks.parts.each_ref().map(Option::as_ref);
        let (part_meshes, mut made) =
            parts(&mut commands, entity, &mut meshes, [column, ring, glow]);
        made.extend(model.as_ref().map(|(root, ..)| *root));
        debug!(
            "forge elements: the element {} ({:?}) shows",
            element.variant, element.name
        );
        commands.entity(entity).insert(Shown {
            element: ForgeElement::new(element.variant),
            model,
            meshes: part_meshes,
            parts: made,
        });
    }
}

/// Dresses each element's model's meshes in its material once they are there (`0x40D310`:
/// unlit, its depth tested and written).
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes): (
        Query<&Children>,
        Query<Entity, With<MeshMaterial3d<ModelMaterial>>>,
    ),
) {
    for mut shown in &mut shown {
        let Some((root, material, dressed)) = shown.model.as_mut() else {
            continue;
        };
        if *dressed {
            continue;
        }
        let own: Vec<Entity> = children
            .iter_descendants(*root)
            .filter(|&entity| meshes.contains(entity))
            .collect();
        if own.is_empty() {
            continue;
        }
        *dressed = true;
        for entity in own {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(material.clone()));
        }
    }
}

/// Hands the scripts' signals to the elements of their names (`0x5A4E17`).
fn signal(mut signals: MessageReader<Signal>, mut shown: Query<(&Element, &mut Shown)>) {
    for Signal(name) in signals.read() {
        for (element, mut shown) in &mut shown {
            if element.name.as_deref() == Some(name.as_str()) {
                debug!("forge elements: the element {} signalled", element.variant);
                shown.element.signal();
            }
        }
    }
}

/// The column's quads (`+0x20B8`, drawn as a strip): between each pair and the next about the
/// circle, its foot at `foot`, of the radius `radius` and the height `height`, each pair in its
/// grey.
fn column(element: &ForgeElement, foot: [f32; 3], radius: f32, height: f32) -> Quads {
    let mut quads = Quads::default();
    let point = |pair: u8, up: f32| {
        let angle = std::f32::consts::TAU * f32::from(pair) / f32::from(FORGE_COLUMN_PAIRS - 1);
        world([
            foot[0] + radius * angle.sin(),
            foot[1] + radius * angle.cos(),
            foot[2] + up,
        ])
    };
    for pair in 0..FORGE_COLUMN_PAIRS - 1 {
        let next = pair + 1;
        let [u, v] = [0.1 * f32::from(pair), 0.1 * f32::from(next)];
        let grey = |pair: u8| {
            let g = element.column_grey(pair);
            [g, g, g, 1.0]
        };
        quads.push_corners_shaded(
            [
                point(pair, 0.0),
                point(pair, height),
                point(next, height),
                point(next, 0.0),
            ],
            [
                Vec2::new(u, 1.0),
                Vec2::new(u, 0.1),
                Vec2::new(v, 0.1),
                Vec2::new(v, 1.0),
            ],
            [grey(pair), grey(pair), grey(next), grey(next)],
        );
    }
    quads
}

/// A flat square over its place at `lift`, of the half side `half`, turned by `turn` about the
/// upright, the whole texture, white.
fn flat(lift: f32, half: f32, turn: f32) -> Quads {
    let (sin, cos) = turn.sin_cos();
    let corner = |x: f32, y: f32| world([x * cos - y * sin, x * sin + y * cos, lift]);
    let mut quads = Quads::default();
    quads.push_corners(
        [
            corner(-half, -half),
            corner(-half, half),
            corner(half, half),
            corner(half, -half),
        ],
        [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X],
        [1.0; 4],
    );
    quads
}

/// Runs each element a frame on (`0x7A7500`) and draws it (`0x7A7720`): its model floating and
/// turning at its scale and fade, its column, its ring and its glow; the air's quake and its
/// beam as it asks them; it goes as it ends, its object kept.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    mut models: Query<&mut Transform, Without<Shown>>,
    (mut meshes, orders, heights): (ResMut<Assets<Mesh>>, Orders, Res<crate::terrain::Heights>),
    (mut weather, mut sounds): (
        Option<ResMut<crate::weather::Weather>>,
        MessageWriter<crate::sfx::PlayAt>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut shown, place) in &mut shown {
        let shown = &mut *shown;
        let asks = shown.element.update(dt);
        if let (Some(value), Some(weather)) = (asks.quake, weather.as_deref_mut()) {
            weather.set(opensacred_rules::weather::Event::Quake(value));
        }
        let origin = place.translation();
        if asks.beam {
            // At the forge's middle, on the ground's height at the air element (`0x5B38A0`).
            let [x, z] = FORGE_BEAM_AT;
            let y = heights
                .standing(origin.x, origin.z, None)
                .unwrap_or(origin.y);
            let at = Vec3::new(x, y, z);
            debug!("forge elements: the beam at {at}");
            commands.spawn((
                Unmade,
                Transform::from_translation(at),
                Visibility::default(),
                daynight::Light(FORGE_BEAM_LIGHT),
                DespawnOnExit(AppState::InGame),
            ));
            sounds.write(crate::sfx::PlayAt {
                sound: crate::sfx::Of::Id(FORGE_BEAM_SOUND),
                point: at,
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
        if shown.element.ended() {
            for &part in &shown.parts {
                commands.entity(part).try_despawn();
            }
            commands.entity(entity).remove::<(Shown, Element)>();
            continue;
        }
        let (fade, scale) = (shown.element.fade(), shown.element.scale());
        let visible = shown.element.shows();
        if let Some((root, ..)) = &shown.model
            && let Ok(mut transform) = models.get_mut(*root)
        {
            let width = if visible { scale * fade * s } else { 0.0 };
            *transform =
                Transform::from_translation(origin + world([0.0, 0.0, shown.element.model_lift()]))
                .with_rotation(Quat::from_rotation_y(shown.element.model_turn()))
                .with_scale(Vec3::new(width, if visible { s } else { 0.0 }, width));
        }
        let order = orders.at(origin);
        let mut drawn = if visible {
            [
                column(
                    &shown.element,
                    [0.0, 0.0, FORGE_LIFT],
                    FORGE_COLUMN_RADIUS * scale,
                    FORGE_COLUMN_HEIGHT * fade,
                ),
                flat(
                    FORGE_LIFT,
                    FORGE_RING_HALF * scale,
                    shown.element.ring_turn(),
                ),
                flat(FORGE_LIFT, FORGE_RING_HALF * fade, 0.0),
            ]
        } else {
            std::array::from_fn(|_| Quads::default())
        };
        for (quads, mesh) in drawn.iter_mut().zip(&shown.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, origin);
                std::mem::take(quads).into_mesh(&mut mesh);
            }
        }
    }
}

/// An element's model, which stands in the world as every model does, of its element.
#[derive(Component)]
struct Of(Entity);

/// Takes away the models of the elements gone with their sectors.
fn orphans(
    mut commands: Commands,
    models: Query<(Entity, &Of)>,
    elements: Query<(), With<Shown>>,
) {
    for (model, &Of(element)) in &models {
        if !elements.contains(element) {
            commands.entity(model).try_despawn();
        }
    }
}

/// A beam to make, at its place.
#[derive(Component)]
struct Unmade;

/// Gives each new beam its parts (`0x7A8430`).
fn make_beam(
    mut commands: Commands,
    unmade: Query<Entity, With<Unmade>>,
    looks: Res<Looks>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for entity in &unmade {
        let [core, horizon, rings, glow] = looks.beam.each_ref().map(Option::as_ref);
        let (core_mesh, _) = parts(&mut commands, entity, &mut meshes, [core]);
        let (horizon_mesh, _) = parts(&mut commands, entity, &mut meshes, [horizon]);
        let (rest, _) = parts(&mut commands, entity, &mut meshes, [rings, glow]);
        let [core_mesh] = core_mesh;
        let [horizon_mesh] = horizon_mesh;
        let [rings_mesh, glow_mesh] = rest;
        commands.entity(entity).remove::<Unmade>().insert((
            Beam {
                beam: ForgeBeam::new(),
                meshes: [core_mesh, rings_mesh, glow_mesh],
            },
            Horizon(horizon_mesh),
        ));
    }
}

/// The mesh of a beam's horizon.
#[derive(Component)]
struct Horizon(Handle<Mesh>);

/// Runs each beam a frame on (`0x7A7F50`) and draws it (`0x7A8030`): its core and its horizon
/// upright facing the viewer and its glow at its pulse and fade, its rings rising; it goes as it
/// ends.
fn shine(
    mut commands: Commands,
    time: Res<Time>,
    mut beams: Query<(Entity, &mut Beam, &Horizon, &GlobalTransform)>,
    (mut meshes, mut dice, orders): (ResMut<Assets<Mesh>>, ResMut<Dice>, Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut shown, horizon, place) in &mut beams {
        shown.beam.update(dt, &mut dice.0);
        if shown.beam.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let (pulse, fade) = (shown.beam.pulse(), shown.beam.fade());
        let wide = pulse * fade;
        let upright = |x: f32, z: f32| world([x, 0.0, FORGE_BEAM_LIFT + z]);
        let [across, up] = FORGE_BEAM_CORE;
        let mut core = Quads::default();
        core.push_corners(
            [
                upright(-across * wide, 0.0),
                upright(-across * wide, up * fade),
                upright(across * wide, up * fade),
                upright(across * wide, 0.0),
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            [1.0; 4],
        );
        let [across, up] = FORGE_BEAM_HORIZON;
        let mut streak = Quads::default();
        streak.push_corners(
            [
                upright(-across * wide, -up * fade),
                upright(-across * wide, up * fade),
                upright(across * wide, up * fade),
                upright(across * wide, -up * fade),
            ],
            [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X],
            FORGE_BEAM_HORIZON_COLOUR,
        );
        let mut rings = Quads::default();
        for ring in &shown.beam.system.particles {
            rings.push(
                world(ring.place),
                ring.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                [1.0; 4],
            );
        }
        let mut glow = Quads::default();
        glow.push(
            world([0.0, 0.0, FORGE_BEAM_LIFT]),
            FORGE_BEAM_GLOW_HALF * wide * s,
            0.0,
            (Vec2::ZERO, 1.0),
            [1.0; 4],
        );
        let origin = place.translation();
        let order = orders.at(origin);
        let [core_mesh, rings_mesh, glow_mesh] = &shown.meshes;
        for (mut quads, mesh) in [
            (core, core_mesh),
            (streak, &horizon.0),
            (rings, rings_mesh),
            (glow, glow_mesh),
        ] {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The forge's elements and its beam.
pub struct ForgeElementsPlugin;

impl Plugin for ForgeElementsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Signal>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, dress, signal, run, orphans, make_beam, shine)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
