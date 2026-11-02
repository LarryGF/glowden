<script lang="ts">
  import { api, type Device, type VoiceState } from "../api";

  // G HUB's mic test: record a sample once (without processing), then play it in a loop;
  // every change to Blue VO!CE or the effects applies to what you hear.
  let {
    device,
    voice,
    onstate,
    onerror,
  }: { device: Device; voice: VoiceState; onstate: (s: VoiceState) => void; onerror: (e: unknown) => void } = $props();

  const SECONDS = 8;
  let left = $state(0);

  async function record() {
    if (voice.previewing) onstate(await api.voicePreview(device.id, false).catch((e) => (onerror(e), voice)));
    left = SECONDS;
    const tick = setInterval(() => (left = Math.max(0, left - 0.1)), 100);
    try {
      await api.voiceTestRecord(device.id, SECONDS);
      onstate({ ...voice, has_test: true, previewing: false });
    } catch (e) {
      onerror(e);
    } finally {
      clearInterval(tick);
      left = 0;
    }
  }

  async function toggle() {
    try {
      onstate(await api.voicePreview(device.id, !voice.previewing));
    } catch (e) {
      onerror(e);
    }
  }

  const help = $derived(
    left > 0
      ? "Keep talking: a few sentences in your normal voice."
      : voice.previewing
        ? "You're hearing your sample with the current settings, in a loop. Change presets or controls and listen; record again any time."
        : voice.has_test
          ? "Press play to hear your sample in a loop. It was recorded without processing, so you don't need to record again when you change settings."
          : "Record a short sample of your voice to hear how you sound.",
  );
</script>

<section class="test">
  <h3>Mic test</h3>
  <p class="note">{help}</p>
  <div class="row">
    <button class="record" class:active={left > 0} onclick={record} disabled={left > 0}>
      {left > 0 ? `Recording… ${Math.ceil(left)} s` : voice.has_test ? "● Record again" : "● Record"}
    </button>
    <button class="play" class:on={voice.previewing} onclick={toggle} disabled={!voice.has_test || left > 0}>
      {voice.previewing ? "■ Stop" : "▶ Play"}
    </button>
  </div>
  {#if left > 0}
    <div class="progress"><span style:width="{100 - (left / SECONDS) * 100}%"></span></div>
  {/if}
  <p class="note faint">Use headphones, or the speakers will feed back into the microphone.</p>
</section>

<style>
  .test { display: flex; flex-direction: column; gap: 10px; padding: 14px; border-radius: 12px; background: var(--bg); }
  h3 { margin: 0; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .row { display: flex; gap: 8px; }
  .row button { flex: 1; font-weight: 600; }
  .record.active { border-color: var(--danger); color: var(--danger); }
  .play.on { background: var(--accent); color: #062a26; border-color: transparent; }
  .progress { height: 6px; border-radius: 3px; background: var(--bg-card); overflow: hidden; }
  .progress span { display: block; height: 100%; background: var(--danger); transition: width 0.1s linear; }
  .note { margin: 0; color: var(--text-dim); font-size: 12px; line-height: 1.5; }
  .note.faint { color: var(--text-faint); }
</style>
