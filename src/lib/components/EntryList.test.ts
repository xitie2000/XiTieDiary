import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import EntryList from './EntryList.svelte';
import type { Entry } from '../types';

afterEach(cleanup);

function entry(id: string, date: string, content: string, updated = 2): Entry {
  return { id, date, content, created_at: 1, updated_at: updated, deleted: false };
}

describe('EntryList', () => {
  it('renders_date_groups_desc', () => {
    render(EntryList, {
      props: {
        entries: [
          entry('e3', '2026-09-26', 'same day second', 3),
          entry('e1', '2026-09-26', '今天心情不错'),
          entry('e2', '2026-09-25', 'yesterday'),
        ],
      },
    });
    const groups = screen.getAllByTestId('date-group');
    expect(groups).toHaveLength(2);
    expect(groups[0].textContent).toBe('9月26日 周六');
    expect(groups[1].textContent).toBe('9月25日 周五');
    const items = screen.getAllByTestId('entry-item');
    expect(items).toHaveLength(3);
    expect(items[0].textContent).toContain('same day second');
  });

  it('shows_preview_and_media_count', () => {
    const long = 'A'.repeat(60);
    render(EntryList, {
      props: {
        entries: [entry('e2', '2026-09-25', long)],
        mediaCounts: { e2: 3 },
      },
    });
    expect(screen.getByText('A'.repeat(50) + '…')).toBeTruthy();
    expect(screen.getByText('3张')).toBeTruthy();
  });

  it('click_entry_fires_select', async () => {
    const onselect = vi.fn();
    render(EntryList, {
      props: { entries: [entry('e1', '2026-09-26', 'hi')], onselect },
    });
    await fireEvent.click(screen.getByTestId('entry-item'));
    expect(onselect).toHaveBeenCalledWith('e1');
  });
});
