<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Device, type Layout, type LightState } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";

  let { device, live = true }: { device: Device; live?: boolean } = $props();

  let layout = $state<Layout | null>(null);
  let loaded = $state(false);
  let state = $state<LightState | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    layout = await api.deviceLayout(device.id).catch(() => null);
    loaded = true;
    if (!live) return;
    const poll = () => api.lightingState(device.id).then((s) => (state = s)).catch(() => {});
    poll();
    timer = setInterval(poll, 100);
  });
  onDestroy(() => clearInterval(timer));

  const color = (zone: number) => state?.zones[String(zone)] ?? "#000000";
  const zones = $derived(Object.keys(state?.zones ?? {}).map(Number).sort((a, b) => a - b));
  // Device-run effects animate on the device; mirror that loosely here.
  const animation = $derived(state?.mode === "device" ? state.effect : null);
</script>

<div class="stage {animation ?? ''}">
  {#if layout}
    <svg viewBox="0 0 {layout.width} {layout.height}" preserveAspectRatio="xMidYMid meet">
      <image href={layout.image} width={layout.width} height={layout.height} />
      {#if live}
        <g class="leds">
          {#each layout.shapes as { zone, shape }}
            {#if shape.type === "rect"}
              <rect x={shape.rect.x} y={shape.rect.y} width={shape.rect.width} height={shape.rect.height} rx="10" fill={color(zone)} />
            {:else if shape.type === "circle"}
              <circle cx={shape.rect.x} cy={shape.rect.y} r={shape.rect.r} fill={color(zone)} />
            {:else}
              <path d={shape.d} fill={color(zone)} />
            {/if}
          {/each}
        </g>
      {/if}
    </svg>
  {:else if loaded}
    <div class="plain"><DeviceVisual {device} size={420} /></div>
    {#if live && zones.length}
      <div class="strip">
        {#each zones as z}
          <span class="dot" style:background={color(z)} title="zone {z}"></span>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .stage { width: 100%; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 20px; }
  svg { width: 100%; height: 100%; max-height: 72vh; display: block; filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.55)); }
  .leds { mix-blend-mode: screen; opacity: 0.85; }
  .breathing .leds, .breathing .dot { animation: breathe 3s ease-in-out infinite; }
  .cycle .leds, .cycle .dot { animation: hue 6s linear infinite; }
  @keyframes breathe { 50% { opacity: 0.15; } }
  @keyframes hue { to { filter: hue-rotate(360deg); } }
  .plain { display: grid; place-items: center; color: var(--text-dim); }
  .strip { display: flex; gap: 12px; justify-content: center; }
  .dot { width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--line); box-shadow: 0 0 14px rgba(255, 255, 255, 0.08); }
</style>
