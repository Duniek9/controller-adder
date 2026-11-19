import { writable } from 'svelte/store';

/** Shared game directory for the current app session. */
export const selectedGamePath = writable<string>('');
