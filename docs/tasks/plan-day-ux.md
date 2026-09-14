# Plan day-cell redesign and Plans page restructure

Source: [handoff-plan-day-ux.md](../handoff-plan-day-ux.md). Not a slice of
`multi-week-training-plan.md` (explicitly run in its own session per that handoff).
Branch `feat/training-plans` (same branch as the parent plan, uncommitted alongside
slice 6b at the time this work started).

## Status

Both asks implemented, gates green, reviewer APPROVE (round 2). **Not committed**
pending explicit request, per project convention. Manual QA not yet run.

## Decisions resolved this session

- **FTP-test week-summary "bug"**: not a code bug. `weekLoad()` increments `workouts`
  before the FTP-test TSS exclusion, so an indexed FTP test still produces a
  duration-only summary. The screenshot issue is most likely a library-scan or
  filename-mismatch data issue on the real "test" plan, confirm on real data
  (handoff QA item 5, still open, requires the author's own library/plan state).
- **No active plan covers today**: show a clear empty-state message ("No active plan
  this week."), not a hidden section (author's choice, `AskUserQuestion`).
- **Multiple active plans covering today**: moot. `check_no_overlap` in
  `src-tauri/src/plan/schedule.rs` already forbids overlapping active plans, so
  `currentPlan()` needs no tie-break (author's own answer: "un seul plan actif
  autorisé").

## Implementation summary

**Ask 1 (day-cell restyle + intensity indicator)**
- `src/lib/plan-load.ts`: new pure `entryIntensity(entry, index, ftpWatts): WorkoutType | null`,
  mirrors `weekLoad`'s no-data precedence (missing > file_name null > unindexed > classified);
  `entryIntensities(weeks, index, ftpWatts)` folds it over a whole plan into one
  `entry_id -> WorkoutType | null` map. Kept as a plain function in this module (not a
  `$derived.by` building a `Map` in the component) so `svelte/prefer-svelte-reactivity`
  stays green, same reasoning as `indexByFileName`.
- `src/routes/plans/[id]/+page.svelte`: `planIntensities = $derived(entryIntensities(...))`,
  passed to `PlanWeekRow` as `intensities`.
- `PlanWeekRow.svelte`: forwards `intensities` to `PlanDayCell`.
- `PlanDayCell.svelte`: `.entry` raised to `0.75rem`/600 (primary content); `.start-btn`
  restyled as a filled `1.35rem` circular accent button with a white-tinted fill inside
  `.today` cells; intensity dot (`workoutTypeColor()`) rendered before the name when known,
  nothing when null/unindexed.

**Ask 2 (`/plans` current-week section)**
- `src/lib/plan-date.ts`: `todayIso(now)` added, `todayMonday` refactored to delegate to it
  (no behavior change, existing tests unmodified and green).
- `src/lib/plan-current.ts` (new): `currentPlan(plans, today)`, single active plan whose
  `[start_date, planEndDate]` range contains `today`.
- `src/routes/plans/+page.svelte`: new read-only section above the untouched management UI
  (create form + active/archived lists). Loads settings/library best-effort, resolves the
  current plan and its "Today"-marked week (backend marker reused, not recomputed), renders
  `PlanWeekRow` read-only with a plan-wide `maxTss` (matches `/plans/[id]`'s scaling), and a
  distinct "Loading..." vs. confirmed-empty state to avoid a flash of the empty message.

## Review round 1 findings (all fixed, round 2 = APPROVE)

- **major**: `/plans/+page.svelte` passed the current week's own `tss` as `maxTss`,
  making the load bar always full. Fixed: `maxTss` now `maxWeekTss()` across all of the
  current plan's weeks (already fetched to locate "today"), matching `/plans/[id]`.
- **minor**: `plan-current.ts` doc comment exceeded 2 lines. Trimmed.
- **minor**: `PlanDayCell.svelte` re-guarded `entry.missing` redundantly on top of
  `entryIntensity`'s own null-for-missing rule. Dropped the redundant ternary.

## Gate results (final, full `.gates.yml`)

| key | result |
|---|---|
| format | PASS |
| lint (clippy + eslint) | PASS |
| typecheck (`pnpm check`) | PASS, 0 errors/0 warnings |
| test (`cargo test` + `pnpm test`) | PASS, 216 Rust + 117 frontend |
| bindings | PASS, no drift |
| sast (`cargo audit`) | PASS (11 pre-existing informational advisories, no vulnerability) |
| size | PASS |

Re-run after the review-fix round: `pnpm check` / `pnpm lint` / `pnpm test` all green
(frontend-only fix, Rust side untouched, not re-run in full a second time).

## Subagent orchestration log

| round | agent | task | tokens | tool_uses | duration |
|---|---|---|---|---|---|
| 1 | implementer (fallback: general-purpose) | Ask 1: `entryIntensity` + day-cell restyle | 82,596 | 30 | 193s |
| 1 | implementer (fallback: general-purpose) | Ask 2: `currentPlan`/`todayIso` + `/plans` section | 85,923 | 30 | 255s |
| 2 | gate-keeper (fallback: general-purpose) | full `.gates.yml` run | 51,211 | 13 | 172s |
| 2 | reviewer (fallback: general-purpose) | review round 1 | 81,836 | 28 | 206s |
| 3 | implementer (fallback: general-purpose) | fix 1 major + 2 minor findings | 65,514 | 15 | 100s |
| 4 | reviewer (fallback: general-purpose) | review round 2 | 77,758 | 19 | 145s |

Note: `~/.claude/agents` was a broken symlink at session start (target
`agent-workflow/claude/agents`, missing the leading dot); fixed early this session, but
the custom `implementer`/`gate-keeper`/`reviewer` subagent types still did not resolve
in this environment's Agent tool (available types listed `general-purpose`, `Explore`,
`Plan`, etc. only). All dispatches above used `general-purpose` with the role's
instructions embedded directly in the delegation prompt.

## Manual QA script (run once, not yet done)

1. Open `/plans/[id]` for a plan with workouts of different intensities (recovery,
   endurance, threshold, VO2max) across different days. Confirm: the workout name is
   clearly the dominant text in the cell, the Start button reads as a real button (not
   a faint icon), and each entry's intensity dot color matches `workoutTypeColor()`'s
   existing mapping (cross-check against `WorkoutPreview`/`ZoneBar` elsewhere in the app
   for the same workout).
2. On that same page, confirm a day cell whose workout is not in the scanned library
   (or a plan opened with no library configured) shows no intensity dot rather than a
   wrong color, and that a `missing` entry (deleted file) also shows no dot.
3. Open `/plans` with exactly one active plan covering today. Confirm the new
   "current week" section shows that plan's name (linking to `/plans/{id}`), the
   correct week, and a load bar fill level that visually matches what `/plans/[id]`
   shows for that same week (same TSS text, same relative bar fill).
4. Open `/plans` with no active plan covering today (archive the only active plan, or
   use one entirely in the future/past). Confirm a clear "No active plan this week."
   message, no crash, no stale data, and no visible flash of that message before the
   plans/library finish loading.
5. Confirm the existing `/plans` management UI (create form, active/archived lists,
   edit/archive/delete) is pixel-for-pixel unchanged below the new section.
6. Re-check the FTP-test week-summary question from the handoff on the real "test"
   plan: scan its library, confirm the FTP-test's `.zwo` file name is actually indexed,
   and confirm week 1's summary now shows a duration (TSS still excluded, by design).
