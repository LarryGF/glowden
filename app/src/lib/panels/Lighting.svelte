<script lang="ts">
  import { onMount } from "svelte";
  import { onDestroy } from "svelte";
  import { api, debounce, type Device, type Lighting } from "../api";
  import ColorPicker from "../ColorPicker.svelte";
  import { freestyle } from "../freestyle.svelte";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  /** "fixed", "pulse", … or "fw:<id>" for firmware effects from the device's driver file. */
  type Key = string;
  type Option = { key: Key; label: string; group: "device" | "live" };

  const options = $derived<Option[]>([
    { key: "fixed", label: "Fixed", group: "device" },
    { key: "breathing", label: "Breathing", group: "device" },
    { key: "cycle", label: "Color cycle", group: "device" },
    ...device.firmware_effects.map((f) => ({ key: `fw:${f.id}`, label: f.label, group: "device" as const })),
    ...(device.caps.includes("per_key")
      ? [
          { key: "wave", label: "Wave", group: "live" as const },
          { key: "starlight", label: "Starlight", group: "live" as const },
          { key: "visualizer", label: "Audio visualizer", group: "live" as const },
          { key: "screen", label: "Screen sampler", group: "live" as const },
          { key: "sequence", label: "Custom sequence", group: "live" as const },
        ]
      : []),
    ...(device.caps.includes("reactive") ? [{ key: "pulse", label: "Pulse", group: "live" as const }] : []),
  ]);

  let effect = $state<Key>("fixed");
  let color = $state("#2dd4bf");
  let base = $state("#02060c");
  let peak = $state("#ff3ea5");
  let speed = $state(60);
  let intensity = $state(100);
  let saturation = $state(255);
  let width = $state(35);
  let fade = $state(40);
  let density = $state(30);
  let boost = $state(0);
  let steps = $state<{ color: string; ms: number }[]>([
    { color: "#ff2d55", ms: 1000 },
    { color: "#2dd4bf", ms: 1000 },
    { color: "#7c3aed", ms: 1000 },
  ]);
  let blend = $state(true);
  let spread = $state(0);
  let rainbow = $state(false);
  let glow = $state(100);
  let backdrop = $state(100);
  let brightness = $state(100);
  let loaded = $state(false);
  let tab = $state<"presets" | "freestyle">("presets");
  let power = $state(true);
  let syncMessage = $state("");

  function setPower() {
    api.setPower(device.id, power).catch(onerror);
  }

  async function sync() {
    try {
      const names = await api.syncLighting(device.id);
      syncMessage = names.length ? `Applied to ${names.join(", ")}` : "No other device can show this effect";
    } catch (e) {
      onerror(e);
    }
  }
  const canPaint = $derived(device.caps.includes("per_key"));

  const swatches = ["#ff2a2a", "#ff8a00", "#ffd400", "#2dd4bf", "#2f7bff", "#8b5cf6", "#ff3ea5", "#ffffff"];

  /** Puts saved lighting into the controls. */
  function show(l: Lighting) {
    {
      {
        if (l.effect === "freestyle") {
          freestyle.base = l.base;
          freestyle.keys = { ...l.keys };
          tab = "freestyle";
        } else if (l.effect === "firmware") {
          ({ color, glow } = l);
          effect = `fw:${l.id}`;
        } else if (l.effect === "ripple") {
          color = l.color;
          effect = "fw:ripple";
        } else {
          effect = l.effect;
          if ("color" in l) color = l.color;
          if ("speed" in l) speed = l.speed;
          if ("intensity" in l) intensity = l.intensity;
          if ("saturation" in l) saturation = l.saturation;
          if ("glow" in l) ({ glow, backdrop } = l);
          if (l.effect === "pulse") ({ base, width, fade, rainbow } = l);
          if (l.effect === "starlight") ({ star: color, sky: base, density } = l);
          if (l.effect === "visualizer") ({ low: color, high: peak, background: base } = l);
          if (l.effect === "screen") ({ boost } = l);
          if (l.effect === "sequence") ({ steps, blend, spread } = l);
        }
      }
    }
  }

  let saved = $state<{ name: string; lighting: Lighting }[]>([]);
  let presetName = $state("");
  let presetMessage = $state("");
  /** Saved presets this device can show (live effects need per-key lighting, and so on). */
  const usable = $derived(
    saved.filter(({ lighting: l }) =>
      l.effect === "freestyle" ? canPaint : l.effect === "firmware" ? options.some((o) => o.key === `fw:${l.id}`) : options.some((o) => o.key === l.effect) || l.effect === "ripple",
    ),
  );
  function applyPreset(l: Lighting) {
    show(l);
    if (l.effect === "freestyle") pushFreestyle();
    else push();
  }
  async function savePreset() {
    try {
      const l = tab === "freestyle" ? ({ effect: "freestyle", base: freestyle.base, keys: { ...freestyle.keys } } as Lighting) : lighting();
      saved = await api.saveLightingPreset(presetName, l);
      presetName = "";
    } catch (e) {
      onerror(e);
    }
  }

  onMount(async () => {
    api.lightingPresets().then((p) => (saved = p)).catch(() => {});
    try {
      const cfg = await api.getConfig(device.id);
      if (cfg.lighting) show(cfg.lighting);
      if (cfg.brightness !== undefined) brightness = cfg.brightness;
      power = cfg.power ?? true;
    } catch (e) {
      onerror(e);
    }
    loaded = true;
  });

  function lighting(): Lighting {
    if (effect.startsWith("fw:")) return { effect: "firmware", id: effect.slice(3), color, glow };
    switch (effect) {
      case "breathing": return { effect, color, speed, intensity };
      case "cycle": return { effect, speed, intensity, saturation };
      case "wave": return { effect, speed };
      case "pulse": return { effect, color, base, speed, width, fade, rainbow, glow, backdrop };
      case "starlight": return { effect, star: color, sky: base, density, glow, backdrop };
      case "visualizer": return { effect, low: color, high: peak, background: base, glow, backdrop };
      case "screen": return { effect, speed, boost };
      case "sequence": return { effect, steps: steps.map((s) => ({ ...s })), blend, spread };
      default: return { effect: "fixed", color };
    }
  }

  const push = debounce(() => loaded && api.setLighting(device.id, lighting()).catch(onerror), 120);
  const pushFreestyle = debounce(
    () => loaded && api.setLighting(device.id, { effect: "freestyle", base: freestyle.base, keys: { ...freestyle.keys } }).catch(onerror),
    150,
  );

  // The stage paints keys while the Freestyle tab is open; every stroke is saved.
  $effect(() => {
    freestyle.active = tab === "freestyle";
    freestyle.onchange = pushFreestyle;
  });
  onDestroy(() => (freestyle.active = false));

  function clearPaint() {
    freestyle.keys = {};
    pushFreestyle();
  }
  const pushBrightness = debounce(() => api.setBrightness(device.id, brightness).catch(onerror));

  const current = $derived(options.find((o) => o.key === effect));
  const firmware = $derived(effect.startsWith("fw:"));
  const hasColor = $derived(!["cycle", "wave", "screen", "sequence"].includes(effect));
  const hasSpeed = $derived(["breathing", "cycle", "wave", "pulse", "screen"].includes(effect));
  const layered = $derived(["pulse", "starlight", "visualizer"].includes(effect));
  const colorLabel = $derived(
    effect === "starlight" ? "Star color" : effect === "pulse" ? "Pulse color" : effect === "visualizer" ? "Low color" : firmware ? "Effect color" : "Color",
  );
</script>

<div class="panel">
  <label class="power">
    <span class="label">Power</span>
    <button class="switch" class:on={power} onclick={() => { power = !power; setPower(); }} aria-pressed={power}>{power ? "On" : "Off"}</button>
  </label>

  <div class:disabled={!power} class="body">
  {#if canPaint}
    <div class="tabs">
      <button class:active={tab === "presets"} onclick={() => { tab = "presets"; push(); }}>Presets</button>
      <button class:active={tab === "freestyle"} onclick={() => { tab = "freestyle"; pushFreestyle(); }}>Freestyle</button>
    </div>
  {/if}

  {#if tab === "freestyle"}
    <section>
      <p class="note">Click keys on the picture to paint them; click again to clear one.</p>
      <span class="label">Brush</span>
      <ColorPicker bind:value={freestyle.brush} onchange={() => {}} />
      <label class="inline"><span class="label">Unpainted keys</span><input type="color" bind:value={freestyle.base} oninput={pushFreestyle} /></label>
      <button onclick={clearPaint}>Clear all</button>
    </section>
  {:else}
  <section>
    <h3>On the device</h3>
    <div class="effects">
      {#each options.filter((o) => o.group === "device") as o}
        <button class:active={effect === o.key} onclick={() => { effect = o.key; push(); }}>{o.label}</button>
      {/each}
    </div>
    {#if options.some((o) => o.group === "live")}
      <h3>Live <span class="hint">drawn by the GlowDen service</span></h3>
      <div class="effects">
        {#each options.filter((o) => o.group === "live") as o}
          <button class:active={effect === o.key} onclick={() => { effect = o.key; push(); }}>{o.label}</button>
        {/each}
      </div>
    {/if}
  </section>

  {#if hasColor}
    <section>
      <span class="label">{colorLabel}</span>
      <ColorPicker bind:value={color} onchange={push} />
      {#if effect === "visualizer"}
        <label class="inline"><span class="label">Peak color</span><input type="color" bind:value={peak} oninput={push} /></label>
      {/if}
      {#if layered}
        <label class="inline">
          <span class="label">{effect === "pulse" ? "Resting color" : effect === "visualizer" ? "Background" : "Sky color"}</span>
          <input type="color" bind:value={base} oninput={push} />
        </label>
      {/if}
    </section>
  {/if}
  <section>
    {#if hasSpeed}
      <label><span class="label">Speed <em>{speed}</em></span><input type="range" min="1" max="100" bind:value={speed} oninput={push} /></label>
    {/if}
    {#if effect === "breathing" || effect === "cycle"}
      <label><span class="label">Intensity <em>{intensity}%</em></span><input type="range" min="1" max="100" bind:value={intensity} oninput={push} /></label>
    {/if}
    {#if effect === "cycle"}
      <label><span class="label">Saturation <em>{Math.round((saturation / 255) * 100)}%</em></span><input type="range" min="0" max="255" bind:value={saturation} oninput={push} /></label>
    {/if}
    {#if effect === "pulse"}
      <label><span class="label">Ring width <em>{width}</em></span><input type="range" min="1" max="100" bind:value={width} oninput={push} /></label>
      <label><span class="label">Fade time <em>{fade}</em></span><input type="range" min="1" max="100" bind:value={fade} oninput={push} /></label>
      <label class="inline"><input type="checkbox" bind:checked={rainbow} onchange={push} /><span>Rainbow rings</span></label>
    {/if}
    {#if effect === "sequence"}
      <span class="label">Steps</span>
      <ol class="steps">
        {#each steps as s, i}
          <li>
            <input type="color" bind:value={s.color} oninput={push} aria-label="Step {i + 1} color" />
            <label class="ms"><input type="number" min="50" max="60000" step="50" bind:value={s.ms} oninput={push} /> ms</label>
            <button class="ghost" disabled={i === 0} onclick={() => { [steps[i - 1], steps[i]] = [steps[i], steps[i - 1]]; push(); }} aria-label="Move up">↑</button>
            <button class="ghost" disabled={steps.length <= 1} onclick={() => { steps = steps.filter((_, j) => j !== i); push(); }} aria-label="Remove step">✕</button>
          </li>
        {/each}
      </ol>
      {#if steps.length < 64}
        <button onclick={() => { steps = [...steps, { color: steps.at(-1)?.color ?? "#ffffff", ms: 1000 }]; push(); }}>+ Step</button>
      {/if}
      <label class="inline"><input type="checkbox" bind:checked={blend} onchange={push} /><span>Blend into the next step</span></label>
      <label><span class="label">Sweep across the board <em>{spread}</em></span><input type="range" min="0" max="100" bind:value={spread} oninput={push} /></label>
      <p class="note">Save it under "My presets" to keep it, export it or apply it to other devices.</p>
    {/if}
    {#if effect === "screen"}
      <label><span class="label">Color boost <em>{boost}</em></span><input type="range" min="0" max="100" bind:value={boost} oninput={push} /></label>
      <p class="note">
        The first time, your desktop asks which screen to share; GlowDen remembers the answer.
        Each key shows the part of the screen at its place on the board.
      </p>
    {/if}
    {#if effect === "starlight"}
      <label><span class="label">Density <em>{density}</em></span><input type="range" min="1" max="100" bind:value={density} oninput={push} /></label>
    {/if}
    {#if layered || firmware}
      <label><span class="label">Effect brightness <em>{glow}%</em></span><input type="range" min="0" max="100" bind:value={glow} oninput={push} /></label>
    {/if}
    {#if layered}
      <label><span class="label">Background brightness <em>{backdrop}%</em></span><input type="range" min="0" max="100" bind:value={backdrop} oninput={push} /></label>
    {/if}
    {#if device.caps.includes("brightness")}
      <label><span class="label">{layered || firmware ? "Overall brightness" : "Brightness"} <em>{brightness}%</em></span><input type="range" min="0" max="100" bind:value={brightness} oninput={pushBrightness} /></label>
    {/if}
  </section>

  {/if}

  </div>

  <section>
    <h3>My presets</h3>
    {#if usable.length}
      <div class="effects">
        {#each usable as p}
          <span class="mine">
            <button onclick={() => applyPreset(p.lighting)}>{p.name}</button>
            <button class="x" onclick={async () => (saved = await api.deleteLightingPreset(p.name))} aria-label="Delete {p.name}">✕</button>
          </span>
        {/each}
      </div>
    {/if}
    <div class="save-row">
      <input placeholder="Save current lighting as…" bind:value={presetName} onkeydown={(e) => e.key === "Enter" && presetName.trim() && savePreset()} />
      <button onclick={savePreset} disabled={!presetName.trim()}>Save</button>
    </div>
    <div class="save-row">
      <button class="ghost" onclick={async () => (presetMessage = `Saved to ${await api.exportLightingPresets()}`)}>Export all</button>
      <button class="ghost" onclick={async () => {
        const path = prompt("Path of a presets file to import");
        if (path) saved = await api.importLightingPresets(path).catch((e) => (onerror(e), saved));
      }}>Import…</button>
    </div>
    {#if presetMessage}<span class="note">{presetMessage}</span>{/if}
  </section>

  {#if tab === "presets" && !effect.startsWith("fw:")}
    <div class="sync">
      <button onclick={sync} disabled={!power}>Sync lighting to all devices</button>
      {#if syncMessage}<span class="note">{syncMessage}</span>{/if}
    </div>
  {/if}

  <p class="note">
    {tab === "freestyle"
      ? "Freestyle is sent to the device once and stays there."
      : current?.group === "live"
      ? "Live effects keep running after you close this window, as long as the GlowDen service runs."
      : "This effect runs on the device itself and stays after the service stops."}
  </p>
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 26px; }
  .body { display: flex; flex-direction: column; gap: 26px; }
  .body.disabled { opacity: 0.35; pointer-events: none; }
  .power { flex-direction: row; align-items: center; justify-content: space-between; }
  .switch { min-width: 64px; border-radius: 999px; font-weight: 700; }
  .switch.on { background: var(--text); color: var(--bg); border-color: transparent; }
  .sync { display: flex; flex-direction: column; gap: 8px; }
  .mine { display: flex; }
  .mine button:first-child { flex: 1; text-align: left; padding: 10px 12px; border-top-right-radius: 0; border-bottom-right-radius: 0; }
  .mine .x { border-top-left-radius: 0; border-bottom-left-radius: 0; padding: 0 10px; color: var(--text-dim); }
  .save-row { display: flex; gap: 8px; }
  .save-row input { flex: 1; }
  .ghost { background: none; }
  .tabs { display: flex; gap: 18px; border-bottom: 1px solid var(--line); }
  .tabs button { border: none; background: none; border-radius: 0; padding: 8px 2px; color: var(--text-dim); font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; font-size: 12px; }
  .tabs button.active { color: var(--text); box-shadow: inset 0 -2px 0 var(--accent); }
  section { display: flex; flex-direction: column; gap: 12px; }
  h3 { margin: 4px 0 0; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .hint { text-transform: none; letter-spacing: 0; font-weight: 400; color: var(--text-faint); margin-left: 6px; }
  .effects { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; }
  .effects button { text-align: left; padding: 10px 12px; }
  .effects button.active { border-color: var(--accent); color: var(--accent); background: var(--bg-hover); }
  label { display: flex; flex-direction: column; gap: 8px; }
  label.inline { flex-direction: row; align-items: center; justify-content: space-between; }
  .label em { font-style: normal; color: var(--text); margin-left: 6px; text-transform: none; letter-spacing: 0; }
  .swatches { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .swatch { width: 28px; height: 28px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.active { border-color: var(--text); }
  input[type="color"] { width: 38px; height: 30px; border: 1px solid var(--line); border-radius: 8px; background: none; padding: 2px; cursor: pointer; }
  input[type="checkbox"] { accent-color: var(--accent); width: 16px; height: 16px; }
  .note { color: var(--text-faint); font-size: 12px; margin: 0; }
  .steps { margin: 0; padding-left: 22px; display: flex; flex-direction: column; gap: 6px; }
  .steps li { display: flex; gap: 8px; align-items: center; }
  .steps .ms { display: flex; gap: 6px; align-items: center; font-size: 12px; color: var(--text-dim); }
  .steps .ms input { width: 80px; }
  .ghost { background: none; border: none; color: var(--text-dim); padding: 2px 6px; }
</style>
