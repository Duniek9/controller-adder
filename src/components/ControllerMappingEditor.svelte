<script lang="ts">
  import ReportExport from "./ReportExport.svelte";
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { open, save as chooseSavePath } from '@tauri-apps/plugin-dialog';
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
  type PhysicalInput = { kind: 'button'; index: number } | { kind: 'axis'; index: number; direction: 'negative' | 'positive'; threshold: number };
  type Bindings = Record<string, PhysicalInput[]>;
  type Profile = {
    format_version: 2;
    engine: string;
    game_path: string;
    bindings: Bindings;
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
  const axisOptions = [
    { index: 0, direction: 'negative', label: 'Left stick left' },
    { index: 0, direction: 'positive', label: 'Left stick right' },
    { index: 1, direction: 'negative', label: 'Left stick up' },
    { index: 1, direction: 'positive', label: 'Left stick down' },
    { index: 2, direction: 'negative', label: 'Right stick left' },
    { index: 2, direction: 'positive', label: 'Right stick right' },
    { index: 3, direction: 'negative', label: 'Right stick up' },
    { index: 3, direction: 'positive', label: 'Right stick down' }
  ] as const;

  function inputKey(input: PhysicalInput): string {
    return input.kind === 'button' ? `button:${input.index}` : `axis:${input.index}:${input.direction}`;
  }

  function parseInput(value: string): PhysicalInput | null {
    const button = buttons.find((b) => value === `button:${b.index}`);
    if (button) return { kind: 'button', index: button.index };
    const axis = axisOptions.find((a) => value === `axis:${a.index}:${a.direction}`);
    return axis ? { kind: 'axis', index: axis.index, direction: axis.direction, threshold: 0.5 } : null;
  }

  const storagePrefix = 'controller-adder:gamepad-profile:v1:';

  let report: MappingCatalog | null = null;
  let importedFile = '';
  let validation: Validation | null = null;
  let validationId = 0;
  let busy = false;
  let suggestionWarnings: string[] = [];
  let scannedPath = '';
  let error = '';
  let notice = '';
  let loading = false;
  let filter = '';
  let bindings: Bindings = {};
  let dirty = false;
  let requestId = 0;
  let storageReady = false;

  function profileKey(gamePath: string): string {
    return storagePrefix + gamePath;
  }

  function uniqueOptions(input: MappingCatalog, currentBindings: Bindings): ActionOption[] {
    return [...new Set([...input.actions, ...Object.keys(currentBindings)])].map((id) => {
      const evidence = input.evidence[id] ?? 'unresolved';
      const kind: ActionOption['kind'] = evidence === 'core' ? 'core'
        : evidence === 'unresolved' ? 'unresolved' : 'plugin';
      return { id, kind, evidence: evidence.replaceAll('_', ' ') };
    }).sort((a, b) => {
      const order = { core: 0, plugin: 1, unresolved: 2 };
      return order[a.kind] - order[b.kind] || a.id.localeCompare(b.id);
    });
  }

  function readProfile(gamePath: string, engine: string): Bindings {
    if (!storageReady) return {};
    const raw = localStorage.getItem(profileKey(gamePath));
    if (!raw) return {};
    try {
      const profile = JSON.parse(raw);
      if (![1, 2].includes(profile.format_version) || profile.engine !== engine ||
          !profile.bindings || typeof profile.bindings !== 'object' || Array.isArray(profile.bindings)) return {};
      if (profile.format_version === 2) return profile.bindings as Bindings;
      const restored: Bindings = Object.create(null);
      for (const [action, button] of Object.entries(profile.bindings)) {
        if (typeof button === 'number' && Number.isInteger(button) && button >= 0) {
          restored[action] = [{ kind: 'button', index: button }];
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
    importedFile = '';
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

  function changeInput(action: string, slot: number, value: string) {
    const current = [...(bindings[action] ?? [])];
    const input = parseInput(value);
    if (input) current[slot] = input;
    else current.splice(slot, 1);
    const updated = { ...bindings };
    if (current.length) updated[action] = current;
    else delete updated[action];
    bindings = updated;
    dirty = true;
    notice = '';
  }

  function changeThreshold(action: string, slot: number, threshold: number) {
    const current = [...(bindings[action] ?? [])];
    const input = current[slot];
    if (!input || input.kind !== 'axis' || !Number.isFinite(threshold)) return;
    current[slot] = { ...input, threshold };
    bindings = { ...bindings, [action]: current };
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
      format_version: 2,
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

  async function importProfile() {
    const catalog = report;
    if (!catalog || busy || loading) return;
    const current = requestId;
    busy = true;
    error = '';
    try {
      const directory = await invoke<string | null>('profile_import_directory');
      const selected = await open({
        directory: false, multiple: false,
        title: 'Import controller profile',
        filters: [{ name: 'Controller profile (JSON)', extensions: ['json'] }],
        ...(directory ? { defaultPath: directory } : {})
      });
      if (typeof selected !== 'string' || current !== requestId || report !== catalog) return;
      const profile = await invoke<Profile>('import_mapping_profile', {
        filePath: selected, gamePath: catalog.game_path, engine: catalog.engine
      });
      if (current !== requestId || report !== catalog || scannedPath !== $selectedGamePath.trim()) return;
      if (Object.keys(bindings).length && !confirm('Replace the current controller bindings with this imported profile?')) return;
      bindings = Object.assign(Object.create(null), profile.bindings);
      importedFile = selected;
      dirty = true;
      suggestionWarnings = Object.keys(bindings).filter(action => !catalog.actions.includes(action))
        .map(action => action + ': imported action not found in current scan; review before applying.');
      filter = '';
      notice = 'Imported ' + selected + '. Review, save locally or export the revised profile. No game files changed.';
    } catch (cause) {
      if (current === requestId) error = 'Could not import profile: ' + String(cause);
    } finally {
      busy = false;
    }
  }

  async function exportProfileAs() {
    const profile = createProfile();
    if (!profile || busy) return;
    const current = requestId;
    busy = true;
    error = '';
    try {
      const directory = await invoke<string | null>('profile_import_directory');
      const path = await chooseSavePath({
        title: 'Export revised controller profile',
        filters: [{ name: 'Controller profile (JSON)', extensions: ['json'] }],
        ...(directory ? { defaultPath: directory + '/revised-profile.json' } : {})
      });
      if (!path || current !== requestId) return;
      const written = await invoke<string>('export_mapping_profile_as', { profile, filePath: path });
      if (current === requestId) notice = 'JSON saved: ' + written;
    } catch (cause) {
      if (current === requestId) error = 'Could not export profile: ' + String(cause);
    } finally { busy = false; }
  }

  async function exportProfile() {
    const profile = createProfile();
    if (!profile || busy) return;
    const current = requestId;
    busy = true;
    error = '';
    notice = '';
    try {
      const path = await invoke<string>('export_mapping_profile', { profile });
      if (current === requestId) notice = `JSON saved: ${path}. No game files were changed.`;
    } catch (cause) {
      if (current === requestId) error = `Could not export profile: ${String(cause)}`;
    } finally {
      busy = false;
    }
  }

  onMount(() => { storageReady = typeof localStorage !== 'undefined'; });

  $: options = report ? uniqueOptions(report, bindings) : [];
  $: visibleOptions = options.filter((item) =>
    `${item.id} ${item.evidence}`.toLowerCase().includes(filter.trim().toLowerCase())
  );
  $: assigned = Object.keys(bindings).length;
  async function refreshValidation(catalog: MappingCatalog | null, currentBindings: Bindings) {
    const id = ++validationId;
    validation = null;
    if (!catalog) return;
    try {
      const result = await invoke<Validation>('validate_controller_mapping', {
        profile: { format_version: 2, engine: catalog.engine, game_path: catalog.game_path, bindings: currentBindings }
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
    <p>Assign gamepad buttons and stick directions to engine and plugin actions. Profiles are drafts only; nothing is patched into the game.</p>
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
      {#if importedFile}<span>Imported: {importedFile}</span>{/if}
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
      <button type="button" class="secondary" disabled={busy || loading} on:click={importProfile}>Import JSON</button>
      <button type="button" class="secondary" disabled={busy || !validation?.valid} on:click={exportProfile}>Export JSON</button>
      <button type="button" class="secondary" disabled={busy || !validation?.valid} on:click={exportProfileAs}>Export JSON as…</button>
    </div>

    <div class="rows">
      {#each visibleOptions as item (item.id)}
        <div class="action-row">
          <div class="action-info">
            <strong>{item.id}</strong>
            <span class="kind">{item.kind}</span>
            <small>{item.evidence}</small>
          </div>
          <div class="input-list">
            {#each bindings[item.id] ?? [] as input, slot}
              <div class="input-row">
                <label>
                  <span class="sr-only">Input {slot + 1} for {item.id}</span>
                  <select disabled={busy} value={inputKey(input)}
                    on:change={(event) => changeInput(item.id, slot, event.currentTarget.value)}>
                    <option value="">Remove input</option>
                    {#each buttons as button}
                      <option value={`button:${button.index}`}>{button.label}</option>
                    {/each}
                    {#each axisOptions as axis}
                      <option value={`axis:${axis.index}:${axis.direction}`}>{axis.label}</option>
                    {/each}
                  </select>
                </label>
                {#if input.kind === 'axis'}
                  <label class="threshold">Threshold
                    <input type="number" min="0.05" max="1" step="0.05" value={input.threshold}
                      disabled={busy} on:change={(event) => changeThreshold(item.id, slot, event.currentTarget.valueAsNumber)} />
                  </label>
                {/if}
              </div>
            {/each}
            <label>
              <span class="sr-only">Add input for {item.id}</span>
              <select disabled={busy} value="" on:change={(event) => {
                changeInput(item.id, (bindings[item.id] ?? []).length, event.currentTarget.value);
                event.currentTarget.value = '';
              }}>
                <option value="">+ Add input</option>
                {#each buttons as button}
                  <option value={`button:${button.index}`}>{button.label}</option>
                {/each}
                {#each axisOptions as axis}
                  <option value={`axis:${axis.index}:${axis.direction}`}>{axis.label}</option>
                {/each}
              </select>
            </label>
          </div>
        </div>
      {:else}
        <p>No actions match your search.</p>
      {/each}
    </div>

    <p class="footnote">D-pad directions and stick movement may already work through the engine's built-in handling; these assignments are editable suggestions, not detected runtime mappings. Analog mappings use configurable thresholds and require the future runtime input-handler patch. Saved profiles are local to this app installation; Export JSON for a portable copy.</p>
  {:else}
    <p class="empty">Select a game and scan its actions to begin. A game does not need pre-existing controller support.</p>
  {/if}

  {#if report && scannedPath === $selectedGamePath.trim()}
    <ReportExport gamePath={scannedPath} kind="mapping" report={report} />
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
  .input-list { display:grid; gap:.4rem; }
  .input-row { display:flex; gap:.4rem; align-items:center; flex-wrap:wrap; }
  .threshold { display:flex; align-items:center; gap:.3rem; font-size:.8rem; }
  .threshold input { width:5rem; }
  .action-row select { min-width:220px; max-width:100%; }
  .warning { border-color:#d9a04a88; background:#d9a04a0c; }
  .warning p { margin:.4rem 0; }
  .warning small { opacity:.8; }
  .error { color:#f27676; }
  .notice { color:#7eb6f2; }
  .sr-only { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  @media (max-width:650px) { .path-row,.action-row { flex-direction:column; align-items:stretch; } .action-row select { width:100%; } }
</style>
