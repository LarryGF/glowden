<script lang="ts">
  import { api, type Device, type EffectSettings, type VoiceState } from "../api";
  import PresetPicker from "./PresetPicker.svelte";

  // G HUB's EFFECTS tab: an effect preset and its advanced controls, which come from the
  // device's plugin (`effect_stages`).
  let {
    device,
    voice,
    onstate,
    onerror,
  }: { device: Device; voice: VoiceState; onstate: (s: VoiceState) => void; onerror: (e: unknown) => void } = $props();

  let busy = $state(false);
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

  const val = (s: EffectSettings | null | undefined, stage: string, field: string) => s?.[stage]?.[field];
  const notes = $derived(Object.fromEntries(Object.entries(voice.effect_details).map(([n, c]) => [n, c.join(" · ")])));
  const tuned = $derived(Object.keys(voice.effect_overrides).length > 0);
  const on = $derived(voice.effect !== "Off" && !!voice.effect_settings);

  let advanced = $state(false);
  let pending: Record<string, number | boolean | string> = {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  function tune(path: string, v: number | boolean | string) {
    pending = { ...voice.effect_overrides, ...pending, [path]: v };
    clearTimeout(timer);
    timer = setTimeout(() => {
      const send = pending;
      pending = {};
      run(api.setVoiceEffectOverrides(device.id, send));
    }, 250);
  }

  const fmt = (v: number, unit: string) => `${Math.round(v * 100) / 100}${unit ? ` ${unit}` : ""}`;
</script>

<div class="tab">
  {#if voice.effects.length === 0}
    <p class="note">The voice effects come from G HUB's settings: add the folder with <code>settings.db</code> in Settings → G HUB files.</p>
  {:else}
    <PresetPicker
      curated={voice.effects}
      custom={voice.custom_effects}
      curatedLabel="Effects"
      customLabel="Custom effects"
      selected={voice.effect}
      {tuned}
      {notes}
      disabled={busy}
      onselect={(name) => run(api.setVoiceEffect(device.id, name === "Off" ? null : name))}
      onduplicate={() => run(api.voicePresetAction(device.id, "effect", "duplicate"))}
      onrename={(name, to) => run(api.voicePresetAction(device.id, "effect", "rename", name, to))}
      ondelete={(name) => run(api.voicePresetAction(device.id, "effect", "delete", name))}
      onreset={() => run(api.setVoiceEffectOverrides(device.id, {}))}
    />

    {#if on}
      <label class="enable">
        <input type="checkbox" bind:checked={advanced} />
        <span>Advanced controls</span>
      </label>
      {#if advanced}
        {@const s = voice.effect_settings}
        {#each voice.effect_stages as st}
          {@const enabled = !st.enable || val(s, st.stage, st.enable) === true}
          <div class="group">
            <div class="group-head">
              <span>{st.title}</span>
              {#if st.enable}
                <input type="checkbox" checked={enabled} onchange={() => tune(`${st.stage}.${st.enable}`, !enabled)} aria-label="{st.title} on" />
              {/if}
            </div>
            {#if enabled}
              {#each st.controls as c}
                {@const v = val(s, st.stage, c.field)}
                {#if "modes" in c}
                  <label class="slider">
                    <span>{c.label}</span>
                    <select value={String(v ?? c.modes[0]?.value)} onchange={(e) => tune(`${st.stage}.${c.field}`, (e.target as HTMLSelectElement).value)}>
                      {#each c.modes as m}<option value={m.value}>{m.label}</option>{/each}
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
      {/if}
    {/if}
    <p class="note">
      Effects run on the virtual microphone, after Blue VO!CE when it's on, and also work with
      Blue VO!CE off.
    </p>
  {/if}
</div>

<style>
  .tab { display: flex; flex-direction: column; gap: 16px; }
  .enable { display: flex; align-items: center; gap: 10px; font-weight: 600; font-size: 13px; text-transform: uppercase; letter-spacing: 0.05em; cursor: pointer; }
  .enable input { width: 16px; height: 16px; accent-color: var(--accent); }
  .group { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--bg); }
  .group-head { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 13px; }
  .group-head input { accent-color: var(--accent); }
  .slider { display: grid; grid-template-columns: 110px 1fr 70px; align-items: center; gap: 10px; font-size: 12px; color: var(--text-dim); }
  .slider em { font-style: normal; color: var(--text); text-align: right; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  code { font-size: 11px; }
</style>
