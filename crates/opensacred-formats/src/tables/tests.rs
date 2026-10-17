use super::*;

/// The bytes of a record, written back from its fields: a reader that loses nothing gives back
/// the record it read.
pub(super) fn creature_bytes(c: &Creature) -> Vec<u8> {
    let mut b = c.id.to_le_bytes().to_vec();
    b.extend([c.class, 0, c.flags, 0]);
    c.experience.iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.extend(c.base);
    b.extend([0, 0]);
    b.extend(c.skills);
    b.extend(c.skills_extra);
    c.speed.iter().for_each(|v| b.extend(v.to_le_bytes()));
    c.bonus.iter().for_each(|p| b.extend(p));
    b.extend(c.bonus_value);
    b.extend(c.bonus_class);
    c.damping.iter().for_each(|d| b.extend(d));
    b
}

pub(super) fn weapon_bytes(w: &Weapon) -> Vec<u8> {
    let mut b = Vec::new();
    w.floats_00.iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.extend(w.bytes_14);
    b.extend(w.float_18.to_le_bytes());
    b.extend(w.bytes_1c);
    b.extend(w.parent.to_le_bytes());
    let mut name: Vec<u8> = w.name.chars().map(|c| u8::try_from(c).unwrap()).collect();
    name.resize(0x80 - 0x28, 0);
    b.extend(name);
    b.extend(w.id.to_le_bytes());
    w.words_84.iter().for_each(|v| b.extend(v.to_le_bytes()));
    b
}

pub(super) fn motion_bytes(index: u16, m: &Motion) -> Vec<u8> {
    let mut b = [index, 0, index, m.value_06]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    b.resize(0x46, 0);
    m.floats_46.iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.resize(0x62, 0);
    m.floats_62.iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.resize(0xBE, 0);
    b.extend(m.value_be.to_le_bytes());
    b.resize(MOTION_SIZE, 0);
    b
}

pub(super) fn trigger_bytes(t: Option<&Trigger>) -> Vec<u8> {
    let Some(t) = t else {
        return vec![0; TRIGGER_SIZE];
    };
    let mut b = t.id.to_le_bytes().to_vec();
    let target = [16, t.target & 0xFFFF, t.target >> 16, 1];
    target
        .iter()
        .for_each(|&v| b.extend(u16::try_from(v).unwrap().to_le_bytes()));
    b.extend([0; 4]);
    b
}

fn file(magic: &[u8; 3], version: u8, count: u32, body: &[u8]) -> Vec<u8> {
    let mut d = vec![0; HEADER];
    d[..3].copy_from_slice(magic);
    d[3] = version;
    d[4..8].copy_from_slice(&count.to_le_bytes());
    d.extend_from_slice(body);
    d
}

fn creature() -> Creature {
    Creature {
        id: 50,
        class: 2,
        flags: 0x21,
        experience: [52, 48],
        base: [33, 34, 24, 43, 0, 50],
        skills: [2, 25],
        skills_extra: [3; 16],
        speed: [80, 80],
        bonus: [[1, 4], [2, 5], [0, 0], [0, 0], [0, 0], [0, 0]],
        bonus_value: [75, 75, 0, 0, 0, 0],
        bonus_class: [0; 6],
        damping: [[0, 15, 30, 45, 60]; 4],
    }
}

#[test]
fn records_read_back_to_their_bytes() {
    let c = creature();
    let data = file(b"CIF", 0, 1, &creature_bytes(&c));
    assert_eq!(creatures(&data).unwrap(), [c]);

    let w = Weapon {
        floats_00: [1.0, 0.0, 0.25, -0.9, 1.9],
        bytes_14: [5, 5, 0, 0],
        float_18: -4.0,
        bytes_1c: [1, 1, 0, 0, 0, 0, 2, 0],
        parent: 1724,
        name: "Schwert".into(),
        id: 7901,
        words_84: [7; 63],
    };
    let mut body = weapon_bytes(&w);
    body.extend([0; WEAPON_RESERVE]);
    assert_eq!(weapons(&file(b"WPN", 8, 1, &body)).unwrap(), [w]);

    let m = Motion {
        value_06: 0,
        floats_46: [0.5, 0.25, 0.0, 0.0],
        floats_62: [0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.1],
        value_be: 4,
    };
    let mut body = motion_bytes(0, &m);
    body.extend(motion_bytes(1, &m));
    assert_eq!(motions(&file(b"MHP", 1, 2, &body)).unwrap(), [m.clone(), m]);

    let t = Trigger {
        id: 2266,
        target: 0xF_3016,
    };
    let mut body = [136u32, 0x10C, 32]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    body.extend(trigger_bytes(None));
    body.extend(trigger_bytes(Some(&t)));
    assert_eq!(
        triggers(&file(b"TRG", 1, 2, &body)).unwrap(),
        [None, Some(t)]
    );
}

#[test]
fn malformed_tables_are_errors() {
    let error = |r: Result<Vec<Creature>>| r.unwrap_err().to_string();
    let mut bytes = creature_bytes(&creature());
    assert!(error(creatures(&file(b"CIF", 0, 2, &bytes))).contains("`record count` is 2"));
    assert!(error(creatures(&file(b"WPN", 0, 1, &bytes))).contains("expected magic `CIF`"));
    bytes[5] = 1;
    assert!(error(creatures(&file(b"CIF", 0, 1, &bytes))).contains("creature byte 0x05"));
    let mut body = motion_bytes(
        1,
        &Motion {
            value_06: 0,
            floats_46: [0.0; 4],
            floats_62: [0.0; 9],
            value_be: 0,
        },
    );
    assert!(
        motions(&file(b"MHP", 1, 1, &body))
            .unwrap_err()
            .to_string()
            .contains("`motion index` is 1")
    );
    body[0] = 0;
    body[4] = 0;
    body[0x30] = 1;
    assert!(
        motions(&file(b"MHP", 1, 1, &body))
            .unwrap_err()
            .to_string()
            .contains("motion bytes 0x08")
    );
}
