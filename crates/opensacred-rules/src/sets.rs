//! The weapon sets of the taskbar (#1319; docs/re/inventory.md, §5): five sets of two halves, the
//! half 0 the left hand and the half 1 the right; a weapon's hands (`0x5CEC00`) and whether it is
//! two-handed (`0x5CECD0`) by its kind; the rule of a weapon or a shield put into a half
//! (`0x6E09C0`), the drop that tries the half under the cursor and then the other (`0x6E14C0`),
//! and an item taken out of a half.

use std::ops::RangeInclusive;

/// The sets of the taskbar.
pub const SETS: usize = 5;

/// The fist weapons, of the right hand and two-handed whatever their kind.
const FISTS: RangeInclusive<u32> = 4067..=4072;

/// The hands a weapon goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hands {
    Left,
    Right,
    Either,
}

/// The hands of a weapon of the kind `kind` (`Weapon.pak +0x1E`) and the type `item`, or its
/// parent where its record has one (`0x5CEC00`): the left for the kinds 3, 7 (the bows) and 12
/// (the crossbows); either for 0, 1 (the swords and axes), 5 (the daggers), 6 and 10 (the dark
/// elves' blades); the right for the rest and the fist weapons.
pub fn hands(kind: u8, item: u32) -> Hands {
    if FISTS.contains(&item) {
        return Hands::Right;
    }
    match kind {
        3 | 7 | 12 => Hands::Left,
        0 | 1 | 5 | 6 | 10 => Hands::Either,
        _ => Hands::Right,
    }
}

/// Whether a weapon of the kind `kind` and the type `item`, as [`hands`] takes them, is
/// two-handed (`0x5CECD0`): the kinds 2, 3, 4 (the staves and polearms), 7 and 12, and the fist
/// weapons.
pub fn two_handed(kind: u8, item: u32) -> bool {
    FISTS.contains(&item) || matches!(kind, 2 | 3 | 4 | 7 | 12)
}

/// An item in a half of a set: its key, and whether it is a shield, else a weapon of its hands
/// and its hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub key: u32,
    pub shield: bool,
    pub hands: Hands,
    pub two_handed: bool,
}

impl Piece {
    /// A weapon of the kind `kind` and the type `item` ([`hands`], [`two_handed`]).
    pub fn weapon(key: u32, kind: u8, item: u32) -> Self {
        Self {
            key,
            shield: false,
            hands: hands(kind, item),
            two_handed: two_handed(kind, item),
        }
    }

    /// A shield, of the left hand.
    pub fn shield(key: u32) -> Self {
        Self {
            key,
            shield: true,
            hands: Hands::Left,
            two_handed: false,
        }
    }

    fn two_handed(piece: Option<Piece>) -> bool {
        piece.is_some_and(|piece| piece.two_handed)
    }
}

/// A set's halves, the left hand's and the right's.
pub type Set = [Option<Piece>; 2];

/// What a put makes of a set: the set, and the item that it replaced, which goes onto the cursor
/// or back into the grid.
pub type Put = (Set, Option<Piece>);

/// `piece` put into the half `half` of `set` (`0x6E09C0`), the hero of the skill of the two
/// weapons where `two_weapons`; none where it is refused. A shield goes into the left half alone,
/// in place of a two-handed weapon only where the left half is empty. A one-handed weapon goes
/// into the right half where its hands are the right or either, in place of a two-handed weapon
/// of either half, else of the right half's item; into the left half only with the skill, its
/// hands either, beside a weapon of either hands in the right half, in place of the left half's
/// item. A two-handed weapon takes the set alone in the half its hands name, whichever half it
/// is put on: into an empty set, or in place of its one item, refused beside two. The hero's
/// test of the item (`0x54ABE0`) is the caller's.
pub fn put(set: Set, half: usize, piece: Piece, two_weapons: bool) -> Option<Put> {
    let [left, right] = set;
    if piece.shield {
        return match (half, Piece::two_handed(right), left) {
            (0, false, _) => Some(([Some(piece), right], left)),
            (0, true, None) => Some(([Some(piece), None], right)),
            _ => None,
        };
    }
    if piece.two_handed {
        let named = usize::from(piece.hands != Hands::Left);
        let replaced = match (left, right) {
            (None, None) => None,
            (Some(one), None) | (None, Some(one)) => Some(one),
            (Some(_), Some(_)) => return None,
        };
        let mut set = [None, None];
        set[named] = Some(piece);
        return Some((set, replaced));
    }
    match half {
        1 if piece.hands != Hands::Left => {
            if Piece::two_handed(left) {
                Some(([None, Some(piece)], left))
            } else {
                Some(([left, Some(piece)], right))
            }
        }
        0 if two_weapons
            && piece.hands == Hands::Either
            && right.is_some_and(|right| !right.shield && right.hands == Hands::Either) =>
        {
            Some(([Some(piece), right], left))
        }
        _ => None,
    }
}

/// `piece` dropped on the half `half` of `set` (`0x6E14C0`): put there, else into the other
/// half; the half it went into, and what the put made; none where neither takes it.
pub fn drop(set: Set, half: usize, piece: Piece, two_weapons: bool) -> Option<(usize, Put)> {
    let other = 1 - half.min(1);
    put(set, half, piece, two_weapons)
        .map(|made| (half, made))
        .or_else(|| put(set, other, piece, two_weapons).map(|made| (other, made)))
}

/// The item of the half `half` of `set` taken out: the set, the left half's weapon moved into
/// the right half where the right half's is taken, unless its hands are the left's or it is a
/// shield (docs/re/inventory.md, §5); and the item, none where the half is empty.
pub fn take(set: Set, half: usize) -> (Set, Option<Piece>) {
    let [left, right] = set;
    match half {
        0 => ([None, right], left),
        _ => match left {
            Some(weapon) if !weapon.shield && weapon.hands != Hands::Left => {
                ([None, Some(weapon)], right)
            }
            _ => ([left, None], right),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sword(key: u32) -> Piece {
        Piece::weapon(key, 1, 1700)
    }

    #[test]
    fn a_weapons_hands_and_hold_go_by_its_kind() {
        assert_eq!(hands(7, 1800), Hands::Left);
        assert_eq!(hands(12, 1800), Hands::Left);
        assert_eq!(hands(1, 1700), Hands::Either);
        assert_eq!(hands(10, 1700), Hands::Either);
        assert_eq!(hands(4, 1700), Hands::Right);
        assert_eq!(hands(14, 1700), Hands::Right);
        // The fists, whatever their kind.
        assert_eq!(hands(1, 4068), Hands::Right);
        assert!(two_handed(1, 4068));
        assert!(two_handed(4, 1700) && two_handed(7, 1800) && two_handed(2, 1700));
        assert!(!two_handed(1, 1700) && !two_handed(5, 1700) && !two_handed(14, 1700));
    }

    #[test]
    fn a_shield_goes_into_the_left_half_alone() {
        let shield = Piece::shield(9);
        let staff = Piece::weapon(2, 4, 1700);
        // Beside a sword, the old shield replaced.
        let set = [Some(Piece::shield(8)), Some(sword(1))];
        assert_eq!(
            put(set, 0, shield, false),
            Some(([Some(shield), Some(sword(1))], Some(Piece::shield(8))))
        );
        assert_eq!(put(set, 1, shield, false), None);
        // In place of a two-handed weapon, where the left half is empty.
        assert_eq!(
            put([None, Some(staff)], 0, shield, false),
            Some(([Some(shield), None], Some(staff)))
        );
        // A bow in the left half and a staff in the right: refused.
        let bow = Piece::weapon(3, 7, 1800);
        assert_eq!(put([Some(bow), Some(staff)], 0, shield, false), None);
    }

    #[test]
    fn a_one_handed_weapon_goes_into_the_right_half_or_beside_another() {
        let staff = Piece::weapon(2, 4, 1700);
        let bow = Piece::weapon(3, 7, 1800);
        // Into an empty set's right half; the left refused, so a drop there goes right.
        assert_eq!(
            put([None, None], 1, sword(1), false),
            Some(([None, Some(sword(1))], None))
        );
        assert_eq!(put([None, None], 0, sword(1), true), None);
        assert_eq!(
            drop([None, None], 0, sword(1), false),
            Some((1, ([None, Some(sword(1))], None)))
        );
        // In place of a two-handed weapon of either half.
        assert_eq!(
            put([Some(bow), None], 1, sword(1), false),
            Some(([None, Some(sword(1))], Some(bow)))
        );
        assert_eq!(
            put([None, Some(staff)], 1, sword(1), false),
            Some(([None, Some(sword(1))], Some(staff)))
        );
        // Beside a shield, the right half's sword replaced.
        let shield = Piece::shield(8);
        assert_eq!(
            put([Some(shield), Some(sword(4))], 1, sword(1), false),
            Some(([Some(shield), Some(sword(1))], Some(sword(4))))
        );
        // Into the left half with the skill of the two weapons beside a sword, not without it,
        // nor beside a shield's half alone.
        assert_eq!(
            put([None, Some(sword(4))], 0, sword(1), true),
            Some(([Some(sword(1)), Some(sword(4))], None))
        );
        assert_eq!(put([None, Some(sword(4))], 0, sword(1), false), None);
        assert_eq!(put([Some(shield), None], 0, sword(1), true), None);
        // A weapon of the right hand alone never goes left.
        let musket = Piece::weapon(5, 14, 1700);
        assert_eq!(put([None, Some(sword(4))], 0, musket, true), None);
    }

    #[test]
    fn a_two_handed_weapon_takes_the_set_alone_in_its_hands_half() {
        let staff = Piece::weapon(2, 4, 1700);
        let bow = Piece::weapon(3, 7, 1800);
        // Dropped on either half, it lies in the half its hands name.
        assert_eq!(
            put([None, None], 0, staff, false),
            Some(([None, Some(staff)], None))
        );
        assert_eq!(
            put([None, None], 1, bow, false),
            Some(([Some(bow), None], None))
        );
        // In place of the one item, refused beside two.
        assert_eq!(
            put([None, Some(sword(1))], 0, bow, false),
            Some(([Some(bow), None], Some(sword(1))))
        );
        assert_eq!(
            put([Some(Piece::shield(8)), Some(sword(1))], 1, staff, false),
            None
        );
    }

    #[test]
    fn the_right_halfs_item_taken_moves_the_left_weapon_over() {
        let shield = Piece::shield(8);
        let bow = Piece::weapon(3, 7, 1800);
        assert_eq!(
            take([Some(sword(1)), Some(sword(4))], 1),
            ([None, Some(sword(1))], Some(sword(4)))
        );
        assert_eq!(
            take([Some(shield), Some(sword(4))], 1),
            ([Some(shield), None], Some(sword(4)))
        );
        assert_eq!(take([Some(bow), None], 0), ([None, None], Some(bow)));
        assert_eq!(take([None, None], 1), ([None, None], None));
    }
}
