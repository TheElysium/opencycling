# Multi-Week Training Plan (manual, athlete-scheduled)

**Issue:** [github.com/TheElysium/opencycling/issues/15](https://github.com/TheElysium/opencycling/issues/15)
**Status:** slice 1 done (reviewed, gates green, not committed); slice 2 next.

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

| # | Slice | Status |
|---|-------|--------|
| 1 | Schema + plan CRUD + `/plans` list page | done, awaiting commit |
| 2 | Empty week grid at `/plans/[id]` | not started |
| 3 | Assign / replace / remove a workout on a day | not started |
| 4 | Free-text note per day | not started |
| 5 | Weekly load summary column | not started |
| 6 | Start a session from a day cell, link `session_id` | not started |
| 7 | Today card on the connection page | not started |
