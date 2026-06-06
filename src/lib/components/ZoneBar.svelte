<script lang="ts">
  let { distribution, labels = ['Z1','Z2','Z3','Z4','Z5','Z6'] }:
    { distribution: number[]; labels?: string[] } = $props();

  let parts = $derived(distribution.map((pct, i) => ({
    label: labels[i] ?? `Z${i+1}`,
    color: `var(--z${i+1})`,
    pct,
  })).filter(p => p.pct > 0));
</script>

{#if parts.length > 0}
  <div class="zone-row">
    {#each parts as p}
      <div class="zone-seg" style="background:{p.color}; width:{(p.pct*100).toFixed(2)}%">
        {(p.pct*100) >= 6 ? `${Math.round(p.pct*100)}%` : ''}
      </div>
    {/each}
  </div>
  <div class="zone-badges">
    {#each parts as p}
      <div class="zone-badge">
        <span class="zone-circle" style="background:{p.color}">{p.label}</span>
        <span class="zone-badge-pct">{Math.round(p.pct*100)}%</span>
      </div>
    {/each}
  </div>
{:else}
  <p class="muted">No data.</p>
{/if}

<style>
  .zone-row {
    display: flex;
    height: 22px;
    border-radius: 5px;
    overflow: hidden;
    margin-bottom: 0.85rem;
  }
  .zone-seg {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-size: 0.7rem;
    font-weight: 600;
    min-width: 0;
  }
  .zone-badges {
    display: flex;
    gap: 0.85rem;
    flex-wrap: wrap;
    justify-content: center;
  }
  .zone-badge {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .zone-circle {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.65rem;
    font-weight: 800;
    color: #fff;
  }
  .zone-badge-pct {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--muted);
  }
  .muted { color: var(--muted); font-size: 0.85rem; }
</style>
