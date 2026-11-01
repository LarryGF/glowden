<script lang="ts">
  import type { Device } from "./api";
  import DeviceIcon from "./DeviceIcon.svelte";

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
          <div class="icon"><DeviceIcon kind={d.kind} size={112} /></div>
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
  .home { padding: 40px 48px; }
  .empty { color: var(--text-dim); text-align: center; margin-top: 18vh; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 20px; }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 34px 20px 24px;
    background: linear-gradient(180deg, var(--bg-card), var(--bg-raised));
    border: 1px solid var(--line);
    border-radius: 14px;
    transition: border-color 0.15s, transform 0.15s;
  }
  .card:hover { border-color: var(--accent); transform: translateY(-2px); background: linear-gradient(180deg, var(--bg-hover), var(--bg-raised)); }
  .icon { color: var(--text-dim); }
  .card:hover .icon { color: var(--accent); }
  .name { font-size: 16px; font-weight: 600; text-align: center; }
  .meta { display: flex; gap: 6px; }
</style>
