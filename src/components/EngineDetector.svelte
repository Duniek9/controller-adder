
<script lang="ts">
  import ReportExport from "./ReportExport.svelte";
  import { invoke } from '@tauri-apps/api/core';


type Props = {
  gamePath: string;
  onEngineDetected?: (engine: string | null) => void;
};

let { gamePath, onEngineDetected }: Props = $props();


  let engine = $state<string | null>(null);
  let isScanning = $state(false);
  let reportPath = $state('');
  let error = $state<string | null>(null);

  $effect(() => {
    // Track changes to the selected directory.
    const path = gamePath;
    onEngineDetected?.(null);
    // Clear the previous detection result.
    engine = null;
    error = null;
    isScanning = true;

    let cancelled = false;

    async function scan() {
      try {
        const detected = await invoke<string>(
          'detect_engine',
          { path }
        );

        if (!cancelled) {
          reportPath = path;
          engine = detected;
          onEngineDetected?.(detected)
        }
      } catch (err) {
        if (!cancelled) {
          error = String(err);
          onEngineDetected?.(null);
        }
      } finally {
        if (!cancelled) {
          isScanning = false;
        }
      }
    }

    void scan();

    // Prevent an older scan from overwriting
    // a newer result.
    return () => {
      cancelled = true;
    };
  });
</script>

<section class="engine-detector">
  <h2>Game information</h2>

  <div class="info-row">
    <span>Directory</span>
    <strong>{gamePath}</strong>
  </div>

  <div class="info-row">
    <span>Game engine</span>

    {#if isScanning}
      <strong class="scanning">Scanning...</strong>
    {:else if error}
      <strong class="error">Detection failed</strong>
    {:else if engine === 'Unknown engine'}
      <strong class="unknown">Unknown engine</strong>
    {:else if engine}
      <strong class="detected">{engine}</strong>
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if !isScanning && !error && reportPath === gamePath && engine}
    <ReportExport {gamePath} kind="detect" report={{ path: gamePath, engine }} />
  {/if}
</section>

<style>
  .engine-detector {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    background: #1e1e1e;
    border: 1px solid #414141;
    border-radius: 12px;
    color: #f5f5f5;
  }

  h2 {
    margin: 0;
  }

  .info-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .info-row span {
    font-size: 13px;
    color: #aaaaaa;
  }

  .info-row strong {
    overflow-wrap: anywhere;
  }

  .scanning {
    color: #b2a4ff;
  }

  .detected {
    color: #69d59a;
  }

  .unknown {
    color: #e8b86d;
  }

  .error {
    color: #ff7777;
  }
</style>
