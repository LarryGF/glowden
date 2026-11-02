<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Device } from "./lib/api";
  import DevicePage from "./lib/DevicePage.svelte";
  import Home from "./lib/Home.svelte";
  import Settings from "./lib/Settings.svelte";
  import Profiles from "./lib/Profiles.svelte";

  let devices = $state<Device[]>([]);
  let loading = $state(true);
  let selectedId = $state<string | null>(null);
  let settings = $state(false);
  let profilesPage = $state(false);
  let activeProfile = $state<string | null>(null);
  const pollProfile = () => api.profiles().then((p) => (activeProfile = p.active)).catch(() => {});
  // Looked up in the latest list so battery and charging stay current on the device page.
  const selected = $derived(devices.find((d) => d.id === selectedId) ?? null);
  let error = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  function showError(e: unknown) {
    error = String(e);
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (error = null), 6000);
  }

  async function refresh() {
    loading = true;
    try {
      devices = await api.listDevices();
    } catch (e) {
      showError(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
    // Quiet background refresh: battery level, charging, devices coming and going.
    const timer = setInterval(() => api.listDevices().then((d) => (devices = d)).catch(() => {}), 15000);
    // The active profile follows the focused app.
    pollProfile();
    const profileTimer = setInterval(pollProfile, 2000);
    return () => {
      clearInterval(timer);
      clearInterval(profileTimer);
    };
  });
</script>

<div class="shell">
  <header>
    {#if selected || settings || profilesPage}
      <button class="back" onclick={() => ((selectedId = null), (settings = false), (profilesPage = false))} aria-label="Back to devices">←</button>
      <span class="title">{settings ? "Settings" : profilesPage ? "Profiles" : selected?.name}</span>
      {#if selected}
        <button class="profile" onclick={() => ((selectedId = null), (profilesPage = true))} title="Changes on this page are saved to this profile">
          Profile: {activeProfile ?? "Default"}
        </button>
      {/if}
    {:else}
      <span class="brand"><span class="mark"></span>lhub</span>
      <button class="profile" onclick={() => (profilesPage = true)}>Profile: {activeProfile ?? "Default"}</button>
      <button class="refresh" onclick={refresh} disabled={loading}>{loading ? "Scanning…" : "Rescan"}</button>
      <button class="gear" onclick={() => (settings = true)} aria-label="Settings" title="Settings">
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
        </svg>
      </button>
    {/if}
  </header>

  <div class="body">
    {#if settings}
      <Settings onerror={showError} onchanged={refresh} />
    {:else if profilesPage}
      <Profiles onerror={showError} />
    {:else if selected}
      {#key selected.id}
        <DevicePage device={selected} onerror={showError} />
      {/key}
    {:else}
      <Home {devices} {loading} onopen={(d) => (selectedId = d.id)} />
    {/if}
  </div>

  {#if error}
    <div class="toast" role="alert">{error}</div>
  {/if}
</div>

<style>
  .shell { display: grid; grid-template-rows: 56px 1fr; height: 100%; }
  header { display: flex; align-items: center; gap: 14px; padding: 0 20px; border-bottom: 1px solid var(--line); background: var(--bg-raised); }
  .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 16px; letter-spacing: 0.02em; }
  .mark { width: 18px; height: 18px; border-radius: 50%; border: 4px solid var(--accent); }
  .refresh { margin-left: auto; }
  .profile { border-radius: 999px; padding: 5px 12px; font-size: 12px; font-weight: 600; }
  header .back ~ .profile { margin-left: auto; }
  .gear { border: none; background: none; padding: 6px; display: grid; place-items: center; color: var(--text-dim); }
  .gear:hover { color: var(--text); }
  .back { border: none; background: none; font-size: 20px; padding: 4px 10px; }
  .title { font-weight: 600; }
  .body { min-height: 0; overflow: auto; }
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: #3a1716;
    color: #ffd6d4;
    border: 1px solid #6b2522;
    padding: 10px 16px;
    border-radius: 8px;
    max-width: 70vw;
  }
</style>
