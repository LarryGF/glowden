<script lang="ts">
  import type { Device } from "./api";
  import DeviceIcon from "./DeviceIcon.svelte";
  import Info from "./panels/Info.svelte";
  import Lighting from "./panels/Lighting.svelte";
  import Microphone from "./panels/Microphone.svelte";
  import Planned from "./panels/Planned.svelte";

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
    ...(device.caps.includes("dpi") ? [{ key: "dpi", label: "Sensitivity", planned: "DPI levels and report rate, coming with the G502 work." }] : []),
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
  <aside>
    <div class="hero">
      <DeviceIcon kind={device.kind} size={84} />
      <div class="name">{device.name}</div>
      {#if device.battery}<div class="battery">{device.battery.percent}%{device.battery.charging ? " · charging" : ""}</div>{/if}
    </div>
    <nav>
      {#each sections as s}
        <button class:active={active === s.key} onclick={() => (active = s.key)}>
          {s.label}
          {#if s.planned}<span class="dot" title="in development"></span>{/if}
        </button>
      {/each}
    </nav>
  </aside>

  <main>
    {#key active}
      {#if current.planned}
        <Planned title={current.label} detail={current.planned} />
      {:else if active === "lighting"}
        <Lighting {device} {onerror} />
      {:else if active === "mic"}
        <Microphone {device} {onerror} />
      {:else if active === "info"}
        <Info {device} />
      {/if}
    {/key}
  </main>
</div>

<style>
  .page { display: grid; grid-template-columns: 240px 1fr; height: 100%; }
  aside { border-right: 1px solid var(--line); background: var(--bg-raised); display: flex; flex-direction: column; }
  .hero { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 28px 16px 22px; color: var(--accent); border-bottom: 1px solid var(--line); }
  .name { color: var(--text); font-weight: 600; text-align: center; }
  .battery { color: var(--text-dim); font-size: 12px; }
  nav { display: flex; flex-direction: column; padding: 10px; gap: 2px; }
  nav button { text-align: left; border: none; background: none; border-radius: 8px; padding: 10px 12px; color: var(--text-dim); display: flex; align-items: center; justify-content: space-between; }
  nav button:hover { background: var(--bg-hover); color: var(--text); }
  nav button.active { background: var(--bg-card); color: var(--text); box-shadow: inset 3px 0 0 var(--accent); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--warn); }
  main { padding: 36px 44px; overflow: auto; }
</style>
