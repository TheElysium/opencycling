<script lang="ts">
  import { Zap, Heart, RotateCw, Flame, Target, Battery, Wind } from '@lucide/svelte';
  import MetricsStrip, { type MetricTile } from '$lib/components/MetricsStrip.svelte';

  // 7-cell strip (mirrors a session WITH aero)
  const seven: MetricTile[] = [
    { icon: Zap,      label: 'Avg Power',      value: 167, unit: 'W',   secondary: { label: 'Max', value: 275, unit: 'W' } },
    { icon: Heart,    label: 'Avg Heart rate', value: 134, unit: 'bpm', secondary: { label: 'Max', value: 164 } },
    { icon: RotateCw, label: 'Avg Cadence',    value: 81,  unit: 'rpm', secondary: { label: 'Max', value: 91 } },
    { icon: Flame,    label: 'Stress',         value: 39,               secondary: { label: 'TSS' } },
    { icon: Target,   label: 'Intensity',      value: '0.72',           secondary: { label: 'IF' } },
    { icon: Battery,  label: 'Work',           value: 451, unit: 'kJ',  secondary: { label: 'kJ' } },
    { icon: Wind,     label: 'Aero',           value: 92,  unit: '%',   secondary: { label: 'in position' } },
  ];

  // 6-cell strip (no aero)
  const six = seven.slice(0, 6);
</script>

<div style="padding:2rem; max-width:1100px;">
  <p>7 cells:</p>
  <MetricsStrip tiles={seven} />
  <p>6 cells:</p>
  <MetricsStrip tiles={six} />
</div>
