<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppInfo } from "./api";

  let { onerror, onchanged }: { onerror: (e: unknown) => void; onchanged: () => void } = $props();

  let info = $state<AppInfo | null>(null);
  let newPath = $state("");
  let report = $state("");
  let busy = $state("");
  let backups = $state<string[]>([]);
  const when = (name: string) => new Date(Number(name) * 1000).toLocaleString();
  async function backup(p: Promise<string[]>) {
    try {
      backups = await p;
    } catch (e) {
      onerror(e);
    }
  }

  async function load() {
    try {
      info = await api.appSettings();
    } catch (e) {
      onerror(e);
    }
  }
  onMount(() => {
    load();
    backup(api.backups());
  });

  async function save(change: (s: AppInfo["settings"]) => void) {
    if (!info) return;
    const settings = structuredClone($state.snapshot(info.settings));
    change(settings);
    try {
      info = await api.setAppSettings(settings);
    } catch (e) {
      onerror(e);
    }
  }

  async function addPath() {
    const p = newPath.trim();
    if (!p || info?.settings.ghub_paths.includes(p)) return;
    await save((s) => s.ghub_paths.push(p));
    newPath = "";
  }

  async function scan(paths: string[]) {
    busy = "scan";
    report = "";
    for (const p of paths) {
      try {
        report += `${p}\n${await api.importGhub(p)}\n`;
      } catch (e) {
        report += `${p}\n${String(e)}\n`;
      }
    }
    busy = "";
    onchanged();
  }

  async function createDriver(id: string) {
    busy = id;
    try {
      const file = await api.createDriver(id);
      report = `Created ${file}. Edit it to add pictures, LEDs and buttons (docs/adding-devices.md).`;
      setTimeout(() => {
        load();
        onchanged();
      }, 4000);
    } catch (e) {
      onerror(e);
    } finally {
      busy = "";
    }
  }

  async function autostart(on: boolean) {
    try {
      const now = await api.setAutostart(on);
      if (info) info.autostart = now;
    } catch (e) {
      onerror(e);
    }
  }
</script>

{#if info}
  <div class="page">
    <section class="card">
      <h2>G HUB data</h2>
      <p class="note">
        Folders copied from a Windows G HUB install. Scanning imports device pictures and LED
        layouts, the Blue VO!CE engine and its presets. Useful folders:
        <code>C:\ProgramData\LGHUB\depots</code> and <code>%LOCALAPPDATA%\LGHUB</code> (for
        <code>settings.db</code>).
      </p>
      <ul class="paths">
        {#each info.settings.ghub_paths as p}
          <li>
            <span class="path">{p}</span>
            <button onclick={() => scan([p])} disabled={!!busy}>Scan</button>
            <button class="ghost" onclick={() => save((s) => (s.ghub_paths = s.ghub_paths.filter((x) => x !== p)))}>Remove</button>
          </li>
        {:else}
          <li class="empty">No folders yet.</li>
        {/each}
      </ul>
      <div class="add">
        <input placeholder="/path/to/LGHUB/depots" bind:value={newPath} onkeydown={(e) => e.key === "Enter" && addPath()} />
        <button onclick={addPath}>Add folder</button>
        <button onclick={() => scan(info!.settings.ghub_paths)} disabled={!!busy || info.settings.ghub_paths.length === 0}>
          {busy === "scan" ? "Scanning…" : "Scan all"}
        </button>
      </div>
      {#if report}<pre>{report}</pre>{/if}
    </section>

    <section class="card">
      <h2>Devices</h2>
      <p class="note">
        Each device works with what it reports over HID++; a definition file adds its picture,
        LED layout, buttons and extra effects.
      </p>
      <table>
        <thead><tr><th>Device</th><th>Id</th><th>Definition</th></tr></thead>
        <tbody>
          {#each info.devices as d}
            <tr>
              <td>{d.name}</td>
              <td class="mono">{d.product}</td>
              <td>
                {#if d.driver}
                  <span class="mono file" title={d.driver}>{d.driver.split("/").pop()}</span>
                {:else}
                  <button onclick={() => createDriver(d.id)} disabled={!!busy}>{busy === d.id ? "Creating…" : "Create definition"}</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <details>
        <summary>Loaded definitions ({info.drivers.length})</summary>
        <ul class="drivers">
          {#each info.drivers as d}
            <li><strong>{d.name}</strong> <span class="mono">{d.products.join(", ")}</span><br /><span class="mono file">{d.file}</span></li>
          {/each}
        </ul>
        <p class="note">
          Definitions are read from <code>/usr/share/lhub/drivers</code>, the repo's
          <code>drivers/</code> and <code>~/.local/share/lhub/drivers</code> (yours win). Restart the
          service after editing one.
        </p>
      </details>
    </section>

    <section class="card">
      <h2>Backups</h2>
      <p class="note">A backup is a copy of all LHUB settings: devices, profiles and these app settings.</p>
      <ul class="paths">
        {#each backups as b}
          <li>
            <span class="path">{when(b)}</span>
            <button onclick={() => confirm(`Restore the settings from ${when(b)}? Current settings are replaced.`) && backup(api.restoreBackup(b)).then(() => (load(), onchanged()))}>Restore</button>
            <button class="ghost" onclick={() => backup(api.deleteBackup(b))}>Delete</button>
          </li>
        {:else}
          <li class="empty">No backups yet.</li>
        {/each}
      </ul>
      <div class="add"><button onclick={() => backup(api.saveBackup())}>Back up now</button></div>
    </section>

    <section class="card">
      <h2>App</h2>
      <label class="row">
        <span>Start the LHUB service when you log in</span>
        <button class="switch" class:on={info.autostart} onclick={() => autostart(!info!.autostart)}>{info.autostart ? "On" : "Off"}</button>
      </label>
      <label class="row">
        <span>Turn device lighting off while the screen is locked</span>
        <button class="switch" class:on={info.settings.lighting_off_when_locked} onclick={() => save((s) => (s.lighting_off_when_locked = !s.lighting_off_when_locked))}>
          {info.settings.lighting_off_when_locked ? "On" : "Off"}
        </button>
      </label>
      <label class="row">
        <span>Left-handed mouse (swap left and right buttons)</span>
        <button class="switch" class:on={info.settings.swap_mouse_buttons} onclick={() => save((s) => (s.swap_mouse_buttons = !s.swap_mouse_buttons))}>
          {info.settings.swap_mouse_buttons ? "On" : "Off"}
        </button>
      </label>
      <label class="row">
        <span>Breathe red while a device's battery is low (15 %)</span>
        <button class="switch" class:on={info.settings.low_battery_lighting} onclick={() => save((s) => (s.low_battery_lighting = !s.low_battery_lighting))}>
          {info.settings.low_battery_lighting ? "On" : "Off"}
        </button>
      </label>
      <label class="row">
        <span>Notify when a device's battery is low (15 %)</span>
        <button class="switch" class:on={info.settings.low_battery_alert} onclick={() => save((s) => (s.low_battery_alert = !s.low_battery_alert))}>
          {info.settings.low_battery_alert ? "On" : "Off"}
        </button>
      </label>
      <label class="row">
        <span>Notify when Caps Lock or Num Lock changes</span>
        <button class="switch" class:on={info.settings.lock_notifications} onclick={() => save((s) => (s.lock_notifications = !s.lock_notifications))}>
          {info.settings.lock_notifications ? "On" : "Off"}
        </button>
      </label>
    </section>
  </div>
{/if}

<style>
  .page { height: 100%; overflow: auto; padding: 28px 40px; display: flex; flex-direction: column; gap: 20px; max-width: 980px; margin: 0 auto; }
  .card { background: var(--bg-card); border-radius: 14px; padding: 22px 26px; display: flex; flex-direction: column; gap: 14px; }
  h2 { margin: 0; font-size: 16px; letter-spacing: 0.04em; text-transform: uppercase; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; line-height: 1.5; }
  .paths { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .paths li { display: flex; align-items: center; gap: 10px; }
  .paths .path { flex: 1; font-family: monospace; font-size: 12px; overflow: hidden; text-overflow: ellipsis; }
  .paths .empty { color: var(--text-dim); }
  .add { display: flex; gap: 10px; }
  .add input { flex: 1; }
  .ghost { background: none; }
  pre { margin: 0; max-height: 220px; overflow: auto; background: var(--bg); padding: 12px; border-radius: 8px; font-size: 12px; white-space: pre-wrap; }
  table { border-collapse: collapse; width: 100%; }
  th { text-align: left; font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.06em; padding: 6px 8px; }
  td { padding: 8px; border-top: 1px solid var(--line); }
  .mono { font-family: monospace; font-size: 12px; }
  .file { color: var(--text-dim); }
  .drivers { list-style: none; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  summary { cursor: pointer; color: var(--text-dim); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .switch { min-width: 64px; border-radius: 999px; font-weight: 700; }
  .switch.on { background: var(--text); color: var(--bg); border-color: transparent; }
</style>
