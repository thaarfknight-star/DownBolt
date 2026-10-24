//! اجرای حالت پس از دانلود روی سیستم (۰٫۶٫۳) — بدون وابستگی تازه و
//! بدون نصب هیچ چیزی.
//!
//! ویندوز (ابزارهای استاندارد خود سیستم):
//! - خاموش: `shutdown.exe /s /t 0`
//! - ری‌استارت: `shutdown.exe /r /t 0`
//! - خواب: `rundll32.exe powrprof.dll,SetSuspendState 0,1,0` — همان روش
//!   استاندارد بدون وابستگی؛ پارامتر اول یعنی «خواب، نه هایبرنیت».
//!   اگر هایبرنیت روی سیستم فعال باشد، رفتار دقیق خواب تابع تنظیمات
//!   همان سیستم است؛ فرمان همان فرمان استاندارد است.
//!
//! روی بقیه‌ی سکوها (از جمله اندروید) خطا برمی‌گردد؛ فرانت این بخش
//! را فقط روی دسکتاپ نشان می‌دهد. Ok یعنی «فرمان صادر شد» — نه تضمین
//! انجام (سیستم ممکن است فرمان را رد کند؛ خطایش به فرانت می‌رسد).

use crate::model::PostDownloadAction;

fn run(prog: &str, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("اجرای «{prog}» ناموفق بود: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "«{prog}» با خطا برگشت: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// اجرای اکشن پس از دانلود. None یعنی «هیچ‌کاری نکن» (همیشه Ok).
pub fn execute(action: PostDownloadAction) -> Result<(), String> {
    match action {
        PostDownloadAction::None => Ok(()),
        #[cfg(target_os = "windows")]
        PostDownloadAction::Shutdown => run("shutdown.exe", &["/s", "/t", "0"]),
        #[cfg(target_os = "windows")]
        PostDownloadAction::Restart => run("shutdown.exe", &["/r", "/t", "0"]),
        #[cfg(target_os = "windows")]
        PostDownloadAction::Sleep => run(
            "rundll32.exe",
            &["powrprof.dll,SetSuspendState", "0,1,0"],
        ),
        #[cfg(not(target_os = "windows"))]
        _ => Err("حالت پس از دانلود فقط روی ویندوز پشتیبانی می‌شود.".into()),
    }
}
