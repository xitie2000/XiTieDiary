import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import Toast from './Toast.svelte';
import { toast } from '../toast.svelte';
import EntryEditor from './EntryEditor.svelte';

const mocks = vi.hoisted(() => ({
  getEntry: vi.fn(),
  saveEntry: vi.fn(),
  cleanupEmptyDrafts: vi.fn(),
  deleteEntry: vi.fn(),
  insertMedia: vi.fn(),
  insertMediaBytes: vi.fn(),
  listMedia: vi.fn(),
  deleteMedia: vi.fn(),
}));

vi.mock('$lib/api', () => mocks);

afterEach(() => {
  cleanup();
  toast.items = [];
  vi.resetAllMocks();
  vi.useRealTimers();
});

describe('Toast', () => {
  it('renders_and_auto_dismisses', async () => {
    vi.useFakeTimers();
    render(Toast);
    toast.show('同步失败: 网络错误');
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.getByTestId('toast-item').textContent).toBe('同步失败: 网络错误');
    await vi.advanceTimersByTimeAsync(4500);
    expect(screen.queryByTestId('toast-item')).toBeNull();
  });

  it('shows_error_when_image_insert_fails_and_batch_continues', async () => {
    mocks.getEntry.mockResolvedValue({
      id: 'e1',
      date: '2026-09-26',
      content: 'x',
      created_at: 1,
      updated_at: 2,
      deleted: false,
    });
    mocks.listMedia.mockResolvedValue([]);
    // 第一张 HEIC 解码失败，第二张成功 —— 批次不应中断
    mocks.insertMediaBytes
      .mockRejectedValueOnce(new Error('图片解码失败: heic', { cause: 'io' }))
      .mockResolvedValueOnce({
        id: 'm9',
        entry_id: 'e1',
        mime: 'image/jpeg',
        size: 10,
        updated_at: 3,
        deleted: false,
      });

    render(EntryEditor, { props: { entryId: 'e1', onclose: vi.fn() } });
    await new Promise((r) => setTimeout(r, 0));
    await new Promise((r) => setTimeout(r, 0));

    const f1 = new File([new Uint8Array([1])], 'a.heic', { type: 'image/heic' });
    const f2 = new File([new Uint8Array([2])], 'b.png', { type: 'image/png' });
    const input = screen.getByTestId('media-file-input');
    Object.defineProperty(input, 'files', { value: [f1, f2] });
    await fireEvent.change(input);
    await new Promise((r) => setTimeout(r, 10));
    await new Promise((r) => setTimeout(r, 0));

    expect(mocks.insertMediaBytes).toHaveBeenCalledTimes(2);
    expect(toast.items.length).toBe(2);
    expect(toast.items[0].kind).toBe('error');
    expect(toast.items[0].text).toContain('图片解码失败');
  });
});
