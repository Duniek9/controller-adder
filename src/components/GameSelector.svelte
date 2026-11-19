
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';

  type Props = {
    onGameSelected?: (path: string) => void;
  };

  let { onGameSelected }: Props = $props();

  let gamePath = $state<string | null>(null);
  let isSelecting = $state(false);
  let error = $state<string | null>(null);

  async function selectGame() {
    if (isSelecting) return;

    isSelecting = true;
    error = null;

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Game Directory'
      });

      if (selected === null) return;

      gamePath = selected;
      onGameSelected?.(selected);
    } catch (err) {
      error = err instanceof Error
        ? err.message
        : String(err);
    } finally {
      isSelecting = false;
    }
  }
</script>

<div class="game-selector">
  <h2>Select a game</h2>

  <p>
    Choose the folder containing the game you want
    to add controller support to.
  </p>

  <button onclick={selectGame} disabled={isSelecting}>
    {isSelecting ? 'Opening...' : 'Browse Game Files'}
  </button>

  {#if gamePath}
    <div class="selected-game">
      <strong>Selected directory:</strong>
      <p>{gamePath}</p>
    </div>
  {/if}

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .game-selector {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 24px;
    border: 1px solid #414141;
    border-radius: 12px;
    background: #1e1e1e;
    color: #f5f5f5;
  }

  h2, p {
    margin: 0;
  }

  .game-selector > p {
    color: #aaaaaa;
  }

  button {
    padding: 12px 18px;
    border: none;
    border-radius: 8px;
    background: #6854d9;
    color: white;
    font-weight: 600;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: #806cf0;
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .selected-game {
    padding: 12px;
    border-radius: 8px;
    background: #303030;
    overflow-wrap: anywhere;
  }

  .selected-game p {
    margin-top: 6px;
  }

  .error {
    color: #ff7777 !important;
  }
</style>
