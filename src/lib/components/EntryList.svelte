<script lang="ts">
  import type { Entry } from '../types';

  interface Props {
    entries: Entry[];
    mediaCounts?: Record<string, number>;
    onselect?: (id: string) => void;
  }

  let { entries, mediaCounts = {}, onselect }: Props = $props();

  const groups = $derived.by(() => {
    const map = new Map<string, Entry[]>();
    for (const e of entries) {
      const list = map.get(e.date) ?? [];
      list.push(e);
      map.set(e.date, list);
    }
    return [...map.entries()];
  });

  function dayLabel(date: string): string {
    const [y, m, d] = date.split('-').map(Number);
    const wd = ['日', '一', '二', '三', '四', '五', '六'][
      new Date(Date.UTC(y, m - 1, d)).getUTCDay()
    ];
    return `${m}月${d}日 周${wd}`;
  }

  function preview(content: string): string {
    const text = content.replace(/\s+/g, ' ').trim();
    return text.length > 50 ? text.slice(0, 50) + '…' : text;
  }
</script>

<div class="entry-list">
  {#if groups.length === 0}
    <div class="empty" data-testid="empty">还没有日记，点右上角「＋」开始写吧</div>
  {/if}
  {#each groups as [date, list] (date)}
    <section class="day-group">
      <h2 class="day-label" data-testid="date-group">{dayLabel(date)}</h2>
      {#each list as e (e.id)}
        <button class="entry-item" data-testid="entry-item" onclick={() => onselect?.(e.id)}>
          <span class="preview">{preview(e.content) || '（无文字）'}</span>
          {#if (mediaCounts[e.id] ?? 0) > 0}
            <span class="media-badge">{mediaCounts[e.id]}张</span>
          {/if}
        </button>
      {/each}
    </section>
  {/each}
</div>
