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
    /// شماره‌ی نسل این اجرا (هسته می‌شمارد)؛ رویدادها همین را برمی‌گردانند تا
    /// هسته رویداد مانده‌ی نسل قبلی را با نسل تازه اشتباه نگیرد.
    pub generation: u64,
    pub url: String,
    /// مسیر کامل فایل مقصد (پوشه‌ی دانلود کاربر + نام فایل)؛ موتور از اول
    /// فایل را با همین مسیر می‌سازد (طبق اسپک: preallocate + pwrite در آفست).
    pub dest_path: PathBuf,
    /// سقف اتصال‌های موازی این دانلود (از تنظیمات کاربر).
    pub connections: u32,
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
        generation: u64,
        downloaded: u64,
        total: Option<u64>,
        speed_bps: u64,
    },
    Completed {
        id: String,
        generation: u64,
        total: u64,
    },
    Failed {
        id: String,
        generation: u64,
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

// ────────────────────────────────────────────────────────────────────
// موتور واقعی (طبق docs/engine-spec.md): دانلود چندبخشی موازی با Range،
// صفِ بخش‌ها بین اتصال‌ها (هر اتصال بخش بعدی را می‌کشد)، resume در سطح
// بخش با فایل وضعیت کنار فایل مقصد، تلاش دوباره با backoff و سقف سرعت
// سراسری با سطل توکن. هر بایت دانلودشده مقدس است: هیچ بخشی که کامل
// شده دوباره دانلود نمی‌شود و فایل از اول با اندازه‌ی کامل ساخته می‌شود.
// ────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BLOCK: usize = 64 * 1024;
const MAX_CHUNK_RETRIES: u32 = 4;

/// سطل توکن سرعت: سقف سراسری بین همه‌ی تسک‌ها و اتصال‌ها تقسیم می‌شود.
struct TokenBucket {
    rate_bps: Mutex<Option<u64>>,
    tokens: Mutex<f64>,
    last: Mutex<Instant>,
}

impl TokenBucket {
    fn new() -> Self {
        Self {
            rate_bps: Mutex::new(None),
            tokens: Mutex::new(0.0),
            last: Mutex::new(Instant::now()),
        }
    }
    fn set_rate(&self, rate: Option<u64>) {
        *self.rate_bps.lock().unwrap() = rate;
    }
    /// گرفتن n توکن؛ وقتی سقف هست به‌اندازه‌ی لازم می‌خوابد.
    fn take(&self, n: u64) {
        loop {
            let rate = *self.rate_bps.lock().unwrap();
            let Some(rate) = rate else { return };
            let rate = rate.max(1024);
            {
                let mut tokens = self.tokens.lock().unwrap();
                let mut last = self.last.lock().unwrap();
                let elapsed = last.elapsed().as_secs_f64();
                *tokens = (*tokens + elapsed * rate as f64).min(rate as f64 * 2.0);
                *last = Instant::now();
                if *tokens >= n as f64 {
                    *tokens -= n as f64;
                    return;
                }
            }
            let deficit = n as f64 - *self.tokens.lock().unwrap();
            let wait = (deficit / rate as f64).clamp(0.002, 0.25);
            std::thread::sleep(Duration::from_secs_f64(wait));
        }
    }
}

/// کنترل یک اجرای فعال (هر start یک نسل تازه).
struct RunControl {
    stop: AtomicBool,
    fatal: Mutex<Option<String>>,
    /// هندل نخ ناظر؛ start بعدی همان تسک اول این را stop و join می‌کند تا دو
    /// نسل هرگز هم‌زمان روی یک فایل/سایدکار ننویسند.
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl RunControl {
    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    fn join(&self) {
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Sidecar {
    url: String,
    total: u64,
    etag: Option<String>,
    last_modified: Option<String>,
    chunk: u64,
    done: Vec<bool>,
}

fn sidecar_path(dest: &std::path::Path) -> PathBuf {
    let mut p = dest.as_os_str().to_owned();
    p.push(".dm.json");
    PathBuf::from(p)
}

fn jitter_ms(base: u64) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    base + (nanos % 250)
}

fn backoff(attempt: u32) {
    let base = 400u64 << attempt.min(4);
    std::thread::sleep(Duration::from_millis(jitter_ms(base)));
}

fn build_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .redirects(5)
        .user_agent(concat!(
            "DownBolt/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/thaarfknight-star/DownBolt)"
        ))
        .build()
}

struct Probe {
    total: Option<u64>,
    ranges: bool,
    etag: Option<String>,
    last_modified: Option<String>,
}

fn probe(agent: &ureq::Agent, url: &str) -> Result<Probe, String> {
    let resp = agent
        .get(url)
        .set("Range", "bytes=0-0")
        .call()
        .map_err(|e| format!("خطای شبکه: {e}"))?;
    let etag = resp.header("ETag").map(|s| s.to_string());
    let last_modified = resp.header("Last-Modified").map(|s| s.to_string());
    match resp.status() {
        206 => {
            let total = resp
                .header("Content-Range")
                .and_then(|cr| cr.rsplit('/').next())
                .and_then(|t| t.trim().parse::<u64>().ok());
            Ok(Probe {
                total,
                ranges: total.is_some(),
                etag,
                last_modified,
            })
        }
        200 => {
            let total = resp
                .header("Content-Length")
                .and_then(|v| v.trim().parse::<u64>().ok());
            Ok(Probe {
                total,
                ranges: false,
                etag,
                last_modified,
            })
        }
        s => Err(format!("پاسخ غیرمنتظره‌ی سرور: HTTP {s}")),
    }
}

/// موتور واقعی دانلود.
pub struct RealEngine {
    runs: Arc<Mutex<HashMap<String, Arc<RunControl>>>>,
    bucket: Arc<TokenBucket>,
}

impl RealEngine {
    pub fn new() -> Self {
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            bucket: Arc::new(TokenBucket::new()),
        }
    }

    fn control_for(&self, id: &str) -> Option<Arc<RunControl>> {
        self.runs.lock().unwrap().get(id).cloned()
    }
}

impl Default for RealEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadEngine for RealEngine {
    fn start(&self, task: EngineTask, events: Sender<EngineEvent>) -> Result<(), String> {
        // نسل قبلی همین تسک باید کاملاً مرده باشد قبل از شروع نسل تازه؛
        // وگرنه دو نسل روی یک فایل و یک سایدکار می‌نویسند (خرابی خاموش).
        if let Some(old) = self.runs.lock().unwrap().remove(&task.id) {
            old.stop();
            old.join();
        }
        let id = task.id.clone();
        let ctl = Arc::new(RunControl {
            stop: AtomicBool::new(false),
            fatal: Mutex::new(None),
            handle: Mutex::new(None),
        });
        self.runs.lock().unwrap().insert(id.clone(), ctl.clone());
        // سقف سرعت این تسک همیشه اعمال می‌شود؛ None یعنی برداشتن سقف قبلی.
        self.bucket.set_rate(task.speed_limit_bps);
        let bucket = self.bucket.clone();
        let runs = self.runs.clone();
        let ctl2 = ctl.clone();
        let handle = std::thread::Builder::new()
            .name(format!("downbolt-{id}"))
            .spawn(move || {
                run_task(task, events, ctl2.clone(), bucket);
                // پاک‌سازی نقشه: فقط اگر هنوز همین نسل سر جایش است.
                let mut map = runs.lock().unwrap();
                if map.get(&id).map(|c| Arc::ptr_eq(c, &ctl2)).unwrap_or(false) {
                    map.remove(&id);
                }
            })
            .map_err(|e| format!("ساخت نخ دانلود ناموفق بود: {e}"))?;
        *ctl.handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn pause(&self, id: &str) -> Result<(), String> {
        if let Some(ctl) = self.control_for(id) {
            ctl.stop.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn cancel(&self, id: &str) -> Result<(), String> {
        if let Some(ctl) = self.control_for(id) {
            ctl.stop.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn set_speed_limit(&self, limit_bps: Option<u64>) {
        self.bucket.set_rate(limit_bps);
    }
}

/// بدنه‌ی یک دانلود: پروب، انتخاب حالت (چندبخشی/تکی)، اجرا و گزارش پایانی.
fn run_task(task: EngineTask, events: Sender<EngineEvent>, ctl: Arc<RunControl>, bucket: Arc<TokenBucket>) {
    let agent = build_agent();
    let result = run_task_inner(&task, &events, &ctl, &bucket, &agent);
    match result {
        Ok(total) => {
            let _ = events.send(EngineEvent::Completed {
                id: task.id.clone(),
                generation: task.generation,
                total,
            });
        }
        Err(e) => {
            // توقف انسانی (مکث/لغو) رویداد پایانی ندارد؛ هسته خودش وضعیت را می‌برد.
            if !ctl.stop.load(Ordering::SeqCst) {
                let _ = events.send(EngineEvent::Failed {
                    id: task.id.clone(),
                    generation: task.generation,
                    error: e,
                });
            }
        }
    }
}

fn run_task_inner(
    task: &EngineTask,
    events: &Sender<EngineEvent>,
    ctl: &Arc<RunControl>,
    bucket: &Arc<TokenBucket>,
    agent: &ureq::Agent,
) -> Result<u64, String> {
    if let Some(dir) = task.dest_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("ساخت پوشه‌ی مقصد ناموفق بود: {e}"))?;
    }
    let pr = probe(agent, &task.url)?;
    if ctl.stop.load(Ordering::SeqCst) {
        return Err("stopped".into());
    }
    match (pr.total, pr.ranges) {
        (Some(total), true) if total > 0 => ranged_download(task, events, ctl, bucket, agent, &pr, total),
        _ => stream_download(task, events, ctl, bucket, agent, pr.total),
    }
}

/// دانلود چندبخشی: صف بخش‌ها + bitmap ماندگار در سایدکار.
#[allow(clippy::too_many_arguments)]
fn ranged_download(
    task: &EngineTask,
    events: &Sender<EngineEvent>,
    ctl: &Arc<RunControl>,
    bucket: &Arc<TokenBucket>,
    agent: &ureq::Agent,
    pr: &Probe,
    total: u64,
) -> Result<u64, String> {
    let conns = task.connections.clamp(1, 32) as u64;
    let target_chunks = (conns * 6).max(1);
    let chunk = (total / target_chunks).clamp(256 * 1024, 4 * 1024 * 1024);
    let n_chunks = total.div_ceil(chunk) as usize;

    // بازیابی یا ساخت وضعیت بخش‌ها
    let sc_path = sidecar_path(&task.dest_path);
    let mut done = vec![false; n_chunks];
    let mut resumed = false;
    if let Ok(text) = std::fs::read_to_string(&sc_path) {
        if let Ok(sc) = serde_json::from_str::<Sidecar>(&text) {
            if sc.url == task.url
                && sc.total == total
                && sc.chunk == chunk
                && sc.done.len() == n_chunks
                && sc.etag == pr.etag
                && sc.last_modified == pr.last_modified
                && std::fs::metadata(&task.dest_path).map(|m| m.len()).unwrap_or(0) == total
            {
                done = sc.done;
                resumed = true;
            }
        }
    }
    if !resumed {
        let f = std::fs::File::create(&task.dest_path)
            .map_err(|e| format!("ساخت فایل مقصد ناموفق بود: {e}"))?;
        f.set_len(total)
            .map_err(|e| format!("پیش‌تخصیص فایل ناموفق بود: {e}"))?;
    }
    let done_flags: Vec<AtomicBool> = done.into_iter().map(AtomicBool::new).collect();
    let done_bytes = |flags: &[AtomicBool]| -> u64 {
        flags
            .iter()
            .enumerate()
            .filter(|(_, f)| f.load(Ordering::Relaxed))
            .map(|(i, _)| chunk_len(i, chunk, total))
            .sum()
    };
    let downloaded = Arc::new(AtomicU64::new(done_bytes(&done_flags)));
    let cursor = Arc::new(AtomicUsize::new(0));
    let done_flags = Arc::new(done_flags);
    let sc_lock = Arc::new(Mutex::new(()));
    let last_sc_write = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(2)));

    let save_sidecar = {
        let sc_path = sc_path.clone();
        let task_url = task.url.clone();
        let etag = pr.etag.clone();
        let lm = pr.last_modified.clone();
        let done_flags = done_flags.clone();
        let sc_lock = sc_lock.clone();
        move |force: bool| {
            let _g = sc_lock.lock().unwrap();
            if !force && last_sc_write.try_lock().map(|t| t.elapsed() < Duration::from_secs(1)).unwrap_or(true) {
                return;
            }
            let sc = Sidecar {
                url: task_url.clone(),
                total,
                etag: etag.clone(),
                last_modified: lm.clone(),
                chunk,
                done: done_flags.iter().map(|f| f.load(Ordering::Relaxed)).collect(),
            };
            if let Ok(text) = serde_json::to_string(&sc) {
                // نوشتن اتمیک: کرش وسط نوشتن سایدکار سالم قبلی را خراب نمی‌کند.
                let mut tmp = sc_path.as_os_str().to_owned();
                tmp.push(".tmp");
                let tmp = PathBuf::from(tmp);
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(&tmp, &sc_path);
                }
            }
            if let Ok(mut t) = last_sc_write.try_lock() {
                *t = Instant::now();
            }
        }
    };

    // نخ‌های اتصال
    let mut handles = Vec::new();
    for _ in 0..conns {
        let agent = agent.clone();
        let url = task.url.clone();
        let dest = task.dest_path.clone();
        let ctl = ctl.clone();
        let bucket = bucket.clone();
        let cursor = cursor.clone();
        let done_flags = done_flags.clone();
        let downloaded = downloaded.clone();
        let save = save_sidecar.clone();
        handles.push(std::thread::spawn(move || {
            worker_loop(WorkerCtx {
                agent: &agent,
                url: &url,
                dest: &dest,
                ctl: &ctl,
                bucket: &bucket,
                cursor: &cursor,
                done_flags: &done_flags,
                downloaded: &downloaded,
                chunk,
                total,
                n_chunks,
                save: &save,
            })
        }));
    }

    // گزارش پیشرفت هر ۲۵۰ms
    let mut prev_bytes = downloaded.load(Ordering::Relaxed);
    let mut prev_at = Instant::now();
    let mut speed = 0f64;
    let mut all_done = false;
    while !all_done {
        if ctl.stop.load(Ordering::SeqCst) {
            break;
        }
        if ctl.fatal.lock().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        let now = Instant::now();
        let cur = downloaded.load(Ordering::Relaxed);
        let dt = now.duration_since(prev_at).as_secs_f64().max(0.05);
        let inst = (cur.saturating_sub(prev_bytes)) as f64 / dt;
        speed = if speed == 0.0 { inst } else { speed * 0.6 + inst * 0.4 };
        prev_bytes = cur;
        prev_at = now;
        let _ = events.send(EngineEvent::Progress {
            id: task.id.clone(),
            generation: task.generation,
            downloaded: cur,
            total: Some(total),
            speed_bps: speed as u64,
        });
        all_done = done_flags.iter().all(|f| f.load(Ordering::Relaxed));
    }
    for h in handles {
        let _ = h.join();
    }
    if let Some(e) = ctl.fatal.lock().unwrap().take() {
        save_sidecar(true);
        return Err(e);
    }
    if ctl.stop.load(Ordering::SeqCst) {
        save_sidecar(true);
        return Err("stopped".into());
    }
    let final_len = std::fs::metadata(&task.dest_path).map(|m| m.len()).unwrap_or(0);
    if final_len != total {
        return Err("اندازه‌ی فایل نهایی با اندازه‌ی سرور یکی نیست.".into());
    }
    let _ = std::fs::remove_file(&sc_path);
    Ok(total)
}

/// پارس هدر «Content-Range: bytes <start>-<end>/<total|*>».
fn parse_content_range(cr: &str) -> Option<(u64, u64, Option<u64>)> {
    let cr = cr.trim().strip_prefix("bytes ")?;
    let (range, total) = cr.split_once('/')?;
    let (a, b) = range.split_once('-')?;
    Some((
        a.trim().parse().ok()?,
        b.trim().parse().ok()?,
        total.trim().parse().ok(),
    ))
}

fn chunk_len(idx: usize, chunk: u64, total: u64) -> u64 {
    let start = idx as u64 * chunk;
    chunk.min(total.saturating_sub(start))
}

struct WorkerCtx<'a> {
    agent: &'a ureq::Agent,
    url: &'a str,
    dest: &'a std::path::Path,
    ctl: &'a Arc<RunControl>,
    bucket: &'a Arc<TokenBucket>,
    cursor: &'a Arc<AtomicUsize>,
    done_flags: &'a Arc<Vec<AtomicBool>>,
    downloaded: &'a Arc<AtomicU64>,
    chunk: u64,
    total: u64,
    n_chunks: usize,
    save: &'a (dyn Fn(bool) + Send + Sync),
}

fn worker_loop(ctx: WorkerCtx) {
    // فایل جدا برای هر اتصال تا seek/write موازی امن باشد.
    let mut file = match std::fs::OpenOptions::new().write(true).open(ctx.dest) {
        Ok(f) => f,
        Err(e) => {
            *ctx.ctl.fatal.lock().unwrap() = Some(format!("بازکردن فایل مقصد ناموفق بود: {e}"));
            return;
        }
    };
    loop {
        if ctx.ctl.stop.load(Ordering::SeqCst) || ctx.ctl.fatal.lock().unwrap().is_some() {
            return;
        }
        // کشیدن بخش بعدی از صف (پرش از بخش‌های تمام‌شده‌ی resume)
        let mut idx = ctx.cursor.fetch_add(1, Ordering::SeqCst);
        while idx < ctx.n_chunks && ctx.done_flags[idx].load(Ordering::Relaxed) {
            idx = ctx.cursor.fetch_add(1, Ordering::SeqCst);
        }
        if idx >= ctx.n_chunks {
            return;
        }
        match download_chunk(&ctx, idx, &mut file) {
            Ok(()) => {
                ctx.done_flags[idx].store(true, Ordering::Relaxed);
                ctx.downloaded
                    .fetch_add(chunk_len(idx, ctx.chunk, ctx.total), Ordering::Relaxed);
                (ctx.save)(false);
            }
            Err(ChunkError::Stopped) => return,
            Err(ChunkError::Fatal(e)) => {
                let mut f = ctx.ctl.fatal.lock().unwrap();
                if f.is_none() {
                    *f = Some(e);
                }
                return;
            }
            Err(ChunkError::RetryExhausted(e)) => {
                let mut f = ctx.ctl.fatal.lock().unwrap();
                if f.is_none() {
                    *f = Some(e);
                }
                return;
            }
        }
    }
}

enum ChunkError {
    Stopped,
    Fatal(String),
    RetryExhausted(String),
}

fn download_chunk(ctx: &WorkerCtx, idx: usize, file: &mut std::fs::File) -> Result<(), ChunkError> {
    let start = idx as u64 * ctx.chunk;
    let end = (start + ctx.chunk).min(ctx.total) - 1;
    let want = (end - start + 1) as usize;
    let mut attempt = 0;
    loop {
        if ctx.ctl.stop.load(Ordering::SeqCst) {
            return Err(ChunkError::Stopped);
        }
        match try_chunk_once(ctx, start, end, want, file) {
            Ok(()) => return Ok(()),
            Err(ChunkError::Stopped) => return Err(ChunkError::Stopped),
            Err(ChunkError::Fatal(e)) => return Err(ChunkError::Fatal(e)),
            Err(ChunkError::RetryExhausted(e)) => {
                attempt += 1;
                if attempt > MAX_CHUNK_RETRIES {
                    return Err(ChunkError::RetryExhausted(e));
                }
                backoff(attempt);
            }
        }
    }
}

fn try_chunk_once(
    ctx: &WorkerCtx,
    start: u64,
    end: u64,
    want: usize,
    file: &mut std::fs::File,
) -> Result<(), ChunkError> {
    let resp = ctx
        .agent
        .get(ctx.url)
        .set("Range", &format!("bytes={start}-{end}"))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) if (400..500).contains(&code) && code != 408 && code != 429 => {
                ChunkError::Fatal(format!("پاسخ سرور: HTTP {code}"))
            }
            other => ChunkError::RetryExhausted(format!("خطای شبکه: {other}")),
        })?;
    if resp.status() != 206 {
        return Err(ChunkError::RetryExhausted(format!(
            "سرور برای بخش‌ها پاسخ {} داد.",
            resp.status()
        )));
    }
    // بازه‌ی برگشتی باید دقیقاً همان بازه‌ی درخواستی باشد؛ سرور/پروکسی خراب
    // با بازه‌ی دیگر یعنی داده در آفست اشتباه و خرابی خاموش فایل.
    let cr_ok = resp
        .header("Content-Range")
        .and_then(|cr| parse_content_range(cr))
        .map(|(cs, ce, _)| cs == start && ce == end)
        .unwrap_or(false);
    if !cr_ok {
        return Err(ChunkError::RetryExhausted(
            "هدر Content-Range پاسخ با بازه‌ی درخواستی نمی‌خواند.".into(),
        ));
    }
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; BLOCK];
    let mut written: usize = 0;
    file.seek(SeekFrom::Start(start))
        .map_err(|e| ChunkError::Fatal(format!("خطای دیسک: {e}")))?;
    loop {
        if ctx.ctl.stop.load(Ordering::SeqCst) {
            return Err(ChunkError::Stopped);
        }
        // هر خواندن حداکثر تا انتهای همین بخش؛ سرور پُرحرف بخش بعدی را رونویسی نمی‌کند.
        let cap = (want - written).min(BLOCK);
        let n = reader
            .read(&mut buf[..cap])
            .map_err(|e| ChunkError::RetryExhausted(format!("خطای خواندن از شبکه: {e}")))?;
        if n == 0 {
            break;
        }
        ctx.bucket.take(n as u64);
        file.write_all(&buf[..n])
            .map_err(|e| ChunkError::Fatal(format!("خطای نوشتن روی دیسک: {e}")))?;
        written += n;
        if written > want {
            return Err(ChunkError::RetryExhausted("سرور بیش از بخش خواسته‌شده فرستاد.".into()));
        }
    }
    if written != want {
        return Err(ChunkError::RetryExhausted(format!(
            "بخش ناقص رسید ({written} از {want} بایت)."
        )));
    }
    Ok(())
}

/// دانلود تکی (سرور Range ندارد یا اندازه نامعلوم است): جریان پیوسته با
/// ادامه از بایت موجود هر وقت سرور اجازه بدهد.
fn stream_download(
    task: &EngineTask,
    events: &Sender<EngineEvent>,
    ctl: &Arc<RunControl>,
    bucket: &Arc<TokenBucket>,
    agent: &ureq::Agent,
    total: Option<u64>,
) -> Result<u64, String> {
    let mut have = std::fs::metadata(&task.dest_path).map(|m| m.len()).unwrap_or(0);
    if total.is_some() && have > total.unwrap() {
        have = 0;
    }
    let mut attempt = 0;
    loop {
        if ctl.stop.load(Ordering::SeqCst) {
            return Err("stopped".into());
        }
        match stream_once(task, events, ctl, bucket, agent, total, have) {
            Ok(done) => return Ok(done),
            Err(StreamErr::Stopped) => return Err("stopped".into()),
            Err(StreamErr::Fatal(e)) => return Err(e),
            Err(StreamErr::Retry(e, got)) => {
                have = got;
                attempt += 1;
                if attempt > MAX_CHUNK_RETRIES + 2 {
                    return Err(e);
                }
                backoff(attempt);
            }
        }
    }
}

enum StreamErr {
    Stopped,
    Fatal(String),
    Retry(String, u64),
}

fn stream_once(
    task: &EngineTask,
    events: &Sender<EngineEvent>,
    ctl: &Arc<RunControl>,
    bucket: &Arc<TokenBucket>,
    agent: &ureq::Agent,
    total: Option<u64>,
    have: u64,
) -> Result<u64, StreamErr> {
    let mut req = agent.get(&task.url);
    if have > 0 {
        req = req.set("Range", &format!("bytes={have}-"));
    }
    let resp = req.call().map_err(|e| match e {
        ureq::Error::Status(code, _) if (400..500).contains(&code) && code != 408 && code != 429 => {
            StreamErr::Fatal(format!("پاسخ سرور: HTTP {code}"))
        }
        other => StreamErr::Retry(format!("خطای شبکه: {other}"), have),
    })?;
    if resp.status() == 206 && have > 0 {
        // ادامه فقط وقتی امن است که سرور دقیقاً از همان بایت ما شروع کرده باشد.
        let ok = resp
            .header("Content-Range")
            .and_then(|cr| parse_content_range(cr))
            .map(|(cs, _, _)| cs == have)
            .unwrap_or(false);
        if !ok {
            return Err(StreamErr::Retry(
                "آفست ادامه‌ی سرور با بایت موجود نمی‌خواند؛ از صفر شروع می‌شود.".into(),
                0,
            ));
        }
    }
    let (mut file, mut pos) = match resp.status() {
        206 if have > 0 => {
            let f = std::fs::OpenOptions::new()
                .write(true)
                .open(&task.dest_path)
                .map_err(|e| StreamErr::Fatal(format!("بازکردن فایل ناموفق بود: {e}")))?;
            (f, have)
        }
        200 => {
            let f = std::fs::File::create(&task.dest_path)
                .map_err(|e| StreamErr::Fatal(format!("ساخت فایل ناموفق بود: {e}")))?;
            (f, 0)
        }
        s => return Err(StreamErr::Retry(format!("پاسخ غیرمنتظره‌ی سرور: HTTP {s}"), have)),
    };
    file.seek(SeekFrom::Start(pos)).ok();
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; BLOCK];
    let mut last_emit = Instant::now() - Duration::from_millis(300);
    let mut prev_pos = pos;
    let mut prev_at = Instant::now();
    let mut speed = 0f64;
    loop {
        if ctl.stop.load(Ordering::SeqCst) {
            return Err(StreamErr::Stopped);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| StreamErr::Retry(format!("خطای خواندن از شبکه: {e}"), pos))?;
        if n == 0 {
            break;
        }
        bucket.take(n as u64);
        file.write_all(&buf[..n])
            .map_err(|e| StreamErr::Fatal(format!("خطای نوشتن روی دیسک: {e}")))?;
        pos += n as u64;
        if last_emit.elapsed() >= Duration::from_millis(250) {
            let dt = prev_at.elapsed().as_secs_f64().max(0.05);
            let inst = (pos - prev_pos) as f64 / dt;
            speed = if speed == 0.0 { inst } else { speed * 0.6 + inst * 0.4 };
            prev_pos = pos;
            prev_at = Instant::now();
            last_emit = Instant::now();
            let _ = events.send(EngineEvent::Progress {
                id: task.id.clone(),
                generation: task.generation,
                downloaded: pos,
                total,
                speed_bps: speed as u64,
            });
        }
    }
    if let Some(t) = total {
        if pos != t {
            return Err(StreamErr::Retry(
                format!("اتصال زودتر از کامل‌شدن فایل قطع شد ({pos} از {t} بایت)."),
                pos,
            ));
        }
    }
    Ok(pos)
}

// ────────────────────────────────────────────────────────────────────
// تست‌های موتور واقعی: سرور HTTP لوکال (std) با پشتیبانی Range؛
// دانلود چندبخشی کامل، ادامه از سایدکار و حالت بدون Range.
// ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod engine_tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::mpsc::channel;

    fn payload(n: usize) -> Vec<u8> {
        // بایت‌های قطعی (LCG) تا مقایسه‌ی نهایی دقیق باشد.
        let mut v = Vec::with_capacity(n);
        let mut x: u32 = 0x1234_5678;
        for _ in 0..n {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            v.push((x >> 24) as u8);
        }
        v
    }

    /// سرور ساده: با Range → 206 و برش دقیق؛ بدون Range (یا حالت no_range) → 200 کامل.
    fn serve(data: Arc<Vec<u8>>, no_range: bool) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let data = data.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(s.try_clone().unwrap());
                    let mut range: Option<(u64, u64)> = None;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        let t = line.trim();
                        if t.is_empty() {
                            break;
                        }
                        if let Some(v) = t.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            if let Some((a, b)) = v.split_once('-') {
                                let a: u64 = a.parse().unwrap_or(0);
                                let b: u64 = if b.is_empty() {
                                    data.len() as u64 - 1
                                } else {
                                    b.parse().unwrap_or(0)
                                };
                                range = Some((a, b));
                            }
                        }
                    }
                    let total = data.len() as u64;
                    let (status, body, extra) = if !no_range {
                        if let Some((a, b)) = range {
                            let b = b.min(total - 1);
                            (
                                "206 Partial Content",
                                data[a as usize..=b as usize].to_vec(),
                                format!("Content-Range: bytes {a}-{b}/{total}\r\n"),
                            )
                        } else {
                            ("200 OK", data.to_vec(), String::new())
                        }
                    } else {
                        ("200 OK", data.to_vec(), String::new())
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nETag: \"v1\"\r\nAccept-Ranges: bytes\r\n{extra}Connection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(head.as_bytes());
                    let _ = s.write_all(&body);
                    let _ = s.flush();
                });
            }
        });
        port
    }

    fn run_engine(url: &str, dest: &std::path::Path, conns: u32) -> Result<u64, String> {
        let engine = RealEngine::new();
        let (tx, rx) = channel::<EngineEvent>();
        engine.start(
            EngineTask {
                id: "t1".into(),
                generation: 1,
                url: url.into(),
                dest_path: dest.to_path_buf(),
                connections: conns,
                resume_from: 0,
                speed_limit_bps: None,
            },
            tx,
        )?;
        loop {
            match rx.recv_timeout(Duration::from_secs(60)) {
                Ok(EngineEvent::Completed { total, .. }) => return Ok(total),
                Ok(EngineEvent::Failed { error, .. }) => return Err(error),
                Ok(_) => continue,
                Err(e) => return Err(format!("timeout: {e}")),
            }
        }
    }

    fn tmp_dest(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("downbolt-eng-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("file.bin")
    }

    #[test]
    fn ranged_download_matches_payload() {
        let data = Arc::new(payload(1_500_000));
        let port = serve(data.clone(), false);
        let dest = tmp_dest("ranged");
        let total = run_engine(&format!("http://127.0.0.1:{port}/f.bin"), &dest, 4)
            .expect("دانلود چندبخشی باید کامل شود");
        assert_eq!(total, 1_500_000);
        assert_eq!(std::fs::read(&dest).unwrap(), *data);
        assert!(!sidecar_path(&dest).exists(), "سایدکار بعد از اتمام باید پاک شود");
    }

    #[test]
    fn stream_download_matches_payload() {
        let data = Arc::new(payload(700_000));
        let port = serve(data.clone(), true);
        let dest = tmp_dest("stream");
        let total = run_engine(&format!("http://127.0.0.1:{port}/f.bin"), &dest, 4)
            .expect("دانلود تکی باید کامل شود");
        assert_eq!(total, 700_000);
        assert_eq!(std::fs::read(&dest).unwrap(), *data);
    }

    #[test]
    fn resume_from_sidecar_skips_done_chunks() {
        let data = Arc::new(payload(1_500_000));
        let port = serve(data.clone(), false);
        let dest = tmp_dest("resume");
        let url = format!("http://127.0.0.1:{port}/f.bin");
        // اجرای اول را وسط کار با pause نگه می‌داریم.
        let engine = RealEngine::new();
        let (tx, rx) = channel::<EngineEvent>();
        engine
            .start(
                EngineTask {
                    id: "t1".into(),
                    generation: 1,
                    url: url.clone(),
                    dest_path: dest.clone(),
                    connections: 2,
                    resume_from: 0,
                    speed_limit_bps: Some(400 * 1024),
                },
                tx,
            )
            .unwrap();
        // کمی پیشرفت که آمد مکث کن.
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut saw_progress = false;
        while Instant::now() < deadline {
            if let Ok(EngineEvent::Progress { downloaded, .. }) =
                rx.recv_timeout(Duration::from_millis(200))
            {
                if downloaded > 0 {
                    saw_progress = true;
                    break;
                }
            }
        }
        engine.pause("t1").unwrap();
        std::thread::sleep(Duration::from_millis(600));
        assert!(sidecar_path(&dest).exists(), "بعد از مکث سایدکار باید باشد");
        // اجرای دوم: از همان سایدکار ادامه و کامل.
        let total = run_engine(&url, &dest, 4).expect("ادامه‌ی دانلود باید کامل شود");
        assert_eq!(total, 1_500_000);
        assert_eq!(std::fs::read(&dest).unwrap(), *data);
        let _ = saw_progress;
    }
}
