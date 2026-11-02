<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Device, type VoiceState } from "../api";
  import PresetPicker from "./PresetPicker.svelte";

  // G HUB's Blue VO!CE tab: enable, preset, advanced controls.
  let {
    device,
    voice,
    onstate,
    onerror,
  }: { device: Device; voice: VoiceState; onstate: (s: VoiceState) => void; onerror: (e: unknown) => void } = $props();

  let busy = $state(false);
  let levels = $state<{ input: number; output: number } | null>(null);
  /** dBFS → 0..100 % of a -60..0 dB meter. */
  const bar = (db: number) => Math.max(0, Math.min(100, ((db + 60) / 60) * 100));

  onMount(() => {
    const meter = setInterval(async () => {
      levels = voice.running ? await api.voiceLevels(device.id).catch(() => null) : null;
    }, 100);
    return () => clearInterval(meter);
  });

  async function run(p: Promise<VoiceState>) {
    busy = true;
    try {
      onstate(await p);
    } catch (e) {
      onerror(e);
    } finally {
      busy = false;
    }
  }
  const apply = (enabled: boolean, preset: string | null, defaultMic = voice.default_mic) =>
    run(api.setVoice(device.id, enabled, preset, defaultMic));

  const ready = $derived(voice.engine && voice.presets.length + voice.custom_presets.length > 0);
  const tuned = $derived(Object.keys(voice.overrides).length > 0);
  const notes = $derived(Object.fromEntries(Object.entries(voice.details).map(([n, d]) => [n, d.note ?? ""])));
  const d = $derived(voice.preset ? voice.details[voice.preset] : undefined);
  const db = (v: number | null | undefined) => (v == null ? "–" : `${v > 0 ? "+" : ""}${v} dB`);

  /** Value of a preset field ("a.b.c") in the current settings. */
  function value(path: string): unknown {
    return path.split(".").reduce<unknown>((v, k) => (v as Record<string, unknown> | undefined)?.[k], voice.settings);
  }

  let advanced = $state(false);
  let pending: Record<string, number | boolean> = {};
  let pushTimer: ReturnType<typeof setTimeout> | undefined;
  function tune(path: string, v: number | boolean) {
    pending = { ...voice.overrides, ...pending, [path]: v };
    clearTimeout(pushTimer);
    pushTimer = setTimeout(() => {
      const send = pending;
      pending = {};
      run(api.setVoiceOverrides(device.id, send));
    }, 250);
  }
  const groups = $derived([...new Set(voice.controls.map((c) => c.group))]);
</script>

<div class="tab">
  <label class="enable">
    <input type="checkbox" checked={voice.enabled} disabled={!ready || busy} onchange={() => apply(!voice.enabled, voice.preset)} />
    <span>Enable Blue VO!CE</span>
  </label>

  {#if !voice.engine}
    <p class="note">
      The VO!CE engine comes from your own G HUB installation: add the folder holding it in
      Settings → G HUB files (on a dual-boot Windows drive it's in
      <code>Windows\System32\DriverStore</code> once G HUB's audio driver is installed).
    </p>
  {:else if !ready}
    <p class="note">
      The presets come from G HUB's settings (<code>settings.db</code> in
      <code>%LOCALAPPDATA%\LGHUB</code>): add that folder in Settings → G HUB files.
    </p>
  {:else}
    <PresetPicker
      curated={voice.presets}
      custom={voice.custom_presets}
      curatedLabel="Blue VO!CE presets"
      customLabel="Custom presets"
      selected={voice.preset}
      {tuned}
      {notes}
      disabled={busy}
      onselect={(name) => apply(voice.enabled, name)}
      onduplicate={() => run(api.voicePresetAction(device.id, "voice", "duplicate"))}
      onrename={(name, to) => run(api.voicePresetAction(device.id, "voice", "rename", name, to))}
      ondelete={(name) => run(api.voicePresetAction(device.id, "voice", "delete", name))}
      onreset={() => run(api.setVoiceOverrides(device.id, {}))}
    />
    {#if d}
      <div class="chips">
        <span title="Noise reduction">Noise {db(d.noise_reduction)}</span>
        <span title="Voice EQ low / mid / high">EQ {db(d.bass)} / {db(d.mids)} / {db(d.treble)}</span>
        {#if d.compression}<span title="Compression ratio">{d.compression}:1</span>{/if}
        {#if d.input_gain != null}<span title="Mic gain the preset sets">Gain {Math.round(d.input_gain * 100)}</span>{/if}
      </div>
    {/if}

    <label class="enable">
      <input type="checkbox" bind:checked={advanced} />
      <span>Advanced controls</span>
    </label>
    {#if advanced}
      {#each groups as g}
        {@const sw = voice.switches.find((s) => s.group === g)}
        <div class="group">
          <div class="group-head">
            <span>{g}</span>
            {#if sw}
              <input type="checkbox" checked={value(sw.path) !== false} onchange={() => tune(sw.path, value(sw.path) === false)} aria-label="{g} on" />
            {/if}
          </div>
          {#each voice.controls.filter((c) => c.group === g) as c}
            {@const v = Number(value(c.path) ?? 0)}
            <label class="slider">
              <span>{c.label}</span>
              <input type="range" min={c.min} max={c.max} step={c.step} value={v} disabled={sw && value(sw.path) === false}
                oninput={(e) => tune(c.path, Number((e.target as HTMLInputElement).value))} />
              <em>{v}{c.unit === ":1" ? ":1" : ` ${c.unit}`}</em>
            </label>
          {/each}
        </div>
      {/each}
    {/if}

    {#if voice.plugin_error}
      <p class="status">{voice.plugin_error}</p>
    {:else if voice.enabled || voice.effect !== "Off"}
      <div class="status" class:ok={voice.running}>
        {#if voice.running}
          <span>Running: choose <strong>{device.name} (Blue VO!CE)</strong> as the microphone in your apps.</span>
          {#if levels}
            <span class="meter"><span>Microphone</span><span class="track"><span class="fill" style:width="{bar(levels.input)}%"></span></span><em>{levels.input <= -119 ? "silent" : `${Math.round(levels.input)} dB`}</em></span>
            <span class="meter"><span>Processed</span><span class="track"><span class="fill" style:width="{bar(levels.output)}%"></span></span><em>{levels.output <= -119 ? "silent" : `${Math.round(levels.output)} dB`}</em></span>
            {#if levels.input <= -119}<span class="hint">No sound from the microphone: it may be muted on the mic itself.</span>{/if}
          {/if}
        {:else}
          Not running{voice.error ? `: ${voice.error}` : ""}.
        {/if}
      </div>
    {/if}

    <label class="enable">
      <input type="checkbox" checked={voice.default_mic} onchange={() => apply(voice.enabled, voice.preset, !voice.default_mic)} />
      <span>Use as the default microphone while on</span>
    </label>
  {/if}
</div>

<style>
  .tab { display: flex; flex-direction: column; gap: 16px; }
  .enable { display: flex; align-items: center; gap: 10px; font-weight: 600; font-size: 13px; text-transform: uppercase; letter-spacing: 0.05em; cursor: pointer; }
  .enable input { width: 16px; height: 16px; accent-color: var(--accent); }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chips span { font-size: 11px; padding: 2px 7px; border-radius: 999px; background: var(--bg); color: var(--text-dim); }
  .group { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--bg); }
  .group-head { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 13px; }
  .group-head input { accent-color: var(--accent); }
  .slider { display: grid; grid-template-columns: 100px 1fr 64px; align-items: center; gap: 10px; font-size: 12px; color: var(--text-dim); }
  .slider em { font-style: normal; color: var(--text); text-align: right; }
  .status { margin: 0; color: var(--warn); font-size: 13px; display: flex; flex-direction: column; gap: 8px; }
  .status.ok { color: var(--accent); }
  .meter { display: grid; grid-template-columns: 80px 1fr 56px; gap: 10px; align-items: center; color: var(--text-dim); font-size: 12px; }
  .meter em { font-style: normal; text-align: right; }
  .track { height: 6px; border-radius: 3px; background: var(--bg); overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--accent); transition: width 0.08s linear; }
  .hint { color: var(--warn); font-size: 12px; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  code { font-size: 11px; }
</style>
