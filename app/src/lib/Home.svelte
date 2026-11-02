<script lang="ts">
  import type { Device } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";
  import LightingViewer from "./LightingViewer.svelte";
  import BatteryBadge from "./BatteryBadge.svelte";

  let { devices, loading, onopen }: { devices: Device[]; loading: boolean; onopen: (d: Device) => void } = $props();

  // G HUB's home layout choice; a per-viewer preference.
  const read = () => {
    try {
      return localStorage.getItem("glowden-home-layout") === "list" ? "list" : "grid";
    } catch {
      return "grid";
    }
  };
  let layout = $state<"grid" | "list">(read());
  function setLayout(l: "grid" | "list") {
    layout = l;
    try {
      localStorage.setItem("glowden-home-layout", l);
    } catch {
      /* not kept */
    }
  }
</script>

<section class="home">
  {#if loading && devices.length === 0}
    <p class="empty">Looking for devices…</p>
  {:else if devices.length === 0}
    <p class="empty">No devices found. Check that your user can open <code>/dev/hidraw*</code>.</p>
  {:else}
    <div class="layout" role="group" aria-label="Layout">
      <button class:active={layout === "grid"} onclick={() => setLayout("grid")} title="Grid" aria-label="Grid">▦</button>
      <button class:active={layout === "list"} onclick={() => setLayout("list")} title="List" aria-label="List">☰</button>
    </div>
    <div class="grid" class:list={layout === "list"}>
      {#each devices as d (d.id)}
        <div class="card" class:wide={d.kind === "keyboard" && layout === "grid"}>
          <button class="open" onclick={() => onopen(d)} aria-label="Open {d.name}">
            <h2>{d.name}</h2>
            <div class="meta">
              {#if d.battery}<BatteryBadge battery={d.battery} />{/if}
              <span>{d.connection === "LIGHTSPEED" ? "Wireless" : "USB"}</span>
            </div>
            <div class="picture">
              {#if d.caps.includes("lighting")}
                <LightingViewer device={d} compact size={layout === "list" ? (d.kind === "keyboard" ? 260 : 110) : d.kind === "keyboard" ? 520 : 230} />
              {:else}
                <DeviceVisual device={d} size={layout === "list" ? (d.kind === "keyboard" ? 260 : 110) : d.kind === "keyboard" ? 520 : 230} />
              {/if}
            </div>
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
  .picture { flex: 1; min-height: 240px; display: grid; place-items: center; color: var(--text-dim); padding-top: 10px; }
  .layout { display: flex; justify-content: flex-end; gap: 4px; max-width: 1500px; margin: 0 auto 14px; }
  .layout button { padding: 4px 9px; font-size: 14px; color: var(--text-dim); }
  .layout button.active { color: var(--accent); border-color: var(--accent); }
  .grid.list { grid-template-columns: 1fr; gap: 10px; max-width: 900px; }
  .grid.list .open { flex-direction: row; align-items: center; padding: 14px 20px; gap: 20px; }
  .grid.list h2 { font-size: 17px; flex: 1; }
  .grid.list .picture { flex: none; min-height: 0; padding: 0; order: -1; width: 270px; }
  @media (max-width: 760px) { .card.wide { grid-column: span 1; } }
</style>
