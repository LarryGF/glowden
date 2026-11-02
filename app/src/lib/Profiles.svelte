<script lang="ts">
  import { onMount } from "svelte";
  import { api, type ProfilesInfo } from "./api";

  let { onerror }: { onerror: (e: unknown) => void } = $props();

  let info = $state<ProfilesInfo | null>(null);
  let newName = $state("");
  let editing = $state<string | null>(null);
  let rename = $state("");
  let typedApp = $state("");
  let importPath = $state("");
  let message = $state("");

  async function run(p: Promise<ProfilesInfo>) {
    try {
      info = await p;
    } catch (e) {
      onerror(e);
    }
  }

  onMount(() => {
    run(api.profiles());
    // The focused app and the recent list change as you switch windows.
    const timer = setInterval(() => api.profiles().then((p) => (info = p)).catch(() => {}), 2000);
    return () => clearInterval(timer);
  });

  async function create() {
    const name = newName.trim();
    if (!name) return;
    await run(api.saveProfile(name, []));
    newName = "";
    editing = name;
    rename = name;
  }

  function apps(name: string) {
    return info?.profiles.find((p) => p.name === name)?.apps ?? [];
  }

  function setApps(name: string, list: string[]) {
    run(api.saveProfile(name, [...new Set(list)]));
  }

  async function exportProfile(name: string) {
    try {
      message = `Saved to ${await api.exportProfile(name)}`;
    } catch (e) {
      onerror(e);
    }
  }
</script>

{#if info}
  <div class="page">
    <section class="card">
      <div class="head">
        <h2>Profiles</h2>
        <label class="row">
          <span>Switch with the focused app</span>
          <button class="switch" class:on={info.auto} onclick={() => run(api.setProfileAuto(!info!.auto))}>{info.auto ? "On" : "Off"}</button>
        </label>
      </div>
      <p class="note">
        A profile changes lighting, brightness, DPI, report rate and button assignments while it's
        active; anything it doesn't change comes from the default profile. While a profile is
        active, changes you make on device pages are saved into it. Blue VO!CE stays the same in
        every profile.
        {#if info.focused}Focused now: <code>{info.focused}</code>.{/if}
      </p>

      <ul class="profiles">
        <li class:active={!info.active}>
          <div class="title">
            <strong>Default</strong>
            {#if !info.active}<span class="badge">active</span>{/if}
          </div>
          <button onclick={() => run(api.setActiveProfile(null))} disabled={!info.active}>Activate</button>
        </li>
        {#each info.profiles as p (p.name)}
          <li class:active={info.active === p.name}>
            <div class="title">
              {#if editing === p.name}
                <input bind:value={rename} onkeydown={(e) => e.key === "Enter" && (run(api.saveProfile(rename, p.apps, p.name)), (editing = null))} />
              {:else}
                <strong>{p.name}</strong>
              {/if}
              {#if info.active === p.name}<span class="badge">active</span>{/if}
              <span class="dim">{p.devices.length ? `${p.devices.length} device(s) changed` : "no changes yet"}</span>
            </div>
            <div class="apps">
              {#each p.apps as a}
                <span class="chip">{a}<button class="x" onclick={() => setApps(p.name, p.apps.filter((x) => x !== a))} aria-label="Remove {a}">✕</button></span>
              {:else}
                <span class="dim">No apps linked</span>
              {/each}
            </div>
            <div class="actions">
              <button onclick={() => run(api.setActiveProfile(p.name))} disabled={info.active === p.name}>Activate</button>
              <button onclick={() => (editing === p.name ? (editing = null) : ((editing = p.name), (rename = p.name)))}>{editing === p.name ? "Done" : "Edit"}</button>
              <button onclick={() => exportProfile(p.name)}>Export</button>
              <button class="danger" onclick={() => confirm(`Delete ${p.name}?`) && run(api.deleteProfile(p.name))}>Delete</button>
            </div>
            {#if editing === p.name}
              <div class="link">
                <span class="dim">Link an app you used recently:</span>
                <div class="apps">
                  {#each info.recent.filter((r) => !apps(p.name).includes(r)) as r}
                    <button class="chip add" onclick={() => setApps(p.name, [...apps(p.name), r])}>+ {r}</button>
                  {:else}
                    <span class="dim">Switch to the app once, then come back here.</span>
                  {/each}
                </div>
                <div class="row">
                  <input placeholder="or type a window class" bind:value={typedApp} />
                  <button onclick={() => typedApp.trim() && (setApps(p.name, [...apps(p.name), typedApp.trim()]), (typedApp = ""))}>Add</button>
                </div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>

      <div class="row">
        <input placeholder="New profile name" bind:value={newName} onkeydown={(e) => e.key === "Enter" && create()} />
        <button onclick={create}>Create profile</button>
      </div>
      <div class="row">
        <input placeholder="/path/to/glowden-profile-….json" bind:value={importPath} />
        <button onclick={() => importPath.trim() && run(api.importProfile(importPath.trim()))}>Import</button>
      </div>
      {#if message}<p class="note">{message}</p>{/if}
    </section>
  </div>
{/if}

<style>
  .page { height: 100%; overflow: auto; padding: 28px 40px; max-width: 980px; margin: 0 auto; }
  .card { background: var(--bg-card); border-radius: 14px; padding: 22px 26px; display: flex; flex-direction: column; gap: 14px; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 16px; }
  h2 { margin: 0; font-size: 16px; letter-spacing: 0.04em; text-transform: uppercase; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  .profiles { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .profiles li { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border-radius: 10px; background: var(--bg); border: 1px solid transparent; }
  .profiles li.active { border-color: var(--accent); }
  .profiles li:first-child { flex-direction: row; justify-content: space-between; align-items: center; }
  .title { display: flex; align-items: center; gap: 10px; }
  .badge { font-size: 11px; color: var(--accent); text-transform: uppercase; letter-spacing: 0.06em; }
  .dim { color: var(--text-faint); font-size: 12px; }
  .apps { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { display: inline-flex; align-items: center; gap: 4px; font-family: monospace; font-size: 12px; padding: 3px 8px; border-radius: 999px; background: var(--bg-card); }
  .chip.add { border: 1px dashed var(--line); }
  .x { border: none; background: none; padding: 0 2px; color: var(--text-dim); font-size: 11px; }
  .actions, .row { display: flex; gap: 8px; align-items: center; }
  .row input { flex: 1; }
  .link { display: flex; flex-direction: column; gap: 8px; }
  .danger:hover { border-color: var(--danger); color: var(--danger); }
  .switch { min-width: 64px; border-radius: 999px; font-weight: 700; }
  .switch.on { background: var(--text); color: var(--bg); border-color: transparent; }
  .row > span { color: var(--text-dim); font-size: 13px; }
</style>
