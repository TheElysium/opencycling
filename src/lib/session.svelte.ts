import { invoke } from '@tauri-apps/api/core';
import type { ParsedWorkout, FlatBlock } from './workout.svelte';
import { stepDropDetector, INITIAL_DROP_STATE, type DropState } from './ftp';

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
  is_ftp_test: boolean;
};

class SessionStore {
  metrics             = $state<SessionMetrics | null>(null);
  flat_blocks         = $state<FlatBlock[]>([]);
  ftp_w               = $state<number>(0);
  workout_name        = $state<string | null>(null);
  workout_author      = $state<string | null>(null);
  workout_description = $state<string | null>(null);

  // Whether aero detection was requested for the pending/active session.
  aeroEnabled         = $state(false);

  // FTP ramp test state. `isFtpTest` is rehydrated from the snapshot in apply(), so
  // the countdown / stop prompt and the result screen survive a mid-test page reload.
  isFtpTest           = $state(false);
  // Exhaustion detector advanced by `stepDropDetector` (see lib/ftp.ts). `countdown`
  // drives the on-screen countdown; `prompt` drives the stop popup. We never stop
  // automatically: the rider always decides.
  private drop        = $state<DropState>(INITIAL_DROP_STATE);
  get dropCountdown(): number | null { return this.drop.countdown; }
  get stopPromptVisible(): boolean { return this.drop.prompt; }

  // Deferred start: the detail page records the chosen workout here and navigates,
  // but `start_session` is not sent until the session page consumes it (after aero
  // calibration when enabled). This keeps the session unarmed while the rider is
  // calibrating, so pedaling cannot trigger the auto-start early.
  private pendingWorkout: ParsedWorkout | null = null;
  private pendingFtp = 0;

  prepare(workout: ParsedWorkout, ftpW: number, aero: boolean): void {
    // Drop the previous session's display state so the session page doesn't show the
    // last ride behind the calibration overlay while this one waits to be armed.
    this.apply(null);
    // Clear any countdown / prompt left over from a prior test.
    this.drop = INITIAL_DROP_STATE;
    this.pendingWorkout = workout;
    this.pendingFtp = ftpW;
    this.aeroEnabled = aero;
  }

  get hasPendingStart(): boolean {
    return this.pendingWorkout != null;
  }

  // Consume the pending start. No-op if nothing is pending.
  async startPending(): Promise<void> {
    if (!this.pendingWorkout) return;
    const workout = this.pendingWorkout;
    const ftpW = this.pendingFtp;
    this.pendingWorkout = null;
    await this.start(workout, ftpW);
    // start_session returns (), so pull the snapshot to populate flat_blocks /
    // metrics; without this the session page stays on the empty branch (no blocks)
    // and the live UI never renders behind the "start pedaling" prompt.
    await this.loadSnapshot();
  }

  // Private: the only supported entry point is prepare() + startPending(). Calling
  // start_session directly would re-arm the session during aero calibration, which
  // is exactly what the deferred-start flow exists to prevent.
  private async start(workout: ParsedWorkout, ftpW: number): Promise<void> {
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
    this.isFtpTest           = snap?.is_ftp_test         ?? false;
  }

  // Single ingress for session_metrics events. Updates the live metrics and, during
  // an FTP test, advances the exhaustion detector (see stepDropDetector in lib/ftp.ts).
  ingestMetrics(m: SessionMetrics): void {
    this.metrics = m;
    if (!this.isFtpTest) return;
    this.drop = stepDropDetector(this.drop, {
      running: m.state === 'Running',
      targetW: m.target_w,
      powerW: m.power_w ?? 0,
    });
  }

  // Rider chose to end the test from the prompt: stop and converge on 'Finished',
  // which the result screen reacts to.
  async confirmStopFromPrompt(): Promise<void> {
    this.drop = INITIAL_DROP_STATE;
    await this.stop();
  }

  // Rider chose to keep going: dismiss the prompt and restart the grace period.
  dismissStopPrompt(): void {
    this.drop = INITIAL_DROP_STATE;
  }

  reset(): void {
    this.apply(null);
    this.pendingWorkout = null;
    this.pendingFtp = 0;
    this.aeroEnabled = false;
    this.isFtpTest = false;
    this.drop = INITIAL_DROP_STATE;
  }
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
