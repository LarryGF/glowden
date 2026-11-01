<script lang="ts">
  import { onMount } from "svelte";
  import { api, debounce, type Device } from "../api";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  let gain = $state<number | null>(null);
  let muted = $state(false);

  onMount(() => {
    api.getGain(device.id).then((g) => (gain = g)).catch(onerror);
  });

  const pushGain = debounce(() => gain !== null && api.setGain(device.id, gain).catch(onerror));

  function toggleMute() {
    muted = !muted;
    api.setMuted(device.id, muted).catch(onerror);
  }
</script>

<div class="panel">

  <div class="gain">
    <div class="dial" style:--g={gain ?? 0}>
      <span>{gain ?? "–"}</span>
    </div>
    <label class="row">
      <span class="label">Hardware gain</span>
      <input type="range" min="0" max="100" disabled={gain === null} bind:value={gain} oninput={pushGain} />
    </label>
  </div>

  <div class="mute">
    <button class:on={muted} onclick={toggleMute}>{muted ? "Muted" : "Mute"}</button>
    <span class="hint">Mute state isn't read back from the mic yet, so this shows only what you set here.</span>
  </div>
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 28px; max-width: 560px; }
  h2 { margin: 0; font-size: 20px; }
  .gain { display: flex; align-items: center; gap: 28px; }
  .dial {
    flex: none;
    width: 120px;
    height: 120px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: conic-gradient(var(--accent) calc(var(--g) * 1%), var(--bg-card) 0);
    position: relative;
  }
  .dial::after { content: ""; position: absolute; inset: 10px; border-radius: 50%; background: var(--bg); }
  .dial span { position: relative; z-index: 1; font-size: 28px; font-weight: 600; }
  .row { flex: 1; display: flex; flex-direction: column; gap: 8px; }
  .mute { display: flex; align-items: center; gap: 14px; }
  .mute button.on { background: var(--danger); color: #2a0806; border-color: transparent; font-weight: 600; }
  .hint { color: var(--text-faint); font-size: 12px; }
</style>
