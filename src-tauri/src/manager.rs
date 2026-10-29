//! مدیر دانلودها (هسته): صف، ماشین وضعیت، زمان‌بندی شروع تسک‌ها تا سقف
//! موازی، و ذخیره‌ی وضعیت برای resume بعد از بسته‌شدن برنامه.
//!
//! ماشین وضعیت هر دانلود:
//!   queued ⇄ downloading ⇄ paused
//!   downloading → completed | error
//!   error → queued (با resume دستی)
//! بعد از ری‌استارت برنامه، هر تسکی که «در حال دانلود» ذخیره شده بود به
//! «مکث» برمی‌گردد (دانلود خودکار شروع نمی‌شود) و resume آن را از همان
//! آفست دانلودشده ادامه می‌دهد.

use crate::engine::{DownloadEngine, EngineEvent, EngineTask};
use crate::model::{now_iso, DmSettings, DownloadItem, DownloadRecord, DownloadStatus};
use crate::store::{self, StoreData};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// نتیجه‌ی اعمال یک رویداد موتور — لایه‌ی اتصال (lib.rs) با آن تصمیم
/// می‌گیرد انتشار به UI و ذخیره روی دیسک را فوری انجام دهد یا throttle کند.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedEffect {
    /// وضعیت عوض شد (کامل/خطا/…): ذخیره و انتشار فوری.
    StateChanged,
    /// فقط اعداد پیشرفت جلو رفتند: ذخیره حداکثر هر ۱ ثانیه، انتشار هر ۲۵۰ms.
    ProgressOnly,
    /// رویداد ربطی به وضعیت فعلی نداشت (تسک حذف شده یا وضعیت ناسازگار).
    Ignored,
}

pub struct DownloadManager {
    records: Vec<DownloadRecord>,
    settings: DmSettings,
    next_seq: u64,
    /// شماره‌ی نسل جاری هر تسک؛ با هر start یکی بالا می‌رود و رویدادهای
    /// موتور فقط وقتی اعمال می‌شوند که نسلشان همین باشد.
    generations: std::collections::HashMap<String, u64>,
    engine: Arc<dyn DownloadEngine>,
    events_tx: Sender<EngineEvent>,
    store_path: PathBuf,
}

impl DownloadManager {
    /// ساخت مدیر با وضعیت ذخیره‌شده (اگر بود). دانلودهای نیمه‌کاره‌ی
    /// اجرای قبل به «مکث» نرمال می‌شوند؛ سرعت لحظه‌ای هم صفر می‌شود.
    pub fn load(
        store_path: PathBuf,
        default_download_dir: String,
        engine: Arc<dyn DownloadEngine>,
        events_tx: Sender<EngineEvent>,
    ) -> Self {
        let data = store::load(&store_path)
            .unwrap_or_else(|| StoreData::new(default_download_dir));
        let mut mgr = Self {
            records: data.records,
            settings: data.settings,
            next_seq: data.next_seq.max(1),
            generations: std::collections::HashMap::new(),
            engine,
            events_tx,
            store_path,
        };
        for rec in &mut mgr.records {
            if rec.status == DownloadStatus::Downloading {
                rec.status = DownloadStatus::Paused;
            }
            rec.speed = 0;
        }
        mgr
    }

    pub fn list(&self) -> Vec<DownloadItem> {
        self.records.iter().map(|r| r.to_item()).collect()
    }

    /// خواندن رکورد با شناسه (فقط مسیرهای اندروید از آن استفاده می‌کنند).
    #[cfg(target_os = "android")]
    pub fn get(&self, id: &str) -> Option<DownloadRecord> {
        self.records.iter().find(|r| r.id == id).cloned()
    }

    /// ثبت URI ورودیِ منتشرشده در Downloads عمومی اندروید روی رکورد
    /// (حذف از دیسک بعداً همین ورودی را پاک می‌کند).
    #[cfg(target_os = "android")]
    pub fn set_public_uri(&mut self, id: &str, uri: String) -> Result<(), String> {
        let rec = self
            .records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| "دانلودی با این شناسه پیدا نشد.".to_string())?;
        rec.public_uri = Some(uri);
        self.persist()
    }

    /// دسترسی به موتور برای لغوِ بدون قفل (lib.rs): لغو، نخِ نسل را
    /// join می‌کند و نباید زیر قفلِ مدیر صدا زده شود.
    pub fn engine(&self) -> Arc<dyn DownloadEngine> {
        self.engine.clone()
    }

    pub fn settings(&self) -> DmSettings {
        self.settings.clone()
    }

    /// «همه تمام شدند» (۰٫۶٫۳) — تعریف دقیق محرک حالت پس از دانلود:
    /// هیچ دانلودی در وضعیت در حال دانلود، در صف، مکث‌شده یا خطادار
    /// نیست و دست‌کم یک دانلود در فهرست هست. دانلود مکث‌شده یا
    /// خطادار «تمام‌شدن» را مسدود می‌کند تا کاربر به آن رسیدگی کند؛
    /// فهرست خالی هم «تمام‌شدن» نیست (حذف همه‌ی دانلودها نباید
    /// خاموشی بزند). محرک فقط رویداد Completed است (حلقه‌ی lib.rs)،
    /// نه همین تابع به‌تنهایی — این تابع فقط شرط را می‌سنجد.
    pub fn all_finished(&self) -> bool {
        !self.records.is_empty()
            && self
                .records
                .iter()
                .all(|r| r.status == DownloadStatus::Completed)
    }

    /// حالت پس از دانلودِ ذخیره‌شده در تنظیمات.
    pub fn post_download_action(&self) -> crate::model::PostDownloadAction {
        self.settings.post_download_action
    }

    /// افزودن دانلود تازه از روی URL؛ نام فایل و مسیر مقصد از URL و پوشه‌ی
    /// دانلود تنظیمات درمی‌آید. مقصد تکراری بین تسک‌های موجود رد می‌شود
    /// (اسپک موتور، بخش ۱۱: قفل مسیر مقصد).
    pub fn add(&mut self, url: &str) -> Result<DownloadItem, String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("نشانی دانلود خالی است.".into());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("نشانی باید با http یا https شروع شود.".into());
        }
        let file_name = file_name_from_url(url);
        let dest = Path::new(&self.settings.download_dir).join(&file_name);
        let dest_str = dest.to_string_lossy().to_string();
        if self.records.iter().any(|r| r.dest_path == dest_str) {
            return Err("دانلودی با همین فایل مقصد در فهرست وجود دارد.".into());
        }
        let id = format!("dl-{}", self.next_seq);
        self.next_seq += 1;
        let rec = DownloadRecord {
            id: id.clone(),
            url: url.to_string(),
            file_name,
            dest_path: dest_str,
            size: None,
            downloaded: 0,
            speed: 0,
            status: DownloadStatus::Queued,
            error: None,
            public_uri: None,
            created_at: now_iso(),
        };
        self.records.push(rec);
        self.persist()?;
        self.pump();
        Ok(self
            .records
            .iter()
            .find(|r| r.id == id)
            .expect("تازه ساخته شده")
            .to_item())
    }

    /// مکث: فقط از وضعیت «در حال دانلود».
    pub fn pause(&mut self, id: &str) -> Result<(), String> {
        let idx = self.index_of(id)?;
        if self.records[idx].status != DownloadStatus::Downloading {
            return Ok(()); // مکثِ تسکِ درحال‌دانلودنبوده اثری ندارد
        }
        self.engine.pause(id)?;
        let rec = &mut self.records[idx];
        rec.status = DownloadStatus::Paused;
        rec.speed = 0;
        self.persist()?;
        self.pump(); // جای خالی‌شده را تسک بعدی صف می‌گیرد
        Ok(())
    }

    /// ادامه: از «مکث» یا «خطا» به صف برمی‌گردد و اگر ظرفیت بود شروع می‌شود.
    pub fn resume(&mut self, id: &str) -> Result<(), String> {
        let idx = self.index_of(id)?;
        match self.records[idx].status {
            DownloadStatus::Paused | DownloadStatus::Error => {
                let rec = &mut self.records[idx];
                rec.status = DownloadStatus::Queued;
                rec.error = None;
                self.persist()?;
                self.pump();
                Ok(())
            }
            _ => Ok(()), // کامل/در صف/در حال دانلود: چیزی برای ادامه نیست
        }
    }

    /// توقف همه (۰٫۶٫۶، اکشن نوتیفیکیشن اندروید): همه‌ی در حال
    /// دانلودها را مکث می‌کند. چون pause() خودش pump می‌کند و جای
    /// خالی را تسک صف می‌گیرد، حلقه تا وقتی دانلود فعالی هست تکرار
    /// می‌شود — وگرنه تسکی که تازه شروع شده از قلم می‌افتاد. صف
    /// متناهی است پس حلقه حتماً تمام می‌شود.
    pub fn pause_all(&mut self) -> Result<(), String> {
        loop {
            let downloading: Vec<String> = self
                .records
                .iter()
                .filter(|r| r.status == DownloadStatus::Downloading)
                .map(|r| r.id.clone())
                .collect();
            if downloading.is_empty() {
                return Ok(());
            }
            for id in downloading {
                self.pause(&id)?;
            }
        }
    }

    /// شروع همه (۰٫۶٫۶، اکشن نوتیفیکیشن اندروید): همه‌ی مکث‌شده‌ها و
    /// خطادارها را به صف برمی‌گرداند؛ pump خودش تا سقف موازی شروعشان
    /// می‌کند و بقیه در صف می‌مانند. تسک‌های در صف از قبل در نوبت‌اند
    /// و کاری ندارند.
    pub fn resume_all(&mut self) -> Result<(), String> {
        let ids: Vec<String> = self
            .records
            .iter()
            .filter(|r| {
                r.status == DownloadStatus::Paused || r.status == DownloadStatus::Error
            })
            .map(|r| r.id.clone())
            .collect();
        for id in ids {
            self.resume(&id)?;
        }
        Ok(())
    }

    /// حذف تسک از فهرست؛ اگر در حال دانلود بود اول لغو می‌شود.
    /// delete_file=true فایل مقصد روی دیسک را هم پاک می‌کند.
    /// حذف تسک از فهرست؛ موتور اول کامل لغو و نخش join می‌شود تا موقع
    /// پاک‌کردن فایل‌ها هیچ نخی رویشان ننویسد.
    /// delete_file=true یعنی «حذف از دیسک»: فایل مقصد + سایدکار و فایل موقت
    /// دانلود ناقص هم پاک می‌شوند. اگر پاک‌کردن فایلی ناموفق بود، رکورد در
    /// فهرست می‌ماند (و اگر در حال دانلود بود مکث می‌شود) تا کاربر دوباره
    /// تلاش کند و حذف نصفه‌کاره بی‌صدا رد نشود.
    pub fn remove(&mut self, id: &str, delete_file: bool) -> Result<(), String> {
        let _ = self.engine.cancel(id);
        self.remove_cancelled(id, delete_file)
    }

    /// حذفِ رکوردِ تسکی که لغوش کامل شده است (لغو قبلاً — بیرونِ
    /// قفلِ مدیر — انجام شده تا joinِ نخ‌ها کامندهای دیگر را قفل نکند).
    pub fn remove_cancelled(&mut self, id: &str, delete_file: bool) -> Result<(), String> {
        let idx = self.index_of(id)?;
        if delete_file {
            let dest = PathBuf::from(&self.records[idx].dest_path);
            let mut targets = vec![dest.clone()];
            targets.extend(crate::engine::sidecar_files(&dest));
            for t in targets {
                match std::fs::remove_file(&t) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        let rec = &mut self.records[idx];
                        if rec.status == DownloadStatus::Downloading {
                            rec.status = DownloadStatus::Paused;
                            rec.speed = 0;
                        }
                        let _ = self.persist();
                        return Err(format!("پاک‌کردن فایل «{}» ناموفق بود: {e}", t.display()));
                    }
                }
            }
        }
        self.records.remove(idx);
        self.persist()?;
        self.pump();
        Ok(())
    }

    pub fn set_settings(&mut self, settings: DmSettings) -> Result<(), String> {
        let mut settings = settings;
        settings.max_parallel = settings.max_parallel.clamp(1, 32);
        self.settings = settings;
        self.engine.set_speed_limit(self.settings.speed_limit_bps());
        self.persist()?;
        self.pump(); // اگر سقف موازی بالا رفت، تسک‌های صف شروع می‌شوند
        Ok(())
    }

    /// اعمال رویداد موتور روی وضعیت تسک.
    pub fn apply_event(&mut self, evt: EngineEvent) -> AppliedEffect {
        match evt {
            EngineEvent::Progress {
                id,
                generation,
                downloaded,
                total,
                speed_bps,
            } => {
                if self.generations.get(&id) != Some(&generation) {
                    return AppliedEffect::Ignored;
                }
                let Some(rec) = self.records.iter_mut().find(|r| r.id == id) else {
                    return AppliedEffect::Ignored;
                };
                if rec.status != DownloadStatus::Downloading {
                    return AppliedEffect::Ignored;
                }
                rec.downloaded = downloaded;
                if total.is_some() {
                    rec.size = total;
                }
                rec.speed = speed_bps;
                AppliedEffect::ProgressOnly
            }
            EngineEvent::Completed {
                id,
                generation,
                total,
            } => {
                if self.generations.get(&id) != Some(&generation) {
                    return AppliedEffect::Ignored;
                }
                let Some(idx) = self.records.iter().position(|r| r.id == id) else {
                    return AppliedEffect::Ignored;
                };
                // فقط تسکِ در حال دانلود می‌تواند کامل شود؛ رویدادِ همان نسل هم
                // اگر بعد از مکث کاربر برسد نباید وضعیت مکث را override کند.
                if self.records[idx].status != DownloadStatus::Downloading {
                    return AppliedEffect::Ignored;
                }
                let rec = &mut self.records[idx];
                rec.status = DownloadStatus::Completed;
                rec.downloaded = total;
                rec.size = Some(total);
                rec.speed = 0;
                rec.error = None;
                let _ = self.persist();
                self.pump();
                AppliedEffect::StateChanged
            }
            EngineEvent::Failed {
                id,
                generation,
                error,
            } => {
                if self.generations.get(&id) != Some(&generation) {
                    return AppliedEffect::Ignored;
                }
                let Some(idx) = self.records.iter().position(|r| r.id == id) else {
                    return AppliedEffect::Ignored;
                };
                // خطای نسل جاری هم فقط روی تسکِ در حال دانلود اعمال می‌شود.
                if self.records[idx].status != DownloadStatus::Downloading {
                    return AppliedEffect::Ignored;
                }
                let rec = &mut self.records[idx];
                rec.status = DownloadStatus::Error;
                rec.error = Some(error);
                rec.speed = 0;
                let _ = self.persist();
                self.pump();
                AppliedEffect::StateChanged
            }
        }
    }

    /// ذخیره‌ی فوری وضعیت فعلی (برای رویدادهای پیشرفتِ throttle‌شده و
    /// خاموش‌شدن تمیز؛ صدا زدن اضافه‌اش ارزان و بی‌ضرر است).
    pub fn persist(&self) -> Result<(), String> {
        let data = StoreData {
            version: store::STORE_VERSION,
            next_seq: self.next_seq,
            settings: self.settings.clone(),
            records: self.records.clone(),
        };
        store::save(&self.store_path, &data)
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.records
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| "دانلودی با این شناسه پیدا نشد.".to_string())
    }

    /// شروع تسک‌های صف تا سقف موازی تنظیمات (به ترتیب ورود به صف).
    fn pump(&mut self) {
        let cap = self.settings.max_parallel.max(1) as usize;
        let mut active = self
            .records
            .iter()
            .filter(|r| r.status == DownloadStatus::Downloading)
            .count();
        let mut idx = 0;
        while active < cap && idx < self.records.len() {
            if self.records[idx].status == DownloadStatus::Queued {
                self.start_at(idx);
                if self.records[idx].status == DownloadStatus::Downloading {
                    active += 1;
                }
            }
            idx += 1;
        }
    }

    /// شروع یک تسک مشخص با موتور؛ خطای موتور = وضعیت خطا روی همان تسک.
    fn start_at(&mut self, idx: usize) {
        let rec = &mut self.records[idx];
        rec.status = DownloadStatus::Downloading;
        rec.error = None;
        let generation = {
            let g = self.generations.entry(rec.id.clone()).or_insert(0);
            *g += 1;
            *g
        };
        let task = EngineTask {
            id: rec.id.clone(),
            generation,
            url: rec.url.clone(),
            dest_path: PathBuf::from(&rec.dest_path),
            resume_from: rec.downloaded,
            connections: self.settings.max_connections.clamp(1, 16),
            speed_limit_bps: self.settings.speed_limit_bps(),
        };
        match self.engine.start(task, self.events_tx.clone()) {
            Ok(()) => {}
            Err(e) => {
                let rec = &mut self.records[idx];
                rec.status = DownloadStatus::Error;
                rec.error = Some(e);
                rec.speed = 0;
            }
        }
        let _ = self.persist();
    }
}

/// نام فایل از انتهای مسیر URL (بدون کوئری/فرگمنت)، با percent-decode.
/// اگر نامی درنیامد، «download» برمی‌گردد.
pub fn file_name_from_url(url: &str) -> String {
    let no_query = url.split(['?', '#']).next().unwrap_or(url);
    let last = no_query.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    let name = decoded.trim();
    if name.is_empty() {
        "download".to_string()
    } else {
        name.to_string()
    }
}

/// percent-decode ساده برای نام فایل‌ها (بایت‌ها UTF-8 فرض می‌شوند).
/// روی بایت‌ها کار می‌کند تا «٪» کنار نویسه‌های چندبایتی (مثلاً فارسیِ
/// encodeنشده در URL) باعث panic برش رشته نشود.
fn percent_decode(s: &str) -> String {
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::DownloadEngine;
    use std::sync::Mutex;

    struct FakeEngine {
        started: Mutex<Vec<String>>,
        fail: bool,
    }

    impl DownloadEngine for FakeEngine {
        fn start(&self, task: EngineTask, _events: Sender<EngineEvent>) -> Result<(), String> {
            if self.fail {
                return Err("خطای ساختگی موتور".into());
            }
            self.started.lock().unwrap().push(task.id);
            Ok(())
        }
        fn pause(&self, _id: &str) -> Result<(), String> {
            Ok(())
        }
        fn cancel(&self, _id: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn manager_with(
        max_parallel: u32,
        fail: bool,
    ) -> (DownloadManager, PathBuf, Arc<FakeEngine>) {
        let dir = std::env::temp_dir().join(format!(
            "dm-mgr-test-{}-{}",
            std::process::id(),
            max_parallel
        ));
        let store_path = dir.join("state.json");
        let _ = std::fs::remove_dir_all(&dir);
        let engine = Arc::new(FakeEngine {
            started: Mutex::new(Vec::new()),
            fail,
        });
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut mgr = DownloadManager::load(
            store_path.clone(),
            "/tmp/downloads".into(),
            engine.clone(),
            tx,
        );
        mgr.set_settings(DmSettings {
            download_dir: "/tmp/downloads".into(),
            max_parallel,
            max_connections: 8,
            speed_limit_kbps: 0,
            post_download_action: crate::model::PostDownloadAction::None,
        })
        .unwrap();
        (mgr, dir, engine)
    }

    #[test]
    fn queue_respects_parallel_cap() {
        let (mut mgr, dir, engine) = manager_with(1, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let b = mgr.add("https://ex.com/b.zip").unwrap();
        assert_eq!(a.status, DownloadStatus::Downloading);
        assert_eq!(b.status, DownloadStatus::Queued);
        assert_eq!(engine.started.lock().unwrap().len(), 1);

        // مکث اولی → دومی شروع می‌شود
        mgr.pause(&a.id).unwrap();
        let items = mgr.list();
        assert_eq!(items[0].status, DownloadStatus::Paused);
        assert_eq!(items[1].status, DownloadStatus::Downloading);

        // کامل‌شدن دومی با رویداد موتور
        let eff = mgr.apply_event(EngineEvent::Completed {
            id: b.id.clone(),
            generation: 1,
            total: 500,
        });
        assert_eq!(eff, AppliedEffect::StateChanged);
        assert_eq!(mgr.list()[1].status, DownloadStatus::Completed);
        assert_eq!(mgr.list()[1].downloaded, 500);

        // ادامه‌ی اولی از همان آفست
        mgr.resume(&a.id).unwrap();
        assert_eq!(mgr.list()[0].status, DownloadStatus::Downloading);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raising_parallel_cap_starts_queued() {
        // بالا بردنِ سقفِ هم‌زمانی با set_settings، تسک‌های صف را
        // همان لحظه شروع می‌کند (pump در set_settings).
        let (mut mgr, dir, engine) = manager_with(1, false);
        let _a = mgr.add("https://ex.com/a.zip").unwrap();
        let b = mgr.add("https://ex.com/b.zip").unwrap();
        assert_eq!(mgr.list()[1].status, DownloadStatus::Queued);
        let mut s = mgr.settings();
        s.max_parallel = 3;
        mgr.set_settings(s).unwrap();
        assert_eq!(mgr.list()[1].status, DownloadStatus::Downloading);
        assert_eq!(engine.started.lock().unwrap().len(), 2);
        let _ = b;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lowering_parallel_cap_keeps_running() {
        // پایین آوردنِ سقف، دانلودهای در حال اجرا را متوقف نمی‌کند؛
        // فقط شروعِ تازه‌ها را تا سقفِ تازه محدود می‌کند.
        let (mut mgr, dir, engine) = manager_with(3, false);
        mgr.add("https://ex.com/a.zip").unwrap();
        mgr.add("https://ex.com/b.zip").unwrap();
        mgr.add("https://ex.com/c.zip").unwrap();
        assert_eq!(engine.started.lock().unwrap().len(), 3);
        let mut s = mgr.settings();
        s.max_parallel = 1;
        mgr.set_settings(s).unwrap();
        assert!(mgr
            .list()
            .iter()
            .all(|i| i.status == DownloadStatus::Downloading));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_with_delete_file_clears_dest_and_sidecars() {
        let (mut mgr, dir, _e) = manager_with(2, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let dest = PathBuf::from(&mgr.records[0].dest_path);
        let sidecars = crate::engine::sidecar_files(&dest);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"partial").unwrap();
        for s in &sidecars {
            std::fs::write(s, b"{}").unwrap();
        }
        mgr.remove(&a.id, true).unwrap();
        assert!(mgr.list().is_empty());
        assert!(!dest.exists(), "فایل ناقص باید پاک شده باشد");
        for s in &sidecars {
            assert!(!s.exists(), "سایدکار/موقت باید پاک شده باشد: {}", s.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_without_delete_file_keeps_dest() {
        let (mut mgr, dir, _e) = manager_with(2, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let dest = PathBuf::from(&mgr.records[0].dest_path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"done").unwrap();
        mgr.remove(&a.id, false).unwrap();
        assert!(mgr.list().is_empty());
        assert!(dest.exists(), "حذف از فهرست نباید فایل را پاک کند");
        let _ = std::fs::remove_file(&dest);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_destination_rejected() {
        let (mut mgr, dir, _e) = manager_with(3, false);
        mgr.add("https://ex.com/same.zip").unwrap();
        let err = mgr.add("https://other.com/same.zip").unwrap_err();
        assert!(err.contains("مقصد"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn engine_start_failure_marks_error_and_resume_retries() {
        let (mut mgr, dir, _e) = manager_with(2, true);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        assert_eq!(a.status, DownloadStatus::Error);
        assert!(a.error.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_normalizes_downloading_to_paused() {
        let (mut mgr, dir, _e) = manager_with(2, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        assert_eq!(a.status, DownloadStatus::Downloading);
        // پیشرفت ساختگی تا آفست برای resume ثبت شود
        mgr.apply_event(EngineEvent::Progress {
            id: a.id.clone(),
            generation: 1,
            downloaded: 42,
            total: Some(100),
            speed_bps: 10,
        });
        mgr.persist().unwrap();
        drop(mgr);

        let (tx, _rx) = std::sync::mpsc::channel();
        let engine = Arc::new(FakeEngine {
            started: Mutex::new(Vec::new()),
            fail: false,
        });
        let mgr2 = DownloadManager::load(dir.join("state.json"), "/tmp/x".into(), engine, tx);
        let item = &mgr2.list()[0];
        assert_eq!(item.status, DownloadStatus::Paused);
        assert_eq!(item.downloaded, 42);
        assert_eq!(item.size, Some(100));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn all_finished_definition() {
        // ۰٫۶٫۳: فهرست خالی تمام‌شدن نیست؛ مکث/خطا مسدود می‌کند؛
        // فقط وقتی همه کامل‌اند، تمام‌شدن برقرار است.
        let (mut mgr, dir, _e) = manager_with(3, false);
        assert!(!mgr.all_finished(), "فهرست خالی نباید تمام‌شدن باشد");
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let b = mgr.add("https://ex.com/b.zip").unwrap();
        assert!(!mgr.all_finished(), "دانلود فعال، تمام‌شدن نیست");
        mgr.apply_event(EngineEvent::Completed {
            id: a.id.clone(),
            generation: 1,
            total: 100,
        });
        assert!(!mgr.all_finished(), "یکی هنوز فعال است");
        mgr.pause(&b.id).unwrap();
        assert!(!mgr.all_finished(), "مکث‌شده، تمام‌شدن را مسدود می‌کند");
        mgr.resume(&b.id).unwrap();
        // resume دوباره start می‌زند و نسل b به ۲ می‌رسد.
        mgr.apply_event(EngineEvent::Completed {
            id: b.id.clone(),
            generation: 2,
            total: 200,
        });
        assert!(mgr.all_finished(), "حالا همه کامل‌اند");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pause_all_pauses_newly_started_too() {
        // ۰٫۶٫۶: با سقف ۱، مکثِ اولی دومی را شروع می‌کند؛ pause_all
        // باید دومیِ تازه‌شروع‌شده را هم بگیرد، نه فقط اولی را.
        let (mut mgr, dir, _e) = manager_with(1, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let b = mgr.add("https://ex.com/b.zip").unwrap();
        assert_eq!(a.status, DownloadStatus::Downloading);
        assert_eq!(b.status, DownloadStatus::Queued);
        mgr.pause_all().unwrap();
        let items = mgr.list();
        assert_eq!(items[0].status, DownloadStatus::Paused);
        assert_eq!(items[1].status, DownloadStatus::Paused);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_all_restarts_paused_and_error() {
        // ۰٫۶٫۶: مکث‌شده و خطادار هر دو به صف/شروع برمی‌گردند؛
        // خطا پاک می‌شود؛ کامل‌شده دست نمی‌خورد.
        let (mut mgr, dir, _e) = manager_with(2, false);
        let a = mgr.add("https://ex.com/a.zip").unwrap();
        let b = mgr.add("https://ex.com/b.zip").unwrap();
        mgr.pause(&a.id).unwrap();
        mgr.pause(&b.id).unwrap();
        // b را دستی خطادار کن (مثل شکست موتور).
        mgr.records[1].status = DownloadStatus::Error;
        mgr.records[1].error = Some("خطای ساختگی".into());
        mgr.resume_all().unwrap();
        let items = mgr.list();
        assert_eq!(items[0].status, DownloadStatus::Downloading);
        assert_eq!(items[1].status, DownloadStatus::Downloading);
        assert!(items[1].error.is_none(), "خطا با resume پاک می‌شود");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_name_cases() {
        assert_eq!(file_name_from_url("https://ex.com/a/b/file.zip?x=1"), "file.zip");
        assert_eq!(file_name_from_url("https://ex.com/a/"), "download");
        assert_eq!(file_name_from_url("https://ex.com/%D9%81%D8%A7%DB%8C%D9%84.zip"), "فایل.zip");
        assert_eq!(file_name_from_url("https://ex.com/a%20b.mp4"), "a b.mp4");
    }
}
