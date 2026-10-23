// app.ts — رابط اصلی دانلودمنیجر: هدر، نوار افزودن، آمار، نمودار، لیست دانلودها، تنظیمات
// نام و آیکن محصول placeholder است و با branding/ ایجنت۲ جایگزین می‌شود.

import {
  api,
  etaSeconds,
  onProgressSnapshot,
  onUpdateProgress,
  progressPct,
  type DmSettings,
  type DownloadItem,
} from "../lib/api";
import { faInt, faNum, fmtBytes, fmtEta, fmtPct, fmtSpeed } from "../lib/format";
import { SpeedChart } from "./chart";

const STATUS_LABEL: Record<DownloadItem["status"], string> = {
  queued: "در صف",
  downloading: "در حال دانلود",
  paused: "مکث",
  completed: "کامل شد",
  error: "خطا",
};

const escapeHtml = (s: string): string =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function looksLikeUrl(s: string): boolean {
  return /^(https?|ftp):\/\/\S+$/i.test(s.trim()) || /^magnet:\?/i.test(s.trim());
}

/** آیکن فایل بر اساس پسوند (SVG ساده و سبک) */
function fileIcon(name: string): string {
  const ext = (name.split(".").pop() || "").toLowerCase();
  const kind = ["zip", "rar", "7z", "tar", "gz"].includes(ext)
    ? "archive"
    : ["mp4", "mkv", "avi", "mov", "webm"].includes(ext)
      ? "video"
      : ["mp3", "wav", "flac", "ogg"].includes(ext)
        ? "audio"
        : ["jpg", "jpeg", "png", "gif", "webp", "svg"].includes(ext)
          ? "image"
          : ["exe", "msi", "apk", "dmg", "iso"].includes(ext)
            ? "app"
            : "file";
  const paths: Record<string, string> = {
    file: "M6 2h8l4 4v14H6z M14 2v4h4",
    archive: "M4 4h16v4H4z M5 8h14v12H5z M10 12h4",
    video: "M4 5h16v14H4z M10 9l5 3-5 3z",
    audio: "M9 18V6l10-2v11 M9 18a3 3 0 1 1-3-3 3 3 0 0 1 3 3z M19 15a3 3 0 1 1-3-3 3 3 0 0 1 3 3z",
    image: "M4 4h16v16H4z M8 14l3-3 2 2 3-4 4 5",
    app: "M5 3h14v18H5z M9 7h6 M12 17h0",
  };
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"><path d="${paths[kind]}"/></svg>`;
}

interface RowRefs {
  el: HTMLElement;
  name: HTMLElement;
  meta: HTMLElement;
  barFill: HTMLElement;
  pct: HTMLElement;
  speed: HTMLElement;
  eta: HTMLElement;
  status: HTMLElement;
  toggleBtn: HTMLButtonElement;
}

export function mountApp(root: HTMLElement): void {
  root.innerHTML = `
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <!-- لوگوی ایجنت۲: فایل branding/logo.png باید با نام logo.png در روت وب‌اپ سرو شود؛ اگر نبود، آیکن SVG جایگزین می‌ماند -->
        <div class="brand-logo" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v11"/><path d="m7 10 5 5 5-5"/><path d="M4 20h16"/></svg>
          <img src="logo.png" alt="" onerror="this.remove()" />
        </div>
        <div class="brand-text">
          <div class="brand-name">داون‌بولت</div>
          <div class="brand-sub">DownBolt — دانلود سریع و پایدار</div>
        </div>
      </div>
      <div class="top-actions">
        <div class="total-speed" title="مجموع سرعت دانلود">
          <span class="dot-live"></span><span id="totalSpeed">۰ بایت/ثانیه</span>
        </div>
        <button id="settingsBtn" class="btn ghost" type="button">⚙ تنظیمات</button>
      </div>
    </header>

    <section class="addbar card">
      <input id="urlInput" class="url-input" dir="ltr" type="text" placeholder="https://example.com/file.zip" />
      <button id="addBtn" class="btn primary" type="button">＋ افزودن دانلود</button>
      <button id="clipBtn" class="btn" type="button">📋 از کلیپ‌بورد</button>
    </section>

    <section class="stats">
      <div class="stat card"><span class="stat-label">در حال دانلود</span><b id="statActive">۰</b></div>
      <div class="stat card"><span class="stat-label">مجموع دانلودشده</span><b id="statDone">۰ بایت</b></div>
      <div class="stat card"><span class="stat-label">کل فایل‌ها</span><b id="statCount">۰</b></div>
    </section>

    <section class="card chart-card">
      <div class="chart-head"><span>نمودار سرعت</span><span id="chartNow" class="chart-now">۰ بایت/ثانیه</span></div>
      <canvas id="speedChart" class="speed-chart"></canvas>
    </section>

    <main id="listWrap" class="list"></main>

    <section id="settingsPanel" class="card settings" hidden>
      <h2>تنظیمات</h2>
      <label class="field">
        <span>پوشه‌ی دانلود</span>
        <span class="dir-row">
          <input id="setDir" dir="ltr" type="text" />
          <button id="pickDirBtn" class="btn" type="button">انتخاب پوشه…</button>
        </span>
      </label>
      <div class="field-grid">
        <label class="field">
          <span>سقف اتصال موازی هر دانلود (۱ تا ۱۶)</span>
          <input id="setParallel" type="number" min="1" max="16" step="1" />
        </label>
        <label class="field">
          <span>سقف سرعت کلی (کیلوبایت/ثانیه — ۰ = نامحدود)</span>
          <input id="setSpeed" type="number" min="0" step="64" />
        </label>
      </div>
      <label class="field tray-field">
        <span>بستن به سینی سیستم (با بستن پنجره، برنامه در پس‌زمینه می‌ماند و دانلودها ادامه پیدا می‌کنند)</span>
        <input id="setTrayDm" type="checkbox" checked />
      </label>
      <div class="settings-actions">
        <button id="saveSettingsBtn" class="btn primary" type="button">ذخیره تنظیمات</button>
        <span id="settingsMsg" class="settings-msg"></span>
      </div>

      <div class="update-box">
        <h3>به‌روزرسانی برنامه</h3>
        <div class="field"><span>نسخه‌ی فعلی: <b id="updCurrent">…</b></span></div>
        <label class="field">
          <span>حالت به‌روزرسانی خودکار</span>
          <select id="updMode">
            <option value="notify">اطلاع‌رسانی (پیش‌فرض)</option>
            <option value="auto">دانلود خودکار و اطلاع برای نصب</option>
            <option value="off">خاموش</option>
          </select>
        </label>
        <div class="settings-actions">
          <button id="updCheckBtn" class="btn" type="button">بررسی به‌روزرسانی</button>
          <button id="updFileBtn" class="btn" type="button">بارگذاری فایل به‌روزرسانی…</button>
        </div>
        <p id="updStatus" class="settings-msg"></p>
        <div id="updProgress" class="upd-progress" hidden><div id="updBar" class="upd-bar"></div></div>
        <p id="updNotes" class="upd-notes"></p>
        <div class="settings-actions">
          <button id="updDownloadBtn" class="btn primary" type="button" hidden>دانلود</button>
          <button id="updCancelBtn" class="btn" type="button" hidden>لغو دانلود</button>
          <button id="updApplyBtn" class="btn primary" type="button" hidden>نصب به‌روزرسانی</button>
        </div>
      </div>
    </section>

    <div id="toast" class="toast" hidden></div>
  </div>`;

  const $ = <T extends HTMLElement>(sel: string) => root.querySelector<T>(sel)!;
  const urlInput = $<HTMLInputElement>("#urlInput");
  const listWrap = $<HTMLElement>("#listWrap");
  const totalSpeedEl = $<HTMLElement>("#totalSpeed");
  const chartNowEl = $<HTMLElement>("#chartNow");
  const toastEl = $<HTMLElement>("#toast");
  const settingsPanel = $<HTMLElement>("#settingsPanel");

  const chart = new SpeedChart($<HTMLCanvasElement>("#speedChart"));
  window.addEventListener("resize", () => chart.resize());

  let toastTimer: number | undefined;
  function toast(msg: string, isError = false): void {
    toastEl.textContent = msg;
    toastEl.classList.toggle("error", isError);
    toastEl.hidden = false;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toastEl.hidden = true), 3400);
  }

  /* ── رندر کلیددار لیست: هر ردیف یک بار ساخته می‌شود و بعد فقط فیلدهایش به‌روز می‌شود ── */
  const rows = new Map<string, RowRefs>();
  let items: DownloadItem[] = [];

  function buildRow(it: DownloadItem): RowRefs {
    const el = document.createElement("article");
    el.className = "dl card";
    el.innerHTML = `
      <div class="dl-ic">${fileIcon(it.file_name)}</div>
      <div class="dl-main">
        <div class="dl-top">
          <span class="dl-name"></span>
          <span class="dl-status"></span>
        </div>
        <div class="dl-meta"></div>
        <div class="dl-bar"><div class="dl-fill"></div></div>
        <div class="dl-nums">
          <span class="dl-pct"></span>
          <span class="dl-speed"></span>
          <span class="dl-eta"></span>
        </div>
      </div>
      <div class="dl-actions">
        <button class="btn small toggle" type="button"></button>
        <button class="btn small danger remove" type="button">حذف</button>
      </div>`;
    const refs: RowRefs = {
      el,
      name: el.querySelector(".dl-name")!,
      meta: el.querySelector(".dl-meta")!,
      barFill: el.querySelector(".dl-fill")!,
      pct: el.querySelector(".dl-pct")!,
      speed: el.querySelector(".dl-speed")!,
      eta: el.querySelector(".dl-eta")!,
      status: el.querySelector(".dl-status")!,
      toggleBtn: el.querySelector(".toggle")!,
    };
    refs.toggleBtn.addEventListener("click", () => void onToggle(it.id));
    el.querySelector(".remove")!.addEventListener("click", () => void onRemove(it.id));
    return refs;
  }

  function updateRow(refs: RowRefs, it: DownloadItem): void {
    const pct = progressPct(it);
    refs.name.textContent = it.file_name || "فایل بدون نام";
    refs.name.title = it.url;
    refs.meta.textContent =
      it.size !== null
        ? `${fmtBytes(it.downloaded)} از ${fmtBytes(it.size)}`
        : `${fmtBytes(it.downloaded)} دانلود شده — حجم کل نامشخص`;
    refs.barFill.style.width = pct !== null ? `${pct}%` : it.status === "completed" ? "100%" : "0%";
    refs.el.dataset.status = it.status;
    refs.pct.textContent = pct !== null ? fmtPct(pct) : it.status === "completed" ? "۱۰۰٪" : "—";
    refs.speed.textContent = it.status === "downloading" ? fmtSpeed(it.speed) : "";
    const eta = etaSeconds(it);
    refs.eta.textContent = eta !== null ? `حدود ${fmtEta(eta)} مانده` : "";
    refs.status.textContent =
      it.status === "error" && it.error ? `خطا: ${it.error}` : STATUS_LABEL[it.status];
    refs.toggleBtn.textContent =
      it.status === "downloading" || it.status === "queued" ? "⏸ مکث" : "▶ ادامه";
    refs.toggleBtn.disabled = it.status === "completed" || it.status === "error";
  }

  function render(): void {
    if (items.length > 0) listWrap.querySelector(".empty")?.remove();
    const seen = new Set<string>();
    for (const it of items) {
      seen.add(it.id);
      let refs = rows.get(it.id);
      if (!refs) {
        refs = buildRow(it);
        rows.set(it.id, refs);
        listWrap.appendChild(refs.el);
      }
      updateRow(refs, it);
    }
    for (const [id, refs] of rows) {
      if (!seen.has(id)) {
        refs.el.remove();
        rows.delete(id);
      }
    }
    if (items.length === 0) {
      listWrap.innerHTML = `<div class="empty card">
        <div class="empty-ic">⬇</div>
        <p>هنوز دانلودی ندارید.</p>
        <p class="muted">لینک فایل را در کادر بالا بچسبانید یا از دکمه‌ی «از کلیپ‌بورد» استفاده کنید.</p>
      </div>`;
      rows.clear();
    }

    const totalSpeed = items.reduce((s, i) => s + (i.status === "downloading" ? i.speed : 0), 0);
    totalSpeedEl.textContent = fmtSpeed(totalSpeed);
    chartNowEl.textContent = fmtSpeed(totalSpeed);
    chart.push(totalSpeed);
    $("#statActive").textContent = faInt(items.filter((i) => i.status === "downloading").length);
    $("#statDone").textContent = fmtBytes(items.reduce((s, i) => s + i.downloaded, 0));
    $("#statCount").textContent = faInt(items.length);
  }

  function applySnapshot(next: DownloadItem[]): void {
    items = next;
    render();
  }

  async function refresh(): Promise<void> {
    try {
      applySnapshot(await api.listDownloads());
    } catch {
      /* بک‌اند هنوز آماده نیست یا خطای گذرا — اسنپ‌شات قبلی می‌ماند */
    }
  }

  /* ── اکشن‌ها ── */
  async function onToggle(id: string): Promise<void> {
    const it = items.find((x) => x.id === id);
    if (!it) return;
    try {
      if (it.status === "downloading" || it.status === "queued") await api.pauseDownload(id);
      else await api.resumeDownload(id);
      void refresh();
    } catch (e) {
      toast("خطا: " + String((e as Error)?.message || e), true);
    }
  }

  async function onRemove(id: string): Promise<void> {
    const it = items.find((x) => x.id === id);
    if (!it) return;
    if (!window.confirm(`«${it.file_name}» از لیست حذف شود؟ (فایل دانلودشده روی دیسک می‌ماند)`)) return;
    try {
      await api.removeDownload(id, false);
      void refresh();
    } catch (e) {
      toast("خطا در حذف: " + String((e as Error)?.message || e), true);
    }
  }

  async function addFromInput(raw: string): Promise<void> {
    const url = raw.trim();
    if (!url) {
      toast("اول لینک دانلود را وارد کنید.");
      return;
    }
    if (!looksLikeUrl(url)) {
      toast("این لینک معتبر به نظر نمی‌رسد.", true);
      return;
    }
    try {
      await api.addDownload(url);
      urlInput.value = "";
      toast("دانلود اضافه شد.");
      void refresh();
    } catch (e) {
      toast("افزودن دانلود ناموفق بود: " + String((e as Error)?.message || e), true);
    }
  }

  $("#addBtn").addEventListener("click", () => void addFromInput(urlInput.value));
  urlInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void addFromInput(urlInput.value);
  });

  $("#clipBtn").addEventListener("click", async () => {
    let text = "";
    try {
      text = await api.readClipboard();
    } catch {
      try {
        text = await navigator.clipboard.readText();
      } catch {
        text = "";
      }
    }
    if (!text) {
      urlInput.focus();
      toast("کلیپ‌بورد خوانده نشد؛ لینک را با Ctrl+V در کادر بچسبانید.");
      return;
    }
    urlInput.value = text.trim();
    if (looksLikeUrl(text)) void addFromInput(text);
    else toast("متن کلیپ‌بورد لینک دانلود نیست؛ در کادر قرار گرفت.");
  });

  /* ── تنظیمات ── */
  const settingsMsg = $<HTMLElement>("#settingsMsg");
  // آخرین تنظیمات خوانده‌شده از بک‌اند؛ فیلدهایی که فرم ندارد (مثل max_parallel)
  // موقع ذخیره از همین‌جا حفظ می‌شوند تا قرارداد DmSettings نشکند.
  let lastSettings: DmSettings | null = null;
  $("#settingsBtn").addEventListener("click", async () => {
    const opening = settingsPanel.hidden;
    settingsPanel.hidden = !opening;
    if (opening) {
      try {
        const s = await api.getSettings();
        lastSettings = s;
        $<HTMLInputElement>("#setDir").value = s.download_dir;
        $<HTMLInputElement>("#setParallel").value = String(s.max_connections);
        $<HTMLInputElement>("#setSpeed").value = String(s.speed_limit_kbps);
        settingsMsg.textContent = "";
      } catch {
        settingsMsg.textContent = "خواندن تنظیمات از بک‌اند ممکن نشد.";
      }
    }
  });

  $("#pickDirBtn").addEventListener("click", async () => {
    try {
      const dir = await api.pickDownloadDir();
      if (dir) $<HTMLInputElement>("#setDir").value = dir;
    } catch {
      toast("دیالوگ انتخاب پوشه در دسترس نیست؛ مسیر را دستی وارد کنید.");
    }
  });

  $("#saveSettingsBtn").addEventListener("click", async () => {
    const settings: DmSettings = {
      download_dir: $<HTMLInputElement>("#setDir").value.trim(),
      max_parallel: lastSettings?.max_parallel ?? 3,
      max_connections: Math.min(16, Math.max(1, Number($<HTMLInputElement>("#setParallel").value) || 8)),
      speed_limit_kbps: Math.max(0, Number($<HTMLInputElement>("#setSpeed").value) || 0),
    };
    try {
      await api.setSettings(settings);
      settingsMsg.textContent = "تنظیمات ذخیره شد ✓";
    } catch (e) {
      settingsMsg.textContent = "خطا در ذخیره: " + String((e as Error)?.message || e);
    }
  });

  /* ── چرخه‌ی به‌روزرسانی: رویداد بک‌اند + پول ۱ ثانیه‌ای ── */
  void onProgressSnapshot(applySnapshot).catch(() => undefined);
  void refresh();
  window.setInterval(() => void refresh(), 1000);
  /* ── بستن به سینی سیستم ── */
  const trayBox = $<HTMLInputElement>("#setTrayDm");
  const TRAY_KEY = "dm_close_to_tray";
  trayBox.checked = localStorage.getItem(TRAY_KEY) !== "0";
  void api.setCloseToTray(trayBox.checked).catch(() => {});
  trayBox.addEventListener("change", () => {
    localStorage.setItem(TRAY_KEY, trayBox.checked ? "1" : "0");
    void api.setCloseToTray(trayBox.checked).catch(() => {});
  });

  /* ── به‌روزرسانی (موتور v2 — مثل شیرین‌حساب) ── */
  // نصب آپدیت دو مسیر جدا دارد: ویندوز با هلپر (apply_update2) و اندروید با
  // نصب‌کننده‌ی سیستم (پلاگین install_apk). مسیر را پلتفرم تعیین می‌کند، نه
  // بودن یا نبودن مسیر فایل آماده — روی ویندوز ready_update_path نباید
  // مسیر APK تلقی شود (باگ 0.2.0: خطای ACL پلاگین روی ویندوز).
  const IS_ANDROID = /Android/i.test(navigator.userAgent);
  const updStatus = $<HTMLElement>("#updStatus");
  const updNotes = $<HTMLElement>("#updNotes");
  const updProgress = $<HTMLElement>("#updProgress");
  const updBar = $<HTMLElement>("#updBar");
  const updDownloadBtn = $<HTMLButtonElement>("#updDownloadBtn");
  const updCancelBtn = $<HTMLButtonElement>("#updCancelBtn");
  const updApplyBtn = $<HTMLButtonElement>("#updApplyBtn");
  const updModeSel = $<HTMLSelectElement>("#updMode");
  const MODE_KEY = "dm_update_mode";
  updModeSel.value = localStorage.getItem(MODE_KEY) || "notify";
  updModeSel.addEventListener("change", () =>
    localStorage.setItem(MODE_KEY, updModeSel.value),
  );

  let updDownloading = false;
  function updResetButtons(): void {
    updDownloadBtn.hidden = true;
    updCancelBtn.hidden = true;
    updApplyBtn.hidden = true;
    updProgress.hidden = true;
  }

  async function doUpdateCheck(auto = false): Promise<void> {
    if (!auto) updStatus.textContent = "در حال بررسی…";
    let info;
    try {
      info = await api.checkUpdate();
    } catch (e) {
      if (!auto)
        updStatus.textContent =
          "بررسی ناموفق بود: " + String((e as Error)?.message || e);
      return;
    }
    if (!info) {
      if (!auto) updStatus.textContent = "برنامه به‌روز است.";
      updNotes.textContent = "";
      updResetButtons();
      return;
    }
    updStatus.textContent =
      "نسخه‌ی تازه‌ی " + info.version + " آماده است (" + fmtBytes(info.size) + ").";
    updNotes.textContent = info.notes_fa || "";
    updDownloadBtn.hidden = false;
    if (auto && updModeSel.value === "auto") void startUpdateDownload();
  }

  async function startUpdateDownload(): Promise<void> {
    if (updDownloading) return;
    updDownloading = true;
    updDownloadBtn.hidden = true;
    updApplyBtn.hidden = true;
    updCancelBtn.hidden = false;
    updProgress.hidden = false;
    updBar.style.width = "0%";
    updStatus.textContent = "در حال دانلود به‌روزرسانی…";
    try {
      await api.downloadUpdate();
      const readyPath = IS_ANDROID
        ? await api.readyUpdatePath().catch(() => null)
        : null;
      updCancelBtn.hidden = true;
      if (readyPath) {
        updProgress.hidden = true;
        updApplyBtn.hidden = false;
        updApplyBtn.dataset.apkPath = readyPath;
        updStatus.textContent = "دانلود کامل شد؛ برای نصب دکمه‌ی «نصب به‌روزرسانی» را بزنید.";
      } else if (updDownloading) {
        updStatus.textContent =
          "دانلود کامل شد؛ دکمه‌ی «نصب به‌روزرسانی» را بزنید تا برنامه بسته و نسخه‌ی تازه نصب شود.";
        updApplyBtn.hidden = false;
      }
    } catch (e) {
      updStatus.textContent = "دانلود ناموفق بود: " + String((e as Error)?.message || e);
      updCancelBtn.hidden = true;
      updProgress.hidden = true;
      updDownloadBtn.hidden = false;
    } finally {
      updDownloading = false;
    }
  }

  updDownloadBtn.addEventListener("click", () => void startUpdateDownload());
  updCancelBtn.addEventListener("click", () => {
    updDownloading = false;
    void api.cancelUpdateDownload();
    updCancelBtn.hidden = true;
    updProgress.hidden = true;
    updDownloadBtn.hidden = false;
    updStatus.textContent = "دانلود لغو شد (بخش دانلودشده می‌ماند تا دفعه‌ی بعد ادامه پیدا کند).";
  });
  updApplyBtn.addEventListener("click", async () => {
    const apkPath = updApplyBtn.dataset.apkPath;
    try {
      if (IS_ANDROID && apkPath) {
        await api.installApk(apkPath);
        updStatus.textContent = "نصب‌کننده‌ی سیستم باز شد؛ نصب را آنجا کامل کنید.";
      } else {
        updStatus.textContent = "در حال نصب… برنامه بسته و نسخه‌ی تازه اجرا می‌شود.";
        await api.applyUpdate();
      }
    } catch (e) {
      updStatus.textContent = "نصب ناموفق بود: " + String((e as Error)?.message || e);
    }
  });
  $<HTMLButtonElement>("#updCheckBtn").addEventListener("click", () => void doUpdateCheck());
  $<HTMLButtonElement>("#updFileBtn").addEventListener("click", async () => {
    try {
      const path = await api.openUpdateFile();
      if (!path) return;
      if (path.toLowerCase().endsWith(".apk") && IS_ANDROID) {
        await api.installApk(path);
        updStatus.textContent = "نصب‌کننده‌ی سیستم باز شد؛ نصب را آنجا کامل کنید.";
      } else if (path.toLowerCase().endsWith(".apk")) {
        updStatus.textContent = "این فایل APK مخصوص اندروید است و روی این دستگاه نصب نمی‌شود.";
      } else if (
        confirm("بسته‌ی به‌روزرسانی نصب شود؟ برنامه بسته و نسخه‌ی تازه اجرا می‌شود.")
      ) {
        updStatus.textContent = "در حال نصب بسته‌ی دستی…";
        await api.applyLocalUpdate(path);
      }
    } catch (e) {
      updStatus.textContent = "نصب بسته ناموفق بود: " + String((e as Error)?.message || e);
    }
  });

  void onUpdateProgress((p) => {
    if (!updDownloading) return;
    updBar.style.width = Math.min(100, Math.max(0, p.percent)) + "%";
    updStatus.textContent =
      "در حال دانلود… " + faNum(p.percent) + "٪ (" + fmtBytes(p.downloaded) + ")";
  });

  void (async () => {
    try {
      const { getVersion } = await import("@tauri-apps/api/app");
      $<HTMLElement>("#updCurrent").textContent = await getVersion();
    } catch {
      $<HTMLElement>("#updCurrent").textContent = "—";
    }
    try {
      const rep = await api.getUpdateReport();
      if (rep.just_updated_from)
        toast("به‌روزرسانی انجام شد — از نسخه‌ی " + rep.just_updated_from + " به نسخه‌ی تازه رسیدید.");
      else if (rep.update_failed_from)
        toast("به‌روزرسانی ناموفق بود و به نسخه‌ی قبلی برگشتیم.", true);
    } catch {
      /* گزارش آپدیت اختیاری است */
    }
    if ((localStorage.getItem(MODE_KEY) || "notify") !== "off") {
      setTimeout(() => void doUpdateCheck(true), 1500);
    }
  })();

}

export { escapeHtml };
