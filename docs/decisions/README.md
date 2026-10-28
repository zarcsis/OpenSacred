# Decision log: deviations from the original

Every deliberate difference between the new game's behavior and Sacred Gold 2.28 is recorded here: formulas, balance, UI, engine limitations.
Fidelity to the original is not a goal (PLAN.md, ADR-01), but deviations must be visible and justified.

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
- **Kind**: one of three:
  - `limitation`: an engine's or a format's limit;
  - `repair`: the converter mends a defect of the original's data (CLAUDE.md, "Code conventions");
  - `deviation`: the play or the look differs by choice.
- **Issues**: the issues and pull requests behind the decision.

## Taking a decision

A decision is a row in its subsystem's table, put in its place by its name.
A changed decision is its row edited, and a dropped one its row removed; the history is in git.
A row says what the game does now, without dates or the steps that led there.
What the game does not do yet is not a decision: it belongs in its issue and in the "In `opensacred-game`" passages of `docs/re/`.
A decision is merged without waiting for the maintainer, who opens an issue for what to change, unless it departs from the original far enough to change the play seriously or carries legal consequences: then it waits (docs/03 §3.6).
