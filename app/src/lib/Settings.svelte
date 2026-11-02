<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type AppInfo, type GhubSummary, type Source } from "./api";

  let { onerror, onchanged }: { onerror: (e: unknown) => void; onchanged: () => void } = $props();

  let info = $state<AppInfo | null>(null);
  let report = $state("");
  let found = $state<GhubSummary | null>(null);
  let sources = $state<Source[]>([]);
  let newSource = $state("");
  let sourceErrors = $state<string[]>([]);

  async function sourcesDo(p: Promise<Source[]>, after?: () => void) {
    try {
      sources = await p;
      after?.();
      onchanged();
    } catch (e) {
      onerror(e);
    }
  }
  async function refreshSources() {
    busy = "sources";
    try {
      const r = await api.refreshSources();
      sources = r.sources;
      sourceErrors = r.errors;
    } catch (e) {
      onerror(e);
    }
    busy = "";
  }
  function trust(s: Source) {
    if (!s.trusted && !confirm(`Allow plugins from ${s.name ?? s.url}?\n\nPlugins are programs. They run on this computer with your user's rights, for devices that need them (e.g. Blue VO!CE). Only allow sources you trust.`)) return;
    sourcesDo(api.trustSource(s.url, !s.trusted));
  }
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
    api.scanGhub(false).then((s) => (found = s)).catch(() => {});
    api.sources().then((s) => {
      sources = s;
      if (s.some((x) => !x.fetched)) refreshSources();
    }).catch(() => {});
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
    const p = await open({ directory: true, title: "Choose a folder with G HUB files" }).catch(onerror);
    if (typeof p !== "string" || info?.settings.ghub_paths.includes(p)) return;
    await save((s) => s.ghub_paths.push(p));
    await scan();
  }

  async function scan() {
    busy = "scan";
    try {
      found = await api.scanGhub();
    } catch (e) {
      onerror(e);
    }
    busy = "";
    onchanged();
  }

  async function createDriver(id: string) {
    busy = id;
    try {
      const file = await api.createDriver(id);
      report = `Created ${file}. Edit it to add pictures, LEDs and buttons (ADDING-DEVICES.md in glowden-devices).`;
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
      <h2>G HUB files</h2>
      <p class="note">
        GlowDen can use the device pictures, LED layouts, sound clips, Blue VO!CE engine and
        presets of a G HUB installation you have, for example on a dual-boot Windows drive
        (mount it, then choose the drive or its <code>ProgramData\LGHUB</code> folder). Any
        folder holding those files works. GlowDen reads them where they are and never copies
        them; what you point it at, and under which terms you got it, is up to you.
      </p>
      <ul class="paths">
        {#each info.settings.ghub_paths as p}
          <li>
            <span class="path">{p}</span>
            <button class="ghost" onclick={() => save((s) => (s.ghub_paths = s.ghub_paths.filter((x) => x !== p)))}>Remove</button>
          </li>
        {:else}
          <li class="empty">No folders yet.</li>
        {/each}
      </ul>
      <div class="add">
        <button onclick={addPath} disabled={!!busy}>Add folder…</button>
        <button onclick={scan} disabled={!!busy || info.settings.ghub_paths.length === 0}>
          {busy === "scan" ? "Scanning…" : "Scan again"}
        </button>
      </div>
      {#if found}
        <ul class="found">
          {#each found.devices as d}<li class:ok={d.found}>{d.found ? "✓" : "–"} {d.name}: picture and LED layout</li>{/each}
          <li class:ok={found.sounds > 0}>{found.sounds > 0 ? "✓" : "–"} {found.sounds} sound clips</li>
          <li class:ok={found.voice_presets > 0}>{found.voice_presets > 0 ? "✓" : "–"} {found.voice_presets} Blue VO!CE presets, {found.voice_effects} voice effects{found.settings ? ` (${found.settings.split("/").pop()})` : ""}</li>
          <li class:ok={!!found.engine}>{found.engine ? "✓" : "–"} Blue VO!CE engine{found.engine ? "" : " (in Windows' DriverStore once G HUB's audio driver is installed)"}</li>
        </ul>
      {/if}
      {#if report}<pre>{report}</pre>{/if}
    </section>

    <section class="card">
      <div class="head">
        <h2>Device sources</h2>
        <button onclick={refreshSources} disabled={!!busy}>{busy === "sources" ? "Checking…" : "Check for updates"}</button>
      </div>
      <p class="note">
        Device definitions (what GlowDen knows about each device) come from these sources;
        a device that connects without one is looked up here automatically. Definitions are
        data. Some devices also need a plugin (a program, e.g. for Blue VO!CE), which runs
        only once you allow that source's plugins.
      </p>
      {#each sources as s (s.url)}
        <div class="source">
          <div class="source-head">
            <div>
              <strong>{s.name ?? s.url}</strong>
              {#if s.builtin}<span class="badge">built in</span>{/if}
              {#if s.version}<span class="dim">v{s.version}</span>{/if}
              <div class="dim url">{s.url}</div>
              {#if !s.fetched}<div class="dim">Not reachable yet.</div>{/if}
            </div>
            <div class="actions">
              {#if s.plugins.length}
                <button class="switch small" class:on={s.trusted} onclick={() => trust(s)} title="Let this source's plugins run">
                  {s.trusted ? "Plugins allowed" : "Allow plugins"}
                </button>
              {/if}
              {#if !s.builtin}
                <button class="ghost" onclick={() => confirm(`Remove ${s.name ?? s.url} and its installed definitions?`) && sourcesDo(api.removeSource(s.url))}>Remove</button>
              {/if}
            </div>
          </div>
          {#if s.devices.length}
            <table>
              <tbody>
                {#each s.devices as d (d.id)}
                  <tr>
                    <td>{d.name}{#if d.plugins.length}<span class="dim"> · plugin: {d.plugins.join(", ")}</span>{/if}</td>
                    <td class="kind">{d.kind ?? ""}</td>
                    <td class="act">
                      {#if d.installed}
                        <button class="ghost" onclick={() => sourcesDo(api.uninstallDevice(s.url, d.id))}>Remove</button>
                      {:else}
                        <button onclick={() => sourcesDo(api.installDevice(s.url, d.id))}>Install</button>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>
      {/each}
      {#each sourceErrors as e}<p class="warn">{e}</p>{/each}
      <div class="add">
        <input placeholder="https://…/index.json or a local folder" bind:value={newSource}
          onkeydown={(e) => e.key === "Enter" && newSource.trim() && sourcesDo(api.addSource(newSource.trim()), () => (newSource = ""))} />
        <button disabled={!newSource.trim()} onclick={() => sourcesDo(api.addSource(newSource.trim()), () => (newSource = ""))}>Add source</button>
      </div>
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
          Definitions come from the device sources above, plus your own in
          <code>~/.local/share/glowden/drivers</code> (yours win). Restart the service after
          editing one.
        </p>
      </details>
    </section>

    <section class="card">
      <h2>Backups</h2>
      <p class="note">A backup is a copy of all GlowDen settings: devices, profiles and these app settings.</p>
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
        <span>Start the GlowDen service when you log in</span>
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
  .head { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .source { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border-radius: 10px; background: var(--bg); }
  .source-head { display: flex; justify-content: space-between; gap: 12px; align-items: flex-start; }
  .source .actions { display: flex; gap: 6px; align-items: center; }
  .badge { font-size: 11px; color: var(--accent); text-transform: uppercase; letter-spacing: 0.06em; margin-left: 6px; }
  .dim { color: var(--text-faint); font-size: 12px; }
  .url { font-family: monospace; font-size: 11px; word-break: break-all; }
  .source td.kind { color: var(--text-faint); font-size: 12px; }
  .source td.act { text-align: right; }
  .warn { margin: 0; color: var(--warn); font-size: 12px; }
  .found { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: var(--text-faint); }
  .found .ok { color: var(--text); }
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
