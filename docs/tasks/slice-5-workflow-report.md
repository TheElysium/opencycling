# Workflow Report: Slice 5 (Weekly Load Summary)

Written 2026-09-14, after slice 5 of [multi-week-training-plan.md](multi-week-training-plan.md)
shipped as commit `613711f`. Scope: how the orchestrated workflow actually ran on this
slice, what it caught, what it cost, and what to change before slice 6.

## 1. What ran

| Step | Agent | Outcome |
|---|---|---|
| Spec + decisions | orchestrator + 3 questions to the author | 3 product decisions settled before any code |
| Explore | `explore` | code map: metrics/ftp helpers, plan components, test conventions |
| Implement | `implementer` | full slice in TDD, red then green, 15 tests |
| Verify (round 1) | `gate-keeper` | all green except `gitleaks` (not installed) |
| Review (round 1) | `reviewer` | REQUEST_CHANGES: 1 major, 2 minor |
| Fix | orchestrator, in TDD | tests edited first, watched fail, then implementation |
| Verify + re-review (round 2) | `gate-keeper` + `reviewer` in parallel | green (minus gitleaks) + APPROVE |
| Commit | orchestrator, on explicit request | `613711f`, 6 files, +414/-8 |

Slice 5 touched no Rust and no bindings, so the whole loop stayed on the frontend.

## 2. Cost

| Agent | Tokens | Tool uses | Duration |
|---|---|---|---|
| explore | 49k | 57 | 166 s |
| implementer | 57k | 45 | 239 s |
| reviewer round 1 | 42k | 18 | 108 s |
| reviewer round 2 | 49k | 4 | 13 s |
| gate-keeper round 2 | 26k | 10 | 119 s |
| **Total logged** | **223k** | **134** | **645 s** |

Notes on these figures:

- `gate-keeper` round 1 was never logged. Its numbers are lost, which is exactly the
  failure mode the "log metrics as each subagent completes" rule exists to prevent.
- The parallel round-2 dispatch cut roughly 13 s of wall time off the critical path,
  a small win on this slice but a free one.
- `explore` was the most tool-hungry agent (57 uses for 49k tokens) and the least
  decisive: its output was a code map, not an answer to the one question that actually
  mattered (see friction 5).

## 3. What worked, keep it

- **TDD held through the review fixes, not just the initial build.** The major finding
  was fixed by editing `plan-load.test.ts` first, running it, watching
  `expected 3600 to be +0` fail on "trusts the backend flag over the index", then
  changing `plan-load.ts`. That is the part of the loop most easily skipped under
  "it is only a small fix", and skipping it here would have hidden that the old code
  passed the new test by accident.
- **The reviewer earned its cost.** The major finding was a cross-layer consistency bug:
  `weekLoad` derived `missing` from its own library scan while `PlanDayCell` colored the
  same day from the backend `PlanEntryView.missing` flag. Both were internally correct.
  No lint, no type check and no unit test could have caught the disagreement, because
  each layer was self-consistent. This is the class of defect that justifies a dedicated
  read-only reviewer over "the gate is green, ship it".
- **Re-review on the same agent id.** Resuming the round-1 reviewer kept its context, and
  round 2 cost 4 tool uses and 13 s against 18 and 108 s for round 1. The prompt still
  restated every change self-containedly, which is what made the cheap round possible
  rather than a full re-read of the diff.
- **Parallel gate-keeper and re-review.** Both are read-only on the same tree, so there
  is no reason to serialize them.
- **Decisions recorded in the plan doc as they were taken.** The "why the load is computed
  in the frontend" paragraph is the single most valuable artifact of this slice, and it
  would not exist if the decision had only lived in the conversation.

## 4. Friction

1. **No `.gates.yml` in this repo.** The global workflow mandates one at the repo root and
   says `gate-keeper` reads it verbatim. This repo has none: the commands live in prose in
   `AGENTS.md`, and each `gate-keeper` prompt restates them. That is a hand-copied gate
   list, which is how a command silently goes missing between slices.
2. **`scripts/gate.sh` is broken and stayed broken.** It invokes `cargo-audit` without the
   `audit` subcommand and fails locally. It was recorded as a known issue back in slice 3
   and worked around in every prompt since. A workaround repeated across three slices is a
   bug that should have been fixed the first time.
3. **`gitleaks` has been absent since slice 1.** The gate has therefore been reporting RED
   on a non-skippable SAST step for five slices, and the decision to accept the gap was
   only taken at slice 5. A red gate that everyone learns to read past stops being a gate.
   The gap is now explicitly accepted and dated in the plan doc, which is the right
   outcome, but it took far too long to surface.
4. **The plan doc status header was stale at session start.** It still described a state
   from an earlier slice. The per-slice sections were accurate; only the header at the top,
   the one thing a resuming session reads first, was wrong.
5. **The formula divergence surfaced during implementation, not during exploration.**
   The fact that `computeWorkoutMetrics` (plain 4th-power mean) and Rust
   `metrics::derive_metrics` (Coggan 30 s rolling average) disagree is what forced the
   whole slice into the frontend, against the original module sketch. `explore` ran for
   166 s and did not report it, because it was asked for a code map rather than for the
   specific question "where is planned TSS computed today, and is there more than one
   implementation".
6. **Manual QA is still open and no agent can close it.** The slice is committed and the
   visual check (bar rendering, deload week visibly shorter, `*` tooltip) has not run.
   It is tracked as an open checkbox, which is correct, but the commit went in ahead of it.
7. **Context compaction erased in-flight state.** Mid-slice, the conversation was
   summarized. Everything not written to the plan doc at the moment it happened had to be
   recovered from that summary. The plan doc survived; the round-1 gate-keeper metrics did not.

## 5. Refinements

Applied on 2026-09-14, right after this report was written:

- [x] **`.gates.yml` written at the repo root.** Keys `format`, `lint`, `typecheck`,
      `test`, `bindings`, `sast`, `size`, each a command runnable verbatim from the repo
      root. `gate-keeper` prompts shrink to "read `.gates.yml`, run each key, report",
      which removes the hand-copied command list of friction 1. A comment names the three
      copies that must change together: this file, `scripts/gate.sh`, `ci.yml`.
- [x] **Fixed the `cargo-audit` call in `scripts/gate.sh`.** Root cause: `cargo-audit` is
      a cargo subcommand binary. Cargo invokes it as `cargo-audit audit`, so its clap
      parser expects `audit` as argv[1]. Called bare it printed usage and exited non-zero,
      and the `2>/dev/null` swallowed the very message that explained it while the
      `|| cargo-audit.exe` fallback reproduced the same failure. Now `$CARGO audit`, which
      also reuses the `cargo.exe` detection already done at the top of the script.
- [x] **Recorded the accepted `gitleaks` gap inside `.gates.yml`**, on the `sast` key,
      with its date and the exact command to re-add. An accepted gap has to be visible
      where the gate is defined, not only in a task doc nobody opens during a gate run.
- [x] **Closed two drifts found while doing the above.** `scripts/gate.sh` claimed to
      mirror CI but had no bindings-drift check, so a stale `bindings.ts` could only be
      caught after pushing; it now regenerates and runs `git diff --exit-code`. In the
      other direction, CI never ran `pnpm test`, so every frontend unit test in the repo,
      including the 15 written for this slice, only ever ran on the author's machine; the
      `frontend` job now runs it (and is renamed, it was still called "svelte-check").
- [x] **`bash scripts/gate.sh` now runs green end to end**, for the first time since the
      `cargo-audit` bug landed: 9 test files, 99 frontend tests, 0 svelte-check errors and
      warnings, no bindings drift, 11 allowed audit warnings.

Still open, process rather than files:

- [ ] **Make the plan-doc status header part of the commit step.** The slice is not done
      until the header names the new commit and the next slice. Treat it like a version
      bump: same commit, not a follow-up edit.
- [ ] **Give `explore` a question, not a territory.** For slice 6, ask "where does the
      frontend learn a session's `session_id` today, and what is the exact call path from
      a day cell to a started session", not "map the session and plan modules". A named
      question is also a cheaper prompt to answer.
- [ ] **Log every subagent's metrics at completion, including gate-keeper rounds.** The
      round-1 gate-keeper line is missing from slice 5 and cannot be reconstructed.
- [ ] **Close slice 5 manual QA before starting slice 6 code.** It is the one open item
      standing between slice 5 and "done", and slice 6 builds on the same page.

## 6. Slice 6 is HITL, plan for it differently

Slice 6 (start a session from a day cell, link `session_id` back) is the first slice
classified HITL: its acceptance criterion needs a real ride on real hardware. The loop
above ends at APPROVE plus green gates, which for slice 6 proves strictly less than it
did for slice 5. Two adjustments:

- Split the slice so the testable part is testable: the pure "which entry does this
  session fulfill" mapping belongs in a unit-tested module, and only the actor wiring
  stays manual. `AGENTS.md` already excludes actors from TDD, so the more logic sits
  outside the actor, the more the gate actually covers.
- Write the manual QA script before implementing, not after: the exact click path, the
  expected state after finishing a session, and the expected state after deleting that
  session (`session_id` back to NULL, the planned entry untouched). A manual step with a
  written script is verification; one without is a hope.
