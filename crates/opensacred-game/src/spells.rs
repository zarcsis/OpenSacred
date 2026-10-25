//! What the hero's spells do as they act (#872, #873; docs/re/arts.md, "The spells' cast", "The
//! objects' actions", "The Mage's spells"): `making` casts a spell and, once the share of its
//! clip still to play falls below the spell's, sends [`Acts`]. The fireball leaves for its
//! target's place as a bolt (`bolts`); the regeneration gives its bearer its record's effects
//! (`opensacred_rules::spells::effects`) at once, and the stone skin as its shield's clock passes
//! [`SHIELD_TIME`] at the cast's rate, and the crusader's strength after [`STRENGTH_TIME`]; the
//! holy shield turns arrows aside (`arrows`); the irritation and the conversion hold a hostile
//! creature with the effect 13, under which it leaves the hero alone (`ai`), and 14, which turns
//! its sides over (`effects`); the healing adds its health to its bearer's, its most at most. A spell that lasts on its bearer gives it the effect `0x22` too. The Seraphim's
//! lightning strikes its target at once; her holy light stands at its point, and her blades of
//! light and her light go with their bearer, each striking about itself once a second for its
//! time ([`Aura`]). Their particle systems are stood in for by a flash and translucent discs
//! (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::fight::Effect;
use opensacred_rules::spells::{
    self, BEAT, BORNE, CONVERSION, CRUSADERS_STRENGTH, FIRE_SKIN, FIREBALL, HEALING, HOLY_SHIELD,
    IRRITATION, LIGHTNING, LIGHTNING_SHOWS, QUICKENING, REIKI, SHIELD_TIME, SHIELD_WALL, SPRING,
    STONE_SKIN, STRENGTH_TIME, WALL_TIME,
};

use crate::crowd::InTheWay;
use crate::effects::Effects;
use crate::fight::{self, Combat, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::Level;
use crate::{ai, arrows};

/// A spell acting: its caster, the spell, its target (the caster for one on itself), the point
/// it was aimed at, and its level in play.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Acts {
    pub caster: Entity,
    pub spell: u16,
    pub target: Entity,
    pub point: Vec3,
    pub level: u32,
}

/// A spell's effects waiting for its object's clock, the stone skin's or the crusader's
/// strength's: what is left of it in the world's seconds.
#[derive(Component, Debug, Clone)]
struct Shielding {
    left: f32,
    effects: Vec<(Effect, f32)>,
}

/// An object that strikes about itself once a [`BEAT`] for its time (`opensacred_rules::spells::
/// area`): its caster, the bearer it follows, if any, its reach, the classes it strikes alone,
/// its damages, the level it strikes on, its time left and its cooldown.
#[derive(Component, Debug, Clone)]
struct Aura {
    caster: Entity,
    follows: Option<Entity>,
    reach: f32,
    classes: &'static [u8],
    damages: [f32; 4],
    level: Level,
    left: f32,
    cooldown: f32,
}

/// A sight that goes after its time.
#[derive(Component, Debug, Clone, Copy)]
struct Fades(f32);

/// The Dwarf's sign of his mines, going with him: its caster, the mines it still carries, its
/// cooldown, the mines' damages and its level.
#[derive(Component, Debug, Clone)]
struct MineSign {
    caster: Entity,
    left: u32,
    cooldown: f32,
    damages: [f32; 4],
    level: Level,
}

/// A mine lying where the sign laid it: its caster, its damages, its level and its time left.
#[derive(Component, Debug, Clone)]
struct Mine {
    caster: Entity,
    damages: [f32; 4],
    level: Level,
    left: f32,
}

/// The stand-ins' meshes and materials: a disc of radius 1 and the holy light's, the blades of
/// light's, the light's, the fire skin's and the gust's; a column of light and the lightning's;
/// a mine and its blast.
#[derive(Resource)]
struct Looks {
    disc: Handle<Mesh>,
    discs: [Handle<StandardMaterial>; 5],
    column: Handle<Mesh>,
    lightning: Handle<StandardMaterial>,
    mine: (Handle<Mesh>, Handle<StandardMaterial>),
    blast: (Handle<Mesh>, Handle<StandardMaterial>),
}

/// Makes the stand-ins' looks (**estimate**: the colours of their glows).
fn make_looks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let mut glow = |colour: Color| {
        materials.add(StandardMaterial {
            base_color: colour,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })
    };
    let discs = [
        glow(Color::srgba(1.0, 0.85, 0.4, 0.35)),
        glow(Color::srgba(0.75, 0.85, 1.0, 0.3)),
        glow(Color::srgba(1.0, 1.0, 0.75, 0.2)),
        glow(Color::srgba(1.0, 0.45, 0.1, 0.3)),
        glow(Color::srgba(0.85, 0.9, 0.85, 0.25)),
    ];
    let lightning = glow(Color::srgba(0.8, 0.9, 1.0, 0.8));
    let mine = (
        meshes.add(Cylinder::new(14.0, 6.0)),
        glow(Color::srgba(0.25, 0.22, 0.2, 1.0)),
    );
    let blast = (
        meshes.add(Sphere::new(spells::BLAST_REACH)),
        glow(Color::srgba(1.0, 0.6, 0.2, 0.45)),
    );
    commands.insert_resource(Looks {
        disc: meshes.add(Cylinder::new(1.0, 1.0)),
        discs,
        column: meshes.add(Cylinder::new(8.0, 400.0)),
        lightning,
        mine,
        blast,
    });
}

/// Carries out each spell that acts.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its resources grouped; \
              the spells read best together"
)]
fn act(
    mut commands: Commands,
    mut acts: MessageReader<Acts>,
    (arts, rules, looks, experience): (
        Option<Res<crate::arts::HeroArts>>,
        Res<crate::fight::Rules>,
        Option<Res<Looks>>,
        Res<fight::Experience>,
    ),
    casters: Query<(&Transform, &Body, Option<&Combat>)>,
    places: Query<&GlobalTransform>,
    mut targets: Query<(&mut Health, Option<&ai::Mind>, Option<&fight::Ranked>)>,
    mut launches: MessageWriter<crate::bolts::Launch>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let Some(arts) = arts else {
        acts.clear();
        return;
    };
    for act in acts.read() {
        let Some(record) = arts.tables.spells.iter().find(|s| s.id == act.spell) else {
            warn!("spells: no record {} of the first table", act.spell);
            continue;
        };
        let Ok((at, body, combat)) = casters.get(act.caster) else {
            continue;
        };
        let here = at.translation;
        let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
        let level = i32::try_from(act.level).unwrap_or(1);
        let mut given = Vec::new();
        if spells::lasts(act.spell) {
            given.push(spells::lasting(record, act.level));
        }
        match act.spell {
            FIREBALL => {
                // For its target's place, or the point where it is gone (`0x522480`).
                let toward = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let direction = (toward - here)
                    .xz()
                    .try_normalize()
                    .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
                let damages = bolt_damages(record, level, multipliers);
                info!("spells: the fireball leaves for {toward} with {damages:?}");
                launches.write(crate::bolts::Launch {
                    shooter: act.caster,
                    direction,
                    course: crate::bolts::Course::fireball(act.level),
                    damages,
                    wait: 0.0,
                });
            }
            REIKI | SPRING => given.extend(spells::effects(record, act.level)),
            spells::MINES => {
                // His sign, with the mines' count of its values' ratio; their damages by his
                // weapons' multipliers, the second array (`0x528995`).
                let damages = bolt_damages(
                    record,
                    level,
                    combat.map_or([1.0; 4], |combat| combat.multipliers),
                );
                commands.spawn((
                    MineSign {
                        caster: act.caster,
                        left: spells::ratio(record, 0, 1, act.level),
                        cooldown: 0.0,
                        damages,
                        level: body.level(),
                    },
                    Transform::from_translation(here),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            // The Dwarf's: the greed's and the steel's effects at once; the burrowing's and the
            // armour's with their action 6, the armour's object's effect 35 too.
            spells::GREED | spells::STEEL => given.extend(spells::dwarfs(record, act.level)),
            spells::BURROWING | spells::ARMOUR => {
                given.extend(spells::dwarfs(record, act.level));
                given.extend(spells::effects(record, act.level));
                if act.spell == spells::ARMOUR {
                    let effect = Effect {
                        id: BORNE,
                        whole: 0,
                        value: 0.0,
                        art: u32::from(act.spell),
                        level: 0,
                    };
                    given.push((effect, fight_seconds(spells::value(record, 0, act.level))));
                }
            }
            QUICKENING => {
                // As the stone skin's, its clock at the cast's rate (`0x7883C0`).
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.target).insert(Shielding {
                    left: SHIELD_TIME / rate.max(f32::EPSILON),
                    effects: spells::effects(record, act.level),
                });
            }
            SHIELD_WALL => {
                given.extend(spells::turning(record, act.level));
                commands.entity(act.target).insert(Shielding {
                    left: WALL_TIME,
                    effects: spells::effects(record, act.level),
                });
            }
            HOLY_SHIELD => given.push(spells::holy_shield(record, act.level)),
            IRRITATION | CONVERSION => {
                // On a hostile creature alive, no boss nor champion, nor of a big type for the
                // conversion (`0x524196`, `0x5242D3`, the action 7); the effect's own refusal of
                // `+0x2B8` bit `0x40` is not followed.
                let Ok((health, Some(mind), rank)) = targets.get(act.target) else {
                    continue;
                };
                let ranked = rank.map(|rank| rank.0);
                let held = !matches!(
                    ranked,
                    Some(
                        opensacred_rules::fight::Rank::Boss
                            | opensacred_rules::fight::Rank::Champion
                    )
                );
                if !mind.hostile()
                    || health.current == 0
                    || !held
                    || (act.spell == CONVERSION && mind.big)
                {
                    info!("spells: {} does not hold {}", act.spell, act.target);
                    continue;
                }
                given.extend(spells::holding(
                    record,
                    act.level,
                    experience.level,
                    health.level,
                ));
            }
            CRUSADERS_STRENGTH => {
                commands.entity(act.target).insert(Shielding {
                    left: STRENGTH_TIME,
                    effects: spells::effects(record, act.level),
                });
            }
            STONE_SKIN => {
                // The shield's clock runs at the cast's rate (`+0x3E`).
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.target).insert(Shielding {
                    left: SHIELD_TIME / rate.max(f32::EPSILON),
                    effects: spells::effects(record, act.level),
                });
            }
            HEALING => {
                let lore = opensacred_rules::fight::magic_lore(&arts.skills, &rules);
                let health = spells::healing(record, act.level, lore);
                if let Ok((mut bearer, ..)) = targets.get_mut(act.target) {
                    bearer.current = bearer.current.saturating_add(health).min(bearer.most);
                    info!("spells: the healing gives {health}, to {}", bearer.current);
                }
            }
            LIGHTNING => {
                // Its object at its target strikes it alone at its first frame, of the record's
                // damages by the caster's spells' multipliers (`0x5B5069`).
                let Ok(place) = places.get(act.target) else {
                    continue;
                };
                let damages = bolt_damages(record, level, multipliers);
                info!(
                    "spells: the lightning strikes {} for {damages:?}",
                    act.target
                );
                hits.write(fight::Hit {
                    attacker: act.caster,
                    target: act.target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                });
                if let Some(looks) = &looks {
                    commands.spawn((
                        Mesh3d(looks.column.clone()),
                        MeshMaterial3d(looks.lightning.clone()),
                        Transform::from_translation(place.translation() + Vec3::Y * 200.0),
                        Fades(LIGHTNING_SHOWS),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            spell => {
                let Some(area) = spells::area(spell) else {
                    continue;
                };
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let follows = area.follows.then_some(act.target);
                let centre = match follows {
                    Some(bearer) => places.get(bearer).map_or(here, |at| at.translation()),
                    None => act.point,
                };
                let damages = bolt_damages(record, level, multipliers);
                info!(
                    "spells: {spell} strikes within {} for {seconds} s with {damages:?}",
                    area.reach
                );
                let aura = Aura {
                    caster: act.caster,
                    follows,
                    reach: area.reach,
                    classes: area.classes,
                    damages,
                    level: body.level(),
                    left: seconds,
                    cooldown: 0.0,
                };
                let mut made = commands.spawn((
                    aura,
                    Transform::from_translation(centre),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    let look = match spell {
                        spells::HOLY_LIGHT => 0,
                        spells::BLADES_OF_LIGHT => 1,
                        FIRE_SKIN => 3,
                        spells::GUST => 4,
                        _ => 2,
                    };
                    made.with_child((
                        Mesh3d(looks.disc.clone()),
                        MeshMaterial3d(looks.discs[look].clone()),
                        Transform::from_scale(Vec3::new(area.reach, 2.0, area.reach))
                            .with_translation(Vec3::Y * 4.0),
                    ));
                }
                // The fire skin's effects of its record at once, and the turning aside of the
                // fire skin and the gust.
                given.extend(spells::turning(record, act.level));
                if spell == FIRE_SKIN {
                    given.extend(spells::effects(record, act.level));
                }
                if spells::borne(spell) {
                    let effect = Effect {
                        id: BORNE,
                        whole: 0,
                        value: 0.0,
                        art: u32::from(spell),
                        level: 0,
                    };
                    given.push((effect, seconds));
                }
            }
        }
        give(&mut commands, act.target, given);
    }
}

/// A whole number of seconds as a float.
fn fight_seconds(seconds: u32) -> f32 {
    u16::try_from(seconds).map_or(f32::MAX, f32::from)
}

/// Runs each object that strikes about itself: it goes with its bearer, ends with its time or
/// its bearer, and once its cooldown is over strikes the creatures on its level that are
/// hostile to the hero, alive, of its classes where it names some, and nearer it than its reach
/// and their size, each with the fight's sure strike of its damages, then waits a [`BEAT`]
/// (`0x5A85A8`, `0x5AB490`, `0x5ABD80`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn strike(
    mut commands: Commands,
    time: Res<Time>,
    mut auras: Query<(Entity, &mut Aura, &mut Transform)>,
    bearers: Query<&GlobalTransform, (Without<Aura>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Aura>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut aura, mut transform) in &mut auras {
        aura.left -= dt;
        let bearer = aura.follows.map(|bearer| bearers.get(bearer));
        if aura.left <= 0.0 || bearer.as_ref().is_some_and(Result::is_err) {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(Ok(bearer)) = bearer {
            transform.translation = bearer.translation();
        }
        aura.cooldown = (aura.cooldown - dt).max(0.0);
        if aura.cooldown > 0.0 {
            continue;
        }
        let centre = transform.translation.xz();
        let struck: Vec<Entity> = creatures
            .iter()
            .filter(|(_, at, way, mind, health, size)| {
                way.creature
                    && way.level == aura.level
                    && mind.hostile()
                    && health.current > 0
                    && (aura.classes.is_empty() || aura.classes.contains(&mind.class))
                    && (at.translation().xz() - centre).length()
                        < aura.reach + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|(target, ..)| target)
            .collect();
        if struck.is_empty() {
            continue;
        }
        for target in struck {
            debug!("spells: {entity} strikes {target} for {:?}", aura.damages);
            hits.write(fight::Hit {
                attacker: aura.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(aura.damages),
                sure: true,
            });
        }
        aura.cooldown = BEAT;
    }
}

/// Each sign of mines goes with its Dwarf and lays a mine where it stands once a second, unless
/// another lies within [`spells::MINES_APART`], until it carries none (`0x5B0719`); each mine goes
/// off as a hostile creature comes within its reach while its Dwarf is gone or stands
/// [`spells::MINE_SAFE`] or more off, or after [`spells::MINE_LIFE`] by itself, striking once the
/// hostile creatures within [`spells::BLAST_REACH`] and their size (`0x5B094A`, the object 794).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn mine(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    mut signs: Query<(Entity, &mut MineSign, &mut Transform), Without<Mine>>,
    mut mines: Query<(Entity, &mut Mine, &Transform), Without<MineSign>>,
    casters: Query<&GlobalTransform, (With<Body>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    let laid: Vec<Vec3> = mines.iter().map(|(_, _, at)| at.translation).collect();
    for (entity, mut sign, mut transform) in &mut signs {
        let Ok(caster) = casters.get(sign.caster) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = caster.translation();
        sign.cooldown = (sign.cooldown - dt).max(0.0);
        let here = transform.translation;
        let near = laid
            .iter()
            .any(|at| (*at - here).xz().length() < spells::MINES_APART);
        if sign.cooldown > 0.0 || near {
            continue;
        }
        let mut made = commands.spawn((
            Mine {
                caster: sign.caster,
                damages: sign.damages,
                level: sign.level,
                left: spells::MINE_LIFE,
            },
            Transform::from_translation(here),
            Visibility::Inherited,
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(looks) = &looks {
            made.with_child((
                Mesh3d(looks.mine.0.clone()),
                MeshMaterial3d(looks.mine.1.clone()),
            ));
        }
        sign.left = sign.left.saturating_sub(1);
        sign.cooldown = BEAT;
        if sign.left == 0 {
            commands.entity(entity).despawn();
        }
    }
    // The creatures on `level`, alive and hostile to the hero, nearer `centre` than `reach` and
    // their size.
    let near = |level: Level, centre: Vec2, reach: f32| {
        creatures
            .iter()
            .filter(move |(_, at, way, mind, health, size)| {
                way.creature
                    && way.level == level
                    && mind.hostile()
                    && health.current > 0
                    && (at.translation().xz() - centre).length()
                        < reach + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|(target, ..)| target)
    };
    for (entity, mut mine, at) in &mut mines {
        mine.left -= dt;
        let centre = at.translation.xz();
        let away = casters.get(mine.caster).map_or(true, |caster| {
            (caster.translation().xz() - centre).length() >= spells::MINE_SAFE
        });
        let touched = away
            && near(mine.level, centre, spells::MINE_REACH)
                .next()
                .is_some();
        if !touched && mine.left > 0.0 {
            continue;
        }
        debug!("spells: a mine of {} goes off", mine.caster);
        for target in near(mine.level, centre, spells::BLAST_REACH) {
            hits.write(fight::Hit {
                attacker: mine.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(mine.damages),
                sure: true,
            });
        }
        if let Some(looks) = &looks {
            commands.spawn((
                Mesh3d(looks.blast.0.clone()),
                MeshMaterial3d(looks.blast.1.clone()),
                Transform::from_translation(at.translation),
                Fades(0.3),
                DespawnOnExit(AppState::InGame),
            ));
        }
        commands.entity(entity).despawn();
    }
}

/// Takes each sight away after its time.
fn fade(time: Res<Time>, mut commands: Commands, mut sights: Query<(Entity, &mut Fades)>) {
    for (entity, mut fades) in &mut sights {
        fades.0 -= time.delta_secs();
        if fades.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Gives `bearer` the effects `given`, each for its seconds.
fn give(commands: &mut Commands, bearer: Entity, given: Vec<(Effect, f32)>) {
    if given.is_empty() {
        return;
    }
    info!("spells: {bearer} takes {given:?}");
    commands
        .entity(bearer)
        .entry::<Effects>()
        .or_default()
        .and_modify(move |mut effects| {
            for (effect, seconds) in given {
                effects.add(effect, seconds);
            }
        });
}

/// Runs each stone skin's shield and gives its effects as its clock passes (its system's update
/// and the action 6).
fn shield(time: Res<Time>, mut commands: Commands, mut shields: Query<(Entity, &mut Shielding)>) {
    for (bearer, mut shielding) in &mut shields {
        shielding.left -= time.delta_secs();
        if shielding.left > 0.0 {
            continue;
        }
        let given = std::mem::take(&mut shielding.effects);
        commands.entity(bearer).remove::<Shielding>();
        give(&mut commands, bearer, given);
    }
}

/// The spells' acts.
pub struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Acts>()
            .add_systems(Startup, make_looks)
            .add_systems(
                Update,
                (act, shield, strike, mine, fade)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing),
            );
    }
}
