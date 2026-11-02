<script lang="ts">
  // G HUB's preset picker: the selected preset with its actions, and a list grouped into the
  // presets from G HUB and the user's own.
  let {
    curated,
    custom,
    curatedLabel,
    customLabel,
    selected,
    tuned = false,
    notes = {},
    disabled = false,
    onselect,
    onduplicate,
    onrename,
    ondelete,
    onreset,
  }: {
    curated: string[];
    custom: string[];
    curatedLabel: string;
    customLabel: string;
    selected: string | null;
    tuned?: boolean;
    notes?: Record<string, string>;
    disabled?: boolean;
    onselect: (name: string) => void;
    onduplicate: () => void;
    onrename: (name: string, to: string) => void;
    ondelete: (name: string) => void;
    onreset: () => void;
  } = $props();

  let open = $state(false);
  let renaming = $state(false);
  let newName = $state("");
  const isCustom = $derived(!!selected && custom.includes(selected));

  function pick(name: string) {
    open = false;
    if (name !== selected) onselect(name);
  }
  function startRename() {
    newName = selected ?? "";
    renaming = true;
  }
  function finishRename() {
    renaming = false;
    const to = newName.trim();
    if (selected && to && to !== selected) onrename(selected, to);
  }
</script>

<div class="picker">
  <div class="header">
    {#if renaming}
      <input class="rename" bind:value={newName} onkeydown={(e) => (e.key === "Enter" ? finishRename() : e.key === "Escape" && (renaming = false))} onblur={finishRename} aria-label="Preset name" />
    {:else}
      <button class="current" {disabled} onclick={() => (open = !open)} aria-expanded={open}>
        <span class="name">{selected ?? "Choose a preset"}{tuned ? " *" : ""}</span>
        <span class="caret">{open ? "▴" : "▾"}</span>
      </button>
    {/if}
    <div class="actions">
      {#if tuned}<button class="icon" title="Reset all changes to this preset" aria-label="Reset" onclick={onreset}>↺</button>{/if}
      <button class="icon" title="Duplicate preset" aria-label="Duplicate" {disabled} onclick={onduplicate}>⧉</button>
      {#if isCustom}
        <button class="icon" title="Rename preset" aria-label="Rename" onclick={startRename}>✎</button>
        <button class="icon danger" title="Delete preset" aria-label="Delete" onclick={() => selected && confirm(`Delete ${selected}?`) && ondelete(selected)}>🗑</button>
      {/if}
    </div>
  </div>
  {#if selected && notes[selected]}<p class="note">{notes[selected]}</p>{/if}

  {#if open}
    <div class="list" role="listbox">
      <div class="group">{curatedLabel}</div>
      {#each curated as name}
        <button class="item" class:active={name === selected} role="option" aria-selected={name === selected} onclick={() => pick(name)}>{name}</button>
      {/each}
      <div class="group">{customLabel}</div>
      {#each custom as name}
        <button class="item" class:active={name === selected} role="option" aria-selected={name === selected} onclick={() => pick(name)}>{name}</button>
      {:else}
        <div class="empty">Duplicate a preset to make your own.</div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker { display: flex; flex-direction: column; gap: 6px; position: relative; }
  .header { display: flex; gap: 6px; align-items: center; }
  .current { flex: 1; display: flex; justify-content: space-between; align-items: center; text-align: left; padding: 9px 12px; }
  .name { font-weight: 600; }
  .caret { color: var(--text-dim); }
  .rename { flex: 1; }
  .actions { display: flex; gap: 4px; }
  .icon { padding: 7px 9px; min-width: 34px; color: var(--text-dim); }
  .icon:hover:not(:disabled) { color: var(--accent); }
  .icon.danger:hover { color: var(--danger); }
  .list { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: 10px; background: var(--bg); padding: 6px; max-height: 320px; overflow: auto; }
  .group { padding: 8px 8px 4px; font-size: 11px; font-weight: 600; color: var(--text-faint); text-transform: uppercase; letter-spacing: 0.08em; }
  .item { text-align: left; border: none; background: none; padding: 7px 10px; border-radius: 6px; }
  .item:hover { background: var(--bg-hover); }
  .item.active { color: var(--accent); }
  .empty { padding: 6px 10px; color: var(--text-faint); font-size: 12px; }
  .note { margin: 0; color: var(--text-dim); font-size: 12px; line-height: 1.5; }
</style>
