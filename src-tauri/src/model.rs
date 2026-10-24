//! مدل داده‌ی دانلودمنیجر — دقیقاً مطابق قرارداد فرانت (src/lib/api.ts).
//! DownloadItem همان چیزی است که روی IPC ردوبدل و در رویداد download-progress
//! منتشر می‌شود؛ DownloadRecord رکورد داخلیِ ذخیره‌شده است (با مسیر مقصد).

use serde::{Deserialize, Serialize};

/// وضعیت یک دانلود — مقادیر سیم‌کشی‌شده با فرانت یکی است.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Error,
}

/// آیتم دانلود همان‌طور که فرانت می‌بیند (قرارداد src/lib/api.ts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    pub file_name: String,
    /// حجم کل به بایت؛ None یعنی هنوز/اصلاً مشخص نیست
    pub size: Option<u64>,
    /// بایت دانلودشده تا الان
    pub downloaded: u64,
    /// سرعت لحظه‌ای به بایت‌برثانیه
    pub speed: u64,
    pub status: DownloadStatus,
    pub error: Option<String>,
    /// URI ورودیِ منتشرشده در Downloads عمومی اندروید (فقط موبایل).
    pub public_uri: Option<String>,
    /// زمان ساخت به ISO-8601 (UTC)
    pub created_at: String,
}

/// تنظیمات برنامه (قرارداد DmSettings در فرانت).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmSettings {
    pub download_dir: String,
    /// سقف دانلودهای هم‌زمان
    #[serde(default = "default_max_parallel")]
    pub max_parallel: u32,
    /// سقف اتصال موازی برای هر دانلود (۱ تا ۱۶)
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// سقف سرعت کلی به کیلوبایت‌برثانیه؛ ۰ = نامحدود
    pub speed_limit_kbps: u32,
}

fn default_max_connections() -> u32 {
    8
}

fn default_max_parallel() -> u32 {
    3
}

impl DmSettings {
    pub fn new(default_dir: String) -> Self {
        Self {
            download_dir: default_dir,
            max_parallel: 3,
            max_connections: default_max_connections(),
            speed_limit_kbps: 0,
        }
    }

    /// سقف سرعت به بایت‌برثانیه برای موتور؛ None یعنی نامحدود.
    pub fn speed_limit_bps(&self) -> Option<u64> {
        if self.speed_limit_kbps == 0 {
            None
        } else {
            Some(self.speed_limit_kbps as u64 * 1024)
        }
    }
}

/// رکورد داخلی دانلود — همان فیلدهای آیتم + مسیر مقصد روی دیسک.
/// فقط همین رکورد روی دیسک ذخیره می‌شود (store.rs) تا بعد از بسته‌شدن
/// برنامه، دانلودها با همان آفست ادامه پیدا کنند.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub file_name: String,
    pub dest_path: String,
    /// URI ورودیِ منتشرشده در Downloads عمومی اندروید (MediaStore یا
    /// مسیر فایل شاخه‌ی قدیمی)؛ روی دسکتاپ و رکوردهای قدیمی None است.
    /// حذف از دیسک همین ورودی را هم پاک می‌کند.
    #[serde(default)]
    pub public_uri: Option<String>,
    pub size: Option<u64>,
    pub downloaded: u64,
    pub speed: u64,
    pub status: DownloadStatus,
    pub error: Option<String>,
    pub created_at: String,
}

impl DownloadRecord {
    pub fn to_item(&self) -> DownloadItem {
        DownloadItem {
            id: self.id.clone(),
            url: self.url.clone(),
            file_name: self.file_name.clone(),
            size: self.size,
            downloaded: self.downloaded,
            speed: self.speed,
            status: self.status,
            error: self.error.clone(),
            public_uri: self.public_uri.clone(),
            created_at: self.created_at.clone(),
        }
    }
}

// ---------- زمان ISO-8601 بدون وابستگی اضافه ----------

/// زمان فعلی به‌صورت ISO-8601 در UTC (مثل 2026-10-23T01:40:44Z).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_from_unix(secs)
}

/// تبدیل ثانیه‌ی یونیکس به رشته‌ی ISO-8601 (UTC).
/// تبدیل روز→تاریخ از الگوریتم civil_from_days (Howard Hinnant) است.
pub fn iso_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    if m <= 2 {
        y += 1;
    }
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_epoch() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso_known_date() {
        // 2026-10-23T01:40:44Z
        assert_eq!(iso_from_unix(1_792_719_644), "2026-10-23T01:40:44Z");
    }

    #[test]
    fn settings_without_max_parallel_defaults_to_3() {
        // رگرسیون بازبینی 0.4.0: payload فرانت بدون max_parallel نباید
        // خطای missing field بدهد — serde default مقدار ۳ می‌گذارد.
        let s: DmSettings = serde_json::from_str(
            r#"{"download_dir":"/tmp/dl","max_connections":8,"speed_limit_kbps":0}"#,
        )
        .unwrap();
        assert_eq!(s.max_parallel, 3);
        assert_eq!(s.max_connections, 8);
        assert_eq!(s.speed_limit_kbps, 0);
    }
}
