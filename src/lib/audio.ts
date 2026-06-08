let ctx: AudioContext | null = null;

function getCtx(): AudioContext {
  if (!ctx) ctx = new AudioContext();
  return ctx;
}

function tone(freq: number, durationMs: number, when = 0): void {
  const ac = getCtx();
  const t0 = ac.currentTime + when;
  const dur = durationMs / 1000;
  const osc = ac.createOscillator();
  const gain = ac.createGain();
  osc.type = 'sine';
  osc.frequency.value = freq;
  gain.gain.setValueAtTime(0, t0);
  gain.gain.linearRampToValueAtTime(0.25, t0 + 0.01);
  gain.gain.setValueAtTime(0.25, t0 + dur - 0.02);
  gain.gain.linearRampToValueAtTime(0, t0 + dur);
  osc.connect(gain).connect(ac.destination);
  osc.start(t0);
  osc.stop(t0 + dur + 0.05);
}

export function beepShort(): void {
  tone(880, 80);
}

export function beepLong(): void {
  tone(660, 350);
}

export function beepLow(): void {
  tone(330, 500);
}
