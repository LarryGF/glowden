<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Device } from "./lib/api";
  import DevicePage from "./lib/DevicePage.svelte";
  import Home from "./lib/Home.svelte";

  let devices = $state<Device[]>([]);
  let loading = $state(true);
  let selected = $state<Device | null>(null);
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

  onMount(refresh);
</script>

<div class="shell">
  <header>
    {#if selected}
      <button class="back" onclick={() => (selected = null)} aria-label="Back to devices">←</button>
      <span class="title">{selected.name}</span>
    {:else}
      <span class="brand"><span class="mark"></span>lhub</span>
      <button class="refresh" onclick={refresh} disabled={loading}>{loading ? "Scanning…" : "Rescan"}</button>
    {/if}
  </header>

  <div class="body">
    {#if selected}
      {#key selected.id}
        <DevicePage device={selected} onerror={showError} />
      {/key}
    {:else}
      <Home {devices} {loading} onopen={(d) => (selected = d)} />
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
