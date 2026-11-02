<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type Action, type ButtonsInfo, type Device } from "../api";
  import { keyLabel, linuxKey, orderKeys } from "../keymap";

  let { device, onerror }: { device: Device; onerror: (e: unknown) => void } = $props();

  let info = $state<ButtonsInfo | null>(null);
  let selected = $state<number | null>(null);
  let detecting = $state(false);
  let detectStart = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  // Editor state for the selected button.
  let kind = $state<"default" | "system" | Action["type"]>("default");
  let keys = $state<string[]>([]);
  let capturing = $state(false);
  let button = $state("BTN_LEFT");
  let direction = $state("up");
  let dpi = $state("cycle");
  let command = $state("");
  let sound = $state("");
  let steps = $state<({ keys: string[] } | { ms: number })[]>([]);
  let special = $state("KEY_PLAYPAUSE");

  // Common desktop actions, as the shortcuts KDE Plasma and most apps use.
  const SYSTEM: { group: string; label: string; keys: string[] }[] = [
    { group: "Edit", label: "Copy", keys: ["KEY_LEFTCTRL", "KEY_C"] },
    { group: "Edit", label: "Paste", keys: ["KEY_LEFTCTRL", "KEY_V"] },
    { group: "Edit", label: "Cut", keys: ["KEY_LEFTCTRL", "KEY_X"] },
    { group: "Edit", label: "Undo", keys: ["KEY_LEFTCTRL", "KEY_Z"] },
    { group: "Edit", label: "Redo", keys: ["KEY_LEFTCTRL", "KEY_LEFTSHIFT", "KEY_Z"] },
    { group: "Edit", label: "Select all", keys: ["KEY_LEFTCTRL", "KEY_A"] },
    { group: "Media", label: "Play / pause", keys: ["KEY_PLAYPAUSE"] },
    { group: "Media", label: "Next track", keys: ["KEY_NEXTSONG"] },
    { group: "Media", label: "Previous track", keys: ["KEY_PREVIOUSSONG"] },
    { group: "Media", label: "Stop", keys: ["KEY_STOPCD"] },
    { group: "Media", label: "Volume up", keys: ["KEY_VOLUMEUP"] },
    { group: "Media", label: "Volume down", keys: ["KEY_VOLUMEDOWN"] },
    { group: "Media", label: "Mute", keys: ["KEY_MUTE"] },
    { group: "Media", label: "Microphone mute", keys: ["KEY_MICMUTE"] },
    { group: "Desktop", label: "Switch window", keys: ["KEY_LEFTALT", "KEY_TAB"] },
    { group: "Desktop", label: "Close window", keys: ["KEY_LEFTALT", "KEY_F4"] },
    { group: "Desktop", label: "Show desktop", keys: ["KEY_LEFTMETA", "KEY_D"] },
    { group: "Desktop", label: "Overview", keys: ["KEY_LEFTMETA", "KEY_W"] },
    { group: "Desktop", label: "Next virtual desktop", keys: ["KEY_LEFTCTRL", "KEY_LEFTMETA", "KEY_RIGHT"] },
    { group: "Desktop", label: "Previous virtual desktop", keys: ["KEY_LEFTCTRL", "KEY_LEFTMETA", "KEY_LEFT"] },
    { group: "Desktop", label: "Lock screen", keys: ["KEY_LEFTMETA", "KEY_L"] },
    { group: "Desktop", label: "Screenshot", keys: ["KEY_SYSRQ"] },
    { group: "Browser", label: "Back", keys: ["KEY_BACK"] },
    { group: "Browser", label: "Forward", keys: ["KEY_FORWARD"] },
    { group: "Browser", label: "New tab", keys: ["KEY_LEFTCTRL", "KEY_T"] },
    { group: "Browser", label: "Close tab", keys: ["KEY_LEFTCTRL", "KEY_W"] },
    { group: "Browser", label: "Reopen closed tab", keys: ["KEY_LEFTCTRL", "KEY_LEFTSHIFT", "KEY_T"] },
    { group: "Browser", label: "Zoom in", keys: ["KEY_LEFTCTRL", "KEY_EQUAL"] },
    { group: "Browser", label: "Zoom out", keys: ["KEY_LEFTCTRL", "KEY_MINUS"] },
    { group: "Apps", label: "Calculator", keys: ["KEY_CALC"] },
  ];
  let system = $state(0);
  const systemName = (k: string[]) => SYSTEM.find((x) => x.keys.join() === k.join())?.label;

  async function load() {
    try {
      info = await api.buttons(device.id);
    } catch (e) {
      onerror(e);
    }
  }

  onMount(() => {
    load();
    timer = setInterval(poll, 250);
  });
  onDestroy(() => {
    clearInterval(timer);
    stopCapture();
  });

  /** While detecting, the next mouse button pressed (after the click on "Detect") selects itself. */
  async function poll() {
    if (!detecting) return;
    const b = await api.buttons(device.id).catch(() => null);
    if (b?.last && Date.now() - b.last.ago_ms > detectStart) {
      detecting = false;
      info = b;
      select(b.last.button);
    }
  }

  function detect() {
    detecting = true;
    detectStart = Date.now();
  }

  const count = $derived(
    Math.max(11, ...Object.keys(info?.labels ?? {}).map(Number), ...Object.keys(info?.assignments ?? {}).map(Number)),
  );
  const label = (n: number) => info?.labels[String(n)] ?? `Button ${n}`;

  function describe(a: Action | undefined): string {
    if (!a) return "Default";
    switch (a.type) {
      case "keys": return systemName(a.keys) ?? a.keys.map(keyLabel).join(" + ");
      case "macro": return `Macro (${a.steps.length} steps)`;
      case "mouse_button": return keyLabel(a.button);
      case "scroll": return `Scroll ${a.direction}`;
      case "dpi": return `DPI ${a.action}`;
      case "command": return `Run: ${a.command}`;
      case "sound": return `Sound: ${a.file.split("/").pop()?.replace(/\.[^.]+$/, "")}`;
      case "disabled": return "Disabled";
    }
  }

  function select(n: number) {
    selected = n;
    const a = info?.assignments[String(n)];
    kind = a?.type ?? "default";
    const sys = a?.type === "keys" ? SYSTEM.findIndex((x) => x.keys.join() === a.keys.join()) : -1;
    if (sys >= 0) {
      kind = "system";
      system = sys;
    }
    keys = a?.type === "keys" ? [...a.keys] : [];
    button = a?.type === "mouse_button" ? a.button : "BTN_LEFT";
    direction = a?.type === "scroll" ? a.direction : "up";
    dpi = a?.type === "dpi" ? a.action : "cycle";
    command = a?.type === "command" ? a.command : "";
    sound = a?.type === "sound" ? a.file : (info?.sounds[0]?.file ?? "");
    steps = a?.type === "macro" ? a.steps.map((s) => ({ ...s })) : [];
  }

  // Shortcut capture: keys held together become the shortcut.
  let held = new Set<string>();
  function onKey(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    const k = linuxKey(e.code);
    if (!k) return;
    if (e.type === "keydown") {
      held.add(k);
      keys = orderKeys([...new Set([...keys, k])]);
    } else {
      held.delete(k);
      if (held.size === 0) stopCapture();
    }
  }
  function startCapture(into: "keys" | number) {
    captureTarget = into;
    keys = into === "keys" ? [] : keys;
    capturing = true;
    held = new Set();
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);
  }
  let captureTarget: "keys" | number = "keys";
  function stopCapture() {
    if (capturing && typeof captureTarget === "number") {
      const s = steps[captureTarget];
      if (s && "keys" in s) s.keys = keys;
    }
    capturing = false;
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("keyup", onKey, true);
  }
  function captureStep(i: number) {
    keys = [];
    startCapture(i);
  }

  function build(): Action | null {
    switch (kind) {
      case "default": return null;
      case "system": return { type: "keys", keys: SYSTEM[system].keys };
      case "keys": return keys.length ? { type: "keys", keys } : null;
      case "macro": return { type: "macro", steps: steps.filter((s) => !("keys" in s) || s.keys.length) };
      case "mouse_button": return { type: "mouse_button", button };
      case "scroll": return { type: "scroll", direction };
      case "dpi": return { type: "dpi", action: dpi };
      case "command": return command.trim() ? { type: "command", command: command.trim() } : null;
      case "sound": return sound ? { type: "sound", file: sound } : null;
      case "disabled": return { type: "disabled" };
    }
  }

  async function save() {
    if (selected === null) return;
    if (selected === 1 && kind !== "default" && !confirm("Change the left click? Keep another button on Left click so you can still click.")) return;
    try {
      await api.setAssignment(device.id, selected, build());
      await load();
      select(selected);
    } catch (e) {
      onerror(e);
    }
  }

  const kinds: { key: typeof kind; label: string }[] = [
    { key: "default", label: "Default" },
    { key: "system", label: "System action" },
    { key: "keys", label: "Key / shortcut" },
    { key: "macro", label: "Macro" },
    { key: "mouse_button", label: "Mouse button" },
    { key: "scroll", label: "Scroll" },
    { key: "dpi", label: "DPI" },
    { key: "command", label: "Run a program" },
    { key: "sound", label: "Play a sound" },
    { key: "disabled", label: "Disable" },
  ];
</script>

{#if info}
  <div class="panel">
    {#if info.uinput}
      <p class="warn">{info.uinput}. Assigned keys and buttons can't be sent until LHUB can use /dev/uinput (see packaging/udev).</p>
    {/if}

    <div class="detect">
      <button onclick={detect} class:active={detecting}>{detecting ? "Press a button on the mouse…" : "Detect button"}</button>
    </div>

    <ul class="buttons">
      {#each Array.from({ length: count }, (_, i) => i + 1) as n}
        <li>
          <button class:active={selected === n} onclick={() => select(n)}>
            <span class="name">{label(n)}</span>
            <span class="what" class:set={info.assignments[String(n)]}>{describe(info.assignments[String(n)])}</span>
          </button>
        </li>
      {/each}
    </ul>

    {#if selected !== null}
      <section class="editor">
        <h3>{label(selected)}</h3>
        <div class="kinds">
          {#each kinds as k}
            <button class:active={kind === k.key} onclick={() => (kind = k.key)}>{k.label}</button>
          {/each}
        </div>

        {#if kind === "system"}
          <select bind:value={system}>
            {#each [...new Set(SYSTEM.map((x) => x.group))] as g}
              <optgroup label={g}>
                {#each SYSTEM.map((x, i) => ({ ...x, i })).filter((x) => x.group === g) as x}
                  <option value={x.i}>{x.label} ({x.keys.map(keyLabel).join(" + ")})</option>
                {/each}
              </optgroup>
            {/each}
          </select>
        {:else if kind === "keys"}
          <div class="row">
            <button class="capture" class:active={capturing} onclick={() => startCapture("keys")}>
              {capturing ? "Press the keys…" : keys.length ? keys.map(keyLabel).join(" + ") : "Click, then press the shortcut"}
            </button>
          </div>
          <label class="row small">
            <span>Or a media/system key</span>
            <select bind:value={special} onchange={() => (keys = [special])}>
              {#each info.keys.filter((k) => /PLAYPAUSE|NEXTSONG|PREVIOUSSONG|STOPCD|VOLUME|MUTE|MICMUTE|CALC|MAIL|HOMEPAGE|BRIGHTNESS|F1[3-9]|F2[0-4]|SYSRQ|BACK$|FORWARD$/.test(k)) as k}
                <option value={k}>{keyLabel(k)}</option>
              {/each}
            </select>
          </label>
        {:else if kind === "macro"}
          <ol class="steps">
            {#each steps as s, i}
              <li>
                {#if "keys" in s}
                  <button class="capture" class:active={capturing && captureTarget === i} onclick={() => captureStep(i)}>
                    {capturing && captureTarget === i ? "Press the keys…" : s.keys.length ? s.keys.map(keyLabel).join(" + ") : "Set keys"}
                  </button>
                {:else}
                  <label class="pause">Pause <input type="number" min="1" max="10000" bind:value={s.ms} /> ms</label>
                {/if}
                <button class="ghost" onclick={() => (steps = steps.filter((_, j) => j !== i))} aria-label="Remove step">✕</button>
              </li>
            {/each}
          </ol>
          <div class="row">
            <button onclick={() => (steps = [...steps, { keys: [] }])}>+ Keys</button>
            <button onclick={() => (steps = [...steps, { ms: 100 }])}>+ Pause</button>
          </div>
        {:else if kind === "mouse_button"}
          <select bind:value={button}>
            {#each ["BTN_LEFT", "BTN_RIGHT", "BTN_MIDDLE", "BTN_SIDE", "BTN_EXTRA"] as b}<option value={b}>{keyLabel(b)}</option>{/each}
          </select>
        {:else if kind === "scroll"}
          <select bind:value={direction}>
            {#each ["up", "down", "left", "right"] as d}<option value={d}>Scroll {d}</option>{/each}
          </select>
        {:else if kind === "dpi"}
          <select bind:value={dpi}>
            <option value="up">DPI up</option>
            <option value="down">DPI down</option>
            <option value="cycle">DPI cycle</option>
            <option value="shift">DPI shift (lowest level while held)</option>
          </select>
        {:else if kind === "sound"}
          {#if info.sounds.length}
            <select bind:value={sound}>
              {#each info.sounds as s}<option value={s.file}>{s.name}</option>{/each}
            </select>
          {/if}
          <input placeholder="or a path to any sound file" bind:value={sound} />
        {:else if kind === "command"}
          <input placeholder="e.g. spectacle -r" bind:value={command} />
        {/if}

        <div class="row">
          <button class="primary" onclick={save}>Save</button>
        </div>
        <p class="note">
          Assigned buttons stop sending their normal click; LHUB performs the action instead, while
          the service runs.
        </p>
      </section>
    {/if}
  </div>
{/if}

<style>
  .panel { display: flex; flex-direction: column; gap: 16px; max-width: 560px; }
  .warn { margin: 0; color: var(--warn); font-size: 12px; }
  .buttons { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .buttons button { width: 100%; display: flex; justify-content: space-between; gap: 12px; text-align: left; padding: 9px 12px; }
  .buttons button.active { border-color: var(--accent); background: var(--bg-hover); }
  .name { font-weight: 600; }
  .what { color: var(--text-faint); font-size: 12px; }
  .what.set { color: var(--accent); }
  .detect button.active { border-color: var(--accent); color: var(--accent); }
  .editor { display: flex; flex-direction: column; gap: 12px; padding-top: 6px; border-top: 1px solid var(--line); }
  h3 { margin: 6px 0 0; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .kinds { display: flex; flex-wrap: wrap; gap: 6px; }
  .kinds button { padding: 6px 10px; font-size: 12px; }
  .kinds button.active { border-color: var(--accent); color: var(--accent); }
  .row { display: flex; gap: 8px; align-items: center; }
  .row.small { justify-content: space-between; font-size: 12px; color: var(--text-dim); }
  .capture { flex: 1; text-align: left; }
  .capture.active { border-color: var(--accent); color: var(--accent); }
  .steps { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; }
  .steps li { display: flex; gap: 6px; align-items: center; }
  .pause { display: flex; align-items: center; gap: 6px; font-size: 12px; }
  .pause input { width: 80px; }
  .ghost { background: none; border: none; color: var(--text-dim); }
  .primary { background: var(--accent); color: #062a26; border-color: transparent; font-weight: 600; }
  .note { margin: 0; color: var(--text-faint); font-size: 12px; }
</style>
