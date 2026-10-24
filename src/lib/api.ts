// api.ts — قرارداد فرانت‌اند با بک‌اند Rust (Tauri invoke)
// ─────────────────────────────────────────────────────────────
// کامندهایی که بک‌اند باید پیاده کند:
//   list_downloads()                      -> DownloadItem[]
//   add_download({ url })                 -> DownloadItem
//   pause_download({ id })                -> void
//   resume_download({ id })               -> void
//   remove_download({ id, deleteFile })   -> void
//   get_settings()                        -> DmSettings
//   set_settings({ settings })            -> void
//   pick_download_dir()                   -> string | null   (دیالوگ انتخاب پوشه؛ فقط دسکتاپ — روی اندروید UI آن را نشان نمی‌دهد)
//   app_platform()                        -> string          ("android" یا "desktop" برای گیت UI موبایل)
//   open_download({ id })                 -> void            (باز کردن فایل با برنامه‌ی مناسب؛ فقط اندروید)
//   share_download({ id })                -> void            (اشتراک‌گذاری فایل؛ فقط اندروید)
//   read_clipboard()                      -> string          (متن کلیپ‌بورد؛ اگر نبود، فرانت fallback دارد)
//   copy_to_clipboard({ text })           -> void            (کپی لینک هر دانلود — مثل ADM و FDM)
//   cancel_post_download_action()         -> void            (لغو شمارش معکوس پس از دانلود)
// رویدادهای حالت پس از دانلود (۰٫۶٫۳، فقط دسکتاپ):
//   emit("post-download-countdown", { action, remaining, run })
//   emit("post-download-cancelled", { run })
//   emit("post-download-executed", { action, run })
//   emit("post-download-failed", { action, run, error })
// رویداد (اختیاری ولی توصیه‌شده): بک‌اند هر تغییر/تیک پیشرفت را با
//   emit("download-progress", DownloadItem[])  (اسنپ‌شات کامل لیست) بفرستد؛
//   اگر رویداد نیاید، فرانت خودش هر ۱ ثانیه list_downloads را پول می‌کند.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type DownloadStatus = "queued" | "downloading" | "paused" | "completed" | "error";

export interface DownloadItem {
  id: string;
  url: string;
  file_name: string;
  /** حجم کل به بایت؛ null یعنی هنوز/اصلاً مشخص نیست */
  size: number | null;
  /** بایت دانلودشده تا الان */
  downloaded: number;
  /** سرعت لحظه‌ای به بایت‌برثانیه */
  speed: number;
  status: DownloadStatus;
  error?: string | null;
  created_at: string; // ISO
}

/** کاری که بعد از تمام‌شدن همه‌ی دانلودها انجام می‌شود (۰٫۶٫۳) */
export type PostDownloadAction = "none" | "shutdown" | "sleep" | "restart";

export interface DmSettings {
  download_dir: string;
  /** سقف دانلودهای هم‌زمان (سمت Rust هم serde default دارد) */
  max_parallel: number;
  /** سقف اتصال موازی برای هر دانلود/هاست — بازه‌ی ۱ تا ۱۶، پیش‌فرض موتور ۸ (اسپک موتور §۴٫۳) */
  max_connections: number;
  /** سقف سرعت کلی به کیلوبایت‌برثانیه؛ ۰ = نامحدود */
  speed_limit_kbps: number;
  /** حالت پس از دانلود؛ "none" یعنی هیچ‌کاری نکن */
  post_download_action: PostDownloadAction;
}

/** رویداد تیک شمارش معکوس پس از دانلود */
export interface PostDownloadCountdown {
  action: PostDownloadAction;
  remaining: number;
  run: number;
}

/** نتیجه‌ی بررسی آپدیت (قرارداد Rust: UpdateInfo2) */
export interface UpdateInfo {
  version: string;
  notes_fa: string;
  published_at: string;
  mandatory: boolean;
  min_version: string | null;
  package_kind: string;
  size: number;
}

/** گزارش یک‌باره‌ی نتیجه‌ی آپدیت در استارت برنامه (قرارداد Rust: UpdateReport) */
export interface UpdateReport {
  just_updated_from: string | null;
  update_failed_from: string | null;
}

export const api = {
  listDownloads: (): Promise<DownloadItem[]> => invoke("list_downloads"),
  addDownload: (url: string): Promise<DownloadItem> => invoke("add_download", { url }),
  pauseDownload: (id: string): Promise<void> => invoke("pause_download", { id }),
  resumeDownload: (id: string): Promise<void> => invoke("resume_download", { id }),
  removeDownload: (id: string, deleteFile = false): Promise<void> =>
    invoke("remove_download", { id, deleteFile }),
  platform: (): Promise<string> => invoke("app_platform"),
  openDownload: (id: string): Promise<void> => invoke("open_download", { id }),
  shareDownload: (id: string): Promise<void> => invoke("share_download", { id }),
  checkUpdate: (): Promise<UpdateInfo | null> => invoke("check_update2"),
  downloadUpdate: (): Promise<void> => invoke("download_update2"),
  cancelUpdateDownload: (): Promise<void> => invoke("cancel_update_download2"),
  applyUpdate: (): Promise<void> => invoke("apply_update2"),
  readyUpdatePath: (): Promise<string | null> => invoke("ready_update_path"),
  openUpdateFile: (): Promise<string | null> => invoke("open_update_file_dialog"),
  applyLocalUpdate: (path: string): Promise<void> => invoke("apply_local_update", { path }),
  getUpdateReport: (): Promise<UpdateReport> => invoke("get_update_report"),
  installApk: (path: string): Promise<void> =>
    invoke("plugin:downbolt-install|install_apk", { path }),
  setCloseToTray: (enabled: boolean): Promise<void> =>
    invoke("set_close_to_tray", { enabled }),
  getSettings: (): Promise<DmSettings> => invoke("get_settings"),
  setSettings: (settings: DmSettings): Promise<void> => invoke("set_settings", { settings }),
  pickDownloadDir: (): Promise<string | null> => invoke("pick_download_dir"),
  readClipboard: (): Promise<string> => invoke("read_clipboard"),
  copyToClipboard: (text: string): Promise<void> => invoke("copy_to_clipboard", { text }),
  cancelPostDownload: (): Promise<void> => invoke("cancel_post_download_action"),
};

/** اشتراک رویداد اسنپ‌شات پیشرفت؛ اگر بک‌اند رویداد نفرستد، هیچ‌وقت صدا زده نمی‌شود */
export function onProgressSnapshot(cb: (items: DownloadItem[]) => void): Promise<() => void> {
  return listen<DownloadItem[]>("download-progress", (e) => cb(e.payload));
}

/** رویدادهای حالت پس از دانلود (۰٫۶٫۳) */
export function onPostDownloadCountdown(
  cb: (p: PostDownloadCountdown) => void,
): Promise<() => void> {
  return listen<PostDownloadCountdown>("post-download-countdown", (e) => cb(e.payload));
}

export function onPostDownloadCancelled(cb: (p: { run: number }) => void): Promise<() => void> {
  return listen<{ run: number }>("post-download-cancelled", (e) => cb(e.payload));
}

export function onPostDownloadExecuted(
  cb: (p: { action: PostDownloadAction; run: number }) => void,
): Promise<() => void> {
  return listen<{ action: PostDownloadAction; run: number }>(
    "post-download-executed",
    (e) => cb(e.payload),
  );
}

export function onPostDownloadFailed(
  cb: (p: { action: PostDownloadAction; run: number; error: string }) => void,
): Promise<() => void> {
  return listen<{ action: PostDownloadAction; run: number; error: string }>(
    "post-download-failed",
    (e) => cb(e.payload),
  );
}

/** درصد پیشرفت یک آیتم (۰ تا ۱۰۰)؛ حجم نامشخص = null */
export function progressPct(it: DownloadItem): number | null {
  if (!it.size || it.size <= 0) return null;
  return Math.min(100, (it.downloaded / it.size) * 100);
}

/** ثانیه‌ی باقی‌مانده‌ی تخمینی؛ نامشخص = null */
export function etaSeconds(it: DownloadItem): number | null {
  if (!it.size || it.speed <= 0 || it.status !== "downloading") return null;
  const left = it.size - it.downloaded;
  return left <= 0 ? 0 : left / it.speed;
}

/** رویداد پیشرفت دانلود آپدیت */
export function onUpdateProgress(
  cb: (p: { percent: number; downloaded: number; total: number }) => void,
): Promise<() => void> {
  return listen("update-progress2", (e) =>
    cb(e.payload as { percent: number; downloaded: number; total: number }),
  );
}
