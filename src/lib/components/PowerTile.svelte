<script lang="ts">
  import { Zap } from '@lucide/svelte';
  import { targetKind, kindColor } from '$lib/session-visuals';

  let { power_w, target_w }: { power_w: number | null; target_w: number | null } = $props();

  let view = $derived.by(() => {
    const kind = targetKind(power_w, target_w);
    const color = kindColor(kind);
    let deltaText = '—';
    if (power_w != null && target_w != null && target_w > 0) {
      const d = power_w - target_w;
      if (kind === 'success') deltaText = 'on target · 0 W';
      else                    deltaText = d < 0 ? `${d} W below target` : `+${d} W above target`;
    } else if (power_w != null) {
      deltaText = '';
    }
    return { color, deltaText };
  });
</script>

<div class="card power-tile">
  <div>
    <div class="lbl"><Zap size={13} aria-hidden="true" /> Power</div>
    <div class="val" style="color: {view.color};">{power_w ?? '—'}</div>
    <div class="unit">watts</div>
    <div class="delta" style="color: {view.color};">{view.deltaText}</div>
  </div>
</div>

<style>
  .power-tile {
    display: grid;
    place-items: center;
    text-align: center;
  }
  .lbl {
    font-size: 0.85rem;
    letter-spacing: 0.15em;
    color: var(--muted);
    text-transform: uppercase;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .val {
    font-size: clamp(7rem, 18vw, 14rem);
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 1rem;
    letter-spacing: 0.2em;
    color: var(--muted);
    text-transform: uppercase;
  }
  .delta {
    margin-top: 0.4rem;
    font-size: 0.9rem;
    font-variant-numeric: tabular-nums;
  }
</style>
