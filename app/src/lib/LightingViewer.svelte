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
   * The lit layer may only show where the photo has light-emitting surfaces: bright legend
   * pixels on keyboards, the lighter diffuser regions elsewhere. That alpha mask is computed
   * once from the photo's luminance.
   */
  async function prepare(l: Layout) {
    photo = new Image();
    photo.src = l.image;
    await photo.decode();
    const [lo, hi] = l.legends ? [0.42, 0.7] : [0.12, 0.45];
    mask = document.createElement("canvas");
    mask.width = l.width;
    mask.height = l.height;
    const m = mask.getContext("2d", { willReadFrequently: true })!;
    m.drawImage(photo, 0, 0, l.width, l.height);
    const px = m.getImageData(0, 0, l.width, l.height);
    const d = px.data;
    for (let i = 0; i < d.length; i += 4) {
      const lum = (0.3 * d[i] + 0.59 * d[i + 1] + 0.11 * d[i + 2]) / 255;
      const a = Math.min(1, Math.max(0, (lum - lo) / (hi - lo)));
      d[i] = d[i + 1] = d[i + 2] = 255;
      d[i + 3] = (a * d[i + 3]) | 0;
    }
    m.putImageData(px, 0, 0);
    layer = document.createElement("canvas");
    layer.width = l.width;
    layer.height = l.height;
    paths = l.shapes.map(({ zone, shape }) => {
      const p = new Path2D();
      if (shape.type === "rect") p.roundRect(shape.rect.x, shape.rect.y, shape.rect.width, shape.rect.height, 10);
      else if (shape.type === "circle") p.arc(shape.rect.x, shape.rect.y, shape.rect.r, 0, Math.PI * 2);
      else p.addPath(new Path2D(shape.d));
      return { zone, path: p };
    });
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
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(photo, 0, 0, layout.width, layout.height);
    if (!(live || freestyle.active)) return;

    const lc = layer.getContext("2d")!;
    lc.globalCompositeOperation = "source-over";
    lc.clearRect(0, 0, layer.width, layer.height);
    for (const { zone, path } of paths) {
      lc.fillStyle = color(zone);
      lc.fill(path);
    }
    lc.globalCompositeOperation = "destination-in";
    lc.drawImage(mask, 0, 0);

    // A soft halo first, then the crisp lit pixels on top.
    ctx.save();
    ctx.globalAlpha = 0.55;
    ctx.filter = "blur(6px)";
    ctx.drawImage(layer, 0, 0);
    ctx.restore();
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
