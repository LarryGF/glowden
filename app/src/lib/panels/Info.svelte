<script lang="ts">
  import type { Device } from "../api";

  let { device }: { device: Device } = $props();
</script>

<div class="panel">
  <dl>
    <dt>Model</dt><dd>{device.name}</dd>
    <dt>USB id</dt><dd>{device.product}</dd>
    <dt>Connection</dt><dd>{device.connection}</dd>
    <dt>Device id</dt><dd><code>{device.id}</code></dd>
    {#if device.battery}<dt>Battery</dt><dd>{device.battery.percent}%{device.battery.charging ? ", charging" : ""}</dd>{/if}
  </dl>

  <h3>Features reported by the device</h3>
  <table>
    <tbody>
      {#each device.features as f}
        <tr class:unknown={f.name === "unknown" || f.name === "?"}>
          <td><code>{f.id}</code></td>
          <td>{f.name}</td>
          <td class="v">v{f.version}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .panel { max-width: 640px; }
  h2 { margin: 0 0 18px; font-size: 20px; }
  h3 { margin: 28px 0 10px; font-size: 14px; color: var(--text-dim); font-weight: 500; }
  dl { display: grid; grid-template-columns: 120px 1fr; gap: 8px 16px; margin: 0; }
  dt { color: var(--text-dim); }
  dd { margin: 0; }
  table { width: 100%; border-collapse: collapse; }
  td { padding: 6px 8px; border-bottom: 1px solid var(--line); }
  td.v { color: var(--text-faint); text-align: right; }
  tr.unknown td { color: var(--text-faint); }
  code { font-size: 12px; color: var(--text-dim); }
</style>
