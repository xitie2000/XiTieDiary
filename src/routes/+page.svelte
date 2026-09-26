<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { store } from '$lib/stores.svelte';
  import { createDraftEntry, syncNow } from '$lib/api';
  import { toast } from '$lib/toast.svelte';
  import EntryList from '$lib/components/EntryList.svelte';
  import EntryEditor from '$lib/components/EntryEditor.svelte';
  import SyncBar from '$lib/components/SyncBar.svelte';
  import Toast from '$lib/components/Toast.svelte';

  let view = $state<{ page: 'list' } | { page: 'editor'; id: string }>({ page: 'list' });
  let creating = $state(false);

  onMount(async () => {
    store.loadEntries();
    const unlisten = await listen<{ status: string; message?: string }>(
      'sync://status',
      (e) => {
        const { status, message } = e.payload;
        if (status === 'syncing' || status === 'ok' || status === 'error') {
          store.syncStatus = status;
          if (status === 'ok') {
            store.loadEntries();
          }
          if (status === 'error' && message) {
            toast.show(`同步失败: ${message}`);
          }
        }
      }
    );
    syncNow().catch(() => {});
    return unlisten;
  });

  function todayLocal(): string {
    const d = new Date();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${mm}-${dd}`;
  }

  async function newEntry() {
    if (creating) return;
    creating = true;
    try {
      const e = await createDraftEntry(todayLocal());
      view = { page: 'editor', id: e.id };
    } finally {
      creating = false;
    }
  }

  function backToList() {
    store.loadEntries();
    view = { page: 'list' };
  }
</script>

<main>
  <header class="topbar">
    <span class="sync-slot" data-testid="sync-slot"><SyncBar /></span>
    <button class="new-btn" data-testid="new-btn" onclick={newEntry} disabled={creating}>
      ＋ 新写一篇
    </button>
  </header>

  {#if view.page === 'list'}
    <EntryList
      entries={store.entries}
      mediaCounts={store.mediaCounts}
      onselect={(id) => (view = { page: 'editor', id })}
    />
  {:else}
    <EntryEditor entryId={view.id} onclose={backToList} />
  {/if}
</main>

<Toast />
