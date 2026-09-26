export interface Entry {
  id: string;
  date: string;
  content: string;
  created_at: number;
  updated_at: number;
  deleted: boolean;
}

export interface MediaMeta {
  id: string;
  entry_id: string;
  mime: string;
  size: number;
  updated_at: number;
  deleted: boolean;
}

export interface ConfigStatus {
  configured: boolean;
  provider: string | null;
  bucket: string | null;
}

export interface AppErrorDto {
  code: string;
  message: string;
}
