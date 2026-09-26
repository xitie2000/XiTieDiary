<script lang="ts">
  import { onMount } from 'svelte';
  import { store } from '$lib/stores.svelte';
  import { createDraftEntry } from '$lib/api';
  import EntryList from '$lib/components/EntryList.svelte';
  import EntryEditor from '$lib/components/EntryEditor.svelte';

  let view = $state<{ page: 'list' } | { page: 'editor'; id: string }>({ page: 'list' });
  let creating = $state(false);

  onMount(() => {
    store.loadEntries();
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
    <span class="sync-slot" data-testid="sync-slot"></span>
    <button class="new-btn" data-testid="new-btn" onclick={newEntry} disabled={creating}>
      ＋ 新写一篇
    </button>
  </header>

  {#if view.page === 'list'}
    <EntryList entries={store.entries} onselect={(id) => (view = { page: 'editor', id })} />
  {:else}
    <EntryEditor entryId={view.id} onclose={backToList} />
  {/if}
</main>
