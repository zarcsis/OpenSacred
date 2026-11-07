//! The gold arrows that a script's `Partikel` of the variant 3 sets over a place (#1846;
//! docs/re/effects.md, the 856's variant 3): `ARROW2.GRN` in `ARROW2_GOLD`, turning and bobbing,
//! with a helix of `FX_HELIX01` in the record's colour about it, added by its alpha, which climbs
//! and fades in and out, and a light that pulses (`opensacred_rules::particles::Beacon`). The
//! arrow is lit as the world's models are, the helix untinted by the night; neither ends.

use bevy::prelude::*;
use opensacred_rules::particles::{self, Beacon};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture, texture_path};
use crate::game::{AppState, Data};
use crate::npcs::Retexture;
use crate::{daynight, iso, models, npcs};

/// The seed of the helices' angles (**estimate**: any).
const SEED: usize = 0x1846;

/// A gold arrow to set at its entity's place, its helix of the colour `0xAARRGGBB`.
#[derive(Component)]
struct Unset(u32);

/// Sets a gold arrow at `at`, its helix of the colour `colour`.
pub fn spawn(commands: &mut Commands, at: Vec3, colour: u32) {
    commands.spawn((
        Unset(colour),
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// The arrows' model and texture, relative to the asset directory, the helices' material, and
/// the dice.
#[derive(Resource)]
struct Looks {
    model: Option<String>,
    texture: Option<String>,
    helix: Option<Handle<crate::fx::Effect>>,
    dice: Rng,
}

/// A gold arrow that stands: its beacon, its helix's colour, its arrow's model and its helix's
/// mesh.
#[derive(Component)]
struct Set {
    beacon: Beacon,
    colour: [f32; 3],
    arrow: Option<Entity>,
    helix: Handle<Mesh>,
}

/// The arrows' looks of a new game.
fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<crate::fx::Effect>>,
) {
    let Some(data) = data else {
        return;
    };
    let shared = &data.0;
    let model = npcs::model_files(&shared.assets)
        .ok()
        .and_then(|models| models.get(particles::ARROW_MODEL).cloned());
    let texture = texture_path(&shared.assets, particles::ARROW_TEXTURE);
    let helix = particle_texture(&shared.assets, &assets, particles::HELIX_TEXTURE)
        .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
    if model.is_none() || helix.is_none() {
        warn!(
            "beacons: no {} or {}",
            particles::ARROW_MODEL,
            particles::HELIX_TEXTURE
        );
    }
    commands.insert_resource(Looks {
        model,
        texture,
        helix,
        dice: crate::seed::dice(SEED),
    });
}

/// Sets each new arrow: its model a child of it, scaled to the world, and its helix's mesh in
/// the camera that draws after the night's pass (#1191).
fn set(
    mut commands: Commands,
    unset: Query<(Entity, &Unset)>,
    looks: Option<ResMut<Looks>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mut looks) = looks else {
        return;
    };
    for (entity, &Unset(colour)) in &unset {
        let [_, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
        let helix = meshes.add(empty());
        let arrow = looks.model.as_ref().map(|model| {
            let mut arrow = commands.spawn((
                models::scene(&assets, model),
                Transform::from_scale(Vec3::splat(iso::MODEL_SCALE)),
                models::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = &looks.texture {
                arrow.insert(Retexture::load(&assets, texture));
            }
            arrow.id()
        });
        let beacon = Beacon::new(&mut looks.dice);
        let mut set = commands.entity(entity);
        set.remove::<Unset>().insert((
            Set {
                beacon,
                colour: [r, g, b],
                arrow,
                helix: helix.clone(),
            },
            daynight::Light(beacon.light()),
        ));
        if let Some(material) = looks.helix.clone() {
            set.with_child((
                Mesh3d(helix),
                MeshMaterial3d(material),
                Transform::default(),
                bevy::camera::visibility::NoFrustumCulling,
                bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each arrow a frame on: its model turned and bobbing over its place, its light, and its
/// helix's ribbons as quads, the texture along each from end to end.
fn run(
    time: Res<Time>,
    looks: Option<ResMut<Looks>>,
    mut set: Query<(&mut Set, &Transform, &mut daynight::Light)>,
    mut arrows: Query<&mut Transform, Without<Set>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let Some(mut looks) = looks else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    for (mut set, at, mut light) in &mut set {
        set.beacon.update(dt, &mut looks.dice);
        light.0 = set.beacon.light();
        let (turn, height) = set.beacon.arrow();
        if let Some(mut arrow) = set.arrow.and_then(|arrow| arrows.get_mut(arrow).ok()) {
            // The models' frame turns the other way about the world's upright.
            arrow.translation = at.translation + Vec3::Y * height * iso::MODEL_SCALE;
            arrow.rotation = Quat::from_rotation_y(-turn);
        }
        let Some(mut mesh) = meshes.get_mut(&set.helix) else {
            continue;
        };
        let [r, g, b] = set.colour;
        let colour = [r, g, b, set.beacon.alpha()];
        let mut quads = Quads::default();
        ribbon_quads(&mut quads, &set.beacon.helix(), colour);
        // At its object's place in the order, as the original draws the system (#2220).
        quads.in_order(orders.at(at.translation), at.translation);
        quads.into_mesh(&mut mesh);
    }
}

/// Lays a helix's ribbons (`opensacred_rules::particles::helix_ribbons`, `0x762C30`) into
/// `quads` in `colour`, each a strip of quads between its points' pairs from the models' frame
/// about its entity, the texture's `u` along it from 0 to 1 and its `v` across.
pub(crate) fn ribbon_quads(
    quads: &mut Quads,
    ribbons: &[opensacred_rules::particles::Ribbon],
    colour: [f32; 4],
) {
    let world = crate::bolt_looks::world;
    for ribbon in ribbons {
        let last = f32::from(u16::try_from(ribbon.len().saturating_sub(1).max(1)).unwrap_or(1));
        for (k, pair) in (0_u16..).zip(ribbon.windows(2)) {
            let (u0, u1) = (f32::from(k) / last, f32::from(k + 1) / last);
            let ([a0, b0], [a1, b1]) = (pair[0], pair[1]);
            quads.push_corners(
                [world(a0), world(b0), world(b1), world(a1)],
                [
                    Vec2::new(u0, 0.0),
                    Vec2::new(u0, 1.0),
                    Vec2::new(u1, 1.0),
                    Vec2::new(u1, 0.0),
                ],
                colour,
            );
        }
    }
}

/// The scripts' gold arrows.
pub struct BeaconsPlugin;

impl Plugin for BeaconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(
                Update,
                (set, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
