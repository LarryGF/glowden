<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Device, type Layout, type LightState } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";
  import { freestyle } from "./freestyle.svelte";

  let { device, live = true }: { device: Device; live?: boolean } = $props();

  let layout = $state<Layout | null>(null);
  let loaded = $state(false);
  let state = $state<LightState | null>(null);
  let canvas = $state<HTMLCanvasElement>();
  let timer: ReturnType<typeof setInterval> | undefined;

  // Drawing resources, built once per layout.
  let photo: HTMLImageElement | null = null;
  let mask: HTMLCanvasElement | null = null;
  let layer: HTMLCanvasElement | null = null;
  let paths: { zone: number; path: Path2D }[] = [];

  onMount(async () => {
    layout = await api.deviceLayout(device.id).catch(() => null);
    loaded = true;
    if (layout) await prepare(layout);
    const poll = () =>
      api.lightingState(device.id).then((s) => {
        state = s;
        draw();
      }).catch(() => {});
    poll();
    timer = setInterval(poll, 100);
  });
  onDestroy(() => clearInterval(timer));

  /**
   * Device pictures have transparent cutouts where light shines through (key legends, the
   * Yeti GX logo glyph, diffusers). The lit layer is the LED shapes restricted to those
   * holes; the photo's opaque parts are drawn around it. Computed once per layout.
   */
  async function prepare(l: Layout) {
    photo = new Image();
    photo.src = l.image;
    await photo.decode();
    paths = l.shapes.map(({ zone, shape }) => {
      const p = new Path2D();
      if (shape.type === "rect") p.roundRect(shape.rect.x, shape.rect.y, shape.rect.width, shape.rect.height, 10);
      else if (shape.type === "circle") p.arc(shape.rect.x, shape.rect.y, shape.rect.r, 0, Math.PI * 2);
      else p.addPath(new Path2D(shape.d));
      return { zone, path: p };
    });
    mask = document.createElement("canvas");
    mask.width = l.width;
    mask.height = l.height;
    const m = mask.getContext("2d")!;
    m.fillStyle = "#fff";
    for (const { path } of paths) m.fill(path);
    m.globalCompositeOperation = "destination-out";
    m.drawImage(photo, 0, 0, l.width, l.height);
    layer = document.createElement("canvas");
    layer.width = l.width;
    layer.height = l.height;
    draw();
  }

  /** While painting, show strokes immediately instead of waiting for the device. */
  function color(zone: number): string {
    if (freestyle.active) return freestyle.keys[String(zone)] ?? freestyle.base;
    return state?.zones[String(zone)] ?? "#000000";
  }

  function draw() {
    if (!canvas || !layout || !photo || !mask || !layer) return;
    const ctx = canvas.getContext("2d")!;
    const lit = live || freestyle.active;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(photo, 0, 0, layout.width, layout.height);

    const lc = layer.getContext("2d")!;
    lc.globalCompositeOperation = "source-over";
    lc.clearRect(0, 0, layer.width, layer.height);
    for (const { zone, path } of paths) {
      lc.fillStyle = lit ? color(zone) : "#3a3d44";
      lc.fill(path);
    }
    lc.globalCompositeOperation = "destination-in";
    lc.drawImage(mask, 0, 0);

    if (lit) {
      ctx.save();
      ctx.globalAlpha = 0.6;
      ctx.filter = "blur(5px)";
      ctx.drawImage(layer, 0, 0);
      ctx.restore();
    }
    ctx.drawImage(layer, 0, 0);
  }


  function paint(e: MouseEvent) {
    if (!freestyle.active || !canvas || !layout) return;
    const r = canvas.getBoundingClientRect();
    // The canvas keeps its aspect ratio inside the box (object-fit: contain).
    const scale = Math.min(r.width / layout.width, r.height / layout.height);
    const x = (e.clientX - r.left - (r.width - layout.width * scale) / 2) / scale;
    const y = (e.clientY - r.top - (r.height - layout.height * scale) / 2) / scale;
    const ctx = canvas.getContext("2d")!;
    const hit = paths.find(({ path }) => ctx.isPointInPath(path, x, y));
    if (hit) paintZone(hit.zone);
  }

  function paintZone(zone: number) {
    if (!freestyle.active) return;
    const k = String(zone);
    freestyle.keys[k] = freestyle.keys[k] === freestyle.brush ? freestyle.base : freestyle.brush;
    freestyle.onchange();
    draw();
  }

  $effect(() => {
    // Redraw when painting mode or the live flag changes.
    void freestyle.active;
    void live;
    draw();
  });

  const zones = $derived(Object.keys(state?.zones ?? {}).map(Number).sort((a, b) => a - b));
  const animation = $derived(state?.mode === "device" ? state.effect : null);
</script>

<div class="stage {animation ?? ''}" class:painting={freestyle.active}>
  {#if layout}
    <canvas bind:this={canvas} width={layout.width} height={layout.height} onclick={paint}></canvas>
  {:else if loaded}
    <div class="plain"><DeviceVisual {device} size={420} /></div>
    {#if (live || freestyle.active) && zones.length}
      <div class="strip">
        {#each zones as z}
          <button class="dot" style:background={color(z)} title="zone {z}" onclick={() => paintZone(z)} aria-label="zone {z}"></button>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .stage { width: 100%; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 20px; }
  canvas { width: 100%; height: 100%; max-height: 74vh; object-fit: contain; display: block; filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); }
  .painting canvas { cursor: crosshair; }
  .breathing canvas { animation: breathe 3s ease-in-out infinite; }
  .cycle canvas { animation: hue 6s linear infinite; }
  @keyframes breathe { 50% { filter: brightness(0.6) drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); } }
  @keyframes hue { to { filter: hue-rotate(360deg) drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); } }
  .plain { display: grid; place-items: center; color: var(--text-dim); }
  .strip { display: flex; gap: 12px; justify-content: center; }
  .dot { width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 1px solid var(--line); }
  .painting .dot { cursor: crosshair; }
</style>
