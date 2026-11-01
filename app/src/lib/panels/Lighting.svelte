<script lang="ts">
  import { onMount } from "svelte";
  import { api, debounce, type Device, type Lighting } from "../api";
  import LightingViewer from "../LightingViewer.svelte";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  type Effect = Lighting["effect"];

  const effects = $derived(
    [
      { key: "fixed", label: "Fixed", host: false },
      { key: "breathing", label: "Breathing", host: false },
      { key: "cycle", label: "Color cycle", host: false },
      ...(device.caps.includes("per_key")
        ? [
            { key: "wave", label: "Wave", host: true },
            { key: "starlight", label: "Starlight", host: true },
            { key: "visualizer", label: "Audio visualizer", host: true },
          ]
        : []),
      ...(device.caps.includes("firmware_ripple") ? [{ key: "ripple", label: "Ripple (onboard)", host: false }] : []),
      ...(device.caps.includes("reactive") ? [{ key: "pulse", label: "Pulse (live)", host: true }] : []),
    ] as { key: Effect; label: string; host: boolean }[],
  );

  // One set of controls shared by all effects, so switching effects keeps your choices.
  let effect = $state<Effect>("fixed");
  let color = $state("#2dd4bf");
  let base = $state("#02060c");
  let speed = $state(60);
  let intensity = $state(100);
  let saturation = $state(255);
  let width = $state(35);
  let fade = $state(40);
  let density = $state(30);
  let rainbow = $state(false);
  let peak = $state("#ff3ea5");
  let brightness = $state(100);
  let loaded = $state(false);

  const swatches = ["#ff2a2a", "#ff8a00", "#ffd400", "#2dd4bf", "#2f7bff", "#8b5cf6", "#ff3ea5", "#ffffff"];

  onMount(async () => {
    try {
      const cfg = await api.getConfig(device.id);
      const l = cfg.lighting;
      if (l) {
        effect = l.effect;
        if ("color" in l) color = l.color;
        if ("speed" in l) speed = l.speed;
        if ("intensity" in l) intensity = l.intensity;
        if ("saturation" in l) saturation = l.saturation;
        if (l.effect === "pulse") ({ base, width, fade, rainbow } = l);
        if (l.effect === "starlight") ({ star: color, sky: base, density } = l);
        if (l.effect === "visualizer") ({ low: color, high: peak, background: base } = l);
      }
      if (cfg.brightness !== undefined) brightness = cfg.brightness;
    } catch (e) {
      onerror(e);
    }
    loaded = true;
  });

  function lighting(): Lighting {
    switch (effect) {
      case "fixed": return { effect, color };
      case "breathing": return { effect, color, speed, intensity };
      case "cycle": return { effect, speed, intensity, saturation };
      case "ripple": return { effect, color };
      case "wave": return { effect, speed };
      case "pulse": return { effect, color, base, speed, width, fade, rainbow };
      case "starlight": return { effect, star: color, sky: base, density };
      case "visualizer": return { effect, low: color, high: peak, background: base };
    }
  }

  const push = debounce(() => loaded && api.setLighting(device.id, lighting()).catch(onerror));
  const pushBrightness = debounce(() => api.setBrightness(device.id, brightness).catch(onerror));

  const hostRendered = $derived(effects.find((e) => e.key === effect)?.host ?? false);
  const hasColor = $derived(effect !== "cycle" && effect !== "wave");
  const hasSpeed = $derived(!["fixed", "starlight", "ripple", "visualizer"].includes(effect));
  const untested = $derived(device.kind === "mouse" && hostRendered);
</script>

<div class="panel">
  <header>
    <h2>Lighting</h2>
    {#if hostRendered}<span class="badge ok" title="Rendered by lhubd and streamed to the device">live effect</span>{/if}
    {#if untested}<span class="badge warn">zone layout not mapped yet</span>{/if}
  </header>

  <div class="effects">
    {#each effects as e}
      <button class:active={effect === e.key} onclick={() => { effect = e.key; push(); }}>{e.label}</button>
    {/each}
  </div>

  <LightingViewer {device} />

  {#if hasColor}
    <div class="row">
      <span class="label">{effect === "starlight" ? "Star color" : effect === "pulse" ? "Pulse color" : effect === "visualizer" ? "Low color" : "Color"}</span>
      <div class="swatches">
        {#each swatches as s}
          <button class="swatch" class:active={color === s} style:background={s} aria-label={s} onclick={() => { color = s; push(); }}></button>
        {/each}
        <input type="color" bind:value={color} oninput={push} />
      </div>
    </div>
  {/if}

  {#if effect === "visualizer"}
    <label class="row inline">
      <span class="label">Peak color</span>
      <input type="color" bind:value={peak} oninput={push} />
    </label>
  {/if}

  {#if effect === "pulse" || effect === "starlight" || effect === "visualizer"}
    <label class="row inline">
      <span class="label">{effect === "pulse" ? "Resting color" : effect === "visualizer" ? "Background" : "Sky color"}</span>
      <input type="color" bind:value={base} oninput={push} />
    </label>
  {/if}

  {#if hasSpeed}
    <label class="row">
      <span class="label">Speed <em>{speed}</em></span>
      <input type="range" min="1" max="100" bind:value={speed} oninput={push} />
    </label>
  {/if}

  {#if effect === "breathing" || effect === "cycle"}
    <label class="row">
      <span class="label">Intensity <em>{intensity}%</em></span>
      <input type="range" min="1" max="100" bind:value={intensity} oninput={push} />
    </label>
  {/if}

  {#if effect === "cycle"}
    <label class="row">
      <span class="label">Saturation <em>{Math.round((saturation / 255) * 100)}%</em></span>
      <input type="range" min="0" max="255" bind:value={saturation} oninput={push} />
    </label>
  {/if}

  {#if effect === "pulse"}
    <label class="row">
      <span class="label">Ring width <em>{width}</em></span>
      <input type="range" min="1" max="100" bind:value={width} oninput={push} />
    </label>
    <label class="row">
      <span class="label">Fade time <em>{fade}</em></span>
      <input type="range" min="1" max="100" bind:value={fade} oninput={push} />
    </label>
    <label class="row inline">
      <input type="checkbox" bind:checked={rainbow} onchange={push} />
      <span>Rainbow rings (each press shifts the hue)</span>
    </label>
  {/if}

  {#if effect === "starlight"}
    <label class="row">
      <span class="label">Density <em>{density}</em></span>
      <input type="range" min="1" max="100" bind:value={density} oninput={push} />
    </label>
  {/if}

  {#if device.caps.includes("brightness")}
    <label class="row">
      <span class="label">Brightness <em>{brightness}%</em></span>
      <input type="range" min="0" max="100" bind:value={brightness} oninput={pushBrightness} />
    </label>
  {/if}

  <p class="note">
    {hostRendered
      ? "Live effects are drawn by the lhub service and keep running after you close this window."
      : "This effect runs on the device itself and is saved to it."}
  </p>
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 22px; max-width: 580px; }
  header { display: flex; align-items: center; gap: 10px; }
  h2 { margin: 0; font-size: 20px; }
  .effects { display: flex; gap: 8px; flex-wrap: wrap; }
  .effects button.active { border-color: var(--accent); color: var(--accent); background: var(--bg-hover); }
  .preview { height: 72px; border-radius: var(--radius); border: 1px solid var(--line); background: var(--bg-raised); position: relative; overflow: hidden; }
  .preview.fixed { background: radial-gradient(ellipse at center, var(--c), transparent 75%), var(--bg-raised); }
  .preview.breathing { background: radial-gradient(ellipse at center, var(--c), transparent 75%), var(--bg-raised); animation: breathe var(--p) ease-in-out infinite; }
  .preview.cycle, .preview.wave {
    background: linear-gradient(90deg, #ff2a2a, #ffd400, #2dd4bf, #2f7bff, #8b5cf6, #ff2a2a);
    background-size: 200% 100%;
    animation: slide var(--p) linear infinite;
  }
  .preview.cycle { background-size: 1200% 100%; }
  .preview.pulse, .preview.ripple { background: var(--b); }
  .preview.visualizer { background: linear-gradient(0deg, var(--c), transparent 70%), var(--b); }
  .preview.pulse::after, .preview.ripple::after {
    content: ""; position: absolute; left: 50%; top: 50%; width: 10px; height: 10px; margin: -5px;
    border-radius: 50%; border: 3px solid var(--c); animation: ring 1.4s ease-out infinite;
  }
  .preview.starlight { background: radial-gradient(circle at 20% 40%, var(--c) 0 2px, transparent 3px), radial-gradient(circle at 70% 60%, var(--c) 0 2px, transparent 3px), radial-gradient(circle at 45% 25%, var(--c) 0 1.5px, transparent 2.5px), var(--b); }
  @keyframes breathe { 50% { opacity: 0.15; } }
  @keyframes slide { to { background-position: 200% 0; } }
  @keyframes ring { to { transform: scale(30); opacity: 0; } }
  .row { display: flex; flex-direction: column; gap: 8px; }
  .row.inline { flex-direction: row; align-items: center; gap: 12px; }
  .label em { font-style: normal; color: var(--text); margin-left: 6px; text-transform: none; letter-spacing: 0; }
  .swatches { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .swatch { width: 30px; height: 30px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.active { border-color: var(--text); }
  input[type="color"] { width: 38px; height: 32px; border: 1px solid var(--line); border-radius: 8px; background: none; padding: 2px; cursor: pointer; }
  input[type="checkbox"] { accent-color: var(--accent); width: 16px; height: 16px; }
  .note { color: var(--text-faint); font-size: 12px; margin: 0; }
</style>
