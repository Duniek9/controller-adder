
<script lang="ts">
  import ReportExport from "./ReportExport.svelte";
  import { invoke } from '@tauri-apps/api/core';

  type KeyBinding = {
    key_code: number;
    key_name: string;
  };

  type InputAction = {
    action: string;
    keyboard_keys: KeyBinding[];
    source: string;
    confidence: string;
  };

  type Props = {
    gamePath: string;
  };

  let { gamePath }: Props = $props();

  let actions = $state<InputAction[]>([]);
  let isScanning = $state(false);
  let reportPath = $state('');
  let error = $state<string | null>(null);

  let expandedActions = $state<string[]>([]);

  function toggleAction(action: string) {
    if (expandedActions.includes(action)) {
      expandedActions = expandedActions.filter(
        (item) => item !== action
      );
    } else {
      expandedActions = [...expandedActions, action];
    }
  }

  $effect(() => {
    const path = gamePath;

    actions = [];
    expandedActions = [];
    error = null;
    isScanning = true;

    let cancelled = false;

    async function scan() {
      try {
        const result = await invoke<InputAction[]>(
          'scan_inputs',
          { path }
        );

        if (!cancelled) {
          reportPath = path;
          actions = result;
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

<section class="input-scanner">
  <div class="header">
    <div>
      <h2>Detected Input Actions</h2>
      <p>Keyboard bindings discovered in game files.</p>
    </div>

    {#if !isScanning && !error}
      <span class="count">{actions.length} actions</span>
    {/if}
  </div>

  {#if isScanning}
    <p class="status">Scanning input mappings...</p>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {:else if actions.length === 0}
    <p class="status">No input actions found.</p>
  {:else}
    <div class="action-list">
      {#each actions as item (item.action)}
        <div class="action-item">
          <button
            class="action-button"
            onclick={() => toggleAction(item.action)}
            aria-expanded={expandedActions.includes(item.action)}
          >
            <span class="action-name">{item.action}</span>

            <span class="action-summary">
              {item.keyboard_keys.length}
              {item.keyboard_keys.length === 1 ? 'key' : 'keys'}
            </span>

            <span class="arrow">
              {expandedActions.includes(item.action) ? '▲' : '▼'}
            </span>
          </button>

          {#if expandedActions.includes(item.action)}
            <div class="action-details">
              <div class="detail-row">
                <span>Keyboard bindings</span>

                <div class="keys">
                  {#each item.keyboard_keys as key (key.key_code)}
                    <span class="key">
                      {key.key_name}
                    </span>
                  {/each}
                </div>
              </div>

              <div class="detail-row">
                <span>Source</span>
                <strong>{item.source}</strong>
              </div>

              <div class="detail-row">
                <span>Detection</span>
                <strong>{item.confidence}</strong>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if !isScanning && !error && reportPath === gamePath}
    <ReportExport {gamePath} kind="inputs" report={actions} />
  {/if}
</section>

<style>
  .input-scanner {
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
    font-size: 13px;
    color: #aaaaaa;
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

  .action-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .action-item {
    border: 1px solid #414141;
    border-radius: 8px;
    overflow: hidden;
  }

  .action-button {
    width: 100%;
    padding: 14px;
    background: #2a2a2a;
    color: #f5f5f5;
    border: none;
    cursor: pointer;

    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
  }

  .action-button:hover {
    background: #363636;
  }

  .action-name {
    flex: 1;
    font-weight: 600;
  }

  .action-summary {
    font-size: 12px;
    color: #aaaaaa;
  }

  .arrow {
    color: #aaaaaa;
    font-size: 11px;
  }

  .action-details {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
    background: #222222;
  }

  .detail-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .detail-row > span {
    color: #aaaaaa;
    font-size: 12px;
  }

  .detail-row strong {
    font-size: 13px;
    font-weight: 500;
  }

  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .key {
    padding: 5px 9px;
    border: 1px solid #555555;
    border-radius: 6px;
    background: #333333;
    color: #f5f5f5;
    font-size: 12px;
  }
</style>
