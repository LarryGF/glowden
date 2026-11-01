<script lang="ts">
  import type { Device } from "./api";
  import LightingViewer from "./LightingViewer.svelte";
  import Info from "./panels/Info.svelte";
  import Lighting from "./panels/Lighting.svelte";
  import Microphone from "./panels/Microphone.svelte";
  import Planned from "./panels/Planned.svelte";
  import Sensitivity from "./panels/Sensitivity.svelte";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  type Section = { key: string; label: string; planned?: string };

  const sections: Section[] = $derived([
    ...(device.caps.includes("lighting") ? [{ key: "lighting", label: "Lighting" }] : []),
    ...(device.caps.includes("mic_gain") ? [{ key: "mic", label: "Microphone" }] : []),
    ...(device.caps.includes("blue_voice")
      ? [{ key: "voice", label: "Blue VO!CE", planned: "Logitech's own VO!CE 2 engine already runs natively in testing. Presets and the virtual microphone come next." }]
      : []),
    ...(device.caps.includes("equalizer")
      ? [{ key: "eq", label: "Equalizer", planned: "Parametric EQ is exposed by the headset (feature 0x020d); decoding is scheduled with the G522 work." }]
      : []),
    ...(device.caps.includes("logivoice")
      ? [{ key: "logivoice", label: "Mic processing", planned: "The headset runs noise reduction, gate, compressor, de-esser and EQ on-device (features 0x0900–0x0908)." }]
      : []),
    ...(device.caps.includes("sidetone") ? [{ key: "sidetone", label: "Sidetone", planned: "Feature 0x0604, coming with the G522 work." }] : []),
    ...(device.caps.includes("dpi") ? [{ key: "dpi", label: "Sensitivity" }] : []),
    ...(device.caps.includes("assignments")
      ? [{ key: "assign", label: "Assignments", planned: "Button and key remapping, coming with the keyboard and mouse work." }]
      : []),
    { key: "info", label: "Info" },
  ]);

  let active = $state("");
  $effect(() => {
    if (!sections.some((s) => s.key === active)) active = sections[0].key;
  });
  const current = $derived(sections.find((s) => s.key === active) ?? sections[0]);
</script>

<div class="page">
  <nav>
    {#each sections as s}
      <button class:active={active === s.key} onclick={() => (active = s.key)}>
        {s.label}
        {#if s.planned}<span class="dot" title="in development"></span>{/if}
      </button>
    {/each}
  </nav>

  <section class="stage">
    <div class="title">
      <h1>{device.name}</h1>
      <span class="meta">
        {device.connection}{#if device.battery} · {device.battery.percent}%{device.battery.charging ? " charging" : ""}{/if}
      </span>
    </div>
    <div class="visual">
      {#key device.id}
        <LightingViewer {device} live={active === "lighting"} />
      {/key}
    </div>
  </section>

  <aside class="settings">
    <h2>{current.label}</h2>
    {#key active}
      {#if current.planned}
        <Planned title={current.label} detail={current.planned} />
      {:else if active === "lighting"}
        <Lighting {device} {onerror} />
      {:else if active === "mic"}
        <Microphone {device} {onerror} />
      {:else if active === "dpi"}
        <Sensitivity {device} {onerror} />
      {:else if active === "info"}
        <Info {device} />
      {/if}
    {/key}
  </aside>
</div>

<style>
  .page { display: grid; grid-template-columns: 200px minmax(0, 1fr) 400px; height: 100%; }
  nav { display: flex; flex-direction: column; gap: 2px; padding: 18px 10px; border-right: 1px solid var(--line); }
  nav button { text-align: left; border: none; background: none; border-radius: 8px; padding: 11px 14px; color: var(--text-dim); display: flex; align-items: center; justify-content: space-between; }
  nav button:hover { background: var(--bg-hover); color: var(--text); }
  nav button.active { background: var(--bg-card); color: var(--text); box-shadow: inset 3px 0 0 var(--accent); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--warn); }
  .stage { display: grid; grid-template-rows: auto 1fr; min-width: 0; background: radial-gradient(ellipse at 50% 55%, #1c2230 0%, var(--bg) 70%); }
  .title { padding: 26px 34px 0; }
  h1 { margin: 0; font-size: 26px; font-weight: 600; letter-spacing: 0.01em; }
  .meta { color: var(--text-dim); font-size: 13px; }
  .visual { min-height: 0; padding: 20px 34px 34px; color: var(--text-dim); }
  .settings { border-left: 1px solid var(--line); background: var(--bg-raised); padding: 26px 26px 40px; overflow: auto; }
  .settings h2 { margin: 0 0 20px; font-size: 18px; }
</style>
