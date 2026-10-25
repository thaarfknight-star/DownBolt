//! پلاگین سرویس foreground اندروید DownBolt: وقتی دانلودی فعال است،
//! سرویس foreground با نوتیفیکیشن دائمی (ongoing) بالا می‌ماند تا سیستم
//! پروسه را نکشد و دانلود در بک‌گراند کامل شود؛ وقتی هیچ دانلودی فعال
//! نیست، سرویس و نوتیفیکیشن برداشته می‌شوند.
//!
//! جهت فراخوانی فقط Rust → Kotlin است (مثل پلاگین نصب): سمت Rust وضعیت
//! را هم‌گام می‌کند و اکشن‌های نوتیفیکیشن از راه فایل صندوق‌پستی
//! (fg_action.json در dataDir) به نخ Rust برمی‌گردند — چون
//! BroadcastReceiver به AppHandle دسترسی ندارد. روی دسکتاپ هر دو متد
//! no-op هستند (سینی سیستم دسکتاپ دست‌نخورده است).

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.foreground";

pub struct DownboltForeground<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _marker: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> DownboltForeground<R> {
    /// هم‌گام‌سازی نوتیفیکیشن با وضعیت فعلی: دانلود فعال هست →
    /// سرویس بالا + نوتیفیکیشن با تعداد فعال و سرعت کلی؛ وگرنه صدا
    /// زده نمی‌شود (توقف با stop_notification جداست تا فراخواننده
    /// خودش تصمیم بگیرد). روی دسکتاپ no-op.
    pub fn sync_notification(&self, active_count: u32, total_speed: u64) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                active_count: u32,
                total_speed: u64,
            }
            self.handle
                .run_mobile_plugin::<()>(
                    "syncNotification",
                    Args {
                        active_count,
                        total_speed,
                    },
                )
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (active_count, total_speed);
            Ok(())
        }
    }

    /// برداشتن سرویس و نوتیفیکیشن (وقتی هیچ دانلودی فعال نیست).
    /// روی دسکتاپ no-op.
    pub fn stop_notification(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin::<()>("stopNotification", ())
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            Ok(())
        }
    }
}

pub trait DownboltForegroundExt<R: Runtime> {
    fn downbolt_foreground(&self) -> &DownboltForeground<R>;
}

impl<R: Runtime, T: Manager<R>> DownboltForegroundExt<R> for T {
    fn downbolt_foreground(&self) -> &DownboltForeground<R> {
        self.state::<DownboltForeground<R>>().inner()
    }
}

#[tauri::command]
async fn sync_notification<R: Runtime>(
    app: AppHandle<R>,
    active_count: u32,
    total_speed: u64,
) -> Result<(), String> {
    app.downbolt_foreground()
        .sync_notification(active_count, total_speed)
}

#[tauri::command]
async fn stop_notification<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.downbolt_foreground().stop_notification()
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("downbolt-foreground")
        .invoke_handler(tauri::generate_handler![
            sync_notification,
            stop_notification
        ])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "ForegroundPlugin")?;
            app.manage(DownboltForeground {
                #[cfg(target_os = "android")]
                handle,
                #[cfg(not(target_os = "android"))]
                _marker: std::marker::PhantomData::<fn() -> R>,
            });
            Ok(())
        })
        .build()
}
