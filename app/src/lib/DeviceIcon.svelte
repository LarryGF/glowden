<script lang="ts">
  import type { Kind } from "./api";

  let { kind, size = 96 }: { kind: Kind; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 96 96" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
  {#if kind === "microphone"}
    <rect x="34" y="10" width="28" height="46" rx="14" />
    <path d="M26 44a22 22 0 0 0 44 0" />
    <path d="M48 66v14M36 84h24" />
    <path d="M40 24h16M40 32h16M40 40h16" opacity="0.5" />
  {:else if kind === "headset"}
    <path d="M18 56V46a30 30 0 0 1 60 0v10" />
    <rect x="12" y="52" width="16" height="26" rx="6" />
    <rect x="68" y="52" width="16" height="26" rx="6" />
    <path d="M20 78c0 6 8 9 18 9h6" />
  {:else if kind === "keyboard"}
    <rect x="8" y="28" width="80" height="40" rx="6" />
    <path d="M18 40h4M28 40h4M38 40h4M48 40h4M58 40h4M68 40h10M18 48h6M30 48h4M40 48h4M50 48h4M60 48h4M70 48h8M26 57h44" />
  {:else if kind === "mouse"}
    <rect x="28" y="12" width="40" height="72" rx="20" />
    <path d="M48 12v26M28 38h40" />
    <rect x="45" y="20" width="6" height="10" rx="3" />
  {:else}
    <rect x="20" y="20" width="56" height="56" rx="10" />
  {/if}
</svg>
