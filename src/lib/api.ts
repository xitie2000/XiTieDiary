import { invoke } from '@tauri-apps/api/core';
import type { AppErrorDto, ConfigStatus, Entry, MediaMeta, MediaWithUrl } from './types';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    const dto = e as AppErrorDto;
    if (dto && typeof dto.code === 'string' && typeof dto.message === 'string') {
      throw new Error(dto.message, { cause: dto.code });
    }
    throw e;
  }
}

export function listEntries(from?: string, to?: string): Promise<Entry[]> {
  return call<Entry[]>('list_entries', { from: from ?? null, to: to ?? null });
}

export function getEntry(id: string): Promise<Entry | null> {
  return call<Entry | null>('get_entry', { id });
}

export function createDraftEntry(date: string): Promise<Entry> {
  return call<Entry>('create_draft_entry', { date });
}

export function saveEntry(id: string, date: string, content: string): Promise<Entry> {
  return call<Entry>('save_entry', { id, date, content });
}

export function deleteEntry(id: string): Promise<void> {
  return call<void>('delete_entry', { id });
}

export function insertMedia(entryId: string, path: string): Promise<MediaMeta> {
  return call<MediaMeta>('insert_media', { entryId: entryId, path });
}

export function deleteMedia(id: string): Promise<void> {
  return call<void>('delete_media', { id });
}

export function listMedia(entryId: string): Promise<MediaWithUrl[]> {
  return call<MediaWithUrl[]>('list_media', { entryId });
}

export function mediaCounts(): Promise<Record<string, number>> {
  return call<Record<string, number>>('media_counts');
}

export function getConfigStatus(): Promise<ConfigStatus> {
  return call<ConfigStatus>('get_config_status');
}

export function cleanupEmptyDrafts(): Promise<number> {
  return call<number>('cleanup_empty_drafts');
}
