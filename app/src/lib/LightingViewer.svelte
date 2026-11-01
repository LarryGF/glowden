<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Device, type Layout, type LightState } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";
  import { freestyle } from "./freestyle.svelte";

  let { device, live = true }: { device: Device; live?: boolean } = $props();

  let layout = $state<Layout | null>(null);
  let loaded = $state(false);
  let state = $state<LightState | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  const uid = Math.random().toString(36).slice(2, 8);

  onMount(async () => {
    layout = await api.deviceLayout(device.id).catch(() => null);
    loaded = true;
    const poll = () => api.lightingState(device.id).then((s) => (state = s)).catch(() => {});
    poll();
    timer = setInterval(poll, 100);
  });
  onDestroy(() => clearInterval(timer));

  /** While painting, show the brush strokes immediately instead of waiting for the device. */
  function color(zone: number): string {
    if (freestyle.active) return freestyle.keys[String(zone)] ?? freestyle.base;
    return state?.zones[String(zone)] ?? "#000000";
  }

  function paint(zone: number) {
    if (!freestyle.active) return;
    const k = String(zone);
    freestyle.keys[k] = freestyle.keys[k] === freestyle.brush ? freestyle.base : freestyle.brush;
    freestyle.onchange();
  }

  const view = $derived(
    layout?.crop
      ? `${layout.crop.x - 20} ${layout.crop.y - 20} ${layout.crop.width + 40} ${layout.crop.height + 40}`
      : layout
        ? `0 0 ${layout.width} ${layout.height}`
        : "",
  );
  const zones = $derived(Object.keys(state?.zones ?? {}).map(Number).sort((a, b) => a - b));
  const animation = $derived(state?.mode === "device" ? state.effect : null);
  const showLeds = $derived(live || freestyle.active);
</script>

<div class="stage {animation ?? ''}" class:painting={freestyle.active}>
  {#if layout}
    <svg viewBox={view} preserveAspectRatio="xMidYMid meet">
      <defs>
        <!-- Bright legend pixels of the photo become the alpha of the lit layer. -->
        <filter id="legend-{uid}" color-interpolation-filters="sRGB">
          <feColorMatrix type="matrix" values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0.3 0.59 0.11 0 0" />
          <feComponentTransfer><feFuncA type="linear" slope="4.5" intercept="-1.9" /></feComponentTransfer>
        </filter>
        <mask id="legend-mask-{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width={layout.width} height={layout.height}>
          <image href={layout.image} width={layout.width} height={layout.height} filter="url(#legend-{uid})" />
        </mask>
        <filter id="bloom-{uid}"><feGaussianBlur stdDeviation="6" /></filter>
      </defs>

      <image href={layout.image} width={layout.width} height={layout.height} />

      {#if showLeds}
        {#snippet shapes(fillFor: (z: number) => string)}
          {#each layout!.shapes as { zone, shape }}
            {#if shape.type === "rect"}
              <rect x={shape.rect.x} y={shape.rect.y} width={shape.rect.width} height={shape.rect.height} rx="10" fill={fillFor(zone)} />
            {:else if shape.type === "circle"}
              <circle cx={shape.rect.x} cy={shape.rect.y} r={shape.rect.r} fill={fillFor(zone)} />
            {:else}
              <path d={shape.d} fill={fillFor(zone)} />
            {/if}
          {/each}
        {/snippet}

        {#if layout.legends}
          <g mask="url(#legend-mask-{uid})">
            <g filter="url(#bloom-{uid})" opacity="0.7">{@render shapes(color)}</g>
            {@render shapes(color)}
          </g>
        {:else}
          <g class="tint">{@render shapes(color)}</g>
          <g class="glow">{@render shapes(color)}</g>
        {/if}

        {#if freestyle.active}
          <g class="hit">{@render shapes(() => "transparent")}</g>
          {#each layout.shapes as { zone, shape }}
            {#if shape.type === "rect"}
              <rect class="target" x={shape.rect.x} y={shape.rect.y} width={shape.rect.width} height={shape.rect.height} rx="10" onclick={() => paint(zone)} role="button" tabindex="-1" aria-label="zone {zone}" />
            {:else if shape.type === "circle"}
              <circle class="target" cx={shape.rect.x} cy={shape.rect.y} r={shape.rect.r} onclick={() => paint(zone)} role="button" tabindex="-1" aria-label="zone {zone}" />
            {:else}
              <path class="target" d={shape.d} onclick={() => paint(zone)} role="button" tabindex="-1" aria-label="zone {zone}" />
            {/if}
          {/each}
        {/if}
      {/if}
    </svg>
  {:else if loaded}
    <div class="plain"><DeviceVisual {device} size={420} /></div>
    {#if showLeds && zones.length}
      <div class="strip">
        {#each zones as z}
          <button class="dot" style:background={color(z)} title="zone {z}" onclick={() => paint(z)} aria-label="zone {z}"></button>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .stage { width: 100%; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 20px; }
  svg { width: 100%; height: 100%; max-height: 74vh; display: block; filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); }
  /* Non-keyboard LEDs: keep the photo's shading, take the zone's hue, add a little light. */
  .tint { mix-blend-mode: color; }
  .glow { mix-blend-mode: screen; opacity: 0.35; }
  .breathing .tint, .breathing .glow, .breathing :global(g[mask]) { animation: breathe 3s ease-in-out infinite; }
  .cycle svg { animation: hue 6s linear infinite; }
  @keyframes breathe { 50% { opacity: 0.2; } }
  @keyframes hue { to { filter: hue-rotate(360deg) drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); } }
  .target { fill: transparent; cursor: crosshair; }
  .target:hover { fill: rgba(255, 255, 255, 0.12); }
  .plain { display: grid; place-items: center; color: var(--text-dim); }
  .strip { display: flex; gap: 12px; justify-content: center; }
  .dot { width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 1px solid var(--line); }
  .painting .dot { cursor: crosshair; }
</style>
