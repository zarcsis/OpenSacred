//! The effects' own lights on the models (#2106; docs/re/effects.md, "The systems' Direct3D
//! lights"): a system that makes a Direct3D point light (`0x642B90` of the kind 2, its handle at
//! `+0xB4`) and moves it to its place at each draw (`0x642EF0`) lights the models about it in its
//! colour, the manager giving every such light the range 200 and the attenuations
//! (1, 0, 0.000125), so `1 / (1 + 0.000125 d²)` within 200 of it; the ground and the sprites,
//! lit by the world's lights alone, take none. Each is a Bevy `PointLight` beside the world's
//! sun, of the same scale: the sun's illuminance the whole colour of a Direct3D light.

use bevy::prelude::*;

use crate::iso;

/// The sun's illuminance (`game::setup`), the world view's white directional light (1, 1, 1),
/// which a Direct3D light of the colour 1 at the attenuation 1 matches.
const SUN: f32 = 4_000.0;

/// Every effect's light's range and its quadratic attenuation (`0x642B90`), in the models' units.
const RANGE: f32 = 200.0;
const QUADRATIC: f32 = 0.000_125;

/// The fireball's (778), the fire bolt's (922), the Dwarf's guns' light: (1, 0.8, 0.4).
pub(crate) const FIRE: [f32; 3] = [1.0, 0.8, 0.4];
/// The lightning strike's (798), the holy light's (804): (0.8, 0.8, 1.0).
pub(crate) const PALE_BLUE: [f32; 3] = [0.8, 0.8, 1.0];
/// The chain lightning's first bolt (799) and its impacts (872): (0.7, 0.7, 1.0).
pub(crate) const ICE_BLUE: [f32; 3] = [0.7, 0.7, 1.0];
/// The lost souls' (850), the dryad ghost's (1162): (0.2, 1, 0.2).
pub(crate) const SOUL_GREEN: [f32; 3] = [0.2, 1.0, 0.2];
/// The thorns' plants' (848), a hero's (0.25, 1, 0.25) and a creature's (0.25, 0.25, 1).
pub(crate) const PLANTS: [[f32; 3]; 2] = [[0.25, 1.0, 0.25], [0.25, 0.25, 1.0]];
/// The ground poison's (792): (0.3, 1, 0.3).
pub(crate) const POISON: [f32; 3] = [0.3, 1.0, 0.3];
/// The flying skull's (905): (1, 0.8, 0.1).
pub(crate) const SKULL: [f32; 3] = [1.0, 0.8, 0.1];
/// The hell disk's (1005), the demon fire's (1015): (1, 0.4, 0.2).
pub(crate) const HELL: [f32; 3] = [1.0, 0.4, 0.2];
/// The torches' flames' (787, `cParticleSystem_torchsmoke`): (1, 1, 0).
pub(crate) const TORCH: [f32; 3] = [1.0, 1.0, 0.0];

/// An effect's light (`0x642B90`): its colour, the look it lives with, and what it follows, else
/// where it stands, plus an offset.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct FxLight {
    pub(crate) colour: [f32; 3],
    pub(crate) owner: Entity,
    pub(crate) follows: Option<Entity>,
    pub(crate) offset: Vec3,
}

/// A light of `colour` at `place` that lives with `owner` and follows `follows` while it is
/// there, `offset` above it, standing where it was last after.
pub(crate) fn light(
    colour: [f32; 3],
    place: Vec3,
    owner: Entity,
    (follows, offset): (Option<Entity>, Vec3),
) -> impl Bundle {
    (
        FxLight {
            colour,
            owner,
            follows,
            offset,
        },
        Transform::from_translation(place + offset),
        Visibility::default(),
        DespawnOnExit(crate::game::AppState::InGame),
    )
}

/// A light of `colour` at its parent `owner`'s place, a child of it; visible, as the Bevy light
/// that [`kindle`] puts under it is (#2201).
pub(crate) fn child(colour: [f32; 3], owner: Entity) -> impl Bundle {
    (
        FxLight {
            colour,
            owner,
            follows: None,
            offset: Vec3::ZERO,
        },
        Transform::IDENTITY,
        Visibility::default(),
        ChildOf(owner),
    )
}

/// How high over its place a Bevy light stands, in the models' units: `1/√0.000125`, 89. Bevy's
/// light falls by the square of the distance alone, so one at this height over the plane of its
/// place falls as `1 / (h² + x²)` along it, `x` the distance in the plane, which is the fixed
/// function's `1 / (1 + 0.000125 x²)` up to its scale: as bright beside the effect as the
/// original's, at none of the inverse square's glare near it; it lights from above rather than
/// from the effect's own place (docs/decisions/drawing.md).
fn lift() -> f32 {
    QUADRATIC.recip().sqrt()
}

/// The lumens of a Bevy point light of the colour's brightest channel `brightest`, at
/// [`lift`] over its place: the sun's illuminance there times `4π h²`.
fn lumens(brightest: f32) -> f32 {
    let s = iso::MODEL_SCALE;
    4.0 * std::f32::consts::PI * SUN * brightest * (lift() * s).powi(2)
}

/// The Bevy light of an effect's light of `colour`, its range reaching 200 along its place's
/// plane.
pub(crate) fn point_light(colour: [f32; 3]) -> PointLight {
    let brightest = colour.iter().copied().fold(0.0_f32, f32::max);
    let [r, g, b] = colour.map(|c| c / brightest.max(f32::EPSILON));
    PointLight {
        color: Color::linear_rgb(r, g, b),
        intensity: lumens(brightest),
        range: RANGE.hypot(lift()) * iso::MODEL_SCALE,
        shadow_maps_enabled: false,
        ..default()
    }
}

/// Gives each new effect's light its Bevy light, a child of it at [`lift`] over it.
fn kindle(mut commands: Commands, lights: Query<(Entity, &FxLight), Added<FxLight>>) {
    for (entity, light) in &lights {
        commands.entity(entity).with_child((
            point_light(light.colour),
            Transform::from_translation(Vec3::Y * lift() * iso::MODEL_SCALE),
        ));
    }
}

/// Moves each light to what it follows (`0x642EF0`), and takes it off with its look (`0x642D90`,
/// the system's destructor).
fn follow(
    mut commands: Commands,
    mut lights: Query<(Entity, &FxLight, &mut Transform)>,
    (places, alive): (Query<&GlobalTransform>, Query<()>),
) {
    for (entity, light, mut transform) in &mut lights {
        if !alive.contains(light.owner) {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(place) = light.follows.and_then(|follows| places.get(follows).ok()) {
            transform.translation = place.translation() + light.offset;
        }
    }
}

/// Gives the Bevy light of each effect's light whose colour changed that colour, as a system
/// sets its Direct3D light's colour at a draw (`0x642880`): the Dwarf's armour's and steel's.
fn recolour(
    lights: Query<(&FxLight, &Children), Changed<FxLight>>,
    mut points: Query<&mut PointLight>,
) {
    for (light, children) in &lights {
        for &child in children {
            if let Ok(mut point) = points.get_mut(child) {
                *point = point_light(light.colour);
            }
        }
    }
}

/// The effects' lights.
pub struct FxLightsPlugin;

impl Plugin for FxLightsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (kindle, follow, recolour)
                .chain()
                .run_if(in_state(crate::game::AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_light_falls_along_its_plane_as_the_fixed_functions() {
        let s = iso::MODEL_SCALE;
        assert!((lift() - 89.44).abs() < 1e-2);
        let point = point_light(FIRE);
        for x in [0.0_f32, 20.0, 89.0, 150.0] {
            let fixed = SUN / (1.0 + QUADRATIC * x * x);
            let bevy =
                point.intensity / (4.0 * std::f32::consts::PI) / (x.hypot(lift()) * s).powi(2);
            assert!((bevy / fixed - 1.0).abs() < 1e-3, "{x}: {bevy} {fixed}");
        }
        assert!((point.range - 200.0_f32.hypot(lift()) * s).abs() < 1e-2);
        assert!(!point.shadow_maps_enabled);
    }

    #[test]
    fn a_light_under_its_owner_is_visible_as_its_bevy_light_is() {
        let mut app = App::new();
        app.add_systems(Update, kindle);
        let owner = app.world_mut().spawn(Visibility::default()).id();
        let light = app.world_mut().spawn(child(FIRE, owner)).id();
        app.update();
        let world = app.world();
        assert!(world.entity(light).contains::<InheritedVisibility>());
        let lit = world
            .entity(light)
            .get::<Children>()
            .map(|children| children.to_vec())
            .unwrap_or_default();
        assert_eq!(lit.len(), 1);
        assert!(world.entity(lit[0]).contains::<PointLight>());
    }
}
