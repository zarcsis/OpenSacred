# The sets' bonuses

What wearing pieces of an item set gives the hero in Sacred Gold 2.28, and what its tooltip says of it.
Read 2026-11-01 for #1727 by code: the disassembly of `0x5711F0`, `0x571710` (its 65 cases, jump table `0x578DC0`) and the recomputation `0x5796A0`, and an emulation of `0x571710` in Unicorn (the PE image at its base, `bin/Balance.bin` at `0xAD4490`, a fake stats block and creature, every set 1 to 70 at every count 0 to 10 and every difficulty 0 to 4, in both modes; the callees that write the stats or the text stubbed and recorded, the arithmetic run natively).
**Confirmed** by code and the emulation unless marked otherwise; nothing here is seen in the running original yet.
`D` is the difficulty's factor (`0x51CEF0`, `0x55E070`, `0x55E0F0`): 1.0, 1.34, 1.67, 2.0, 2.34 for `0x182EE44` 0 to 4; `⌊c·D⌋` is `0x55E070(c)`, the float product truncated (`__ftol`); `c·D` a float added as it is.
The stats block is the creature's `+0x3A8` (so the stats' `+0x148` is the creature's `+0x4F0`, `+0x14C`/`+0x150` its `+0x4F4`/`+0x4F8`).

## 1. Counting the pieces: the recomputation and `0x5711F0`

The recomputation `0x5796A0(text, flag)` (a `__thiscall` on the stats, `ret 8`) builds the list of worn set pieces and calls `0x5711F0` twice:

1. For the local hero only (the creature's `+0xC` equal to the local player's id, `0x7D84A0()+0x14`): the count vector (`+0x150`, its size `+0x14C`) is zeroed, the items' bonus list `0xAD4218` emptied, and the flags `+0x148` saved and cleared (`0x57970F`–`0x57978C`).
   For every creature: the type list `0xAD4224` (100 dwords), its id list `0xAD4008`, its count `0xAD43B8`, the per-set socket masks `+0x154…+0x1B7` (one byte per set number), the four dominance counters (a dword on the stack) and the creature's `+0x1F4` bit 31 are cleared (`0x579792`–`0x5797F7`).
2. The arts' bonus bytes (`+7` of each 22-byte record of the art list `+0x102`/`+0x106`) and the skills' bonus bytes (`+0x34…+0x3B`) are cleared (`0x579CB8`, `0x579CCF`).
3. The item loop (`0x579D22`–`0x57AD55`) over the slots −1 to 18: −1 the ridden mount's slot 0 (`0x552660`: the creature's `+0x1EC` while its `+0x14` bit 2 is set, of a type `0x4266F0` accepts), 0 to 18 the creature's 19 slots `+0x1A4 + 4·i` (`0x556F50`; the hands `+0x1D4`, `+0x1D8` are slots 12 and 13; an item whose type passes `0x426550` is skipped, not read).
   For each item, the item itself (k = −1) and each of its sockets k (`0x5BE990` their count, `0x5BE9E0(k)` filled, `0x5BEB30(k)` the socketed item's data):
   - its type's `Weapon.pak` record (`0x434030`) with a set number `+0x23` ≠ 0 is a set piece; its type (the object's `+0x10` where the data's type is below 1000) is appended to `0xAD4224` unless the list already holds it (the list holds distinct types, at most 100), and the object's id to `0xAD4008`;
   - for a socket k whose piece's set equals the set of the item that holds it, the byte `+0x154 + set` takes the bit `1 << k` (sets 60–65 test this byte for 7: sockets 0, 1 and 2 of a piece of the same set each hold another piece);
   - for the item itself only: the dominance counters (below), the items' damages and resistances, its bonus slots (the list `0xAD4218` at `0x57A42D`, local hero only) and its skills' and arts' bonuses (`0x57A9D3`, `0x57AA32`, `0x57AAC2`, `0x57ACD3`, `0x57AD17`).
4. The carried pieces of set 23 (`0x57AD5B`): for a creature with a player's inventory (`0x5DEBC0(id)` → `0x5DEC90`), its 256 entries (`0x5DA2D0(i, 0)`; **hypothesis**: the backpack) are searched for the types 5101 to 5106 (`0x13ED`–`0x13F2`), each appended once.
5. Only for a creature whose id is at most 16 (a player; `0x57AE56`): the creature's `+0x1FC` bits 4 and 8 are cleared, then `0x5711F0(&counters, 0xAD4224, 0)` applies the bonuses (`0x57AE80`) and, when `text` is given, `0x5711F0(&counters, 0xAD4224, text)` writes their text (`0x57AE9D`).
   A creature of a higher id (every NPC) gets no set bonus.
6. Right after (`0x57AEA2`–`0x57B09B`), for the local hero: set 23's count above 5 (all six) sets the creature's `+0x1FC` bit 1, else clears it, and a change of the bit calls `0x54D760(0x11A)` at the end; a Mage (type 3) with `+0x1FC` bit 8 (set 32) takes the effect 40 (`0x28`; `0x582D50(0x28, 0, 0, 1.5, 0x1F, …)`) once and the effect 11's record `+0x18` = 10.0, losing both without the bit; a Gladiator (type 2) with `+0x148` bit `0x20000` (set 43) takes the effect 40 with 1.5 and the art 1015 (`0x3F7`), losing it without the bit.
7. The rest of the recomputation follows: the attributes' parts (`0x57B32A` on), so the sets' attribute points count in them; the skills' effects (`0x566070` at `0x57C658`), so the sets' skill bonuses count in their curves; the floors of `+0x3C`, `+0x3E`, `+0x40` (80 at `0x57D1C0` or 100 at `0x57D299`, by a condition not read); and the events of the flags that changed against the saved ones (set 39's bit `0x2`: a weather event at `0x57D21B`; bit `0x40000000`: the effects keyed 1097 removed at `0x57C1B3`; bit `0x40000` at `0x57D38D`).

The vector itself: 100 zeroed bytes allocated for a creature whose id is below 16 as it is set up (`0x52E287`: `+0x4F8` the buffer, `+0x4F4` = 100); index `n` holds set n's count.
Its readers test the index against the size and read a dummy byte `0xAB78A0` (`0xAD3EEC` at `0x54A71D`) when it is out of range.

**`0x5711F0(counters, list, text)`** (`__thiscall` on the stats, `ret 0xC`):

- With `list` and `counters` both null and a `text`: it calls `0x5796A0(text, 1)` and returns (the equipment window's way in, below).
- Without `text` it first clears the creature's `+0xB0` bits `0x7800`.
- The dominance bonus (`counters`, four bytes built in the item loop at `0x57A1A7`–`0x57A2C3`: an item whose most damage of one kind, its record's `+0x24/+0x26/+0x28/+0x2A`, is non-zero and strictly greater than the three others adds 1 to that kind's byte, and the same again for its resistances `+0x32…+0x38`; a tie adds nothing): the physical byte above 7, or the fire, magic or poison byte above 5, sets the creature's `+0xB0` bit `0x800`, `0x1000`, `0x2000` or `0x4000` and its `+0x1F4` bit 31, and without `text` adds 0.15 (`0x8907E8`, not times D) to that kind's resistance factor `+0xD6`, `+0xDA`, `+0xDE` or `+0xE2`; with `text` it writes the line `%s %+d%%\n` (`0x9577FC`) of the key `INVENTAR_RES_PHYSICAL`, `_FIRE`, `_MAGIC` or `_POISON` and 15 instead.
- The pieces: 256 byte counters zeroed; for each entry of `list` (`0xAD43B8` of them) whose `Weapon.pak` record has a set number s, `count[s] += 1` and the creature's `+0x1F4` bit 31 is set (the equipment window's set icon, docs/re/inventory.md).
- Then for s = 1 to 255 with `count[s]` non-zero, `0x571710(s, count[s], text)`; its answers are summed in the text mode and the sum dropped.

Callers of `0x5711F0`: `0x57AE80` and `0x57AE9D` in the recomputation, and `0x6B7F0F`, the equipment window's hover of the set icon: `0x5711F0(0, 0, &text)` on the hero's stats, so a recomputation with the text, then, the text not empty, a tooltip `0x756F40(512, 256, text, 0, 0x8B, 1)` (`0x808B` for a text above 700 characters) shown by `0x75B390`.
`0x571710` has no other caller; no item tooltip and no statistics window line calls either (the statistics window reads the vector directly, set 28 below).

## 2. `0x571710(set, count, text)` (`__thiscall` on the stats, `ret 0xC`)

- A set number at or above the sets' table's size (`[0xAAB5E4]+0x563244…+0x563248`, records of 0x70 bytes) answers 0 at once.
- Apply mode (`text` null): `count` is stored at `vector[set]` (the dummy when `set` ≥ `+0x14C`) **before** the switch, so every set's count is kept, set 23's too; then the case adds its bonuses.
- Text mode: nothing is stored or applied; it writes the header and the case's lines for the pieces reached:
  `\n`, `\cffffd070` (`0x957BEC`), the text of the key `SetName<n>` (`0x957C04`), `\n`, then for each piece of the set's record (its count at `+0x2C`, its types from `+0x00`) `\c%x%s\n` (`0x957BDC`) in `0xFF15FF25` where `0xAD4224` holds the type, else `0xFFFF0000`, with the text keyed by the type's decimal number (`_itoa`, `0x672CF0`).
  The lines use `0x672C90(id)` (a text by number, `T<id>` below) and `0x672CF0(key)` (a text by key, `K:<key>`) and `swprintf` (`0x84B7E4`); the formats are in the table's text column (the D0 line, then the D4 line).
- The switch is over `set − 1` up to 64 (sets 1–65); 23 and every number above 65 have no case; 33 to 37 share one.
- The answer is 1 for sets 1–22 once the first threshold is reached, else 0; nothing uses it.
- Adders: a skill's bonus goes to the first slot holding the skill (`0x5827E0(skill, v, 0)` or an inline search of `+0x24…+0x2B` and `0x5824A0(slot, v, 0)`), its byte `+0x34 + slot` held within 0 and 255, nothing where no slot holds it; an art's to its record's byte `+7` (`0x581F80(i, v, 0, 0)`, i the index in the art list, the arts found by their id `+4`; a bonus that would pass 255 is refused, not clamped); the flags by `or`.
- No case writes the items' bonus list `0xAD4218` or any global (the emulation recorded no write outside the stats block, the count vector and the creature's `+0x1FC`): the list holds only the items' own bonus slots, 12-byte entries `{condition, effect, Σ value}`.

## 3. The table of `0x571710`, per set and pieces

"pieces" is the count from which the row applies (each case tests `count ≥ n`); the values are the five difficulties'; "text" is the case's line in the text mode at D0 and D4; a row "text only" is a line `0x571710` writes without applying anything, its effect, where one exists, being applied by a reader of the count elsewhere (M-numbers, section 4).
Field names: a0…a5 the attributes `+0x10…+0x1A` (texts 1090–1095); m, m′, n the three damage multiplier arrays (docs/re/fight.md); the resistance factors `+0xD6…`; the kinds in the order physical, fire, magic, poison (texts 1078–1081).

| set | pieces | target field / kind | value or formula (D0/D1/D2/D3/D4) | text (D0 line; D4 line) | notes |
|---|---|---|---|---|---|
| 1 | 2 | — (text only in 0x571710) | — | 50% {T1079} {T1161} {T1180} ; 116% {T1079} {T1161} {T1180} | Text only here; applied in the strike (M1): fire ×(1 + 0.6·D) against creature kind 5 — the text says 50 %. |
| 1 | 5 | — (text only in 0x571710) | — | 30% {T1154} {T1161} {T1180} ; 70% {T1154} {T1161} {T1180} | Text only here; applied in the strike (M1): all four kinds ×(1 + 0.3·D) against kind 5. |
| 1 | 6 | poison resistance factor (+0xE2) | +0.6·D (0.6/0.804/1.002/1.2/1.404) | {K:INVENTAR_RES_POISON}: 60% ; {K:INVENTAR_RES_POISON}: 140%<br>+30% {K:UI_RARE_EXORZIST} ; +70% {K:UI_RARE_EXORZIST} | The second line is applied at the death (M5): the undead's rising chance ×(1 − 0.3·D). |
| 2 | 3 | a1 (+0x12) | +⌊10·D⌋ (10/13/16/20/23) | {T1091}: +10 ; {T1091}: +23<br>{T1093}: +10 ; {T1093}: +23 |  |
| 2 | 3 | a3 (+0x16) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 2 | 4 | skill 9's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9408}: +3 ; {T9408}: +7<br>{T9410}: +3 ; {T9410}: +7 |  |
| 2 | 4 | skill 11's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 2 | 5 | skill 10's bonus | +⌊2·D⌋ (2/2/3/4/4) | 10% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 23% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | The text announces a life leech; the code raises skill 10 instead, and no reader of set 2 exists: no leech. |
| 3 | 3 | arts 1014–1016: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T8221}: +2 ; {T8221}: +4<br>{T8220}: +2 ; {T8220}: +4<br>{T8222}: +2 ; {T8222}: +4 |  |
| 3 | 4 | flags +0x148 bit(s) | or 0x80000000 | {K:UI_KNOW_ENEMY} ; {K:UI_KNOW_ENEMY} |  |
| 3 | 6 | — (text only in 0x571710) | — | {T1143} 10% ; {T1143} 23% | Text only; no reader of set 3 at 6 pieces (T1143 is the effect 856's line, cf. set 30). |
| 3 | 8 | — (text only in 0x571710) | — | {T1124} 25% ; {T1124} 58% | Text only here; applied in the luck (M2) as ×(1 + 0.2·D), not 25 %·D. |
| 4 | 2 | skill 5's bonus | +⌊4·D⌋ (4/5/6/8/9) | {T9404}: +4 ; {T9404}: +9<br>{T9401}: +4 ; {T9401}: +9 | The text lists skills 5 and 2 at 2 pieces; skill 2 is applied only from 3 pieces. |
| 4 | 3 | skill 2's bonus | +⌊4·D⌋ (4/5/6/8/9) | — |  |
| 4 | 4 | — (text only in 0x571710) | — | {T1132} 5% ; {T1132} 11% | Text only here; applied in the strike's effect 859 (M3): +5·D. |
| 5 | 3 | magic damage mult. m (+0x9E) | +0.5·D (0.5/0.67/0.835/1/1.17) | 50% {T1154} {T1080} ; 116% {T1154} {T1080} |  |
| 5 | 5 | fire resistance factor (+0xDA) | +1·D (1/1.34/1.67/2/2.34) | 100% {T1079} {T1077} ; 233% {T1079} {T1077}<br>30% {T1154} {T1161} {T1181} ; 70% {T1154} {T1161} {T1181} | The second line is applied in the strike (M1): all kinds ×(1 + 0.3·D) against kind 10. |
| 6 | 3 | a4 (+0x18) | +⌊5·D⌋ (5/6/8/10/11) | {T1094}: +5 ; {T1094}: +11<br>{T1093}: +5 ; {T1093}: +11 | The text lists a4 and a3 at 3; a3 is applied from 4. |
| 6 | 4 | a3 (+0x16) | +⌊5·D⌋ (5/6/8/10/11) | — |  |
| 6 | 5 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23<br>{T1092}: +10 ; {T1092}: +23<br>{T1093}: +10 ; {T1093}: +23 | a3 gains a second ⌊10·D⌋ (total ⌊5·D⌋+⌊10·D⌋). |
| 6 | 5 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 6 | 5 | a3 (+0x16) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 6 | 8 | arts 15,18,20,22: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T817}: +2 ; {T817}: +4<br>{T804}: +2 ; {T804}: +4<br>{T816}: +2 ; {T816}: +4<br>{T819}: +2 ; {T819}: +4 |  |
| 6 | 9 | arts 13,17,19,21,1019: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | — | No text line for these. |
| 7 | 3 | attack speed (+0x3C) | +⌊20·D⌋ (20/26/33/40/46) | +20% {T1100} ; +46% {T1100} |  |
| 7 | 5 | skill 25's bonus | +⌊3·D⌋ (3/4/5/6/7) | +15% {T1074} ; +35% {T1074}<br>{T9424}: +3 ; {T9424}: +7 |  |
| 7 | 5 | A multiplier (+0xE6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | 〃 |  |
| 7 | 8 | — (text only in 0x571710) | — | +10% {T1132} ; +23% {T1132} | Text at 8; applied from 7 in the effect 859 (M3): +10·D. |
| 8 | 3 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23<br>{T1091}: + 8 ; {T1091}: + 18 |  |
| 8 | 3 | a1 (+0x12) | +⌊8·D⌋ (8/10/13/16/18) | 〃 |  |
| 8 | 5 | skill 4's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9403}: +3 ; {T9403}: +7 |  |
| 8 | 8 | — (text only in 0x571710) | — | 3% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 7% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no reader of set 8 (no leech). |
| 9 | 4 | skill 2's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9401}: +3 ; {T9401}: +7<br>{T9409}: +3 ; {T9409}: +7<br>{T9408}: +3 ; {T9408}: +7 |  |
| 9 | 4 | skill 9's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 9 | 4 | skill 10's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 9 | 7 | — (text only in 0x571710) | — | 20% {T1124} ; 46% {T1124} | Text only here; luck ×(1 + 0.2·D) from 7 (M2). |
| 9 | 8 | — (text only in 0x571710) | — | 6% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 14% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 10 | 3 | skill 7's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T1092}: +10 ; {T1092}: +23<br>{T9406}: +3 ; {T9406}: +7 |  |
| 10 | 3 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 10 | 5 | arts 1004–1008: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T8199}: +2 ; {T8199}: +4<br>{T8201}: +2 ; {T8201}: +4<br>{T8202}: +2 ; {T8202}: +4<br>{T8203}: +2 ; {T8203}: +4<br>{T8204}: +2 ; {T8204}: +4 |  |
| 11 | 3 | skill 2's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9401}: +3 ; {T9401}: +7<br>{T9402}: +3 ; {T9402}: +7 |  |
| 11 | 3 | skill 3's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 11 | 5 | a0 (+0x10) | +⌊20·D⌋ (20/26/33/40/46) | {T1090}: +20 ; {T1090}: +46<br>{T1091}: +10 ; {T1091}: +23 |  |
| 11 | 5 | a1 (+0x12) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 12 | 2 | skill 14's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9413}: +3 ; {T9413}: +7 |  |
| 12 | 4 | skill 19's bonus | +⌊2·D⌋ (2/2/3/4/4) | {T1094}: +10 ; {T1094}: +23<br>{T9418}: +3 ; {T9418}: +7 | Skill 19: the text shows ⌊3·D⌋, the code adds ⌊2·D⌋. |
| 12 | 4 | a4 (+0x18) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 12 | 6 | — (text only in 0x571710) | — | {T1138}: +2 ; {T1138}: +4 | Text only; no reader of set 12. |
| 13 | 3 | a2 (+0x14) | +⌊15·D⌋ (15/20/25/30/35) | {T1092}: +15 ; {T1092}: +35 |  |
| 13 | 5 | skill 26's bonus | +⌊2·D⌋ (2/2/3/4/4) | {T9426}: +3 ; {T9426}: +7<br>{T9425}: +3 ; {T9425}: +7 | Skills 26 and 27: the text shows ⌊3·D⌋, the code adds ⌊2·D⌋. |
| 13 | 5 | skill 27's bonus | +⌊2·D⌋ (2/2/3/4/4) | 〃 |  |
| 13 | 7 | arts 1049–1053: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T8256}: +2 ; {T8256}: +4<br>{T8255}: +2 ; {T8255}: +4<br>{T8254}: +2 ; {T8254}: +4<br>{T8253}: +2 ; {T8253}: +4<br>{T8252}: +2 ; {T8252}: +4 |  |
| 14 | 3 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | {T1092}: +10 ; {T1092}: +23 |  |
| 14 | 4 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23 |  |
| 14 | 5 | skill 25's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9424}: +3 ; {T9424}: +7 |  |
| 14 | 6 | skill 2's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9401}: +3 ; {T9401}: +7 |  |
| 14 | 7 | — (text only in 0x571710) | — | {T1137}: +2 ; {T1137}: +4 | Text only (all arts +2): 0x571710 applies nothing at 7 and no reader of set 14 exists. |
| 15 | 3 | a1 (+0x12) | +⌊10·D⌋ (10/13/16/20/23) | {T1091}: +10 ; {T1091}: +23<br>{T1092}: +10 ; {T1092}: +23 |  |
| 15 | 3 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 15 | 5 | skill 8's bonus | +⌊2·D⌋ (2/2/3/4/4) | {T9426}: +2 ; {T9426}: +4<br>{T9407}: +2 ; {T9407}: +4 |  |
| 15 | 5 | skill 27's bonus | +⌊2·D⌋ (2/2/3/4/4) | 〃 |  |
| 15 | 6 | — (text only in 0x571710) | — | 6% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 14% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 16 | 3 | skill 12's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9413}: +3 ; {T9413}: +7<br>{T9411}: +3 ; {T9411}: +7 |  |
| 16 | 3 | skill 14's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 16 | 5 | phys. resistance factor (+0xD6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | {K:INVENTAR_RES_PHYSICAL}: +15% ; {K:INVENTAR_RES_PHYSICAL}: +35% |  |
| 16 | 6 | — (text only in 0x571710) | — | {T1134}: +30 ; {T1134}: +70 | Text only here; applied in the strike (M4): the local defender's effect 866 + 30·D. |
| 17 | 2 | skill 2's bonus | +⌊4·D⌋ (4/5/6/8/9) | {T9403}: +4 ; {T9403}: +9<br>{T9401}: +4 ; {T9401}: +9 |  |
| 17 | 2 | skill 4's bonus | +⌊4·D⌋ (4/5/6/8/9) | 〃 |  |
| 17 | 4 | attack speed (+0x3C) | +⌊30·D⌋ (30/40/50/60/70) | +30% {T1100} ; +70% {T1100} |  |
| 17 | 5 | — (text only in 0x571710) | — | 6% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 14% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 18 | 3 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23 |  |
| 18 | 5 | skill 2's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9401}: +3 ; {T9401}: +7<br>{T9404}: +3 ; {T9404}: +7 |  |
| 18 | 5 | skill 3's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 18 | 7 | skill 11's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9410}: +3 ; {T9410}: +7<br>+30% {K:UI_RARE_EXORZIST} ; +70% {K:UI_RARE_EXORZIST} | The exorcism line is read at the death with count > 7 (M5), unreachable: set 18 has 7 pieces. |
| 19 | 2 | — (text only in 0x571710) | — | 3% {T1127} ; 7% {T1127} | Text only; no reader of set 19 at 2. |
| 19 | 4 | — (text only in 0x571710) | — | 6% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 14% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 19 | 5 | every art the creature holds: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T1137}: +2 ; {T1137}: +4<br>{T1071}: +10% ; {T1071}: +23% | Every art the creature holds. The experience line is applied by M6: ×(1 + 0.1·D). |
| 20 | 3 | skill 9's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9408}: +3 ; {T9408}: +7<br>{T9410}: +3 ; {T9410}: +7 |  |
| 20 | 3 | skill 11's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 20 | 4 | — (text only in 0x571710) | — | {T1132}: +10% ; {T1132}: +23% | Text only here; effect 859 + 10·D (M3). |
| 20 | 7 | A multiplier (+0xE6) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | {T1074}: +30% ; {T1074}: +70%<br>{T1075}: +30% ; {T1075}: +70% |  |
| 20 | 7 | D multiplier (+0xEA) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | 〃 |  |
| 21 | 3 | skill 9's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9419}: +3 ; {T9419}: +7<br>{T9408}: +3 ; {T9408}: +7 |  |
| 21 | 3 | skill 20's bonus | +⌊3·D⌋ (3/4/5/6/7) | 〃 |  |
| 21 | 5 | — (text only in 0x571710) | — | 10% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 23% {K:TOOLTIP_LEBEN_ABSAUGUNG2}<br>25% {T1124} ; 58% {T1124} | Text only. The leech has no reader; the luck reader (M2) needs 6 pieces, unreachable (5 pieces), and gives 20 %·D. |
| 22 | 3 | fire resistance (+0x6A) | +⌊30·D⌋ (30/40/50/60/70) | {K:INVENTAR_RES_FIRE}: +30 ; {K:INVENTAR_RES_FIRE}: +70 | Stored as the float of ⌊30·D⌋. M7 (the potion drop) reads set 22 at 5 without a line. |
| 22 | 5 | fire weapon mult. m′ (+0xAA) | +1·D (1/1.34/1.67/2/2.34) | {K:INVENTAR_RES_FIRE}: +100% ; {K:INVENTAR_RES_FIRE}: +233%<br>{T1079}: +100% ; {T1079}: +233% |  |
| 22 | 5 | fire resistance factor (+0xDA) | +1·D (1/1.34/1.67/2/2.34) | 〃 |  |
| 23 | 1–6 | — (no case; the count alone is stored) | — | — (header only) | No case: only the count is stored; read by the recomputation (M8). |
| 24 | 3 | phys. spell mult. n (+0xB6) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | {T1076}: +30 ; {T1076}: +70 |  |
| 24 | 3 | fire spell mult. n (+0xBA) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | 〃 |  |
| 24 | 3 | magic spell mult. n (+0xBE) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | 〃 |  |
| 24 | 3 | poison spell mult. n (+0xC2) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | 〃 |  |
| 24 | 5 | phys. least (+0x86) and phys. most (+0x76) | +⌊a0·⌊40·D⌋/100⌋ (as a float) | {T1154}: +40% {T1090} ; {T1154}: +93% {T1090} | Strength part: ⌊a0·⌊40·D⌋/100⌋ added to the physical least and most (see the row). |
| 24 | 6 | spells' regeneration speed (+0xEE) | +0.2·D (0.2/0.268/0.334/0.4/0.468) | {T1148}: +20% ; {T1148}: +46%<br>{T798}: +10 ; {T798}: +23 | The art 13 found by its id in the art list. |
| 24 | 6 | arts 13: bonus byte | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 25 | 3 | phys. spell mult. n (+0xB6) | +0.2·D (0.2/0.268/0.334/0.4/0.468) | {T1076}: +20 ; {T1076}: +46 |  |
| 25 | 3 | fire spell mult. n (+0xBA) | +0.2·D (0.2/0.268/0.334/0.4/0.468) | 〃 |  |
| 25 | 3 | magic spell mult. n (+0xBE) | +0.2·D (0.2/0.268/0.334/0.4/0.468) | 〃 |  |
| 25 | 3 | poison spell mult. n (+0xC2) | +0.2·D (0.2/0.268/0.334/0.4/0.468) | 〃 |  |
| 25 | 4 | spells' regeneration speed (+0xEE) | +0.1·D (0.1/0.134/0.167/0.2/0.234) | {T1148}: +10% ; {T1148}: +23%<br>{T814}: +10 ; {T814}: +23 | The art 17 found by its id. |
| 25 | 4 | arts 17: bonus byte | +⌊10·D⌋ (10/13/16/20/23) | 〃 |  |
| 26 | 3 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 26 | 4 | D multiplier (+0xEA) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | {T1075}: +30% ; {T1075}: +70% |  |
| 26 | 5 | poison damage mult. m (+0xA2) | +0.4·D (0.4/0.536/0.668/0.8/0.936) | 40% {T1085} {T1081} ; 93% {T1085} {T1081} |  |
| 26 | 6 | — (text only in 0x571710) | — | {T1135}: +10% ; {T1135}: +23% | Text only here; attacker's effect 867 + 0.1·D (M10). |
| 26 | 7 | — (text only in 0x571710) | — | {T783} ; {T783} | Text only here; the Dark Elf's state (M9). |
| 27 | 3 | arts 1049: bonus byte | +⌊6·D⌋ (6/8/10/12/14) | {T8252}: +6 ; {T8252}: +14 |  |
| 27 | 4 | fire damage mult. m (+0x9A) | +0.5·D (0.5/0.67/0.835/1/1.17) | 50% {T1085} {T1079} ; 116% {T1085} {T1079} |  |
| 27 | 5 | — (text only in 0x571710) | — | {T1132}: +5% ; {T1132}: +11% | Text only here; effect 859 + 5·D (M3). |
| 27 | 6 | creature +0x1FC bit(s) | or 0x4 | {T788} ; {T788} | creature +0x1FC bit 4 (via 0x4EB860 + 0xC); cleared each recomputation before 0x5711F0. |
| 28 | 3 | skill 9's bonus | +⌊7·D⌋ (7/9/11/14/16) | {T9408}: +7 ; {T9408}: +16 |  |
| 28 | 4 | — (text only in 0x571710) | — | {T1161} {K:RES_GEG_FERNK} 50% ; {T1161} {K:RES_GEG_FERNK} 116% | Text only here; applied in the strike against a ranged attacker (M11). |
| 28 | 5 | poison damage mult. m (+0xA2) | +0.4·D (0.4/0.536/0.668/0.8/0.936) | 40% {T1085} {T1081} ; 93% {T1085} {T1081} |  |
| 28 | 6 | — (text only in 0x571710) | — | {T1159} +10% ; {T1159} +23% | Text only here; the survival bonus +10·D (M12). |
| 29 | 2 | skill 7's bonus | +⌊8·D⌋ (8/10/13/16/18) | {T9406}: +8 ; {T9406}: +18 |  |
| 29 | 3 | — (text only in 0x571710) | — | {T1130}: +50 ; {T1130}: +116 | Text only here; the map's radius +50·D (M13). |
| 29 | 4 | fire damage mult. m (+0x9A) | +0.5·D (0.5/0.67/0.835/1/1.17) | 50% {T1085} {T1079} ; 116% {T1085} {T1079} |  |
| 29 | 5 | — (text only in 0x571710) | — | {T1132}: +6% ; {T1132}: +14% | Text only here; effect 859 + 6·D (M3). |
| 30 | 2 | skill 21's bonus | +⌊10·D⌋ (10/13/16/20/23) | {T9420}: +10 ; {T9420}: +23 |  |
| 30 | 3 | arts 25: bonus byte | +⌊4·D⌋ (4/5/6/8/9) | {T822}: +4 ; {T822}: +9 |  |
| 30 | 4 | magic damage mult. m (+0x9E) | +0.25·D (0.25/0.335/0.4175/0.5/0.585) | 25% {T1085} {T1080} ; 58% {T1085} {T1080} |  |
| 30 | 5 | — (text only in 0x571710) | — | {T1143}: +10% ; {T1143}: +23% | Text only here; effect 856 + 10·D (M14). |
| 30 | 6 | — (text only in 0x571710) | — | {T1124} 30% ; {T1124} 70% | Text only; no luck reader of set 30. |
| 31 | 2 | skill 17's bonus | +⌊6·D⌋ (6/8/10/12/14) | {T9416}: +6 ; {T9416}: +14 |  |
| 31 | 3 | skill 15's bonus | +⌊6·D⌋ (6/8/10/12/14) | {T9414}: +6 ; {T9414}: +14 |  |
| 31 | 4 | fire damage mult. m (+0x9A) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | 30% {T1085} {T1079} ; 70% {T1085} {T1079} |  |
| 31 | 5 | creature +0x1FC bit(s) | or 0x4 | {T788} ; {T788}<br>{T1151}: +10% ; {T1151}: +23% | creature +0x1FC bit 4; the second line is applied by M15 (open wounds ⌊10·D⌋ %). |
| 32 | 3 | skill 16's bonus | +⌊6·D⌋ (6/8/10/12/14) | {T9415}: +6 ; {T9415}: +14 |  |
| 32 | 4 | skill 18's bonus | +⌊6·D⌋ (6/8/10/12/14) | {T9417}: +6 ; {T9417}: +14 |  |
| 32 | 5 | magic damage mult. m (+0x9E) | +0.4·D (0.4/0.536/0.668/0.8/0.936) | 40% {T1085} {T1080} ; 93% {T1085} {T1080} |  |
| 32 | 6 | — (text only in 0x571710) | — | {T1138}: +3 ; {T1138}: +7 | Text only; no reader of set 32. |
| 32 | 7 | creature +0x1FC bit(s) | or 0x8 | {T835} ; {T835} | creature +0x1FC bit 8; read by the recomputation for the Mage (type 3): effect 40 (0x28). |
| 33 | 3 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Sets 33–37 share one case. Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 33 | 3 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 33 | 3 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 34 | 3 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 34 | 3 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 34 | 3 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 35 | 3 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 35 | 3 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 35 | 3 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 36 | 3 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 36 | 3 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 36 | 3 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 37 | 3 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 37 | 3 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 37 | 3 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 38 | 5 | all four most damages (+0x76…+0x82) | = a0 (set, not added) | 8 lines `\c%x%s: %d-%d` (T1078–T1081, colours 0xFFFFFFA0, 0xFFFFA0A0, 0xFFA0A0FF, 0xFFA0FFA0, values ⌊a0/2⌋ and a0) and `\c%x%s: %d` (K INVENTAR_RES_PHYSICAL/_FIRE/_MAGIC/_POISON, value a1) | Applied by the shared code at 0x575CA3 (33–37) and 0x575F32 (38); the values replace whatever the items gave. |
| 38 | 5 | all four least damages (+0x86…+0x92) | = ⌊a0/2⌋ | 〃 |  |
| 38 | 5 | all four resistances (+0x66…+0x72) | = a1 | 〃 |  |
| 39 | 2 | skill 14's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9413}: +5 ; {T9413}: +11 |  |
| 39 | 4 | a4 (+0x18) | +⌊10·D⌋ (10/13/16/20/23) | {T1094}: +10 ; {T1094}: +23 |  |
| 39 | 6 | flags +0x148 bit(s) | or 0x2 | {K:SET_SNOWEVER} ; {K:SET_SNOWEVER}<br>6% {K:FROSTSCHADEN} ; 14% {K:FROSTSCHADEN} | Bit 0x2 is one of 0x7C2: the strike multiplies the whole damage by D (docs/re/items.md); its change starts or ends a weather event (0x57D21B). The text says 6 %·D. |
| 40 | 2 | — (text only in 0x571710) | — | {T1151}: 6% ; {T1151}: 14% | Text only here; open wounds ⌊6·D⌋ % (M15). |
| 40 | 4 | flags +0x148 bit(s) | or 0x2000 | {K:TIERSCHRECK} ; {K:TIERSCHRECK} |  |
| 40 | 6 | flags +0x148 bit(s) | or 0x80000 | 4% {T1073} ; 9% {T1073} | New bit 0x80000: the most health ×104/100 (0x565B51, docs/re/fight.md), not ×D. |
| 41 | 2 | skill 22's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9421}: +5 ; {T9421}: +11 |  |
| 41 | 4 | — (text only in 0x571710) | — | {T1150}: 20% ; {T1150}: 46% | Text only here; a chance ⌊20·D⌋ % (M16). |
| 41 | 5 | flags +0x148 bit(s) | or 0x100000 | 15% {K:MAGIEABWEHR} ; 35% {K:MAGIEABWEHR} | Magic defence: effect 858 + 15·D while the bit is set (M17). |
| 41 | 6 | flags +0x148 bit(s) | or 0x400 | 6% {K:DUNKELSCHADEN} ; 14% {K:DUNKELSCHADEN} | New bit 0x400 (of 0x7C2: all damage ×D). |
| 42 | 2 | skill 28's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9427}: +5 ; {T9427}: +11 |  |
| 42 | 4 | every art the creature holds: bonus byte | +⌊3·D⌋ (3/4/5/6/7) | +3 {T8225} {T9031} ; +7 {T8225} {T9031} | With the creature's +0x14 bit 0x2000 every art the creature holds; without it the arts 1030–1037 only (both branches emulated). |
| 42 | 5 | flags +0x148 bit(s) | or 0x200000 | +5 {K:UI_RARE_SCHOCK} ; +11 {K:UI_RARE_SCHOCK} | Stun: +0.05·D while no other stun set applies (M18). |
| 42 | 6 | flags +0x148 bit(s) | or 0x400000 | 40% {T8225} ; 93% {T8225} | New bit 0x400000: arts 1022/1023's f = 1 + 0.4·D (docs/re/arts.md). |
| 43 | 2 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 43 | 4 | — (text only in 0x571710) | — | {T1137}: +3 ; {T1137}: +7 | Text only (all arts +3): nothing applied. |
| 43 | 5 | flags +0x148 bit(s) | or 0x800000 | {T1128}: +6% ; {T1128}: +14% | Bit 0x800000 (reader 0x57DC05, not read). |
| 43 | 6 | flags +0x148 bit(s) | or 0x20000 | {T8220} ; {T8220} | New bit 0x20000: the Gladiator's effect 40 with the art 1015 (0x57B025; docs/re/arts.md). |
| 44 | 2 | skill 5's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9404}: +5 ; {T9404}: +11 |  |
| 44 | 3 | flags +0x148 bit(s) | or 0x100000 | 10% {K:MAGIEABWEHR} ; 23% {K:MAGIEABWEHR} | Magic defence: effect 858 + 10·D (M17). |
| 44 | 4 | — (text only in 0x571710) | — | +9% {T1132} ; +21% {T1132} | Text only here; effect 859 + 9·D (M3). |
| 44 | 5 | flags +0x148 bit(s) | or 0x40 | 6% {K:BLUTSCHADEN} ; 14% {K:BLUTSCHADEN} | New bit 0x40 (of 0x7C2). |
| 45 | 2 | D multiplier (+0xEA) | +0.3·D (0.3/0.402/0.501/0.6/0.702) | {T1075}: +30% ; {T1075}: +70% |  |
| 45 | 3 | a4 (+0x18) | +⌊10·D⌋ (10/13/16/20/23) | {T1094}: +10 ; {T1094}: +23 |  |
| 45 | 4 | — (text only in 0x571710) | — | {T1138}: +2 ; {T1138}: +4 | Text only; no reader of set 45. |
| 45 | 5 | flags +0x148 bit(s) | or 0x200 | 6% {K:GEHEIMSCHADEN} ; 14% {K:GEHEIMSCHADEN} | Bit 0x200 (of 0x7C2). |
| 46 | 2 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 46 | 3 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23 |  |
| 46 | 4 | the four resistance factors (+0xD6…+0xE2) | +curve(R…Ph/Fe/Ma/Gi, L)/100 (0.12 each at L 1) | {T9410}: +1 ; {T9410}: +1 | The Resistance skill's (11) curves at L = max(1, ⌊level/4⌋): each resistance factor += curve(R…k)/100, pace = ⌊pace + curve(R…SP)⌋ (0x565D70 with RoffPh/R__sPh/R__wPh … RoffSP/R_s_SP/R_w_SP). Values shown at level 0 (L 1); at level 40 (L 10): +0.8461, +0.69356 ×3, pace +3. |
| 46 | 4 | pace (+0x40) | = ⌊pace + curve(R…SP, L)⌋ (+1 at L 1) | 〃 |  |
| 46 | 5 | flags +0x148 bit(s) | or 0x1 | {K:GEGNERTELEPORT} ; {K:GEGNERTELEPORT} | Bit 0x1 (readers 0x52278F, 0x543E58, not read). |
| 47 | 2 | skill 26's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9425}: +5 ; {T9425}: +11 |  |
| 47 | 3 | a3 (+0x16) | +⌊10·D⌋ (10/13/16/20/23) | {T1093}: +10 ; {T1093}: +23 |  |
| 47 | 4 | every art the creature holds: bonus byte | +⌊3·D⌋ (3/4/5/6/7) | {T1137}: +3 ; {T1137}: +7 | Every art the creature holds. |
| 47 | 5 | flags +0x148 bit(s) | or 0x100 | 6% {K:SEELESCHADEN} ; 14% {K:SEELESCHADEN} | Bit 0x100 (of 0x7C2). |
| 48 | 2 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 48 | 3 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | {T1092}: +10 ; {T1092}: +23 |  |
| 48 | 4 | — (text only in 0x571710) | — | {T1151}: 6% ; {T1151}: 14% | Text only here; open wounds ⌊6·D⌋ % (M15). |
| 48 | 6 | A multiplier (+0xE6) | +curve(FEK…AW, L)/100 (0.09 at L 1) | {K:EWT_FERNKAMPF}:+1 ; {K:EWT_FERNKAMPF}:+1 | Only while 0x54A2A0 holds (a hand's item of category 5 whose weapon group, 0x427ED0, is 7 or 13: hypothesis a bow or crossbow). L = max(1, ⌊level/3⌋): A multiplier += curve(FEKoffAW, L, FEK_s_AW, FEK_w_AW)/100, attack speed = ⌊speed + curve(FEKoffSP, L, FEK_s_SP, FEK_w_SP)⌋. Values at level 0; at level 40 (L 13): +0.63581, +42. |
| 48 | 6 | attack speed (+0x3C) | = ⌊speed + curve(FEK…SP, L)⌋ (+6 at L 1) | 〃 |  |
| 49 | 2 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 49 | 3 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23 |  |
| 49 | 4 | flags +0x148 bit(s) | or 0x200000 | +15% {K:UI_RARE_SCHOCK} ; +35% {K:UI_RARE_SCHOCK} | Stun +0.15·D (M18). |
| 49 | 6 | flags +0x148 bit(s) | or 0x80 | 6% {K:GOTTSCHADEN} ; 14% {K:GOTTSCHADEN} | New bit 0x80 (of 0x7C2). |
| 50 | 2 | skill 5's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9404}: +5 ; {T9404}: +11 |  |
| 50 | 3 | arts 1083–1084: bonus byte | +⌊2·D⌋ (2/2/3/4/4) | {T8289}: +2 ; {T8289}: +4<br>{T8290}: +2 ; {T8290}: +4 |  |
| 50 | 5 | — (text only in 0x571710) | — | +15% {T1132} ; +35% {T1132} | Text at 5; effect 859 + 15·D applied from 4 (M3). |
| 50 | 6 | flags +0x148 bit(s) | or 0x200000 | +15% {K:UI_RARE_SCHOCK} ; +35% {K:UI_RARE_SCHOCK} | Stun +0.15·D (M18 reads set 50 at 5, the bit comes at 6). |
| 51 | 2 | skill 2's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9401}: +5 ; {T9401}: +11 |  |
| 51 | 3 | A multiplier (+0xE6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | +15% {T1074} ; +35% {T1074} |  |
| 51 | 4 | pace (+0x40) | = ⌊pace × 1.15·D⌋ | +15 {T1100} ; +35 {T1100} | pace = ⌊pace × 1.15·D⌋ (0x55E0F0(1.15) times the pace); the text announces +⌊15·D⌋ attack speed (T1100). |
| 51 | 6 | flags +0x148 bit(s) | or 0x10000 | 40 {T1098} ; 40 {T1098} | Bit 0x10000 (reader 0x57D153 in the recomputation, not read). |
| 52 | 2 | skill 31's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9430}: +5 ; {T9430}: +11 |  |
| 52 | 3 | a3 (+0x16) | +⌊10·D⌋ (10/13/16/20/23) | {T1093}: +10 ; {T1093}: +23 |  |
| 52 | 4 | — (text only in 0x571710) | — | {T1130}: +100 ; {T1130}: +233 | Text only here; the map's radius +100·D (M13). |
| 52 | 6 | flags +0x148 bit(s) | or 0x800 | 33% {K:NOROOT} ; 33% {K:NOROOT} | Bit 0x800 (reader not found). |
| 53 | 2 | skill 33's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9432}: +5 ; {T9432}: +11 |  |
| 53 | 3 | arts 55: bonus byte | +⌊3·D⌋ (3/4/5/6/7) | {T853}: +3 ; {T853}: +7 |  |
| 53 | 5 | — (text only in 0x571710) | — | 3% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 7% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 53 | 6 | — (text only in 0x571710) | — | 20% {T1124} ; 46% {T1124} | Text at 6; the luck reader needs 7 (M2). |
| 53 | 7 | — (text only in 0x571710) | — | {K:UI_SET_VERSILBERN} ; {K:UI_SET_VERSILBERN} | Text only here; the query 0x51 answers 1 (M19). |
| 54 | 2 | skill 11's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9410}: +5 ; {T9410}: +11 |  |
| 54 | 3 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | {T1092}: +10 ; {T1092}: +23 |  |
| 54 | 4 | — (text only in 0x571710) | — | +50%  {T1079} {T1161} {T1180} ; +116%  {T1079} {T1161} {T1180} | Text at 4 names fire; the strike multiplies poison ×(1 + 0.5·D) against kind 5 from 5 pieces (M1). |
| 54 | 6 | poison resistance factor (+0xE2) | +0.25·D (0.25/0.335/0.4175/0.5/0.585) | {K:INVENTAR_RES_POISON}: 25% ; {K:INVENTAR_RES_POISON}: 58% |  |
| 54 | 7 | — (text only in 0x571710) | — | +30% {K:UI_RARE_EXORZIST} ; +70% {K:UI_RARE_EXORZIST} | Text only here; effect 863 + 0.3·D (M20). |
| 55 | 2 | skill 2's bonus | +⌊3·D⌋ (3/4/5/6/7) | {T9401}: +3 ; {T9401}: +7 |  |
| 55 | 3 | skill 25's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9424}: +5 ; {T9424}: +11 |  |
| 55 | 4 | A multiplier (+0xE6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | +15% {T1074} ; +35% {T1074} |  |
| 55 | 6 | a0 (+0x10) | +⌊10·D⌋ (10/13/16/20/23) | {T1090}: +10 ; {T1090}: +23 |  |
| 55 | 8 | flags +0x148 bit(s) | or 0x1000 | 66% {K:NOSTUNN} ; 66% {K:NOSTUNN} | Bit 0x1000 (reader not found). |
| 55 | 10 | flags +0x148 bit(s) | or 0x40000000 | {T8299} ; {T8299} | New bit 0x40000000 (0x563C29; its clearing removes the effects keyed 0x449 at 0x57C1B3). |
| 56 | 2 | skill 25's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9424}: +5 ; {T9424}: +11 |  |
| 56 | 4 | a3 (+0x16) | +⌊10·D⌋ (10/13/16/20/23) | {T1093}: +10 ; {T1093}: +23 |  |
| 56 | 5 | — (text only in 0x571710) | — | +9% {T1132} ; +21% {T1132} | Text only here; effect 859 + 9·D (M3). |
| 56 | 6 | poison resistance factor (+0xE2) | +0.25·D (0.25/0.335/0.4175/0.5/0.585) | {K:INVENTAR_RES_POISON}: 25.0% ; {K:INVENTAR_RES_POISON}: 58.5% | The line's format is %3.1f. |
| 56 | 7 | flags +0x148 bit(s) | or 0x10 | {K:NOPOISEN} ; {K:NOPOISEN} | Bit 0x10 (0x582DA6 in the effects' adder, 0x55DB2D; not read). |
| 57 | 2 | skill 11's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9410}: +5 ; {T9410}: +11 |  |
| 57 | 3 | D multiplier (+0xEA) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | {T1075}: +15.0% ; {T1075}: +35.1% |  |
| 57 | 4 | a1 (+0x12) | +⌊10·D⌋ (10/13/16/20/23) | {T1091}: +10 ; {T1091}: +23 |  |
| 57 | 6 | — (text only in 0x571710) | — | {T1130}: +100 ; {T1130}: +233 | Text only here; the map's radius +100·D (M13). |
| 57 | 8 | — (text only in 0x571710) | — | 10% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 23% {K:TOOLTIP_LEBEN_ABSAUGUNG2} | Text only; no leech reader. |
| 58 | 2 | skill 32's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9431}: +5 ; {T9431}: +11 |  |
| 58 | 3 | skill 25's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9424}: +5 ; {T9424}: +11 |  |
| 58 | 4 | a2 (+0x14) | +⌊10·D⌋ (10/13/16/20/23) | {T1092}: +10 ; {T1092}: +23 |  |
| 58 | 5 | — (text only in 0x571710) | — | {T1076} {T1161} {T1182}: +50% ; {T1076} {T1161} {T1182}: +116% | Text only here; all kinds ×(1 + 0.5·D) against kind 9 (M1). |
| 58 | 6 | flags +0x148 bit(s) | or 0x4000 | {K:UI_SET_FLUCHT} ; {K:UI_SET_FLUCHT} | Bit 0x4000 (reader not found). |
| 59 | 2 | skill 14's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9413}: +5 ; {T9413}: +11 |  |
| 59 | 3 | flags +0x148 bit(s) | or 0x100000 | 15% {K:MAGIEABWEHR} ; 35% {K:MAGIEABWEHR} | Magic defence: effect 858 + 15·D (M17). |
| 59 | 4 | a4 (+0x18) | +⌊10·D⌋ (10/13/16/20/23) | {T1094}: +10 ; {T1094}: +23 |  |
| 59 | 6 | flags +0x148 bit(s) | or 0x8000 | +40% {K:UI_SET_FLIEGEN} ; +93% {K:UI_SET_FLIEGEN} | New bit 0x8000 (reader not found). |
| 59 | 8 | — (text only in 0x571710) | — | {T1138}: +3 ; {T1138}: +7 | Text only; no reader of set 59 at 8. |
| 60 | 2 | skill 4's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9403}: +5 ; {T9403}: +11 |  |
| 60 | 3 | — (text only in 0x571710) | — | {T1151}: 9% ; {T1151}: 21% | Text only here; open wounds ⌊10·D⌋ % (M15), not 9 %. |
| 60 | 4 | flags +0x148 bit(s) | or 0x1000000 | 5% {K:TOOLTIP_LEBEN_ABSAUGUNG2} ; 11% {K:TOOLTIP_LEBEN_ABSAUGUNG2}<br>30% {K:UI_SET_ROOTEN} ; 70% {K:UI_SET_ROOTEN} | The bit and the second line only while stats +0x154+60 (+0x190) is 7; the leech line always (text only, no reader). |
| 61 | 2 | skill 5's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9404}: +5 ; {T9404}: +11 |  |
| 61 | 3 | — (text only in 0x571710) | — | +6% {T1132} ; +14% {T1132} | Text only here; effect 859 + 6·D (M3). |
| 61 | 4 | flags +0x148 bit(s) | or 0x6000000 | +15% {K:UI_RARE_EXORZIST} ; +35% {K:UI_RARE_EXORZIST}<br>15% {T778} ; 35% {T778} | 0x2000000 always (effect 863 + 0.15·D, M20); 0x4000000 and its line only while +0x154+61 is 7 (reader 0x53533D, not read). |
| 62 | 2 | a2 (+0x14) | +⌊6·D⌋ (6/8/10/12/14) | {T1092}: +6 ; {T1092}: +14 |  |
| 62 | 3 | phys. weapon mult. m′ (+0xA6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | +15 {T1078} ; +35 {T1078} | The text calls it +⌊15·D⌋ T1078. |
| 62 | 4 | flags +0x148 bit(s) | or 0x8000000 | +20% {T1135} ; +46% {T1135}<br>-40% {K:UI_STAT_REDUCE_ENEMY_PHY_RES} ; -93% {K:UI_STAT_REDUCE_ENEMY_PHY_RES} | The first line is applied by M10 (effect 867 + 0.2·D); the bit 0x8000000 and its line only while +0x154+62 is 7 (0x56C825: the defender's resistance × max(0, 1 − 0.4·f)). |
| 63 | 2 | skill 7's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9406}: +5 ; {T9406}: +11 |  |
| 63 | 3 | — (text only in 0x571710) | — | +6% {T1132} ; +14% {T1132} | Text only here; effect 859 + 6·D (M3). |
| 63 | 4 | attack speed (+0x3C) | +⌊10·D⌋ (10/13/16/20/23) | {K:UI_SET_DURCHBOHREN} ; {K:UI_SET_DURCHBOHREN}<br>+10 {T1100} ; +23 {T1100} | 0x10000000 always (0x5BBD87, not read); the attack speed and its line only while +0x154+63 is 7. |
| 63 | 4 | flags +0x148 bit(s) | or 0x10000000 | 〃 |  |
| 64 | 2 | skill 3's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9402}: +5 ; {T9402}: +11 |  |
| 64 | 3 | flags +0x148 bit(s) | or 0x200000 | +9 {K:UI_RARE_SCHOCK} ; +21 {K:UI_RARE_SCHOCK} | Stun +0.9·D (M18). |
| 64 | 4 | flags +0x148 bit(s) | or 0x20800000 | {T1128}: +6% ; {T1128}: +14%<br>15% {T861} ; 35% {T861} | 0x800000 always; 0x20000000 and its line only while +0x154+64 is 7 (reader 0x535560, not read). |
| 65 | 2 | skill 14's bonus | +⌊5·D⌋ (5/6/8/10/11) | {T9413}: +5 ; {T9413}: +11 |  |
| 65 | 3 | flags +0x148 bit(s) | or 0x100000 | 10% {K:MAGIEABWEHR} ; 23% {K:MAGIEABWEHR} | Magic defence: effect 858 + 10·D (M17). |
| 65 | 4 | phys. spell mult. n (+0xB6) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | {T1154}: +15 ; {T1154}: +35<br>{T1148}: +15 ; {T1148}: +35 | The four spell multipliers always; the spells' regeneration speed and its line only while +0x154+65 is 7. |
| 65 | 4 | fire spell mult. n (+0xBA) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | 〃 |  |
| 65 | 4 | magic spell mult. n (+0xBE) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | 〃 |  |
| 65 | 4 | poison spell mult. n (+0xC2) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | 〃 |  |
| 65 | 4 | spells' regeneration speed (+0xEE) | +0.15·D (0.15/0.201/0.2505/0.3/0.351) | 〃 |  |

## 4. The bonuses applied elsewhere: the readers of the count vector

Every reader of `+0x4F8`/`+0x150` with the dummy `0xAB78A0` (159 references, all of them the set counts, plus `0xAD3EEC` once); "≥ n" is the test as coded (`jb`/`jbe`), "attacker local" means the attacker's `+0xC` is the local player's id.
Creature kinds: `0x426A80`, `0x426B70`, `0x426B20` test the creature type's `Creature.pak` record byte `+4` for 5, 10 and 9 (the texts 1180, 1181, 1182 name them).

| M | sets ≥ pieces | where | effect | value |
|---|---|---|---|---|
| M1 | 1 ≥ 2; 1 ≥ 5; 5 ≥ 5; 54 ≥ 5; 58 ≥ 5 | the strike `0x56B130` (`0x56CA85`, `0x56CCB5`, `0x56CE31`, `0x56CEC1`), attacker local, after the four kinds' damages through the resistances | set 1 ≥ 2 vs kind 5: fire ×(1 + 0.6·D) where not 0; set 1 ≥ 5 vs kind 5: all four kinds ×(1 + 0.3·D); set 5 vs kind 10: all four ×(1 + 0.3·D); set 54 vs kind 5: poison ×(1 + 0.5·D); set 58 vs kind 9: all four ×(1 + 0.5·D) | as written |
| M2 | 3 ≥ 8; 9 ≥ 7; 21 ≥ 6; 53 ≥ 7 | the luck `0x564A60` (`0x564AB3`, `0x564B4B`, `0x564BE6`, `0x564C81`), for a player creature (id below 16), the counts read from the local hero (`0x603E30`) | each in turn: ℓ = ⌊ℓ·(1 + 0.2·D) + 0.5⌋ | 0.2·D; set 21 has 5 pieces, so its test never holds |
| M3 | 4 ≥ 4; 44 ≥ 4; 50 ≥ 4; 56 ≥ 5; 61 ≥ 3; 63 ≥ 3; 7 ≥ 7; 20 ≥ 4; 27 ≥ 5; 29 ≥ 5 | the strike (`0x56E8CC`–`0x56EE25`), attacker local | the attacker's effect 859 (`0x35B`, its text 1132) against the target (`0x570ED0`) plus, in that order: 5·D, 9·D, 15·D, 9·D, 6·D, 6·D, 10·D, 10·D, 5·D, 6·D; then a roll `rand() % 101` below min(cap, (1 − a/(b + v))·100 + 0.5) (`0x5F23A0(0x35B)` gives a, b and the cap; its consequence not read) | as written |
| M4 | 16 ≥ 6 | the strike (`0x56D83F`), the defender local | the defender's effect 866 (`0x362`, text 1134) + 30·D | 30·D |
| M5 | 1 ≥ 6 (`> 5`); 18 ≥ 8 (`> 7`) | the death (`0x532C6F`, `0x532CD6`), the local player's killer | the undead's rising chance c (docs/re/fight.md) = ⌊c·(100 − 30·D)·0.01⌋ for each, then at least 0 | set 18 has 7 pieces: never |
| M6 | 19 ≥ 5 | the experience of a kill (`0x57DDE3`) | ×(1 + 0.1·D) | 0.1·D |
| M7 | 22 ≥ 5 | the drop (`0x559CA9`), the local hero | in one case of ten a potion: 5137, 5138, 5144 or 5146 in a case of ten each, else 5140 (docs/re/items.md) | — |
| M8 | 23 ≥ 6 (`> 5`) | the recomputation (`0x57AEB8`), the local hero | the creature's `+0x1FC` bit 1 (readers `0x57AEE4`, `0x63918C`, not read); a change calls `0x54D760(0x11A)` | — |
| M9 | 26 ≥ 7 | the creature's update (`0x5396DB`) and the query `0x54A6D0` (`0x54A71D`) | a Dark Elf (type 4) whose action `+0x150` is 0, 1 or 2 and whose AI state `+0xFC` is not 4, 6, 8 or 11: the update gives it the effect 63 (`0x3F`) where it has none; the query answers 1 (**hypothesis**: its stealth, the line T783) | — |
| M10 | 26 ≥ 6; 62 ≥ 4 | the strike (`0x56E178`, `0x56E1F4`) | the attacker's effect 867 (`0x363`, text 1135) + 0.1·D, + 0.2·D | as written |
| M11 | 28 ≥ 4 | the strike (`0x56B6D1`), on the stats of the struck one | against an attacker holding a ranged weapon (`0x54A2A0`) or a missile whose type passes `0x427E80`: a value of the attack record (`[esp+0x198]` after `0x568D70`; **hypothesis**: the defence against it, the line `RES_GEG_FERNK`) ×(1 + 0.5·D) | 0.5·D |
| M12 | 28 ≥ 6 | the survival bonus: `0x47D2C3`, `0x48468C`, `0x551126` (in tenths, + D), `0x56A598`, `0x56C240` (the strike), `0x6A7D34`, `0x6A83C5`, `0x6AA0C4` (the statistics window); `0x5374B7` reads the size and drops it | SB + 10·D (`SB = 100 + curve·LongivityCombatFactor` where in per cent) | 10·D |
| M13 | 29 ≥ 3; 52 ≥ 4 or 57 ≥ 6 | the interface (`0x6CE9B0`, `0x6CEA26`): a radius of 300 or 350 plus the local hero's effect 853 (`0x355`) times 8 | + 50·D; + 100·D (either of the two) (**hypothesis**: the map's view radius, text 1130) | as written |
| M14 | 30 ≥ 5 | the strike (`0x56F68F`) | the attacker's effect 856 (gold from a hit, text 1143) + 10·D | 10·D |
| M15 | 31 ≥ 5 or 60 ≥ 3; 40 ≥ 2 or 48 ≥ 4 | a hit (`0x535183`, `0x535261`), the attacker's counts | `rand` in 0…100 at most ⌊10·D⌋ (resp. ⌊6·D⌋): `0x4E87E0(100, 1)` and, where the struck one lacks it, the effect 23 (`0x17`, open wounds) by `0x582D50(0x17, 0, ⌊x/10⌋, 10·D, …)` (x a value of the hit, not read) | ⌊10·D⌋ %, ⌊6·D⌋ % |
| M16 | 41 ≥ 4 | `0x4E263D` (a hit's handler, not identified) | where the struck one has no effect 855 (`0x357`) of its own: `rand() % 101` below ⌊20·D⌋ takes the same consequence (`0x54E6D0(1, 0)`, `+0x1F4` bit `0x200000` cleared, `0x44D520`; not read further) | ⌊20·D⌋ % |
| M17 | 41 ≥ 5; 44 ≥ 3; 59 ≥ 3; 65 ≥ 3 | `0x561880` (`0x5618AC`…), while the stats' `+0x148` bit `0x100000` | the effect 858 (`0x35A`, magic defence) + 15·D, + 10·D, + 15·D, + 10·D (each that holds) | as written |
| M18 | 64 ≥ 3, else 49 ≥ 4 or 50 ≥ 5, else any | the strike (`0x56D4E6`), while the attacker's `+0x4F0` bit `0x200000` | the stun's value (the effect 862's sum, `0x5F23A0(0x35E)`) + 0.9·D, else + 0.15·D, else + 0.05·D (only one) | as written |
| M19 | 53 ≥ 7 | the query `0x51` of `0x55D260` (`0x55DE75`) | answers 1 for a player hero (docs/re/inventory.md; the line `UI_SET_VERSILBERN`) | — |
| M20 | 54 ≥ 7 | the death (`0x532D9F`) | the killer's effect 863 (`0x35F`, the exorcism, `UI_RARE_EXORZIST`) + 0.3·D, after + 0.15·D for the flag `0x2000000` (set 61); then `rand() % 100` against it (`0x590570`) | 0.3·D |

## 5. The flags of `+0x148` and `+0x1FC` the sets set, and their readers

| bit | sets ≥ pieces | key of the line | readers found (by scans of `test`/`and` on `+0x148`/`+0x4F0`) |
|---|---|---|---|
| `0x1` | 46 ≥ 5 | `GEGNERTELEPORT` | `0x52278F`, `0x543E58` (not read) |
| `0x2` | 39 ≥ 6 | `SET_SNOWEVER`, `FROSTSCHADEN` | the 0x7C2 group: the strike's whole damage × D (`0x5321AD`, `0x56D0C9`); `0x532237`; the weather event on change (`0x57D21B`) |
| `0x10` | 56 ≥ 7 | `NOPOISEN` | `0x582DA6` (in the effects' adder `0x582D50`), `0x55DB2D` (not read) |
| `0x40` | 44 ≥ 5 | `BLUTSCHADEN` | 0x7C2 group; `0x4E1058` |
| `0x80` | 49 ≥ 6 | `GOTTSCHADEN` | 0x7C2 group; `0x519A64` |
| `0x100` | 47 ≥ 5 | `SEELESCHADEN` | 0x7C2 group |
| `0x200` | 45 ≥ 5 | `GEHEIMSCHADEN` | 0x7C2 group |
| `0x400` | 41 ≥ 6 | `DUNKELSCHADEN` | 0x7C2 group |
| `0x800` | 52 ≥ 6 | `NOROOT` | not found |
| `0x1000` | 55 ≥ 8 | `NOSTUNN` | not found |
| `0x2000` | 40 ≥ 4 | `TIERSCHRECK` | not found |
| `0x4000` | 58 ≥ 6 | `UI_SET_FLUCHT` | not found |
| `0x8000` | 59 ≥ 6 | `UI_SET_FLIEGEN` | not found |
| `0x10000` | 51 ≥ 6 | T1098 | `0x57D153` (the recomputation) |
| `0x20000` | 43 ≥ 6 | T8220 | `0x57B025` (the Gladiator's effect 40 with 1015), `0x57C48F`, `0x586EC5` |
| `0x80000` | 40 ≥ 6 | T1073 | `0x565B51` (the most health ×104/100) |
| `0x100000` | 41 ≥ 5, 44 ≥ 3, 59 ≥ 3, 65 ≥ 3 | `MAGIEABWEHR` | `0x5618A1` (M17) |
| `0x200000` | 42 ≥ 5, 49 ≥ 4, 50 ≥ 6, 64 ≥ 3 | `UI_RARE_SCHOCK` | `0x56D4D6` (M18) |
| `0x400000` | 42 ≥ 6 | T8225 | `0x5128D6`, `0x58DE2B` (arts 1022/1023's f = 1 + 0.4·D) |
| `0x800000` | 43 ≥ 5, 64 ≥ 4 | T1128 | `0x57DC05` (not read) |
| `0x1000000` | 60 ≥ 4 with `+0x190` = 7 | `UI_SET_ROOTEN` | `0x526367`, `0x4E0713` (not read) |
| `0x2000000` | 61 ≥ 4 | `UI_RARE_EXORZIST` | `0x532D77` (M20) |
| `0x4000000` | 61 ≥ 4 with `+0x191` = 7 | T778 | `0x53533D` (not read) |
| `0x8000000` | 62 ≥ 4 with `+0x192` = 7 | `UI_STAT_REDUCE_ENEMY_PHY_RES` | `0x56C825` (the defender's resistance × max(0, 1 − 0.4·f), docs/re/fight.md) |
| `0x10000000` | 63 ≥ 4 | `UI_SET_DURCHBOHREN` | `0x5BBD87` (not read) |
| `0x20000000` | 64 ≥ 4 with `+0x194` = 7 | T861 | `0x535560` (not read) |
| `0x40000000` | 55 ≥ 10 | T8299 | `0x563C29`; its loss removes the effects keyed 1097 (`0x57C1B3`) |
| `0x80000000` | 3 ≥ 4 | `UI_KNOW_ENEMY` | `0x62737F`, `0x62759D`, `0x6279E8` (the interface; not read) |
| creature `+0x1FC` 1 | 23 = 6 | — | `0x57AEE4`, `0x63918C` |
| creature `+0x1FC` 4 | 27 ≥ 6, 31 ≥ 5 | T788 | `0x56E0A4` (the strike; not read) |
| creature `+0x1FC` 8 | 32 ≥ 7 | T835 | `0x57AF1E` (the Mage's effect 40), `0x586EBC` |

The flags `+0x148` are cleared at the start of each recomputation for the local hero only; another player's are not (not followed).

## 6. Where the text and the code disagree

- Thresholds: set 4 (skill 2 shown at 2, applied at 3), 6 (a3 shown at 3, applied at 4), 7 (effect 859 shown at 8, applied at 7), 50 (shown at 5, applied at 4), 53 (luck shown at 6, applied at 7), 54 (shown at 4, applied at 5), 18 and 21 (applied only at a count above their pieces: never).
- Values: set 1 (fire vs kind 5: 50 % shown, 60 % applied), 3 (luck 25 % shown, 20 % applied), 12 and 13 (skills ⌊3·D⌋ shown, ⌊2·D⌋ applied), 51 (attack speed +⌊15·D⌋ shown, the pace × 1.15·D applied), 60 (open wounds 9 % shown, 10 % applied), 40 (health 4 %·D shown, ×1.04 applied), 39/41/44/45/47/49 (a 6 %·D damage of a kind shown, all damage × D applied).
- Kinds: set 54 (fire shown, poison applied); set 2 (a life leech shown, skill 10 raised).
- Shown and applied nowhere: every life leech line (sets 2, 8, 9, 15, 17, 19, 21, 53, 57, 60), set 3's T1143 at 6, 12's T1138, 14's and 43's all arts, 19's T1127, 30's luck, 32's T1138, 45's T1138, 59's T1138.
- Applied and shown nowhere: set 6's arts at 9, set 22's potion drop (M7).

## 7. Not read

- The consequences of the effects 855, 859, 862 and 863's rolls, the effect 23's values (`x`), the readers of the flags marked "not found" or "not read", and the creature's `+0xB0` bits `0x800…0x4000` of the dominance bonus.
- `0x426550` (an item skipped in the slots), `0x427E80` (M11), the inventory behind `0x5DEBC0`.
- Whether the floors of `+0x3C`/`+0x40` (80 or 100) apply after the sets' pace (sets 46, 51) in every case.

## 8. The tooltip in OpenSacred

The maintainer's decision of 2026-11-01 (#1727, the rule of #1737): where a line of `0x571710`'s text mode disagrees with what the code applies, the code stays and the line tells it.

- A line whose bonus nothing applies is dropped: every `TOOLTIP_LEBEN_ABSAUGUNG2` line (sets 2, 8, 9, 15, 17, 19, 21, 53, 57, 60), set 19's own-life drain at 2, set 3's gold on a hit at 6, set 30's luck at 6, the all-spells lines of sets 12, 32, 45, 59 and the all-arts lines of sets 14 and 43, and the lines of set 18 at 7 and set 21 at 5, whose readers test a count the set cannot reach.
- A bonus applied with no line gets one, of the game's own texts: set 2's skill 10 at 5, set 6's arts 13, 17, 19, 21 and 1019 at 9; set 22's potion drop at 5 has none that fits (open), and set 23's flag gets its line once its readers are read.
- The thresholds, values and kinds are the code's (section 6), the numbers taken from the table that applies them.
- The six sets' whole damage × `D` (39, 41, 44, 45, 47, 49) stays, and its line tells the real share, +34 %, +67 %, +100 %, +134 % from silver to niobium; on bronze, where `D` is 1.0 and the factor changes nothing, the line is dropped.
- The flags whose readers are not found or not read (section 5) are read before their lines are repaired.

In `opensacred-game` nothing of the sets is followed yet: the pieces carry their set's number (`drops::Tables::set_name`, #1700), and the counting, the bonuses, their readers and the equipment window's icon and its list (#1670) wait for #1727's later steps.
Since #1727's second step `opensacred_rules::set_bonuses` holds section 3's bonuses of the stats as a table (`TABLE`), the difficulty's factor, the worn pieces counted by their distinct types (`Worn`) and what the sets reached give the stats (`stats`); the game does not take them yet.
