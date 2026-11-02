<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Device, type VoiceState } from "../api";

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

  async function apply(enabled: boolean, preset: string | null) {
    busy = true;
    try {
      state = await api.setVoice(device.id, enabled, preset);
    } catch (e) {
      onerror(e);
    } finally {
      busy = false;
    }
  }

  const ready = $derived(state?.engine && (state?.presets.length ?? 0) > 0);
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
            <button
              class:active={state.preset === name}
              disabled={busy}
              onclick={() => apply(state!.enabled, name)}>{name}</button
            >
          {/each}
        </div>
      </section>

      {#if state.enabled}
        <p class="status" class:ok={state.running}>
          {#if state.running}
            Running. Choose <strong>{device.name} (Blue VO!CE)</strong> as the microphone in your apps.
          {:else}
            Not running{state.error ? `: ${state.error}` : ""}.
          {/if}
        </p>
      {/if}
      <p class="note">
        A preset also sets the microphone's input gain, as in G HUB. Logitech's engine runs on this
        computer; the microphone keeps its normal input too.
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
  .presets { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; }
  .presets button { text-align: left; padding: 10px 12px; }
  .presets button.active { border-color: var(--accent); color: var(--accent); background: var(--bg-hover); }
  .status { margin: 0; color: var(--warn); }
  .status.ok { color: var(--accent); }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  code { font-size: 11px; }
</style>
