# Names of the project's own for the items without one

Texts (`opensacred-convert text`), 2026-10-26 (#685, of #403); the maintainer's decision of 2026-10-23 on #403 (words of the project's own) and of 2026-10-26 on #685 (the 300 that a kill can make first, by groups).

## The original

566 of the 4,883 `Weapon.pak` records have no name: the string whose id is the hash of the type in decimal is not in `global.res` (docs/02 §8).
300 of them have an `items.pak` slot and stand in the drops' lists of a category that a drop asks for (weapons, armour, shields, helmets, boots, belts, pauldrons, bracers, greaves, gloves and amulets), so that a kill can make them (#685).
Most are the Underworld's classes' sets and the Daemon's and the Dwarf's pieces and amulets.
Seen in the running original: such an item shows no name; worn, its tooltip begins with the line of the class it is made for, in the class's colour, then its price and levels; lying on the ground it shows no label.

## Ours

`opensacred-convert text` adds a name for each of the 300 (`crates/opensacred-convert/src/item_names.rs`), under the hash of its type, unless the file holds one:

- an armour piece or an amulet takes the class it is made for (by its model's name: `Daem` the Daemon's, `Dwarf` the Dwarf's, `DElve` the Dark Elf's, `elve` the Wood Elf's, `Glad` the Gladiator's, `mage` the Battle Mage's, `Sera` the Seraphim's, `VLady` the Vampiress's), a word for its family of models, and its piece by its category: Armour, Bracers, Greaves, Boots, Pauldrons, Helmet (Great Helm for the large ones), Belt and Gloves;
- an amulet its design (`_ED`, the finer of each, Noble);
- a weapon or a shield its model's look.

The pieces of one model share their name: 23 types are the Daemon's Plain Armour, which their levels and values tell apart.
One more reaches the player through the scripts and is named: the lung 1331, which the creatures 99 drop for quest 15072 (its `SetDrop`).

The other 147 nameless types with a slot stay nameless, read on the Seraphim's tree:
- 24 doors and a dwarven chest, which the scripts make and the original never names on the screen;
- five quest items (bones, a waterskin, innards), whose `CreateObj`s give them a name of their own (`res:ZIELOBJEKT_…`);
- seven weapons and shields of the guards (`Garde_*`), which only the scripts' weapons' pools hold;
- six rabbit outfits (`rab_*`), which only `hasenoutfit` makes, and no record calls it;
- 105 that no record names, which only a trader's stock or a hero's template could hand the player: they wait for the trade's reading (#1066).

| Name | Types |
|---|---|
| Angular Hammer | 1847 |
| Battle Mage's Runeweave Armour | 1429 |
| Battle Mage's Runeweave Bracers | 1428 |
| Battle Mage's Runeweave Greaves | 1431 |
| Battle Mage's Runeweave Helmet | 1430 |
| Battle Mage's Starweave Armour | 1424 |
| Battle Mage's Starweave Belt | 1423 |
| Battle Mage's Starweave Boots | 1425 |
| Battle Mage's Starweave Bracers | 1422 |
| Battle Mage's Starweave Greaves | 1427 |
| Battle Mage's Starweave Helmet | 1426 |
| Bright Axe | 1843 |
| Daemon's Air Amulet | 1386 |
| Daemon's Barbed Boots | 1240 |
| Daemon's Barbed Greaves | 1239 |
| Daemon's Barbed Pauldrons | 1241 |
| Daemon's Battle Sword | 1244 |
| Daemon's Bone Amulet | 1385 |
| Daemon's Brooch | 1398 |
| Daemon's Cinder Bracers | 3189 |
| Daemon's Cinder Pauldrons | 3195 |
| Daemon's Earth Amulet | 1384 |
| Daemon's Ember Armour | 1882 |
| Daemon's Ember Belt | 1881 |
| Daemon's Ember Bracers | 1880 |
| Daemon's Ember Greaves | 1884 |
| Daemon's Ember Helmet | 1883 |
| Daemon's Fire Amulet | 1399 |
| Daemon's Greatsword | 1245 |
| Daemon's Heavy Bow | 1377 |
| Daemon's Horned Armour | 1228 |
| Daemon's Infernal Armour | 3645 |
| Daemon's Infernal Belt | 3644 |
| Daemon's Infernal Boots | 3649 |
| Daemon's Infernal Bracers | 3643 |
| Daemon's Infernal Gloves | 3646 |
| Daemon's Infernal Greaves | 3648 |
| Daemon's Infernal Helmet | 3647 |
| Daemon's Infernal Pauldrons | 3650 |
| Daemon's Moon Amulet | 1400 |
| Daemon's Noble Brooch | 1382 |
| Daemon's Noble Moon Amulet | 1387 |
| Daemon's Noble Skull Amulet | 1401 |
| Daemon's Noble Spiral Amulet | 1391 |
| Daemon's Noble Star Amulet | 1393 |
| Daemon's Noble Sun Amulet | 1389 |
| Daemon's Plain Armour | 1935–1957 |
| Daemon's Plain Belt | 1905–1910 |
| Daemon's Plain Bracers | 1899–1904 |
| Daemon's Plain Great Helm | 1959, 1961 |
| Daemon's Plain Greaves | 1962–1967 |
| Daemon's Plain Helmet | 1960 |
| Daemon's Plain Pauldrons | 1968–1973 |
| Daemon's Seraph Amulet | 1395 |
| Daemon's Shadow Amulet | 1383 |
| Daemon's Shield | 1246 |
| Daemon's Skull Amulet | 1394 |
| Daemon's Spiral Amulet | 1390 |
| Daemon's Star Amulet | 1392 |
| Daemon's Sun Amulet | 1388 |
| Daemon's Sword | 1242 |
| Daemon's Warshield | 1247 |
| Daemon's Warsword | 1243 |
| Daemon's Water Amulet | 1402 |
| Dark Axe | 1844 |
| Dark Elf's Nightweave Armour | 1251 |
| Dark Elf's Nightweave Bracers | 1250 |
| Dark Elf's Nightweave Greaves | 1253 |
| Dark Elf's Nightweave Helmet | 1252 |
| Dark Elf's Nightweave Pauldrons | 1254 |
| Dark Elf's Shade Armour | 1257, 1264 |
| Dark Elf's Shade Belt | 1256 |
| Dark Elf's Shade Boots | 1260 |
| Dark Elf's Shade Bracers | 1255 |
| Dark Elf's Shade Greaves | 1259 |
| Dark Elf's Shade Helmet | 1258 |
| Dark Elf's Shade Pauldrons | 1261 |
| Dark Elf's Umbral Belt | 1263 |
| Dark Elf's Umbral Boots | 1267 |
| Dark Elf's Umbral Bracers | 1262 |
| Dark Elf's Umbral Greaves | 1266 |
| Dark Elf's Umbral Helmet | 1265 |
| Dwarf's Air Amulet | 1409 |
| Dwarf's Blackened Armour | 3632 |
| Dwarf's Blackened Belt | 3631 |
| Dwarf's Blackened Boots | 3633 |
| Dwarf's Blackened Gloves | 3634 |
| Dwarf's Blackened Helmet | 3635 |
| Dwarf's Blackened Pauldrons | 3636 |
| Dwarf's Bone Amulet | 1408 |
| Dwarf's Brooch | 1403 |
| Dwarf's Curious Armour | 3638 |
| Dwarf's Curious Belt | 3637 |
| Dwarf's Curious Boots | 3639 |
| Dwarf's Curious Gloves | 3640 |
| Dwarf's Curious Helmet | 3641 |
| Dwarf's Curious Pauldrons | 3642 |
| Dwarf's Earth Amulet | 1406 |
| Dwarf's Fire Amulet | 1407 |
| Dwarf's Forge Armour | 3658 |
| Dwarf's Forge Boots | 3661 |
| Dwarf's Forge Gloves | 3659 |
| Dwarf's Forge Helmet | 3660 |
| Dwarf's Forge Pauldrons | 3662 |
| Dwarf's Iron Armour | 3626 |
| Dwarf's Iron Belt | 3625 |
| Dwarf's Iron Boots | 3627 |
| Dwarf's Iron Gloves | 3628 |
| Dwarf's Iron Helmet | 3629 |
| Dwarf's Iron Pauldrons | 3630 |
| Dwarf's Leather Armour | 3009 |
| Dwarf's Leather Belt | 3008 |
| Dwarf's Leather Boots | 3010 |
| Dwarf's Leather Gloves | 3011 |
| Dwarf's Leather Helmet | 3012 |
| Dwarf's Leather Pauldrons | 3013 |
| Dwarf's Moon Amulet | 1410 |
| Dwarf's Noble Brooch | 1404 |
| Dwarf's Noble Moon Amulet | 1411 |
| Dwarf's Noble Skull Amulet | 1420 |
| Dwarf's Noble Spiral Amulet | 1416 |
| Dwarf's Noble Star Amulet | 1418 |
| Dwarf's Noble Sun Amulet | 1414 |
| Dwarf's Plate Armour | 3552 |
| Dwarf's Plate Belt | 3551 |
| Dwarf's Plate Boots | 3553 |
| Dwarf's Plate Gloves | 3554 |
| Dwarf's Plate Helmet | 3555 |
| Dwarf's Plate Pauldrons | 3556 |
| Dwarf's Runic Armour | 1378 |
| Dwarf's Runic Boots | 1380 |
| Dwarf's Runic Helmet | 1379 |
| Dwarf's Runic Pauldrons | 1381 |
| Dwarf's Seraph Amulet | 1412 |
| Dwarf's Shadow Amulet | 1405 |
| Dwarf's Skull Amulet | 1419 |
| Dwarf's Spiral Amulet | 1415 |
| Dwarf's Star Amulet | 1417 |
| Dwarf's Sturdy Armour | 3869–3877 |
| Dwarf's Sturdy Belt | 3863–3868 |
| Dwarf's Sturdy Boots | 3878–3883 |
| Dwarf's Sturdy Gloves | 3884–3885, 3887–3890 |
| Dwarf's Sturdy Great Helm | 3891–3892 |
| Dwarf's Sturdy Helmet | 3894–3896 |
| Dwarf's Sturdy Pauldrons | 3897–3902 |
| Dwarf's Sun Amulet | 1413 |
| Dwarf's Water Amulet | 1421 |
| Dwarven Warhammer | 1711, 1846 |
| Fine Pistol | 1856 |
| Gladiator's Arena Armour | 3017 |
| Gladiator's Arena Belt | 1987, 1992, 1997 |
| Gladiator's Arena Boots | 1989, 1994, 1999 |
| Gladiator's Arena Bracers | 1986, 1991, 1996 |
| Gladiator's Arena Helmet | 1988, 1993, 1998 |
| Gladiator's Arena Pauldrons | 1990, 1995, 3000 |
| Gladiator's Lion Armour | 1269 |
| Gladiator's Lion Belt | 1268 |
| Gladiator's Lion Boots | 1272 |
| Gladiator's Lion Greaves | 1271 |
| Gladiator's Lion Helmet | 1270 |
| Gladiator's Lion Pauldrons | 1273 |
| Heavy Axe | 1845 |
| Heavy Hammer | 1848 |
| Heavy Musket | 1854 |
| Leather Shield | 3014 |
| Long Musket | 1853 |
| Lung | 1331 |
| Many-Barrelled Pistol | 1855 |
| Musket | 1849 |
| Plate Shield | 3096 |
| Seraphim's Black Armour | 3003 |
| Seraphim's Black Belt | 3002 |
| Seraphim's Black Boots | 3006 |
| Seraphim's Black Bracers | 3001, 3004 |
| Seraphim's Black Helmet | 3005 |
| Seraphim's Black Pauldrons | 3007 |
| Simple Pistol | 1857 |
| Vampiress's Crimson Armour | 3024 |
| Vampiress's Crimson Belt | 3023 |
| Vampiress's Crimson Boots | 3027 |
| Vampiress's Crimson Bracers | 3022 |
| Vampiress's Crimson Greaves | 3026 |
| Vampiress's Crimson Helmet | 3025 |
| Vampiress's Crimson Pauldrons | 3028 |
| Vampiress's Midnight Armour | 3031 |
| Vampiress's Midnight Belt | 3030 |
| Vampiress's Midnight Boots | 3034 |
| Vampiress's Midnight Bracers | 3029 |
| Vampiress's Midnight Greaves | 3033 |
| Vampiress's Midnight Helmet | 3032 |
| Wood Elf's Leafwoven Armour | 1976 |
| Wood Elf's Leafwoven Belt | 1975 |
| Wood Elf's Leafwoven Boots | 1979 |
| Wood Elf's Leafwoven Bracers | 1974 |
| Wood Elf's Leafwoven Greaves | 1978 |
| Wood Elf's Leafwoven Helmet | 1977 |
| Wood Elf's Mossgreen Armour | 1887 |
| Wood Elf's Mossgreen Belt | 1886 |
| Wood Elf's Mossgreen Boots | 1890 |
| Wood Elf's Mossgreen Bracers | 1885 |
| Wood Elf's Mossgreen Greaves | 1889 |
| Wood Elf's Mossgreen Helmet | 1888 |
| Wood Elf's Silverbark Armour | 1894 |
| Wood Elf's Silverbark Belt | 1893 |
| Wood Elf's Silverbark Boots | 1897 |
| Wood Elf's Silverbark Bracers | 1892 |
| Wood Elf's Silverbark Greaves | 1896 |
| Wood Elf's Silverbark Helmet | 1895 |
| Wood Elf's Thornguard Armour | 1982 |
| Wood Elf's Thornguard Belt | 1981 |
| Wood Elf's Thornguard Boots | 1985 |
| Wood Elf's Thornguard Bracers | 1980 |
| Wood Elf's Thornguard Greaves | 1984 |
| Wood Elf's Thornguard Helmet | 1983 |

## Why

The player meets these items as drops of the play, and the original shows them with no name, which reads as a fault.
The names are words of the project's own, never another release's wording (#403); the maintainer changes any of them through an issue.
