<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { selectedGamePath } from '../lib/stores/selectedGame';

  type MappingCandidate = {
    plugin: string;
    action: string;
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
  };
  type ActionEntry = { action: string; classification: string; references: number };
  type EffectiveReport = {
    schema_version: number;
    engine: string;
    inputs: InputSlot[];
    unresolved_actions: ActionEntry[];
    candidate_mapped_actions: ActionEntry[];
    warnings: string[];
  };
  type ActionOption = { id: string; kind: 'core' | 'plugin' | 'unresolved'; evidence: string };
  type Profile = {
    format_version: 1;
    engine: string;
    game_path: string;
    bindings: Record<string, number>;
  };

  const buttons = [
    { index: 0, label: 'A / Cross' },
    { index: 1, label: 'B / Circle' },
    { index: 2, label: 'X / Square' },
    { index: 3, label: 'Y / Triangle' },
    { index: 4, label: 'LB / L1' },
    { index: 5, label: 'RB / R1' },
    { index: 6, label: 'LT / L2' },
    { index: 7, label: 'RT / R2' },
    { index: 8, label: 'Back / View' },
    { index: 9, label: 'Start / Menu' },
    { index: 10, label: 'Left stick click (L3)' },
    { index: 11, label: 'Right stick click (R3)' },
    { index: 12, label: 'D-pad up' },
    { index: 13, label: 'D-pad down' },
    { index: 14, label: 'D-pad left' },
    { index: 15, label: 'D-pad right' }
  ];
  const buttonNames = new Map(buttons.map((button) => [button.index, button.label]));
  const recommended: Record<string, number> = {
    ok: 0, cancel: 1, shift: 2, menu: 3,
    pageup: 4, pagedown: 5,
    up: 12, down: 13, left: 14, right: 15
  };
  const storagePrefix = 'controller-adder:gamepad-profile:v1:';

  let report: EffectiveReport | null = null;
  let scannedPath = '';
  let error = '';
  let notice = '';
  let loading = false;
  let filter = '';
  let bindings: Record<string, number> = {};
  let dirty = false;
  let requestId = 0;
  let storageReady = false;

  function profileKey(gamePath: string): string {
    return storagePrefix + gamePath;
  }

  function uniqueOptions(input: EffectiveReport): ActionOption[] {
    const result = new Map<string, ActionOption>();
    function add(id: string, kind: ActionOption['kind'], evidence: string) {
      if (!id || result.has(id)) return;
      result.set(id, { id, kind, evidence });
    }

    for (const slot of input.inputs) {
      for (const action of slot.core_default_actions) {
        add(action, 'core', 'RPG Maker default');
      }
    }
    // RPG Maker accepts cancel and menu as Escape-compatible engine actions.
    add('cancel', 'core', 'Engine-compatible action');
    add('menu', 'core', 'Engine-compatible action');

    for (const slot of input.inputs) {
      for (const candidate of slot.plugin_candidates) {
        if (candidate.configuration_status === 'disabled_by_configuration') continue;
        add(candidate.action, 'plugin', `${candidate.plugin} · ${candidate.configuration_status.replaceAll('_', ' ')}`);
      }
    }
    for (const entry of input.candidate_mapped_actions) {
      add(entry.action, 'plugin', 'Unverified plugin mapper');
    }
    for (const entry of input.unresolved_actions) {
      add(entry.action, 'unresolved', `${entry.references} reference(s); no verified mapping`);
    }

    const order: Record<ActionOption['kind'], number> = { core: 0, plugin: 1, unresolved: 2 };
    return [...result.values()].sort((a, b) =>
      order[a.kind] - order[b.kind] || a.id.localeCompare(b.id)
    );
  }

  function readProfile(gamePath: string, engine: string): Record<string, number> {
    if (!storageReady) return {};
    const raw = localStorage.getItem(profileKey(gamePath));
    if (!raw) return {};
    try {
      const value: unknown = JSON.parse(raw);
      if (!value || typeof value !== 'object') return {};
      const profile = value as Partial<Profile>;
      if (profile.format_version !== 1 || profile.engine !== engine || !profile.bindings ||
          typeof profile.bindings !== 'object' || Array.isArray(profile.bindings)) return {};
      const restored: Record<string, number> = {};
      const validActions = new Set(uniqueOptions(report!).map((item) => item.id));
      for (const [action, button] of Object.entries(profile.bindings)) {
        if (validActions.has(action) && typeof button === 'number' &&
            Number.isInteger(button) && button >= 0 && button <= 15) {
          restored[action] = button;
        }
      }
      return restored;
    } catch {
      return {};
    }
  }

  async function scan() {
    const path = $selectedGamePath.trim();
    if (!path) { error = 'Select a game on Home, or enter its path here.'; return; }
    if (dirty && report && scannedPath === path &&
        !confirm('Discard unsaved controller bindings and rescan?')) return;
    const current = ++requestId;
    loading = true;
    error = '';
    notice = '';
    report = null;
    bindings = {};
    try {
      const found = await invoke<EffectiveReport>('scan_effective_inputs', { gamePath: path });
      if (current !== requestId || $selectedGamePath.trim() !== path) return;
      if (found.schema_version !== 2) {
        throw new Error(`Unsupported effective-input schema ${found.schema_version}; expected 2.`);
      }
      report = found;
      scannedPath = path;
      bindings = readProfile(path, found.engine);
      dirty = false;
      notice = Object.keys(bindings).length
        ? 'Saved profile loaded for this game.'
        : 'Scan complete. Choose buttons or apply suggested defaults.';
    } catch (cause) {
      if (current === requestId) error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      if (current === requestId) loading = false;
    }
  }

  function changeButton(action: string, value: string) {
    const updated = { ...bindings };
    if (value === '') delete updated[action];
    else updated[action] = Number(value);
    bindings = updated;
    dirty = true;
    notice = '';
  }

  function applySuggestions() {
    if (!report) return;
    const updated = { ...bindings };
    for (const action of options) {
      if (!(action.id in updated) && action.id in recommended) {
        updated[action.id] = recommended[action.id];
      }
    }
    bindings = updated;
    dirty = true;
    notice = 'Suggested defaults filled in. Review them before saving.';
  }

  function createProfile(): Profile | null {
    if (!report) return null;
    return {
      format_version: 1,
      engine: report.engine,
      game_path: scannedPath,
      bindings: { ...bindings }
    };
  }

  function save() {
    const profile = createProfile();
    if (!profile) return;
    if (!storageReady) { error = 'Local storage is not available.'; return; }
    try {
      localStorage.setItem(profileKey(scannedPath), JSON.stringify(profile));
      dirty = false;
      error = '';
      notice = 'Profile saved locally for this game. No game files were changed.';
    } catch (cause) {
      error = `Could not save profile: ${cause instanceof Error ? cause.message : String(cause)}`;
    }
  }

  function exportProfile() {
    const profile = createProfile();
    if (!profile) return;
    const blob = new Blob([JSON.stringify(profile, null, 2)], { type: 'application/json' });
    const href = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = href;
    anchor.download = 'controller-adder-gamepad-profile.json';
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(href);
    notice = 'JSON export requested. This does not install a patch.';
  }

  onMount(() => { storageReady = typeof localStorage !== 'undefined'; });

  $: options = report ? uniqueOptions(report) : [];
  $: visibleOptions = options.filter((item) =>
    `${item.id} ${item.evidence}`.toLowerCase().includes(filter.trim().toLowerCase())
  );
  $: assigned = Object.keys(bindings).length;
  $: duplicates = Object.entries(
    Object.entries(bindings).reduce<Record<number, string[]>>((groups, [action, index]) => {
      (groups[index] ??= []).push(action);
      return groups;
    }, {})
  ).filter(([, actions]) => actions.length > 1);
  $: missingEssentials = ['ok', 'cancel'].filter((action) =>
    options.some((option) => option.id === action) && !(action in bindings)
  );
  $: if (scannedPath && $selectedGamePath.trim() !== scannedPath) {
    requestId += 1;
    report = null;
    bindings = {};
    dirty = false;
    scannedPath = '';
    notice = '';
  }
</script>

<section class="mapping-editor">
  <header>
    <h1>Controller Mapping Editor</h1>
    <p>Assign gamepad buttons to engine and plugin actions. Profiles are drafts only; nothing is patched into the game.</p>
  </header>

  <form class="scan-panel" on:submit|preventDefault={scan}>
    <label for="mapping-path">Game directory</label>
    <div class="path-row">
      <input id="mapping-path" type="text" bind:value={$selectedGamePath}
        placeholder="Select a game on Home, or paste its directory" spellcheck="false" />
      <button type="submit" disabled={loading || !$selectedGamePath.trim()}>
        {loading ? 'Scanning…' : report ? 'Rescan actions' : 'Scan actions'}
      </button>
    </div>
  </form>

  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  {#if report}
    <div class="summary">
      <span><strong>{report.engine}</strong></span>
      <span>{options.length} actions discovered</span>
      <span>{assigned} assigned</span>
      <span>{dirty ? 'Unsaved changes' : 'No unsaved changes'}</span>
    </div>

    {#if duplicates.length}
      <div class="warning" role="alert">
        <strong>Duplicate button assignments</strong>
        {#each duplicates as [index, actions]}
          <p>{buttonNames.get(Number(index)) ?? `Button ${index}`} → {actions.join(', ')}</p>
        {/each}
        <small>One physical button maps to one action in a simple RPG Maker mapper. Resolve duplicates before generating a future patch.</small>
      </div>
    {/if}
    {#if missingEssentials.length}
      <div class="warning">
        Missing essential mappings: <strong>{missingEssentials.join(', ')}</strong>.
      </div>
    {/if}

    <div class="toolbar">
      <label>Search actions <input type="search" bind:value={filter} placeholder="Action or plugin…" /></label>
      <button type="button" class="secondary" on:click={applySuggestions}>Fill suggested defaults</button>
      <button type="button" on:click={save}>Save profile locally</button>
      <button type="button" class="secondary" on:click={exportProfile}>Export JSON</button>
    </div>

    <div class="rows">
      {#each visibleOptions as item (item.id)}
        <div class="action-row">
          <div class="action-info">
            <strong>{item.id}</strong>
            <span class="kind">{item.kind}</span>
            <small>{item.evidence}</small>
          </div>
          <label>
            <span class="sr-only">Gamepad button for {item.id}</span>
            <select value={item.id in bindings ? String(bindings[item.id]) : ''}
              on:change={(event) => changeButton(item.id, event.currentTarget.value)}>
              <option value="">Unassigned</option>
              {#each buttons as button}
                <option value={button.index}>{button.index} — {button.label}</option>
              {/each}
            </select>
          </label>
        </div>
      {:else}
        <p>No actions match your search.</p>
      {/each}
    </div>

    <p class="footnote">D-pad directions and stick movement may already work through the engine's built-in handling; these assignments are editable suggestions, not detected runtime mappings. Triggers and nonstandard controllers may require a future input-handler extension. Saved profiles are local to this app installation; Export JSON for a portable copy.</p>
  {:else}
    <p class="empty">Select a game and scan its actions to begin. A game does not need pre-existing controller support.</p>
  {/if}
</section>

<style>
  .mapping-editor { max-width:1100px; margin:0 auto; padding:1.25rem; display:grid; gap:1rem; }
  header h1 { margin:0 0 .3rem; font-size:1.55rem; }
  header p, .footnote, .empty { opacity:.75; font-size:.9rem; line-height:1.5; }
  .scan-panel, .warning, .summary { border:1px solid #8885; border-radius:.75rem; padding:1rem; }
  .scan-panel { display:grid; gap:.5rem; }
  .path-row { display:flex; gap:.5rem; }
  input, select { background:transparent; color:inherit; border:1px solid #8888; border-radius:.5rem; padding:.6rem; min-width:0; }
  select option { color:#141414; }
  .path-row input { flex:1; }
  button { background:#6246ea; color:white; border:0; border-radius:.5rem; padding:.65rem .85rem; font-weight:600; cursor:pointer; }
  button:disabled { opacity:.5; cursor:not-allowed; }
  button.secondary { background:#353545; }
  button:hover:not(:disabled) { filter:brightness(1.1); }
  .summary { display:flex; gap:1rem; flex-wrap:wrap; }
  .summary span { padding-right:.5rem; }
  .toolbar { display:flex; flex-wrap:wrap; gap:.6rem; align-items:end; }
  .toolbar label { display:grid; gap:.3rem; font-size:.85rem; flex:1; min-width:170px; }
  .rows { display:grid; gap:.5rem; }
  .action-row { display:flex; justify-content:space-between; align-items:center; gap:1rem; padding:.85rem 1rem; border:1px solid #8884; border-radius:.6rem; }
  .action-info { display:flex; align-items:center; flex-wrap:wrap; column-gap:.5rem; row-gap:.3rem; }
  .action-info small { flex-basis:100%; opacity:.7; }
  .kind { border-radius:1rem; padding:.15rem .5rem; font-size:.75rem; background:#7774; }
  .action-row select { min-width:220px; max-width:100%; }
  .warning { border-color:#d9a04a88; background:#d9a04a0c; }
  .warning p { margin:.4rem 0; }
  .warning small { opacity:.8; }
  .error { color:#f27676; }
  .notice { color:#7eb6f2; }
  .sr-only { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  @media (max-width:650px) { .path-row,.action-row { flex-direction:column; align-items:stretch; } .action-row select { width:100%; } }
</style>
