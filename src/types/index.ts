export type DownloadStatus =
  | "queued"
  | "downloading"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export interface Download {
  id: number;
  url: string;
  title: string;
  platform: string;
  thumbnail: string | null;
  filename: string | null;
  file_path: string | null;
  format: string;
  resolution: string | null;
  audio_only: boolean;
  duration: number | null;
  file_size: number | null;
  status: DownloadStatus;
  error: string | null;
  created_at: string;
  completed_at: string | null;
  priority: number;
  position: number;
  clip_start: number | null;
  clip_end: number | null;
}

export interface MediaFormat {
  format_id: string;
  ext: string;
  height: number | null;
  fps: number | null;
  vcodec: string | null;
  acodec: string | null;
  filesize: number | null;
  format_note: string | null;
}

export interface PlaylistEntry {
  url: string;
  title: string;
  duration: number | null;
  thumbnail: string | null;
}

export interface MediaInfo {
  kind: "video" | "playlist";
  url: string;
  title: string;
  platform: string;
  thumbnail: string | null;
  duration: number | null;
  uploader: string | null;
  formats: MediaFormat[];
  entries: PlaylistEntry[];
}

export interface DownloadRequest {
  url: string;
  title: string;
  platform: string;
  thumbnail: string | null;
  duration: number | null;
  audio_only: boolean;
  quality: string | null;
  container: string;
  clip_start?: number | null;
  clip_end?: number | null;
}

export interface ActivationState {
  is_pro: boolean;
}

export type EditOperation = "trim" | "convert";

export interface EditRequest {
  source_id: number;
  operation: EditOperation;
  output_format?: string | null;
  start?: number | null;
  end?: number | null;
}

export interface EditProgress {
  job_id: string;
  percent: number | null;
}

export interface TranscribeRequest {
  source_id?: number | null;
  input_path?: string | null;
  language: string;
}

export interface ExtractAudioRequest {
  source_id?: number | null;
  input_path?: string | null;
  formats: string[];
}

export interface TranscribeResult {
  text: string;
  txt_path: string;
  srt_path: string;
}

export interface TranscribeProgress {
  job_id: string;
  percent: number | null;
  stage: string;
}

export interface Settings {
  download_path: string;
  theme: "dark" | "light" | "system";
  language: string;
  max_concurrent_downloads: number;
  auto_update: boolean;
  notifications: boolean;
  filename_template: string;
  organize_by_platform: boolean;
  cookies_browser: string;
}

export interface ProgressPayload {
  id: number;
  downloaded_bytes: number;
  total_bytes: number | null;
  speed: number | null;
  eta: number | null;
  percent: number | null;
}

export interface StatusPayload {
  id: number;
  status: DownloadStatus;
  error: string | null;
}

export type LibrarySort = "date" | "title" | "size" | "duration";

export interface LibraryQuery {
  query: string;
  platform: string | null;
  kind: "all" | "video" | "audio";
  sort: LibrarySort;
  descending: boolean;
  limit: number | null;
}
