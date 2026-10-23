//! پلاگین نصب APK برای موتور آپدیت DownBolt: فایل APK دانلودشده را با
//! نصب‌کننده‌ی سیستم اندروید باز می‌کند (FileProvider + Intent).
//! روی دسکتاپ هیچ کاری نمی‌کند (آپدیت آنجا با هلپر انجام می‌شود).

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.install";

pub struct DownboltInstall<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _marker: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> DownboltInstall<R> {
    pub fn install_apk(&self, path: String) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args {
                path: String,
            }
            self.handle
                .run_mobile_plugin::<()>("installApk", Args { path })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = path;
            Err("نصب APK فقط روی اندروید ممکن است.".to_string())
        }
    }
}

pub trait DownboltInstallExt<R: Runtime> {
    fn downbolt_install(&self) -> &DownboltInstall<R>;
}

impl<R: Runtime, T: Manager<R>> DownboltInstallExt<R> for T {
    fn downbolt_install(&self) -> &DownboltInstall<R> {
        self.state::<DownboltInstall<R>>().inner()
    }
}

#[tauri::command]
async fn install_apk<R: Runtime>(app: AppHandle<R>, path: String) -> Result<(), String> {
    app.downbolt_install().install_apk(path)
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("downbolt-install")
        .invoke_handler(tauri::generate_handler![install_apk])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "InstallPlugin")?;
            app.manage(DownboltInstall {
                #[cfg(target_os = "android")]
                handle,
                #[cfg(not(target_os = "android"))]
                _marker: std::marker::PhantomData::<fn() -> R>,
            });
            Ok(())
        })
        .build()
}
