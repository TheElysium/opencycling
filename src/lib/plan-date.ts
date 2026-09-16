// All arithmetic stays in UTC: `new Date('2026-09-14')` is UTC midnight, and a
// rider in UTC-5 must not see the previous day. Only `todayMonday` reads local time.

import type { TrainingPlan } from '$lib/bindings';

const MS_PER_DAY = 86_400_000;
const DAYS_PER_WEEK = 7;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

function toIso(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

function pad2(value: number): string {
  return String(value).padStart(2, '0');
}

function isParsable(isoDate: string): boolean {
  return !Number.isNaN(Date.parse(isoDate));
}

/** The day before an ISO date, e.g. to turn an EXCLUSIVE end into an inclusive display end. */
function dayBefore(isoDate: string): string {
  return toIso(Date.parse(isoDate) - MS_PER_DAY);
}

/**
 * Snaps any ISO `YYYY-MM-DD` date back to the Monday of its week.
 * Contract: an unparseable input is returned unchanged, never throws.
 */
export function mondayOf(isoDate: string): string {
  if (!isParsable(isoDate)) return isoDate;
  const ms = Date.parse(isoDate);
  // getUTCDay() is 0 on Sunday, which is 6 days past Monday in an ISO week.
  const weekday = new Date(ms).getUTCDay();
  const backDays = (weekday + 6) % DAYS_PER_WEEK;
  return toIso(ms - backDays * MS_PER_DAY);
}

/**
 * Today as a plain ISO `YYYY-MM-DD`, from the rider's LOCAL calendar date: "what day is
 * it here" is the one question UTC cannot answer (UTC+13 on a Monday morning is Sunday).
 */
export function todayIso(now: Date = new Date()): string {
  return `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
}

/** Today's Monday, from the rider's LOCAL calendar date (see todayIso). */
export function todayMonday(now: Date = new Date()): string {
  return mondayOf(todayIso(now));
}

/**
 * Day-of-month number for a day cell, e.g. `'2026-09-14'` -> `14`.
 * Contract: an unparseable input returns 0, never throws.
 */
export function dayOfMonth(isoDate: string): number {
  if (!isParsable(isoDate)) return 0;
  return parts(isoDate).day;
}

function parts(isoDate: string): { day: number; month: string; year: number } {
  const d = new Date(isoDate);
  return { day: d.getUTCDate(), month: MONTHS[d.getUTCMonth()], year: d.getUTCFullYear() };
}

/**
 * e.g. "14 Sep – 11 Oct 2026". `endDateExclusive` is the backend's EXCLUSIVE end
 * (`TrainingPlan.end_date`); the label shows the day before it.
 * Contract: an unparseable input is returned unchanged, never throws.
 */
export function formatPlanRange(startDate: string, endDateExclusive: string): string {
  if (!isParsable(startDate) || !isParsable(endDateExclusive)) return startDate;
  const a = parts(startDate);
  const b = parts(dayBefore(endDateExclusive));
  if (a.year !== b.year) return `${a.day} ${a.month} ${a.year} – ${b.day} ${b.month} ${b.year}`;
  const head = a.month === b.month ? `${a.day}` : `${a.day} ${a.month}`;
  return `${head} – ${b.day} ${b.month} ${b.year}`;
}

/** The single active plan covering `today` (ISO date), if any; the backend forbids
 *  overlapping active plans (src-tauri/src/plan/schedule.rs, check_no_overlap). */
export function currentPlan(plans: TrainingPlan[], today: string): TrainingPlan | null {
  return (
    plans.find((p) => p.archived_at === null && p.start_date <= today && today < p.end_date) ?? null
  );
}
