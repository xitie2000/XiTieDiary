import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { listEntries, saveEntry, createDraftEntry, deleteEntry } from './api';
import type { Entry } from './types';

const sampleEntry: Entry = {
  id: 'e1',
  date: '2026-09-26',
  content: '你好',
  created_at: 1000,
  updated_at: 2000,
  deleted: false,
};

describe('api', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('api_calls_invoke_with_correct_command_and_args', async () => {
    invoke.mockResolvedValue([]);
    await listEntries('2026-01-01', '2026-09-30');
    expect(invoke).toHaveBeenCalledWith('list_entries', {
      from: '2026-01-01',
      to: '2026-09-30',
    });

    invoke.mockResolvedValue([sampleEntry]);
    await listEntries();
    expect(invoke).toHaveBeenCalledWith('list_entries', { from: null, to: null });

    invoke.mockResolvedValue(sampleEntry);
    await saveEntry('e1', '2026-09-26', '你好');
    expect(invoke).toHaveBeenCalledWith('save_entry', {
      id: 'e1',
      date: '2026-09-26',
      content: '你好',
    });

    invoke.mockResolvedValue(sampleEntry);
    await createDraftEntry('2026-09-26');
    expect(invoke).toHaveBeenCalledWith('create_draft_entry', { date: '2026-09-26' });

    invoke.mockResolvedValue(null);
    await deleteEntry('e1');
    expect(invoke).toHaveBeenCalledWith('delete_entry', { id: 'e1' });
  });

  it('api_throws_with_code_on_error_dto', async () => {
    invoke.mockRejectedValue({
      code: 'config',
      message: '未找到同步配置 local.json',
    });
    await expect(listEntries()).rejects.toMatchObject({
      message: '未找到同步配置 local.json',
      cause: 'config',
    });
  });
});
