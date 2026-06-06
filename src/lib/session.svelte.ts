import { invoke } from '@tauri-apps/api/core';
import type { ParsedWorkout, FlatBlock } from './workout.svelte';

export type { FlatBlock };

export type SessionStateKind = 'WaitingForRider' | 'Running' | 'Paused' | 'Finished';

export type SessionMetrics = {
  state: SessionStateKind;
  total_elapsed_s: number;
  total_active_s: number;
  current_block_idx: number;
  current_block_elapsed_s: number;
  target_w: number | null;
  cadence_target_rpm: number | null;
  power_w: number | null;
  hr_bpm: number | null;
  cadence_rpm: number | null;
  ftp_w: number;
  blocks_total: number;
  session_id: number | null;
};

export type SessionSnapshot = {
  flat_blocks: FlatBlock[];
  ftp_w: number;
  workout_name: string | null;
  workout_author: string | null;
  workout_description: string | null;
  metrics: SessionMetrics | null;
};

class SessionStore {
  metrics             = $state<SessionMetrics | null>(null);
  flat_blocks         = $state<FlatBlock[]>([]);
  ftp_w               = $state<number>(0);
  workout_name        = $state<string | null>(null);
  workout_author      = $state<string | null>(null);
  workout_description = $state<string | null>(null);

  async start(workout: ParsedWorkout, ftpW: number): Promise<void> {
    await invoke('start_session', { workout, ftpW });
  }

  pause():  Promise<void> { return invoke('pause_session'); }
  resume(): Promise<void> { return invoke('resume_session'); }
  stop():   Promise<void> { return invoke('stop_session'); }
  skip():   Promise<void> { return invoke('skip_block'); }

  async loadSnapshot(): Promise<void> {
    const snap = await invoke<SessionSnapshot | null>('get_session_snapshot');
    this.apply(snap);
  }

  apply(snap: SessionSnapshot | null): void {
    this.metrics             = snap?.metrics             ?? null;
    this.flat_blocks         = snap?.flat_blocks         ?? [];
    this.ftp_w               = snap?.ftp_w               ?? 0;
    this.workout_name        = snap?.workout_name        ?? null;
    this.workout_author      = snap?.workout_author      ?? null;
    this.workout_description = snap?.workout_description ?? null;
  }

  reset(): void { this.apply(null); }
}

export const session = new SessionStore();

export function formatClock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const m = Math.floor(s / 60);
  const sec = s % 60;
  if (m >= 60) {
    const h = Math.floor(m / 60);
    const mm = m % 60;
    return `${h}:${String(mm).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
  }
  return `${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
}

export function totalFlatDuration(blocks: FlatBlock[]): number {
  let t = 0;
  for (const b of blocks) t += b.duration_s;
  return t;
}

export function isRamp(b: FlatBlock): boolean {
  return b.power_start_w !== b.power_end_w;
}

export function flatBlockPct(b: FlatBlock, ftpW: number, kind: 'start' | 'end' | 'avg'): number {
  if (ftpW <= 0) return 0;
  if (kind === 'start') return b.power_start_w / ftpW;
  if (kind === 'end')   return b.power_end_w / ftpW;
  return ((b.power_start_w + b.power_end_w) / 2) / ftpW;
}

export const flatBlockStartPct = (b: FlatBlock, ftpW: number) => flatBlockPct(b, ftpW, 'start');
export const flatBlockEndPct   = (b: FlatBlock, ftpW: number) => flatBlockPct(b, ftpW, 'end');
export const flatBlockAvgPct   = (b: FlatBlock, ftpW: number) => flatBlockPct(b, ftpW, 'avg');
