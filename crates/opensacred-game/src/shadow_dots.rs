//! The models' shadow dots (`0x407960`, #2377; docs/re/drawing.md, "The models' shadows"): at the
//! options' detail 0, where the original draws no model's shadow, `cObject3D`'s draw (`0x44B230`)
//! lays five dots of `SHADOWDOT.TGA` flat on the ground under each creature that casts one, in its
//! place among the models: one at its root (`__Root`), one halfway from the root to each foot
//! (`Bip01 R Foot`, `Bip01 L Foot`) and one at each foot, of half sides 0.6, 0.4 and 0.27 times
//! the type's size for the arrows' hits (`items.pak +0x14`, 50 for 0, `0x428CE0`), black at an
//! alpha of 80 (the vertices' colour `0x50000000`) times the texture's. A dot at a foot that the
//! model lacks lies at its root. The chests (`0x5C97B0`) and the arrows (`0x5BAF00`) draw none.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;

use crate::arrows::Size;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::shadows::Casts;

/// The dots' texture, `SHADOWDOT.TGA`.
const TEXTURE: &str = "SHADOWDOT";
/// The dots' alpha, the vertices' `0x50` of 255.
const ALPHA: f32 = 80.0 / 255.0;
/// The root's dot's half side, a share of the type's size, and how much smaller each next pair's
/// is.
const HALF: f32 = 0.6;
const SMALLER: f32 = 2.0 / 3.0;
/// The bones the dots lie under: the root and the right and the left foot.
const ROOT: &str = "__Root";
const FEET: [&str; 2] = ["Bip01 R Foot", "Bip01 L Foot"];

/// A caster's dots: the entity of their mesh, and the mesh.
#[derive(Component)]
struct Dots {
    entity: Entity,
    mesh: Handle<Mesh>,
}

/// The mesh of a caster's dots.
#[derive(Component)]
struct DotsMesh;

/// The dots' material, none where the texture is missing, made as first asked.
#[derive(Resource, Default)]
struct Look(Option<Option<Handle<Effect>>>);

/// Whether the options' detail is the lowest, at which the dots stand for the shadows.
fn lowest(settings: Option<&crate::settings::Settings>) -> bool {
    crate::settings::detail(settings) == 0
}

/// Gives each creature that casts a shadow its dots at the lowest detail.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn make(
    mut commands: Commands,
    casters: Query<
        Entity,
        (
            With<Casts>,
            Without<Dots>,
            Without<crate::npcs::Object>,
            Without<crate::arrows::Arrow>,
        ),
    >,
    settings: Option<Res<crate::settings::Settings>>,
    (data, assets, mut look): (Option<Res<Data>>, Res<AssetServer>, ResMut<Look>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    if !lowest(settings.as_deref()) || casters.is_empty() {
        return;
    }
    let material = look
        .0
        .get_or_insert_with(|| {
            let image = data
                .as_ref()
                .and_then(|data| particle_texture(&data.0.assets, &assets, TEXTURE));
            if image.is_none() {
                warn!("shadow dots: no texture {TEXTURE}; the dots are not drawn");
            }
            image.map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
        })
        .clone();
    let Some(material) = material else {
        return;
    };
    for caster in &casters {
        let mesh = meshes.add(empty());
        let entity = commands
            .spawn((
                DotsMesh,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Hidden,
                NoFrustumCulling,
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        commands.entity(caster).insert(Dots { entity, mesh });
    }
}

/// Takes the dots away above the lowest detail.
fn unmake(
    mut commands: Commands,
    casters: Query<Entity, With<Dots>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    if lowest(settings.as_deref()) {
        return;
    }
    for caster in &casters {
        commands.entity(caster).remove::<Dots>();
    }
}

/// Despawns the mesh of the dots that a caster loses, or loses with it.
fn gone(removed: On<Remove<Dots>>, dots: Query<&Dots>, mut commands: Commands) {
    if let Ok(dots) = dots.get(removed.entity) {
        commands.entity(dots.entity).try_despawn();
    }
}

/// The dots' centres about a root at `root`, its feet at `feet`, flat on the ground, and their
/// half sides for the root's `half`: the root's, the two halfway to the feet, the feet's.
fn laid(root: Vec3, feet: [Vec3; 2], half: f32) -> [(Vec3, f32); 5] {
    let [right, left] = feet;
    let middle = half * SMALLER;
    let small = middle * SMALLER;
    [
        (root, half),
        (root + (right - root) * 0.5, middle),
        (root + (left - root) * 0.5, middle),
        (right, small),
        (left, small),
    ]
}

/// Lays each caster's dots under its root and its feet at its model's place in the order, just
/// behind the model, as the original draws them right before it; hidden with the caster.
fn run(
    casters: Query<(
        Entity,
        &Dots,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&Size>,
    )>,
    mut placed: Query<(&mut Transform, &mut Visibility), With<DotsMesh>>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (caster, dots, at, shown, size) in &casters {
        let Ok((mut transform, mut visibility)) = placed.get_mut(dots.entity) else {
            continue;
        };
        visibility.set_if_neq(if shown.get() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let origin = at.translation();
        // Flat on the ground at the model's height, about its place.
        let flat = |point: Vec3| Vec3::new(point.x - origin.x, 0.0, point.z - origin.z);
        let bone = |name: &str| crate::making::bone(caster, &[name], &children, (&bones, &worn));
        let root = bone(ROOT).map_or(Vec3::ZERO, flat);
        let feet = FEET.map(|foot| bone(foot).map_or(root, flat));
        let half = HALF * size.map_or(crate::arrows::size(0), |size| size.0) * s;
        let mut quads = Quads::default();
        for (centre, half) in laid(root, feet, half) {
            let (a, d) = (across * half, depth * half);
            quads.push_corners(
                [
                    centre - a - d,
                    centre + a - d,
                    centre + a + d,
                    centre - a + d,
                ],
                [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
                [0.0, 0.0, 0.0, ALPHA],
            );
        }
        quads.in_order(orders.behind(caster).or_else(|| orders.at(origin)), origin);
        transform.translation = origin;
        if let Some(mut mesh) = meshes.get_mut(&dots.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The models' shadow dots at the lowest detail.
pub struct ShadowDotsPlugin;

impl Plugin for ShadowDotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Look>().add_observer(gone).add_systems(
            Update,
            (make, unmake, run)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dots_lie_at_the_root_halfway_to_the_feet_and_at_the_feet() {
        let root = Vec3::ZERO;
        let feet = [Vec3::new(6.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -6.0)];
        let dots = laid(root, feet, 30.0);
        let centres = dots.map(|(centre, _)| centre);
        assert_eq!(
            centres,
            [
                root,
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -3.0),
                feet[0],
                feet[1],
            ]
        );
        let halves = dots.map(|(_, half)| half);
        assert!((halves[0] - 30.0).abs() < 1e-5);
        assert!((halves[1] - 20.0).abs() < 1e-5 && (halves[2] - 20.0).abs() < 1e-5);
        assert!((halves[3] - 40.0 / 3.0).abs() < 1e-4);
    }
}
