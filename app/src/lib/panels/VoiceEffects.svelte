<script lang="ts">
  import { api, type Device, type EffectSettings, type VoiceState } from "../api";

  let {
    device,
    state,
    onstate,
    onerror,
    onplay,
  }: {
    device: Device;
    state: VoiceState;
    onstate: (s: VoiceState) => void;
    onerror: (e: unknown) => void;
    onplay: (effect: string) => void;
  } = $props();

  type Control =
    | { field: string; label: string; min: number; max: number; step: number; unit: string }
    | { field: string; label: string; modes: string };
  type Stage = { stage: string; title: string; enable?: string; controls: Control[] };

  // G HUB's effect controls and ranges (its app bundle); "enable" is the stage's on switch.
  const STAGES: Stage[] = [
    {
      stage: "vocalChanger",
      title: "Voice",
      controls: [
        { field: "vocalMode1", label: "Primary voice", modes: "vocalMode" },
        { field: "voiceModFactor1", label: "Primary amount", min: -2400, max: 2400, step: 50, unit: "cents" },
        { field: "voiceGain1", label: "Primary level", min: -60, max: 0, step: 1, unit: "dB" },
        { field: "vocalMode2", label: "Secondary voice", modes: "vocalMode" },
        { field: "voiceModFactor2", label: "Secondary amount", min: -2400, max: 2400, step: 50, unit: "cents" },
        { field: "voiceGain2", label: "Secondary level", min: -60, max: 0, step: 1, unit: "dB" },
      ],
    },
    {
      stage: "vocalChanger",
      title: "Chorus",
      enable: "chorusEnabled",
      controls: [
        { field: "chorusDelay", label: "Delay", min: 0, max: 100, step: 1, unit: "ms" },
        { field: "chorusDelayMod", label: "Delay swing", min: 0, max: 100, step: 1, unit: "ms" },
        { field: "chorusDelayModRate", label: "Delay swing rate", min: 0, max: 2, step: 0.05, unit: "Hz" },
        { field: "chorusPitchMod", label: "Pitch swing", min: -600, max: 600, step: 10, unit: "cents" },
        { field: "chorusPitchModRate", label: "Pitch swing rate", min: 0, max: 2, step: 0.05, unit: "Hz" },
      ],
    },
    {
      stage: "flangerPhaser",
      title: "Flanger / phaser",
      enable: "enabled",
      controls: [
        { field: "phaserMode", label: "Mode", modes: "phaserMode" },
        { field: "depth", label: "Depth", min: 0, max: 1, step: 0.05, unit: "" },
        { field: "rate", label: "Rate", min: 0, max: 5, step: 0.1, unit: "Hz" },
      ],
    },
    {
      stage: "ringModulation",
      title: "Ring modulator",
      enable: "enabled",
      controls: [
        { field: "shapeMode", label: "Shape", modes: "shapeMode" },
        { field: "carrierFrequency", label: "Frequency", min: 1, max: 2000, step: 10, unit: "Hz" },
        { field: "mix", label: "Mix", min: 0, max: 1, step: 0.05, unit: "" },
      ],
    },
    {
      stage: "reverb",
      title: "Reverb",
      enable: "enabled",
      controls: [
        { field: "reflectionMode", label: "Reflections", modes: "reflectionMode" },
        { field: "mix", label: "Mix", min: 0, max: 1, step: 0.05, unit: "" },
        { field: "reflectionLowPassFrequency", label: "Reflection low-pass", min: 10, max: 10000, step: 20, unit: "Hz" },
        { field: "earlyLateMix", label: "Early / late", min: 0, max: 1, step: 0.05, unit: "" },
        { field: "roomSize", label: "Room size", min: 0.01, max: 1, step: 0.05, unit: "" },
      ],
    },
    {
      stage: "delay",
      title: "Echo",
      enable: "enabled",
      controls: [
        { field: "inputMode", label: "Input", modes: "inputMode" },
        { field: "feedbackMode", label: "Feedback routing", modes: "feedbackMode" },
        { field: "feedbackTimbre", label: "Feedback tone", modes: "feedbackTimbre" },
        { field: "mix", label: "Mix", min: 0, max: 1, step: 0.05, unit: "" },
        { field: "feedback", label: "Feedback", min: 0, max: 0.6, step: 0.05, unit: "" },
        { field: "leftDelay", label: "Left delay", min: 10, max: 2000, step: 10, unit: "ms" },
        { field: "rightDelay", label: "Right delay", min: 10, max: 2000, step: 10, unit: "ms" },
      ],
    },
  ];

  /** "VOCAL_MODE_FORMANT_WARP" → "Formant warp". */
  function modeLabel(mode: string): string {
    const words = mode.replace(/^(VOCAL|PHASER|REFLECTION|SHAPE|INPUT|FEEDBACK)_(MODE_|TIMBRE_)?/, "").replace(/_/g, " ").toLowerCase();
    const fixed: Record<string, string> = { "st flutter": "stereo flutter", noice: "noise", whispez: "whisper", halfwaverectifier: "half-wave", "saw tooth": "sawtooth", largediff: "large diffuse", midmix: "mid mix" };
    const w = fixed[words] ?? words;
    return w.charAt(0).toUpperCase() + w.slice(1);
  }

  const on = (s: EffectSettings, stage: string, key: string) => s[stage]?.[key] === true;
  const val = (s: EffectSettings | null | undefined, stage: string, field: string) => s?.[stage]?.[field];

  /** What a preset turns on, from its settings. */
  function chips(s: EffectSettings): string[] {
    const out: string[] = [];
    const v = s.vocalChanger ?? {};
    for (const [mode, factor] of [["vocalMode1", "voiceModFactor1"], ["vocalMode2", "voiceModFactor2"]]) {
      const m = v[mode] as string | undefined;
      if (!m || m === "VOCAL_MODE_OFF") continue;
      const f = Number(v[factor] ?? 0);
      const semis = Math.round(f / 10) / 10;
      out.push(m.includes("PITCH") || m.includes("FORMANT") ? `${modeLabel(m)} ${semis > 0 ? "+" : ""}${semis} st` : modeLabel(m));
    }
    if (on(s, "vocalChanger", "chorusEnabled")) out.push("Chorus");
    if (on(s, "flangerPhaser", "enabled")) out.push(modeLabel(String(s.flangerPhaser.phaserMode ?? "PHASER_MODE_FLANGER_UP")));
    if (on(s, "ringModulation", "enabled")) out.push(`Ring mod ${s.ringModulation.carrierFrequency ?? 0} Hz`);
    if (on(s, "reverb", "enabled")) out.push("Reverb");
    if (on(s, "delay", "enabled")) out.push("Echo");
    return out;
  }

  async function choose(effect: string) {
    try {
      onstate(await api.setVoiceEffect(device.id, effect === "Off" ? null : effect));
    } catch (e) {
      onerror(e);
    }
  }

  let advanced = $state(false);
  let pending: Record<string, number | boolean | string> = {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  function tune(path: string, v: number | boolean | string) {
    pending = { ...state.effect_overrides, ...pending, [path]: v };
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const send = pending;
      pending = {};
      try {
        onstate(await api.setVoiceEffectOverrides(device.id, send));
      } catch (e) {
        onerror(e);
      }
    }, 250);
  }
  async function reset() {
    pending = {};
    try {
      onstate(await api.setVoiceEffectOverrides(device.id, {}));
    } catch (e) {
      onerror(e);
    }
  }

  const fmt = (v: number, unit: string) => `${Math.round(v * 100) / 100}${unit ? ` ${unit}` : ""}`;
</script>

<section>
  <h3>Voice effects</h3>
  {#if state.effects.length === 0}
    <p class="note">Import G HUB's voice effects with <code>lhub import-voice-presets settings.json</code> (or the G HUB folder scan in Settings).</p>
  {:else}
    <div class="effects">
      {#each state.effects as e (e.name)}
        <button class="effect" class:active={state.effect === e.name} onclick={() => choose(e.name)}>
          <span class="name">{e.name}</span>
          {#if e.name !== "Off"}
            <span class="chips">{#each chips(e.settings) as c}<span>{c}</span>{/each}</span>
          {/if}
        </button>
      {/each}
    </div>

    {#if state.effect !== "Off" && state.effect_settings}
      {@const s = state.effect_settings}
      <button class="disclose" onclick={() => (advanced = !advanced)} aria-expanded={advanced}>
        {advanced ? "▾" : "▸"} Adjust {state.effect}{Object.keys(state.effect_overrides).length ? " (tuned)" : ""}
      </button>
      {#if advanced}
        {#each STAGES as st}
          {@const enabled = !st.enable || val(s, st.stage, st.enable) === true}
          <div class="group">
            <div class="group-head">
              <span>{st.title}</span>
              {#if st.enable}
                <button class="switch small" class:on={enabled} onclick={() => tune(`${st.stage}.${st.enable}`, !enabled)}>{enabled ? "On" : "Off"}</button>
              {/if}
            </div>
            {#if enabled}
              {#each st.controls as c}
                {@const v = val(s, st.stage, c.field)}
                {#if "modes" in c}
                  <label class="slider">
                    <span>{c.label}</span>
                    <select value={String(v ?? state.effect_modes[c.modes][0])} onchange={(e) => tune(`${st.stage}.${c.field}`, (e.target as HTMLSelectElement).value)}>
                      {#each state.effect_modes[c.modes] as m}<option value={m}>{modeLabel(m)}</option>{/each}
                    </select>
                    <em></em>
                  </label>
                {:else}
                  {@const n = Number(v ?? 0)}
                  <label class="slider">
                    <span>{c.label}</span>
                    <input type="range" min={c.min} max={c.max} step={c.step} value={n}
                      oninput={(e) => tune(`${st.stage}.${c.field}`, Number((e.target as HTMLInputElement).value))} />
                    <em>{fmt(n, c.unit)}</em>
                  </label>
                {/if}
              {/each}
            {/if}
          </div>
        {/each}
        <div class="buttons">
          <button onclick={reset} disabled={!Object.keys(state.effect_overrides).length}>Reset to {state.effect}</button>
          <button onclick={() => onplay(state.effect)} disabled={!state.has_test}>Play test with {state.effect}</button>
        </div>
      {/if}
    {/if}
    <p class="note">
      Effects run on the virtual microphone, after Blue VO!CE when it's on, and also work with
      VO!CE off. Like G HUB's, they are for fun and chat, not for a clean voice.
    </p>
  {/if}
</section>

<style>
  section { display: flex; flex-direction: column; gap: 12px; }
  h3 { margin: 0; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .effects { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 8px; }
  .effect { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; text-align: left; padding: 9px 11px; }
  .effect.active { border-color: var(--accent); background: var(--bg-hover); }
  .effect.active .name { color: var(--accent); }
  .name { font-weight: 600; font-size: 13px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chips span { font-size: 10px; padding: 1px 6px; border-radius: 999px; background: var(--bg); color: var(--text-dim); }
  .disclose { background: none; border: none; padding: 0; text-align: left; color: var(--text-dim); font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; font-size: 12px; }
  .group { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--bg); }
  .group-head { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 13px; }
  .slider { display: grid; grid-template-columns: 130px 1fr 80px; align-items: center; gap: 10px; font-size: 12px; color: var(--text-dim); }
  .slider em { font-style: normal; color: var(--text); text-align: right; }
  .switch { min-width: 48px; padding: 3px 10px; font-size: 12px; border-radius: 999px; font-weight: 700; }
  .switch.on { background: var(--text); color: var(--bg); border-color: transparent; }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  code { font-size: 11px; }
</style>
