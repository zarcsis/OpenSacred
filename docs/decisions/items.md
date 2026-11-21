# Items

The drops and the items on the ground (docs/re/items.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Drops lie until the game ends | A drop lies until the game ends wherever the hero goes, at height 0 if it flew out of the loaded sectors. | How long the original's items lie, and whether they stay when their sector is left, is not read; it is to be (#2871), and the row leaves once the game follows it. | limitation | #415, #2871 |
| Drops rolled by the game's generator | The drops' rolls come from the game's own generator rather than the C library's `rand()`, so that a death drops what the original's might, not what it would. | The C library's `rand()` is one sequence that every draw of the client shares, an order that the game does not reproduce. | simpler | #414 |
| Drops' class choice drawn evenly | With a class asked, a drop's type is drawn evenly among the candidates of the class's bit or of none (`opensacred_rules::drops::Catalogue::choose`), where the original keeps their indices as bytes, so that past 256 candidates an index wraps and some types come oftener than others, as the weapons', the armours' and the rings' lists pass 256 (docs/re/items.md, "A pass: what it makes"). | The wrap is an overflow of the original's list, whose meaning is the even draw; the maintainer's choice (#2950). | repair | #2950 |
| Emptied sockets priced anew | An item whose sockets the smith empties is priced by its own values again, as an item that never had them. | The original takes every socket's item's values off the item (`0x5C0430`) but leaves its price (`+0x08`) at the worth that the last filling gave it with them (`0x5C0810`), so that the empty item sells as dear as the filled one; the game prices an item by its values whenever it asks, and keeps no stale price. | repair | #1661 |
