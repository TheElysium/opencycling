// Pure FTP estimation from a ramp test, unit-tested like lib/metrics.ts.
// Operates on the measured 1 Hz power samples of a finished test session.

/** Best average over any contiguous `windowS`-second window of 1 Hz samples.
 *  Shorter-than-window series average the whole series. */
export function bestRollingAvg(power: number[], windowS: number): number {
  if (power.length === 0) return 0;
  const w = Math.min(windowS, power.length);
  let sum = 0;
  for (let i = 0; i < w; i++) sum += power[i];
  let best = sum;
  for (let i = w; i < power.length; i++) {
    sum += power[i] - power[i - w];
    if (sum > best) best = sum;
  }
  return best / w;
}

/** Ramp-test FTP estimate: 75% of the best 1-minute power, rounded. */
export function estimateFtpFromRamp(power: number[]): number {
  return Math.round(bestRollingAvg(power, 60) * 0.75);
}

/** A ramp test is authored in raw watts (Power="2.20" == 220 W). Rendering it at a
 *  reference FTP of 100 makes "% of FTP" equal the wattage, so charts/metrics show
 *  watts without a real FTP. */
export const FTP_TEST_REFERENCE_W = 100;

/** FTP to render/run a workout at: the reference for a test, else the rider's FTP. */
export function workoutFtp(w: { is_ftp_test: boolean } | null | undefined, ftp: number): number {
  return w?.is_ftp_test ? FTP_TEST_REFERENCE_W : ftp;
}

// --- Exhaustion detection for a ramp test ---------------------------------
// In ERG mode the trainer holds the target until the rider can no longer turn it
// over, so measured power collapsing below the target for a few seconds is the
// clearest failure signal. We never stop on our own: after a short countdown we
// surface a prompt and let the rider decide.

const POWER_DROP_RATIO = 0.5; // power below 50% of target = failing
const POWER_DROP_SECONDS = 5; // grace period (1 Hz ticks) before the prompt

/** Drop-detector state, advanced one 1 Hz tick at a time by `stepDropDetector`. */
export type DropState = {
  /** Consecutive ticks spent below the failing threshold. */
  lowTicks: number;
  /** Seconds left before the prompt (5,4,3,2,1), or null when on target / prompt up. */
  countdown: number | null;
  /** True once the grace period elapsed: the UI asks the rider whether to stop. */
  prompt: boolean;
};

export const INITIAL_DROP_STATE: DropState = { lowTicks: 0, countdown: null, prompt: false };

/** Advance the drop detector by one 1 Hz sample. Pure: same inputs → same output. */
export function stepDropDetector(
  prev: DropState,
  input: { running: boolean; targetW: number | null; powerW: number },
): DropState {
  // Once the prompt is up it waits on the rider; nothing changes until they answer.
  if (prev.prompt) return prev;
  // Only meaningful while Running against a real ERG target.
  if (!input.running || input.targetW == null || input.targetW <= 0) {
    return INITIAL_DROP_STATE;
  }
  const failing = input.powerW < input.targetW * POWER_DROP_RATIO;
  if (!failing) return INITIAL_DROP_STATE; // recovered, back on target
  const lowTicks = prev.lowTicks + 1;
  const remaining = POWER_DROP_SECONDS - lowTicks + 1;
  return remaining > 0
    ? { lowTicks, countdown: remaining, prompt: false }
    : { lowTicks, countdown: null, prompt: true };
}
