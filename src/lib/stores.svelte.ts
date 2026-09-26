import { getConfigStatus, listEntries, mediaCounts } from './api';
import type { ConfigStatus, Entry } from './types';

export type SyncStatus = 'idle' | 'syncing' | 'ok' | 'error';

class DiaryStore {
  entries: Entry[] = $state([]);
  mediaCounts: Record<string, number> = $state({});
  syncStatus: SyncStatus = $state('idle');
  configStatus: ConfigStatus | null = $state(null);

  async loadEntries(): Promise<void> {
    const [entries, counts] = await Promise.all([listEntries(), mediaCounts()]);
    this.entries = entries;
    this.mediaCounts = counts;
  }

  async loadConfigStatus(): Promise<void> {
    this.configStatus = await getConfigStatus();
  }
}

export const store = new DiaryStore();
