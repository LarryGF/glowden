<script lang="ts">
  import type { Device } from "./api";
  import LightingViewer from "./LightingViewer.svelte";
  import BatteryBadge from "./BatteryBadge.svelte";
  import Info from "./panels/Info.svelte";
  import Lighting from "./panels/Lighting.svelte";
  import Microphone from "./panels/Microphone.svelte";
  import Planned from "./panels/Planned.svelte";
  import Sensitivity from "./panels/Sensitivity.svelte";
  import Assignments from "./panels/Assignments.svelte";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  type Section = { key: string; label: string; planned?: string };

  const sections: Section[] = $derived([
    ...(device.caps.includes("lighting") ? [{ key: "lighting", label: "Lighting" }] : []),
    ...(device.caps.includes("mic_gain") ? [{ key: "mic", label: "Microphone" }] : []),
    ...(device.caps.includes("equalizer")
      ? [{ key: "eq", label: "Equalizer", planned: "Parametric EQ is exposed by the headset (feature 0x020d); decoding is scheduled with the G522 work." }]
      : []),
    ...(device.caps.includes("logivoice")
      ? [{ key: "logivoice", label: "Mic processing", planned: "The headset runs noise reduction, gate, compressor, de-esser and EQ on-device (features 0x0900–0x0908)." }]
      : []),
    ...(device.caps.includes("sidetone") ? [{ key: "sidetone", label: "Sidetone", planned: "Feature 0x0604, coming with the G522 work." }] : []),
    ...(device.caps.includes("dpi") ? [{ key: "dpi", label: "Sensitivity" }] : []),
    ...(device.caps.includes("buttons")
      ? [{ key: "assign", label: "Assignments" }]
      : device.caps.includes("assignments")
        ? [{ key: "assign", label: "Assignments", planned: "Key remapping for keyboards is next; mouse buttons can already be assigned." }]
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
  <nav aria-label="Sections">
    {#each sections as s}
      <button class:active={active === s.key} onclick={() => (active = s.key)} title={s.label} aria-label={s.label}>
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
          {#if s.key === "lighting"}
            <circle cx="12" cy="12" r="3.2" /><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8" />
          {:else if s.key === "mic" || s.key === "voice" || s.key === "logivoice"}
            <rect x="9" y="3" width="6" height="11" rx="3" /><path d="M6 11a6 6 0 0 0 12 0M12 17v4" />
          {:else if s.key === "dpi"}
            <rect x="7" y="3" width="10" height="18" rx="5" /><path d="M12 3v6" />
          {:else if s.key === "eq"}
            <path d="M5 4v16M12 4v16M19 4v16" /><circle cx="5" cy="14" r="2" /><circle cx="12" cy="8" r="2" /><circle cx="19" cy="16" r="2" />
          {:else if s.key === "sidetone"}
            <path d="M4 14v-2a8 8 0 0 1 16 0v2" /><rect x="3" y="14" width="4" height="6" rx="1.5" /><rect x="17" y="14" width="4" height="6" rx="1.5" />
          {:else if s.key === "assign"}
            <rect x="3" y="3" width="18" height="18" rx="3" /><path d="M12 8v8M8 12h8" />
          {:else}
            <circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7.5v.5" />
          {/if}
        </svg>
        {#if s.planned}<span class="dot" title="in development"></span>{/if}
      </button>
    {/each}
  </nav>

  <aside class="settings">
    <h2>{current.label}</h2>
    {#key active}
      {#if current.planned}
        <Planned title={current.label} detail={current.planned} />
      {:else if active === "lighting"}
        <Lighting {device} {onerror} />
      {:else if active === "mic"}
        <Microphone {device} {onerror} />
      {:else if active === "assign"}
        <Assignments {device} {onerror} />
      {:else if active === "dpi"}
        <Sensitivity {device} {onerror} />
      {:else if active === "info"}
        <Info {device} />
      {/if}
    {/key}
  </aside>

  <section class="stage">
    <div class="status">
      <span class="connection">{device.connection}</span>
      {#if device.battery}
        <BatteryBadge battery={device.battery} detailed />
      {/if}
    </div>
    <div class="visual">
      {#key device.id}
        <LightingViewer {device} />
      {/key}
    </div>
  </section>
</div>

<style>
  .page { display: grid; grid-template-columns: 64px 380px minmax(0, 1fr); height: 100%; }
  nav { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 18px 0; }
  nav button { position: relative; width: 46px; height: 46px; padding: 0; display: grid; place-items: center; border: none; border-radius: 12px; background: none; color: var(--text-dim); }
  nav button:hover { background: var(--bg-hover); color: var(--text); }
  nav button.active { background: var(--accent); color: #062a26; }
  .dot { position: absolute; top: 7px; right: 7px; width: 6px; height: 6px; border-radius: 50%; background: var(--warn); }
  .settings { margin: 14px 0 14px; padding: 24px 24px 40px; background: var(--bg-card); border-radius: 14px; overflow: auto; }
  .settings h2 { margin: 0 0 20px; font-size: 20px; letter-spacing: 0.03em; text-transform: uppercase; }
  .stage { display: grid; grid-template-rows: auto 1fr; min-width: 0; }
  .status { display: flex; justify-content: flex-end; gap: 14px; padding: 22px 34px 0; color: var(--text-dim); font-size: 13px; letter-spacing: 0.05em; text-transform: uppercase; }
  .visual { min-height: 0; padding: 10px 40px 40px; color: var(--text-dim); }
</style>
