<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Device, type VoiceState } from "../api";
  import VoiceEffects from "./VoiceEffects.svelte";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  let state = $state<VoiceState | null>(null);
  let busy = $state(false);

  onMount(() => {
    const load = () => api.voiceState(device.id).then((s) => (state = s)).catch(onerror);
    load();
    // Picks up a host that stopped on its own.
    const timer = setInterval(load, 3000);
    return () => clearInterval(timer);
  });

  async function apply(enabled: boolean, preset: string | null, defaultMic = state?.default_mic ?? true) {
    busy = true;
    try {
      state = await api.setVoice(device.id, enabled, preset, defaultMic);
    } catch (e) {
      onerror(e);
    } finally {
      busy = false;
    }
  }

  const ready = $derived(state?.engine && (state?.presets.length ?? 0) > 0);

  /** Our own wording for G HUB's preset names: "<placement> - <character>". */
  function describe(name: string): string {
    const [place, character] = name.split(" - ");
    const where =
      place === "Desk" ? "Mic on the desk, further from your mouth: more gain." :
      place === "Boom Arm" ? "Mic on a boom arm, close to your mouth." : "";
    const how =
      character === "Natural" ? "Lightest processing, closest to the mic's own sound." :
      character === "Classic" ? "All-round cleanup with a balanced tone." :
      character === "More Bass" ? "Warmer and fuller: bass up, muddy mids down." :
      character === "Broadcaster" ? "Radio voice: strong compression for an even, up-front level." : "";
    return [how, where].filter(Boolean).join(" ");
  }

  /** Value of a preset field ("a.b.c") in the current settings. */
  function value(path: string): unknown {
    return path.split(".").reduce<unknown>((v, k) => (v as Record<string, unknown> | undefined)?.[k], state?.settings);
  }

  let advanced = $state(false);
  let pending: Record<string, number | boolean> = {};
  let pushTimer: ReturnType<typeof setTimeout> | undefined;
  function tune(path: string, v: number | boolean) {
    pending = { ...(state?.overrides ?? {}), ...pending, [path]: v };
    clearTimeout(pushTimer);
    pushTimer = setTimeout(async () => {
      const send = pending;
      pending = {};
      try {
        state = await api.setVoiceOverrides(device.id, send);
      } catch (e) {
        onerror(e);
      }
    }, 250);
  }
  async function resetTuning() {
    pending = {};
    try {
      state = await api.setVoiceOverrides(device.id, {});
    } catch (e) {
      onerror(e);
    }
  }
  const groups = $derived([...new Set(state?.controls.map((c) => c.group) ?? [])]);

  const db = (v: number | null | undefined) => (v == null ? "–" : `${v > 0 ? "+" : ""}${v} dB`);

  let recording = $state(0);
  let playing = $state<string | null>(null);

  async function record() {
    recording = 5;
    const tick = setInterval(() => (recording = Math.max(0, recording - 1)), 1000);
    try {
      await api.voiceTestRecord(device.id, 5);
      if (state) state.has_test = true;
    } catch (e) {
      onerror(e);
    } finally {
      clearInterval(tick);
      recording = 0;
    }
  }

  async function play(preset: string | null, effect: string | null = null) {
    try {
      await api.voiceTestPlay(preset, effect);
      playing = effect ? `${preset ?? ""}+${effect}` : (preset ?? "original");
    } catch (e) {
      onerror(e);
    }
  }

  async function stopPlaying() {
    playing = null;
    await api.voiceTestStop().catch(onerror);
  }
</script>

{#if state}
  <div class="panel">
    <div class="head">
      <span class="label">Blue VO!CE</span>
      <button
        class="switch"
        class:on={state.enabled}
        disabled={!ready || busy}
        aria-pressed={state.enabled}
        onclick={() => apply(!state!.enabled, state!.preset)}>{state.enabled ? "On" : "Off"}</button
      >
    </div>

    {#if !state.engine}
      <p class="note">
        The VO!CE engine comes from your own G HUB download. Import it with
        <code>lhub import-ghub &lt;G HUB depots folder&gt;</code>.
      </p>
    {:else if state.presets.length === 0}
      <p class="note">
        Import the presets from G HUB's settings with <code>lhub import-voice-presets settings.json</code>.
      </p>
    {:else}
      <section>
        <h3>Presets</h3>
        <div class="presets">
          {#each state.presets as name}
            {@const d = state.details[name]}
            <button
              class="preset"
              class:active={state.preset === name}
              disabled={busy}
              onclick={() => apply(state!.enabled, name)}
            >
              <span class="name">{name}</span>
              <span class="desc">{describe(name)}</span>
              {#if d}
                <span class="chips">
                  <span title="Noise reduction">Noise {db(d.noise_reduction)}</span>
                  <span title="Equalizer bass / mids / treble">EQ {db(d.bass)} / {db(d.mids)} / {db(d.treble)}</span>
                  {#if d.compression}<span title="Compression ratio">{d.compression}:1</span>{/if}
                  {#if d.input_gain != null}<span title="Input gain">Gain {Math.round(d.input_gain * 100)}</span>{/if}
                </span>
              {/if}
            </button>
          {/each}
        </div>
      </section>

      <section>
        <button class="disclose" onclick={() => (advanced = !advanced)} aria-expanded={advanced}>
          {advanced ? "▾" : "▸"} Advanced{Object.keys(state.overrides).length ? " (tuned)" : ""}
        </button>
        {#if advanced}
          <p class="note">Fine-tune the selected preset. Changes apply live and are kept until you pick another preset.</p>
          {#each groups as g}
            {@const sw = state.switches.find((s) => s.group === g)}
            <div class="group">
              <div class="group-head">
                <span>{g}</span>
                {#if sw}
                  <button class="switch small" class:on={value(sw.path) !== false} onclick={() => tune(sw.path, value(sw.path) === false)}>
                    {value(sw.path) === false ? "Off" : "On"}
                  </button>
                {/if}
              </div>
              {#each state.controls.filter((c) => c.group === g) as c}
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
          <div class="buttons"><button onclick={resetTuning} disabled={!Object.keys(state.overrides).length}>Reset to preset</button></div>
        {/if}
      </section>

      <VoiceEffects {device} {state} onstate={(s) => (state = s)} {onerror}
        onplay={(effect) => play(state!.enabled ? state!.preset : null, effect)} />

      <section class="test">
        <h3>Test</h3>
        <p class="note">Record a few seconds, then compare the original with any preset on the same take. Use headphones.</p>
        <div class="buttons">
          <button onclick={record} disabled={recording > 0}>{recording ? `Recording… ${recording}` : "Record 5 s"}</button>
          <button onclick={() => play(null)} disabled={!state.has_test || recording > 0} class:active={playing === "original"}>Play original</button>
          <button onclick={() => play(state!.preset)} disabled={!state.has_test || recording > 0 || !state.preset} class:active={playing !== null && playing !== "original"}>
            Play with {state.preset ?? "preset"}
          </button>
          {#if state.effect !== "Off"}
            <button onclick={() => play(state!.enabled ? state!.preset : null, state!.effect)} disabled={!state.has_test || recording > 0}>
              Play with {state.enabled ? `${state.preset} + ` : ""}{state.effect}
            </button>
          {/if}
          <button onclick={stopPlaying} disabled={!playing}>Stop</button>
        </div>
      </section>

      <label class="row-inline">
        <span>Use as the default microphone while on</span>
        <button class="switch small" class:on={state.default_mic} onclick={() => apply(state!.enabled, state!.preset, !state!.default_mic)}>
          {state.default_mic ? "On" : "Off"}
        </button>
      </label>

      {#if state.enabled || state.effect !== "Off"}
        <p class="status" class:ok={state.running}>
          {#if state.running}
            Running. Choose <strong>{device.name} (Blue VO!CE)</strong> as the microphone in your apps.
          {:else}
            Not running{state.error ? `: ${state.error}` : ""}.
          {/if}
        </p>
      {/if}
      <p class="note">
        Switching VO!CE on or changing preset sets the microphone's input gain from the preset and
        its smart-lock setting, as G HUB does. Logitech's engine runs on this computer; the plain
        microphone stays available too.
      </p>
    {/if}
  </div>
{/if}

<style>
  .panel { display: flex; flex-direction: column; gap: 22px; max-width: 560px; }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .label { font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; font-size: 13px; }
  .switch { min-width: 64px; border-radius: 999px; font-weight: 700; }
  .switch.on { background: var(--text); color: var(--bg); border-color: transparent; }
  section { display: flex; flex-direction: column; gap: 12px; }
  h3 { margin: 0; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .presets { display: flex; flex-direction: column; gap: 8px; }
  .preset { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; text-align: left; padding: 10px 12px; }
  .preset.active { border-color: var(--accent); background: var(--bg-hover); }
  .preset.active .name { color: var(--accent); }
  .name { font-weight: 600; }
  .desc { color: var(--text-dim); font-size: 12px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chips span { font-size: 11px; padding: 2px 7px; border-radius: 999px; background: var(--bg); color: var(--text-dim); }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .buttons button.active { border-color: var(--accent); color: var(--accent); }
  .disclose { background: none; border: none; padding: 0; text-align: left; color: var(--text-dim); font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; font-size: 12px; }
  .group { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--bg); }
  .group-head { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 13px; }
  .slider { display: grid; grid-template-columns: 110px 1fr 70px; align-items: center; gap: 10px; font-size: 12px; color: var(--text-dim); }
  .slider em { font-style: normal; color: var(--text); text-align: right; }
  .switch.small { min-width: 48px; padding: 3px 10px; font-size: 12px; }
  .row-inline { display: flex; justify-content: space-between; align-items: center; font-size: 13px; }
  .status { margin: 0; color: var(--warn); }
  .status.ok { color: var(--accent); }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  code { font-size: 11px; }
</style>
