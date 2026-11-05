//! The scripts' Pong (docs/re/effects.md, "The scripts' Pong", #2008): a court of two bats and a
//! ball that every tree's `Sector84080Enter` makes. The hero's bat follows the hero across the
//! court, the other bat follows the ball, the ball bounces off the court's walls and the bats and
//! goes back to its start once past a bat, and each ball that the hero's bat returns gives the
//! hero experience by the ball's speed.
//!
//! Places are the world's on the ground, in its units: `x` along the cells' columns and `y` along
//! their rows (the game's X and Z); a heading is a unit vector of that plane.

use crate::Cell;
use crate::spawning::Dice;

/// The court's columns within which a bat follows what it follows (`0x5ABD80`: above 5397 and
/// below 5420), and the rows outside which a ball that a cell stops has passed a bat
/// (`0x5A9152`: below 5133 or above 5161).
pub const COLUMNS: (i32, i32) = (5398, 5419);
pub const ROWS: (i32, i32) = (5133, 5161);
/// The ball's speed as it starts and at most, in the flight's units a second, and its rise
/// (`0x7BD7A0`, `0x7BD560`): 20 every 5 s, its first rise 10 s after a ball is lost.
pub const START_SPEED: f32 = 250.0;
pub const TOP_SPEED: f32 = 1000.0;
const SPEED_STEP: f32 = 20.0;
const SPEED_EVERY: f32 = 5.0;
const SPEED_AFTER_LOSS: f32 = 10.0;
/// The ball's wait before it flies, falling by a half a second from 1: 2 s.
const WAIT_FALLS: f32 = 0.5;
/// How near a bat a ball returns (`0x5ABD80`): within 30 world units of the bat's row on the
/// court's side and within 80 across.
pub const REACH_ALONG: f32 = 30.0;
pub const REACH_ACROSS: f32 = 80.0;
/// A bat's brightening at a return, falling by 1 a second (`0x7BCBC0`), and its sound, `PONG` at
/// a rate's factor of 0.5.
pub const BRIGHTENING: f32 = 1.0;
pub const SOUND: u32 = 16_303;
pub const SOUND_RATE: f32 = 0.5;

/// The experience that a ball of the speed `speed` gives the holder of the bat it returns off:
/// twice its speed less 400 (`0x5B1400`), 100 at its start's speed.
pub fn experience(speed: f32) -> u32 {
    let points = (2.0 * speed - 400.0).max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a speed of 250 to 1,000, its experience a whole number of 0 to 1,600"
    )]
    let points = points as u32;
    points
}

/// Whether a bat follows what stands in the column `column` (`0x5ABD80`).
pub fn follows(column: i32) -> bool {
    (COLUMNS.0..=COLUMNS.1).contains(&column)
}

/// Which bat: the hero's, at the court's end of the higher rows, whose ball comes back toward the
/// lower rows (its normal (1, −1) of the flight's frame), or the other, at the lower rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bat {
    Hero,
    Other,
}

impl Bat {
    /// The way along the rows from the bat into the court: −1 for the hero's, +1 for the other's.
    fn inward(self) -> f32 {
        match self {
            Bat::Hero => -1.0,
            Bat::Other => 1.0,
        }
    }
}

/// A ball of the court: its place and heading, its speed, its wait before it flies, its clock
/// to its next rise of speed, and its start, where it goes back to.
#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    pub place: [f32; 2],
    pub heading: [f32; 2],
    pub speed: f32,
    /// From 1 to 0 while it stands; it flies once it is 0 (`+0x20AC`).
    pub wait: f32,
    pub flying: bool,
    rise: f32,
    start: [f32; 2],
}

impl Ball {
    /// A ball at `start` of the heading `heading` (`0x7BD7A0`).
    pub fn new(start: [f32; 2], heading: [f32; 2]) -> Self {
        Self {
            place: start,
            heading,
            speed: START_SPEED,
            wait: 1.0,
            flying: false,
            rise: SPEED_EVERY,
            start,
        }
    }

    /// A run of `dt` seconds of its class (`0x7BD560`): its speed rises by 20 every 5 s, to 1,000
    /// at most, and while it stands its wait falls by a half a second until it flies.
    pub fn tick(&mut self, dt: f32) {
        self.rise -= dt;
        if self.rise < 0.0 {
            self.rise = SPEED_EVERY;
            self.speed = (self.speed + SPEED_STEP).min(TOP_SPEED);
        }
        if !self.flying {
            self.wait -= dt * WAIT_FALLS;
            if self.wait < 0.0 {
                self.wait = 0.0;
                self.flying = true;
            }
        }
    }

    /// Its flight of `dt` seconds (`0x5A9152`), `scale` world units a unit of its speed along its
    /// heading: where the cell it comes into `stops` it, it turns back along the axis of the step
    /// that crossed into it, off the court's walls; outside the court's rows, past a bat, it is
    /// lost: it goes back to its start, its heading turned by up to a half a radian either way
    /// (`0x529720`), and stands as it started, its speed 250. Whether it was lost.
    pub fn fly(
        &mut self,
        dt: f32,
        scale: f32,
        (stops, cell_of): (impl Fn(Cell) -> bool, impl Fn([f32; 2]) -> Cell),
        dice: &mut impl Dice,
    ) -> bool {
        if !self.flying {
            return false;
        }
        let step = self.speed * scale * dt;
        let next = [
            self.place[0] + self.heading[0] * step,
            self.place[1] + self.heading[1] * step,
        ];
        let (here, there) = (cell_of(self.place), cell_of(next));
        if !stops(there) {
            self.place = next;
            return false;
        }
        if there.1 < ROWS.0 || there.1 > ROWS.1 {
            self.lose(dice);
            return true;
        }
        // Back along the axis whose step crossed into the stopping cell.
        let across = there.0 != here.0 && stops((there.0, here.1));
        let along = there.1 != here.1 && stops((here.0, there.1));
        if across || !along {
            self.heading[0] = -self.heading[0];
        }
        if along || !across {
            self.heading[1] = -self.heading[1];
        }
        false
    }

    /// Lost past a bat: back at its start, turned, standing, its speed 250 and its next rise in
    /// 10 s (the ball's action 8, `0x7BD560`).
    fn lose(&mut self, dice: &mut impl Dice) {
        let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
        let (sin, cos) = (roll - 0.5).sin_cos();
        let [x, y] = self.heading;
        self.heading = [x * cos - y * sin, x * sin + y * cos];
        self.place = self.start;
        self.flying = false;
        self.wait = 1.0;
        self.speed = START_SPEED;
        self.rise = SPEED_AFTER_LOSS;
    }

    /// Whether it comes back off the bat of the side `bat` at `at` (`0x5ABD80`): near it and going
    /// toward it, it turns back along the rows; the experience it gives the bat's holder.
    pub fn returns(&mut self, bat: Bat, at: [f32; 2]) -> Option<u32> {
        if !self.flying {
            return None;
        }
        let inward = bat.inward();
        // Within 30 of the bat's row on the court's side, within 80 across, and coming at it.
        let near = (self.place[1] - at[1]) * inward < REACH_ALONG
            && (self.place[0] - at[0]).abs() < REACH_ACROSS;
        if !near || self.heading[1] * inward >= 0.0 {
            return None;
        }
        self.heading[1] = -self.heading[1];
        Some(experience(self.speed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Rng;

    const CELL: f32 = 53.665_63;

    fn cell_of([x, y]: [f32; 2]) -> Cell {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a test's world point, whole cells"
        )]
        let cell = ((x / CELL).floor() as i32, (y / CELL).floor() as i32);
        cell
    }

    fn centre((x, y): Cell) -> [f32; 2] {
        #[expect(clippy::cast_precision_loss, reason = "a test's cell")]
        let point = [(x as f32 + 0.5) * CELL, (y as f32 + 0.5) * CELL];
        point
    }

    #[test]
    fn a_ball_waits_two_seconds_then_flies_and_speeds_up() {
        let mut ball = Ball::new(centre((5401, 5147)), [0.0, 1.0]);
        for _ in 0..19 {
            ball.tick(0.1);
        }
        assert!(!ball.flying);
        ball.tick(0.2);
        assert!(ball.flying);
        // 20 every 5 s, to 1,000 at most.
        let mut ball = Ball::new([0.0; 2], [0.0, 1.0]);
        for _ in 0..51 {
            ball.tick(0.1);
        }
        assert_eq!(ball.speed, 270.0);
        for _ in 0..400 {
            ball.tick(1.0);
        }
        assert_eq!(ball.speed, TOP_SPEED);
    }

    #[test]
    fn a_ball_returns_off_a_bat_coming_at_it_and_gives_its_experience() {
        let bat = centre((5406, 5161));
        // A cell short of its row it is not yet near; 20 short it is.
        let mut ball = Ball::new(centre((5406, 5160)), [0.0, 1.0]);
        ball.flying = true;
        assert_eq!(ball.returns(Bat::Hero, bat), None);
        ball.place = [bat[0], bat[1] - 20.0];
        assert_eq!(ball.returns(Bat::Hero, bat), Some(100));
        assert!(ball.heading[1] < 0.0);
        // Going away from it, or past it across, it does not.
        assert_eq!(ball.returns(Bat::Hero, bat), None);
        let mut wide = Ball::new([bat[0] + 90.0, bat[1] - 20.0], [0.0, 1.0]);
        wide.flying = true;
        assert_eq!(wide.returns(Bat::Hero, bat), None);
        // The other bat turns it back toward the higher rows.
        let other = centre((5406, 5133));
        let mut ball = Ball::new([other[0], other[1] + 20.0], [0.0, -1.0]);
        ball.flying = true;
        assert_eq!(ball.returns(Bat::Other, other), Some(100));
        assert!(ball.heading[1] > 0.0);
        assert_eq!(experience(TOP_SPEED), 1_600);
        assert!(follows(5398) && follows(5419) && !follows(5420));
    }

    #[test]
    fn a_ball_bounces_off_the_walls_and_is_lost_past_a_bat() {
        let mut dice = Rng::of(0, 0, 5);
        // A wall along the column 5420.
        let stops = |(x, y): Cell| x >= 5420 || !(5120..=5175).contains(&y);
        let mut ball = Ball::new(centre((5419, 5147)), [1.0, 0.0]);
        ball.flying = true;
        assert!(!ball.fly(0.5, 1.0, (stops, cell_of), &mut dice));
        assert!(ball.heading[0] < 0.0 && ball.heading[1].abs() < 1e-6);
        // Past the hero's bat, it goes back to its start and stands.
        let start = centre((5401, 5147));
        let mut ball = Ball::new(start, [0.0, 1.0]);
        ball.flying = true;
        ball.place = centre((5406, 5170));
        let lost = |(_, y): Cell| y > 5170;
        assert!(ball.fly(0.5, 1.0, (lost, cell_of), &mut dice));
        assert_eq!(ball.place, start);
        assert!(!ball.flying && ball.speed == START_SPEED);
        assert!((ball.heading[0].hypot(ball.heading[1]) - 1.0).abs() < 1e-5);
    }
}
