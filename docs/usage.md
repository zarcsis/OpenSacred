# Claude Code usage tracking

Dollar figures are the hypothetical cost of the same tokens at Anthropic API prices; development runs on a Claude subscription, so they are not money actually spent.

Since 2026-10-17 (#16), every pull request carries its own usage: a "Claude Code usage" section at the end of its description, with the tokens per model, their cost at API prices and the agent-hours.
The badge in the README sums every pull request's figures and the sessions before them (the table below).
After 30 days, the estimates in `docs/07-roadmap-estimates.md` are recalculated from them (PLAN.md §10, step 8).

## How it is calculated

`python -I tools/usage/pr_usage.py <N> --update` writes the usage of pull request `<N>` into its description.
It runs after `gh pr create` and again when the work on the pull request is over: after the merge, or, for one that waits for the maintainer, once its checks are green.
The description can be edited after the merge, so the figures include the CI, the fixes and the merge without another commit.

* **Which messages.**
  Claude Code writes a transcript per session (`~/.claude/projects/<project>/<session>.jsonl`, the subagents' under `<session>/`) and records in every message the Git branch it was on.
  A pull request's usage is that of the messages on its branch, in every session and worktree of the repository.
  The script finds the same transcripts from the main checkout and from any of its worktrees: their directories are named after the main checkout's path.
  A message on `main` counts toward the session's next branch, since that is where the task it reads or the issue it opens is done; after the session's last branch, toward that branch.
  A session that never leaves `main` counts toward no pull request.
  The branch recorded is the checkout's, so sessions that share a checkout see each other's switches; a session's message keeps the branch the session itself moved to, by a `git switch`, `git checkout` or `git rebase` command, `gh pr checkout` or `gh pr merge`, or by entering or leaving a worktree, and a change of the recorded branch counts as the session's own only during such a command of its own or within 30 seconds after it (the recorded branch follows a switch a few seconds late).
  A session that moves into a linked worktree with `cd` rather than `EnterWorktree` keeps recording its own checkout's branch, so a message in a linked worktree counts toward the worktree's branch: the one the session gave it by `git worktree add -b` or `EnterWorktree`, else the one it is on when the script runs; a message in a worktree with neither counts toward no pull request.
  A response whose tool calls work in one linked worktree from elsewhere (a `cd`, `Set-Location` or `git -C` into it in a shell command, a `git worktree add` of it, a file tool on a path in it), and the results of those calls, count toward that worktree's branch the same way; the variables a command assigns itself (`W="…"`, `$w = '…'`) are read.
  A branch that the session renamed with `git branch -m`, such as the `worktree-<name>` that `EnterWorktree` with a name creates, counts under its new name.
* **Tokens and cost.**
  Each model response counts once (by its id), at the API prices per 1M tokens (Opus 5.5: $4 input, $20 output, $0.20 cache read, $8 one-hour cache write).
  The tokens are input, output, cache reads and cache writes together; in a long session most of them are cache reads, the context reread by every request.
  The same calculation matched the `/cost` snapshots in the transcripts to the cent.
* **Agent-hours.**
  The time from a prompt of the maintainer to the agent's last action; pauses longer than 10 minutes are not counted.
* **The block.**
  A table for people and a comment `<!-- usage {...} -->` with the same figures for scripts, between the markers `<!-- usage:begin -->` and `<!-- usage:end -->`, which a rerun replaces.
* **The badge.**
  `pr_usage.py --update` then runs the Usage workflow (`.github/workflows/usage.yml`; also daily).
  It sums the blocks of the maintainer's pull requests and the sessions below (`tools/usage/usage_badge.py`), lists the pull requests in its run summary, and publishes `usage.json`, a shields.io endpoint, on the branch `badges`: the ruleset on `main` takes no commit from a workflow.
  Contributors' pull requests are not counted: their descriptions are theirs to edit.

## Sessions before per-pull-request tracking

One row per session, from 2026-10-16 to 2026-10-17, each recorded through a pull request of its own.

| Date | Task | Model | Agent-hours | Tokens, M (input / output / cache) | Cost at API prices, $ |
|---|---|---|---|---|---|
| 2026-10-16 | Open questions §13, SHA-256 manifest, environment setup, M0 (workspace, `CLAUDE.md`, CI) | Opus 5.5 | ≈0.6 (wall clock 1.6) | ≈0 / 0.19 / read 25.8, write 0.37 | 11.9 (+0.03 Haiku 5.5) |
| 2026-10-16 | M1, textures: shared PAK reader, TEX decoder, `opensacred-convert textures`; "code in English only" rule | Opus 5.5 | ≈0.4 (wall clock 1.5) | ≈0 / 0.15 / read 10.7, write 0.32 | 7.7 |
| 2026-10-16 | Repository translated from Russian to English (`PLAN.md`, `docs/`, `tools/`, `CLAUDE.md`); 11 parallel subagents | Opus 5.5 | ≈0.6 (wall clock 0.6) | ≈0 / 0.22 / read 59.1, write 2.39 | 28.6 |
| 2026-10-16 | Project status review; plan fixes (Ghidra moved from M0 to M3, usage tracking marked done); Claude Code hooks (`cargo fmt`, clippy and nextest on Stop) | Opus 5.5 | ≈0.1 (wall clock 0.2) | ≈0 / 0.04 / read 3.6, write 0.18 | 2.9 |
| 2026-10-16 | M1, models: status review; MDL container reader `opensacred_formats::mdl`; GRN container prototype `grn_dump.py` confirmed on every record; GRN container reader `opensacred_formats::grn` | Opus 5.5 | ≈0.5 (wall clock 0.7) | ≈0 / 0.14 / read 20.4, write 0.32 | 9.4 |
| 2026-10-16 | M1, models: GRN skeleton (string table, data extensions, transform channels, bones, names through the form's bone channels) in `grn_dump.py` and `opensacred_formats::grn`, cross-checked on every record | Opus 5.5 | ≈0.4 (wall clock 0.4) | ≈0 / 0.11 / read 11.2, write 0.26 | 6.6 |
| 2026-10-16 | M1, models: GRN meshes, weights, bindings, materials and passes in `grn_dump.py` and `opensacred_formats::grn::Model`, cross-checked on every record; scale/shear order closed; `opensacred-convert models` (GRN → glTF binary), all 1,572 files valid; checks in Blender | Opus 5.5 | ≈0.9 (wall clock 1.4) | ≈0 / 0.28 / read 54.8, write 0.53 | 20.9 |
| 2026-10-16 | Night session to the morning of 2026-10-17: M1 finished (NetScript disassembler, tables, heroes, AMH, bin tables, run manifests, deterministic reruns; coverage 100 %, the four exit criteria met); M2 vertical slice in `opensacred-game` (ground, statics, liquids, streaming, seven heroes walking and running from their starts, NPCs and objects of the start scripts, shadows, footsteps, music and ambient loops, HUD and cursor, a screenshot test in CI); client facts read off the code with capstone (sound names and groups, natures, liquids, footsteps, atmosphere, CreateNPC); a review subagent | Opus 5.5 | ≈7.3 (wall clock 7.8) | ≈0 / 1.52 / read 694.9, write 2.86 | 191.7 |
| 2026-10-17 | The night session continued: CI fixed (the screenshot test over the workspace, no debug info, the dependencies' cache kept when a check fails); `Cargo.lock` stays in the repository; work order (bugs, then dependency upgrades, then features), `cargo update` at the start of every session, the PLAN.md §10.1 priority queue | Opus 5.5 | ≈0.3 (wall clock 1.2) | ≈0 / 0.04 / read 32.4, write 0.89 | 14.3 |
| 2026-10-17 | The repository made public: an audit for publication (code, documents, tools; 4 subagents), the documents reworked for it (4 subagents), the crates renamed `opensacred-*`, MIT OR Apache-2.0 and `THIRD-PARTY-NOTICES.md`, the legal policy (docs/08) rewritten; the manifest cut to the 319 files the game never writes; the issue and pull-request workflow proposed and queued (PLAN.md §10.1) | Opus 5.5 | ≈1.0 (wall clock 4.8) | ≈0 / 0.24 / read 90.2, write 2.15 | 35.0 |
| 2026-10-17 | The issue and pull-request workflow (#1, #7): labels, milestones M2–M8, issue forms and a pull-request template, CI on pull requests and `main` with a Windows job, a ruleset on `main`, the merge guard hook; the queue moved to issues #2–#6; `cargo update` at most once a day; the bug form fixed (#8, #9) | Opus 5.5 | ≈0.3 (wall clock 1.4) | ≈0 / 0.07 / read 10.8, write 0.29 | 5.9 (+ Haiku 5.5, not priced) |
| 2026-10-17 | Versions from the pull requests' labels (#11, #12): the Release workflow (an annotated tag `vX.Y.Z` and a GitHub Release after every `feature` or `bug` merge), no version in `Cargo.toml`, `--version` from `git describe` (`build.rs`), the base `v0.1.0`, a ruleset on the `v*` tags; releases before 1.0.0 are pre-releases (#13, #14) | Opus 5.5 | ≈0.3 (wall clock 0.7) | ≈0 / 0.05 / read 4.3, write 0.10 | 2.8 |

Each row was computed by the script's predecessor, `tools/usage/session_cost.py`, from the whole transcript of its session (in the Git history).

## Observations for calibrating §9

- 2026-10-16: ≈$19 per agent-hour versus $8–18 in the §9 model.
  Output (0.30M/h) and cache writes (0.61M/h) are within the model, but cache reads are 42M/h versus 5–15: a long session rereads its growing context (≈225k tokens per request on average).
  Cache reads are 43% of the cost.
  Short single-task sessions are cheaper.
- 2026-10-16 (translation): agent-hours count only the main session's time, so with 11 parallel subagents the per-agent-hour figures ($47, cache reads 97M/h) are inflated; per task it is ≈$29 for ≈1.8 MB of Russian text, cache reads ≈41% of the cost.
- 2026-10-16/17 (night session): ≈$26 per agent-hour, above the model's $8–18.
  Output (0.21M/h) and cache writes (0.40M/h) are within it; cache reads (95M/h) are six times its top, as in every long session, and make up most of the cost.
  Splitting a night into sessions per milestone would cut them.
- 2026-10-17 (the night session continued after a 6.6-hour pause): ≈$48 per agent-hour.
  Each of the 43 requests reread ≈750k tokens of the old context (cache reads 108M/h), and the hour-long cache had expired during the pause, so the context was written to it anew (cache writes ≈half the cost).
  Short tasks after a break are cheaper in a new session, which starts from `CLAUDE.md`, `PLAN.md` and the memory instead of the old context.
- 2026-10-17 (publication): ≈$36 per agent-hour, but the eight subagents' tokens are in the cost and their time is not, so the per-agent-hour figures are inflated, as in the translation session.
  Cache reads are ≈52% of the cost, cache writes ≈35%: the audit and the editing ran in parallel subagents, each reading its share of the documents into its own context.
- 2026-10-17 (the workflow): ≈$18 per agent-hour, the top of the model; output, cache reads and writes are all within it.
  Most of the wall clock was CI: the first Windows builds took 28 and 29 minutes without a cache, the next pull request 1 minute with one.
- 2026-10-17 (versions): ≈$11 per agent-hour, within the model; cache reads 16M/h, just above its top.
  A short single-task session: most of the wall clock was waiting for the maintainer's merges and for CI.
