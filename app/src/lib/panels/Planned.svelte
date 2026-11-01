<script lang="ts">
  let { title, detail }: { title: string; detail: string } = $props();
</script>

<div class="panel">
  <h2>{title} <span class="badge warn">in development</span></h2>
  <p>{detail}</p>
</div>

<style>
  .panel { max-width: 560px; }
  h2 { margin: 0 0 12px; font-size: 20px; display: flex; align-items: center; gap: 12px; }
  p { color: var(--text-dim); margin: 0; }
</style>
