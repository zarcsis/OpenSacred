use super::*;
use crate::pe::tests::image;

pub(super) fn head_bytes(head: &Head) -> Vec<u8> {
    let mut out = head.id.to_le_bytes().to_vec();
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&head.name.to_le_bytes());
    out.extend_from_slice(&head.description.to_le_bytes());
    let mut icon = head.icon.as_bytes().to_vec();
    icon.resize(ICON, 0);
    out.extend(icon);
    out.extend_from_slice(&[0; 4]);
    for sprite in head.sprites {
        out.extend_from_slice(&sprite.to_le_bytes());
    }
    out
}

/// A spell's record as the table holds it.
pub(super) fn spell_bytes(spell: &Spell) -> Vec<u8> {
    let mut out = head_bytes(&spell.head);
    out.extend_from_slice(&spell.launch.to_le_bytes());
    let element = [spell.element, 0];
    let words = spell
        .regeneration
        .iter()
        .chain(spell.effects.iter().flatten())
        .chain(spell.values.iter().flatten())
        .chain(&spell.caps)
        .chain(&element);
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.extend_from_slice(&spell.classes.to_le_bytes());
    out.extend_from_slice(&[spell.flags, 0, 0, 0]);
    assert_eq!(out.len(), SPELL_SIZE);
    out
}

/// A weapon art's record as the table holds it.
pub(super) fn weapon_art_bytes(art: &WeaponArt) -> Vec<u8> {
    let mut out = head_bytes(&art.head);
    out.extend_from_slice(&art.value_3c.to_le_bytes());
    for value in art.regeneration {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&art.distance.to_le_bytes());
    out.extend_from_slice(&[art.launch, 0, 0, 0]);
    for value in art.factor.iter().chain(&art.value_58) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in art.value_60.iter().chain(&art.value_68) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&art.classes.to_le_bytes());
    out.extend_from_slice(&[art.flags, 0, 0, 0]);
    assert_eq!(out.len(), WEAPON_ART_SIZE);
    out
}

fn head(id: u16, icon: &str) -> Head {
    Head {
        id,
        name: 800 + u32::from(id),
        description: 9200 + u32::from(id),
        icon: icon.to_owned(),
        sprites: [462, 463, 464],
    }
}

fn spell(id: u16) -> Spell {
    let n = |k: u16| id.wrapping_mul(7).wrapping_add(k);
    Spell {
        head: head(id, "GUI_SPELL_IRRITATION.TGA"),
        launch: 1,
        regeneration: [15, 9],
        effects: std::array::from_fn(|k| [n(u16::try_from(k).unwrap()), 5]),
        values: [[14, 1], [0, 0], [400, 25]],
        caps: [30, 0, 700],
        element: 3,
        classes: 0x00C0_003F,
        flags: 1,
    }
}

fn weapon_art(id: u16) -> WeaponArt {
    WeaponArt {
        head: head(id, "GUI_MOVE_RUNDUM.TGA"),
        value_3c: 5,
        regeneration: [5.0, 3.0],
        distance: -290,
        launch: 1,
        factor: [0.8, 0.1],
        value_58: [55.0, 7.0],
        value_60: [160, -20],
        value_68: [0, 871],
        classes: 2,
        flags: 1,
    }
}

/// The empty record 0 of a table of `size` bytes.
fn empty(size: usize) -> Vec<u8> {
    let mut out = head_bytes(&Head {
        id: 0,
        name: 0,
        description: 0,
        icon: INVALID_ICON.to_owned(),
        sprites: [546, 547, 548],
    });
    out.resize(size, 0);
    out
}

/// An image with the two tables at their addresses, the second right after the first's end as in
/// GOG 2.28.
fn exe(arts: &[WeaponArt], spells: &[Spell]) -> Vec<u8> {
    let mut data = empty(WEAPON_ART_SIZE);
    data.extend(arts.iter().flat_map(weapon_art_bytes));
    assert_eq!(
        data.len(),
        usize::try_from(SPELLS_ADDRESS - WEAPON_ARTS_ADDRESS).unwrap()
    );
    data.extend(empty(SPELL_SIZE));
    data.extend(spells.iter().flat_map(spell_bytes));
    image(WEAPON_ARTS_ADDRESS, &data)
}

fn both() -> (Vec<WeaponArt>, Vec<Spell>) {
    let arts = (1000..).take(WEAPON_ART_COUNT).map(weapon_art).collect();
    let spells = (1..).take(SPELL_COUNT).map(spell).collect();
    (arts, spells)
}

#[test]
fn both_tables_read_every_field_of_every_record() {
    let (arts, spells) = both();
    let file = exe(&arts, &spells);
    assert_eq!(weapon_arts(&file).unwrap(), arts);
    assert_eq!(super::spells(&file).unwrap(), spells);
}

#[test]
fn a_table_whose_record_0_is_not_the_empty_one_is_refused() {
    let (arts, spells) = both();
    let mut file = exe(&arts, &spells);
    // The spells' record 0 starts at the file's 0x400 plus the weapon arts' table.
    let first = 0x400 + WEAPON_ART_SIZE * (WEAPON_ART_COUNT + 1);
    file[first] = 1;
    let error = super::spells(&file).unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::Signature { what } if what == "table of spells"),
        "{error}"
    );
    assert!(weapon_arts(&file).is_ok());
}

#[test]
fn a_record_with_bytes_where_zeros_belong_names_its_record() {
    let (arts, spells) = both();
    for (at, what) in [(0x02, "art bytes 0x02"), (0x2C, "art icon handle")] {
        let mut file = exe(&arts, &spells);
        file[0x400 + WEAPON_ART_SIZE * 3 + at] = 1;
        let error = weapon_arts(&file).unwrap_err();
        assert_eq!(error.place.record, Some(3), "{error}");
        assert!(error.to_string().contains(what), "{error}");
    }
    let mut file = exe(&arts, &spells);
    file[0x400 + WEAPON_ART_SIZE * (WEAPON_ART_COUNT + 1) + SPELL_SIZE * 70 + 0x92] = 1;
    let error = super::spells(&file).unwrap_err();
    assert_eq!(error.place.record, Some(70), "{error}");
    assert!(error.to_string().contains("spell bytes 0x91"), "{error}");
}

#[test]
fn an_executable_that_does_not_hold_the_tables_is_refused() {
    let (arts, _) = both();
    let mut data = empty(WEAPON_ART_SIZE);
    data.extend(arts.iter().flat_map(weapon_art_bytes));
    // The spells' table lies past what the file stores.
    let file = image(WEAPON_ARTS_ADDRESS, &data);
    assert!(weapon_arts(&file).is_ok());
    let error = super::spells(&file).unwrap_err().to_string();
    assert!(
        error.contains("a virtual address the file stores"),
        "{error}"
    );
    assert!(weapon_arts(b"MZ").is_err());
}
