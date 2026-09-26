<script lang="ts">
  import { onMount } from 'svelte';
  import { getEntry, saveEntry, cleanupEmptyDrafts, deleteEntry } from '$lib/api';
  import type { Entry } from '$lib/types';

  interface Props {
    entryId: string;
    onclose: () => void;
  }

  let { entryId, onclose }: Props = $props();

  let entry = $state<Entry | null>(null);
  let content = $state('');
  let date = $state('');
  let dirty = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    entry = await getEntry(entryId);
    if (entry) {
      content = entry.content;
      date = entry.date;
    }
  });

  function scheduleSave() {
    dirty = true;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(doSave, 800);
  }

  async function doSave() {
    clearTimeout(saveTimer);
    if (!dirty || entry === null) return;
    dirty = false;
    entry = await saveEntry(entryId, date, content);
  }

  async function close() {
    clearTimeout(saveTimer);
    if (content.trim() === '' && entry !== null && entry.content === '') {
      await cleanupEmptyDrafts();
    } else {
      await doSave();
    }
    onclose();
  }

  async function remove() {
    clearTimeout(saveTimer);
    dirty = false;
    await deleteEntry(entryId);
    onclose();
  }
</script>

<div class="editor">
  <header class="editor-bar">
    <button class="icon-btn" data-testid="close-btn" onclick={close}>←</button>
    <input class="date-input" type="date" bind:value={date} onchange={scheduleSave} />
    <button class="icon-btn danger" data-testid="delete-btn" onclick={remove}>删除</button>
  </header>

  <textarea
    class="editor-textarea"
    data-testid="editor-textarea"
    bind:value={content}
    oninput={scheduleSave}
    placeholder="今天想写点什么…"
  ></textarea>

  <div class="media-area" data-testid="media-area">
    <div class="media-placeholder">图片功能（Task 8 接入）</div>
  </div>
</div>
