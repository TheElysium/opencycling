<script lang="ts">
  import { CircleCheck } from '@lucide/svelte';
  import { formatClock } from '$lib/session.svelte';

  let { total_active_s }: { total_active_s: number } = $props();
</script>

<div class="card finished">
  <div class="finished-label">
    <CircleCheck size={14} strokeWidth={2.5} />
    Workout complete
  </div>
  <div class="finished-time">{formatClock(total_active_s)}</div>
  <div class="finished-sub">total active time</div>
</div>

<style>
  .finished {
    display: grid;
    place-items: center;
    text-align: center;
    border-left: 6px solid var(--success);
    background: #f0fdf4;
    padding: 1.25rem;
  }
  .finished-label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--success);
    font-weight: 700;
  }
  .finished-time {
    font-size: 2.2rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    margin-top: 0.25rem;
  }
  .finished-sub {
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
