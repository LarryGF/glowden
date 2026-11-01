<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Device, type Layout, type LightState } from "./api";

  let { device }: { device: Device } = $props();

  let layout = $state<Layout | null>(null);
  let state = $state<LightState | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    layout = await api.deviceLayout(device.id).catch(() => null);
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

<div class="viewer {animation ?? ''}">
  {#if layout}
    <svg viewBox="0 0 {layout.width} {layout.height}" preserveAspectRatio="xMidYMid meet">
      <image href={layout.image} width={layout.width} height={layout.height} />
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
    </svg>
  {:else if zones.length}
    <div class="strip">
      {#each zones as z}
        <span class="dot" style:background={color(z)} title="zone {z}"></span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .viewer { border-radius: var(--radius); border: 1px solid var(--line); background: radial-gradient(ellipse at center, #1b1f28, var(--bg-raised)); padding: 12px; }
  svg { width: 100%; max-height: 300px; display: block; }
  .leds { mix-blend-mode: screen; opacity: 0.85; }
  .breathing .leds, .breathing .dot { animation: breathe 3s ease-in-out infinite; }
  .cycle .leds, .cycle .dot { animation: hue 6s linear infinite; }
  @keyframes breathe { 50% { opacity: 0.15; } }
  @keyframes hue { to { filter: hue-rotate(360deg); } }
  .strip { display: flex; gap: 10px; justify-content: center; padding: 18px 0; }
  .dot { width: 26px; height: 26px; border-radius: 50%; box-shadow: 0 0 12px currentColor; border: 1px solid var(--line); }
</style>
