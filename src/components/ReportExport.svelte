<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  let { gamePath, kind, report }: { gamePath: string; kind: string; report: unknown } = $props();
  let busy = $state(false);
  let notice = $state('');
  let error = $state('');
  let generation = 0;

  $effect(() => {
    void gamePath;
    void kind;
    void report;
    generation++;
    notice = '';
    error = '';
  });

  async function save() {
    if (busy) return;
    const current = generation;
    busy = true;
    notice = '';
    error = '';
    try {
      const path = await invoke<string>('export_scan_report', {
        gamePath, kind, report: JSON.parse(JSON.stringify(report))
      });
      if (current === generation) notice = 'JSON saved: ' + path;
    } catch (cause) {
      if (current === generation) error = String(cause);
    } finally {
      busy = false;
    }
  }
</script>

<div class="export">
  <button type="button" disabled={busy || !gamePath} onclick={save}>{busy ? 'Saving…' : 'Export JSON'}</button>
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .export { display:grid; gap:.4rem; justify-items:start; }
  button { background:#7355c9; color:#fff; border:0; padding:.6rem 1rem; border-radius:.5rem; cursor:pointer; }
  button:disabled { opacity:.5; cursor:not-allowed; }
  p { margin:0; font-size:.85rem; overflow-wrap:anywhere; }
  .error { color:#ff7777; }
</style>
