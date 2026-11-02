<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Device, type Layout, type LightState } from "./api";
  import DeviceVisual from "./DeviceVisual.svelte";
  import { freestyle } from "./freestyle.svelte";

  let {
    device,
    live = true,
    compact = false,
    size = 420,
  }: { device: Device; live?: boolean; compact?: boolean; size?: number } = $props();

  let layout = $state<Layout | null>(null);
  let loaded = $state(false);
  let state = $state<LightState | null>(null);
  let canvas = $state<HTMLCanvasElement>();
  let timer: ReturnType<typeof setInterval> | undefined;

  // Drawing resources, built once per layout.
  let photo: HTMLCanvasElement | null = null;
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
    timer = setInterval(poll, compact ? 250 : 100);
  });
  onDestroy(() => clearInterval(timer));

  /**
   * Device pictures have transparent cutouts where light shines through (key legends, the
   * Yeti GX logo glyph, diffusers). The lit layer is the LED shapes restricted to those
   * holes; the photo's opaque parts are drawn around it. Computed once per layout.
   */
  async function prepare(l: Layout) {
    const img = new Image();
    img.src = l.image;
    await img.decode();
    // The picture as shown: cropped to the device when the layout says so.
    photo = document.createElement("canvas");
    photo.width = l.width;
    photo.height = l.height;
    const pc = photo.getContext("2d", { willReadFrequently: true })!;
    if (l.crop) pc.drawImage(img, l.crop.x, l.crop.y, l.crop.width, l.crop.height, 0, 0, l.width, l.height);
    else pc.drawImage(img, 0, 0, l.width, l.height);

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
    const m = mask.getContext("2d", { willReadFrequently: true })!;
    m.fillStyle = "#fff";
    for (const { path } of paths) m.fill(path);
    if (l.cutout === "vivid") openVivid(pc, m, l.width, l.height);
    else {
      m.globalCompositeOperation = "destination-out";
      m.drawImage(photo, 0, 0);
    }
    if (l.clip) {
      m.globalCompositeOperation = "destination-in";
      m.fill(new Path2D(l.clip));
    }
    layer = document.createElement("canvas");
    layer.width = l.width;
    layer.height = l.height;
    draw();
  }

  /**
   * Product photos show their LEDs already lit. Inside the LED regions, strongly colored
   * pixels become openings: the photo turns grey and transparent there, and the mask keeps
   * them, so live colors replace the photo's.
   */
  function openVivid(pc: CanvasRenderingContext2D, m: CanvasRenderingContext2D, w: number, h: number) {
    const px = pc.getImageData(0, 0, w, h);
    const mk = m.getImageData(0, 0, w, h);
    const d = px.data;
    const k = mk.data;
    const ramp = (x: number, lo: number, span: number) => Math.min(1, Math.max(0, (x - lo) / span));
    for (let i = 0; i < d.length; i += 4) {
      if (k[i + 3] === 0) continue;
      const r = d[i] / 255, g = d[i + 1] / 255, b = d[i + 2] / 255, a = d[i + 3] / 255;
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      const sat = max > 0 ? (max - min) / max : 0;
      const vivid = ramp(sat, 0.25, 0.35) * ramp(max, 0.18, 0.35) * a;
      if (vivid > 0) {
        const lum = (0.3 * r + 0.59 * g + 0.11 * b) * 255;
        d[i] = d[i + 1] = d[i + 2] = lum;
        d[i + 3] = a * (1 - vivid) * 255;
      }
      k[i + 3] = vivid * 255;
    }
    pc.putImageData(px, 0, 0);
    m.putImageData(mk, 0, 0);
  }

  /** While painting, show strokes immediately instead of waiting for the device. */
  function color(zone: number): string {
    if (freestyle.active && !compact) return freestyle.keys[String(zone)] ?? freestyle.base;
    return state?.zones[String(zone)] ?? "#000000";
  }

  /** Device-run animations are approximated on the LEDs only, not the whole picture. */
  function animated(): { alpha: number; hue: number } {
    const t = performance.now();
    if (animation === "breathing") return { alpha: 0.2 + 0.8 * (0.5 - 0.5 * Math.cos((2 * Math.PI * t) / 3000)), hue: 0 };
    if (animation === "cycle") return { alpha: 1, hue: ((t / 6000) * 360) % 360 };
    return { alpha: 1, hue: 0 };
  }

  function draw() {
    if (!canvas || !layout || !photo || !mask || !layer) return;
    const ctx = canvas.getContext("2d")!;
    const lit = live || (freestyle.active && !compact);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(photo, 0, 0);

    const lc = layer.getContext("2d")!;
    lc.globalCompositeOperation = "source-over";
    lc.clearRect(0, 0, layer.width, layer.height);
    for (const { zone, path } of paths) {
      lc.fillStyle = lit ? color(zone) : "#3a3d44";
      lc.fill(path);
    }
    lc.globalCompositeOperation = "destination-in";
    lc.drawImage(mask, 0, 0);

    const { alpha, hue } = lit ? animated() : { alpha: 1, hue: 0 };
    const tint = hue ? ` hue-rotate(${hue}deg)` : "";
    if (lit) {
      ctx.save();
      ctx.globalAlpha = 0.6 * alpha;
      ctx.filter = `blur(5px)${tint}`;
      ctx.drawImage(layer, 0, 0);
      ctx.restore();
    }
    ctx.save();
    ctx.globalAlpha = alpha;
    if (tint) ctx.filter = tint.trim();
    ctx.drawImage(layer, 0, 0);
    ctx.restore();
  }

  // Animations redraw every frame; other states redraw when the device state is polled.
  let frame = 0;
  $effect(() => {
    if (animation !== "breathing" && animation !== "cycle") return;
    const tick = () => {
      draw();
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  });

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

<div class="stage {animation ?? ''}" class:painting={freestyle.active && !compact} class:compact>
  {#if layout}
    <canvas bind:this={canvas} width={layout.width} height={layout.height} onclick={compact ? undefined : paint}></canvas>
  {:else if loaded}
    <div class="plain"><DeviceVisual {device} {size} /></div>
    {#if (live || (freestyle.active && !compact)) && zones.length}
      <div class="strip">
        {#each zones as z}
          <button class="dot" style:background={color(z)} title="zone {z}" onclick={compact ? undefined : () => paintZone(z)} aria-label="zone {z}"></button>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .stage { width: 100%; max-width: 100%; min-width: 0; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 20px; }
  canvas { width: 100%; height: 100%; max-height: 74vh; object-fit: contain; display: block; filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.6)); }
  .painting canvas { cursor: crosshair; }
  /* Devices without a picture: animate the zone dots only. */
  .breathing .dot { animation: breathe 3s ease-in-out infinite; }
  .cycle .dot { animation: hue 6s linear infinite; }
  @keyframes breathe { 50% { opacity: 0.2; } }
  @keyframes hue { to { filter: hue-rotate(360deg); } }
  .plain { display: grid; place-items: center; color: var(--text-dim); max-width: 100%; }
  /* Without a picture the zones are dots; keyboards have dozens, so they wrap. */
  .strip { display: flex; flex-wrap: wrap; gap: 12px; justify-content: center; max-width: min(100%, 720px); }
  .dot { width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 1px solid var(--line); }
  .painting .dot { cursor: crosshair; }
  .compact { gap: 12px; }
  .compact canvas { max-height: 300px; filter: drop-shadow(0 10px 22px rgba(0, 0, 0, 0.5)); }
  .compact .strip { gap: 8px; max-width: 100%; }
  .compact .dot { width: 12px; height: 12px; pointer-events: none; }
</style>
