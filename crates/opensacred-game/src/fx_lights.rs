//! The effects' own lights on the models (#2106; docs/re/effects.md, "The systems' Direct3D
//! lights"): a system that makes a Direct3D point light (`0x642B90` of the kind 2, its handle at
//! `+0xB4`) and moves it to its place at each draw (`0x642EF0`) lights the models about it in its
//! colour, the manager giving every such light the range 200 and the attenuations
//! (1, 0, 0.000125), so `1 / (1 + 0.000125 d²)` within 200 of it; the ground and the sprites,
//! lit by the world's lights alone, take none. The models' shader sums them from their own places
//! as the fixed function does (`models::Anchors::effect_lights`, `standard.wesl`, #3692).
//!
//! The manager has as many slots as the device's lights, 8, the sun's among them, and a light
//! made with every slot taken goes into the first slot of the lowest priority where its own is at
//! least as high, putting that light out for good, else it shines nowhere (#2282): so at most
//! seven effects' lights shine at once, as the game keeps them ([`Slots`]), and one hidden with its
//! owner shines none (#2144, #2201, #3672).

use bevy::prelude::*;

use crate::models::{Anchors, EFFECT_LIGHTS};

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

/// The manager's priorities of the effects' lights (`0x642B90`'s last argument; the table of
/// docs/re/effects.md, "The systems' Direct3D lights"): the higher most systems', the lower the
/// others'; the world view's sun holds 4, which none of them reaches.
pub(crate) const HIGH: u8 = 2;
pub(crate) const LOW: u8 = 1;

/// The device's lights, the manager's slots (`cDX7LightMgr`'s `+0x04`, the count with hardware
/// transform and lighting; 8 seen 2026-11-08 by a probe of `0x642B90` in the original), and those
/// of them left to the effects beside the sun's.
const DEVICE_LIGHTS: usize = 8;
const SLOTS: usize = DEVICE_LIGHTS - 1;
const _: () = assert!(SLOTS == EFFECT_LIGHTS);

/// An effect's light (`0x642B90`): its colour, the look it lives with, what it follows, else
/// where it stands, plus an offset, and its priority among the manager's slots ([`Slots`]).
/// It holds a `Visibility` of its own whoever spawns it, so that it hides with its owner and
/// shines none meanwhile (#2144, #2201, #3672).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Transform, Visibility)]
pub(crate) struct FxLight {
    pub(crate) colour: [f32; 3],
    pub(crate) owner: Entity,
    pub(crate) follows: Option<Entity>,
    pub(crate) offset: Vec3,
    pub(crate) priority: u8,
}

/// An effect's light that the manager put out or never lit, its slots taken ([`Slots`]): it
/// shines no more while it lives.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dark;

/// The manager's slots of the effects' lights, beside the sun's (`0x642B90`): the light in each
/// and its priority.
#[derive(Resource, Debug, Default)]
pub(crate) struct Slots([Option<(Entity, u8)>; SLOTS]);

/// The slot that a light of `priority` takes among `slots`, the priorities of the lights in them
/// (`0x642B90`): the first free one; with none, the first of the lowest priority where `priority`
/// is at least as high, whether it puts a light out; else none.
fn slot_for(slots: &[Option<u8>], priority: u8) -> Option<(usize, bool)> {
    if let Some(free) = slots.iter().position(Option::is_none) {
        return Some((free, false));
    }
    let (lowest, at) = slots
        .iter()
        .enumerate()
        .filter_map(|(at, slot)| slot.map(|held| (held, at)))
        .fold(None, |best: Option<(u8, usize)>, (held, at)| match best {
            Some((low, _)) if held >= low => best,
            _ => Some((held, at)),
        })?;
    (priority >= lowest).then_some((at, true))
}

/// A light of `colour` and `priority` at `place` that lives with `owner` and follows `follows`
/// while it is there, `offset` above it, standing where it was last after.
pub(crate) fn light(
    colour: [f32; 3],
    priority: u8,
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
            priority,
        },
        Transform::from_translation(place + offset),
        Visibility::default(),
        DespawnOnExit(crate::game::AppState::InGame),
    )
}

/// A light of `colour` and `priority` at its parent `owner`'s place, a child of it, visible with
/// it (#2201).
pub(crate) fn child(colour: [f32; 3], priority: u8, owner: Entity) -> impl Bundle {
    (
        FxLight {
            colour,
            owner,
            follows: None,
            offset: Vec3::ZERO,
            priority,
        },
        Transform::IDENTITY,
        Visibility::default(),
        ChildOf(owner),
    )
}

/// Gives each new effect's light a slot of the manager's; one that takes another's slot puts that
/// one out, and one that takes none stays dark (`0x642B90`, [`slot_for`]).
fn kindle(
    mut commands: Commands,
    mut slots: ResMut<Slots>,
    lights: Query<(Entity, &FxLight), Added<FxLight>>,
) {
    let mut added: Vec<(Entity, u8)> = lights
        .iter()
        .map(|(entity, light)| (entity, light.priority))
        .collect();
    added.sort_by_key(|&(entity, _)| entity);
    for (entity, priority) in added {
        let held = slots.0.map(|slot| slot.map(|(_, held)| held));
        let Some((at, puts_out)) = slot_for(&held, priority) else {
            debug!("fx lights: {entity} finds no slot");
            commands.entity(entity).insert(Dark);
            continue;
        };
        if puts_out && let Some((old, _)) = slots.0[at] {
            debug!("fx lights: {entity} puts {old} out");
            commands.entity(old).try_insert(Dark);
        }
        slots.0[at] = Some((entity, priority));
    }
}

/// Frees the slot of each effect's light gone (`0x642D90`).
fn release(mut slots: ResMut<Slots>, mut gone: RemovedComponents<FxLight>) {
    for entity in gone.read() {
        for slot in &mut slots.0 {
            if slot.is_some_and(|(held, _)| held == entity) {
                *slot = None;
            }
        }
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

/// Hands the models' shader the effects' lights that shine: those that hold a slot, in the slots'
/// order, each at its place in its colour, but one hidden with its owner (#2144, #2201, #3672).
/// The colour is the light's as its system last set it (`0x642880`): the Dwarf's armour's and
/// steel's change theirs.
fn shine(
    slots: Res<Slots>,
    lights: Query<(&FxLight, &GlobalTransform, &InheritedVisibility)>,
    mut anchors: ResMut<Anchors>,
) {
    anchors.effect_lights(slots.0.iter().flatten().filter_map(|&(entity, _)| {
        let (light, place, visible) = lights.get(entity).ok()?;
        visible.get().then(|| (place.translation(), light.colour))
    }));
}

/// The effects' lights.
pub struct FxLightsPlugin;

impl Plugin for FxLightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Slots>().add_systems(
            Update,
            (release, kindle, follow, shine)
                .chain()
                .before(crate::frame::Step::Models)
                .run_if(in_state(crate::game::AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `light` holds a slot of the manager's.
    fn lit(app: &App, light: Entity) -> bool {
        app.world()
            .resource::<Slots>()
            .0
            .iter()
            .flatten()
            .any(|&(held, _)| held == light)
    }

    #[test]
    fn a_light_in_a_slot_shines_at_its_place_while_it_is_visible() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<bevy::render::storage::ShaderBuffer>()
            .init_resource::<Anchors>()
            .init_resource::<Slots>()
            .add_systems(Update, (kindle, shine).chain());
        let owner = app.world_mut().spawn(Visibility::default()).id();
        let place = Vec3::new(1.0, 2.0, 3.0);
        let light = app
            .world_mut()
            .spawn((
                child(FIRE, HIGH, owner),
                GlobalTransform::from_translation(place),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.update();
        assert!(lit(&app, light));
        let mut shown = Anchors::from_world(app.world_mut());
        shown.effect_lights([(place, FIRE)]);
        assert_eq!(app.world().resource::<Anchors>().effects(), shown.effects());
        // Hidden with its owner: none.
        app.world_mut()
            .entity_mut(light)
            .insert(InheritedVisibility::HIDDEN);
        app.update();
        shown.effect_lights([]);
        assert_eq!(app.world().resource::<Anchors>().effects(), shown.effects());
    }

    #[test]
    fn a_light_spawned_bare_holds_a_visibility_of_its_own() {
        // As a torch's loop makes its light: the component alone under its flame (#3672).
        let mut app = App::new();
        app.init_resource::<Slots>().add_systems(Update, kindle);
        let owner = app.world_mut().spawn(Visibility::Hidden).id();
        let light = app
            .world_mut()
            .spawn((
                FxLight {
                    colour: TORCH,
                    owner,
                    follows: None,
                    offset: Vec3::ZERO,
                    priority: HIGH,
                },
                ChildOf(owner),
            ))
            .id();
        app.update();
        let world = app.world();
        let entity = world.entity(light);
        assert_eq!(entity.get::<Visibility>(), Some(&Visibility::Inherited));
        assert!(entity.contains::<InheritedVisibility>() && entity.contains::<Transform>());
        assert!(lit(&app, light));
    }

    #[test]
    fn a_light_takes_a_free_slot_else_the_first_of_the_lowest_priority() {
        assert_eq!(slot_for(&[Some(2), None, None], LOW), Some((1, false)));
        // Full: the first of the lowest, for one at least as high; none for a lower one.
        assert_eq!(slot_for(&[Some(2), Some(1), Some(1)], LOW), Some((1, true)));
        assert_eq!(
            slot_for(&[Some(2), Some(1), Some(2)], HIGH),
            Some((1, true))
        );
        assert_eq!(slot_for(&[Some(2), Some(2)], LOW), None);
    }

    #[test]
    fn at_most_seven_effects_lights_shine_and_a_higher_one_puts_out_a_lower() {
        let mut app = App::new();
        app.init_resource::<Slots>()
            .add_systems(Update, (release, kindle).chain());
        let owner = app.world_mut().spawn(Visibility::default()).id();
        let lows: Vec<Entity> = (0..9)
            .map(|_| app.world_mut().spawn(child(FIRE, LOW, owner)).id())
            .collect();
        app.update();
        // Seven lit, each later low one of a full manager putting out the first low one's.
        assert_eq!(lows.iter().filter(|&&l| lit(&app, l)).count(), SLOTS);
        let high = app.world_mut().spawn(child(FIRE, HIGH, owner)).id();
        app.update();
        assert!(lit(&app, high));
        assert_eq!(lows.iter().filter(|&&l| lit(&app, l)).count(), SLOTS - 1);
        // A light gone frees its slot.
        let first_lit = *lows.iter().find(|&&l| lit(&app, l)).expect("a lit light");
        app.world_mut().entity_mut(first_lit).despawn();
        app.update();
        let next = app.world_mut().spawn(child(FIRE, LOW, owner)).id();
        app.update();
        assert!(lit(&app, next));
    }
}
