<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  type MappingCandidate = {
    plugin: string;
    source_file: string;
    line: number;
    action: string;
    assignment_confidence: string;
    configuration_status: string;
    context: string;
  };
  type InputSlot = {
    input_kind: string;
    input_index: number;
    key_name?: string;
    core_default_actions: string[];
    plugin_candidates: MappingCandidate[];
    status: string;
    note: string;
  };
  type ActionRef = { action: string; classification: string; references: number };
  type CandidateBinding = {
    input_kind: string;
    input_index: number;
    key_name?: string;
    configuration_status: string;
    context: string;
  };
  type CandidateAction = ActionRef & { mapping_candidates: CandidateBinding[] };
  type EffectiveReport = {
    schema_version: number;
    engine: string;
    resolver_schema_version: number;
    inputs: InputSlot[];
    unresolved_actions: ActionRef[];
    candidate_mapped_actions: CandidateAction[];
    potential_conflict_count: number;
    warnings: string[];
  };

  // Optional: parent GameSelector can pass its selected path as `gamePath`.
  export let gamePath = '';

  let report: EffectiveReport | null = null;
  let error = '';
  let loading = false;
  let query = '';
  let filter = 'all';
  let showDisabled = false;
  let requestCounter = 0;

  const statusLabels: Record<string, string> = {
    core_default_only: 'Core default',
    disabled_plugin_candidates_only: 'Disabled candidates',
    configured_conditional_candidate: 'Configured candidate',
    unverified_plugin_assignment: 'Unverified assignment',
    plugin_assignment_candidate: 'Plugin candidate',
    potential_override_or_collision: 'Potential conflict'
  };

  function statusLabel(status: string): string {
    return statusLabels[status] ?? status.replaceAll('_', ' ');
  }

  function classFor(status: string): string {
    if (status === 'potential_override_or_collision') return 'risk';
    if (status === 'configured_conditional_candidate') return 'configured';
    if (status === 'unverified_plugin_assignment' || status === 'plugin_assignment_candidate') return 'unknown';
    return 'neutral';
  }

  function candidateStatus(status: string): string {
    if (status === 'enabled_by_configuration') return 'Enabled by configuration';
    if (status === 'disabled_by_configuration') return 'Disabled by configuration';
    return 'Not verified';
  }

  function formatCode(slot: InputSlot): string {
    return slot.key_name || `${slot.input_kind} ${slot.input_index}`;
  }

  async function scan() {
    const path = gamePath.trim();
    if (!path) {
      error = 'Enter or select a game folder before scanning.';
      report = null;
      return;
    }
    const thisRequest = ++requestCounter;
    loading = true;
    error = '';
    report = null;
    try {
      const result = await invoke<EffectiveReport>('scan_effective_inputs', { gamePath: path });
      if (thisRequest !== requestCounter) return;
      if (result.schema_version !== 2) {
        throw new Error(`Unsupported effective-input schema ${result.schema_version}; expected 2.`);
      }
      report = result;
    } catch (cause) {
      if (thisRequest === requestCounter) {
        error = cause instanceof Error ? cause.message : String(cause);
      }
    } finally {
      if (thisRequest === requestCounter) loading = false;
    }
  }

  $: visibleInputs = report
    ? report.inputs.filter((slot) => {
        const text = [
          slot.key_name ?? '', slot.input_kind, String(slot.input_index),
          ...slot.core_default_actions,
          ...slot.plugin_candidates.flatMap((candidate) => [candidate.plugin, candidate.action])
        ].join(' ').toLowerCase();
        return (filter === 'all' || slot.status === filter)
          && (showDisabled || slot.status !== 'disabled_plugin_candidates_only')
          && text.includes(query.trim().toLowerCase());
      })
    : [];

  $: configuredCount = report?.inputs.filter((slot) => slot.status === 'configured_conditional_candidate').length ?? 0;
  $: unverifiedCount = report?.inputs.filter((slot) => slot.status === 'unverified_plugin_assignment' || slot.status === 'plugin_assignment_candidate').length ?? 0;
  $: disabledCount = report?.inputs.filter((slot) => slot.status === 'disabled_plugin_candidates_only').length ?? 0;
</script>

<section class="effective-inventory">
  <header>
    <div>
      <h2>Effective Input Inventory</h2>
      <p>Read-only analysis of RPG Maker keyboard and gamepad mapping evidence.</p>
    </div>
  </header>

  <form class="path-form" on:submit|preventDefault={scan}>
    <label for="effective-path">Game directory</label>
    <div class="path-row">
      <input id="effective-path" type="text" bind:value={gamePath} placeholder="/path/to/RPG Maker game" spellcheck="false" />
      <button type="submit" disabled={loading || !gamePath.trim()}>{loading ? 'Scanning…' : 'Scan inputs'}</button>
    </div>
    <p class="small">Use the folder containing <code>www/js</code> or <code>js</code>. No game files are modified.</p>
  </form>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if report}
    <div class="summary" aria-label="Scan summary">
      <div><span>Engine</span><strong>{report.engine}</strong></div>
      <div><span>Input slots</span><strong>{report.inputs.length}</strong></div>
      <div><span>Configured candidates</span><strong>{configuredCount}</strong></div>
      <div><span>Unverified slots</span><strong>{unverifiedCount}</strong></div>
      <div><span>Disabled-only slots</span><strong>{disabledCount}</strong></div>
      <div><span>Potential conflicts</span><strong>{report.potential_conflict_count}</strong></div>
    </div>

    {#if report.warnings.length}
      <div class="warnings"><strong>Analysis limitations</strong>
        {#each report.warnings as warning}
          <p>{warning}</p>
        {/each}
      </div>
    {/if}

    <div class="filters">
      <label>Search <input type="search" bind:value={query} placeholder="Key, action, plugin…" /></label>
      <label>Status
        <select bind:value={filter}>
          <option value="all">All statuses</option>
          {#each Object.entries(statusLabels) as [key, value]}
            <option value={key}>{value}</option>
          {/each}
        </select>
      </label>
      <label class="checkbox"><input type="checkbox" bind:checked={showDisabled} /> Show disabled-only slots</label>
    </div>

    <p class="small">Showing {visibleInputs.length} of {report.inputs.length} input slots.</p>
    <div class="slot-list">
      {#each visibleInputs as slot (`${slot.input_kind}:${slot.input_index}`)}
        <details class="slot">
          <summary>
            <span class="slot-name">{formatCode(slot)} <small>({slot.input_kind} #{slot.input_index})</small></span>
            <span class="slot-action">{slot.core_default_actions.length ? slot.core_default_actions.join(', ') : 'No core action'}</span>
            <span class:configured={classFor(slot.status) === 'configured'} class:risk={classFor(slot.status) === 'risk'} class:unknown={classFor(slot.status) === 'unknown'} class="status">{statusLabel(slot.status)}</span>
          </summary>
          <div class="slot-details">
            <p>{slot.note}</p>
            {#if slot.plugin_candidates.length}
              <div class="candidate-list">
                {#each slot.plugin_candidates as candidate, i (`${candidate.plugin}:${candidate.line}:${candidate.action}:${i}`)}
                  <div class="candidate">
                    <strong>{candidate.action}</strong> <span>from {candidate.plugin}</span>
                    <small>{candidateStatus(candidate.configuration_status)} · {candidate.context.replaceAll('_', ' ')}</small>
                    <code>{candidate.source_file}:{candidate.line}</code>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="small">No literal plugin mapper writes detected for this slot.</p>
            {/if}
          </div>
        </details>
      {:else}
        <p class="empty">No inputs match the current filters.</p>
      {/each}
    </div>

    <div class="action-panels">
      <div>
        <h3>Mapping candidates ({report.candidate_mapped_actions.length})</h3>
        {#each report.candidate_mapped_actions as entry}
          <div class="action-entry"><strong>{entry.action}</strong> · {entry.mapping_candidates.map((candidate) => `${candidate.key_name ?? candidate.input_index} (${candidate.configuration_status.replaceAll('_', ' ')})`).join(', ')}</div>
        {:else}
          <p class="small">No custom actions with non-disabled mapping candidates.</p>
        {/each}
      </div>
      <div>
        <h3>Unresolved actions ({report.unresolved_actions.length})</h3>
        {#each report.unresolved_actions as entry}
          <div class="action-entry"><strong>{entry.action}</strong> · {entry.references} code reference(s)</div>
        {:else}
          <p class="small">No unresolved custom actions detected.</p>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .effective-inventory { display:grid; gap:1rem; width:100%; color:var(--text-color,inherit); }
  header h2 { margin:0 0 .25rem; font-size:1.45rem; }
  header p, .small { margin:.15rem 0; opacity:.75; font-size:.85rem; }
  .path-form, .warnings, .slot, .action-panels>div { padding:1rem; border:1px solid #7775; border-radius:.75rem; }
  .path-form { display:grid; gap:.5rem; }
  .path-row { display:flex; gap:.65rem; }
  input:not([type='checkbox']), select { min-width:0; padding:.6rem .7rem; background:transparent; color:inherit; border:1px solid #8888; border-radius:.5rem; }
  .path-row input { flex:1; }
  select option { color:#101010; }
  button { padding:.65rem 1rem; border:0; border-radius:.5rem; background:#7355c9; color:white; font-weight:600; cursor:pointer; }
  button:disabled { opacity:.5; cursor:not-allowed; }
  .error { color:#ef7070; }
  .summary { display:grid; grid-template-columns:repeat(auto-fit,minmax(140px,1fr)); gap:.65rem; }
  .summary>div { padding:.8rem; border:1px solid #7775; border-radius:.7rem; display:grid; gap:.35rem; }
  .summary span { opacity:.7; font-size:.8rem; }
  .summary strong { font-size:1.1rem; }
  .warnings { border-color:#c89a4677; }
  .warnings p { font-size:.85rem; margin:.4rem 0 0; }
  .filters { display:flex; flex-wrap:wrap; align-items:end; gap:.8rem; }
  .filters label:not(.checkbox) { display:grid; gap:.25rem; font-size:.83rem; }
  .filters .checkbox { display:flex; align-items:center; gap:.35rem; padding-bottom:.5rem; font-size:.85rem; }
  .slot-list { display:grid; gap:.5rem; }
  .slot { padding:0; }
  .slot summary { cursor:pointer; display:flex; flex-wrap:wrap; align-items:center; gap:.6rem; padding:.8rem 1rem; }
  .slot-name { min-width:130px; font-weight:650; }
  .slot-name small { font-weight:400; opacity:.65; }
  .slot-action { flex:1; font-size:.85rem; opacity:.78; }
  .status { font-size:.75rem; padding:.2rem .5rem; border-radius:1rem; background:#8883; }
  .status.configured { background:#24965a33; }
  .status.unknown { background:#cb934833; }
  .status.risk { background:#d9545433; }
  .slot-details { border-top:1px solid #7774; padding:.85rem 1rem; display:grid; gap:.6rem; }
  .slot-details>p { margin:0; font-size:.85rem; }
  .candidate-list { display:grid; gap:.5rem; }
  .candidate { padding:.6rem; background:#8881; border-radius:.45rem; display:grid; gap:.25rem; font-size:.86rem; }
  .candidate small { opacity:.7; }
  .candidate code { font-size:.78rem; overflow-wrap:anywhere; }
  .action-panels { display:grid; grid-template-columns:repeat(auto-fit,minmax(260px,1fr)); gap:.8rem; }
  .action-panels h3 { margin:0 0 .8rem; font-size:1rem; }
  .action-entry { padding:.45rem 0; border-bottom:1px solid #7773; font-size:.86rem; }
  .empty { opacity:.7; text-align:center; padding:1rem; }
  @media (max-width:580px) { .path-row { flex-direction:column; } .slot-name { width:100%; } }
</style>
