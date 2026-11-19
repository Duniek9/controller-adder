
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  type PluginInfo = {
    name: string;
    enabled: boolean;
    description: string;
    file_exists: boolean;
  };

  type Props = {
    gamePath: string;
  };

  let { gamePath }: Props = $props();

  let plugins = $state<PluginInfo[]>([]);
  let isScanning = $state(false);
  let error = $state<string | null>(null);
  let showEnabledOnly = $state(false);

  let filteredPlugins = $derived(
    showEnabledOnly
      ? plugins.filter((plugin) => plugin.enabled)
      : plugins
  );

  let enabledCount = $derived(
    plugins.filter((plugin) => plugin.enabled).length
  );

  $effect(() => {
    const path = gamePath;

    plugins = [];
    error = null;
    isScanning = true;

    let cancelled = false;

    async function scan() {
      try {
        const result = await invoke<PluginInfo[]>(
          'scan_plugins',
          { path }
        );

        if (!cancelled) {
          plugins = result;
        }
      } catch (err) {
        if (!cancelled) {
          error = String(err);
        }
      } finally {
        if (!cancelled) {
          isScanning = false;
        }
      }
    }

    void scan();

    return () => {
      cancelled = true;
    };
  });
</script>

<section class="plugin-scanner">
  <div class="header">
    <div>
      <h2>Installed Game Plugins</h2>
      <p>Plugins registered by the selected game.</p>
    </div>

    {#if !isScanning && !error}
      <span class="count">
        {enabledCount} / {plugins.length} enabled
      </span>
    {/if}
  </div>

  {#if isScanning}
    <p class="status">Scanning plugins...</p>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {:else}
    <label class="filter">
      <input type="checkbox" bind:checked={showEnabledOnly} />
      Show enabled plugins only
    </label>

    {#if filteredPlugins.length === 0}
      <p class="status">
        {showEnabledOnly
          ? 'No enabled plugins found.'
          : 'No plugins found.'}
      </p>
    {:else}
      <div class="plugin-list">
        {#each filteredPlugins as plugin, index (index)}
          <div class="plugin-item">
            <div class="plugin-heading">
              <strong class="plugin-name">
                {plugin.name}
              </strong>

              <span
                class:enabled={plugin.enabled}
                class:disabled={!plugin.enabled}
                class="badge"
              >
                {plugin.enabled ? 'Enabled' : 'Disabled'}
              </span>
            </div>

            {#if plugin.description}
              <p class="description">
                {plugin.description}
              </p>
            {/if}

            <div class="file-status">
              {#if plugin.file_exists}
                <span class="found">✓ File found</span>
              {:else}
                <span class="missing">✕ File missing</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .plugin-scanner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    border: 1px solid #414141;
    border-radius: 12px;
    background: #1e1e1e;
    color: #f5f5f5;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  h2, p {
    margin: 0;
  }

  .header p {
    margin-top: 6px;
    color: #aaaaaa;
    font-size: 13px;
  }

  .count {
    color: #b2a4ff;
    font-size: 13px;
    white-space: nowrap;
  }

  .status {
    color: #aaaaaa;
  }

  .error {
    color: #ff7777;
    overflow-wrap: anywhere;
  }

  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: #cccccc;
    cursor: pointer;
  }

  .filter input {
    accent-color: #806cf0;
  }

  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 400px;
    overflow-y: auto;
  }

  .plugin-item {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    background: #292929;
    border: 1px solid #414141;
    border-radius: 8px;
  }

  .plugin-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .plugin-name {
    overflow-wrap: anywhere;
  }

  .badge {
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 11px;
    white-space: nowrap;
  }

  .enabled {
    background: #214431;
    color: #69d59a;
  }

  .disabled {
    background: #414141;
    color: #bbbbbb;
  }

  .description {
    color: #aaaaaa;
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .file-status {
    font-size: 12px;
  }

  .found {
    color: #69d59a;
  }

  .missing {
    color: #ff7777;
  }
</style>
