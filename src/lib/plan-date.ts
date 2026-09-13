// All arithmetic stays in UTC: `new Date('2026-09-14')` is UTC midnight, and a
// rider in UTC-5 must not see the previous day. Only `todayMonday` reads local time.

const MS_PER_DAY = 86_400_000;
const DAYS_PER_WEEK = 7;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

function toIso(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

function pad2(value: number): string {
  return String(value).padStart(2, '0');
}

function isParsable(isoDate: string, weeks = 0): boolean {
  return !Number.isNaN(Date.parse(isoDate)) && Number.isFinite(weeks);
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
 * Today's Monday, from the rider's LOCAL calendar date: "what day is it here"
 * is the one question UTC cannot answer (UTC+13 on a Monday morning is Sunday).
 */
export function todayMonday(now: Date = new Date()): string {
  return mondayOf(`${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`);
}

/**
 * Last day of the plan, INCLUSIVE, for display.
 * Mirror of `plan_range` in src-tauri/src/plan/schedule.rs, whose end is EXCLUSIVE.
 * Contract: an unparseable input is returned unchanged, never throws.
 */
export function planEndDate(startDate: string, weeks: number): string {
  if (!isParsable(startDate, weeks)) return startDate;
  return toIso(Date.parse(startDate) + (weeks * DAYS_PER_WEEK - 1) * MS_PER_DAY);
}

function parts(isoDate: string): { day: number; month: string; year: number } {
  const d = new Date(Date.parse(isoDate));
  return { day: d.getUTCDate(), month: MONTHS[d.getUTCMonth()], year: d.getUTCFullYear() };
}

/**
 * e.g. "14 Sep – 11 Oct 2026".
 * Contract: an unparseable input is returned unchanged, never throws.
 */
export function formatPlanRange(startDate: string, weeks: number): string {
  if (!isParsable(startDate, weeks)) return startDate;
  const a = parts(startDate);
  const b = parts(planEndDate(startDate, weeks));
  if (a.year !== b.year) return `${a.day} ${a.month} ${a.year} – ${b.day} ${b.month} ${b.year}`;
  const head = a.month === b.month ? `${a.day}` : `${a.day} ${a.month}`;
  return `${head} – ${b.day} ${b.month} ${b.year}`;
}
