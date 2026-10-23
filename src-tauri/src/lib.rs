//! دانلودمنیجر (DownBolt) — لایه‌ی اتصال Tauri.
//!
//! این فایل فقط سیم‌کشی است: ساخت مدیر دانلود با وضعیت ذخیره‌شده، ثبت
//! کامندهای قرارداد فرانت (src/lib/api.ts)، حلقه‌ی رویداد موتور (انتشار
//! اسنپ‌شات download-progress با throttle) و پلاگین‌های دیالوگ/کلیپ‌بورد.
//! منطق واقعی در manager.rs (هسته) و engine.rs (نقطه‌ی اتصال موتور) است.

mod engine;
mod manager;
mod model;
mod store;

use engine::{EngineEvent, PlaceholderEngine};
use manager::{AppliedEffect, DownloadManager};
use model::{DmSettings, DownloadItem};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
#[cfg(desktop)] // انتخاب‌گر پوشه فقط دسکتاپ است (pick_download_dir_impl)
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    manager: Mutex<DownloadManager>,
}

/// انتشار اسنپ‌شات کامل لیست به فرانت (رویداد download-progress در قرارداد).
fn emit_snapshot(app: &AppHandle, mgr: &DownloadManager) {
    let _ = app.emit("download-progress", mgr.list());
}

// ---------- کامندهای قرارداد فرانت (src/lib/api.ts) ----------

#[tauri::command]
fn list_downloads(state: State<AppState>) -> Vec<DownloadItem> {
    state.manager.lock().unwrap().list()
}

#[tauri::command]
fn add_download(
    app: AppHandle,
    state: State<AppState>,
    url: String,
) -> Result<DownloadItem, String> {
    let mut mgr = state.manager.lock().unwrap();
    let item = mgr.add(&url)?;
    emit_snapshot(&app, &mgr);
    Ok(item)
}

#[tauri::command]
fn pause_download(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.pause(&id)?;
    emit_snapshot(&app, &mgr);
    Ok(())
}

#[tauri::command]
fn resume_download(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.resume(&id)?;
    emit_snapshot(&app, &mgr);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn remove_download(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    delete_file: bool,
) -> Result<(), String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.remove(&id, delete_file)?;
    emit_snapshot(&app, &mgr);
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> DmSettings {
    state.manager.lock().unwrap().settings()
}

#[tauri::command]
fn set_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: DmSettings,
) -> Result<(), String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.set_settings(settings)?;
    emit_snapshot(&app, &mgr);
    Ok(())
}

/// دیالوگ انتخاب پوشه‌ی دانلود (برگشتی: مسیر یا None اگر کاربر لغو کرد).
/// انتخاب پوشه در پلاگین دیالوگ فقط روی دسکتاپ وجود دارد؛ روی موبایل
/// (اندروید) این کامند None برمی‌گرداند و پوشه‌ی دانلود همان پیش‌فرض
/// Tauri می‌ماند که با path API روی هر دو سکو کار می‌کند. این تنها نقطه‌ی
/// وابسته به سکو در لایه‌ی اتصال است و پشت cfg جدا شده.
#[tauri::command]
async fn pick_download_dir(app: AppHandle) -> Option<String> {
    pick_download_dir_impl(&app).await
}

/// نسخه‌ی دسکتاپ: انتخاب‌گر پوشه‌ی پلاگین (callback-based؛ نسخه‌های
/// blocking هم فقط دسکتاپ‌اند و برای یکدستی از همان مسیر callback می‌رویم).
#[cfg(desktop)]
async fn pick_download_dir_impl(app: &AppHandle) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let picked = rx.recv().ok().flatten()?;
    picked
        .into_path()
        .ok()
        .map(|b| b.to_string_lossy().to_string())
}

/// نسخه‌ی موبایل: انتخاب‌گر پوشه وجود ندارد — بدون تغییر پوشه.
#[cfg(mobile)]
async fn pick_download_dir_impl(_app: &AppHandle) -> Option<String> {
    None
}

/// متن کلیپ‌بورد برای چسباندن سریع لینک؛ خطا به فرانت می‌رسد تا fallback
/// خودش (paste کاربر) را اجرا کند.
#[tauri::command]
fn read_clipboard(app: AppHandle) -> Result<String, String> {
    app.clipboard().read_text().map_err(|e| e.to_string())
}

// ---------- راه‌اندازی ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("مسیر داده‌ی برنامه در دسترس نیست");
            let store_path = data_dir.join(store::STORE_FILE_NAME);
            let default_download_dir = app
                .path()
                .download_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| data_dir.join("downloads").to_string_lossy().to_string());

            let (events_tx, events_rx) = mpsc::channel::<EngineEvent>();

            // نقطه‌ی اتصال موتور واقعی: پیاده‌سازی docs/engine-spec.md
            // (ایجنت موتور) این‌جا جای PlaceholderEngine می‌نشیند؛ هسته و
            // بقیه‌ی سیم‌کشی دست نمی‌خورند.
            let engine = Arc::new(PlaceholderEngine);
            let manager = DownloadManager::load(store_path, default_download_dir, engine, events_tx);
            app.manage(AppState {
                manager: Mutex::new(manager),
            });

            // حلقه‌ی رویداد موتور: رویدادها را روی وضعیت اعمال می‌کند؛
            // تغییر وضعیت → ذخیره + انتشار فوری؛ پیشرفت → انتشار حداکثر
            // هر ۲۵۰ms و ذخیره حداکثر هر ۱ ثانیه (اعداد اسپک، بخش ۲ و ۱۰).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last_emit = Instant::now() - Duration::from_millis(300);
                let mut last_save = Instant::now();
                for evt in events_rx {
                    let state = handle.state::<AppState>();
                    let mut mgr = state.manager.lock().unwrap();
                    match mgr.apply_event(evt) {
                        AppliedEffect::StateChanged => {
                            emit_snapshot(&handle, &mgr);
                            last_emit = Instant::now();
                            last_save = Instant::now();
                        }
                        AppliedEffect::ProgressOnly => {
                            if last_save.elapsed() >= Duration::from_secs(1) {
                                let _ = mgr.persist();
                                last_save = Instant::now();
                            }
                            if last_emit.elapsed() >= Duration::from_millis(250) {
                                emit_snapshot(&handle, &mgr);
                                last_emit = Instant::now();
                            }
                        }
                        AppliedEffect::Ignored => {}
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_downloads,
            add_download,
            pause_download,
            resume_download,
            remove_download,
            get_settings,
            set_settings,
            pick_download_dir,
            read_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("خطا هنگام اجرای برنامه");
}
