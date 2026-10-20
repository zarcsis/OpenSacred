//! The game's texts in the open format: `text/en-US/global.ftl`, one Fluent message `id-N` for
//! each string of the original's `global.res`, `N` the hash of the string's name (docs/02 §8).
//! The game finds a text by its name through the same hash.

/// The texts of the game's language, relative to the asset directory.
pub const GLOBAL: &str = "text/en-US/global.ftl";

/// The id of the string named `name` (`FUN_0080E780`, per sacred-sdk, MIT): a hash over the
/// bytes, ASCII letters upper-cased, with the original's 32-bit overflow and signed remainder
/// (docs/02 §8). A numeric reference `res:N` hashes the text of `N`.
pub fn sacred_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in name {
        let c = u32::from(b.to_ascii_uppercase());
        let v = h.wrapping_mul(113).wrapping_add(c).cast_signed() % 999_999_991;
        h = v.cast_unsigned();
    }
    h & 0x7FFF_FFFF
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
    }
}
