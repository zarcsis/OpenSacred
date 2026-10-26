# The dialogues' window before the interface

Dialogues (`opensacred-game`'s `talk`), 2026-10-26 (#1028).

## The original

A creature's dialogue shows in a window of the interface at the screen's middle (a `cUI_Tooltip`, docs/re/netscript.md, "Dialogues"): the creature's portrait at its left, an image of the interface chosen by its type, its name in light yellow over its texts, and up to four buttons, the window drawn with the interface's frame.
The hero plays its talk's clip (the slot 177) as it begins, and nothing pauses: a click beside the window reaches the hero's orders, which end the talk (**hypothesis**).

## Ours

The window is a panel of the game's own: a dark, half-clear field with a thin border at the screen's middle, the name in light yellow over the texts in the interface's font, the buttons in green under them, one centred, more spread apart, as the original's are seen.
It shows no portrait and no frame of the interface's images, the hero plays no talk's clip, and while it shows the hero's clicks on the world are held back, as the console holds them, so that a click on the window does not send the hero off; the world runs on.

## Why

The interface's frames are M5's, as the menus wait for them (docs/decisions/2026-10-19-menus.md), and the portraits, by the client's table of the creatures' types to the interface's images (`0x4345C0`), are #1032.
The hero's talk's clip needs the hero's clips by slot, which the hero does not keep yet.
Holding the clicks back keeps a button's press from walking the hero away and ending its own talk, which the original's window does by taking the clicks within its rectangle.
