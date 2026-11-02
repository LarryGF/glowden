<script lang="ts">
  import type { Device } from "./api";

  let { battery, detailed = false }: { battery: NonNullable<Device["battery"]>; detailed?: boolean } = $props();

  const low = $derived(battery.percent <= 15 && !battery.charging);
  const fill = $derived(Math.max(1, Math.round((battery.percent / 100) * 14)));
</script>

<span class="battery" class:low class:charging={battery.charging} title={battery.charging ? "Charging" : "On battery"}>
  <svg viewBox="0 0 24 12" width="24" height="12" aria-hidden="true">
    <rect x="0.75" y="0.75" width="19.5" height="10.5" rx="2.5" fill="none" stroke="currentColor" stroke-width="1.5" />
    <rect x="21.5" y="3.5" width="2" height="5" rx="1" fill="currentColor" />
    <rect x="3" y="3" width={fill} height="6" rx="1" fill="currentColor" />
    {#if battery.charging}
      <path d="M11.5 1.5 7.5 6.6h3l-1 3.9 4-5.1h-3z" fill="var(--bg-card)" stroke="var(--bg-card)" stroke-width="2.4" stroke-linejoin="round" />
      <path d="M11.5 1.5 7.5 6.6h3l-1 3.9 4-5.1h-3z" fill="currentColor" />
    {/if}
  </svg>
  <span>{battery.percent}%</span>
  {#if detailed}
    {#if battery.charging}<span>· charging</span>{:else if battery.hours}<span>· approx {Math.round(battery.hours)} h</span>{/if}
  {:else if battery.hours && !battery.charging}<span>· {Math.round(battery.hours)} h</span>{/if}
</span>

<style>
  .battery { display: inline-flex; align-items: center; gap: 6px; }
  .battery.charging { color: var(--accent); }
  .battery.low { color: var(--warn); }
  svg { flex: none; }
</style>
