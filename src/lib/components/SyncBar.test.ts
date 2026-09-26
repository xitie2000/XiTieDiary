import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import SyncBar from './SyncBar.svelte';
import { store } from '../stores.svelte';

const mocks = vi.hoisted(() => ({
  syncNow: vi.fn(),
  listEntries: vi.fn(),
  mediaCounts: vi.fn(),
  getConfigStatus: vi.fn(),
  importConfigFromPath: vi.fn(),
  saveConfig: vi.fn(),
}));

const dialogMocks = vi.hoisted(() => ({ open: vi.fn() }));

vi.mock('$lib/api', () => mocks);
vi.mock('@tauri-apps/plugin-dialog', () => dialogMocks);

const emptyReport = {
  downloaded_entries: 0,
  uploaded_entries: 0,
  downloaded_media: 0,
  uploaded_media: 0,
  conflicts: 0,
  skipped_objects: 0,
};

beforeEach(() => {
  vi.resetAllMocks();
  mocks.listEntries.mockResolvedValue([]);
  mocks.mediaCounts.mockResolvedValue({});
  mocks.getConfigStatus.mockResolvedValue({ configured: true, provider: 'oss', bucket: 'b' });
  store.syncStatus = 'idle';
  store.configStatus = null;
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
    mocks.syncNow.mockResolvedValue(emptyReport);
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

  it('shows_import_button_when_unconfigured', () => {
    store.configStatus = { configured: false, provider: null, bucket: null };
    render(SyncBar);
    expect(screen.getByTestId('import-config-btn')).toBeTruthy();
  });

  it('configured_hides_import_button', () => {
    store.configStatus = { configured: true, provider: 'oss', bucket: 'b' };
    render(SyncBar);
    expect(screen.queryByTestId('import-config-btn')).toBeNull();
  });

  it('desktop_import_uses_dialog_then_path', async () => {
    store.configStatus = { configured: false, provider: null, bucket: null };
    dialogMocks.open.mockResolvedValue('C:/cfg/local.json');
    mocks.importConfigFromPath.mockResolvedValue(undefined);
    mocks.syncNow.mockResolvedValue(emptyReport);
    render(SyncBar);
    await fireEvent.click(screen.getByTestId('import-config-btn'));
    await new Promise((r) => setTimeout(r, 10));
    await new Promise((r) => setTimeout(r, 0));
    expect(dialogMocks.open).toHaveBeenCalled();
    expect(mocks.importConfigFromPath).toHaveBeenCalledWith('C:/cfg/local.json');
    expect(mocks.getConfigStatus).toHaveBeenCalled();
    expect(mocks.syncNow).toHaveBeenCalled();
  });

  it('android_import_reads_file_and_saves', async () => {
    store.configStatus = { configured: false, provider: null, bucket: null };
    mocks.saveConfig.mockResolvedValue(undefined);
    mocks.syncNow.mockResolvedValue(emptyReport);
    render(SyncBar);
    const file = new File(['{"provider":"oss"}'], 'local.json', {
      type: 'application/json',
    });
    const input = screen.getByTestId('config-file-input');
    Object.defineProperty(input, 'files', { value: [file] });
    await fireEvent.change(input);
    await new Promise((r) => setTimeout(r, 10));
    await new Promise((r) => setTimeout(r, 0));
    expect(mocks.saveConfig).toHaveBeenCalledWith('{"provider":"oss"}');
    expect(mocks.syncNow).toHaveBeenCalled();
  });
});
