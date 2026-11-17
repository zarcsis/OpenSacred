# Claude Code usage tracking

Dollar figures are the hypothetical cost of the same tokens at Anthropic API prices; development runs on a Claude subscription, so they are not money actually spent.

Since 2026-10-17 (#16), every pull request that a session makes carries its own usage: a "Claude Code usage" section at the end of its description, with the tokens per model, their cost at API prices and the agent-hours.
The daily dependency update's pull request, which a workflow makes without a session, has none.
The badge in the README sums every pull request's figures and the sessions before them (the table below).
What they sum to beside the estimates is in `docs/07-roadmap-estimates.md`, §1.4, and PLAN.md §9.1.

## How it is calculated

`uv run python -I tools/usage/pr_usage.py <N> --update` writes the usage of pull request `<N>` into its description.
It runs after `gh pr create` and again when the work on the pull request is over: after the merge, or, for one that waits for the maintainer, once its checks are green.
The description can be edited after the merge, so the figures include the CI, the fixes and the merge without another commit.
Several numbers are counted in one pass over the transcripts, with one start of the badge's workflow: a recount of many pull requests is one run.
A block that would hold fewer tokens than the one in the description is not written (`--lower` writes it): Claude Code keeps a transcript 30 days, so an older pull request's run finds less than it once had, and a branch's messages count elsewhere once its worktree is on another branch.

* **Which messages.**
  Claude Code writes a transcript per session (`~/.claude/projects/<project>/<session>.jsonl`, the subagents' under `<session>/`) and records in every message the Git branch it was on.
  A pull request's usage is that of the messages on its branch, in every session and worktree of the repository.
  The script finds the same transcripts from the main checkout and from any of its worktrees: their directories are named after the main checkout's path.
  A message on `main` counts toward the session's next branch, since that is where the task it reads or the issue it opens is done; after the session's last branch, toward that branch.
  A session that never leaves `main` counts toward no pull request.
  The branch recorded is the checkout's, so sessions that share a checkout see each other's switches; a session's message keeps the branch the session itself moved to, by a `git switch`, `git checkout` or `git rebase` command, `gh pr checkout` or `gh pr merge`, or by entering or leaving a worktree, and a change of the recorded branch counts as the session's own only during such a command of its own or within 30 seconds after it (the recorded branch follows a switch a few seconds late).
  A session that moves into a linked worktree with `cd` rather than `EnterWorktree` keeps recording its own checkout's branch, so a message in a linked worktree counts toward the worktree's branch: the latest the session gave it by then, by `git worktree add -b`, `EnterWorktree`, or a `git switch` or `git checkout` run in it (there, after a `cd` into it earlier in the same command, or with `git -C`), else the one it is on when the script runs, else, for a worktree removed since, the branch that its directory is named after as `claim.py` names them (`.claude/worktrees/<N>-<slug>` of `<type>/<N>-<slug>`), when the repository or the pull request has one branch that ends so; a message in a worktree with none of them counts toward no pull request.
  `claim.py` makes the worktree itself, so the subagents that work in it by paths leave no branch of it in the transcript: until #2849 the run after the merge, the worktree removed, lost their messages.
  A branch given under a name that the command does not tell, `git switch -c "bug/$(gh issue create …)-slug"` or a variable set from a command's output, is not known: from then on the worktree's messages go by the rest of that list, and are not counted toward the command's text as a name.
  A switch counts from the response that runs it on; a `git checkout <name>` without `-b` may restore a path instead, so it counts only when the name is a branch (one the repository has when the script runs, the pull request's own, or one the session recorded or gave a worktree), and a `gh pr checkout` or `git switch -` in a worktree is not followed.
  A response whose tool calls work in one linked worktree from elsewhere (a `cd`, `Set-Location` or `git -C` into it in a shell command, a `git worktree add` of it, a file tool on a path in it), and the results of those calls, count toward that worktree's branch the same way; the variables a command assigns itself (`W="…"`, `$w = '…'`) are read.
  A branch that the session renamed with `git branch -m`, such as the `worktree-<name>` that `EnterWorktree` with a name creates, counts under its new name.
  Work pushed under another name than the branch it was done on is joined by hand: `pr_usage.py <N> --also <branch> --update` counts that branch's messages too, and the block keeps the name (its comment's `also`, and a sentence under the table), so that the runs after count it without being told.
* **Tokens and cost.**
  Each model response counts once (by its id), at the API prices per 1M tokens (Opus 5.5: $4 input, $20 output, $0.20 cache read, $8 one-hour cache write).
  The script's table of prices (`PRICES`) holds Opus 5.5, Sonnet 5.5, Haiku 5.5, Fable 5.1, Fable 5.2 and Mythos 5.2, as the Claude API's pricing page gave them on 2026-11-17 (Mythos 5.2 and Fable 5.2: $8 input, $40 output, $0.20 cache read, $16 one-hour cache write).
  A request of Haiku 5.5 pays by its prompt's length: five times the prices once its input, the cache's reads and writes among it, is over 100,000 tokens.
  A response of another model is listed as "not priced": its tokens count and its dollars do not, and the script ends with an error that names the model, so that its prices are added before the next pull request (#2834: the pull requests of 2026-11-17 on Mythos 5.2 counted no dollars until then).
  The tokens are input, output, cache reads and cache writes together; in a long session most of them are cache reads, the context reread by every request.
  The same calculation matched the `/cost` snapshots in the transcripts to the cent.
* **Agent-hours.**
  The time from a prompt of the maintainer to the agent's last action; a pause longer than 10 minutes counts as 10.
* **The block.**
  A table for people and a comment `<!-- usage {...} -->` with the same figures for scripts, between the markers `<!-- usage:begin -->` and `<!-- usage:end -->`, which a rerun replaces.
  The description is written with LF line ends, and one whose block did not change is not written again; until #2853 each run on Windows added a carriage return to every line, which the next run of a pull request drops.
* **The badge.**
  `pr_usage.py --update` then runs the Usage workflow (`.github/workflows/usage.yml`; also daily).
  It sums the blocks of the maintainer's pull requests and the sessions below (`tools/usage/usage_badge.py`), lists the pull requests in its run summary, and publishes `usage.json`, a shields.io endpoint, on the branch `badges`: the ruleset on `main` takes no commit from a workflow.
  The workflow lists every pull request, and the script keeps the maintainer's (`--author`): `gh pr list --author` is a search, which answers 1,000 results at most, so until #2847 the badge counted the newest 1,000 pull requests alone.
  Contributors' pull requests are not counted: their descriptions are theirs to edit.

## Sessions before per-pull-request tracking

One row per session, from 2026-10-16 to 2026-10-17, recorded before the pull requests carried their own usage (the last two through #10 and #15).

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
- 2026-11-17 (the sessions above and the blocks of 1,480 pull requests, M0 to M5 done and every issue of M6 closed): 465.3 agent-hours, 47.0B tokens, $13,220, ≈$28 per agent-hour.
  Output (0.18M/h) and cache writes (0.66M/h) are at or under the model; cache reads (100M/h) are nearly seven times its top and about seven tenths of the cost, as in the long sessions above.
  The sums by the milestones' closing days are in `docs/07-roadmap-estimates.md`, §1.4.
- 2026-11-17 (the recount, #2849): one pass over the transcripts beside the block of each of the account's 1,487 pull requests, of which 1,473 had a block.
  1,033 blocks held what the transcripts give; 327 were counted anew and 5 pull requests that had no block got one, 3.4B tokens, ≈$1.7k and 31 agent-hours more than the blocks had summed before it (43.6B tokens, ≈$11.5k and 434 agent-hours with the sessions above).
  ≈$0.75k of it is the 15 pull requests on Mythos 5.2 that had counted no dollars (#2834); most of the rest is the beginning of a next task, which a session in a worktree does on the merged branch after that branch's last block (294 blocks, 2.75B tokens; #2852).
  A block was raised only where it equalled the tokens that its branch held at the last `pr_usage.py --update` in the transcripts, so that what the branch holds beyond it came after the block: today's count of an old session is not always the one of its day, a worktree being removed or on another branch since.
  Ten pull requests count the work done under another name of their branch (`--also`).
  Left as they were: 87 blocks that hold more than the transcripts give now (40 of them of 2026-10-18, whose transcripts Claude Code has removed after 30 days), 31 that differ from the transcripts both at their last run and now (0.5B tokens more in the transcripts), 3 pull requests without a block whose work lies under no name of theirs (#1623, #1881, #2349), and ≈0.3B tokens on branches of no pull request (research and scratch branches, removed worktrees).
