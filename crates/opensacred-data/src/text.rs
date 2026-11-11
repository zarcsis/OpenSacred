//! The game's texts in the open format: `text/en-US/global.ftl`, one Fluent message `id-N` for
//! each string of the original's `global.res`, `N` the hash of the string's name (docs/02 §8).
//! The game finds a text by its name through the same hash.

/// The texts of the game's language, relative to the asset directory.
pub const GLOBAL: &str = "text/en-US/global.ftl";

/// The name of the project's own label of set 22's potions in a slain foe's drop, which no string
/// of `global.res` tells: `opensacred-convert text` adds it, and the equipment window's list of the
/// sets puts the per cent after it (#1823; docs/decisions/interface.md, "Set list of what the code
/// applies").
pub const SET_POTIONS: &str = "OPENSACRED_SET_POTIONS";

/// The name of the project's own label of the wild animals held and caged as they flee: the
/// items' effect 855 and set 41's terror, whose own text speaks of flying opponents
/// (`opensacred-convert text` adds it; #2606, docs/decisions/interface.md, "Fleeing animals'
/// trap told").
pub const TRAP_FLEEING: &str = "OPENSACRED_TRAP_FLEEING";

/// The id of the string named `name` (`FUN_0080E780`, per sacred-sdk, MIT): its [`sacred_key`]
/// without the top bit (docs/02 §8). A numeric reference `res:N` hashes the text of `N`.
pub fn sacred_hash(name: &[u8]) -> u32 {
    sacred_key(name) & 0x7FFF_FFFF
}

/// The hash over the bytes of `name`, ASCII letters upper-cased, with the original's 32-bit
/// overflow and signed remainder, its top bit kept: the key that the scripts keep a creature's
/// name `res:<name>` by (`CreateNPC`'s, docs/re/netscript.md, "`Attack`"), whose top bit is set
/// for about half of the names.
pub fn sacred_key(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in name {
        let c = u32::from(b.to_ascii_uppercase());
        let v = h.wrapping_mul(113).wrapping_add(c).cast_signed() % 999_999_991;
        h = v.cast_unsigned();
    }
    h
}

/// The Fluent message of the string named `name`: `id-N`, `N` its [`sacred_hash`].
pub fn message(name: &str) -> String {
    format!("id-{}", sacred_hash(name.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_hashes_as_the_original_does() {
        assert_eq!(sacred_hash(b"1"), 49);
        assert_eq!(sacred_hash(b"1024"), 71_320_567);
        assert_eq!(sacred_hash(b"seraphim"), sacred_hash(b"SERAPHIM"));
        assert_eq!(message("1"), "id-49");
        // Quest 10252's boss, its key as `CreateNPC` logs it (seen 2026-11-03), and its id.
        assert_eq!(sacred_key(b"ENDGEGNER_10252_A"), 0xCF02_88D9);
        assert_eq!(sacred_hash(b"ENDGEGNER_10252_A"), 0x4F02_88D9);
    }
}
