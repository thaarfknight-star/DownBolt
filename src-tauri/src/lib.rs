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
mod updater;

use engine::{EngineEvent, RealEngine};
use manager::{AppliedEffect, DownloadManager};
use model::{DmSettings, DownloadItem};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
#[cfg(target_os = "android")] // مصرف‌کننده‌های downbolt_install همه اندرویدی‌اند
use tauri_plugin_downbolt_install::DownboltInstallExt;
#[cfg(desktop)] // انتخاب‌گر پوشه فقط دسکتاپ است (pick_download_dir_impl)
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    manager: Mutex<DownloadManager>,
}

/// «بستن به سینی سیستم» — پیش‌فرض روشن؛ فرانت از تنظیمات کاربر پرش می‌کند.
pub struct TrayState(pub Mutex<bool>);

/// تنظیم «بستن به سینی سیستم» از فرانت.
#[tauri::command]
fn set_close_to_tray(state: tauri::State<TrayState>, enabled: bool) {
    *state.0.lock().unwrap() = enabled;
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
    // روی اندروید، «حذف فایل» ورودیِ منتشرشده در Downloads عمومی را هم
    // پاک می‌کند؛ خطای آن مثل خطای پاک‌کردن فایل، رکورد را نگه می‌دارد
    // تا کاربر دوباره تلاش کند.
    #[cfg(target_os = "android")]
    if delete_file {
        let uri = state
            .manager
            .lock()
            .unwrap()
            .get(&id)
            .and_then(|r| r.public_uri);
        if let Some(uri) = uri {
            app.downbolt_install()
                .delete_published(uri)
                .map_err(|e| format!("ورودی Downloads عمومی پاک نشد: {e}"))?;
        }
    }
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
/// فرانت روی اندروید این دکمه را اصلاً نشان نمی‌دهد (به‌جاش توضیح
/// Downloads عمومی آمده) تا دکمه‌ی بی‌اثرِ بی‌صدا نماند؛ فایل‌های
/// کامل‌شده روی اندروید با publish_to_downloads در Downloads عمومی
/// منتشر می‌شوند.
#[cfg(mobile)]
async fn pick_download_dir_impl(_app: &AppHandle) -> Option<String> {
    None
}

/// سکوی در حال اجرا برای فرانت ("android" یا "desktop") — فرانت با آن
/// دکمه‌ی انتخاب پوشه را روی اندروید پنهان و دکمه‌های باز کردن/
/// اشتراک‌گذاری را فقط برای دانلودهای کامل‌شده‌ی اندروید نشان می‌دهد.
#[tauri::command]
fn app_platform() -> &'static str {
    #[cfg(target_os = "android")]
    {
        "android"
    }
    #[cfg(not(target_os = "android"))]
    {
        "desktop"
    }
}

/// حدس MIME از پسوند نام فایل (برای Intent باز کردن/اشتراک‌گذاری اندروید).
#[cfg(target_os = "android")]
fn mime_for(file_name: &str) -> &'static str {
    let ext = file_name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "zip" => "application/zip",
        "apk" => "application/vnd.android.package-archive",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "txt" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// باز کردن فایل دانلود با برنامه‌ی مناسب سیستم — فقط اندروید (روی
/// دسکتاپ فایل از قبل در پوشه‌ی کاربر دیده می‌شود و این کامند اصلاً
/// ثبت نمی‌شود).
#[cfg(target_os = "android")]
#[tauri::command]
fn open_download(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let rec = state
        .manager
        .lock()
        .unwrap()
        .get(&id)
        .ok_or_else(|| "دانلودی با این شناسه پیدا نشد.".to_string())?;
    app.downbolt_install().open_file(
        rec.dest_path,
        rec.public_uri,
        mime_for(&rec.file_name).to_string(),
    )
}

/// اشتراک‌گذاری فایل دانلود با chooser سیستم — فقط اندروید.
#[cfg(target_os = "android")]
#[tauri::command]
fn share_download(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let rec = state
        .manager
        .lock()
        .unwrap()
        .get(&id)
        .ok_or_else(|| "دانلودی با این شناسه پیدا نشد.".to_string())?;
    app.downbolt_install().share_file(
        rec.dest_path,
        rec.public_uri,
        mime_for(&rec.file_name).to_string(),
    )
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
    // پاک‌سازی فایل‌های ‎.old‎ باقی‌مانده از آپدیت قبلی (موتور آپدیت v2).
    updater::cleanup_old_files();

    tauri::Builder::default()
        .manage(TrayState(Mutex::new(true)))
        .manage(updater::UpdateV2State::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_downbolt_install::init())
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
            let engine = Arc::new(RealEngine::new());
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
                    // شناسه‌ی دانلودی که همین رویداد کاملش می‌کند — برای
                    // انتشار در Downloads عمومی اندروید بعد از اعمال رویداد.
                    #[cfg(target_os = "android")]
                    let completed_id = match &evt {
                        EngineEvent::Completed { id, .. } => Some(id.clone()),
                        _ => None,
                    };
                    let state = handle.state::<AppState>();
                    let mut mgr = state.manager.lock().unwrap();
                    match mgr.apply_event(evt) {
                        AppliedEffect::StateChanged => {
                            // روی اندروید، فایل کامل‌شده در Downloads عمومی
                            // منتشر می‌شود تا در فایل‌منیجر دیده شود.
                            // بهترین‌تلاش است: خطا دانلود را خراب نمی‌کند —
                            // فایل از مسیر داخلی با باز کردن/اشتراک‌گذاری
                            // همچنان در دسترس است.
                            #[cfg(target_os = "android")]
                            if let Some(id) = completed_id {
                                if let Some(rec) = mgr.get(&id) {
                                    // گارد is_none: رویداد تکراری Completed
                                    // ورودی یتیمِ دوم در MediaStore نسازد.
                                    if rec.status == model::DownloadStatus::Completed
                                        && rec.public_uri.is_none()
                                    {
                                        // انتشار بهترین‌تلاش است، ولی خطا
                                        // دیگر بی‌صدا قورت داده نمی‌شود —
                                        // همان سکوت بود که باگ نام را نامرئی
                                        // کرده بود.
                                        let mime = mime_for(&rec.file_name).to_string();
                                        match handle.downbolt_install().publish_to_downloads(
                                            rec.dest_path,
                                            rec.file_name,
                                            mime,
                                        ) {
                                            Ok(uri) => {
                                                let _ = mgr.set_public_uri(&id, uri);
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "DownBolt: انتشار در Downloads عمومی ناموفق بود: {e}"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
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

            // آیکن سینی سیستم با منوی «نمایش / خروج» — دانلودها موقع
            // مخفی‌بودن پنجره هم ادامه پیدا می‌کنند. (فقط دسکتاپ)
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
                use tauri::Manager;
                let show_item = MenuItem::with_id(app, "show", "نمایش DownBolt", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "خروج", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
                TrayIconBuilder::with_id("main-tray")
                    .icon(icon)
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // بستن به سینی: اگر تنظیم روشن است، بستن پنجره فقط مخفی‌اش می‌کند.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use tauri::Manager;
                let state = window.state::<TrayState>();
                if *state.0.lock().unwrap() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
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
            app_platform,
            #[cfg(target_os = "android")]
            open_download,
            #[cfg(target_os = "android")]
            share_download,
            read_clipboard,
            set_close_to_tray,
            updater::check_update2,
            updater::download_update2,
            updater::cancel_update_download2,
            updater::apply_update2,
            updater::ready_update_path,
            updater::open_update_file_dialog,
            updater::apply_local_update,
            updater::get_update_report,
        ])
        .run(tauri::generate_context!())
        .expect("خطا هنگام اجرای برنامه");
}
