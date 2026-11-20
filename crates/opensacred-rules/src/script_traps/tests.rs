use super::*;
use crate::spawning::Script;

/// The script's field's point (`create_obj` of 38,98's `Enter`): the corner (2438, 6286)
/// truncated.
const ORIGIN: [f32; 2] = [130_837.0, 337_343.0];

fn ghosts(field: &Field) -> Vec<Ghost> {
    field
        .pieces
        .iter()
        .filter_map(|piece| match piece {
            Piece::Ghost(ghost) => Some(*ghost),
            Piece::Pill { .. } => None,
        })
        .collect()
}

#[test]
fn a_field_lays_its_pills_on_the_walkable_points_and_twelve_ghosts_among_them() {
    let mut dice = Script::new(vec![1; 64]);
    let field = Field::new(ORIGIN, |_| true, &mut dice);
    let pills = field
        .pieces
        .iter()
        .filter(|piece| matches!(piece, Piece::Pill { .. }))
        .count();
    assert_eq!(pills, 21 * 19);
    let ghosts = ghosts(&field);
    assert_eq!(ghosts.len(), GHOSTS);
    // Columns first: (4, 4…7), (4, 12…15), (5, 4…7), each heading its roll.
    let indices: Vec<(i32, i32)> = ghosts.iter().map(|ghost| (ghost.i, ghost.j)).collect();
    assert_eq!(&indices[..5], &[(4, 4), (4, 5), (4, 6), (4, 7), (4, 12)]);
    assert_eq!(indices[11], (5, 7));
    assert!(ghosts.iter().all(|ghost| ghost.heading == 1));
    // A ghost follows its pill in the order made.
    let first = field
        .pieces
        .iter()
        .position(|piece| matches!(piece, Piece::Ghost(_)))
        .unwrap();
    assert_eq!(
        field.pieces[first - 1],
        Piece::Pill {
            at: point(ORIGIN, 4.0, 4.0),
            taken: false
        }
    );
    // The points truncated: a hair before the cells' corners.
    assert_eq!(point(ORIGIN, 1.0, 1.0), [130_944.0, 337_450.0]);
    assert_eq!(point(ORIGIN, 20.0, 18.0), [132_983.0, 339_274.0]);
    // A point that may not be walked on gets nothing, a ghost's neither.
    let field = Field::new(ORIGIN, |at| at[0] < point(ORIGIN, 4.0, 0.0)[0], &mut dice);
    assert_eq!(field.pieces.len(), 4 * 19);
}

#[test]
fn a_ghost_steps_along_its_heading_every_half_second_and_turns_at_random() {
    // The roll 7 of 11 and above turns it, below keeps it.
    let mut keep = Script::new(vec![0; 16]);
    let mut ghost = Ghost::new(ORIGIN, (4, 4), &mut keep);
    assert_eq!(ghost.heading, 0);
    ghost.update(0.25, ORIGIN, |_| true, &mut keep);
    assert_eq!((ghost.i, ghost.j), (4, 4));
    assert_eq!(ghost.at, point(ORIGIN, 4.5, 4.0));
    ghost.update(0.25, ORIGIN, |_| true, &mut keep);
    assert_eq!((ghost.i, ghost.j, ghost.heading), (5, 4, 0));
    assert_eq!(ghost.at, point(ORIGIN, 5.0, 4.0));
    // A roll that keeps not turns it by the next: 7 of 11, then the heading 3.
    let mut turn = Script::new(vec![7, 3, 0]);
    ghost.step = 0.9;
    ghost.update(0.1, ORIGIN, |_| true, &mut turn);
    assert_eq!((ghost.i, ghost.j, ghost.heading), (6, 4, 3));
    assert_eq!(turn.taken(), 3);
    // A heading into a cell that may not be walked on, or off the grid, is turned with no roll
    // of keeping: the point a step on from its place tested.
    let mut ghost = Ghost {
        i: 19,
        j: 4,
        heading: 0,
        step: 0.9,
        at: point(ORIGIN, 19.9, 4.0),
    };
    let mut dice = Script::new(vec![2, 0]);
    ghost.update(0.1, ORIGIN, |_| true, &mut dice);
    assert_eq!((ghost.i, ghost.heading), (20, 2));
    let wall = point(ORIGIN, 20.0, 4.0)[1] + 20.0;
    let mut ghost = Ghost {
        i: 4,
        j: 3,
        heading: 1,
        step: 0.95,
        at: point(ORIGIN, 4.0, 3.95),
    };
    let mut dice = Script::new(vec![0, 0]);
    ghost.update(0.1, ORIGIN, |at| at[1] < wall, &mut dice);
    assert_eq!((ghost.j, ghost.heading), (4, 0));
    assert_eq!(dice.taken(), 2);
    // Up, down: its place moves along −y with the heading 3.
    let mut ghost = Ghost {
        i: 4,
        j: 4,
        heading: 3,
        step: 0.0,
        at: point(ORIGIN, 4.0, 4.0),
    };
    ghost.update(0.25, ORIGIN, |_| true, &mut dice);
    assert_eq!(ghost.at, point(ORIGIN, 4.0, 3.5));
    assert_eq!(heading(1), (0, 1));
    assert_eq!(heading(2), (-1, 0));
}

#[test]
fn the_hero_takes_the_first_pill_near_him_and_a_ghost_strikes_once_a_second() {
    let mut dice = Script::new(vec![0; 64]);
    let mut field = Field::new(ORIGIN, |_| true, &mut dice);
    // The ghost of (5, 4), after 95 pills of the first five columns, the column 4's eight ghosts
    // and the four pills before its own.
    let ghost_at = match field.pieces[95 + 8 + 4 + 1] {
        Piece::Ghost(ghost) => ghost.at,
        Piece::Pill { .. } => panic!("no ghost"),
    };
    assert_eq!(ghost_at, point(ORIGIN, 5.0, 4.0));
    // Far from all: nothing.
    assert_eq!(field.touch([0.0, 0.0]), Touched::default());
    // At a pill: taken, once.
    let at = point(ORIGIN, 1.0, 1.0);
    let touched = field.touch([at[0] + 40.0, at[1] + 40.0]);
    assert_eq!(touched.pill, Some(20));
    assert!(matches!(field.pieces[20], Piece::Pill { taken: true, .. }));
    let touched = field.touch([at[0] + 40.0, at[1] + 40.0]);
    assert_eq!(touched, Touched::default());
    // Within 4000 squared, about 63: along the diagonal toward the next column's next pill.
    let at = point(ORIGIN, 3.0, 3.0);
    assert_eq!(field.touch([at[0] + 44.8, at[1] + 44.8]).pill, None);
    assert_eq!(
        field.touch([at[0] + 44.7, at[1] + 44.7]).pill,
        Some(19 * 3 + 3)
    );
    // At a ghost: its pill first, which ends the test.
    let touched = field.touch(ghost_at);
    assert!(touched.pill.is_some() && !touched.struck);
    let touched = field.touch(ghost_at);
    assert_eq!(touched.pill, None);
    assert!(touched.struck);
    assert_eq!(field.cooldown, GHOST_PERIOD);
    // A second's cooldown for the whole field, counted down by its update.
    assert!(!field.touch(ghost_at).struck);
    field.update(0.5, |_| true, &mut dice);
    assert_eq!(field.cooldown, 0.5);
    field.update(0.6, |_| true, &mut dice);
    assert_eq!(field.cooldown, 0.0);
    assert!((field.clock - 1.1).abs() < 1e-6);
}

#[test]
fn a_fields_glows_beat_once_a_second() {
    let mut field = Field::new(ORIGIN, |_| false, &mut Script::default());
    assert_eq!(field.halves(), [10.0, 15.0]);
    field.clock = 0.3;
    let [pill, ghost] = field.halves();
    assert!((pill - 13.0).abs() < 1e-5 && (ghost - 18.0).abs() < 1e-5);
    field.clock = 0.65;
    let [pill, _] = field.halves();
    assert!((pill - 11.5).abs() < 1e-5);
}

#[test]
fn the_fences_come_every_three_seconds_and_every_five_and_a_half() {
    let mut fences = Fences::default();
    assert_eq!(fences.update(0.1), [true, true]);
    assert_eq!(fences.update(2.9), [false, false]);
    assert_eq!(fences.update(0.2), [true, false]);
    assert_eq!(fences.update(2.6), [false, true]);
}

#[test]
fn a_fence_runs_eight_segments_from_its_far_end_to_its_maker() {
    let mut dice = Script::new((10..20).collect());
    let slow = fence(false, &mut dice);
    assert_eq!(slow.len(), 8);
    let lengths: Vec<f32> = slow.iter().map(|segment| segment.length).collect();
    assert_eq!(lengths, [30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 20.0]);
    let [nx, ny] = FENCE_LINE;
    assert!((nx * nx + ny * ny - 1.0).abs() < 1e-5 && (ny / nx + 1.1).abs() < 1e-5);
    let first = slow[0];
    assert!((first.start[0] - 230.0 * nx).abs() < 1e-3);
    assert!((first.start[1] - 230.0 * ny).abs() < 1e-3);
    assert_eq!(first.start[2], 40.0);
    let last = slow[7];
    assert!((last.start[0] - 20.0 * nx).abs() < 1e-3 && (last.start[1] - 20.0 * ny).abs() < 1e-3);
    // Each joint shared: a segment's end joint the next one's start.
    assert_eq!(first.joints, [10, 11]);
    assert_eq!(slow[1].joints, [11, 12]);
    assert_eq!(slow[1].sways[0], first.sways[1]);
    // Swaying 0 at the far end, 4 near the middle, a little back past the maker's end.
    assert!(first.sways[0].abs() < 1e-5);
    assert!((slow[4].sways[0] - 4.0 * (110.0 * std::f32::consts::PI / 230.0).sin()).abs() < 1e-5);
    assert!((last.sways[1] + 4.0 * (10.0 * std::f32::consts::PI / 230.0).sin()).abs() < 1e-5);
    assert_eq!(first.index, 0);
    assert_eq!(last.index, 7);
    assert!(!first.fast);
    // The fast one 800, 880 off.
    let fast = fence(true, &mut dice);
    assert!((fast[0].start[0] - 800.0 - 230.0 * nx).abs() < 1e-3);
    assert!((fast[0].start[1] - 880.0 - 230.0 * ny).abs() < 1e-3);
    assert!(fast[0].fast);
    // Its line toward the maker; its texture's u on from the last segment's; its glows.
    assert_eq!(first.along(), [-nx, -ny]);
    let close = |got: [[f32; 2]; 2], want: [[f32; 2]; 2]| {
        got.iter()
            .flatten()
            .zip(want.iter().flatten())
            .all(|(got, want)| (got - want).abs() < 1e-5)
    };
    assert!(close(slow[2].u(), [[0.2, 0.3], [1.2, 1.8]]));
    assert!(close(last.u(), [[0.7, 0.766_666_7], [4.2, 4.6]]));
    assert_eq!(first.glows(), [true, false]);
    assert_eq!(slow[3].glows(), [false, false]);
    assert_eq!(last.glows(), [false, true]);
}

#[test]
fn a_segments_joints_sway_by_the_table_as_the_frames_go() {
    let mut table = [[0.0; 3]; 256];
    table[12] = [0.5, -1.0, 1.0];
    table[13] = [0.25, 0.5, -0.5];
    let segment = Segment {
        start: [0.0; 3],
        length: 30.0,
        index: 1,
        joints: [10, 11],
        sways: [2.0, 4.0],
        fast: false,
    };
    let ([near, far], shift) = segment.sway(&table, 2);
    assert_eq!(near, [-2.0, 2.0]);
    assert_eq!(far, [2.0, -2.0]);
    assert_eq!(shift, 0.5);
    // The frames wrap at 256.
    let ([near, _], _) = segment.sway(&table, 258);
    assert_eq!(near, [-2.0, 2.0]);
    let table = sway_table(&mut Script::new(vec![0, 32_767, 16_384]));
    assert_eq!(table[0][0], -1.0);
    assert_eq!(table[0][1], 1.0);
    assert!(table[0][2].abs() < 1e-3);
}

#[test]
fn a_segment_flies_then_stops_fades_and_strikes_once_a_second() {
    let mut dice = Script::default();
    let segment = fence(false, &mut dice)[0];
    let mut laser = Laser::new(segment);
    assert_eq!(laser.speed, 100.0);
    assert!(laser.update(1.0, 0.5));
    let [hx, hy] = laser.heading();
    assert!((hx - 0.672_673_6).abs() < 1e-6 && (hy - 0.739_940_9).abs() < 1e-6);
    assert!((laser.place[0] - segment.start[0] - 100.0 * hx).abs() < 1e-3);
    assert!((laser.place[1] - segment.start[1] - 100.0 * hy).abs() < 1e-3);
    assert_eq!(laser.place[2], 40.0);
    assert_eq!(laser.glow, 20.0);
    assert_eq!(laser.light(), 100.0);
    // Held 12 s, then stopped and fading by twice the time.
    assert!(laser.update(11.0, 0.0));
    assert_eq!(laser.fade, 1.0);
    assert!(laser.update(0.25, 1.0));
    assert_eq!(laser.speed, 0.0);
    assert_eq!(laser.fade, 0.5);
    assert_eq!(laser.glow, 23.0);
    assert_eq!(laser.light(), 50.0);
    assert_eq!(laser.shade(), 127.0 / 255.0);
    let stopped = laser.place;
    assert!(!laser.update(0.3, 0.0));
    assert_eq!(laser.place, stopped);
    // Another segment's light 150, a fast one back at 450 for 2.6 s.
    let fast = fence(true, &mut dice)[3];
    let mut laser = Laser::new(fast);
    assert_eq!(laser.light(), 150.0);
    assert_eq!(laser.speed, 450.0);
    assert_eq!(laser.heading(), [-hx, -hy]);
    assert!(laser.update(2.6, 0.0));
    assert_eq!(laser.speed, 450.0);
    assert!(laser.update(0.1, 0.0));
    assert_eq!(laser.speed, 0.0);
    // Its strike: only with a target, then a second's cooldown.
    assert!(!laser.strikes(false));
    assert!(laser.strikes(true));
    assert!(!laser.strikes(true));
    laser.update(0.6, 0.0);
    assert!(!laser.strikes(true));
    laser.update(0.4, 0.0);
    assert!(laser.strikes(true));
}
