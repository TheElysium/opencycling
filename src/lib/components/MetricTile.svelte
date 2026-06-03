<script lang="ts">
  import type { Component } from 'svelte';
  import { targetKind, kindColor } from '$lib/session-visuals';

  let { label, value, unit, target = null, icon: Icon = undefined }:
    { label: string; value: number | null; unit: string; target?: number | null; icon?: Component } = $props();

  let color = $derived(kindColor(targetKind(value, target)));
</script>

<div class="card metric">
  <div class="lbl">
    {#if Icon}<Icon size={12} aria-hidden="true" />{/if}
    {label}
  </div>
  <div class="val" style="color: {color};">{value ?? '—'}</div>
  <div class="unit">{unit}</div>
</div>

<style>
  .metric { text-align: center; }
  .lbl {
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    color: var(--muted);
    text-transform: uppercase;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .val {
    font-size: 3rem;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 0.75rem;
    color: var(--muted);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
</style>
