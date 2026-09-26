import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import EntryEditor from './EntryEditor.svelte';
import type { Entry } from '../types';

const mocks = vi.hoisted(() => ({
  getEntry: vi.fn(),
  saveEntry: vi.fn(),
  cleanupEmptyDrafts: vi.fn(),
  deleteEntry: vi.fn(),
  insertMedia: vi.fn(),
  listMedia: vi.fn(),
  deleteMedia: vi.fn(),
}));

vi.mock('$lib/api', () => mocks);

const dialogMocks = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => dialogMocks);

beforeAll(() => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    convertFileSrc: (p: string) => `asset://${p}`,
  };
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

beforeEach(() => {
  vi.resetAllMocks();
  mocks.listMedia.mockResolvedValue([]);
});

function mockEntry(over: Partial<Entry> = {}): Entry {
  return {
    id: 'e1',
    date: '2026-09-26',
    content: '',
    created_at: 1,
    updated_at: 2,
    deleted: false,
    ...over,
  };
}

async function flush() {
  for (let i = 0; i < 8; i++) {
    await Promise.resolve();
  }
}

function mediaItem(over: Partial<import('../types').MediaWithUrl> = {}): import('../types').MediaWithUrl {
  return {
    id: 'm1',
    entry_id: 'e1',
    mime: 'image/jpeg',
    size: 100,
    updated_at: 5,
    deleted: false,
    url_path: 'C:\\data\\media\\m1.jpg',
    ...over,
  };
}

describe('EntryEditor', () => {
  it('typing_debounces_save', async () => {
    vi.useFakeTimers();
    mocks.getEntry.mockResolvedValue(mockEntry());
    mocks.saveEntry.mockResolvedValue(mockEntry({ content: 'AB' }));
    const onclose = vi.fn();
    render(EntryEditor, { props: { entryId: 'e1', onclose } });
    await vi.advanceTimersByTimeAsync(0);

    const ta = screen.getByTestId('editor-textarea');
    await fireEvent.input(ta, { target: { value: 'A' } });
    await fireEvent.input(ta, { target: { value: 'AB' } });
    expect(mocks.saveEntry).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(800);
    expect(mocks.saveEntry).toHaveBeenCalledTimes(1);
    expect(mocks.saveEntry).toHaveBeenCalledWith('e1', '2026-09-26', 'AB');
  });

  it('close_calls_cleanup_when_empty', async () => {
    mocks.getEntry.mockResolvedValue(mockEntry());
    mocks.cleanupEmptyDrafts.mockResolvedValue(0);
    const onclose = vi.fn();
    render(EntryEditor, { props: { entryId: 'e1', onclose } });
    await flush();

    await fireEvent.click(screen.getByTestId('close-btn'));
    expect(mocks.cleanupEmptyDrafts).toHaveBeenCalledTimes(1);
    expect(mocks.saveEntry).not.toHaveBeenCalled();
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it('delete_button_calls_delete_entry', async () => {
    mocks.getEntry.mockResolvedValue(mockEntry({ content: 'x' }));
    mocks.deleteEntry.mockResolvedValue(undefined);
    const onclose = vi.fn();
    render(EntryEditor, { props: { entryId: 'e1', onclose } });
    await flush();

    await fireEvent.click(screen.getByTestId('delete-btn'));
    expect(mocks.deleteEntry).toHaveBeenCalledWith('e1');
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it('renders_media_thumbnails', async () => {
    mocks.getEntry.mockResolvedValue(mockEntry({ content: 'x' }));
    mocks.listMedia.mockResolvedValue([mediaItem()]);
    render(EntryEditor, { props: { entryId: 'e1', onclose: vi.fn() } });
    await flush();

    const img = screen.getByTestId('media-thumb');
    expect(img.getAttribute('src')).toContain('m1.jpg');
  });

  it('add_button_opens_dialog_and_inserts', async () => {
    mocks.getEntry.mockResolvedValue(mockEntry({ content: 'x' }));
    mocks.listMedia.mockResolvedValue([]);
    dialogMocks.open.mockResolvedValue(['C:\\pics\\a.png', 'C:\\pics\\b.png']);
    mocks.insertMedia.mockResolvedValue(mediaItem());
    render(EntryEditor, { props: { entryId: 'e1', onclose: vi.fn() } });
    await flush();

    await fireEvent.click(screen.getByTestId('add-media-btn'));
    await flush();
    expect(dialogMocks.open).toHaveBeenCalled();
    expect(mocks.insertMedia).toHaveBeenCalledWith('e1', 'C:\\pics\\a.png');
    expect(mocks.insertMedia).toHaveBeenCalledWith('e1', 'C:\\pics\\b.png');
    expect(mocks.listMedia).toHaveBeenCalledTimes(2);
  });

  it('remove_media_calls_delete', async () => {
    mocks.getEntry.mockResolvedValue(mockEntry({ content: 'x' }));
    mocks.listMedia.mockResolvedValue([mediaItem()]);
    mocks.deleteMedia.mockResolvedValue(undefined);
    render(EntryEditor, { props: { entryId: 'e1', onclose: vi.fn() } });
    await flush();

    await fireEvent.click(screen.getByTestId('media-remove-btn'));
    await flush();
    expect(mocks.deleteMedia).toHaveBeenCalledWith('m1');
    expect(mocks.listMedia).toHaveBeenCalledTimes(2);
  });
});
