import { invoke } from "@tauri-apps/api/core";
import type {
  ActivationState,
  Download,
  DownloadRequest,
  EditRequest,
  ExtractAudioRequest,
  LibraryQuery,
  MediaInfo,
  Settings,
  TranscribeRequest,
  TranscribeResult,
} from "@/types";

export const api = {
  fetchMetadata: (url: string) => invoke<MediaInfo>("fetch_metadata", { url }),

  checkDuplicate: (url: string) =>
    invoke<Download | null>("check_duplicate", { url }),

  startDownload: (request: DownloadRequest) =>
    invoke<Download>("start_download", { request }),

  pauseDownload: (id: number) => invoke<void>("pause_download", { id }),
  resumeDownload: (id: number) => invoke<void>("resume_download", { id }),
  cancelDownload: (id: number) => invoke<void>("cancel_download", { id }),
  retryDownload: (id: number) => invoke<void>("retry_download", { id }),

  removeDownload: (id: number, deleteFile: boolean) =>
    invoke<void>("remove_download", { id, deleteFile }),

  clearHistory: (deleteFiles: boolean) =>
    invoke<void>("clear_history", { deleteFiles }),

  reorderQueue: (orderedIds: number[]) =>
    invoke<void>("reorder_queue", { orderedIds }),

  listDownloads: () => invoke<Download[]>("list_downloads"),

  searchLibrary: (query: LibraryQuery) =>
    invoke<Download[]>("search_library", { query }),

  listPlatforms: () => invoke<string[]>("list_platforms"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) =>
    invoke<void>("update_settings", { settings }),

  openFile: (id: number) => invoke<void>("open_file", { id }),
  showInFolder: (id: number) => invoke<void>("show_in_folder", { id }),
  openDownloadFolder: () => invoke<void>("open_download_folder"),

  getActivation: () => invoke<ActivationState>("get_activation"),
  activatePro: (key: string) => invoke<ActivationState>("activate_pro", { key }),
  deactivatePro: () => invoke<ActivationState>("deactivate_pro"),

  runEdit: (request: EditRequest, jobId: string) =>
    invoke<Download>("run_edit", { request, jobId }),

  transcribe: (request: TranscribeRequest, jobId: string) =>
    invoke<TranscribeResult>("transcribe", { request, jobId }),

  extractAudio: (request: ExtractAudioRequest, jobId: string) =>
    invoke<Download[]>("extract_audio", { request, jobId }),
};
