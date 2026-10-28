# Text

The converter's repairs of the English `global.res` (docs/02-data-formats.md §8; no docs/re file covers them).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Fish shoals named | The fish types 164 and 165, which have no name in `global.res`, are named Shoal of Fish and Shoal of Small Fish. | They are the only nameless creatures that the game makes without a script to name them; the maintainer's choice of the project's words (#403). | repair | #684 |
| Gameboxes named by their art | The eleven gamebox items 5150 to 5160, which share one generic word, are each named by the art of its box, without naming the product it shows. | One word cannot tell eleven boxes apart, and each box shows another owner's title and trademark; the maintainer's choice of the project's words (#403). | repair | #686 |
| Nameless droppable items named | The 300 nameless `Weapon.pak` types that a kill can make, and the lung 1331 of quest 15072, get names of the project's own: an armour piece by its class, its model's family and its piece, an amulet by its design, a weapon or a shield by its look. | The player meets them as drops, and the original shows them without a name, which reads as a fault; the maintainer's choice (#685). | repair | #403, #685 |
| Quest log's missing lines written | Four `QuestBook` keys that resolve to no string get lines of the project's own, each in the form of its siblings. | The quest log would show nothing, or a key, where the scripts write these lines; the maintainer's choice of the project's words (#403). | repair | #687 |
| Text defects mended in the project's words | `opensacred-convert text` mends the defects of the English `global.res` with English words written for OpenSacred, each applied only to the text of 2.28 whose hash it names; the goblins' god's German name becomes Stompa in its ten strings. | No installed file holds the right text, and another language's file or an overlay from the user would make the result depend on the user's copy; the maintainer's choice (#403). | repair | #403, #683 |
