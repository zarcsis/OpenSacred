//! The auras of the effect 33 (#2767; docs/re/effects.md, "The effect 33's auras"): while a
//! creature holds the effect 33 of a whole value 0 to 5 (a statue's power-ups of the four damage
//! kinds, the purple and the blue potions), the model's draw (`0x44A9D0`) draws over its head
//! (`0x40A690`) two camera-facing quads of `FX_AURA01`, a ring of rays, in the whole value's
//! colour, pulsing and turning slowly against each other by the real time, added whole, untinted
//! by the night; each whole value held its own pair (`opensacred_rules::particles::{power_aura,
//! aura_quads}`). The quads lie where the head's texels lie, drawn after the model and depth
//! tested against it (`fx::Orders::over`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::fight::effects::DAMAGE_OF;
use opensacred_rules::particles::{self, AURA_BONE, AURA_RISE, AURA_TEXTURE};

use crate::effects::Effects;
use crate::frame::Playing;
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// An aura's look: its bearer and its quads' mesh.
#[derive(Component)]
struct Aura {
    bearer: Entity,
    mesh: Handle<Mesh>,
}

/// The entity of a bearer's aura.
#[derive(Component)]
struct Bears(Entity);

/// The auras' material, made as the first shows; none where its texture is missing.
#[derive(Resource, Default)]
struct Made(Option<Option<Handle<Effect>>>);

/// The colours of the auras that `effects` puts over its bearer's head: one for each whole value
/// of its effects 33, as each sets its bit of `+0xB0` once, in the order of the bits.
fn colours(effects: &Effects) -> Vec<u32> {
    let mut wholes: Vec<i32> = effects
        .timed()
        .iter()
        .filter(|timed| timed.effect.id == DAMAGE_OF)
        .map(|timed| timed.effect.whole)
        .collect();
    wholes.sort_unstable();
    wholes.dedup();
    wholes
        .into_iter()
        .filter_map(particles::power_aura)
        .collect()
}

/// Gives each creature that comes to hold an aura its look, and takes it from one that no
/// longer does.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    bearers: Query<(Entity, &Effects, Option<&Bears>), Changed<Effects>>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Made>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (bearer, effects, bears) in &bearers {
        let wanted = !colours(effects).is_empty();
        match (wanted, bears) {
            (true, None) => {
                let material = made
                    .0
                    .get_or_insert_with(|| {
                        let image = data.as_ref().and_then(|data| {
                            particle_texture(&data.0.assets, &assets, AURA_TEXTURE)
                        });
                        if image.is_none() {
                            warn!(
                                "power auras: no texture {AURA_TEXTURE}; the auras are not drawn"
                            );
                        }
                        image.map(|image| materials.add(whole_material(image)))
                    })
                    .clone();
                let mesh = meshes.add(empty());
                let mut aura = commands.spawn((
                    Transform::IDENTITY,
                    Visibility::default(),
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(material) = material {
                    aura.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
                }
                let aura = aura.insert(Aura { bearer, mesh }).id();
                commands.entity(bearer).insert(Bears(aura));
                debug!("power auras: {bearer} shows its aura");
            }
            (false, Some(&Bears(aura))) => {
                commands.entity(aura).despawn();
                commands.entity(bearer).remove::<Bears>();
                debug!("power auras: {bearer}'s aura goes");
            }
            _ => {}
        }
    }
}

/// Lays each aura's quads over its bearer's head at the real time, where the head's texels lie
/// in the order; none while the bearer is hidden, and the look goes with its bearer.
fn keep(
    mut commands: Commands,
    mut auras: Query<(Entity, &Aura, &mut Transform)>,
    bearers: Query<(&Effects, &GlobalTransform, Option<&InheritedVisibility>)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, orders, time): (ResMut<Assets<Mesh>>, Orders, Res<Time<Real>>),
) {
    let millis = u64::try_from(time.elapsed().as_millis()).unwrap_or(0);
    let pair = particles::aura_quads(millis);
    for (aura, look, mut at) in &mut auras {
        let Ok((effects, root, shown)) = bearers.get(look.bearer) else {
            commands.entity(aura).despawn();
            continue;
        };
        let root = root.translation();
        at.translation = root;
        let mut quads = Quads::default();
        let head = crate::making::bone(look.bearer, &[AURA_BONE], &children, (&bones, &worn));
        if let Some(head) = head.filter(|_| shown.is_none_or(|shown| shown.get())) {
            let centre = head + Vec3::Y * AURA_RISE * iso::MODEL_SCALE - root;
            for colour in colours(effects) {
                let [a, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
                for (half, angle) in pair {
                    quads.push_facing_turned(
                        centre,
                        half * iso::MODEL_SCALE,
                        angle,
                        [[r, g, b, a]; 4],
                    );
                }
            }
            quads.as_drawn(orders.over(look.bearer), root);
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The auras of the effect 33.
pub struct PowerAurasPlugin;

impl Plugin for PowerAurasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Made>().add_systems(
            Update,
            (show, keep)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use opensacred_rules::fight::Effect;

    use super::*;

    #[test]
    fn each_whole_value_of_the_effect_33_shows_one_aura() {
        let mut effects = Effects::default();
        assert!(colours(&effects).is_empty());
        // A blue potion's, a statue's magic power-up twice, and another effect.
        for (id, whole) in [(DAMAGE_OF, 5), (DAMAGE_OF, 2), (DAMAGE_OF, 2), (30, 2)] {
            effects.add(
                Effect {
                    whole,
                    ..Effect::new(id, 0.5)
                },
                60.0,
            );
        }
        assert_eq!(colours(&effects), [0xFF00_3000, 0xFFFF_8030]);
    }
}
