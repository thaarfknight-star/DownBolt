//! پلاگین پل اندروید DownBolt: نصب APK (موتور آپدیت) + انتشار فایل در
//! Downloads عمومی و باز کردن/اشتراک‌گذاری فایل با Intentهای سیستم.
//! روی دسکتاپ متدهای اندرویدی خطا می‌دهند (آپدیت آنجا با هلپر انجام
//! می‌شود و فایل‌ها از قبل در پوشه‌ی کاربر دیده می‌شوند).

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

    /// انتشار فایل کامل‌شده در Downloads عمومی (MediaStore در اندروید ۱۰+
    /// وگرنه کپی + اسکن مدیا). خروجی: URI ورودیِ منتشرشده (برای
    /// پاک‌کردنِ بعدی) یا مسیر فایل عمومی در شاخه‌ی قدیمی.
    /// بهترین‌تلاش است؛ خطا یعنی فایل فقط در پوشه‌ی داخلی برنامه می‌ماند
    /// و با باز کردن/اشتراک‌گذاری در دسترس است.
    pub fn publish_to_downloads(
        &self,
        path: String,
        display_name: String,
        mime_type: String,
    ) -> Result<String, String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                path: String,
                display_name: String,
                mime_type: String,
            }
            #[derive(serde::Deserialize)]
            struct Resp {
                uri: String,
            }
            let resp = self
                .handle
                .run_mobile_plugin::<Resp>(
                    "publishToDownloads",
                    Args {
                        path,
                        display_name,
                        mime_type,
                    },
                )
                .map_err(|e| e.to_string())?;
            Ok(resp.uri)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, display_name, mime_type);
            Err("انتشار در Downloads فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// پاک‌کردن ورودیِ منتشرشده (مکمل حذف از دیسک): URI محتوایی از
    /// MediaStore حذف می‌شود، مسیر فایلِ شاخه‌ی قدیمی مستقیم پاک می‌شود.
    pub fn delete_published(&self, uri: String) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args {
                uri: String,
            }
            self.handle
                .run_mobile_plugin::<()>("deletePublished", Args { uri })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = uri;
            Err("حذف ورودی منتشرشده فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// باز کردن فایل با برنامه‌ی مناسب سیستم (URI عمومی در اولویت است).
    pub fn open_file(
        &self,
        path: String,
        uri: Option<String>,
        mime_type: String,
    ) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            // سمت اندرویدِ Tauri نام‌ها را تبدیل نمی‌کند؛ Kotlin فیلدها
            // را camelCase می‌خواهد (displayName/mimeType) — بدون این
            // rename، parseArgs همیشه reject می‌شد.
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                path: String,
                uri: Option<String>,
                mime_type: String,
            }
            self.handle
                .run_mobile_plugin::<()>("openFile", Args { path, uri, mime_type })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, uri, mime_type);
            Err("باز کردن فایل با Intent فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// انتخاب پوشه با SAF (۰٫۶٫۱۰): خروجی None یعنی کاربر لغو کرد،
    /// وگرنه (URI درخت، نام نمایشی) با مجوزِ ماندگار.
    pub fn pick_folder(&self) -> Result<Option<(String, String)>, String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Deserialize)]
            struct Resp {
                uri: String,
                name: String,
            }
            let resp = self
                .handle
                .run_mobile_plugin::<Resp>("pickFolder", ())
                .map_err(|e| e.to_string())?;
            if resp.uri.is_empty() {
                Ok(None)
            } else {
                Ok(Some((resp.uri, resp.name)))
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            Err("انتخاب پوشه‌ی SAF فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// کپی فایل کامل‌شده در پوشه‌ی انتخاب‌شده با SAF (۰٫۶٫۱۰).
    /// خروجی: URI سندِ ساخته‌شده (برای پاک‌کردنِ بعدی).
    pub fn copy_to_folder(
        &self,
        path: String,
        display_name: String,
        mime_type: String,
        folder_uri: String,
    ) -> Result<String, String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                path: String,
                display_name: String,
                mime_type: String,
                folder_uri: String,
            }
            #[derive(serde::Deserialize)]
            struct Resp {
                uri: String,
            }
            let resp = self
                .handle
                .run_mobile_plugin::<Resp>(
                    "copyToFolder",
                    Args {
                        path,
                        display_name,
                        mime_type,
                        folder_uri,
                    },
                )
                .map_err(|e| e.to_string())?;
            Ok(resp.uri)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, display_name, mime_type, folder_uri);
            Err("کپی در پوشه‌ی SAF فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// پاک‌کردن سندِ کپی‌شده با SAF (مکمل حذف از دیسک).
    pub fn delete_document(&self, uri: String) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args {
                uri: String,
            }
            self.handle
                .run_mobile_plugin::<()>("deleteDocument", Args { uri })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = uri;
            Err("حذف سند SAF فقط روی اندروید ممکن است.".to_string())
        }
    }

    /// اشتراک‌گذاری فایل با chooser سیستم (URI عمومی در اولویت است).
    pub fn share_file(
        &self,
        path: String,
        uri: Option<String>,
        mime_type: String,
    ) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            // مثل open_file: نام‌های camelCase برای Kotlin.
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                path: String,
                uri: Option<String>,
                mime_type: String,
            }
            self.handle
                .run_mobile_plugin::<()>("shareFile", Args { path, uri, mime_type })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, uri, mime_type);
            Err("اشتراک‌گذاری فایل فقط روی اندروید ممکن است.".to_string())
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

#[tauri::command]
async fn publish_to_downloads<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    display_name: String,
    mime_type: String,
) -> Result<String, String> {
    app.downbolt_install()
        .publish_to_downloads(path, display_name, mime_type)
}

#[tauri::command]
async fn delete_published<R: Runtime>(app: AppHandle<R>, uri: String) -> Result<(), String> {
    app.downbolt_install().delete_published(uri)
}

#[tauri::command]
async fn open_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    uri: Option<String>,
    mime_type: String,
) -> Result<(), String> {
    app.downbolt_install().open_file(path, uri, mime_type)
}

#[tauri::command]
async fn share_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    uri: Option<String>,
    mime_type: String,
) -> Result<(), String> {
    app.downbolt_install().share_file(path, uri, mime_type)
}

/// پوشه‌ی انتخاب‌شده با SAF (۰٫۶٫۱۰) برای فرانت؛ None یعنی لغو.
#[derive(serde::Serialize)]
pub struct SafFolder {
    pub uri: String,
    pub name: String,
}

#[tauri::command]
async fn pick_folder<R: Runtime>(app: AppHandle<R>) -> Result<Option<SafFolder>, String> {
    #[cfg(target_os = "android")]
    {
        Ok(app
            .downbolt_install()
            .pick_folder()?
            .map(|(uri, name)| SafFolder { uri, name }))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(None)
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("downbolt-install")
        .invoke_handler(tauri::generate_handler![
            install_apk,
            publish_to_downloads,
            delete_published,
            open_file,
            share_file,
            pick_folder
        ])
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
