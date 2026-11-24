Closes #

## What and why

<!-- Where the change shows on the screen or in the sound: its frames before and after, cropped to it, or recordings, attached with `gh pr create --attach` or `gh pr comment --attach` (docs/08 §3). -->

## Checks run locally

CI runs fmt, clippy, the tests without the game files, the screenshot test on Mesa's lavapipe, `cargo deny` and the mutants of the lines changed in `opensacred-rules`, `opensacred-data` and `opensacred-formats`; it has no game files, so the rest is on you.

- [ ] `cargo nextest run --workspace --run-ignored all` on my own installation (the tests on the game files)
- [ ] the mutants of the branch's lines (`cargo mutants --in-diff`, after changing `opensacred-rules`, `opensacred-data` or `opensacred-formats`)
- [ ] `cargo deny check` (after changing dependencies)
- [ ] a new screenshot reference (`UPDATE_SCREENSHOTS=1`), looked at

## Documents updated

<!-- docs/02 for a fact about a format, docs/re/ for the original's behavior, docs/decisions/ for a deviation (a row of its subsystem's table), PLAN.md §13 for a closed question; or "none". -->

## Rules (docs/08 §3)

- [ ] No game files, converted assets, dumps or game texts in the diff; frames and recordings only as attachments.
- [ ] No decompiler output or code translated from it.

<!-- A pull request that changes .claude/hooks/, .claude/settings.json, .claude/settings.local.json, .github/, deny.toml, the README of docs/decisions/, a screenshot reference, the rules or the process, or takes a decision whose harm to the play cannot be ruled out at a glance or that carries legal consequences, waits for the maintainer (label needs-maintainer); one that takes any other decision (label decision), or changes CLAUDE.md or a skill only to describe, is merged like any other. -->
