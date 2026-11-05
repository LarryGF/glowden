<script lang="ts">
  import { onMount } from "svelte";
  import { api, debounce, type Device, type VoiceState } from "../api";
  import { page } from "../visibility.svelte";
  import MicTest from "./MicTest.svelte";
  import Voice from "./Voice.svelte";
  import VoiceEffects from "./VoiceEffects.svelte";

  // G HUB's microphone panel: INPUT, Blue VO!CE and EFFECTS tabs, with the mic test under the
  // Blue VO!CE and EFFECTS tabs. Devices without Blue VO!CE show the input settings only.
  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  const hasVoice = $derived(device.caps.includes("blue_voice"));
  let tab = $state<"input" | "voice" | "effects">("input");
  let voice = $state<VoiceState | null>(null);
  let gain = $state<number | null>(null);
  let muted = $state(false);

  onMount(() => {
    api.getGain(device.id).then((g) => (gain = g)).catch(onerror);
    api.getMuted(device.id).then((m) => (muted = m)).catch(onerror);
    if (!hasVoice) return;
    tab = "voice";
    const load = () => api.voiceState(device.id).then((s) => (voice = s)).catch(onerror);
    load();
    // Picks up a plugin that stopped on its own.
    const timer = setInterval(() => page.visible && load(), 3000);
    return () => {
      clearInterval(timer);
      // Leaving the page ends the mic test, as closing G HUB's panel does.
      if (voice?.previewing) api.voicePreview(device.id, false).catch(() => {});
    };
  });

  const pushGain = debounce(() => gain !== null && api.setGain(device.id, gain).catch(onerror));
  function toggleMute() {
    muted = !muted;
    api.setMuted(device.id, muted).catch(onerror);
  }
  const setVoice = (s: VoiceState) => (voice = s);
</script>

<div class="panel">
  {#if hasVoice}
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === "input"} class:active={tab === "input"} onclick={() => (tab = "input")}>Input</button>
      <button role="tab" aria-selected={tab === "voice"} class:active={tab === "voice"} onclick={() => (tab = "voice")}>Blue VO!CE</button>
      <button role="tab" aria-selected={tab === "effects"} class:active={tab === "effects"} onclick={() => (tab = "effects")}>Effects</button>
    </div>
  {/if}

  {#if tab === "input"}
    <div class="gain">
      <div class="dial" style:--g={gain ?? 0}><span>{gain ?? "–"}</span></div>
      <label class="row">
        <span class="label">Mic level (input gain)</span>
        <input type="range" min="0" max="100" disabled={gain === null} bind:value={gain} oninput={pushGain} />
      </label>
    </div>
    <div class="mute">
      <button class:on={muted} onclick={toggleMute}>{muted ? "Muted" : "Mute"}</button>
    </div>
  {:else if voice}
    {#if tab === "voice"}
      <Voice {device} {voice} onstate={setVoice} {onerror} />
    {:else}
      <VoiceEffects {device} {voice} onstate={setVoice} {onerror} />
    {/if}
    {#if voice.engine}
      <MicTest {device} {voice} onstate={setVoice} {onerror} />
    {/if}
  {/if}
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 22px; }
  .tabs { display: flex; border-bottom: 1px solid var(--line); }
  .tabs button { flex: 1; border: none; border-radius: 0; background: none; padding: 10px 6px; font-weight: 600; font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-dim); border-bottom: 2px solid transparent; }
  .tabs button.active { color: var(--text); border-bottom-color: var(--accent); }
  .gain { display: flex; align-items: center; gap: 28px; }
  .dial { flex: none; width: 120px; height: 120px; border-radius: 50%; display: grid; place-items: center; background: conic-gradient(var(--accent) calc(var(--g) * 1%), var(--bg-card) 0); position: relative; }
  .dial::after { content: ""; position: absolute; inset: 10px; border-radius: 50%; background: var(--bg); }
  .dial span { position: relative; z-index: 1; font-size: 28px; font-weight: 600; }
  .row { flex: 1; display: flex; flex-direction: column; gap: 8px; }
  .mute button.on { background: var(--danger); color: #2a0806; border-color: transparent; font-weight: 600; }
</style>
