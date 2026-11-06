<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Action, type Device } from "../api";
  import { keyLabel, linuxKey, orderKeys } from "../keymap";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  let remaps = $state<Record<string, Action>>({});
  let from = $state<string | null>(null);
  let kind = $state<"keys" | "disabled" | "command">("keys");
  let keys = $state<string[]>([]);
  let command = $state("");
  let capturing = $state<"from" | "keys" | null>(null);
  let held = new Set<string>();

  const describe = (a: Action) =>
    a.type === "keys" ? a.keys.map(keyLabel).join(" + ") : a.type === "command" ? `Run: ${a.command}` : a.type === "disabled" ? "Does nothing" : a.type;

  async function load() {
    try {
      remaps = (await api.getConfig(device.id)).key_remaps ?? {};
    } catch (e) {
      onerror(e);
    }
  }
  onMount(load);
  onDestroy(stopCapture);

  function onKey(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    const k = linuxKey(e.code);
    if (!k) return;
    if (capturing === "from") {
      if (e.type === "keydown") {
        from = k;
        stopCapture();
      }
      return;
    }
    if (e.type === "keydown") {
      held.add(k);
      keys = orderKeys([...new Set([...keys, k])]);
    } else {
      held.delete(k);
      if (held.size === 0) stopCapture();
    }
  }
  function startCapture(what: "from" | "keys") {
    if (what === "keys") keys = [];
    capturing = what;
    held = new Set();
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);
  }
  function stopCapture() {
    capturing = null;
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("keyup", onKey, true);
  }

  function edit(k: string) {
    from = k;
    const a = remaps[k];
    kind = a.type === "command" || a.type === "disabled" ? a.type : "keys";
    keys = a.type === "keys" ? [...a.keys] : [];
    command = a.type === "command" ? a.command : "";
  }

  async function save() {
    if (!from) return;
    const action: Action = kind === "keys" ? { type: "keys", keys } : kind === "command" ? { type: "command", command } : { type: "disabled" };
    try {
      await api.setKeyRemap(device.id, from, action);
      from = null;
      await load();
    } catch (e) {
      onerror(e);
    }
  }
  async function remove(k: string) {
    try {
      await api.setKeyRemap(device.id, k, null);
      if (from === k) from = null;
      await load();
    } catch (e) {
      onerror(e);
    }
  }
</script>

<div class="panel">
  <p class="note">
    Experimental. While a key is remapped, GlowDen takes over this keyboard's typing keys and passes the others on.
    <strong>Left Ctrl + Right Ctrl + Esc</strong> releases the keyboard at once.
  </p>

  {#if Object.keys(remaps).length}
    <ul class="list">
      {#each Object.entries(remaps) as [k, a] (k)}
        <li>
          <button class="item" class:active={from === k} onclick={() => edit(k)}>
            <span class="name">{keyLabel(k)}</span><span class="what">{describe(a)}</span>
          </button>
          <button class="remove" onclick={() => remove(k)} title="Remove this remap" aria-label="Remove {keyLabel(k)} remap">✕</button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="editor">
    <div class="row">
      <span class="label">Key</span>
      <button class="capture" class:active={capturing === "from"} onclick={() => startCapture("from")}>
        {capturing === "from" ? "Press the key…" : from ? keyLabel(from) : "Click, then press a key"}
      </button>
    </div>
    {#if from}
      <div class="kinds">
        <button class:active={kind === "keys"} onclick={() => (kind = "keys")}>Key or shortcut</button>
        <button class:active={kind === "disabled"} onclick={() => (kind = "disabled")}>Disable</button>
        <button class:active={kind === "command"} onclick={() => (kind = "command")}>Run a program</button>
      </div>
      {#if kind === "keys"}
        <button class="capture" class:active={capturing === "keys"} onclick={() => startCapture("keys")}>
          {capturing === "keys" ? "Press the keys…" : keys.length ? keys.map(keyLabel).join(" + ") : "Click, then press the key or shortcut"}
        </button>
      {:else if kind === "command"}
        <input bind:value={command} placeholder="Command, e.g. konsole" />
      {/if}
      <div class="row">
        <button class="primary" onclick={save} disabled={(kind === "keys" && !keys.length) || (kind === "command" && !command.trim())}>Save</button>
        <button onclick={() => (from = null)}>Cancel</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 16px; max-width: 560px; }
  .note { margin: 0; color: var(--text-dim); font-size: 12px; line-height: 1.5; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .list li { display: flex; gap: 6px; }
  .item { flex: 1; display: flex; justify-content: space-between; gap: 12px; text-align: left; padding: 9px 12px; }
  .item.active { border-color: var(--accent); background: var(--bg-hover); }
  .name { font-weight: 600; }
  .what { color: var(--accent); font-size: 12px; }
  .editor { display: flex; flex-direction: column; gap: 12px; padding-top: 6px; border-top: 1px solid var(--line); }
  .row { display: flex; gap: 8px; align-items: center; }
  .label { width: 40px; color: var(--text-dim); font-size: 12px; }
  .kinds { display: flex; flex-wrap: wrap; gap: 6px; }
  .kinds button { padding: 6px 10px; font-size: 12px; }
  .kinds button.active, .capture.active { border-color: var(--accent); color: var(--accent); }
  .capture { flex: 1; text-align: left; }
</style>
