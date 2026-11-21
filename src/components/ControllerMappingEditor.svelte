<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { selectedGamePath } from '../lib/stores/selectedGame';

  type MappingCatalog = {
    engine: string;
    game_path: string;
    actions: string[];
    evidence: Record<string, string>;
    warnings: string[];
  };
  type Validation = {
    valid: boolean;
    errors: { code: string; message: string }[];
    warnings: { code: string; message: string }[];
  };
  type Suggestion = { profile: Profile; unassigned: string[]; added: string[]; warnings: string[] };
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
  const storagePrefix = 'controller-adder:gamepad-profile:v1:';

  let report: MappingCatalog | null = null;
  let validation: Validation | null = null;
  let validationId = 0;
  let busy = false;
  let suggestionWarnings: string[] = [];
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

  function uniqueOptions(input: MappingCatalog): ActionOption[] {
    return input.actions.map((id) => {
      const evidence = input.evidence[id] ?? 'unresolved';
      const kind: ActionOption['kind'] = evidence === 'core' ? 'core'
        : evidence === 'unresolved' ? 'unresolved' : 'plugin';
      return { id, kind, evidence: evidence.replaceAll('_', ' ') };
    }).sort((a, b) => {
      const order = { core: 0, plugin: 1, unresolved: 2 };
      return order[a.kind] - order[b.kind] || a.id.localeCompare(b.id);
    });
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
      for (const [action, button] of Object.entries(profile.bindings)) {
        if (typeof button === 'number' && Number.isInteger(button) && button >= 0) {
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
      const found = await invoke<MappingCatalog>('scan_mapping_actions', { gamePath: path });
      if (current !== requestId || $selectedGamePath.trim() !== path) return;

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

  async function applySuggestions(mode: 'automatic' | 'conservative') {
    const profile = createProfile();
    const catalog = report;
    if (!profile || !catalog || busy) return;
    const current = requestId;
    busy = true;
    error = '';
    try {
      const result = await invoke<Suggestion>('suggest_controller_mapping', { catalog, profile, mode });
      if (current !== requestId || report !== catalog) return;
      bindings = result.profile.bindings;
      suggestionWarnings = result.warnings;
      dirty = true;
      notice = `Added ${result.added.length} bindings; ${result.unassigned.length} detected actions remain unassigned. Review before saving.`;
    } catch (cause) {
      if (current === requestId) error = String(cause);
    } finally {
      busy = false;
    }
  }

  function createProfile(): Profile | null {
    if (!report) return null;
    return {
      format_version: 1,
      engine: report.engine,
      game_path: report.game_path,
      bindings: { ...bindings }
    };
  }

  async function save() {
    const profile = createProfile();
    if (!profile) return;
    if (!storageReady) { error = 'Local storage is not available.'; return; }
    try {
      const checked = await invoke<Validation>('validate_controller_mapping', { profile });
      if (!checked.valid) { error = 'Resolve validation errors before saving.'; return; }
      if (JSON.stringify(profile) !== JSON.stringify(createProfile())) return;
      localStorage.setItem(profileKey(scannedPath), JSON.stringify(profile));
      dirty = false;
      error = '';
      notice = 'Profile saved locally for this game. No game files were changed.';
    } catch (cause) {
      error = `Could not save profile: ${cause instanceof Error ? cause.message : String(cause)}`;
    }
  }

  async function exportProfile() {
    const profile = createProfile();
    if (!profile) return;
    try {
      const checked = await invoke<Validation>('validate_controller_mapping', { profile });
      if (!checked.valid) { error = 'Resolve validation errors before exporting.'; return; }
      if (JSON.stringify(profile) !== JSON.stringify(createProfile())) return;
    } catch (cause) { error = String(cause); return; }
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
  async function refreshValidation(catalog: MappingCatalog | null, currentBindings: Record<string, number>) {
    const id = ++validationId;
    validation = null;
    if (!catalog) return;
    try {
      const result = await invoke<Validation>('validate_controller_mapping', {
        profile: { format_version: 1, engine: catalog.engine, game_path: catalog.game_path, bindings: currentBindings }
      });
      if (id === validationId) validation = result;
    } catch (cause) {
      if (id === validationId) error = String(cause);
    }
  }
  $: void refreshValidation(report, bindings);
  $: if (scannedPath && $selectedGamePath.trim() !== scannedPath) {
    requestId += 1;
    report = null;
    bindings = {};
    dirty = false;
    scannedPath = '';
    notice = '';
    suggestionWarnings = [];
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
      <button type="submit" disabled={busy || loading || !$selectedGamePath.trim()}>
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

    {#if validation && validation.errors.length}
      <div class="warning" role="alert">
        <strong>Invalid bindings</strong>
        {#each validation.errors as issue}<p>{issue.message}</p>{/each}
      </div>
    {/if}
    {#if validation && validation.warnings.length}
      <div class="warning">
        {#each validation.warnings as issue}<p>{issue.message}</p>{/each}
      </div>
    {/if}
    {#if suggestionWarnings.length}
      <div class="warning">
        {#each suggestionWarnings as warning}<p>{warning}</p>{/each}
      </div>
    {/if}

    <div class="toolbar">
      <label>Search actions <input type="search" bind:value={filter} placeholder="Action or plugin…" /></label>
      <button type="button" class="secondary" disabled={busy || !validation?.valid} on:click={() => applySuggestions('automatic')}>Auto-assign plugin actions</button>
      <button type="button" class="secondary" disabled={busy || !validation?.valid} on:click={() => applySuggestions('conservative')}>Fill standard defaults</button>
      <button type="button" disabled={busy || !validation?.valid} on:click={save}>Save profile locally</button>
      <button type="button" class="secondary" disabled={busy || !validation?.valid} on:click={exportProfile}>Export JSON</button>
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
            <select disabled={busy} value={item.id in bindings ? String(bindings[item.id]) : ''}
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
