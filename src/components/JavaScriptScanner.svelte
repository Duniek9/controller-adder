
<script lang="ts">
  import ReportExport from "./ReportExport.svelte";
  import { invoke } from '@tauri-apps/api/core';

  type JavaScriptInputFinding = {
    plugin: string;
    source_file: string;
    line: number;
    pattern: string;
    category: string;
    confidence: string;
  };

  type SkippedPlugin = {
    plugin: string;
    reason: string;
  };

  type JavaScriptScanReport = {
    plugins_scanned: number;
    findings: JavaScriptInputFinding[];
    skipped: SkippedPlugin[];
  };

  type Props = {
    gamePath: string;
  };

  let { gamePath }: Props = $props();

  let report = $state<JavaScriptScanReport | null>(null);
  let isScanning = $state(false);
  let reportPath = $state('');
  let error = $state<string | null>(null);

  let categoryFilter = $state('all');
  let expandedPlugins = $state<string[]>([]);
  let showSkipped = $state(false);

  const categories = [
    { value: 'all', label: 'All findings' },
    {
      value: 'keyboard_mapping_access',
      label: 'Keyboard mappings'
    },
    {
      value: 'gamepad_mapping_access',
      label: 'Gamepad mappings'
    },
    { value: 'input_check', label: 'Input checks' },
    {
      value: 'event_listener_candidate',
      label: 'Event listeners'
    },
    {
      value: 'keyboard_handler_candidate',
      label: 'Keyboard handlers'
    }
  ];

  let filteredFindings = $derived(
    (report?.findings ?? []).filter(
      (finding) =>
        categoryFilter === 'all' ||
        finding.category === categoryFilter
    )
  );

  let groupedFindings = $derived.by(() => {
    const groups = new Map<
      string,
      JavaScriptInputFinding[]
    >();

    for (const finding of filteredFindings) {
      const existing = groups.get(finding.plugin);

      if (existing) {
        existing.push(finding);
      } else {
        groups.set(finding.plugin, [finding]);
      }
    }

    return Array.from(groups, ([name, findings]) => ({
      name,
      findings
    }));
  });

  let matchedPluginCount = $derived(
    new Set(report?.findings.map(
      (finding) => finding.plugin
    ) ?? []).size
  );

  function togglePlugin(name: string) {
    if (expandedPlugins.includes(name)) {
      expandedPlugins = expandedPlugins.filter(
        (item) => item !== name
      );
    } else {
      expandedPlugins = [...expandedPlugins, name];
    }
  }

  function readableCategory(category: string): string {
    switch (category) {
      case 'keyboard_mapping_access':
        return 'Keyboard mapping access';
      case 'gamepad_mapping_access':
        return 'Gamepad mapping access';
      case 'input_check':
        return 'Input check';
      case 'event_listener_candidate':
        return 'Event listener candidate';
      case 'keyboard_handler_candidate':
        return 'Keyboard handler candidate';
      default:
        return category;
    }
  }

  $effect(() => {
    const path = gamePath;

    report = null;
    error = null;
    isScanning = true;
    expandedPlugins = [];
    categoryFilter = 'all';
    showSkipped = false;

    let cancelled = false;

    async function scan() {
      try {
        const result = await invoke<JavaScriptScanReport>(
          'scan_javascript',
          { path }
        );

        if (!cancelled) {
          reportPath = path;
          report = result;
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

<section class="javascript-scanner">
  <div class="header">
    <div>
      <h2>JavaScript Input Analysis</h2>
      <p>
        Input-related code found in enabled RPG Maker plugins.
      </p>
    </div>
  </div>

  {#if isScanning}
    <p class="status">Analyzing JavaScript plugins...</p>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {:else if report}
    <div class="summary">
      <div class="stat">
        <strong>{report.plugins_scanned}</strong>
        <span>Scanned</span>
      </div>

      <div class="stat">
        <strong>{matchedPluginCount}</strong>
        <span>With findings</span>
      </div>

      <div class="stat">
        <strong>{report.findings.length}</strong>
        <span>Findings</span>
      </div>
    </div>

    <label class="filter">
      <span>Show category</span>
      <select bind:value={categoryFilter}>
        {#each categories as category (category.value)}
          <option value={category.value}>
            {category.label}
          </option>
        {/each}
      </select>
    </label>

    {#if groupedFindings.length === 0}
      <p class="status">
        No input-related JavaScript findings
        match the selected filter.
      </p>
    {:else}
      <div class="plugin-list">
        {#each groupedFindings as group (group.name)}
          <div class="plugin-item">
            <button
              class="plugin-button"
              onclick={() => togglePlugin(group.name)}
              aria-expanded={expandedPlugins.includes(group.name)}
            >
              <span class="plugin-name">
                {group.name}
              </span>

              <span class="finding-count">
                {group.findings.length} findings
              </span>

              <span class="arrow">
                {expandedPlugins.includes(group.name)
                  ? '▲'
                  : '▼'}
              </span>
            </button>

            {#if expandedPlugins.includes(group.name)}
              <div class="plugin-details">
                {#each group.findings as finding, index (index)}
                  <div class="finding">
                    <div class="finding-heading">
                      <strong>{finding.pattern}</strong>
                      <span>Line {finding.line}</span>
                    </div>

                    <p>
                      {readableCategory(finding.category)}
                    </p>

                    <small>
                      {finding.source_file}
                    </small>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if report.skipped.length > 0}
      <div class="skipped-section">
        <button
          class="skipped-button"
          onclick={() => showSkipped = !showSkipped}
          aria-expanded={showSkipped}
        >
          Skipped plugins ({report.skipped.length})
          <span>{showSkipped ? '▲' : '▼'}</span>
        </button>

        {#if showSkipped}
          <div class="skipped-list">
            {#each report.skipped as item, index (index)}
              <div class="skipped-item">
                <strong>{item.plugin}</strong>
                <p>{item.reason}</p>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <p class="disclaimer">
      Static analysis only. Findings indicate potential
      input-handling code, not verified working controls.
    </p>
  {/if}

  {#if !isScanning && !error && reportPath === gamePath && report}
    <ReportExport {gamePath} kind="javascript" report={report} />
  {/if}
</section>

<style>
  .javascript-scanner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    background: #1e1e1e;
    border: 1px solid #414141;
    border-radius: 12px;
    color: #f5f5f5;
  }

  h2, p {
    margin: 0;
  }

  .header p {
    margin-top: 6px;
    color: #aaaaaa;
    font-size: 13px;
  }

  .summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .stat {
    padding: 14px;
    background: #292929;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .stat strong {
    font-size: 22px;
    color: #b2a4ff;
  }

  .stat span {
    font-size: 12px;
    color: #aaaaaa;
  }

  .filter {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: #cccccc;
  }

  select {
    width: 100%;
    padding: 10px 12px;
    background: #303030;
    color: #ffffff;
    color-scheme: dark;
    border: 1px solid #555555;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }

  select option {
    background: #303030;
    color: #ffffff;
  }

  select:focus-visible {
    outline: 2px solid #806cf0;
    outline-offset: 2px;
  }

  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 480px;
    overflow-y: auto;
    min-height: 0;
  }

  .plugin-item {
    flex-shrink: 0;
    border: 1px solid #414141;
    border-radius: 8px;
    overflow: hidden;
  }

  .plugin-button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 14px;
    border: none;
    background: #292929;
    color: #ffffff;
    text-align: left;
    cursor: pointer;
  }

  .plugin-button:hover {
    background: #363636;
  }

  .plugin-name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .finding-count {
    font-size: 12px;
    color: #aaaaaa;
    white-space: nowrap;
  }

  .arrow {
    font-size: 11px;
    color: #aaaaaa;
  }

  .plugin-details {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: #222222;
  }

  .finding {
    padding: 12px;
    border-radius: 6px;
    background: #303030;
    overflow-wrap: anywhere;
  }

  .finding-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .finding-heading strong {
    font-size: 13px;
  }

  .finding-heading span {
    font-size: 11px;
    color: #b2a4ff;
    white-space: nowrap;
  }

  .finding p {
    margin-top: 6px;
    font-size: 12px;
    color: #cccccc;
  }

  .finding small {
    display: block;
    margin-top: 6px;
    color: #999999;
    font-size: 11px;
  }

  .status {
    color: #aaaaaa;
  }

  .error {
    color: #ff7777;
    overflow-wrap: anywhere;
  }

  .skipped-section {
    border: 1px solid #554b35;
    border-radius: 8px;
    overflow: hidden;
  }

  .skipped-button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    padding: 12px;
    background: #30291c;
    color: #e8b86d;
    border: none;
    cursor: pointer;
  }

  .skipped-list {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .skipped-item strong {
    font-size: 13px;
  }

  .skipped-item p {
    margin-top: 4px;
    font-size: 12px;
    color: #aaaaaa;
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
