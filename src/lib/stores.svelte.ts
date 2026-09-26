import { listEntries } from './api';
import type { Entry } from './types';

export type SyncStatus = 'idle' | 'syncing' | 'ok' | 'error';

class DiaryStore {
  entries: Entry[] = $state([]);
  syncStatus: SyncStatus = $state('idle');

  async loadEntries(): Promise<void> {
    this.entries = await listEntries();
  }
}

export const store = new DiaryStore();
