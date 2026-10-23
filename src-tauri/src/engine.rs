//! نقطه‌ی اتصال موتور دانلود — قرارداد بین «هسته» (صف/وضعیت/ذخیره) و «موتور»
//! (انتقال واقعی بایت‌ها). پیاده‌سازی موتور طبق اسپک docs/engine-spec.md است
//! (معماری Manager/Scheduler/Worker روی tokio + reqwest؛ بخش‌های ۲ تا ۹ اسپک)
//! و سهم ایجنت موتور است؛ هسته فقط از همین trait استفاده می‌کند و با عوض‌شدن
//! پیاده‌سازی موتور هیچ تغییری نمی‌کند.
//!
//! قرارداد رویدادها:
//! - موتور برای هر تسک روی همان کانالی که موقع start گرفته رویداد می‌فرستد.
//! - Progress ممکن است پُرفرکانس باشد (اسپک: حداکثر هر ۲۵۰ms برای هر تسک)؛
//!   هسته انتشار به UI و ذخیره روی دیسک را خودش throttle می‌کند.
//! - وضعیت‌های داخلی موتور (probing / retrying / verifying در بخش ۱۰ اسپک)
//!   به هسته نمی‌رسند؛ تا وقتی رویداد پایانی نیامده، تسک از دید هسته و UI
//!   «در حال دانلود» است. done در اسپک = Completed این‌جا، failed = Failed.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// کاری که هسته برای شروع/ادامه‌ی یک دانلود به موتور می‌دهد.
// فیلدها را مصرف‌کننده‌ی واقعی (موتور طبق اسپک) می‌خواند، نه هسته؛ تا وقتی
// PlaceholderEngine وصل است dead_code محسوب می‌شوند.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EngineTask {
    pub id: String,
    pub url: String,
    /// مسیر کامل فایل مقصد (پوشه‌ی دانلود کاربر + نام فایل)؛ موتور از اول
    /// فایل را با همین مسیر می‌سازد (طبق اسپک: preallocate + pwrite در آفست).
    pub dest_path: PathBuf,
    /// بایت دانلودشده‌ی قبلی روی دیسک — نقطه‌ی شروع مورد انتظار هسته.
    /// اعتبارسنجی واقعی resume (ETag/اندازه/بیت‌مپ بخش‌ها) با خود موتور است
    /// (اسپک بخش ۵)؛ اگر سرور عوض شده بود موتور از صفر می‌آید و با Progress
    /// بعدی عدد واقعی را گزارش می‌کند.
    pub resume_from: u64,
    /// سقف سرعت لحظه‌ای این تسک به بایت‌برثانیه؛ None = نامحدود.
    /// (سقف سراسری تنظیمات موقع هر start به موتور داده می‌شود و با
    /// set_speed_limit هم زنده به‌روز می‌شود.)
    pub speed_limit_bps: Option<u64>,
}

/// رویدادهای موتور به هسته.
// واریانت‌ها را موتور واقعی می‌سازد؛ هسته فقط مصرف می‌کند.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Progress {
        id: String,
        downloaded: u64,
        total: Option<u64>,
        speed_bps: u64,
    },
    Completed {
        id: String,
        total: u64,
    },
    Failed {
        id: String,
        error: String,
    },
}

/// موتور دانلود — پیاده‌سازی واقعی طبق docs/engine-spec.md می‌آید.
pub trait DownloadEngine: Send + Sync {
    /// شروع یا ادامه‌ی دانلود. رویدادها روی events فرستاده می‌شوند.
    /// خطای برگشتی یعنی «اصلاً شروع نشد» و هسته تسک را به وضعیت خطا می‌برد.
    fn start(&self, task: EngineTask, events: Sender<EngineEvent>) -> Result<(), String>;

    /// مکث انسانی (اسپک ۵.۳): توقف انتقال و بستن اتصال‌ها؛ فایل و وضعیت
    /// روی دیسک می‌مانند تا resume از همان آفست ادامه دهد.
    fn pause(&self, id: &str) -> Result<(), String>;

    /// لغو کامل از طرف هسته (حذف تسک): انتقال می‌ایستد؛ پاک‌کردن فایل
    /// مقصد با هسته است (طبق پرچم delete_file فرانت)، نه موتور.
    fn cancel(&self, id: &str) -> Result<(), String>;

    /// به‌روزرسانی زنده‌ی سقف سرعت سراسری؛ پیاده‌سازی پیش‌فرض: بی‌اثر
    /// (موتورهایی که throttle زنده ندارند، مقدار تازه را از start بعدی می‌گیرند).
    fn set_speed_limit(&self, _limit_bps: Option<u64>) {}
}

/// موتور جای‌نگهدار تا وقتی پیاده‌سازی اسپک برسد: هر start با خطای شفاف
/// رد می‌شود تا ماشین وضعیت هسته (صف → دانلود → خطا) و UI از همین حالا
/// قابل تست باشد و هیچ‌کس تصور نکند دانلودی واقعاً در جریان است.
pub struct PlaceholderEngine;

impl DownloadEngine for PlaceholderEngine {
    fn start(&self, _task: EngineTask, _events: Sender<EngineEvent>) -> Result<(), String> {
        Err("موتور دانلود هنوز پیاده‌سازی نشده است (در انتظار پیاده‌سازی طبق docs/engine-spec.md)".into())
    }

    fn pause(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    fn cancel(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}
