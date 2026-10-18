# Decision log: deviations from the original

Every deliberate difference between the new game's behavior and Sacred Gold 2.28 is recorded here: formulas, balance, UI, engine limitations.
Fidelity to the original is not a goal (PLAN.md, ADR-01), but deviations must be visible and justified.

Each decision is a file of its own, `<date>-<subject>.md` with the date it was taken: its title, the subsystem, then what the original does, what the game does instead and why.
A new decision is a new file, so that two pull requests that each take one do not touch the same lines; a decision that a later one replaces says so and links it.
