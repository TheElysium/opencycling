import {
  isRamp, flatBlockAvgPct, flatBlockStartPct, flatBlockEndPct,
  type FlatBlock,
} from './session.svelte';
import { zoneOf } from './metrics';

export type TargetKind = 'normal' | 'muted' | 'success' | 'warning' | 'danger';

export function targetKind(value: number | null, target: number | null, tolerance = 0.05): TargetKind {
  if (value == null) return 'muted';
  if (target == null || target <= 0) return 'normal';
  const delta = value - target;
  if (Math.abs(delta) <= target * tolerance) return 'success';
  return delta < 0 ? 'warning' : 'danger';
}

const KIND_COLORS: Record<TargetKind, string> = {
  normal:  'var(--text)',
  muted:   'var(--muted)',
  success: 'var(--success)',
  warning: 'var(--warning)',
  danger:  'var(--danger)',
};

export function kindColor(k: TargetKind): string {
  return KIND_COLORS[k];
}

export function stateClass(i: number, currentIdx: number): '' | 'done' | 'active' {
  if (i < currentIdx) return 'done';
  if (i === currentIdx) return 'active';
  return '';
}

export function zoneBg(b: FlatBlock, ftpW: number, dir: 'to right' | 'to bottom'): string {
  if (isRamp(b)) {
    const zS = zoneOf(flatBlockStartPct(b, ftpW));
    const zE = zoneOf(flatBlockEndPct(b, ftpW));
    return `linear-gradient(${dir}, var(--z${zS}), var(--z${zE}))`;
  }
  return `var(--z${zoneOf(flatBlockAvgPct(b, ftpW))})`;
}

export function tintBg(zone: number, alpha = 10): string {
  return `color-mix(in srgb, var(--z${zone}) ${alpha}%, var(--surface))`;
}

export function pctRound(p: number): number {
  return Math.round(p * 100);
}
