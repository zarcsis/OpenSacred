//! Names of the project's own for the item types that `global.res` leaves without one and that a
//! kill can make (#685, the maintainer's decision of 2026-10-23 on #403;
//! docs/decisions/2026-10-26-item-names.md): the 300 `Weapon.pak` records whose name's string the
//! file lacks, each with an `items.pak` slot and listed among the drops of a category that a drop
//! asks for (docs/re/items.md, "Drops"). The original shows such an item with no name, its
//! tooltip beginning with its class's line (#685). A name is the class the item is made for, the
//! word of its family of models and its piece by its category, one name for one model; the
//! weapons and the shields by their models. And the one more that the scripts drop: the lung of
//! quest 15072's `SetDrop`. `text` adds each under the hash of its type in decimal (docs/02 §6.2,
//! §8), unless the file holds it.

/// The types and their names, each with its model.
pub const ITEM_NAMES: [(u32, &str); NAMES] = [
    (1228, "Daemon's Horned Armour"),        // Daemonia_Armor02_Body.grn
    (1239, "Daemon's Barbed Greaves"),       // Daemonia_Armor03_Legs.grn
    (1240, "Daemon's Barbed Boots"),         // Daemonia_Armor03_Shoes.grn
    (1241, "Daemon's Barbed Pauldrons"),     // Daemonia_Armor03_Shoulder.grn
    (1242, "Daemon's Sword"),                // daem_schwert01.grn
    (1243, "Daemon's Warsword"),             // daem_schwert02.grn
    (1244, "Daemon's Battle Sword"),         // daem_schwert03.grn
    (1245, "Daemon's Greatsword"),           // daem_schwert04.grn
    (1246, "Daemon's Shield"),               // daem_shield01.grn
    (1247, "Daemon's Warshield"),            // daem_shield02.grn
    (1250, "Dark Elf's Nightweave Bracers"), // DElve_Sa5_arms.grn
    (1251, "Dark Elf's Nightweave Armour"),  // DElve_Sa5_body.grn
    (1252, "Dark Elf's Nightweave Helmet"),  // DElve_Sa5_helm.grn
    (1253, "Dark Elf's Nightweave Greaves"), // DElve_Sa5_legs.grn
    (1254, "Dark Elf's Nightweave Pauldrons"), // DElve_Sa5_shoulder.grn
    (1255, "Dark Elf's Shade Bracers"),      // DElve_Sa6_arms.grn
    (1256, "Dark Elf's Shade Belt"),         // DElve_Sa6_belt.grn
    (1257, "Dark Elf's Shade Armour"),       // DElve_Sa6_body.grn
    (1258, "Dark Elf's Shade Helmet"),       // DElve_Sa6_helm.grn
    (1259, "Dark Elf's Shade Greaves"),      // DElve_Sa6_legs.grn
    (1260, "Dark Elf's Shade Boots"),        // DElve_Sa6_shoes.grn
    (1261, "Dark Elf's Shade Pauldrons"),    // DElve_Sa6_shoulder.grn
    (1262, "Dark Elf's Umbral Bracers"),     // DElve_Sa7_arms.grn
    (1263, "Dark Elf's Umbral Belt"),        // DElve_Sa7_belt.grn
    (1264, "Dark Elf's Shade Armour"),       // DElve_Sa6_body.grn
    (1265, "Dark Elf's Umbral Helmet"),      // DElve_Sa7_helm.grn
    (1266, "Dark Elf's Umbral Greaves"),     // DElve_Sa7_legs.grn
    (1267, "Dark Elf's Umbral Boots"),       // DElve_Sa7_shoes.grn
    (1268, "Gladiator's Lion Belt"),         // Glad_SA7_belt.grn
    (1269, "Gladiator's Lion Armour"),       // Glad_SA7_body.grn
    (1270, "Gladiator's Lion Helmet"),       // Glad_SA7_helm.grn
    (1271, "Gladiator's Lion Greaves"),      // Glad_SA7_legs.grn
    (1272, "Gladiator's Lion Boots"),        // Glad_SA7_shoes.grn
    (1273, "Gladiator's Lion Pauldrons"),    // Glad_SA7_shoulder.grn
    (1377, "Daemon's Heavy Bow"),            // Daem_heavybow.grn
    (1378, "Dwarf's Runic Armour"),          // Dwarf_SA1_Body.grn
    (1379, "Dwarf's Runic Helmet"),          // Dwarf_SA1_Helm.grn
    (1380, "Dwarf's Runic Boots"),           // Dwarf_SA1_Shoes.grn
    (1381, "Dwarf's Runic Pauldrons"),       // Dwarf_SA1_Shoulder.grn
    (1382, "Daemon's Noble Brooch"),         // Daem_Brosche_ED.grn
    (1383, "Daemon's Shadow Amulet"),        // Daem_AM_Dunkelelf.grn
    (1384, "Daemon's Earth Amulet"),         // Daem_AM_Erde.grn
    (1385, "Daemon's Bone Amulet"),          // Daem_AM_Knochen.grn
    (1386, "Daemon's Air Amulet"),           // Daem_AM_Luft.grn
    (1387, "Daemon's Noble Moon Amulet"),    // Daem_AM_Mond_ED.grn
    (1388, "Daemon's Sun Amulet"),           // Daem_AM_Sonne.grn
    (1389, "Daemon's Noble Sun Amulet"),     // Daem_AM_Sonne_ED.grn
    (1390, "Daemon's Spiral Amulet"),        // Daem_AM_Spiral.grn
    (1391, "Daemon's Noble Spiral Amulet"),  // Daem_AM_Spiral_ED.grn
    (1392, "Daemon's Star Amulet"),          // Daem_AM_Stern.grn
    (1393, "Daemon's Noble Star Amulet"),    // Daem_AM_Stern_ED.grn
    (1394, "Daemon's Skull Amulet"),         // Daem_AM_Totenkopf.grn
    (1395, "Daemon's Seraph Amulet"),        // Daem_AM_Seraphim.grn
    (1398, "Daemon's Brooch"),               // Daem_Brosche.grn
    (1399, "Daemon's Fire Amulet"),          // Daem_AM_Fire.grn
    (1400, "Daemon's Moon Amulet"),          // Daem_AM_Mond.grn
    (1401, "Daemon's Noble Skull Amulet"),   // Daem_AM_Totenkopf_ED.grn
    (1402, "Daemon's Water Amulet"),         // Daem_AM_Wasser.grn
    (1403, "Dwarf's Brooch"),                // Dwarf_AM_Brosche.grn
    (1404, "Dwarf's Noble Brooch"),          // Dwarf_AM_Brosche_ED.grn
    (1405, "Dwarf's Shadow Amulet"),         // Dwarf_AM_Dunkelelf.grn
    (1406, "Dwarf's Earth Amulet"),          // Dwarf_AM_Erde.grn
    (1407, "Dwarf's Fire Amulet"),           // Dwarf_AM_feuer.grn
    (1408, "Dwarf's Bone Amulet"),           // Dwarf_AM_Knochen.grn
    (1409, "Dwarf's Air Amulet"),            // Dwarf_AM_Luft.grn
    (1410, "Dwarf's Moon Amulet"),           // Dwarf_AM_Mond.grn
    (1411, "Dwarf's Noble Moon Amulet"),     // Dwarf_AM_Mond_ED.grn
    (1412, "Dwarf's Seraph Amulet"),         // Dwarf_AM_Seraphim.grn
    (1413, "Dwarf's Sun Amulet"),            // Dwarf_AM_Sonne.grn
    (1414, "Dwarf's Noble Sun Amulet"),      // Dwarf_AM_Sonne_ED.grn
    (1415, "Dwarf's Spiral Amulet"),         // Dwarf_AM_Spiral.grn
    (1416, "Dwarf's Noble Spiral Amulet"),   // Dwarf_AM_Spiral_ED.grn
    (1417, "Dwarf's Star Amulet"),           // Dwarf_AM_Stern.grn
    (1418, "Dwarf's Noble Star Amulet"),     // Dwarf_AM_Stern_ED.grn
    (1419, "Dwarf's Skull Amulet"),          // Dwarf_AM_Totenkopf.grn
    (1420, "Dwarf's Noble Skull Amulet"),    // Dwarf_AM_Totenkopf_ED.grn
    (1421, "Dwarf's Water Amulet"),          // Dwarf_AM_Wasser.grn
    (1422, "Battle Mage's Starweave Bracers"), // mage_sa5_arms.grn
    (1423, "Battle Mage's Starweave Belt"),  // mage_sa5_belt.grn
    (1424, "Battle Mage's Starweave Armour"), // mage_sa5_body.grn
    (1425, "Battle Mage's Starweave Boots"), // mage_sa5_boots.grn
    (1426, "Battle Mage's Starweave Helmet"), // mage_sa5_helm.grn
    (1427, "Battle Mage's Starweave Greaves"), // mage_sa5_legs.grn
    (1428, "Battle Mage's Runeweave Bracers"), // mage_sa6_arms.grn
    (1429, "Battle Mage's Runeweave Armour"), // mage_sa6_body.grn
    (1430, "Battle Mage's Runeweave Helmet"), // mage_sa6_helm.grn
    (1431, "Battle Mage's Runeweave Greaves"), // mage_sa6_legs.grn
    (1711, "Dwarven Warhammer"),             // DWARF_WARHAMMER.GRN
    (1843, "Bright Axe"),                    // Axe_bright.grn
    (1844, "Dark Axe"),                      // Axe_dark.grn
    (1845, "Heavy Axe"),                     // Axe_heavy.grn
    (1846, "Dwarven Warhammer"),             // dwarf_warhammer.grn
    (1847, "Angular Hammer"),                // Hammer_eckig.grn
    (1848, "Heavy Hammer"),                  // Hammer_heavy.grn
    (1849, "Musket"),                        // muskette_1.grn
    (1853, "Long Musket"),                   // muskette_2.grn
    (1854, "Heavy Musket"),                  // muskette_3.grn
    (1855, "Many-Barrelled Pistol"),         // Pistol_multi.grn
    (1856, "Fine Pistol"),                   // Pistol_nice.grn
    (1857, "Simple Pistol"),                 // Pistol_simple.grn
    (1880, "Daemon's Ember Bracers"),        // Daem_Sa1_arms.grn
    (1881, "Daemon's Ember Belt"),           // Daem_Sa1_belt.grn
    (1882, "Daemon's Ember Armour"),         // Daem_Sa1_body.grn
    (1883, "Daemon's Ember Helmet"),         // Daem_Sa1_helm.grn
    (1884, "Daemon's Ember Greaves"),        // Daem_Sa1_legs.grn
    (1885, "Wood Elf's Mossgreen Bracers"),  // elve_sa7_arms.grn
    (1886, "Wood Elf's Mossgreen Belt"),     // elve_sa7_belt.grn
    (1887, "Wood Elf's Mossgreen Armour"),   // elve_sa7_body.grn
    (1888, "Wood Elf's Mossgreen Helmet"),   // elve_sa7_helm.grn
    (1889, "Wood Elf's Mossgreen Greaves"),  // elve_sa7_legs.grn
    (1890, "Wood Elf's Mossgreen Boots"),    // elve_sa7_shoes.grn
    (1892, "Wood Elf's Silverbark Bracers"), // elve_sa8_arms.grn
    (1893, "Wood Elf's Silverbark Belt"),    // elve_sa8_belt.grn
    (1894, "Wood Elf's Silverbark Armour"),  // elve_sa8_body.grn
    (1895, "Wood Elf's Silverbark Helmet"),  // elve_sa8_helm.grn
    (1896, "Wood Elf's Silverbark Greaves"), // elve_sa8_legs.grn
    (1897, "Wood Elf's Silverbark Boots"),   // elve_sa8_shoes.grn
    (1899, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1900, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1901, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1902, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1903, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1904, "Daemon's Plain Bracers"),        // Daem_std_arms.grn
    (1905, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1906, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1907, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1908, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1909, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1910, "Daemon's Plain Belt"),           // Daem_std_belt.grn
    (1935, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1936, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1937, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1938, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1939, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1940, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1941, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1942, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1943, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1944, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1945, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1946, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1947, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1948, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1949, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1950, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1951, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1952, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1953, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1954, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1955, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1956, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1957, "Daemon's Plain Armour"),         // Daem_std_body.grn
    (1959, "Daemon's Plain Great Helm"),     // Daem_std_helm_l.grn
    (1960, "Daemon's Plain Helmet"),         // Daem_std_helm_m.grn
    (1961, "Daemon's Plain Great Helm"),     // Daem_std_helm_l.grn
    (1962, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1963, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1964, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1965, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1966, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1967, "Daemon's Plain Greaves"),        // Daem_std_legs.grn
    (1968, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1969, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1970, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1971, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1972, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1973, "Daemon's Plain Pauldrons"),      // Daem_std_shoulder.grn
    (1974, "Wood Elf's Leafwoven Bracers"),  // elve_Sa5_arms.grn
    (1975, "Wood Elf's Leafwoven Belt"),     // elve_Sa5_belt.grn
    (1976, "Wood Elf's Leafwoven Armour"),   // elve_Sa5_body.grn
    (1977, "Wood Elf's Leafwoven Helmet"),   // elve_Sa5_helm.grn
    (1978, "Wood Elf's Leafwoven Greaves"),  // elve_Sa5_legs.grn
    (1979, "Wood Elf's Leafwoven Boots"),    // elve_sa5_shoes.grn
    (1980, "Wood Elf's Thornguard Bracers"), // elve_Sa6_arms.grn
    (1981, "Wood Elf's Thornguard Belt"),    // elve_Sa6_belt.grn
    (1982, "Wood Elf's Thornguard Armour"),  // elve_Sa6_body.grn
    (1983, "Wood Elf's Thornguard Helmet"),  // elve_Sa6_helm.grn
    (1984, "Wood Elf's Thornguard Greaves"), // elve_Sa6_legs.grn
    (1985, "Wood Elf's Thornguard Boots"),   // elve_sa6_shoes.grn
    (1986, "Gladiator's Arena Bracers"),     // Glad_SA5_Arm.grn
    (1987, "Gladiator's Arena Belt"),        // Glad_SA5_belt.grn
    (1988, "Gladiator's Arena Helmet"),      // Glad_SA5_helm.grn
    (1989, "Gladiator's Arena Boots"),       // Glad_SA5_shoes.grn
    (1990, "Gladiator's Arena Pauldrons"),   // Glad_SA5_shoulder.grn
    (1991, "Gladiator's Arena Bracers"),     // Glad_SA5_Arm.grn
    (1992, "Gladiator's Arena Belt"),        // Glad_SA5_belt.grn
    (1993, "Gladiator's Arena Helmet"),      // Glad_SA5_helm.grn
    (1994, "Gladiator's Arena Boots"),       // Glad_SA5_shoes.grn
    (1995, "Gladiator's Arena Pauldrons"),   // Glad_SA5_shoulder.grn
    (1996, "Gladiator's Arena Bracers"),     // Glad_SA5_Arm.grn
    (1997, "Gladiator's Arena Belt"),        // Glad_SA5_belt.grn
    (1998, "Gladiator's Arena Helmet"),      // Glad_SA5_helm.grn
    (1999, "Gladiator's Arena Boots"),       // Glad_SA5_shoes.grn
    (3000, "Gladiator's Arena Pauldrons"),   // Glad_SA5_shoulder.grn
    (3001, "Seraphim's Black Bracers"),      // Sera_black_arms.grn
    (3002, "Seraphim's Black Belt"),         // Sera_black_belt.grn
    (3003, "Seraphim's Black Armour"),       // Sera_black_body.grn
    (3004, "Seraphim's Black Bracers"),      // Sera_black_gloves.grn
    (3005, "Seraphim's Black Helmet"),       // Sera_black_helm.grn
    (3006, "Seraphim's Black Boots"),        // Sera_black_legs.grn
    (3007, "Seraphim's Black Pauldrons"),    // Sera_black_shoulder.grn
    (3008, "Dwarf's Leather Belt"),          // Dwarf_leather_belt.grn
    (3009, "Dwarf's Leather Armour"),        // Dwarf_leather_body.grn
    (3010, "Dwarf's Leather Boots"),         // Dwarf_leather_boots.grn
    (3011, "Dwarf's Leather Gloves"),        // Dwarf_leather_gloves.grn
    (3012, "Dwarf's Leather Helmet"),        // Dwarf_leather_helmet.grn
    (3013, "Dwarf's Leather Pauldrons"),     // Dwarf_leather_shoulder.grn
    (3014, "Leather Shield"),                // leathershield.grn
    (3017, "Gladiator's Arena Armour"),      // Glad_SA5_body.grn
    (3022, "Vampiress's Crimson Bracers"),   // VLady_Sa1_arms.grn
    (3023, "Vampiress's Crimson Belt"),      // VLady_Sa1_belt.grn
    (3024, "Vampiress's Crimson Armour"),    // VLady_Sa1_body.grn
    (3025, "Vampiress's Crimson Helmet"),    // VLady_Sa1_helm.grn
    (3026, "Vampiress's Crimson Greaves"),   // VLady_Sa1_legs.grn
    (3027, "Vampiress's Crimson Boots"),     // VLady_Sa1_Shoes.grn
    (3028, "Vampiress's Crimson Pauldrons"), // VLady_Sa1_Shoulder.grn
    (3029, "Vampiress's Midnight Bracers"),  // VLady_SA2_arms.grn
    (3030, "Vampiress's Midnight Belt"),     // VLady_SA2_belt.grn
    (3031, "Vampiress's Midnight Armour"),   // VLady_SA2_body.grn
    (3032, "Vampiress's Midnight Helmet"),   // VLady_SA2_helm.grn
    (3033, "Vampiress's Midnight Greaves"),  // VLady_SA2_legs.grn
    (3034, "Vampiress's Midnight Boots"),    // VLady_SA2_Shoes.grn
    (3096, "Plate Shield"),                  // plateshield.grn
    (3189, "Daemon's Cinder Bracers"),       // Daem_Sa2_arms.grn
    (3195, "Daemon's Cinder Pauldrons"),     // Daem_Sa2_shoulder.grn
    (3551, "Dwarf's Plate Belt"),            // dwarf_platearmor_belt.grn
    (3552, "Dwarf's Plate Armour"),          // dwarf_platearmor_body.grn
    (3553, "Dwarf's Plate Boots"),           // dwarf_platearmor_boots.grn
    (3554, "Dwarf's Plate Gloves"),          // dwarf_platearmor_gloves.grn
    (3555, "Dwarf's Plate Helmet"),          // dwarf_platearmor_helmet.grn
    (3556, "Dwarf's Plate Pauldrons"),       // dwarf_platearmor_shoulder.grn
    (3625, "Dwarf's Iron Belt"),             // Dwarf_metal_belt.grn
    (3626, "Dwarf's Iron Armour"),           // Dwarf_metal_body.grn
    (3627, "Dwarf's Iron Boots"),            // Dwarf_metal_boots.grn
    (3628, "Dwarf's Iron Gloves"),           // Dwarf_metal_gloves.grn
    (3629, "Dwarf's Iron Helmet"),           // Dwarf_metal_helmet.grn
    (3630, "Dwarf's Iron Pauldrons"),        // Dwarf_metal_shoulder.grn
    (3631, "Dwarf's Blackened Belt"),        // Dwarf_black_belt.grn
    (3632, "Dwarf's Blackened Armour"),      // Dwarf_black_body.grn
    (3633, "Dwarf's Blackened Boots"),       // Dwarf_black_boots.grn
    (3634, "Dwarf's Blackened Gloves"),      // Dwarf_black_gloves.grn
    (3635, "Dwarf's Blackened Helmet"),      // Dwarf_black_helmet.grn
    (3636, "Dwarf's Blackened Pauldrons"),   // Dwarf_black_shoulder.grn
    (3637, "Dwarf's Curious Belt"),          // Dwarf_curious_belt.grn
    (3638, "Dwarf's Curious Armour"),        // Dwarf_curious_body.grn
    (3639, "Dwarf's Curious Boots"),         // Dwarf_curious_boots.grn
    (3640, "Dwarf's Curious Gloves"),        // Dwarf_curious_gloves.grn
    (3641, "Dwarf's Curious Helmet"),        // Dwarf_curious_helmet.grn
    (3642, "Dwarf's Curious Pauldrons"),     // Dwarf_curious_shoulder.grn
    (3643, "Daemon's Infernal Bracers"),     // Daemonia_Armor04_Arms.grn
    (3644, "Daemon's Infernal Belt"),        // Daemonia_Armor04_Belt.grn
    (3645, "Daemon's Infernal Armour"),      // Daemonia_Armor04_Body.grn
    (3646, "Daemon's Infernal Gloves"),      // Daemonia_Armor04_Gloves.grn
    (3647, "Daemon's Infernal Helmet"),      // Daemonia_Armor04_Helm.grn
    (3648, "Daemon's Infernal Greaves"),     // Daemonia_Armor04_Legs.grn
    (3649, "Daemon's Infernal Boots"),       // Daemonia_Armor04_Shoes.grn
    (3650, "Daemon's Infernal Pauldrons"),   // Daemonia_Armor04_Shoulder.grn
    (3658, "Dwarf's Forge Armour"),          // Dwarf_SA2_body.grn
    (3659, "Dwarf's Forge Gloves"),          // Dwarf_SA2_gloves.grn
    (3660, "Dwarf's Forge Helmet"),          // Dwarf_SA2_helm.grn
    (3661, "Dwarf's Forge Boots"),           // Dwarf_SA2_shoes.grn
    (3662, "Dwarf's Forge Pauldrons"),       // Dwarf_SA2_shoulder.grn
    (3863, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3864, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3865, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3866, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3867, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3868, "Dwarf's Sturdy Belt"),           // Dwarf_nl_belt.grn
    (3869, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3870, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3871, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3872, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3873, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3874, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3875, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3876, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3877, "Dwarf's Sturdy Armour"),         // Dwarf_nl_body.grn
    (3878, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3879, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3880, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3881, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3882, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3883, "Dwarf's Sturdy Boots"),          // Dwarf_nl_Boots.grn
    (3884, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3885, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3887, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3888, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3889, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3890, "Dwarf's Sturdy Gloves"),         // Dwarf_nl_gloves.grn
    (3891, "Dwarf's Sturdy Great Helm"),     // Dwarf_nl_Helm_L.grn
    (3892, "Dwarf's Sturdy Great Helm"),     // Dwarf_nl_Helm_L.grn
    (3894, "Dwarf's Sturdy Helmet"),         // Dwarf_nl_Helm_M.grn
    (3895, "Dwarf's Sturdy Helmet"),         // Dwarf_nl_Helm_M.grn
    (3896, "Dwarf's Sturdy Helmet"),         // Dwarf_nl_Helm_M.grn
    (3897, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    (3898, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    (3899, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    (3900, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    (3901, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    (3902, "Dwarf's Sturdy Pauldrons"),      // Dwarf_nl_Shoulder.grn
    // Quest 15072's `SetDrop`, the creatures 99 dropping it.
    (1331, "Lung"), // qi_gibs_lunge.grn
];

/// How many names there are.
const NAMES: usize = 301;
