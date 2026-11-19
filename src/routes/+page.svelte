
<script lang="ts">
  import GameSelector from '../components/GameSelector.svelte';
  import EngineDetector from '../components/EngineDetector.svelte';
  import InputScanner from '../components/InputScanner.svelte';
  import PluginScanner from '../components/PluginScanner.svelte';
  import JavaScriptScanner from '../components/JavaScriptScanner.svelte';
  import { selectedGamePath } from '../lib/stores/selectedGame';
  

  let detectedEngine = $state<string | null>(null);

  function handleGameSelected(path: string) {
    selectedGamePath.set(path);
    detectedEngine = null;
  }

  function handleEngineDetected(engine: string | null) {
    detectedEngine = engine;
  }
</script>

<svelte:head>
  <title>Controller Mod Generator</title>
</svelte:head>

<main>
  <h1>Controller Mod Generator</h1>

  <GameSelector onGameSelected={handleGameSelected} />

  {#if $selectedGamePath}
    {#key $selectedGamePath}
      <EngineDetector
        gamePath={$selectedGamePath}
        onEngineDetected={handleEngineDetected}
      />

      {#if detectedEngine === 'RPG Maker MZ' ||
          detectedEngine === 'RPG Maker MV'}
        <InputScanner gamePath={$selectedGamePath} />
        <JavaScriptScanner gamePath={$selectedGamePath} />
        <PluginScanner gamePath={$selectedGamePath} />
      {:else if detectedEngine && detectedEngine !== 'Unknown engine'}
        <p class="notice">
          Input scanning for {detectedEngine} is not supported yet.
        </p>
      {/if}
    {/key}
  {/if}
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 20px;
    max-width: 800px;
    margin: 40px auto;
    padding: 20px;
  }

  h1 {
    margin: 0 0 10px;
  }

  .notice {
    padding: 16px;
    background: #30291c;
    color: #e8b86d;
    border-radius: 8px;
  }


</style>
