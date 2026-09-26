<script lang="ts">
  import { store } from '$lib/stores.svelte';
  import { syncNow } from '$lib/api';
  import { toast } from '$lib/toast.svelte';

  const labels: Record<string, string> = {
    idle: '未同步',
    syncing: '同步中',
    ok: '已同步',
    error: '同步失败',
  };

  let busy = $state(false);

  async function doSync() {
    if (busy || store.syncStatus === 'syncing') return;
    busy = true;
    store.syncStatus = 'syncing';
    try {
      await syncNow();
      store.syncStatus = 'ok';
      await store.loadEntries();
    } catch (e) {
      store.syncStatus = 'error';
      const msg = e instanceof Error ? e.message : String(e);
      toast.show(`同步失败: ${msg}`);
    } finally {
      busy = false;
    }
  }
</script>

<span class="sync-bar" data-testid="sync-bar">
  <span class="sync-label" data-testid="sync-label">{labels[store.syncStatus] ?? ''}</span>
  <button
    class="sync-btn"
    data-testid="sync-btn"
    onclick={doSync}
    disabled={busy || store.syncStatus === 'syncing'}
  >同步</button>
</span>
