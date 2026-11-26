//! The element stones' looks (827 to 830, 841, 939, 768; `cParticleSystem_elementstone`, #2915;
//! docs/re/effects.md, "The element stones"): the scripts' FX objects of the main quest's elements,
//! of quest 74's heart and of the finale's (`npcs`), each its element's model floating 200 over
//! its point at twice the models' scale and turning a radian a second, added whole in a veined
//! texture in place of its own; three of its element's signs going round it 40 out and 210 up,
//! added by their alpha; a lightning's stretch standing under it from 118 to 200, which jumps at
//! every frame, bright at its foot and fading to nothing at the model; and a large glow about the
//! model and a small flickering spark at the beam's foot, added whole
//! (`opensacred_rules::particles::ElementStone`). None darkens by night, none lights the ground,
//! and none ends; their loops are `fx_loops`'.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    self, ELEMENT_BEAM_ACROSS, ELEMENT_BEAM_FOOT, ELEMENT_BEAM_HALF, ELEMENT_BEAM_TOP,
    ELEMENT_GLOW_HALF, ELEMENT_SIGN_COLOUR, ELEMENT_SIGN_HALF, ELEMENT_SIGN_LIFT,
    ELEMENT_SPARK_HALF, ELEMENT_STONE_LIFT, ELEMENT_STONE_LOOKS, ELEMENT_STONE_SCALE,
    ELEMENT_STONE_TEXTURES, ElementStone,
};

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, texture_path,
    whole_material, wrapped_texture,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::spawning::Rng;

/// An element stone's object, of its variant, 1 to 7, which takes its look ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Stone(u8);

impl Stone {
    /// The stone of an FX object of the type `kind`, if it is one.
    pub(crate) fn of(kind: u32) -> Option<Self> {
        particles::element_stone(kind).map(Self)
    }

    /// Its variant less 1, by which its model and its sign are found.
    fn index(self) -> usize {
        usize::from(self.0.saturating_sub(1))
    }
}

/// The stones' looks, made as the first stone shows: each variant's model, relative to the asset
/// directory, and its sign's material; the models' two materials, the veined blue and red ones;
/// the beam's; and the large and the small glows'. None where a file is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: [Option<String>; 7],
    signs: [Option<Handle<Effect>>; 7],
    bodies: [Option<Handle<ModelMaterial>>; 2],
    beam: Option<Handle<Effect>>,
    glows: [Option<Handle<Effect>>; 2],
}

/// The stones' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x36))
    }
}

/// A stone that shows: its run, its model and whether its meshes wear their material, and the
/// meshes of its signs, its beam and its two glows.
#[derive(Component)]
struct Shown {
    stone: ElementStone,
    model: Option<(Entity, bool)>,
    meshes: [Handle<Mesh>; 4],
}

/// A stone's model, which stands in the world as every model does, of its stone.
#[derive(Component)]
struct Of(Entity);

/// Finds the stones' files at the first stone (`0x7897E0`).
fn find_looks(
    looks: &mut Looks,
    data: &Data,
    server: &AssetServer,
    (effects, materials, anchors): (&mut Assets<Effect>, &mut Assets<ModelMaterial>, &Anchors),
) {
    looks.made = true;
    let assets = &data.0.assets;
    let files = crate::npcs::model_files(assets).ok();
    let missing = |name: &str| warn!("element stones: no {name}; its part is not drawn");
    looks.models = ELEMENT_STONE_LOOKS.map(|(model, _)| {
        let found = files.as_ref().and_then(|files| files.get(model).cloned());
        if found.is_none() {
            missing(model);
        }
        found
    });
    looks.signs = ELEMENT_STONE_LOOKS.map(|(_, sign)| {
        let image = particle_texture(assets, server, sign);
        if image.is_none() {
            missing(sign);
        }
        image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
    });
    let [blue, red, beam, large, small] = ELEMENT_STONE_TEXTURES;
    looks.bodies = [blue, red].map(|name| {
        let Some(path) = texture_path(assets, name) else {
            missing(name);
            return None;
        };
        let texture = crate::npcs::load_texture(server, &path);
        Some(materials.add(in_order(
            StandardMaterial {
                base_color_texture: Some(texture),
                unlit: true,
                ..default()
            },
            anchors,
            Pass::Added(Layer::Objects),
        )))
    });
    looks.beam = wrapped_texture(assets, server, beam)
        .map(|image| effects.add(particle_material(image, AlphaMode::Add)));
    if looks.beam.is_none() {
        missing(beam);
    }
    looks.glows = [large, small].map(|name| {
        let image = particle_texture(assets, server, name);
        if image.is_none() {
            missing(name);
        }
        image.map(|image| effects.add(whole_material(image)))
    });
}

/// Gives each new stone its look: its model and its signs', beam's and glows' meshes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    stones: Query<(Entity, &Stone), Without<Shown>>,
    (data, server, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, anchors, mut materials): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    for (entity, &stone) in &stones {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(
                &mut looks,
                data,
                &server,
                (&mut effects, &mut materials, &anchors),
            );
        }
        let model = looks
            .models
            .get(stone.index())
            .cloned()
            .flatten()
            .map(|model| {
                let root = commands
                    .spawn((
                        model_looks::scene(&server, &model),
                        Transform::from_scale(Vec3::ZERO),
                        model_looks::Ordered::default(),
                        Of(entity),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();
                (root, false)
            });
        let sign = looks.signs.get(stone.index()).cloned().flatten();
        let [large, small] = looks.glows.clone();
        // Its parts under it, which go with it; its model by [`orphans`].
        let made = [sign, looks.beam.clone(), large, small].map(|material| {
            let mesh = meshes.add(empty());
            if let Some(material) = material {
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
        });
        debug!("element stones: the stone {} shows", stone.0);
        commands.entity(entity).insert(Shown {
            stone: ElementStone::default(),
            model,
            meshes: made,
        });
    }
}

/// Dresses each stone's model's meshes in its veined material once they are there (`0x40D310`:
/// added whole, its depth tested and not written, unlit), the blue one, the unholy symbol's red.
fn dress(
    mut commands: Commands,
    mut shown: Query<(&Stone, &mut Shown)>,
    looks: Res<Looks>,
    (children, meshes): (
        Query<&Children>,
        Query<Entity, With<MeshMaterial3d<ModelMaterial>>>,
    ),
) {
    for (stone, mut shown) in &mut shown {
        let Some((root, dressed)) = shown.model.as_mut() else {
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
        let Some(material) = looks.bodies[usize::from(stone.0 == 7)].clone() else {
            continue;
        };
        for entity in own {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(material.clone()));
        }
    }
}

/// Runs each stone a frame on (`0x7892C0`) and draws it (`0x789310`): its model turning 200 over
/// its point; its three signs about it; its beam's stretch at a roll; its large glow, and its
/// spark of the same roll.
fn run(
    time: Res<Time>,
    mut shown: Query<(&Stone, &mut Shown, &GlobalTransform)>,
    mut models: Query<&mut Transform, Without<Shown>>,
    (mut meshes, orders, mut dice, preroll): (
        ResMut<Assets<Mesh>>,
        Orders,
        ResMut<Dice>,
        Res<crate::fx::Preroll>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (stone, mut shown, place) in &mut shown {
        let shown = &mut *shown;
        // A load's first frame runs it on first (#3805).
        for (dt, _) in preroll.steps(dt) {
            shown.stone.update(dt);
        }
        let origin = place.translation();
        if let Some((root, _)) = &shown.model
            && let Ok(mut transform) = models.get_mut(*root)
        {
            *transform =
                Transform::from_translation(origin + world([0.0, 0.0, ELEMENT_STONE_LIFT]))
                    .with_rotation(Quat::from_rotation_y(shown.stone.phase))
                    .with_scale(Vec3::splat(ELEMENT_STONE_SCALE * s));
        }
        // `rand()` over 32,767 (`0x849B40`, `0x890718`).
        let r = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0;
        let [foot, large, small] = ElementStone::colours(stone.0);
        // The signs, each tangent to the circle about the stone, the texture's top up and its
        // `u` from the circle's turning side.
        let mut signs = Quads::default();
        for index in 0..3 {
            let ([x, y, z], [ax, ay]) = shown.stone.sign(index);
            let h = ELEMENT_SIGN_HALF;
            let corner =
                |side: f32, up: f32| world([x + ax * side * h, y + ay * side * h, z + up * h]);
            signs.push_corners(
                [
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                    corner(-1.0, -1.0),
                ],
                [Vec2::Y, Vec2::ZERO, Vec2::X, Vec2::ONE],
                ELEMENT_SIGN_COLOUR,
            );
        }
        // The beam, upright facing the viewer, its stretch at the roll, black at its top.
        let [top_u, foot_u] = ElementStone::beam_u(r);
        let [b, f, t] = [ELEMENT_BEAM_HALF, ELEMENT_BEAM_FOOT, ELEMENT_BEAM_TOP];
        let black = [0.0, 0.0, 0.0, 1.0];
        let mut beam = Quads::default();
        beam.push_corners_shaded(
            [
                world([-b, 0.0, f]),
                world([-b, 0.0, t]),
                world([b, 0.0, t]),
                world([b, 0.0, f]),
            ],
            [
                Vec2::new(foot_u, ELEMENT_BEAM_ACROSS),
                Vec2::new(top_u, ELEMENT_BEAM_ACROSS),
                Vec2::new(top_u, 0.0),
                Vec2::new(foot_u, 0.0),
            ],
            [foot, black, black, foot],
        );
        let mut glow = Quads::default();
        glow.push_facing(
            world([0.0, 0.0, ELEMENT_SIGN_LIFT]),
            ELEMENT_GLOW_HALF * s,
            [large; 4],
        );
        let mut spark = Quads::default();
        spark.push_facing(
            world([0.0, 0.0, ELEMENT_BEAM_FOOT]),
            (ELEMENT_SPARK_HALF + r) * s,
            [small; 4],
        );
        let order = orders.at(origin);
        for (mut quads, mesh) in [signs, beam, glow, spark].into_iter().zip(&shown.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// Takes away the models of the stones gone with their sectors.
fn orphans(mut commands: Commands, models: Query<(Entity, &Of)>, stones: Query<(), With<Shown>>) {
    for (model, &Of(stone)) in &models {
        if !stones.contains(stone) {
            commands.entity(model).try_despawn();
        }
    }
}

pub struct ElementStonesPlugin;

impl Plugin for ElementStonesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, dress, run, orphans)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
