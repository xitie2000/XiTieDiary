import { listEntries, mediaCounts } from './api';
import type { Entry } from './types';

export type SyncStatus = 'idle' | 'syncing' | 'ok' | 'error';

class DiaryStore {
  entries: Entry[] = $state([]);
  mediaCounts: Record<string, number> = $state({});
  syncStatus: SyncStatus = $state('idle');

  async loadEntries(): Promise<void> {
    const [entries, counts] = await Promise.all([listEntries(), mediaCounts()]);
    this.entries = entries;
    this.mediaCounts = counts;
  }
}

export const store = new DiaryStore();
