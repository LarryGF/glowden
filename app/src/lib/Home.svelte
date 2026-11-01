<script lang="ts">
  import type { Device } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";

  let { devices, loading, onopen }: { devices: Device[]; loading: boolean; onopen: (d: Device) => void } = $props();
</script>

<section class="home">
  {#if loading && devices.length === 0}
    <p class="empty">Looking for devices…</p>
  {:else if devices.length === 0}
    <p class="empty">No devices found. Check that your user can open <code>/dev/hidraw*</code>.</p>
  {:else}
    <div class="grid">
      {#each devices as d (d.id)}
        <div class="card" class:wide={d.kind === "keyboard"}>
          <button class="open" onclick={() => onopen(d)} aria-label="Open {d.name}">
            <h2>{d.name}</h2>
            <div class="meta">
              {#if d.battery}<span class:low={d.battery.percent <= 15}>{d.battery.percent}%{d.battery.charging ? " ⚡" : ""}{#if d.battery.hours && !d.battery.charging} · {Math.round(d.battery.hours)} h{/if}</span>{/if}
              <span>{d.connection === "LIGHTSPEED" ? "Wireless" : "USB"}</span>
            </div>
            <div class="picture"><DeviceVisual device={d} size={d.kind === "keyboard" ? 520 : 230} /></div>
          </button>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .home { height: 100%; overflow: auto; padding: 32px 40px; }
  .empty { color: var(--text-dim); text-align: center; margin-top: 20vh; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 24px; max-width: 1500px; margin: 0 auto; }
  .card { background: var(--bg-card); border-radius: 14px; border: 1px solid transparent; transition: border-color 0.15s; }
  .card.wide { grid-column: span 2; }
  .card:hover { border-color: var(--line); }
  .open { width: 100%; height: 100%; display: flex; flex-direction: column; align-items: stretch; text-align: left; gap: 8px; padding: 24px 26px; background: none; border: none; }
  .open:hover { background: none; }
  h2 { margin: 0; font-size: 22px; font-weight: 700; }
  .meta { display: flex; gap: 12px; color: var(--text-dim); font-size: 13px; font-weight: 600; }
  .meta .low { color: var(--warn); }
  .picture { flex: 1; min-height: 240px; display: grid; place-items: center; color: var(--text-dim); padding-top: 10px; }
  @media (max-width: 760px) { .card.wide { grid-column: span 1; } }
</style>
