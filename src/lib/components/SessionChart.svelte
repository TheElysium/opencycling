<script lang="ts">
  import type { FlatBlock } from '$lib/workout.svelte';
  import type { MetricSample } from '$lib/db';
  import WorkoutChart from './WorkoutChart.svelte';

  let { metrics, blocks, ftpWatts, maxHr = 0 }:
    { metrics: MetricSample[]; blocks: FlatBlock[]; ftpWatts: number; maxHr?: number } = $props();
</script>

<div class="chart-section">
  <WorkoutChart
    {blocks}
    height={260}
    showFtpLine
    showTimeAxis
    showToggles
    {ftpWatts}
    {maxHr}
    actualMetrics={metrics}
  />
</div>

<style>
  .chart-section {
    background: var(--surface-dark);
    border-radius: 10px;
    padding: 1.25rem 1rem 1rem;
    margin-bottom: 1.25rem;
    --chart-gap: var(--surface-dark);
    --muted: #94a3b8;
    --text: #e2e8f0;
    --border: #334155;
  }
</style>
