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
        <button class="card" onclick={() => onopen(d)}>
          <div class="icon"><DeviceVisual device={d} size={210} /></div>
          <div class="name">{d.name}</div>
          <div class="meta">
            <span class="badge">{d.connection}</span>
            {#if d.battery}
              <span class="badge" class:warn={d.battery.percent <= 15}>
                {d.battery.percent}%{d.battery.charging ? " ⚡" : ""}
              </span>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  {/if}
</section>

<style>
  .home { height: 100%; display: grid; place-items: center; padding: 40px; background: radial-gradient(ellipse at 50% 60%, #1c2230 0%, var(--bg) 70%); }
  .empty { color: var(--text-dim); text-align: center; }
  .grid { display: flex; flex-wrap: wrap; justify-content: center; align-items: flex-end; gap: 28px; max-width: 1400px; }
  .card {
    width: 260px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 24px 16px 22px;
    background: none;
    border: 1px solid transparent;
    border-radius: 16px;
    transition: background 0.15s, border-color 0.15s, transform 0.15s;
  }
  .card:hover { background: rgba(255, 255, 255, 0.03); border-color: var(--line); transform: translateY(-3px); }
  .icon { height: 220px; display: grid; place-items: center; color: var(--text-dim); }
  .card:hover .icon { color: var(--accent); }
  .name { font-size: 15px; font-weight: 600; text-align: center; letter-spacing: 0.02em; }
  .meta { display: flex; gap: 6px; }
</style>
