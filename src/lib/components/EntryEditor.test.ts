import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import EntryEditor from './EntryEditor.svelte';
import type { Entry } from '../types';

const mocks = vi.hoisted(() => ({
  getEntry: vi.fn(),
  saveEntry: vi.fn(),
  cleanupEmptyDrafts: vi.fn(),
  deleteEntry: vi.fn(),
}));

vi.mock('$lib/api', () => mocks);

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

beforeEach(() => {
  vi.resetAllMocks();
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
  await Promise.resolve();
  await Promise.resolve();
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
});
