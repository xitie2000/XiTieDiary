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
    insertMediaBytes,
    listMedia,
    deleteMedia,
  } from '$lib/api';
  import { toast } from '$lib/toast.svelte';
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
  let fileInput: HTMLInputElement | undefined = $state();

  const isAndroid = /android/i.test(navigator.userAgent);

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
    try {
      if (content.trim() === '' && entry !== null && entry.content === '' && media.length === 0) {
        await cleanupEmptyDrafts();
      } else {
        await doSave();
      }
    } catch (e) {
      toast.show(`保存失败: ${errorMessage(e)}`);
    }
    onclose();
  }

  async function remove() {
    clearTimeout(saveTimer);
    dirty = false;
    try {
      await deleteEntry(entryId);
    } catch (e) {
      toast.show(`删除失败: ${errorMessage(e)}`);
    }
    onclose();
  }

  async function addMedia() {
    if (isAndroid) {
      fileInput?.click();
      return;
    }
    try {
      const picked = await openFileDialog({
        multiple: true,
        filters: [
          { name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] },
        ],
      });
      const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
      if (paths.length === 0) return;
      for (const p of paths) {
        try {
          await insertMedia(entryId, p);
        } catch (e) {
          toast.show(`图片插入失败: ${errorMessage(e)}`);
        }
      }
      await loadMedia();
    } catch (e) {
      toast.show(`选图失败: ${errorMessage(e)}`);
    }
  }

  function fileToBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = String(reader.result);
        resolve(result.slice(result.indexOf(',') + 1));
      };
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  }

  function errorMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function onFilesPicked(event: Event) {
    const input = event.target as HTMLInputElement;
    const files = input.files ? Array.from(input.files) : [];
    input.value = '';
    if (files.length === 0) return;
    let failed = 0;
    for (const f of files) {
      try {
        const b64 = await fileToBase64(f);
        await insertMediaBytes(entryId, b64);
      } catch (e) {
        failed += 1;
        toast.show(`${f.name} 插入失败: ${errorMessage(e)}`);
      }
    }
    await loadMedia();
    if (failed > 0 && files.length > 1) {
      toast.show(`${failed}/${files.length} 张图片未插入（格式不支持？）`);
    }
  }

  async function removeMedia(id: string) {
    try {
      await deleteMedia(id);
      await loadMedia();
    } catch (e) {
      toast.show(`图片删除失败: ${errorMessage(e)}`);
    }
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
    <input
      type="file"
      accept="image/*"
      multiple
      class="hidden-file-input"
      data-testid="media-file-input"
      bind:this={fileInput}
      onchange={onFilesPicked}
    />
  </div>
</div>
