<script lang="ts">
  import { onMount } from "svelte";
  import { api, debounce, type Device } from "../api";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  let levels = $state<number[]>([800, 1200, 1600, 2400, 3200]);
  let active = $state(0);
  let defaultLevel = $state<number | null>(null);
  let shift = $state<number | null>(null);
  let min = $state(100);
  let max = $state(25600);
  let step = $state(50);
  let interval = $state<number | null>(null);
  let intervals = $state<number[]>([]);
  // Nothing is shown or saved until the mouse's settings are read: otherwise the
  // placeholder levels above could overwrite the saved ones.
  let loaded = $state(false);
  let failed = $state(false);

  onMount(load);

  async function load() {
    failed = false;
    try {
      const [info, cfg] = await Promise.all([api.mouseInfo(device.id), api.getConfig(device.id)]);
      // A range is encoded as low, 0xe000 | step, high.
      const list = info.dpi_list;
      const r = list.findIndex((w) => w >= 0xe000);
      if (r > 0) {
        min = list[r - 1];
        step = list[r] & 0x1fff;
        max = list[r + 1] ?? max;
      }
      interval = cfg.report_interval ?? info.report_interval;
      intervals = info.report_intervals;
      if (cfg.dpi_levels?.length) {
        levels = cfg.dpi_levels;
        active = cfg.dpi_level ?? 0;
        defaultLevel = cfg.dpi_default ?? null;
        shift = cfg.dpi_shift ?? null;
      } else {
        // No saved levels yet: make the mouse's current DPI the active one.
        const i = levels.indexOf(info.dpi);
        if (i >= 0) active = i;
        else levels[active] = info.dpi;
      }
      loaded = true;
    } catch (e) {
      failed = true;
      onerror(e);
    }
  }

  const pushDpi = debounce(() => loaded && api.setDpi(device.id, levels, active, defaultLevel, shift).catch(onerror));

  function toggleDefault(i: number) {
    defaultLevel = defaultLevel === i ? null : i;
    pushDpi();
  }

  function select(i: number) {
    active = i;
    pushDpi();
  }

  function add() {
    if (levels.length >= 5) return;
    levels = [...levels, Math.min(max, (levels.at(-1) ?? 800) + 400)];
    select(levels.length - 1);
  }

  function remove(i: number) {
    if (levels.length <= 1) return;
    levels = levels.filter((_, j) => j !== i);
    if (defaultLevel !== null) defaultLevel = defaultLevel === i ? null : defaultLevel > i ? defaultLevel - 1 : defaultLevel;
    select(Math.min(active, levels.length - 1));
  }

  function setInterval_(ms: number) {
    interval = ms;
    api.setReportInterval(device.id, ms).catch(onerror);
  }
</script>

{#if !loaded}
  <p class="note">
    {#if failed}The mouse didn't answer (it may be asleep: move it). <button onclick={load}>Try again</button>{:else}Reading the mouse…{/if}
  </p>
{:else}
<div class="panel">

  <div class="row">
    <span class="label">DPI levels</span>
    <div class="levels">
      {#each levels as dpi, i}
        <div class="chip" class:active={i === active}>
          <button class="value" onclick={() => select(i)}>{dpi}</button>
          <button class="star" class:on={defaultLevel === i} title={defaultLevel === i ? "Default level (click to unset)" : "Make this the default level"} aria-label="Default level" onclick={() => toggleDefault(i)}>◆</button>
          {#if levels.length > 1}<button class="x" aria-label="Remove level" onclick={() => remove(i)}>×</button>{/if}
        </div>
      {/each}
      {#if levels.length < 5}<button class="add" onclick={add}>+ level</button>{/if}
    </div>
  </div>

  <label class="row">
    <span class="label">Level {active + 1} <em>{levels[active]} DPI</em></span>
    <input type="range" {min} {max} {step} bind:value={levels[active]} oninput={pushDpi} />
  </label>

  <div class="row">
    <span class="label">DPI shift <em>{shift ?? levels[0]} DPI{shift === null ? " (level 1)" : ""}</em></span>
    <div class="rates">
      <button class:active={shift === null} onclick={() => ((shift = null), pushDpi())}>Level 1</button>
      <button class:active={shift !== null} onclick={() => ((shift = shift ?? 400), pushDpi())}>Custom</button>
    </div>
    {#if shift !== null}
      <input type="range" {min} {max} {step} bind:value={shift} oninput={pushDpi} />
    {/if}
  </div>

  {#if intervals.length}
    <div class="row">
      <span class="label">Report rate</span>
      <div class="rates">
        {#each intervals as ms}
          <button class:active={interval === ms} onclick={() => setInterval_(ms)}>{Math.round(1000 / ms)} Hz</button>
        {/each}
      </div>
    </div>
  {/if}

  <p class="note">
    The active level is applied to the mouse and kept when you change it with the DPI buttons.
    With a default level (◆), the mouse goes back to it when it reconnects or a profile
    switches, as in G HUB. DPI shift applies while a DPI shift button is held.
  </p>
</div>
{/if}

<style>
  .panel { display: flex; flex-direction: column; gap: 24px; max-width: 580px; }
  h2 { margin: 0; font-size: 20px; }
  .row { display: flex; flex-direction: column; gap: 10px; }
  .label em { font-style: normal; color: var(--text); margin-left: 6px; text-transform: none; letter-spacing: 0; }
  .levels, .rates { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .chip { display: flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .chip.active { border-color: var(--accent); }
  .chip button { border: none; border-radius: 0; background: var(--bg-card); }
  .chip.active .value { color: var(--accent); }
  .chip .x { color: var(--text-faint); padding: 7px 9px; }
  .chip .star { color: var(--text-faint); padding: 7px 4px 7px 2px; font-size: 10px; }
  .chip .star.on { color: var(--accent); }
  .rates button.active { border-color: var(--accent); color: var(--accent); }
  .add { color: var(--text-dim); }
  .note { color: var(--text-faint); font-size: 12px; margin: 0; }
</style>
