# Decision log: deviations from the original

Every deliberate difference between the new game's behavior and Sacred Gold 2.28 is recorded here: formulas, balance, UI, engine limitations.
The original is the reference, followed in spirit and not copied with its flaws (PLAN.md §1.1, ADR-01): a departure is welcome, and must be visible and have its ground.

## The files

The log is a file per subsystem, named as `docs/re/` names its notes where one exists:

- [models.md](models.md): the converter's models and animations, GRN → glTF, and their textures;
- [drawing.md](drawing.md): the ground, the sprites, the models and their shadows, the liquids, the night, the effects and the textures;
- [sound.md](sound.md): the converter's sound and the game's music, ambience and noises;
- [text.md](text.md): the converter's repairs of the English `global.res`;
- [interface.md](interface.md): the interface over the world, the menus and the dialogues' window;
- [ai.md](ai.md): the creatures' life while no foe is near;
- [fight.md](fight.md): the bosses, the hero's attack, the pursuit, the archers and the doors in the way;
- [arts.md](arts.md): the heroes' combat arts and spells;
- [items.md](items.md): the drops and the items on the ground;
- [spawning.md](spawning.md): the sectors' creatures as they come and go;
- [netscript.md](netscript.md): the scripts' runtime and the converter's repairs of the script trees;
- [movement.md](movement.md): walking and the way;
- [weather.md](weather.md): the rain, the fog and the snow.

## The table

Each file holds one table, a row per decision, sorted by its first column:

- **Decision**: a short name of what the game does, by which the documents and the code refer to the row (`docs/decisions/<file>.md, "<Decision>"`).
- **Ours, against the original**: what the game does, against what the original does.
- **Why**: the reason, with "the maintainer's choice (#N)" where the maintainer chose it.
- **Kind**: the ground of the departure, one of four:
  - `simpler`: the game does it a simpler way, which does no harm to the play;
  - `better`: the game does it another way, for which the play is better;
  - `repair`: a defect of the original is mended: of its data by the converter, of its behavior by the game, or a text of the original that disagrees with what its code does (CLAUDE.md, "Code conventions");
  - `limitation`: an engine's or a format's limit, or a way of the original that costs too much to follow yet; such a departure is never written as `simpler`.

  A row of before 2026-11-17 may still carry `deviation`, the play or the look differing by choice, until its ground is sorted (#2863).
- **Issues**: the issues and pull requests behind the decision.

## Taking a decision

A decision is a row in its subsystem's table, put in its place by its name.
A changed decision is its row edited, and a dropped one its row removed; the history is in git.
A row says what the game does now, without dates or the steps that led there.
What the game does not do yet is not a decision: it belongs in its issue and in the "In `opensacred-game`" passages of `docs/re/`.
A text of the original repaired to tell what its code does is a `repair` row of the subsystem whose window shows it: what the original's text says, what the code does and what the game's text now tells; the code's behaviour stays, and the text's numbers come from the code that applies them (CLAUDE.md, "Code conventions").
Claude Code takes a decision itself where it is plainly harmless to the play, and merges it without waiting for the maintainer, who reads the rows after and opens an issue for what to change; a decision whose harm to the play cannot be ruled out at a glance, or that carries legal consequences, waits (CLAUDE.md, "Issues and pull requests"; docs/03 §3.6).
