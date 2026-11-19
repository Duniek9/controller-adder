
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  type InputEvidence = {
    plugin: string;
    parameter: string;
    value: string;
    kind: string;
    confidence: string;
  };

  type Props = {
    gamePath: string;
  };

  let { gamePath }: Props = $props();

  let evidence = $state<InputEvidence[]>([]);
  let isScanning = $state(false);
  let error = $state<string | null>(null);

  let filter = $state('all');
  let expandedItems = $state<number[]>([]);

  let filteredEvidence = $derived(
    evidence
      .map((item, index) => ({ ...item, index }))
      .filter((item) => {
        if (filter === 'all') return true;
        return item.kind === filter;
      })
  );

  let bindingCount = $derived(
    evidence.filter(
      (item) => item.kind === 'key_binding_candidate'
    ).length
  );

  let eventCount = $derived(
    evidence.filter(
      (item) => item.kind === 'key_event_candidate'
    ).length
  );

  function toggleItem(index: number) {
    if (expandedItems.includes(index)) {
      expandedItems = expandedItems.filter(
        (item) => item !== index
      );
    } else {
      expandedItems = [...expandedItems, index];
    }
  }

  function readableKind(kind: string): string {
    switch (kind) {
      case 'key_binding_candidate':
        return 'Keyboard binding';
      case 'key_event_candidate':
        return 'Key-triggered event';
      default:
        return kind;
    }
  }

  $effect(() => {
    const path = gamePath;

    evidence = [];
    error = null;
    isScanning = true;
    expandedItems = [];

    let cancelled = false;

    async function scan() {
      try {
        const result = await invoke<InputEvidence[]>(
          'scan_parameters',
          { path }
        );

        if (!cancelled) {
          evidence = result;
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

<section class="parameter-scanner">
  <div class="header">
    <div>
      <h2>Plugin Input Candidates</h2>
      <p>Possible controls discovered from plugin parameters.</p>
    </div>

    {#if !isScanning && !error}
      <span class="count">{evidence.length} candidates</span>
    {/if}
  </div>

  {#if isScanning}
    <p class="status">Analyzing plugin parameters...</p>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {:else}
    <div class="summary">
      <div>
        <strong>{bindingCount}</strong>
        <span>Keyboard bindings</span>
      </div>

      <div>
        <strong>{eventCount}</strong>
        <span>Key-triggered events</span>
      </div>
    </div>

    <label class="filter">
      Show:
      <select bind:value={filter}>
        <option value="all">All candidates</option>
        <option value="key_binding_candidate">
          Keyboard bindings
        </option>
        <option value="key_event_candidate">
          Key-triggered events
        </option>
      </select>
    </label>

    {#if filteredEvidence.length === 0}
      <p class="status">
        No matching input candidates found.
      </p>
    {:else}
      <div class="candidate-list">
        {#each filteredEvidence as item (item.index)}
          <div class="candidate-item">
            <button
              class="candidate-button"
              onclick={() => toggleItem(item.index)}
              aria-expanded={expandedItems.includes(item.index)}
            >
              <span class="candidate-name">
                {item.parameter}
              </span>

              <span class="candidate-value">
                {item.value}
              </span>

              <span class="arrow">
                {expandedItems.includes(item.index) ? '▲' : '▼'}
              </span>
            </button>

            {#if expandedItems.includes(item.index)}
              <div class="candidate-details">
                <div class="detail">
                  <span>Plugin</span>
                  <strong>{item.plugin}</strong>
                </div>

                <div class="detail">
                  <span>Parameter</span>
                  <strong>{item.parameter}</strong>
                </div>

                <div class="detail">
                  <span>Value</span>
                  <strong>{item.value}</strong>
                </div>

                <div class="detail">
                  <span>Type</span>
                  <strong>{readableKind(item.kind)}</strong>
                </div>

                <div class="detail">
                  <span>Confidence</span>
                  <strong class="warning">
                    {item.confidence === 'needs_verification'
                      ? 'Needs verification'
                      : item.confidence}
                  </strong>
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    <p class="disclaimer">
      These are possible controls found in plugin
      configurations. They are not confirmed runtime
      mappings.
    </p>
  {/if}
</section>

<style>
  .parameter-scanner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    background: #1e1e1e;
    border: 1px solid #414141;
    border-radius: 12px;
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

  .summary {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .summary > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px;
    background: #292929;
    border-radius: 8px;
  }

  .summary strong {
    font-size: 22px;
    color: #b2a4ff;
  }

  .summary span {
    font-size: 12px;
    color: #aaaaaa;
  }

  .filter {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #cccccc;
    font-size: 13px;
  }

  
select {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;

  appearance: none;
  -webkit-appearance: none;

  background-color: #303030;
  color: #ffffff;

  border: 1px solid #555555;
  border-radius: 6px;

  font-size: 13px;
  cursor: pointer;

  color-scheme: dark;
}

select option {
  background-color: #303030;
  color: #ffffff;
}

select:focus {
  outline: 2px solid #806cf0;
  outline-offset: 2px;
}

  .candidate-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: none;
    overflow: visible;
}

  .candidate-item {
    border: 1px solid #414141;
    border-radius: 8px;
    overflow: hidden;
  }

  .candidate-button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: none;
    background: #292929;
    color: #ffffff;
    text-align: left;
    cursor: pointer;
  }

  .candidate-button:hover {
    background: #363636;
  }

  .candidate-name {
    flex: 1;
    font-weight: 600;
    overflow-wrap: anywhere;
    min-width: 0;
  }

  .candidate-value {
    padding: 5px 9px;
    background: #393148;
    border-radius: 6px;
    color: #cfbaff;
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .arrow {
    color: #aaaaaa;
    font-size: 11px;
  }

  .candidate-details {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: #222222;
  }

  .detail {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .detail span {
    font-size: 12px;
    color: #aaaaaa;
  }

  .detail strong {
    font-size: 13px;
    font-weight: 500;
    overflow-wrap: anywhere;
  }

  .warning {
    color: #e8b86d;
  }

  .status {
    color: #aaaaaa;
  }

  .error {
    color: #ff7777;
    overflow-wrap: anywhere;
  }

  .disclaimer {
    padding: 12px;
    border-left: 3px solid #e8b86d;
    background: #30291c;
    color: #e8b86d;
    border-radius: 4px;
    font-size: 12px;
    line-height: 1.5;
  }
</style>
