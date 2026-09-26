<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import {
    getEntry,
    saveEntry,
    cleanupEmptyDrafts,
    deleteEntry,
    insertMedia,
    listMedia,
    deleteMedia,
  } from '$lib/api';
  import type { Entry, MediaWithUrl } from '$lib/types';

  interface Props {
    entryId: string;
    onclose: () => void;
  }

  let { entryId, onclose }: Props = $props();

  let entry = $state<Entry | null>(null);
  let content = $state('');
  let date = $state('');
  let media = $state<MediaWithUrl[]>([]);
  let dirty = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    entry = await getEntry(entryId);
    if (entry) {
      content = entry.content;
      date = entry.date;
    }
    await loadMedia();
  });

  async function loadMedia() {
    media = await listMedia(entryId);
  }

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
    if (content.trim() === '' && entry !== null && entry.content === '' && media.length === 0) {
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

  async function addMedia() {
    const picked = await openFileDialog({
      multiple: true,
      filters: [
        { name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] },
      ],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length === 0) return;
    for (const p of paths) {
      await insertMedia(entryId, p);
    }
    await loadMedia();
  }

  async function removeMedia(id: string) {
    await deleteMedia(id);
    await loadMedia();
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
    <div class="thumbs">
      {#each media as m (m.id)}
        <div class="thumb-wrap" data-testid="media-thumb-wrap">
          <img class="thumb" data-testid="media-thumb" src={convertFileSrc(m.url_path)} alt={m.id} />
          <button
            class="thumb-remove"
            data-testid="media-remove-btn"
            onclick={() => removeMedia(m.id)}
          >×</button>
        </div>
      {/each}
    </div>
    <button class="add-media-btn" data-testid="add-media-btn" onclick={addMedia}>＋ 图片</button>
  </div>
</div>
