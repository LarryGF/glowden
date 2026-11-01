<script lang="ts">
  import { api, debounce, type Device, type Effect } from "../api";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  type Mode = Effect["kind"];
  let mode = $state<Mode>("solid");
  let color = $state("#2dd4bf");
  let period = $state(5000);
  let intensity = $state(100);
  let saturation = $state(255);
  let brightness = $state(100);

  const swatches = ["#ff2a2a", "#ff8a00", "#ffd400", "#2dd4bf", "#2f7bff", "#8b5cf6", "#ff3ea5", "#ffffff"];

  function effect(): Effect {
    if (mode === "solid") return { kind: "solid", color };
    if (mode === "breathe") return { kind: "breathe", color, period_ms: period, intensity };
    return { kind: "cycle", period_ms: period, intensity, saturation };
  }

  const pushEffect = debounce(() => api.setEffect(device.id, effect()).catch(onerror));
  const pushBrightness = debounce(() => api.setBrightness(device.id, brightness).catch(onerror));

  const untested = $derived(device.kind !== "microphone");
</script>

<div class="panel">
  <header>
    <h2>Lighting</h2>
    {#if untested}<span class="badge warn">untested on this device</span>{/if}
  </header>

  <div class="modes">
    {#each [["solid", "Fixed"], ["breathe", "Breathing"], ["cycle", "Color cycle"]] as [m, label]}
      <button class:active={mode === m} onclick={() => { mode = m as Mode; pushEffect(); }}>{label}</button>
    {/each}
  </div>

  <div class="preview" style:--c={color} class:cycle={mode === "cycle"} class:breathe={mode === "breathe"} style:--p="{period}ms"></div>

  {#if mode !== "cycle"}
    <div class="row">
      <span class="label">Color</span>
      <div class="swatches">
        {#each swatches as s}
          <button class="swatch" class:active={color === s} style:background={s} aria-label={s} onclick={() => { color = s; pushEffect(); }}></button>
        {/each}
        <input type="color" bind:value={color} oninput={pushEffect} />
      </div>
    </div>
  {/if}

  {#if mode !== "solid"}
    <label class="row">
      <span class="label">Speed <em>{(period / 1000).toFixed(1)} s</em></span>
      <input type="range" min="1000" max="20000" step="100" bind:value={period} oninput={pushEffect} />
    </label>
    <label class="row">
      <span class="label">Intensity <em>{intensity}%</em></span>
      <input type="range" min="1" max="100" bind:value={intensity} oninput={pushEffect} />
    </label>
  {/if}

  {#if mode === "cycle"}
    <label class="row">
      <span class="label">Saturation <em>{Math.round((saturation / 255) * 100)}%</em></span>
      <input type="range" min="0" max="255" bind:value={saturation} oninput={pushEffect} />
    </label>
  {/if}

  {#if device.caps.includes("brightness")}
    <label class="row">
      <span class="label">Brightness <em>{brightness}%</em></span>
      <input type="range" min="0" max="100" bind:value={brightness} oninput={pushBrightness} />
    </label>
  {/if}
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 22px; max-width: 560px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .modes { display: flex; gap: 8px; }
  .modes button.active { border-color: var(--accent); color: var(--accent); background: var(--bg-hover); }
  .preview {
    height: 72px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: radial-gradient(ellipse at center, var(--c), transparent 75%), var(--bg-raised);
  }
  .preview.breathe { animation: breathe var(--p) ease-in-out infinite; }
  .preview.cycle {
    background: linear-gradient(90deg, #ff2a2a, #ffd400, #2dd4bf, #2f7bff, #8b5cf6, #ff2a2a);
    background-size: 200% 100%;
    animation: cycle var(--p) linear infinite;
  }
  @keyframes breathe { 50% { opacity: 0.15; } }
  @keyframes cycle { to { background-position: 200% 0; } }
  .row { display: flex; flex-direction: column; gap: 8px; }
  .label em { font-style: normal; color: var(--text); margin-left: 6px; text-transform: none; letter-spacing: 0; }
  .swatches { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .swatch { width: 30px; height: 30px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.active { border-color: var(--text); }
  input[type="color"] { width: 38px; height: 32px; border: 1px solid var(--line); border-radius: 8px; background: none; padding: 2px; cursor: pointer; }
</style>
