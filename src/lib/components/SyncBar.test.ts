import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import SyncBar from './SyncBar.svelte';
import { store } from '../stores.svelte';

const mocks = vi.hoisted(() => ({
  syncNow: vi.fn(),
  listEntries: vi.fn(),
  mediaCounts: vi.fn(),
}));

vi.mock('$lib/api', () => mocks);

beforeEach(() => {
  vi.resetAllMocks();
  mocks.listEntries.mockResolvedValue([]);
  mocks.mediaCounts.mockResolvedValue({});
  store.syncStatus = 'idle';
});

afterEach(cleanup);

describe('SyncBar', () => {
  it('renders_four_states', () => {
    const labels: Record<string, string> = {
      idle: '未同步',
      syncing: '同步中',
      ok: '已同步',
      error: '同步失败',
    };
    for (const [status, text] of Object.entries(labels)) {
      store.syncStatus = status as typeof store.syncStatus;
      render(SyncBar);
      expect(screen.getByTestId('sync-label').textContent).toBe(text);
      cleanup();
    }
  });

  it('click_triggers_sync_and_sets_status', async () => {
    mocks.syncNow.mockResolvedValue({
      downloaded_entries: 0,
      uploaded_entries: 0,
      downloaded_media: 0,
      uploaded_media: 0,
      conflicts: 0,
    });
    render(SyncBar);
    await fireEvent.click(screen.getByTestId('sync-btn'));
    expect(mocks.syncNow).toHaveBeenCalledTimes(1);
    expect(store.syncStatus).toBe('ok');
    expect(mocks.listEntries).toHaveBeenCalled();
  });

  it('click_failure_sets_error', async () => {
    mocks.syncNow.mockRejectedValue(new Error('同步失败: x', { cause: 'sync' }));
    render(SyncBar);
    await fireEvent.click(screen.getByTestId('sync-btn'));
    expect(store.syncStatus).toBe('error');
    expect(screen.getByTestId('sync-label').textContent).toBe('同步失败');
  });
});
