<script lang="ts">
  import { api, type Device } from "./api";
  import DeviceIcon from "./DeviceIcon.svelte";

  let { device, size }: { device: Device; size: number } = $props();

  // Pictures come from the user's own G HUB install (lhub import-ghub); cached per device.
  const cache: Map<string, Promise<string | null>> = ((globalThis as any).__lhubImages ??= new Map());
  let src = $state<string | null>(null);

  $effect(() => {
    const id = device.id;
    if (!cache.has(id)) cache.set(id, api.deviceImage(id).catch(() => null));
    cache.get(id)!.then((s) => (src = s));
  });
</script>

{#if src}
  <img {src} alt={device.name} style:width="{size}px" style:height="{size}px" />
{:else}
  <DeviceIcon kind={device.kind} {size} />
{/if}

<style>
  img { object-fit: contain; filter: drop-shadow(0 8px 18px rgba(0, 0, 0, 0.45)); }
</style>
