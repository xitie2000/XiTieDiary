<script lang="ts">
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import {
    syncNow,
    getConfigStatus,
    saveConfig,
    importConfigFromPath,
  } from '$lib/api';
  import { store } from '$lib/stores.svelte';
  import { toast } from '$lib/toast.svelte';

  const labels: Record<string, string> = {
    idle: '未同步',
    syncing: '同步中',
    ok: '已同步',
    error: '同步失败',
  };

  let busy = $state(false);
  let configInput: HTMLInputElement | undefined = $state();

  const isAndroid = /android/i.test(navigator.userAgent);

  function errorMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

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
      toast.show(`同步失败: ${errorMessage(e)}`);
    } finally {
      busy = false;
    }
  }

  async function afterImport() {
    toast.show('配置已导入，开始同步', 'ok');
    await store.loadConfigStatus();
    await doSync();
  }

  async function importConfig() {
    if (isAndroid) {
      configInput?.click();
      return;
    }
    try {
      const picked = await openFileDialog({
        multiple: false,
        filters: [{ name: 'XiTieDiary 配置', extensions: ['json'] }],
      });
      const path = typeof picked === 'string' ? picked : null;
      if (!path) return;
      await importConfigFromPath(path);
      await afterImport();
    } catch (e) {
      toast.show(`配置导入失败: ${errorMessage(e)}`);
    }
  }

  async function onConfigPicked(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0] ?? null;
    input.value = '';
    if (!file) return;
    try {
      const content = await file.text();
      await saveConfig(content);
      await afterImport();
    } catch (e) {
      toast.show(`配置导入失败: ${errorMessage(e)}`);
    }
  }
</script>

<span class="sync-bar" data-testid="sync-bar">
  <span class="sync-label" data-testid="sync-label">{labels[store.syncStatus] ?? ''}</span>
  {#if store.configStatus && !store.configStatus.configured}
    <button
      class="sync-btn"
      data-testid="import-config-btn"
      onclick={importConfig}
    >导入配置</button>
  {/if}
  <button
    class="sync-btn"
    data-testid="sync-btn"
    onclick={doSync}
    disabled={busy || store.syncStatus === 'syncing'}
  >同步</button>
  <input
    type="file"
    accept=".json,application/json"
    class="hidden-file-input"
    data-testid="config-file-input"
    bind:this={configInput}
    onchange={onConfigPicked}
  />
</span>
