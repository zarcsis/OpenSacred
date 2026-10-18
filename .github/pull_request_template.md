Closes #

## What and why

## Checks run locally

CI runs fmt, clippy, the tests without the game files and `cargo deny`; it has no game files, so the rest is on you.

- [ ] `cargo nextest run --workspace --run-ignored all` on my own installation (the tests on the game files)
- [ ] `cargo deny check` (after changing dependencies)
- [ ] a new screenshot reference (`UPDATE_SCREENSHOTS=1`), looked at

## Documents updated

<!-- docs/02 for a fact about a format, docs/re/ for the original's behavior, docs/decisions.md for a deviation, PLAN.md §13 for a closed question; or "none". -->

## Rules (docs/08 §3)

- [ ] No game files, converted assets, dumps or screenshots of them, and no game texts.
- [ ] No decompiler output or code translated from it.

<!-- A pull request that changes CLAUDE.md, .claude/, .github/, deny.toml, docs/decisions.md, a screenshot reference or the process waits for the maintainer (label needs-maintainer). -->
