# Multi-Week Training Plan (manual, athlete-scheduled)

**Issue:** [github.com/TheElysium/opencycling/issues/15](https://github.com/TheElysium/opencycling/issues/15)
**Status:** slices 1-5 committed (latest `613711f`); slice 6a (link `session_id` back,
pure mapping) committed `ea8d947`; slice 6b (day-cell start affordance + manual QA) not started.

## Problem

Workouts are standalone `.zwo` files picked one at a time. There is no way to lay out
which workout goes on which day over several weeks, so the rider cannot see a block of
training as a whole, nor tell where they stand inside it.

## Solution

A named training plan: name, start date (a Monday), number of weeks. The rider assigns
existing library workouts to specific days of that plan, sees weekly load at a glance,
starts a session directly from a planned day, and the plan records which session
fulfilled which planned entry. No automatic adjustment: the rider stays in control.

## Decisions (agreed 2026-09-13)

1. **Absolute dates.** Every entry carries a real `YYYY-MM-DD` date. No template /
   instance split, no relative week numbering in storage.
2. **Named plans.** `training_plans` + `plan_entries`, not a single global calendar.
   Plans can be archived.
3. **One active plan at a time.** Non-archived plans may not overlap in time. The
   overlap check is a pure function, validated before insert/update.
4. **Week grid view.** One row per plan week, 7 day columns (Mon to Sun), plus a weekly
   summary column. No month calendar, no navigation outside the plan bounds.
5. **Click to assign.** Clicking a day opens a picker that reuses the library search and
   tag filters. No drag and drop in this version.
6. **`session_id` persisted.** Starting a session from a planned day links that entry to
   the resulting session id, so the link survives a session done on the wrong day.
7. **Plan shape is explicit.** `start_date` (forced to a Monday) + `weeks`, both editable.
8. **In scope for v1:** free-text note per day (including days with no workout, which is
   how cross-training and rest days are expressed), several entries per day
   (`position`), weekly planned load (duration + TSS), and a "today" card on the
   connection page with a direct start button.

### Derived decisions (not raised, taken by default)

- **Entry identity:** `plan_entries.file_name` references the library cache key
  (`workouts.file_name`), with `workout_name` denormalized alongside it. A deleted
  `.zwo` leaves a readable entry flagged as missing instead of a dangling row.
- **No `SessionActor` change:** the frontend already receives `session_id` in the
  `session_metrics` payload (`session/types.rs:182`). The plan entry id is held in a
  rune store while the session runs, and a dedicated command links the two once the
  session is finalized. Actors stay untouched and untested, per `AGENTS.md`.
- **Deleting a session** sets `plan_entries.session_id` back to NULL (`ON DELETE SET
  NULL`); it must never delete the planned entry.

## Out of scope

- Automatic plan adjustment based on performance or compliance.
- Plan templates, sharing, import/export, plan marketplace.
- Non-cycling activity tracking (swim/run are free-text notes only).
- Drag and drop reordering.
- Recurrence rules ("every Tuesday"): the rider assigns days explicitly.

## Data model (migration v9 -> v10, append only)

```sql
CREATE TABLE training_plans(
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    start_date  TEXT    NOT NULL,          -- ISO date, always a Monday
    weeks       INTEGER NOT NULL,
    created_at  TEXT    NOT NULL,
    archived_at TEXT                        -- NULL = active
);

CREATE TABLE plan_entries(
    id           INTEGER PRIMARY KEY,
    plan_id      INTEGER NOT NULL REFERENCES training_plans(id) ON DELETE CASCADE,
    date         TEXT    NOT NULL,          -- ISO date, inside the plan bounds
    position     INTEGER NOT NULL DEFAULT 0,
    file_name    TEXT,                      -- library cache key, NULL for a note-only day
    workout_name TEXT,                      -- denormalized display name
    note         TEXT,
    session_id   INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    CHECK (file_name IS NOT NULL OR note IS NOT NULL)
);

CREATE INDEX idx_plan_entries_plan ON plan_entries(plan_id, date, position);
```

## Module sketch

**Rust, new module `src-tauri/src/plan/`:**

- `types.rs` - `TrainingPlan`, `PlanEntry`, `PlanWeek`, `PlanDay`, `WeekLoad`, `NewPlan`,
  `NewEntry`. All `specta::Type`.
- `schedule.rs` (pure, TDD) - the whole deep module behind a small interface:
  - `plan_range(start_date, weeks) -> (NaiveDate, NaiveDate)` (end exclusive)
  - `overlaps(a: &TrainingPlan, b: &TrainingPlan) -> bool`
  - `validate_new_plan(candidate, active_plans) -> Result<(), AppError>`
  - `build_weeks(plan, entries, today) -> Vec<PlanWeek>` (grid shape, past/today/future)
  - `week_load(entries, flats_by_file_name, ftp_w) -> WeekLoad` (duration + TSS, reuses
    `metrics::derive_metrics`)
  - `entry_for_date(plan, entries, date) -> Option<&PlanEntry>` (today card)
- DB CRUD follows the existing actor pattern: new `DbCommand` variants and
  `DbActorHandle` methods in `db/command.rs`, helper methods in `db/actor.rs`.
- Tauri commands in `lib.rs::specta_builder()`, bindings regenerated.

**Frontend:**

- Routes `/plans` (list, create, archive) and `/plans/[id]` (the week grid).
- Sidebar gains a `Plan` item between `Workouts` and `History`.
- Components: `PlanWeekRow`, `PlanDayCell`, `WorkoutPicker` (reuses
  `workout-filter.ts`), `WeekLoadSummary`, `TodayCard`.
- `lib/plan.svelte.ts` - selected plan + the pending `plan_entry_id` of a session
  started from the grid.

## Slices (vertical, dependency ordered)

| # | Slice | Kind | Demoable outcome |
|---|-------|------|------------------|
| 1 | Schema + plan CRUD + `/plans` list page | AFK | Create, rename, archive and delete a plan; it survives a restart. |
| 2 | Empty week grid at `/plans/[id]` | AFK | Open a plan, see N week rows with 7 dated day cells, today highlighted. |
| 3 | Assign / replace / remove a workout on a day | AFK | Click a day, pick a workout from the library, it sticks after reload. |
| 4 | Free-text note per day (with or without a workout) | AFK | Write "swim 45min" on a blank Thursday. |
| 5 | Weekly load summary column | AFK | Each week row shows planned duration and TSS. |
| 6 | Start a session from a day cell, link `session_id` back | HITL | Launch from the grid, finish, the day shows as done (needs a real ride to verify). |
| 7 | Today card on the connection page | AFK | The home page shows today's planned workout with a start button. |

Slice 3 carries `position` support in the schema and the cell UI from the start, even
though a second entry per day only becomes reachable in slice 4.

## Gate

Every slice is done only when `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, `cargo test`, `pnpm check`, `pnpm lint`, `pnpm test` and
`bash scripts/check_size.sh` all pass. Bindings are regenerated with
`cargo run --bin export_bindings`, never hand edited.

## Progress

Branch: `feat/training-plans` (created 2026-09-13).

Orchestration decisions taken while implementing (not in the original spec):

- SQL helpers for plans live in a new `src-tauri/src/db/plan_store.rs` (free functions
  over `&Connection`), not in `db/actor.rs`: that file is already at 833 lines against a
  1000-line gate. The actor only gains its `match` arms.
- `validate_new_plan` takes an extra `editing_id: Option<i64>` so updating a plan does
  not make it overlap itself.
- Overlap validation runs in the Tauri command layer (`lib.rs`): read the active plans,
  validate, then insert. Single-user desktop app, no locking needed.
- `AppError::PlanValidation(String)` is the new error variant; it reaches the frontend
  as a bare string, displayed raw.

Decisions taken during the slice 1 review round (peer review returned
REQUEST_CHANGES on a real bug, then APPROVE):

- Archived-ness is an input to validation. `set_plan_archived(id, false)` reloads the
  row and revalidates against the active plans, otherwise archiving a plan, creating an
  overlapping one and unarchiving the first persisted two overlapping active plans.
  Conversely an archived plan skips the overlap check entirely so it stays renamable.
  Expressed by a pure `validate_plan_write(candidate, editing_id, archived, peers)`.
- `training_plans` gained `CHECK (weeks BETWEEN 1 AND 52)`, edited into the v10 script
  in place since nothing had shipped. A dev machine that already ran the app against the
  earlier v10 stays at `user_version = 10` without the constraint: delete the local dev
  database to pick it up.
- `plan_store` mutators map `rows_affected == 0` to the same not-found error `get`
  produces, instead of reporting success for a row that no longer exists.
- Name trimming is a pure `normalize_plan`, called before validating and storing, so
  validation and persistence cannot disagree. The store stays free of business rules.
- `plan-date.ts` keeps all arithmetic in UTC; only `todayMonday` reads the local
  calendar date, because "what day is it for this rider" is a local question.

Known follow-ups, deliberately not done in slice 1:

- `validate_plan_write` takes a positional `archived: bool` next to `editing_id`. A
  `PlanStatus` enum would make the unsafe direction unrepresentable. Worth doing when
  slice 6 adds a second branching write path.
- `vite.config.ts` does not pin `TZ`, so the `todayMonday` tests discriminate against a
  UTC-based regression only when the runner is not itself on UTC.
- "Not found" rides on `AppError::PlanValidation`, so it surfaces as
  `Invalid plan: plan 7 not found`. Deserves its own variant once a second not-found
  case appears.
- SAST is incomplete: `cargo audit` passes on its known baseline, but `gitleaks` is not
  installed on the dev machine.

Decisions taken during the slice 2 review round (peer review APPROVED twice, second
round on the incremental fixes):

- `build_weeks` rejects `weeks == 0` with a `PlanValidation` error: write-time
  validation (`CHECK weeks BETWEEN 1 AND 52`) cannot be trusted for rows corrupted
  outside the app, and a silently empty grid would look like a plan with no weeks.
- `dayOfMonth` lives in `plan-date.ts` (pure, UTC-only, returns 0 on unparseable
  input) instead of an inline `new Date(...)` in `PlanDayCell`, so all date
  arithmetic stays in one tested file.
- Accepted for now, revisit when slice 3 touches the page: the detail page reads the
  plan row twice (`getPlan` + `getPlanWeeks` each fetch it); one command returning
  `{ plan, weeks }` would fix it. `.gitignore` blanket-ignores `.claude/`,
  `.opencode/`, `.idea/` — this also hides shareable project-level agent overrides;
  narrow it if those need to be committed.
- `cargo fmt` reformatted pre-existing uncommitted schedule.rs/types.rs code.

Subagent metrics (slice 2): implementer `ses_f6539fd14ffec2BxzAoE9eyjsz`
(command + bindings + frontend); gate-keeper `ses_f653364deffeGt991jTZrGG4GM`
(8 gates green), `ses_f653065f1ffeGYeiEL95wtPCsN` (7 gates green after fixes);
reviewer `ses_f65335738ffeK05VqPkG28k28C` (APPROVE then APPROVE on fixes).

Decisions taken while implementing slice 3:

- Archived plans reject entry writes (create/update/delete): the rider confirmed
  archiving means frozen. Rejection message points at unarchiving. Past days are
  assignable (any day inside plan bounds; rider logs retroactively).
- `build_weeks` now takes `(plan, entries, known_files, today)` per the original
  module sketch: entries attach to their day (sort position then id), out-of-bounds
  entries are ignored (corrupt row must not fail the whole grid), `missing` flag is
  computed against the library `workouts.file_name` set.
- Replacing a workout on an entry sets `session_id` back to NULL: the linked session
  fulfilled the previous workout.
- The detail page reads the plan row twice (`getPlan` + `getPlanWeeks`); still
  accepted for now — revisit when the today card or per-day fetches land.
- Known follow-up (reviewer): a parsed workout could carry a file_name but a null
  name, persisting an empty `workout_name`; backend non-empty check worth adding
  when note-entries land in slice 4.
- Known issue (pre-existing, not this slice): `scripts/gate.sh` invokes
  `cargo-audit` without the `audit` subcommand, so the wrapper script fails under
  WSL where only `cargo-audit.exe` exists; `cargo audit` itself passes.

Subagent metrics (slice 3): explore `ses_f652a4061ffeFTUBGJUm9v8jWg`;
implementer `ses_f652688adffe0WaS34VKplp02l` (full slice); gate-keeper
`ses_f650f3814ffeoxPE4DcbsM17GN` (8 gates green), `ses_f650b0393ffeNs9yHAWtLy7BV7`
(fmt drift caught, fixed); reviewer `ses_f650f1f17ffebo37BcbazCvjQ3`
(REQUEST_CHANGES on double-submit race, then APPROVE on fixes).

Decisions taken while implementing slice 5:

- The weekly load is computed **in the frontend**, not in Rust, against the original
  module sketch. Reason: two planned-load formulas already exist and disagree.
  `computeWorkoutMetrics` (`src/lib/metrics.ts:86`, used by `/workouts`) takes a plain
  4th-power mean over the flattened 1 Hz series, while `metrics::derive_metrics`
  (`src-tauri/src/metrics.rs:91`, used for real sessions) applies the Coggan 30 s
  rolling average. A Rust `week_load` would have shown a different TSS in the grid than
  the library shows for the same workout, or required mirroring the TS formula into a
  third place. Reusing the TS function keeps the numbers consistent by construction and
  adds zero duplication. `plan/schedule.rs::week_load` is therefore not built, and
  `build_weeks` keeps its `known_files: HashSet<String>` signature. Unifying both
  formulas behind a single Rust computation (and rewiring `/workouts` onto it) is the
  real fix, deliberately deferred: it changes the TSS values already displayed.
- The summary cell shows duration, TSS and a mini bar normalized on the plan's largest
  week TSS, so deload weeks read at a glance.
- Entries whose `.zwo` left the library are excluded from the totals and the week is
  flagged incomplete (`*` + tooltip). A silently under-reported total is worse than an
  annotated one.
- FTP tests contribute duration but not TSS: their blocks are authored against
  `FTP_TEST_REFERENCE_W`, so their TSS is not comparable. Mirrors the library page,
  which already hides TSS for FTP tests.
- Layout: `PlanWeekRow` stacks its label above a 7-column day grid, so the "summary
  column" lands right-aligned on the week-label line rather than as an 8th column,
  which would have squeezed the day cells.
- Slice 5 touches no Rust and no bindings.

Subagent metrics (slice 5): explore 49k tokens / 57 tool uses / 166 s (code map);
implementer 57k tokens / 45 tool uses / 239 s (full slice, red then green);
reviewer round 1 42k tokens / 18 tool uses / 108 s (REQUEST_CHANGES, 1 major + 2 minor);
reviewer round 2 49k tokens / 4 tool uses / 13 s (APPROVE);
gate-keeper round 2 26k tokens / 10 tool uses / 119 s (all green except gitleaks).

Workflow retrospective for this slice (cost, friction, refinements to apply before
slice 6): [slice-5-workflow-report.md](slice-5-workflow-report.md).

Slice 6 is split per that report's §6 (HITL, plan for it differently):

- **6a (AFK, TDD):** a dedicated command to set `plan_entries.session_id` after a ride,
  plus the pure frontend gate deciding *when* to call it.
- **6b (HITL):** the day-cell start affordance, the session store wiring, and the manual
  QA that only a real ride can close.

Explore findings used to design 6a/6b (`ses` id logged below):

- `SessionMetrics.session_id` (`session/types.rs:182`) already reaches the frontend via
  the `session_metrics` event into `session.svelte.ts`'s `metrics` state
  (`session.svelte.ts:94-101`); no store field isolates it today, consumers read
  `session.metrics.session_id` directly.
- Starting a workout today: `workoutSelection.workout` (`lib/workout.svelte.ts:16-18`) to
  `/workouts/detail`, `startRide()` calls `session.prepare(w, ftp, aero)`
  (`detail/+page.svelte:33`) which stores private `pendingWorkout`/`pendingFtp`
  (`session.svelte.ts:32-40`), then `/session`'s `onMount` calls `startPending()`
  (`+page.svelte:69-85`) which calls `commands.startSession(...)`.
- `plan_entries` has **no** dedicated "set session_id" command. `EntryContent`
  (`plan/types.rs:66-71`) only carries `file_name`/`workout_name`/`note`; `update_entry`
  (`plan_store.rs:154-173`) clears `session_id` via the pure `clears_session` decision but
  never sets it. The only existing `UPDATE ... SET session_id` is a test-only helper,
  `plan_store.rs:448-455`.
- Session deletion (`db/actor.rs:551-556`) is a bare `DELETE FROM sessions`; the
  `ON DELETE SET NULL` FK does the rest at the SQLite level, exercised today only by a
  migration test (`migrations.rs:210-230`). No frontend command or store reacts to it -
  correct, nothing needs to: the day cell just re-reads `plan_entries` on next load.
- `PlanDayCell.svelte` never reads `entry.session_id` today; the whole cell is one button
  that opens the assignment picker. There is no start affordance yet.

Decisions for 6a:

- New `DbCommand::LinkPlanEntrySession { entry_id, session_id }`, handle method
  `link_plan_entry_session`, backed by a promoted (non-test) `plan_store::link_session`
  (rows_affected == 0 -> the same not-found error as `delete_entry`, per the slice 1
  convention). The existing test helper of the same name is renamed
  `seed_linked_session` to avoid confusion with the production function it now exercises.
- No archived-plan gate on this write: linking a finished ride records history, it is not
  editing plan content, so it stays allowed even on an archived plan (asymmetric with
  slice 3's create/update/delete rejection, and worth a one-line comment saying so).
- New Tauri command `link_plan_entry_session_cmd`, added to `specta_builder`, bindings
  regenerated.
- Pure frontend gate, `src/lib/plan-link.ts` (new, unit-tested): a function deciding
  exactly once per finished ride whether to call the link command, given only the *current*
  session state, the session id, and the already-linked session id (no previous-state
  parameter: the real caller, `ingestMetrics`, ticks every second with no transition tracked,
  so a state-based check also fires correctly for a page mounting already `Finished`) - a
  repeated `Finished` tick never double-calls the command, a session with no pending plan
  entry never calls it, and a null `session_id` never calls it.

Decisions for 6b:

- `PlanDayCell.svelte`'s outer element changes from `<button class="cell">` to
  `<div class="cell" role="button" tabindex="0">` (click / Enter / Space still opens the
  picker), which frees each entry with `file_name && !missing` to carry a real nested
  `<button>` "Start" affordance (`Play` icon, matching the icon set already used for
  session controls) without violating button-in-button nesting. The start click calls
  `stopPropagation()` so it does not also open the picker. An entry with `session_id`
  set gets a `CircleCheck` "done" badge, but its Start button stays enabled: redoing a
  day (manual QA step 6) must remain possible.
- No archived-plan gate on starting a ride from a day cell, symmetric with 6a's decision
  on the link write itself: starting a ride records history, it does not edit plan
  content.
- `/plans/[id]/+page.svelte` gets `startEntry(entry)`: resolves the `ParsedWorkout` via
  the existing `workoutIndex` (keyed by `file_name`, already built for the load summary),
  reads `aero_enabled` from the settings already fetched in `loadLibrary()`, calls
  `session.prepare(workout, workoutFtp(workout, libraryFtp), aero, entry.entry_id)`, then
  `goto('/session')`. A workout not found in the index (library not scanned yet, or the
  file went missing) shows the existing error box and does not navigate.
- `session.svelte.ts`'s `prepare()` gains a 4th optional parameter `planEntryId: number |
  null = null`, stored privately (`pendingPlanEntryId`) and cleared in `reset()`.
  `ingestMetrics()` calls the already-tested `shouldLinkPlanEntry` (from `plan-link.ts`)
  on every tick; when it returns true, `commands.linkPlanEntrySessionCmd(...)` fires
  fire-and-forget with `.catch(() => {})`, matching the existing convention at
  `aero.svelte.ts:227` (a link failure must not crash the live session UI), and the
  linked `session_id` is recorded privately so the next tick does not call it again.
- No new unit test for this slice: it is plumbing (store + page wiring), which is what
  the HITL classification exists for; the pure decision it drives (`shouldLinkPlanEntry`)
  is already covered by 6a's tests. Returning to `/plans/[id]` re-triggers the existing
  `onMount` -> `load()` -> `getPlanWeeks`, so the "done" badge appears with no extra
  wiring.

Manual QA script for 6b, written before implementing it:

1. Open a plan at `/plans/[id]` with a workout assigned on a day cell.
2. Click the day's new start affordance -> lands on `/session` running that workout
   (or aero calibration first, if aero is enabled).
3. Finish the ride (or use `OPENYCLING_SIM=1` to reach `Finished` quickly).
4. Go back to `/plans/[id]`: the day cell shows as done (its `session_id` is set); the
   session appears in `/history`.
5. Delete that session from its `/history/[id]` page, return to `/plans/[id]`: the day
   cell reverts to not-done, but the workout assignment (`file_name`/`workout_name`/
   `note`) is untouched.
6. Redo the same day (start again, finish again): the day cell links to the new session,
   overwriting the old (now-deleted) id.

Pending before slice 5 can be called done:
- [x] Manual QA (no automated gate covers CSS/layout): run `pnpm tauri dev`, open a plan
      at `/plans/[id]`, check the duration / TSS / bar rendering on the week label line,
      that the deload week's bar reads visibly shorter, and that a week with a deleted
      `.zwo` shows the `*` with its tooltip. Confirmed by the author 2026-09-14.
- [x] SAST gap: `gitleaks` is not installed on this machine (pre-existing since
      slice 1). Decided on 2026-09-14 to accept the gap rather than install it, so
      the secrets scan stays absent from the local gate; `cargo audit` still runs.

Review round 1 findings, all fixed before the approval:
- major: `weekLoad` derived `missing` from its own library scan, which could contradict
  the backend `PlanEntryView.missing` flag that colors the day cells. It now counts
  `missing` from that flag only; an entry absent from the index but not flagged
  contributes nothing and is not reported missing (library not scanned yet).
- minor: the TSS guard rounded after testing `> 0`, so a sub-0.5 TSS week rendered
  "0 TSS". The guard now tests the rounded value.
- minor: a 3-line comment exceeded the 2-line rule.

| # | Slice | Status |
|---|-------|--------|
| 1 | Schema + plan CRUD + `/plans` list page | done, committed `b5439af` |
| 2 | Empty week grid at `/plans/[id]` | done, committed `7469806` |
| 3 | Assign / replace / remove a workout on a day | done, committed `5161ceb` |
| 4 | Free-text note per day | done, committed `1a7473c` |
| 5 | Weekly load summary column | done, committed `613711f`, manual QA confirmed |
| 6 | Start a session from a day cell, link `session_id` | 6a done, committed `ea8d947`; 6b implemented + APPROVE + gates green, uncommitted; manual QA pending |
| 7 | Today card on the connection page | not started |

## Slice 6a orchestration log, 2026-09-14

Subagent metrics (tokens / tool uses / duration), logged at each subagent's completion:

| Agent | Round | Tokens | Tool uses | Duration |
|---|---|---|---|---|
| explore | - | 57249 | 42 | 155841 ms |
| implementer | 1 | 93746 | 46 | 447844 ms |
| gate-keeper | 1 | 58792 | 8 | 91844 ms |
| reviewer | 1 | 76439 | 19 | 186068 ms |
| gate-keeper | 2 (after fixes) | 53249 | 12 | 150412 ms |
| reviewer | 2 (resumed, after fixes) | 81353 | 2 | 18292 ms |

Review round 1: **APPROVE with 2 minor findings**, both fixed and re-approved in round 2:
- `plan-link.ts`'s JSDoc on `shouldLinkPlanEntry` ran 4 lines against the 2-line comment
  rule; trimmed to a 2-line `//` comment keeping only the load-bearing "why".
- The "Decisions for 6a" text above originally said the link gate decides "given the
  previous and new session state"; the shipped function only takes the *current* state
  (correct, matches `ingestMetrics` which ticks with no transition tracked) - text
  corrected in place above rather than left inconsistent with the code.

Gate-keeper found a real bindings drift in round 1 (`bindings.ts` was missing
`linkPlanEntrySessionCmd` despite the implementer's report claiming regeneration);
fixed by re-running `cargo run --bin export_bindings` from `src-tauri/` (orchestrator,
not a subagent - mechanical regeneration of a generated file, no TDD applies).

Gate-keeper round 2 still reports the `bindings` key as FAIL. This is not a real drift:
`.gates.yml`'s bindings check is `git diff --exit-code` against the git index, which
equals `HEAD` while nothing is staged - so it reads FAIL for any legitimate uncommitted
new command, including this one, until slice 6a is committed. Verified directly: running
`export_bindings` twice in a row produces the exact same single-line diff
(`linkPlanEntrySessionCmd`) both times, i.e. regeneration is idempotent and the working
tree already matches what the Rust source produces - the only thing "failing" is that
this hasn't been committed yet. Re-run this gate key once more right after commit to
confirm it goes green against the new `HEAD`; if it doesn't, that would be a real bug.

Result: all 7 gate keys pass in substance (bindings pending the commit above); reviewer
APPROVE stands on the current diff. Not committed yet - pending explicit request per
project convention on this task.

## Slice 6b orchestration log, 2026-09-14

Subagent metrics (tokens / tool uses / duration), logged at each subagent's completion:

| Agent | Round | Tokens | Tool uses | Duration |
|---|---|---|---|---|
| implementer | 1 | 77742 | 36 | 150453 ms |
| gate-keeper | 1 | 57581 | 9 | 123401 ms |
| reviewer | 1 | - | - | failed (session rate limit, see below) |

Implementer report: `PlanDayCell.svelte`'s outer element changed from `<button>` to
`<div role="button" tabindex="0">` (click + Enter/Space) so entries can carry a real
nested `<button>` Start affordance (`Play` icon, entries with `file_name && !missing`)
and a `CircleCheck` done badge (entries with `session_id != null`, Start stays enabled
so redoing a day is still possible); `PlanWeekRow.svelte` forwards the new `onstart`
prop; `session.svelte.ts`'s `prepare()` gained an optional `planEntryId` 4th param,
`ingestMetrics()` now calls `shouldLinkPlanEntry` and fires
`commands.linkPlanEntrySessionCmd(...).catch(() => {})` (fire-and-forget, matching the
existing convention at `aero.svelte.ts:227`); `/plans/[id]/+page.svelte` gained
`startEntry()` resolving the workout via the existing `workoutIndex` and navigating to
`/session` after `session.prepare(...)`. One deviation: the "workout not found" error
string uses a comma instead of an em dash, per the project's no-em-dash convention.
Self-reported gates: `pnpm check` 0/0, `pnpm test` 105 tests green, `pnpm lint` 0/0.

Gate-keeper round 1: 6/7 green (`format`, `lint`, `typecheck`, `test` 216 Rust + 105
vitest, `bindings` no drift, `size`). `sast` failed: `cargo audit` found
RUSTSEC-2026-0285 (rustls 0.23.40, TLS 1.3 handshake messages accepted across
encryption level boundaries, medium 5.3), dated 2026-09-14, i.e. new today and not a
previously accepted gap. Not related to the slice 6b diff (frontend-only); rustls is a
transitive dependency pulled in only via `reqwest`'s `rustls-tls` feature (not pinned
in `Cargo.toml`). Fixed directly rather than accepted as a gap, since the advisory
names a fix (`>=0.23.45`) that a plain `cargo update -p rustls --precise 0.23.45`
satisfies with no `Cargo.toml` change: bumped `rustls` 0.23.40 -> 0.23.45 (and
`rustls-webpki` 0.103.13 -> 0.103.15 as its dependent), re-ran `cargo audit` (exit 0,
advisory gone, still the same 11 pre-existing allowed unmaintained/unsound warnings)
and `cargo check --all-targets` (compiles clean) to confirm nothing else moved. This
touches only `Cargo.lock`, orchestrator-applied (mechanical dependency bump, no
production logic, no TDD applies), not part of the reviewed 6b diff.

Reviewer round 1 hit the session's rate limit mid-review (HTTP 429, "You've hit your
session limit", resets 7:50pm Europe/Paris) after confirming `session_id` field-name
consistency between `PlanEntryView` and the link command and starting to check the
keyboard-equivalence and `busy`/`readonly` prop interaction; no verdict produced.
Re-run once the limit resets.

Reviewer round 1 (retry, 74894 tokens / 17 tool uses / 227108 ms): **REQUEST_CHANGES**,
1 major + 3 minor. Major: `PlanDayCell.svelte`'s `cellKeydown` is bound on the outer
`.cell` div with no `target`/`currentTarget` guard; `keydown` bubbles from the nested
Start `<button>`, so Enter/Space while it is focused also fired the div's handler,
which called `preventDefault()` (canceling the button's own native activation) and
`onopen?.(...)` - keyboard users could not activate Start at all, it silently opened
the picker instead. Mouse click was already fine (`startClick` calls
`stopPropagation()`); keydown had no equivalent guard. Minor: `startEntry` didn't
reset `error` first, unlike the `mutate()` convention in the same file; the `.start-btn`
button had no explicit `type="button"`; two informational-only notes requiring no
action (`libraryAero`-from-settings-only vs `/workouts/detail`'s per-ride toggle,
matches the agreed spec; `SessionStore.reset()` has no current call site, pre-existing).

Fixed directly by the orchestrator, not delegated to `implementer`: this repo has no
component-test tooling at all (`grep` for `testing-library`/`jsdom`/`happy-dom` in
`package.json` finds nothing, no `*.test.*` file exists under
`src/lib/components/`), so DOM event wiring in a `.svelte` file is exactly the
irreducibly-manual part TDD does not reach here - the fix is a one-line guard
(`if (event.target !== event.currentTarget) return;` as the first line of
`cellKeydown`), plus the two trivial minors (`type="button"`, `error = null` first in
`startEntry`). Both spec-matching minors left as-is.

Reviewer round 2 (resumed, same agent, 80026 tokens / 2 tool uses / 17305 ms):
verified the guard closes the bug (bubbled Enter/Space now returns early, so the
button's native activation reaches `startClick` -> `onstart` instead of `onopen`) and
both trivial minors are applied exactly as described. **APPROVE**, same two
informational minors repeated (non-blocking).

Gate-keeper round 2 (58128 tokens / 10 tool uses / 215763 ms): all 7 keys PASS,
including `sast` now clean (the rustls fix above holds, 11 pre-existing allowed
warnings only, no advisory), `bindings` no drift, 210+6 Rust tests and 105 vitest
tests green.

Result: reviewer APPROVE on the latest diff (round 2) + all 7 gate keys green
(round 2). Not committed yet - pending explicit request per project convention on
this task. Manual QA script above still needs a real ride before slice 6 as a whole
is "done".
