//! The sign of the Dwarf's war cry (1089, 1150, `cParticleSystem_kriegsschrei_sign`; #1092;
//! docs/re/effects.md, "The Dwarf's war cry's sign"): on the hero for the boon's seconds, a blue
//! glow flat at its feet and two opposite fans of blue rays flat about it, going round, the
//! longest leading, flashing as they pass a side; fading in and out over 2 s; the glow added
//! whole, the rays by their alpha, untinted by the night, drawn just behind its model
//! (`fx::Orders`, #2178) (`opensacred_rules::particles::WarCrySign`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    WAR_CRY_BLUE, WAR_CRY_RAY_FROM, WAR_CRY_RAY_HALF, WAR_CRY_SIGN_TEXTURES, WarCrySign,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the sign lies, in the models' units, clear of its triangles.
const GROUND_LIFT: f32 = 0.5;

/// A sign to make: its bearer and its seconds.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    seconds: f32,
}

/// A sign that shows: its bearer, its look and the meshes of its glow and its rays.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: WarCrySign,
    meshes: [Handle<Mesh>; 2],
}

/// The signs' materials, the glow's added whole and the rays' added by their alpha, made as the
/// first asks; `None` where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// Shows the war cry's sign on `bearer` for `seconds`, the boon's (`0x51630A`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32) {
    commands.spawn((Unmade { bearer, seconds }, DespawnOnExit(AppState::InGame)));
}

/// Gives each new sign its meshes and its materials (`0x7B8740`, `0x7B8E10`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let [glow, rays] = WAR_CRY_SIGN_TEXTURES.map(|name| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("war cry signs: no texture {name}; its part is not drawn");
                    }
                    image
                });
                [
                    glow.map(|image| effects.add(whole_material(image))),
                    rays.map(|image| effects.add(particle_material(image, AlphaMode::Add))),
                ]
            })
            .clone();
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look: WarCrySign::new(unmade.seconds),
                meshes: parts.clone(),
            },
            Transform::from_translation(at),
            Visibility::default(),
        ));
        for (mesh, material) in parts.into_iter().zip(materials) {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Runs each sign a frame on (`0x7B88E0`) and draws it (`0x7B89C0`) at its bearer's feet: its glow
/// flat, then each ray flat from 20 out to its end, 8 wide, turned about the upright (the
/// ray's `(x, y)` to `(x·cos a − y·sin a, x·sin a + y·cos a)`), at each of its alphas; it goes
/// once it has faded out, or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let [r, g, b] = WAR_CRY_BLUE;
    let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
    for (entity, mut sign, mut transform) in &mut shown {
        let sign = &mut *sign;
        let Ok(origin) = places.get(sign.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        sign.look.update(dt);
        if sign.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = origin;
        let half = sign.look.glow_half();
        let mut glow = Quads::default();
        glow.push_corners(
            [
                world([-half, -half, GROUND_LIFT]),
                world([half, -half, GROUND_LIFT]),
                world([half, half, GROUND_LIFT]),
                world([-half, half, GROUND_LIFT]),
            ],
            uvs,
            [r, g, b, 1.0],
        );
        let mut rays = Quads::default();
        for ray in sign.look.rays() {
            let (sin, cos) = ray.angle.sin_cos();
            let corner =
                |x: f32, y: f32| world([x * cos - y * sin, x * sin + y * cos, GROUND_LIFT]);
            let w = WAR_CRY_RAY_HALF;
            // The texture's `v` 1 at the inner end, 0 at the outer.
            let corners = [
                corner(-w, ray.outer),
                corner(w, ray.outer),
                corner(w, WAR_CRY_RAY_FROM),
                corner(-w, WAR_CRY_RAY_FROM),
            ];
            for alpha in ray.alphas {
                rays.push_corners(corners, uvs, [r, g, b, alpha]);
            }
        }
        let order = orders.behind(sign.bearer);
        for (handle, mut part) in sign.meshes.iter().zip([glow, rays]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Dwarf's war cry's signs.
pub struct WarCrySignsPlugin;

impl Plugin for WarCrySignsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
