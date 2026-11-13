//! The original's dead content (#2649): the masters of what nothing in the original names (no
//! script, table, model, world record or string of `Sacred.exe`), which the game therefore never
//! reads and `build` leaves out of its assets. The lists in `dead/`, a kind each, hold the
//! masters' paths, a line each, under `#` comments that say why they are dead; the audit of
//! 2026-11-12 found them (#2645). A feature that comes to use one takes it off its list.
//!
//! Kept off the lists: the sounds that the original's data names wrongly, which the game plays
//! since #2652 (`script_repairs::SOUNDS`, `opensacred-game`'s click lines), and whatever a feature
//! not implemented yet needs, which the original's code or data names.

use std::collections::HashSet;
use std::sync::LazyLock;

/// The lists, a kind each: the samples, the music, the sprites, the textures and the models.
const LISTS: [&str; 5] = [
    include_str!("dead/samples.txt"),
    include_str!("dead/music.txt"),
    include_str!("dead/sprites.txt"),
    include_str!("dead/textures.txt"),
    include_str!("dead/models.txt"),
];

/// The masters' paths of the list `text`, in its order.
fn paths(text: &'static str) -> impl Iterator<Item = &'static str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

static DEAD: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| LISTS.into_iter().flat_map(paths).collect());

/// Whether the master at `path`, relative to the masters' directory, is on one of the lists.
pub fn is_dead(path: &str) -> bool {
    DEAD.contains(path)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use opensacred_formats::mdl::{Kind, MdlPak};
    use opensacred_formats::sound::SoundPak;
    use opensacred_formats::tex::TexPak;

    use super::*;
    use crate::testing::scratch_dir;

    /// A list of [`LISTS`] by its kind: the prefix and the extension of its masters' paths.
    struct List {
        name: &'static str,
        prefix: &'static str,
        extension: &'static str,
        text: &'static str,
    }

    impl List {
        fn paths(&self) -> impl Iterator<Item = &'static str> {
            paths(self.text)
        }
    }

    const KINDS: [(&str, &str, &str); 5] = [
        ("samples", "sound/samples/", ".flac"),
        ("music", "sound/music/", ".flac"),
        ("sprites", "world/sprites/", ".png"),
        ("textures", "textures/", ".png"),
        ("models", "models/", ".glb"),
    ];

    fn lists() -> Vec<List> {
        KINDS
            .into_iter()
            .zip(LISTS)
            .map(|((name, prefix, extension), text)| List {
                name,
                prefix,
                extension,
                text,
            })
            .collect()
    }

    fn list(name: &str) -> List {
        lists().into_iter().find(|l| l.name == name).unwrap()
    }
    use crate::verify::{GOG_MANIFEST, parse_manifest, read_game_file};

    #[test]
    fn each_list_holds_its_kinds_masters_once() {
        let mut all = HashSet::new();
        for list in &lists() {
            let mut count = 0;
            for path in list.paths() {
                assert!(
                    path.starts_with(list.prefix) && path.ends_with(list.extension),
                    "{}: {path}",
                    list.name
                );
                assert!(all.insert(path), "{}: {path} twice", list.name);
                count += 1;
            }
            assert!(count > 0, "{}", list.name);
        }
        // The motions' files go by their own rule (#2647), the menus' tiles by theirs (#2646).
        assert!(all.iter().all(|path| !crate::models::is_motion(path)));
        assert!(all.iter().all(|path| !crate::ui::is_menu_tile(path)));
        assert_eq!(all.len(), 5_067);
        assert!(is_dead("world/sprites/00467.png"));
        assert!(!is_dead("world/sprites/00001.png"));
    }

    /// The index and the name of a master's file (`00467`, `0005-name`), as its list names it.
    fn stem<'a>(list: &List, path: &'a str) -> &'a str {
        let rest = path.strip_prefix(list.prefix).unwrap();
        let file = rest.rsplit('/').next().unwrap();
        file.strip_suffix(list.extension).unwrap()
    }

    /// Each listed master is a record of the original's of the same index and name, as the
    /// extractions name it, so that a list that drifts from them fails here.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_dead_master_is_a_record_of_the_original() {
        let game = opensacred_locate::original();
        // The samples: slots of `sound.pak`.
        let data = read_game_file(&game.join("PAK/sound.pak")).unwrap();
        let slots = SoundPak::parse(&data).unwrap().len();
        for path in list("samples").paths() {
            let slot: usize = stem(&list("samples"), path).parse().unwrap();
            assert!(slot < slots, "{path}");
        }
        // The music: the installation's MP3s.
        let mp3: HashSet<String> = parse_manifest(GOG_MANIFEST)
            .unwrap()
            .iter()
            .filter_map(|e| e.path.strip_prefix("mp3/").map(str::to_ascii_uppercase))
            .collect();
        for path in list("music").paths() {
            let name = format!("{}.MP3", stem(&list("music"), path).to_ascii_uppercase());
            assert!(mp3.contains(&name), "{path}");
        }
        // The textures: the records of their paks at their indices.
        for (pak, dir) in crate::textures::PAKS {
            let data = read_game_file(&game.join(pak)).unwrap();
            let pak = TexPak::parse(&data).unwrap();
            for path in list("textures").paths() {
                let Some(file) = path.strip_prefix(&format!("textures/{dir}/")) else {
                    continue;
                };
                let index: usize = file.split('-').next().unwrap().parse().unwrap();
                let record = pak.record(index).unwrap().unwrap();
                let name = crate::textures::file_name(index, record.header.name());
                assert_eq!(name, file, "{path}");
            }
        }
        // The models: model records of their paks at their indices.
        for (pak, dir) in crate::models::PAKS {
            let data = read_game_file(&game.join(pak)).unwrap();
            let pak = MdlPak::parse(&data).unwrap();
            for path in list("models").paths() {
                let Some(file) = path.strip_prefix(&format!("models/{dir}/")) else {
                    continue;
                };
                let index: usize = file.split('-').next().unwrap().parse().unwrap();
                let record = pak.record(index).unwrap();
                assert_eq!(record.kind, Kind::Model, "{path}");
                let name = crate::models::file_name(index, record.name(), record.kind);
                assert_eq!(name, file, "{path}");
            }
        }
    }

    /// Each dead sprite is a sprite of the world's, and no static of any sector draws it; no dead
    /// texture is a sheet or an atlas that the world's index names.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn no_static_draws_a_dead_sprite() {
        use opensacred_data::world::{INDEX, SPRITES_FILE, Sector, Sprites, StaticObject, World};
        fn drawn(statics: &[StaticObject], out: &mut HashSet<u32>) {
            for object in statics {
                out.insert(object.sprite);
                drawn(&object.children, out);
            }
        }
        let out = scratch_dir("dead-world");
        crate::world::convert(&opensacred_locate::original(), &out, None).unwrap();
        let dir = crate::world::output_dir(&out);
        let sprites = Sprites::decode(&fs::read(dir.join(SPRITES_FILE)).unwrap()).unwrap();
        let world: World = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        let mut statics = HashSet::new();
        for entry in &world.sectors {
            let sector = Sector::decode(&fs::read(dir.join(&entry.file)).unwrap()).unwrap();
            drawn(&sector.statics, &mut statics);
        }
        let sprites_list = list("sprites");
        for path in sprites_list.paths() {
            let sprite: u32 = stem(&sprites_list, path).parse().unwrap();
            let held = usize::try_from(sprite)
                .ok()
                .and_then(|i| sprites.sprites.get(i))
                .is_some_and(Option::is_some);
            assert!(held, "{path}: no sprite");
            assert!(!statics.contains(&sprite), "{path}: drawn");
        }
        let named: HashSet<&str> = world
            .sheets
            .iter()
            .chain(&world.atlases)
            .filter_map(|t| t.strip_prefix("../"))
            .collect();
        assert!(list("textures").paths().all(|path| !named.contains(path)));
        fs::remove_dir_all(out).unwrap();
    }

    /// No item's model resolves to a dead model's file, as the game resolves an item's name: the
    /// first file of the name in the models' order (`npcs::model_files`).
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn no_item_names_a_dead_model() {
        let game = opensacred_locate::original();
        let mut first: HashMap<String, String> = HashMap::new();
        for (pak, dir) in crate::models::PAKS {
            let data = read_game_file(&game.join(pak)).unwrap();
            let pak = MdlPak::parse(&data).unwrap();
            for index in 0..pak.len() {
                let record = pak.record(index).unwrap();
                if record.kind != Kind::Model {
                    continue;
                }
                let file = crate::models::file_name(index, record.name(), record.kind);
                let grn = file
                    .strip_suffix(".glb")
                    .and_then(|stem| stem.split_once('-'))
                    .map(|(_, name)| format!("{}.GRN", name.to_ascii_uppercase()))
                    .unwrap();
                first
                    .entry(grn)
                    .or_insert_with(|| format!("models/{dir}/{file}"));
            }
        }
        let out = scratch_dir("dead-items");
        crate::tables::convert(&game, &out).unwrap();
        let items = fs::read_to_string(
            crate::tables::output_dir(&out).join(opensacred_data::tables::ITEMS),
        )
        .unwrap();
        let items: Vec<opensacred_data::tables::Item> = ron::from_str(&items).unwrap();
        let dead: Vec<(&str, &String)> = items
            .iter()
            .filter_map(|item| {
                let file = first.get(&item.name.to_ascii_uppercase())?;
                is_dead(file).then_some((item.name.as_str(), file))
            })
            .collect();
        assert!(dead.is_empty(), "{dead:?}");
        fs::remove_dir_all(out).unwrap();
    }
}
