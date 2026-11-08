//! The hero on horseback (#2299; docs/re/horses.md, "Mounting", "Getting off and being thrown
//! off", "The control and the gait"): a click on a horse sends the hero to the point by it that
//! it mounts from (`0x59A3E0`), where it plays its mount and sits on the horse's `horse_bone`;
//! riding, the hero goes at its horse's speeds, the horse under it playing its walk or run, by no
//! stair, teleport or entrance (`hero::run`); Shift with a click on its horse, or its horse gone,
//! gets it off at the point by the horse. A horse mounted is the hero's own (#2301): the
//! portraits' column shows its button and its health's bar, whose click mounts it within 250
//! and calls it from farther.

use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::horses;

use crate::ai::Mind;
use crate::attack::{self, Aim};
use crate::controls::{Action, Keys};
use crate::crowd::InTheWay;
use crate::fight::Health;
use crate::frame::Step;
use crate::game::AppState;
use crate::hero::{self, Body, Clips, GameSpeed, Hero};
use crate::npcs::{Dies, Npcs, Steered, Walk};

/// A horse, a creature of `Creature.pak` class 4, of its class 1 to 9 (`+0x400`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Horse {
    pub class: u8,
}

/// The hero's own horse, the owners linked both ways at the mount's end (`+0x570`, `0x555300`):
/// the portraits' column shows its button and the mount's bar while the hero owns it, ridden or
/// not (`0x6D7A5C`); its death under the hero loses it (`0x5375C0`), one left alone keeps the
/// link. No sector keeps it: it stays in the world wherever it is left, as the original's
/// object does (#2301).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owned(pub Option<Entity>);

/// What a horse's description tells (`0x55C9F0`, #2301): the horse's class and level, its row
/// of the horse data, the prices' floats and the balance's `HorseOverrideTime`, and the local
/// hero's level, riding skill (22) and resistances as the fight takes them (`+0x66` times
/// `+0xD6`).
pub(crate) struct Described<'a> {
    pub horse: (u8, u32),
    pub data: (&'a horses::Row, &'a horses::Prices, f32),
    pub hero: (u32, u32, [f32; 4]),
}

/// The resistances' images of a description's line, the physical, fire, magic and poison
/// (`\g` of `0xD1` to `0xD4`, white).
const RESISTANCE_IMAGES: [u32; 4] = [0xD1, 0xD2, 0xD3, 0xD4];

/// The horse's description (`0x55C9F0`, the format `0x95692C`): its class's name (the text
/// `9069 + class`), its price ([`horses::Prices::price`]), its level, its pace (the horse data's
/// `w0`, the stats' `+0x40`), the trample's seconds, damage and regeneration of the hero on it
/// ([`horses::trample`]), the health bonus ([`horses::health_bonus`]), the hero's resistances by
/// their images, and the riding skill it needs, each number truncated. The trample's running
/// time and value of its other format (`0x956A24`, the horse holding the effect 24) wait for the
/// trample (#2302).
pub(crate) fn description(
    texts: Option<&crate::lines::Texts>,
    described: &Described,
) -> Vec<crate::tooltip::Line> {
    use crate::tooltip::{Line, Piece, WHITE};
    let text = |name: &str| {
        texts
            .and_then(|texts| texts.named(name))
            .unwrap_or(name)
            .to_owned()
    };
    let Described {
        horse: (class, level),
        data: (row, prices, time),
        hero: (hero_level, skill, resistances),
    } = *described;
    let trample = horses::trample(row, (level, hero_level, skill), time);
    let bonus = horses::health_bonus(row, level, hero_level);
    let class_name = text(&(CLASS_NAMES + u32::from(class)).to_string());
    let mut lines: Vec<Line> = [
        format!("{}: {class_name}", text(CLASS_LABEL)),
        format!("{}: {}", text(PRICE_LABEL), prices.price(class, level)),
        format!("{}: {level}", text(LEVEL_LABEL)),
        format!("{}: {}", text(PACE_LABEL), row.pace),
        format!(
            "{}: {}s",
            text("UI_TT_OVERRIDETIME"),
            truncated(trample.seconds)
        ),
        format!(
            "{}: {}",
            text("UI_TT_OVERRIDEDAMAGE"),
            truncated(trample.damage)
        ),
        format!(
            "{}: {}s",
            text("UI_TT_OVERRIDEREGENERATION"),
            truncated(trample.regeneration)
        ),
        format!("{}: {bonus}%", text("UI_TT_OVERRIDEHPBONUS")),
    ]
    .into_iter()
    .map(|line| Line::of(line, WHITE))
    .collect();
    let mut resisted = vec![Piece::Text(
        format!("{}: ", text("UI_TT_RESISTANCES")),
        WHITE,
    )];
    for (k, (&image, &value)) in RESISTANCE_IMAGES.iter().zip(&resistances).enumerate() {
        resisted.push(Piece::Image(image, WHITE));
        let gap = if k + 1 < RESISTANCE_IMAGES.len() {
            " "
        } else {
            ""
        };
        resisted.push(Piece::Text(format!("{}{gap}", truncated(value)), WHITE));
    }
    lines.push(Line(resisted));
    lines.push(Line::of(
        format!("{}: {}", text(RIDING_LABEL), row.riding),
        WHITE,
    ));
    lines
}

/// What the horse's description reads ([`description`]): the horses, the horse data, the
/// balance's prices and trample's time, and the hero's level, riding skill, resistances, seat
/// and what it holds.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct HorseSheet<'w, 's> {
    npcs: Option<Res<'w, Npcs>>,
    data: Option<Res<'w, crate::game::Data>>,
    arts: Option<Res<'w, crate::arts::HeroArts>>,
    experience: Res<'w, crate::fight::Experience>,
    horses: Query<'w, 's, (&'static Horse, &'static crate::ai::Mind)>,
    heroes: Query<'w, 's, SheetHero, With<Body>>,
}

/// What the description reads of the hero: its seat, what it holds and its fight values.
type SheetHero = (
    Option<&'static Seat>,
    Option<&'static crate::fight::Holding>,
    Option<&'static crate::fight::Combat>,
);

impl HorseSheet<'_, '_> {
    /// The horse that the hero rides, and whether it holds a bow or a crossbow ([`riding_with`]).
    pub(crate) fn ridden(&self) -> Option<(Entity, bool)> {
        let (seat, holding, _) = self.heroes.iter().next()?;
        let bow = riding_with(seat, holding)?;
        Some((seat?.ridden()?, bow))
    }

    /// The description of the horse `horse` for the hero, of the texts `texts` (`0x55C9F0`): its
    /// class and level, the horse data's row and the balance's prices and trample's time, the
    /// hero's level, riding skill and resistances as the fight takes them (their own values
    /// times their factors).
    pub(crate) fn lines(
        &self,
        horse: Entity,
        texts: Option<&crate::lines::Texts>,
    ) -> Option<Vec<crate::tooltip::Line>> {
        let (kind, mind) = self.horses.get(horse).ok()?;
        let row = self.npcs.as_deref()?.horse_row(kind.class);
        let balance = self
            .data
            .as_deref()
            .and_then(|data| data.0.tables.balance().ok());
        let prices = horses::Prices::of(balance);
        let time = crate::trample::override_time(self.data.as_deref());
        let skill = self
            .arts
            .as_deref()
            .and_then(|arts| {
                arts.skills
                    .iter()
                    .find(|&&(skill, _)| skill == RIDING_SKILL)
            })
            .map_or(0, |&(_, level)| level);
        let resistances = self
            .heroes
            .iter()
            .next()
            .and_then(|(.., combat)| combat)
            .map_or([0.0; 4], |combat| combat.0.resistance);
        Some(description(
            texts,
            &Described {
                horse: (kind.class, mind.level),
                data: (&row, &prices, time),
                hero: (self.experience.0.level, skill, resistances),
            },
        ))
    }
}

/// The riding skill.
const RIDING_SKILL: u8 = 22;

/// The description's texts: the class's label and the class names from `9069 + 1`, the price's,
/// the level's, the pace's and the riding's labels (`0x236D`, `0x236B`, `0x42E`, `0x44A`,
/// `0x24CD`).
const CLASS_NAMES: u32 = 9069;
const CLASS_LABEL: &str = "9069";
const PRICE_LABEL: &str = "9067";
const LEVEL_LABEL: &str = "1070";
const PACE_LABEL: &str = "1098";
const RIDING_LABEL: &str = "9421";

/// `x` truncated toward 0 as the original's `_ftol`, at least 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a value of a description, whole and within a u32"
)]
fn truncated(x: f32) -> u32 {
    x.max(0.0) as u32
}

/// The mark of the hero's own horse ([`Owned`]), by which a save keeps it apart from its
/// sector's creatures, as a follower (`npcs::mount_of`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnHorse;

/// Makes `horse` the hero's own (the owners linked both ways, `0x555300`): no sector keeps it
/// any longer, and it goes with the game alone. A horse owned before is no longer.
pub fn own(world: &mut World, horse: Entity) {
    let before = world.get_resource::<Owned>().and_then(|owned| owned.0);
    if let Some(before) = before.filter(|&before| before != horse)
        && let Ok(mut entity) = world.get_entity_mut(before)
    {
        entity.remove::<OwnHorse>();
    }
    world.insert_resource(Owned(Some(horse)));
    if let Some(mut streamer) = world.get_resource_mut::<crate::sectors::Streamer>() {
        streamer.disown(horse);
    }
    if let Ok(mut entity) = world.get_entity_mut(horse) {
        entity.insert((OwnHorse, DespawnOnExit(AppState::InGame)));
    }
}

/// The horse under the riding hero, by which its most health rises while it rides (`0x5658F0`
/// at `0x565D05`, #2302): its row of the horse data and its level.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saddle {
    pub row: horses::Row,
    pub level: u32,
}

impl Saddle {
    /// The most health `most` of a hero of the level `rider` raised by the horse under it, if
    /// any ([`horses::raised_health`]).
    pub fn raise(saddle: Option<&Self>, most: u32, rider: u32) -> u32 {
        saddle.map_or(most, |saddle| {
            horses::raised_health(&saddle.row, saddle.level, rider, most)
        })
    }
}

/// The hero's seat on a horse, from its walk to the horse to its getting off.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum Seat {
    /// Walking to the point by its horse that it mounts from, and its heading there.
    Approaching {
        horse: Entity,
        point: Vec3,
        heading: Vec3,
    },
    /// Playing its mount at the point, for `left` seconds still.
    Mounting { horse: Entity, left: f32 },
    /// On its horse, at its speeds (`0x4F75F0`), its model shifted by `offset` onto the horse's
    /// bone, in its own units.
    Riding {
        horse: Entity,
        speeds: [f32; 2],
        offset: Vec3,
    },
    /// Playing its dismount at the point by the horse, for `left` seconds still.
    Dismounting { left: f32 },
}

impl Seat {
    /// Whether its clip plays, the hero going nowhere: mounting or getting off.
    pub fn busy(&self) -> bool {
        matches!(self, Self::Mounting { .. } | Self::Dismounting { .. })
    }

    /// Its speeds riding, walking and running.
    pub fn speeds(&self) -> Option<[f32; 2]> {
        match self {
            Self::Riding { speeds, .. } => Some(*speeds),
            _ => None,
        }
    }

    /// The horse it rides.
    pub fn ridden(&self) -> Option<Entity> {
        match self {
            Self::Riding { horse, .. } => Some(*horse),
            _ => None,
        }
    }
}

/// A horse that the hero rides, its place in the way kept aside: it stands under the hero.
#[derive(Component, Clone, Debug)]
pub struct Ridden {
    pub rider: Entity,
    way: Option<InTheWay>,
}

impl Ridden {
    /// A horse ridden by `rider`, its place in the way `way` kept aside.
    pub(crate) fn by(rider: Entity, way: Option<InTheWay>) -> Self {
        Self { rider, way }
    }

    /// Its place in the way, kept aside while it is ridden.
    pub(crate) fn way(&self) -> Option<&InTheWay> {
        self.way.as_ref()
    }
}

pub struct RidingPlugin;

impl Plugin for RidingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Owned>()
            .add_systems(OnEnter(AppState::InGame), |mut commands: Commands| {
                commands.insert_resource(Owned::default());
            })
            .add_systems(
                Update,
                press_horse
                    .before(hero::steer)
                    .in_set(crate::frame::Unpaused),
            )
            .add_systems(
                Update,
                pick.before(hero::steer)
                    .before(crate::talk::pick)
                    .after(attack::aim)
                    .in_set(crate::frame::Unpaused),
            )
            .add_systems(
                Update,
                (approach, mount, dismount, carry, saddle)
                    .chain()
                    .after(hero::run)
                    .in_set(Step::HeroMove),
            )
            .add_systems(
                PostUpdate,
                seat.after(bevy::app::AnimationSystems)
                    .before(TransformSystems::Propagate),
            );
    }
}

/// The hero's values made again as it mounts a horse and as it leaves it, by whatever way
/// (`0x5796A0`, docs/re/horses.md): its most health raised by the horse under it
/// ([`Saddle`]), or no longer, its health kept and held at the most (**seen** in the original:
/// the Seraphim of the level 30 at 403 of 992 mounted a horse of the level 1 and kept her
/// health below the most of 1,005; at 401 of 1,005 she got off and kept it below 992).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn saddle(
    mut commands: Commands,
    (experience, rules, npcs): (
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        Option<Res<Npcs>>,
    ),
    mut heroes: Query<
        (
            Entity,
            Option<&Seat>,
            Option<&Saddle>,
            &crate::fight::Base,
            Option<&crate::fight::Holding>,
            &mut crate::fight::Health,
        ),
        With<Body>,
    >,
    horses: Query<(&Horse, &crate::ai::Mind)>,
) {
    for (hero, seat, saddle, base, holding, mut health) in &mut heroes {
        let under = seat
            .and_then(Seat::ridden)
            .and_then(|horse| horses.get(horse).ok())
            .map(|(horse, mind)| Saddle {
                row: npcs
                    .as_deref()
                    .map(|npcs| npcs.horse_row(horse.class))
                    .unwrap_or_default(),
                level: mind.level,
            });
        if under.as_ref() == saddle {
            continue;
        }
        match under {
            Some(under) => commands.entity(hero).insert(under),
            None => commands.entity(hero).remove::<Saddle>(),
        };
        let level = experience.level;
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        let skills = opensacred_rules::fight::hero_skills(base, level, &holds);
        let most = Saddle::raise(
            under.as_ref(),
            opensacred_rules::fight::hero_health_held(base, level, &holds, &skills, &rules),
            level,
        );
        debug!(
            "riding: the hero's most health {} becomes {most}",
            health.most
        );
        health.most = most;
        health.current = health.current.min(most);
    }
}

/// Whether the hero rides, and holds a bow or a crossbow where it does, for the arts' test of
/// horseback (`0x54FC20`, `0x52FEEB`); none on foot.
pub fn riding_with(seat: Option<&Seat>, holding: Option<&crate::fight::Holding>) -> Option<bool> {
    seat.and_then(Seat::ridden)
        .map(|_| holding.is_some_and(|holding| holding.0.ranged))
}

/// The rider's type and skill 22, by which it may ride a horse (`0x4267D0`).
fn rider(hero: &Hero, arts: Option<&crate::arts::HeroArts>) -> (u32, u32) {
    let riding = arts
        .and_then(|arts| arts.skills.iter().find(|&&(skill, _)| skill == 22))
        .map_or(0, |&(_, level)| level);
    (hero.creature, riding)
}

/// The line of the hero of the type `hero` that may not ride a horse (`0x67C246`, by the table
/// `0x684C34`): its class's `SOUND_FX_NOTMYHORSE01_*`, the Vampiress's in both her forms.
fn refused(hero: u32) -> Option<&'static str> {
    Some(match hero {
        1 => "SOUND_FX_NOTMYHORSE01_SERA",
        2 => "SOUND_FX_NOTMYHORSE01_GLAD",
        3 => "SOUND_FX_NOTMYHORSE01_MAGE",
        4 => "SOUND_FX_NOTMYHORSE01_DELF",
        5 => "SOUND_FX_NOTMYHORSE01_HELF",
        6 | 7 => "SOUND_FX_NOTMYHORSE01_VAMP",
        8 => "SOUND_FX_NOTMYHORSE01_DWARF",
        9 => "SOUND_FX_NOTMYHORSE01_DEMON",
        _ => return None,
    })
}

/// The hero `me` of the type `hero` refused a horse: its class's line ([`refused`], the sound
/// event (9, `0x26`), `0x67C246`) at it, at its sample's rate.
fn refusal(sfx: Option<&crate::sfx::Sfx>, hero: u32, me: Entity) -> Option<crate::sfx::Play> {
    let (id, _) = sfx?.named(refused(hero)?)?;
    Some(crate::sfx::Play {
        sound: crate::sfx::Of::Id(id),
        at: Some(me),
        rate: (0.5, crate::sfx::Roll::None),
    })
}

/// The call's whistles, one at random (the sound event (9, `0x25`), `0x67C1B0`: `rand() % 3`).
const WHISTLES: u32 = 7012;
const WHISTLE_COUNT: u32 = 3;

/// A click on a horse (`0x6172C0`, the creature event 7): on foot, the hero goes to mount it where
/// it may ride it (`0x53A170`'s sub-state 0); riding, Shift with a click on its own horse gets it
/// off (the event `0x101`). A horse of another rider, dead or lying beyond 1,879 takes no click.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn pick(
    mut commands: Commands,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, Keys),
    pointer: Res<crate::hud::Pointer>,
    mut aim: ResMut<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (hero, npcs, arts): (
        Res<Hero>,
        Option<Res<Npcs>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    horses: Query<
        (Entity, &GlobalTransform, &Horse, &Health, Option<&Ridden>),
        (Without<Dies>, Without<crate::stables::Stabled>),
    >,
    bones: (Query<&Children>, Query<(&Name, &GlobalTransform)>),
    mut heroes: Query<(Entity, &Transform, &mut Body, Option<&Seat>), Without<Dies>>,
    (sfx, mut sounds): (
        Option<Res<crate::sfx::Sfx>>,
        MessageWriter<crate::sfx::Play>,
    ),
) {
    if !buttons.just_pressed(MouseButton::Left) || aim.0 {
        return;
    }
    let (Some(cursor), Some(npcs)) = (pointer.0, npcs) else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let Some((horse, at, class, ridden)) = horses
        .iter()
        .filter(|(.., health, _)| health.current > 0)
        .map(|(entity, at, horse, _, ridden)| {
            let at = at.translation();
            (
                entity,
                at,
                horse.class,
                ridden.map(|r| r.rider),
                attack::from_line(ray, at),
            )
        })
        .filter(|&(.., distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.4.total_cmp(&b.4))
        .map(|(entity, at, class, ridden, _)| (entity, at, class, ridden))
    else {
        return;
    };
    for (me, transform, mut body, seat) in &mut heroes {
        match seat {
            // Riding: Shift and a click on its own horse gets it off.
            Some(Seat::Riding { horse: own, .. }) if *own == horse => {
                aim.0 = true;
                if keys.pressed(Action::Walk) {
                    debug!("riding: the hero gets off {horse}");
                    commands.queue(move |world: &mut World| get_off(world, me, true));
                }
            }
            Some(seat) if seat.busy() || seat.ridden().is_some() => {}
            _ => {
                if ridden.is_some_and(|rider| rider != me)
                    || (at - transform.translation).xz().length() > horses::MOUNTS_FROM
                {
                    continue;
                }
                aim.0 = true;
                let (kind, riding) = rider(&hero, arts.as_deref());
                if !horses::rides(kind, riding, &npcs.horse_row(class)) {
                    debug!("riding: the hero may not ride {horse}");
                    if let Some(play) = refusal(sfx.as_deref(), kind, me) {
                        sounds.write(play);
                    }
                    continue;
                }
                let Ok((_, horse_at, ..)) = horses.get(horse) else {
                    continue;
                };
                let (point, heading) = place_by(&hero, horse, horse_at, &bones, false);
                debug!("riding: the hero goes to mount {horse}");
                body.send_onto(point, None);
                commands.entity(me).insert(Seat::Approaching {
                    horse,
                    point,
                    heading,
                });
            }
        }
    }
}

/// The point by the horse `horse` at `at` that the hero mounts from, or gets `off` at, and the
/// heading it turns to there (`0x59A3E0`): its start bone's for the riders of
/// [`horses::by_start_bone`], else by the hero's model (`horses::place_by`), the heading the
/// horse's facing turned by 90°.
fn place_by(
    hero: &Hero,
    horse: Entity,
    at: &GlobalTransform,
    (children, names): &(Query<&Children>, Query<(&Name, &GlobalTransform)>),
    off: bool,
) -> (Vec3, Vec3) {
    let facing = (at.rotation() * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let centre = at.translation();
    let (offsets, bone) = if horses::by_start_bone(hero.creature) {
        let bone = children
            .iter_descendants(horse)
            .find_map(|e| {
                names
                    .get(e)
                    .ok()
                    .filter(|(n, _)| n.as_str() == horses::START_BONE)
            })
            .map(|(_, t)| t.translation());
        ([0.0, 0.0], bone)
    } else {
        (
            horses::place_by(hero.creature, &model_name(hero), off),
            None,
        )
    };
    let ([x, z], [hx, hz]) = horses::point_by([centre.x, centre.z], [facing.x, facing.z], offsets);
    let point = match bone {
        // Getting off, the bone's place along the horse's other axis, the horse's own along x.
        Some(bone) if off => Vec3::new(centre.x, centre.y, bone.z),
        Some(bone) => Vec3::new(bone.x, centre.y, bone.z),
        None => Vec3::new(x, centre.y, z),
    };
    (point, Vec3::new(hx, 0.0, hz))
}

/// Mounts a hero that has come to its point (within 25, `0x53A170`'s sub-state 4): it stands on
/// the point, turns to its heading and plays its mount at the rate 1.0. The point goes with the
/// horse, the hero sent after it where the horse moved; a hero sent elsewhere meanwhile, stopped
/// short of the point, or whose horse went or was taken, goes on foot.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn approach(
    mut commands: Commands,
    (clips, hero, speed, settings): (
        Option<Res<Clips>>,
        Res<Hero>,
        Res<GameSpeed>,
        Option<Res<crate::settings::Settings>>,
    ),
    horses: Query<&GlobalTransform, (With<Horse>, Without<Dies>, Without<Ridden>)>,
    bones: (Query<&Children>, Query<(&Name, &GlobalTransform)>),
    mut heroes: Query<(Entity, &mut Transform, &mut Body, &mut Seat)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    clip_assets: Res<Assets<AnimationClip>>,
) {
    for (me, mut transform, mut body, mut seat) in &mut heroes {
        let Seat::Approaching { horse, point, .. } = *seat else {
            continue;
        };
        let Ok(at) = horses.get(horse) else {
            commands.entity(me).remove::<Seat>();
            continue;
        };
        if body.ordered() {
            continue;
        }
        let (now, heading) = place_by(&hero, horse, at, &bones, false);
        let elsewhere = body
            .target()
            .is_some_and(|target| (target - point).xz().length() > FOLLOWS_BY);
        let stopped = body.target().is_none() && body.moving().is_none();
        let near = (transform.translation - now).xz().length() <= horses::MOUNTS_WITHIN;
        if elsewhere || (stopped && !near && (now - point).xz().length() <= FOLLOWS_BY) {
            commands.entity(me).remove::<Seat>();
            continue;
        }
        if !near {
            // The horse moved: the hero goes after its new point.
            if (now - point).xz().length() > FOLLOWS_BY {
                body.send_onto(now, None);
                *seat = Seat::Approaching {
                    horse,
                    point: now,
                    heading,
                };
            }
            continue;
        }
        body.stop();
        transform.translation.x = now.x;
        transform.translation.z = now.z;
        transform.rotation = hero::facing(heading);
        // No clip without the options' atmospheric animation: the mount ends at once
        // (`PICKUPANIM`, docs/re/horses.md).
        let mount = clips
            .as_ref()
            .filter(|_| crate::pickup::animated(settings.as_deref()))
            .and_then(|clips| clips.riding[0].clone());
        let left = mount.as_ref().map_or(0.0, |(_, clip)| {
            clip_assets.get(clip).map_or(0.0, AnimationClip::duration) / speed.0.max(f32::EPSILON)
        });
        if let (Some((node, _)), Some(clips)) = (mount, clips.as_ref())
            && let Some((mut player, mut transitions)) =
                clips.player.and_then(|p| players.get_mut(p).ok())
        {
            transitions
                .play(&mut player, node, Duration::ZERO)
                .set_speed(speed.0);
        }
        debug!("riding: the hero mounts {horse} for {left:.2} s");
        *seat = Seat::Mounting { horse, left };
        // The horse stands for its rider from the clip's start (both take `+0x200` bit `0x10`):
        // its walk ends and it leaves the way.
        commands.queue(move |world: &mut World| hold(world, horse, me));
    }
}

/// How far a horse's point may move before the hero walking to it is sent after it anew.
const FOLLOWS_BY: f32 = 10.0;

/// Seats a hero whose mount has played on its horse (`0x5559E0`): the hero takes the horse's place
/// and heading, the horse stops and leaves the way, and the hero goes at the horse's speeds, its
/// `Creature.pak` walk and run times its class's pace (the horse data's first word) times the
/// game's speed (`0x4F75F0`), playing its ride (`0x5467A0`).
fn mount(
    mut commands: Commands,
    time: Res<Time>,
    (npcs, speed, clips): (Option<Res<Npcs>>, Res<GameSpeed>, Option<Res<Clips>>),
    mut heroes: Query<(Entity, &mut Transform, &mut Seat), Without<Horse>>,
    horses: Query<(&Transform, &Horse, &Mind), Without<Dies>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(npcs) = npcs else {
        return;
    };
    for (me, mut transform, mut seat) in &mut heroes {
        let Seat::Mounting { horse, left } = *seat else {
            continue;
        };
        let left = left - time.delta_secs();
        if left > 0.0 {
            *seat = Seat::Mounting { horse, left };
            continue;
        }
        let Ok((at, class, mind)) = horses.get(horse) else {
            commands.entity(me).remove::<Seat>();
            continue;
        };
        let pace = npcs.horse_row(class.class).pace_factor();
        let speeds = npcs
            .speeds_of(mind.creature)
            .map(|s| f32::from(s) * pace * speed.0);
        transform.translation = at.translation;
        transform.rotation = at.rotation;
        *seat = Seat::Riding {
            horse,
            speeds,
            offset: Vec3::ZERO,
        };
        // The owners linked both ways (`0x555300`): the hero's own horse, which no sector keeps.
        commands.entity(horse).insert(Steered(None));
        commands.queue(move |world: &mut World| own(world, horse));
        if let Some(clips) = clips.as_ref()
            && let Some((node, _)) = &clips.riding[1]
            && let Some((mut player, mut transitions)) =
                clips.player.and_then(|p| players.get_mut(p).ok())
        {
            transitions
                .play(&mut player, *node, Duration::ZERO)
                .repeat()
                .set_speed(speed.0);
        }
        info!("riding: the hero rides {horse}, at {speeds:?}");
    }
}

/// A hero that gets off: its dismount plays at the point by the horse, then it stands there.
fn dismount(mut commands: Commands, time: Res<Time>, mut heroes: Query<(Entity, &mut Seat)>) {
    for (me, mut seat) in &mut heroes {
        let Seat::Dismounting { left } = *seat else {
            continue;
        };
        let left = left - time.delta_secs();
        if left > 0.0 {
            *seat = Seat::Dismounting { left };
        } else {
            commands.entity(me).remove::<Seat>();
        }
    }
}

/// Carries each ridden horse under its rider, its walk or run playing as the rider moves
/// (`0x5941A0` copies the horse's place into its rider; here the rider's into its horse); a
/// rider whose horse is dead or gone, or who dies, is thrown off at once (`0x53ECA0`, `0x5375C0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn carry(
    mut commands: Commands,
    mut owned: ResMut<Owned>,
    heroes: Query<(Entity, &Transform, &Body, &Seat, Has<Dies>), Without<Horse>>,
    mut horses: Query<(&mut Transform, &mut Steered, Has<Dies>), With<Horse>>,
) {
    for (me, transform, body, seat, dying) in &heroes {
        let Some(horse) = seat.ridden() else {
            continue;
        };
        match horses.get_mut(horse) {
            Ok((mut at, mut steered, false)) if !dying => {
                at.translation = transform.translation;
                at.rotation = transform.rotation;
                let gait = body.moving().map(|gait| gait == hero::Gait::Run);
                if steered.0 != gait {
                    steered.0 = gait;
                }
            }
            found => {
                // A ridden horse dead or gone is lost, its owner's link with it (`0x5375C0`); the
                // hero's own death keeps it (`0x552050`).
                if !matches!(found, Ok((_, _, false))) && owned.0 == Some(horse) {
                    owned.0 = None;
                    commands.entity(horse).try_remove::<OwnHorse>();
                    info!("riding: the hero loses {horse}");
                }
                debug!("riding: the hero is thrown off {horse}");
                commands.queue(move |world: &mut World| get_off(world, me, false));
            }
        }
    }
}

/// Fells the horse `horse` in its rider's place, as a lethal blow on the rider fells it
/// (`0x56F8DB`, the horse's action set to the dying one, 6): it dies with no killer, giving
/// nobody experience and dropping nothing, its dying clip the one of two that `choice` names,
/// and its death throws its rider off ([`carry`]).
pub(crate) fn fell(world: &mut World, horse: Entity, choice: u32) {
    let Ok(mut entity) = world.get_entity_mut(horse) else {
        return;
    };
    if entity.contains::<Dies>() {
        return;
    }
    if let Some(mut health) = entity.get_mut::<Health>() {
        health.current = 0;
    }
    entity.insert(Dies::by_creature(choice));
    info!("riding: a lethal blow on its rider fells {horse}");
}

/// A left click on the horse's button of the portraits' column (`0x6D6C70`): within 250 of the
/// hero's own horse the ride order (the creature event 7), as a click on the horse gives it,
/// which gets a rider off; farther the call (the event `0x116`, `0x530D68`): a living horse,
/// the hero on foot on the ground, comes to the hero's place and `R(0, 50)` along each axis,
/// walking from within 1,200, else put there (`0x4EBF20`), and the hero goes to mount it, with
/// one of its whistles (#1180).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn press_horse(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
    (owned, mut dice): (Res<Owned>, ResMut<crate::drops::Dice>),
    heroes: Query<(Entity, &Transform, &Body, Option<&Seat>), (Without<Horse>, Without<Dies>)>,
    mut horses: Query<(&mut Transform, &Mind, &InTheWay), (With<Horse>, Without<Dies>)>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(horse) = owned.0 else {
        return;
    };
    let (corner, size) = (crate::bar::HORSE_BUTTON.1, crate::bar::HORSE_BUTTON_SIZE);
    if !crate::bar::column_point(&pointer, &ui_scale, &windows)
        .is_some_and(|at| Rect::from_corners(corner, corner + size).contains(at))
    {
        return;
    }
    let Ok((me, hero, body, seat)) = heroes.single() else {
        return;
    };
    let here = hero.translation;
    if let Ok((mut at, mind, way)) = horses.get_mut(horse) {
        let far = (at.translation - here).xz().length();
        if far > horses::BUTTON_MOUNTS_WITHIN && seat.is_none() && body.level().is_none() {
            let mut about = || {
                let roll = dice.0.below(horses::CALLED_ABOUT + 1);
                f32::from(u16::try_from(roll).unwrap_or(0))
            };
            let to = here + Vec3::new(about(), 0.0, about());
            if far <= horses::CALLED_WALKS_WITHIN {
                commands
                    .entity(horse)
                    .insert(Walk::to(to, way.level, mind.speed));
            } else {
                at.translation = to;
            }
            info!("riding: the hero calls {horse} from {far:.0}");
            // Its whistle (the sound event (9, `0x25`)).
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(WHISTLES + dice.0.below(WHISTLE_COUNT)),
                at: Some(me),
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
    }
    commands.queue(move |world: &mut World| match ride(world, Some(horse)) {
        Ok(line) | Err(line) => debug!("riding: the horse's button: {line}"),
    });
}

/// Throws the hero `me` off its horse at once, without its dismount's clip, as a knock-down
/// throws a rider off (`0x56E462`, #2311).
pub(crate) fn throw_off(world: &mut World, me: Entity) {
    get_off(world, me, false);
}

/// Gets the hero `me` off its horse (the sub-state 5, `0x551E40`, which the horse's death takes
/// too, `0x53ECA0`): its model off the horse's bone, the horse back in the way, standing; the
/// hero stands at the point by the horse, and plays its dismount where `clip`; a horse gone
/// leaves it where it is.
fn get_off(world: &mut World, me: Entity, clip: bool) {
    let Some(Seat::Riding { horse, .. }) = world.get::<Seat>(me).copied() else {
        return;
    };
    if let Ok(mut horse_entity) = world.get_entity_mut(horse) {
        let way = horse_entity.get::<Ridden>().and_then(|ridden| ridden.way);
        horse_entity.remove::<(Ridden, Steered)>();
        if let Some(way) = way {
            horse_entity.insert(way);
        }
    }
    let speed = world.get_resource::<GameSpeed>().map_or(1.0, |s| s.0);
    let placed = place(world, horse, true);
    // Nor without the options' atmospheric animation (`PICKUPANIM`).
    let clip = clip && crate::pickup::animated(world.get_resource::<crate::settings::Settings>());
    let dismount = clip
        .then(|| {
            world
                .get_resource::<Clips>()
                .and_then(|clips| clips.riding[2].clone())
        })
        .flatten();
    let left = dismount.as_ref().map_or(0.0, |(_, handle)| {
        world
            .get_resource::<Assets<AnimationClip>>()
            .and_then(|assets| assets.get(handle))
            .map_or(0.0, AnimationClip::duration)
            / speed.max(f32::EPSILON)
    });
    if let Some((point, heading)) = placed
        && let Some(mut transform) = world.get_mut::<Transform>(me)
    {
        transform.translation.x = point.x;
        transform.translation.z = point.z;
        transform.rotation = hero::facing(heading);
    }
    if let Some(mut body) = world.get_mut::<Body>(me) {
        body.stop();
    }
    let player = world.get_resource::<Clips>().and_then(|clips| clips.player);
    if let (Some((node, _)), Some(player)) = (dismount, player)
        && let Ok(mut entity) = world.get_entity_mut(player)
        && let Some(mut transitions) = entity.take::<AnimationTransitions>()
    {
        if let Some(mut animation) = entity.get_mut::<AnimationPlayer>() {
            transitions
                .play(&mut animation, node, Duration::ZERO)
                .set_speed(speed);
        }
        entity.insert(transitions);
    }
    if left > 0.0 {
        world.entity_mut(me).insert(Seat::Dismounting { left });
    } else {
        world.entity_mut(me).remove::<Seat>();
    }
    info!("riding: the hero gets off {horse}");
}

/// The point by the horse `horse` that the hero mounts from, or gets `off` at, and the heading it
/// turns to there, as [`place_by`] finds them, of the world.
fn place(world: &World, horse: Entity, off: bool) -> Option<(Vec3, Vec3)> {
    let hero = world.get_resource::<Hero>()?;
    let at = world.get::<GlobalTransform>(horse)?;
    let facing = (at.rotation() * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let centre = at.translation();
    let point = if horses::by_start_bone(hero.creature) {
        let bone = bone_of(world, horse, horses::START_BONE).unwrap_or(centre);
        if off {
            Vec3::new(centre.x, centre.y, bone.z)
        } else {
            Vec3::new(bone.x, centre.y, bone.z)
        }
    } else {
        let ([x, z], _) = horses::point_by(
            [centre.x, centre.z],
            [facing.x, facing.z],
            horses::place_by(hero.creature, &model_name(hero), off),
        );
        Vec3::new(x, centre.y, z)
    };
    Some((point, Vec3::new(-facing.z, 0.0, facing.x)))
}

/// The command `ride`: riding, the hero gets off; else it goes to mount the horse `id`, or the
/// nearest within 1,879, as a click on it sends it, where it may ride it; a line of what it does.
pub fn ride(world: &mut World, id: Option<Entity>) -> Result<String, String> {
    let mut heroes = world.query_filtered::<(Entity, &Transform, Option<&Seat>), With<Body>>();
    let (me, at, seat) = heroes
        .iter(world)
        .next()
        .map(|(me, at, seat)| (me, at.translation, seat.copied()))
        .ok_or("the hero is not in the world yet")?;
    if let Some(Seat::Riding { horse, .. }) = seat {
        get_off(world, me, true);
        return Ok(format!("the hero gets off {horse}"));
    }
    let mut horses = world
        .query_filtered::<(Entity, &Transform, &Horse, &Health), (Without<Dies>, Without<Ridden>)>(
        );
    let (horse, class) = horses
        .iter(world)
        .filter(|(entity, .., health)| health.current > 0 && id.is_none_or(|id| id == *entity))
        .map(|(entity, t, horse, _)| (entity, horse.class, (t.translation - at).xz().length()))
        .filter(|&(.., distance)| distance <= horses::MOUNTS_FROM)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(entity, class, _)| (entity, class))
        .ok_or("no horse within 1,879")?;
    let hero = world.get_resource::<Hero>().ok_or("no hero")?;
    let (kind, riding) = rider(hero, world.get_resource::<crate::arts::HeroArts>());
    let row = world
        .get_resource::<Npcs>()
        .map(|npcs| npcs.horse_row(class))
        .unwrap_or_default();
    if !horses::rides(kind, riding, &row) {
        let play = refusal(world.get_resource::<crate::sfx::Sfx>(), kind, me);
        if let Some(play) = play {
            world.write_message(play);
        }
        return Err(format!(
            "the hero may not ride {horse}: it needs riding {}",
            row.riding
        ));
    }
    let (point, heading) = place(world, horse, false).ok_or("the horse has no place yet")?;
    if let Some(mut body) = world.get_mut::<Body>(me) {
        body.send_onto(point, None);
    }
    world.entity_mut(me).insert(Seat::Approaching {
        horse,
        point,
        heading,
    });
    Ok(format!("the hero goes to mount {horse} of class {class}"))
}

/// Holds the horse `horse` for its rider `me`: its walk ended, its place in the way kept aside.
fn hold(world: &mut World, horse: Entity, me: Entity) {
    let Ok(mut entity) = world.get_entity_mut(horse) else {
        return;
    };
    let way = entity.take::<InTheWay>();
    entity.remove::<Walk>().insert(Ridden { rider: me, way });
}

/// The global place of the bone `wanted` among the descendants of `root`.
fn bone_of(world: &World, root: Entity, wanted: &str) -> Option<Vec3> {
    let mut queue = vec![root];
    while let Some(entity) = queue.pop() {
        if world
            .get::<Name>(entity)
            .is_some_and(|name| name.as_str() == wanted)
        {
            return world
                .get::<GlobalTransform>(entity)
                .map(GlobalTransform::translation);
        }
        if let Some(children) = world.get::<Children>(entity) {
            queue.extend(children.iter());
        }
    }
    None
}

/// The hero's model's name as the original's file: `0661-seraphim.glb` is `seraphim.grn`.
fn model_name(hero: &Hero) -> String {
    let file = hero.model.rsplit('/').next().unwrap_or(&hero.model);
    let name = file.split_once('-').map_or(file, |(_, name)| name);
    name.replace(".glb", ".grn")
}

/// Seats the rider on its horse's bone (`0x555300(18)`: Granny's `AttachSequence` of its `Bip01`
/// to the horse's `horse_bone`): its model's root (`__Root`), which its clips set at every frame,
/// shifted after them by where its `Bip01` stood from the horse's bone, the shift kept and
/// bettered from frame to frame.
fn seat(
    mut heroes: Query<(Entity, &mut Seat)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
) {
    for (me, mut seat) in &mut heroes {
        let Seat::Riding { horse, offset, .. } = &mut *seat else {
            continue;
        };
        let named = |root: Entity, wanted: &str| {
            children
                .iter_descendants(root)
                .find(|&e| names.get(e).is_ok_and(|name| name.as_str() == wanted))
        };
        let Some(root) = named(me, MODEL_ROOT) else {
            continue;
        };
        let space = parents
            .get(root)
            .ok()
            .and_then(|parent| globals.get(parent.parent()).ok())
            .map(|parent| parent.affine().inverse());
        if let (Some(saddle), Some(pelvis), Some(space)) = (
            named(*horse, horses::HORSE_BONE).and_then(|e| globals.get(e).ok()),
            named(me, horses::RIDER_BONE).and_then(|e| globals.get(e).ok()),
            space,
        ) {
            *offset += space.transform_vector3(saddle.translation() - pelvis.translation());
        }
        if let Ok(mut transform) = transforms.get_mut(root) {
            transform.translation += *offset;
        }
    }
}

/// The root node of the models' scenes, which their clips move (`__Root`).
const MODEL_ROOT: &str = "__Root";
