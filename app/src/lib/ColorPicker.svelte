<script lang="ts">
  let { value = $bindable(), onchange }: { value: string; onchange: () => void } = $props();

  // Our own palettes; custom colors are a per-user convenience kept in this browser profile.
  const palettes: Record<string, string[]> = {
    Vibrant: ["#ff1f3d", "#ff7a00", "#ffd400", "#3dff6e", "#00e0ff", "#2f6bff", "#9b3dff", "#ff2fb4"],
    Pastel: ["#ff9aa8", "#ffc59a", "#fff0a0", "#b6f5c4", "#a8e9ff", "#a9bcff", "#d3b4ff", "#ffb3e0"],
    Mono: ["#ffffff", "#d9d9d9", "#a6a6a6", "#737373", "#404040", "#1a1a1a"],
    Neon: ["#2dd4bf", "#00ffd5", "#39ff14", "#ff00e6", "#ff3131", "#fdff00"],
  };
  let palette = $state("Vibrant");
  let custom = $state<string[]>(load());
  let hex = $state(value);

  function load(): string[] {
    try {
      return JSON.parse(localStorage.getItem("lhub.customColors") ?? "[]");
    } catch {
      return [];
    }
  }
  function save() {
    try {
      localStorage.setItem("lhub.customColors", JSON.stringify(custom));
    } catch {}
  }

  $effect(() => {
    hex = value;
  });

  function pick(c: string) {
    value = c;
    onchange();
  }
  function typed() {
    const v = hex.trim().replace(/^#?/, "#");
    if (/^#[0-9a-fA-F]{6}$/.test(v)) pick(v.toLowerCase());
  }
  function addCustom() {
    if (!custom.includes(value)) custom = [...custom, value].slice(-10);
    save();
  }
  const rgb = $derived([1, 3, 5].map((i) => parseInt(value.slice(i, i + 2), 16)));
</script>

<div class="picker">
  <div class="top">
    <input type="color" bind:value oninput={onchange} aria-label="Color" />
    <input class="hex" bind:value={hex} onchange={typed} spellcheck="false" aria-label="Hex color" />
    <span class="rgb">R {rgb[0]} · G {rgb[1]} · B {rgb[2]}</span>
  </div>
  <div class="tabs">
    {#each Object.keys(palettes) as name}
      <button class:active={palette === name} onclick={() => (palette = name)}>{name}</button>
    {/each}
  </div>
  <div class="swatches">
    {#each palettes[palette] as c}
      <button class="swatch" class:active={value === c} style:background={c} aria-label={c} onclick={() => pick(c)}></button>
    {/each}
  </div>
  <div class="custom">
    <span class="label">Custom</span>
    {#each custom as c}
      <button class="swatch" class:active={value === c} style:background={c} aria-label={c} onclick={() => pick(c)}></button>
    {/each}
    <button class="add" onclick={addCustom} title="Save current color">+</button>
  </div>
</div>

<style>
  .picker { display: flex; flex-direction: column; gap: 10px; }
  .top { display: flex; align-items: center; gap: 10px; }
  input[type="color"] { width: 44px; height: 34px; border: 1px solid var(--line); border-radius: 8px; background: none; padding: 2px; cursor: pointer; }
  .hex { width: 90px; background: var(--bg-card); border: 1px solid var(--line); border-radius: 8px; color: var(--text); padding: 7px 9px; font: inherit; text-transform: uppercase; }
  .rgb { color: var(--text-faint); font-size: 12px; }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border: none; background: none; padding: 4px 8px; color: var(--text-dim); font-size: 12px; }
  .tabs button.active { color: var(--accent); box-shadow: inset 0 -2px 0 var(--accent); border-radius: 0; }
  .swatches, .custom { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .swatch { width: 26px; height: 26px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.active { border-color: var(--text); }
  .add { width: 26px; height: 26px; padding: 0; border-radius: 50%; color: var(--text-dim); }
</style>
