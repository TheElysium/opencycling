import type { WorkoutBlock } from './workout.svelte';

export function blockDuration(b: WorkoutBlock): number {
  // Truthiness checks, not `'X' in b`: the generated WorkoutBlock union marks the
  // other variants' keys as `?: never`, so `in` does not narrow the property itself.
  if (b.SteadyState) return b.SteadyState.duration_s;
  if (b.Ramp)        return b.Ramp.duration_s;
  if (b.IntervalsT) {
    const { repeat, on, off } = b.IntervalsT;
    return repeat * (blockDuration(on) + blockDuration(off));
  }
  return 0;
}

export function totalDuration(blocks: WorkoutBlock[]): number {
  return blocks.reduce((s, b) => s + blockDuration(b), 0);
}

export function formatDuration(s: number): string {
  if (s >= 3600) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return m > 0 ? `${h}h ${m}m` : `${h}h`;
  }
  const m   = Math.floor(s / 60);
  const sec = s % 60;
  if (m === 0) return `${sec}s`;
  return sec > 0 ? `${m}m ${sec}s` : `${m}m`;
}

export function displayWorkoutName(name: string | null, fallback = 'Untitled'): string {
  if (!name || !name.trim()) return fallback;
  const cleaned = name.replace(/\.zwo$/i, '').replace(/[_-]+/g, ' ').trim();
  return cleaned.replace(/\b\w/g, c => c.toUpperCase());
}

// Strips HTML tags and collapses whitespace. .zwo descriptions often contain
// inline markup like <strong>, <br>, etc. that we want to render as plain text.
export function stripHtml(s: string | null | undefined): string {
  if (!s) return '';
  return s
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/?[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

export function toMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error)    return e.message;
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message);
  try { return JSON.stringify(e); } catch { return String(e); }
}
