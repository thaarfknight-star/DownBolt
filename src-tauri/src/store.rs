//! ذخیره‌ی وضعیت دانلودها و تنظیمات روی دیسک (JSON) — برای resume بعد از
//! بسته‌شدن برنامه. نوشتن اتمیک است (فایل موقت + rename) تا اگر برنامه وسط
//! ذخیره کرش کرد، فایل وضعیت قبلی سالم بماند.
//!
//! نکته‌ی لایه‌بندی: این فایل فقط «صف و تنظیمات و آفست کلی هر دانلود» را نگه
//! می‌دارد. جزئیات داخلی موتور (بیت‌مپ بخش‌ها، ETag و…) طبق اسپک
//! (docs/engine-spec.md بخش ۲ و ۵) در SQLite خود موتور می‌نشیند.

use crate::model::{DmSettings, DownloadRecord};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STORE_VERSION: u32 = 1;
pub const STORE_FILE_NAME: &str = "state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreData {
    pub version: u32,
    /// شمارنده‌ی ساخت شناسه‌ی دانلود (dl-1, dl-2, …) — باید بین اجراها بماند
    /// تا شناسه‌ها تکراری نشوند.
    pub next_seq: u64,
    pub settings: DmSettings,
    pub records: Vec<DownloadRecord>,
}

impl StoreData {
    pub fn new(default_download_dir: String) -> Self {
        Self {
            version: STORE_VERSION,
            next_seq: 1,
            settings: DmSettings::new(default_download_dir),
            records: Vec::new(),
        }
    }
}

/// خواندن وضعیت ذخیره‌شده؛ اگر فایل نبود یا خراب بود None برمی‌گردد
/// (برنامه با وضعیت پیش‌فرض بالا می‌آید و چیزی را از دست نمی‌دهیم جز تاریخچه).
pub fn load(path: &Path) -> Option<StoreData> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// ذخیره‌ی اتمیک: اول در فایل ‎.tmp کنار فایل اصلی نوشته می‌شود، بعد rename.
pub fn save(path: &Path, data: &StoreData) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("ساخت پوشه‌ی داده ناموفق بود: {e}"))?;
    }
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    let tmp: PathBuf = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("نوشتن وضعیت ناموفق بود: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("جایگزینی فایل وضعیت ناموفق بود: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DownloadStatus};

    fn sample() -> StoreData {
        let mut d = StoreData::new("/tmp/dl".into());
        d.next_seq = 7;
        d.records.push(DownloadRecord {
            id: "dl-1".into(),
            url: "https://example.com/f.zip".into(),
            file_name: "f.zip".into(),
            dest_path: "/tmp/dl/f.zip".into(),
            size: Some(100),
            downloaded: 40,
            speed: 0,
            status: DownloadStatus::Paused,
            error: None,
            created_at: "2026-10-23T01:40:44Z".into(),
        });
        d
    }

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("dm-store-test-{}", std::process::id()));
        let path = dir.join(STORE_FILE_NAME);
        let data = sample();
        save(&path, &data).unwrap();
        let back = load(&path).expect("باید خوانده شود");
        assert_eq!(back.next_seq, 7);
        assert_eq!(back.records.len(), 1);
        assert_eq!(back.records[0].downloaded, 40);
        assert_eq!(back.records[0].status, DownloadStatus::Paused);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_is_none() {
        assert!(load(Path::new("/nonexistent-dir-xyz/state.json")).is_none());
    }
}
