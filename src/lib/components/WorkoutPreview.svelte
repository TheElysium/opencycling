<script lang="ts">
  import type { FlatBlock } from '$lib/workout.svelte';
  import WorkoutChart from './WorkoutChart.svelte';

  let { blocks, height = 200, ftpWatts }:
    { blocks: FlatBlock[]; height?: number; ftpWatts: number } = $props();
</script>

<WorkoutChart {blocks} {height} {ftpWatts} showFtpLine showZones />
