//! A saved hero carried into a game, as the original's loader puts the hero's file over the hero
//! made from its class's template (`0x6058A0`; docs/re/underworld.md, "What the hero's file
//! carries"; #2362): once the hero stands, its level and experience, the attribute points spent
//! on each attribute and those left, its skills with their bases, the skill points and the
//! skills left to learn, and its gold, of its creature section (docs/02 §9, "The creature"); its
//! values made anew at its level and its health full. Its combat arts are its file's from the
//! start (`game`).

use bevy::prelude::*;
use opensacred_data::heroes::HeroEntry;

use crate::fight::{Base, Combat, Experience, Health, Holding, Regenerates, Rules};
use crate::frame::Step;
use crate::game::AppState;
use crate::hero::Body;
use crate::pickup::Pack;

/// A saved hero's values to carry into the game once its hero stands: its level and its
/// creature section's stats.
#[derive(Resource, Clone, Debug)]
pub struct Carry {
    level: u32,
    stats: opensacred_data::heroes::HeroStats,
}

impl Carry {
    /// The values of the saved hero `hero`.
    pub fn of(hero: &HeroEntry) -> Self {
        Self {
            level: hero.level,
            stats: hero.stats,
        }
    }

    /// Its experience: its points at its level, the points and skills its file leaves it, and
    /// the skills it holds counted as it learnt them.
    fn experience(&self) -> opensacred_rules::fight::Experience {
        let s = &self.stats;
        let held = s.skills.iter().filter(|&&skill| skill != 0).count();
        opensacred_rules::fight::Experience::carried(
            (u64::from(s.experience), self.level),
            (
                u32::from(s.attribute_points),
                u32::from(s.skill_points),
                u32::from(s.to_learn),
            ),
            u32::try_from(held).unwrap_or(0),
        )
    }

    /// Its class's values `base` with its own skills, their bases and the attribute points it
    /// spent.
    fn base(&self, base: &mut opensacred_rules::fight::Base) {
        base.skills = self.stats.skills;
        base.levels = self.stats.bases;
        base.extras = self.stats.spent;
    }
}

/// Carries the saved hero's values into the game once its hero stands in place (`hero::Ready`):
/// its experience, its base's skills and points, its gold, its values made anew at its level and
/// its health full; the taskbar's "+" as its points left it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn carry(
    mut commands: Commands,
    carry: Res<Carry>,
    (rules, mut experience, mut pack): (Res<Rules>, ResMut<Experience>, ResMut<Pack>),
    mut heroes: Query<
        (
            &mut Base,
            Option<&Holding>,
            Option<&crate::effects::Effects>,
            &mut Combat,
            &mut Regenerates,
            &mut Health,
            Option<&crate::riding::Saddle>,
        ),
        With<Body>,
    >,
    (mut arts, mut hero): (
        Option<ResMut<crate::arts::HeroArts>>,
        Option<ResMut<crate::hero::Hero>>,
    ),
) {
    let Some((mut base, holding, effects, mut combat, mut regenerates, mut health, saddle)) =
        heroes.iter_mut().next()
    else {
        return;
    };
    commands.remove_resource::<Carry>();
    experience.0 = carry.experience();
    carry.base(&mut base.0);
    pack.gold = carry.stats.gold;
    crate::statistics::refit(
        carry.level,
        &rules,
        (&base, holding, effects, saddle),
        (&mut combat, &mut regenerates, &mut health),
        (arts.as_deref_mut(), hero.as_deref_mut()),
    );
    health.current = health.most;
    health.level = carry.level;
    commands.insert_resource(crate::points::Plus::loaded(&experience.0));
    info!(
        "carry: the hero of level {} carried, {} experience, {} gold, the skills {:?}",
        carry.level, carry.stats.experience, carry.stats.gold, carry.stats.skills
    );
}

/// Carries a saved hero's values into its game.
pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), |mut commands: Commands| {
            commands.remove_resource::<Carry>();
        })
        .add_systems(
            Update,
            carry
                .after(Step::HeroSpawn)
                .run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<Carry>)
                .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installations_dwarf_carries_his_files_values() {
        let carry = Carry {
            level: 29,
            stats: opensacred_data::heroes::HeroStats {
                experience: 1_800_000,
                gold: 977_384,
                spent: [0, 0, 14, 14, 0, 0],
                attribute_points: 0,
                skills: [2, 10, 11, 5, 29, 0, 0, 0],
                bases: [23, 13, 10, 10, 13, 0, 0, 0],
                skill_points: 9,
                to_learn: 1,
            },
        };
        let experience = carry.experience();
        assert_eq!(
            (
                experience.points,
                experience.level,
                experience.skill_points,
                experience.to_learn()
            ),
            (1_800_000, 29, 9, 1)
        );
        let mut base = opensacred_rules::fight::Base {
            skills: [2, 10, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        };
        carry.base(&mut base);
        assert_eq!(base.skills, carry.stats.skills);
        assert_eq!(base.levels, carry.stats.bases);
        assert_eq!(base.extras, carry.stats.spent);
    }
}
