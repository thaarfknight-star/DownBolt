// app.ts — رابط اصلی دانلودمنیجر: هدر، نوار افزودن، آمار، نمودار، لیست دانلودها، تنظیمات
// نام و آیکن محصول placeholder است و با branding/ ایجنت۲ جایگزین می‌شود.

import {
  api,
  etaSeconds,
  onPostDownloadCancelled,
  onPostDownloadCountdown,
  onPostDownloadExecuted,
  onPostDownloadFailed,
  onProgressSnapshot,
  onUpdateProgress,
  progressPct,
  type DmSettings,
  type DownloadItem,
  type PostDownloadAction,
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

/** لینکِ داخلِ رشته‌ی خطا برای نمایش بریده می‌شود؛ متنِ کامل فقط در title می‌ماند. */
function stripUrlFromError(err: string): string {
  const cut = err.search(/https?:\/\//);
  const head = (cut === -1 ? err : err.slice(0, cut)).replace(/[\s(:،,;\-]+$/, "").trim();
  return head === "" ? "خطای شبکه" : head;
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
  size: HTMLElement;
  barFill: HTMLElement;
  pct: HTMLElement;
  speed: HTMLElement;
  eta: HTMLElement;
  status: HTMLElement;
  toggleBtn: HTMLButtonElement;
  openBtn: HTMLButtonElement;
  shareBtn: HTMLButtonElement;
  selectBox: HTMLInputElement;
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

    <!-- ناوبری دو صفحه‌ی دانلود (۰٫۶٫۴): در حال دانلود / دانلود شده‌ها -->
    <nav class="dl-nav">
      <button id="tabActive" class="btn dl-tab active" type="button">در حال دانلود <span id="countActive" class="dl-count">۰</span></button>
      <button id="tabDone" class="btn dl-tab" type="button">دانلود شده‌ها <span id="countDone" class="dl-count">۰</span></button>
      <span class="dl-nav-spacer"></span>
      <button id="pauseAllBtn" class="btn" type="button">⏸ توقف همه</button>
      <button id="resumeAllBtn" class="btn" type="button">▶ شروع همه</button>
    </nav>

    <!-- نوار اقدام گروهی: فقط وقتی چیزی انتخاب شده دیده می‌شود -->
    <div id="bulkBar" class="bulk-bar card" hidden>
      <span id="bulkCount" class="bulk-count"></span>
      <div class="bulk-actions">
        <button id="bulkPauseBtn" class="btn small" type="button">⏸ توقف انتخاب‌شده‌ها</button>
        <button id="bulkResumeBtn" class="btn small" type="button">▶ شروع انتخاب‌شده‌ها</button>
        <button id="bulkRemoveBtn" class="btn small danger" type="button">حذف انتخاب‌شده‌ها</button>
      </div>
    </div>

    <main id="listActive" class="list"></main>
    <main id="listDone" class="list" hidden></main>

    <!-- صفحه‌ی تنظیمات (۰٫۶٫۳): نمای جدا — با باز شدنش، بخش‌های دانلود
         پنهان می‌شوند (کلاس settings-open روی .app) و تنظیمات به‌صورت
         صفحه‌ی کامل با بخش‌های جدا نشان داده می‌شود. -->
    <section id="settingsPage" class="settings-page" hidden>
      <div class="settings-head">
        <h2>تنظیمات</h2>
        <button id="backFromSettingsBtn" class="btn" type="button">→ بازگشت به دانلودها</button>
      </div>

      <section class="card settings-section">
        <h3>دانلودها</h3>
        <label class="field">
          <span>پوشه‌ی دانلود</span>
          <span class="dir-row">
            <input id="setDir" dir="ltr" type="text" />
            <button id="pickDirBtn" class="btn" type="button">انتخاب پوشه…</button>
          </span>
        </label>
        <p id="androidDirNote" class="muted" hidden>روی اندروید، فایل‌های کامل‌شده در پوشه‌ی Downloads عمومی ذخیره می‌شوند و «حذف فایل» نسخه‌ی عمومی را هم پاک می‌کند؛ انتخاب پوشه روی موبایل ممکن نیست.</p>
        <div class="field-grid">
          <label class="field">
            <span>تعداد دانلود همزمان (۱ تا ۱۶)</span>
            <input id="setMaxParallel" type="number" min="1" max="16" step="1" />
          </label>
          <label class="field">
            <span>سقف اتصال موازی هر دانلود (۱ تا ۱۶)</span>
            <input id="setParallel" type="number" min="1" max="16" step="1" />
          </label>
          <label class="field">
            <span>سقف سرعت کلی (کیلوبایت/ثانیه — ۰ = نامحدود)</span>
            <input id="setSpeed" type="number" min="0" step="64" />
          </label>
        </div>
        <p class="muted">بالا بردنِ تعداد دانلود همزمان، دانلودهای در صف را فوراً شروع می‌کند؛ پایین آوردنش روی دانلودهای در حال اجرا اثر نمی‌گذارد.</p>
      </section>

      <section id="postDownloadSection" class="card settings-section">
        <h3>پس از پایان دانلودها</h3>
        <label class="field">
          <span>وقتی همه‌ی دانلودها تمام شدند</span>
          <select id="setPostAction">
            <option value="none">هیچ‌کاری نکن</option>
            <option value="shutdown">خاموش کردن سیستم (Shutdown)</option>
            <option value="sleep">خواب سیستم (Sleep)</option>
            <option value="restart">ری‌استارت سیستم (Restart)</option>
          </select>
        </label>
        <p class="muted">«تمام‌شدن» یعنی هیچ دانلودی در حال دانلود، در صف، مکث‌شده یا خطادار نباشد و دست‌کم یک دانلود در فهرست باشد. پیش از اجرا ۶۰ ثانیه فرصت لغو داری؛ اگر دانلود تازه‌ای اضافه شود، شمارش خودش لغو می‌شود. فقط روی دسکتاپ (ویندوز).</p>
      </section>

      <section class="card settings-section">
        <h3>برنامه</h3>
        <label class="field tray-field">
          <span>بستن به سینی سیستم (با بستن پنجره، برنامه در پس‌زمینه می‌ماند و دانلودها ادامه پیدا می‌کنند)</span>
          <input id="setTrayDm" type="checkbox" checked />
        </label>
      </section>

      <section class="card settings-section">
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
      </section>

      <div class="settings-actions settings-save-row">
        <button id="saveSettingsBtn" class="btn primary" type="button">ذخیره تنظیمات</button>
        <span id="settingsMsg" class="settings-msg"></span>
      </div>
    </section>

    <!-- مودال شمارش معکوس پس از دانلود (۰٫۶٫۳) -->
    <div id="powerModal" class="power-modal" hidden>
      <div class="card power-card">
        <h3>دانلودها تمام شدند ✓</h3>
        <p id="powerText" class="muted"></p>
        <div id="powerCount" class="power-count">۶۰</div>
        <button id="powerCancelBtn" class="btn primary" type="button">لغو</button>
      </div>
    </div>

    <div id="toast" class="toast" hidden></div>
  </div>`;

  const $ = <T extends HTMLElement>(sel: string) => root.querySelector<T>(sel)!;
  const urlInput = $<HTMLInputElement>("#urlInput");
  const listActive = $<HTMLElement>("#listActive");
  const listDone = $<HTMLElement>("#listDone");
  const tabActive = $<HTMLButtonElement>("#tabActive");
  const tabDone = $<HTMLButtonElement>("#tabDone");
  const countActive = $<HTMLElement>("#countActive");
  const countDone = $<HTMLElement>("#countDone");
  const totalSpeedEl = $<HTMLElement>("#totalSpeed");
  const chartNowEl = $<HTMLElement>("#chartNow");
  const toastEl = $<HTMLElement>("#toast");
  const settingsPage = $<HTMLElement>("#settingsPage");
  const appEl = root.querySelector<HTMLElement>(".app")!;

  // نمای تنظیمات (۰٫۶٫۳): صفحه‌ی جدا؛ با باز شدنش بخش‌های دانلود
  // با کلاس settings-open پنهان می‌شوند.
  function showSettingsPage(show: boolean): void {
    settingsPage.hidden = !show;
    appEl.classList.toggle("settings-open", show);
  }

  // دو صفحه‌ی دانلود (۰٫۶٫۴): «در حال دانلود» = هرچه هنوز کامل نشده
  // (در حال دانلود، در صف، مکث، خطا)؛ «دانلود شده‌ها» = فقط کامل‌شده‌ها.
  // صفحه‌ی آخر انتخاب‌شده به خاطر سپرده می‌شود.
  type DlPage = "active" | "done";
  const DL_PAGE_KEY = "dm_dl_page";
  let dlPage: DlPage = localStorage.getItem(DL_PAGE_KEY) === "done" ? "done" : "active";
  function showDlPage(which: DlPage): void {
    dlPage = which;
    localStorage.setItem(DL_PAGE_KEY, which);
    listActive.hidden = which !== "active";
    listDone.hidden = which !== "done";
    tabActive.classList.toggle("active", which === "active");
    tabDone.classList.toggle("active", which === "done");
  }
  tabActive.addEventListener("click", () => showDlPage("active"));
  tabDone.addEventListener("click", () => showDlPage("done"));
  showDlPage(dlPage);

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
  // روی اندروید: انتخاب پوشه پنهان می‌شود و دکمه‌های باز کردن/
  // اشتراک‌گذاری برای دانلودهای کامل‌شده نشان داده می‌شوند.
  let isAndroid = false;

  /* ── نوشتنِ محافظه‌کارانه در DOM: مقدارِ تکراری هرگز بازنویسی نمی‌شود ──
     (بازبینیِ هر ثانیه‌ی همه‌ی فیلدها وسطِ فشارِ موس در WebView2 ویندوز
     کلیک را می‌خورد — باگ دکمه‌های ۰٫۶٫۶) */
  function setText(el: HTMLElement, value: string): void {
    if (el.textContent !== value) el.textContent = value;
  }
  function setDisabled(btn: HTMLButtonElement, value: boolean): void {
    if (btn.disabled !== value) btn.disabled = value;
  }

  /* ── انتخابِ گروهی ردیف‌ها ── */
  const selected = new Set<string>();
  const bulkBar = $<HTMLElement>("#bulkBar");
  const bulkCount = $<HTMLElement>("#bulkCount");
  const bulkPauseBtn = $<HTMLButtonElement>("#bulkPauseBtn");
  const bulkResumeBtn = $<HTMLButtonElement>("#bulkResumeBtn");
  const bulkRemoveBtn = $<HTMLButtonElement>("#bulkRemoveBtn");
  const pauseAllBtn = $<HTMLButtonElement>("#pauseAllBtn");
  const resumeAllBtn = $<HTMLButtonElement>("#resumeAllBtn");

  function syncBulkBar(): void {
    bulkBar.hidden = selected.size === 0;
    setText(bulkCount, selected.size === 0 ? "" : `${faNum(selected.size)} دانلود انتخاب شده`);
    const selItems = items.filter((i) => selected.has(i.id));
    setDisabled(
      bulkPauseBtn,
      !selItems.some((i) => i.status === "downloading" || i.status === "queued"),
    );
    setDisabled(bulkResumeBtn, !selItems.some((i) => i.status === "paused"));
    setDisabled(bulkRemoveBtn, selItems.length === 0);
    setDisabled(
      pauseAllBtn,
      !items.some((i) => i.status === "downloading" || i.status === "queued"),
    );
    setDisabled(resumeAllBtn, !items.some((i) => i.status === "paused"));
  }

  function buildRow(it: DownloadItem): RowRefs {
    const el = document.createElement("article");
    el.className = "dl card";
    el.innerHTML = `
      <label class="dl-select"><input class="dl-check" type="checkbox" aria-label="انتخاب دانلود" /></label>
      <div class="dl-ic">${fileIcon(it.file_name)}</div>
      <div class="dl-main">
        <div class="dl-top">
          <span class="dl-name"></span>
          <span class="dl-status"></span>
        </div>
        <div class="dl-line"><span class="dl-size"></span><span class="dl-pct"></span></div>
        <div class="dl-bar"><div class="dl-fill"></div></div>
        <div class="dl-nums">
          <span class="dl-speed"></span>
          <span class="dl-eta"></span>
        </div>
      </div>
      <div class="dl-actions">
        <button class="btn small toggle" type="button"></button>
        <button class="btn small copy-link" type="button">کپی لینک</button>
        <button class="btn small open-file" type="button">باز کردن</button>
        <button class="btn small share-file" type="button">اشتراک‌گذاری</button>
        <button class="btn small danger remove" type="button">حذف</button>
        <button class="btn small danger remove-file" type="button">حذف فایل</button>
      </div>`;
    const refs: RowRefs = {
      el,
      name: el.querySelector(".dl-name")!,
      size: el.querySelector(".dl-size")!,
      barFill: el.querySelector(".dl-fill")!,
      pct: el.querySelector(".dl-pct")!,
      speed: el.querySelector(".dl-speed")!,
      eta: el.querySelector(".dl-eta")!,
      status: el.querySelector(".dl-status")!,
      toggleBtn: el.querySelector(".toggle")!,
      openBtn: el.querySelector(".open-file")!,
      shareBtn: el.querySelector(".share-file")!,
      selectBox: el.querySelector(".dl-check")!,
    };
    refs.selectBox.addEventListener("change", () => {
      if (refs.selectBox.checked) selected.add(it.id);
      else selected.delete(it.id);
      syncBulkBar();
    });
    refs.toggleBtn.addEventListener("click", () => void onToggle(it.id));
    el.querySelector(".copy-link")!.addEventListener("click", () => void onCopyLink(it.id));
    refs.openBtn.addEventListener("click", () => void onOpen(it.id));
    refs.shareBtn.addEventListener("click", () => void onShare(it.id));
    el.querySelector(".remove")!.addEventListener("click", () => void onRemove(it.id));
    el.querySelector(".remove-file")!.addEventListener("click", () => void onRemoveFile(it.id));
    return refs;
  }

  function updateRow(refs: RowRefs, it: DownloadItem): void {
    const pct = progressPct(it);
    setText(refs.name, it.file_name || "فایل بدون نام");
    if (refs.name.title !== it.url) refs.name.title = it.url;
    setText(
      refs.size,
      it.size !== null
        ? `${fmtBytes(it.downloaded)} از ${fmtBytes(it.size)}`
        : `${fmtBytes(it.downloaded)} دانلود شده — حجم کل نامشخص`,
    );
    const width = pct !== null ? `${pct}%` : it.status === "completed" ? "100%" : "0%";
    if (refs.barFill.style.width !== width) refs.barFill.style.width = width;
    if (refs.el.dataset.status !== it.status) refs.el.dataset.status = it.status;
    setText(
      refs.pct,
      pct !== null ? `— ${fmtPct(pct)}` : it.status === "completed" ? "— ۱۰۰٪" : "—",
    );
    setText(refs.speed, it.status === "downloading" ? fmtSpeed(it.speed) : "");
    const eta = etaSeconds(it);
    setText(refs.eta, eta !== null ? `حدود ${fmtEta(eta)} مانده` : "");
    setText(
      refs.status,
      it.status === "error" && it.error ? `خطا: ${stripUrlFromError(it.error)}` : STATUS_LABEL[it.status],
    );
    const statusTitle = it.status === "error" && it.error ? `خطا: ${it.error}` : "";
    if (refs.status.title !== statusTitle) refs.status.title = statusTitle;
    // دکمه‌ی زیرِ فشارِ موس همین تیک بازنویسی نمی‌شود تا کلیک خورده نشود.
    if (!refs.toggleBtn.matches(":active")) {
      setText(
        refs.toggleBtn,
        it.status === "downloading" || it.status === "queued" ? "⏸ مکث" : "▶ ادامه",
      );
      setDisabled(refs.toggleBtn, it.status === "completed" || it.status === "error");
    }
    // باز کردن/اشتراک‌گذاری فقط روی اندروید و فقط برای فایل کامل‌شده
    // (روی دسکتاپ فایل از قبل در پوشه‌ی کاربر دیده می‌شود).
    const showFileActions = isAndroid && it.status === "completed";
    if (refs.openBtn.hidden !== !showFileActions) refs.openBtn.hidden = !showFileActions;
    if (refs.shareBtn.hidden !== !showFileActions) refs.shareBtn.hidden = !showFileActions;
    // همگام‌سازی چک‌باکس انتخاب؛ مقدارِ تکراری بازنویسی نمی‌شود.
    const wantChecked = selected.has(it.id);
    if (refs.selectBox.checked !== wantChecked) refs.selectBox.checked = wantChecked;
  }

  // جای خالی هر صفحه (یک بار ساخته می‌شود؛ با innerHTML پاک نمی‌شود
  // تا ردیف‌های کلیددار سالم بمانند).
  const emptyActive = document.createElement("div");
  emptyActive.className = "empty card";
  emptyActive.innerHTML = `<div class="empty-ic">⬇</div>
    <p>دانلودی در حال انجام نیست.</p>
    <p class="muted">لینک فایل را در کادر بالا بچسبانید یا از دکمه‌ی «از کلیپ‌بورد» استفاده کنید.</p>`;
  const emptyDone = document.createElement("div");
  emptyDone.className = "empty card";
  emptyDone.innerHTML = `<div class="empty-ic">✓</div>
    <p>هنوز دانلودی کامل نشده است.</p>
    <p class="muted">دانلودهای کامل‌شده اینجا می‌آیند.</p>`;

  function render(): void {
    // کارت‌های «خالی» اول کنار می‌روند تا جای ردیف‌ها در محاسبه‌ی
    // ترتیب دقیق باشد؛ در پایان در صورت نیاز دوباره اضافه می‌شوند.
    emptyActive.remove();
    emptyDone.remove();
    const seen = new Set<string>();
    let nActive = 0;
    let nDone = 0;
    // آخرین ردیفِ گذاشته‌شده در هر صفحه؛ ردیف فقط وقتی جابه‌جا می‌شود
    // که والد یا ترتیبِ قرارگیری‌اش واقعاً عوض شده باشد.
    let prevActive: Element | null = null;
    let prevDone: Element | null = null;
    for (const it of items) {
      seen.add(it.id);
      let refs = rows.get(it.id);
      if (!refs) {
        refs = buildRow(it);
        rows.set(it.id, refs);
      }
      const list = it.status === "completed" ? listDone : listActive;
      const prev = it.status === "completed" ? prevDone : prevActive;
      const el = refs.el;
      const needsMove =
        el.parentElement !== list ||
        (prev === null ? el.previousElementSibling !== null : el.previousElementSibling !== prev);
      if (needsMove) {
        if (prev === null) list.prepend(el);
        else prev.after(el);
      }
      if (it.status === "completed") {
        prevDone = el;
        nDone++;
      } else {
        prevActive = el;
        nActive++;
      }
      updateRow(refs, it);
    }
    for (const [id, refs] of rows) {
      if (!seen.has(id)) {
        selected.delete(id);
        refs.el.remove();
        rows.delete(id);
      }
    }
    if (nActive === 0) listActive.appendChild(emptyActive);
    if (nDone === 0) listDone.appendChild(emptyDone);
    setText(countActive, faInt(nActive));
    setText(countDone, faInt(nDone));

    const totalSpeed = items.reduce((s, i) => s + (i.status === "downloading" ? i.speed : 0), 0);
    setText(totalSpeedEl, fmtSpeed(totalSpeed));
    setText(chartNowEl, fmtSpeed(totalSpeed));
    chart.push(totalSpeed);
    setText($("#statActive"), faInt(items.filter((i) => i.status === "downloading").length));
    setText($("#statDone"), fmtBytes(items.reduce((s, i) => s + i.downloaded, 0)));
    setText($("#statCount"), faInt(items.length));
    syncBulkBar();
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

  // کپی لینک دانلود (۰٫۶٫۳، مثل ADM و FDM): اول کامند بک‌اند با
  // پلاگین کلیپ‌بورد، و اگر نشد fallback مرورگر؛ بعد تأیید کوتاه.
  async function onCopyLink(id: string): Promise<void> {
    const it = items.find((x) => x.id === id);
    if (!it) return;
    try {
      await api.copyToClipboard(it.url);
    } catch {
      try {
        await navigator.clipboard.writeText(it.url);
      } catch {
        toast("کپی لینک ناموفق بود.", true);
        return;
      }
    }
    toast("لینک کپی شد ✓");
  }

  async function onOpen(id: string): Promise<void> {
    try {
      await api.openDownload(id);
    } catch (e) {
      toast("باز کردن فایل ناموفق بود: " + String((e as Error)?.message || e), true);
    }
  }

  async function onShare(id: string): Promise<void> {
    try {
      await api.shareDownload(id);
    } catch (e) {
      toast("اشتراک‌گذاری ناموفق بود: " + String((e as Error)?.message || e), true);
    }
  }

  async function onRemoveFile(id: string): Promise<void> {
    const it = items.find((x) => x.id === id);
    if (!it) return;
    const msg =
      it.status === "completed"
        ? `فایل «${it.file_name}» از روی دیسک هم پاک شود؟ این کار قابل برگشت نیست.`
        : `دانلود ناتمام «${it.file_name}» همراه فایل ناقص و داده‌های ادامه‌اش از روی دیسک پاک شود؟ این کار قابل برگشت نیست.`;
    if (!window.confirm(msg)) return;
    try {
      await api.removeDownload(id, true);
      void refresh();
    } catch (e) {
      toast("خطا در حذف فایل: " + String((e as Error)?.message || e), true);
    }
  }

  /* ── اقدام‌های گروهی: همان فرمان‌های تکی، پشتِ سرِ هم ── */
  async function bulkPause(ids: string[]): Promise<void> {
    let done = 0;
    let firstError: unknown = null;
    for (const id of ids) {
      try {
        await api.pauseDownload(id);
        done++;
      } catch (e) {
        if (firstError === null) firstError = e;
      }
    }
    if (done > 0) toast(`توقف ${faNum(done)} دانلود انجام شد.`);
    if (firstError !== null)
      toast("خطا در توقف: " + String((firstError as Error)?.message || firstError), true);
    void refresh();
  }

  async function bulkResume(ids: string[]): Promise<void> {
    let done = 0;
    let firstError: unknown = null;
    for (const id of ids) {
      try {
        await api.resumeDownload(id);
        done++;
      } catch (e) {
        if (firstError === null) firstError = e;
      }
    }
    if (done > 0) toast(`شروع ${faNum(done)} دانلود انجام شد.`);
    if (firstError !== null)
      toast("خطا در شروع: " + String((firstError as Error)?.message || firstError), true);
    void refresh();
  }

  async function bulkRemove(ids: string[]): Promise<void> {
    if (ids.length === 0) return;
    if (
      !window.confirm(
        `${faNum(ids.length)} دانلود انتخاب‌شده از لیست حذف شود؟ (فایل‌های دانلودشده روی دیسک می‌مانند)`,
      )
    )
      return;
    let done = 0;
    let failed = 0;
    for (const id of ids) {
      try {
        await api.removeDownload(id, false);
        selected.delete(id);
        done++;
      } catch {
        failed++;
      }
    }
    if (done > 0) toast(`حذف ${faNum(done)} دانلود انجام شد.`);
    if (failed > 0) toast(`حذف ${faNum(failed)} دانلود ناموفق بود.`, true);
    syncBulkBar();
    void refresh();
  }

  pauseAllBtn.addEventListener("click", () => {
    const ids = items
      .filter((i) => i.status === "downloading" || i.status === "queued")
      .map((i) => i.id);
    if (ids.length > 0) void bulkPause(ids);
  });
  resumeAllBtn.addEventListener("click", () => {
    const ids = items.filter((i) => i.status === "paused").map((i) => i.id);
    if (ids.length > 0) void bulkResume(ids);
  });
  bulkPauseBtn.addEventListener("click", () => {
    const ids = items
      .filter((i) => selected.has(i.id) && (i.status === "downloading" || i.status === "queued"))
      .map((i) => i.id);
    if (ids.length > 0) void bulkPause(ids);
    else toast("موردی برای توقف انتخاب نشده است.");
  });
  bulkResumeBtn.addEventListener("click", () => {
    const ids = items
      .filter((i) => selected.has(i.id) && i.status === "paused")
      .map((i) => i.id);
    if (ids.length > 0) void bulkResume(ids);
    else toast("موردی برای شروع انتخاب نشده است.");
  });
  bulkRemoveBtn.addEventListener("click", () => {
    void bulkRemove(items.filter((i) => selected.has(i.id)).map((i) => i.id));
  });

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
      showDlPage("active");
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

  /* ── درگ‌دراپ لینک: رها کردن لینک روی هرجای پنجره = افزودن دانلود ── */
  const dropOverlay = document.createElement("div");
  dropOverlay.className = "drop-overlay";
  dropOverlay.innerHTML = `<div class="drop-overlay-card"><div class="drop-overlay-ic">⬇</div><p>لینک را رها کنید تا دانلود شود</p></div>`;
  dropOverlay.hidden = true;
  root.appendChild(dropOverlay);

  // شمارنده‌ی ورود/خروج: dragleave موقع رد شدن از روی بچه‌ها هم می‌آید.
  let dragDepth = 0;
  function dragCarriesLink(e: DragEvent): boolean {
    const types: readonly string[] = e.dataTransfer?.types ?? [];
    return types.includes("text/uri-list") || types.includes("text/plain");
  }
  function hideDropOverlay(): void {
    dragDepth = 0;
    dropOverlay.hidden = true;
  }
  window.addEventListener("dragenter", (e) => {
    if (!dragCarriesLink(e)) return;
    e.preventDefault();
    dragDepth++;
    dropOverlay.hidden = false;
  });
  window.addEventListener("dragover", (e) => {
    if (!dragCarriesLink(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  });
  window.addEventListener("dragleave", (e) => {
    if (!dragCarriesLink(e)) return;
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) dropOverlay.hidden = true;
  });
  window.addEventListener("dragend", hideDropOverlay);
  window.addEventListener("drop", (e) => {
    const target = e.target as HTMLElement | null;
    // رها کردن روی فیلدهای ورودی، چسباندنِ بومیِ مرورگر است — دست نمی‌زنیم.
    if (target?.closest("input, textarea, select, [contenteditable]")) {
      hideDropOverlay();
      return;
    }
    if (!dragCarriesLink(e)) {
      hideDropOverlay();
      return;
    }
    e.preventDefault();
    hideDropOverlay();
    const dt = e.dataTransfer;
    if (!dt || dt.files.length > 0) return; // فقط لینک؛ دراپِ فایل نادیده گرفته می‌شود
    const lines = [
      ...dt.getData("text/uri-list").split(/\r?\n/),
      ...dt.getData("text/plain").split(/\r?\n/),
    ]
      .map((s) => s.trim())
      .filter((s) => s !== "" && !s.startsWith("#"));
    const urls = [...new Set(lines.filter(looksLikeUrl))];
    if (urls.length === 0) {
      if (lines.length > 0) toast("این لینک معتبر به نظر نمی‌رسد.", true);
      return;
    }
    void (async () => {
      for (const url of urls) await addFromInput(url);
    })();
  });

  /* ── تنظیمات ── */
  const settingsMsg = $<HTMLElement>("#settingsMsg");
  $("#settingsBtn").addEventListener("click", async () => {
    const opening = settingsPage.hidden;
    showSettingsPage(opening);
    if (opening) {
      try {
        const s = await api.getSettings();
        $<HTMLInputElement>("#setDir").value = s.download_dir;
        $<HTMLInputElement>("#setMaxParallel").value = String(s.max_parallel);
        $<HTMLInputElement>("#setParallel").value = String(s.max_connections);
        $<HTMLInputElement>("#setSpeed").value = String(s.speed_limit_kbps);
        $<HTMLSelectElement>("#setPostAction").value = s.post_download_action || "none";
        settingsMsg.textContent = "";
      } catch {
        settingsMsg.textContent = "خواندن تنظیمات از بک‌اند ممکن نشد.";
      }
    }
  });

  $("#backFromSettingsBtn").addEventListener("click", () => showSettingsPage(false));

  function applyAndroidUi(): void {
    $<HTMLInputElement>("#setDir").disabled = isAndroid;
    $<HTMLElement>("#pickDirBtn").hidden = isAndroid;
    $<HTMLElement>("#androidDirNote").hidden = !isAndroid;
    // حالت پس از دانلود فقط دسکتاپ است (خاموش/خواب/ری‌استارت سیستم).
    $<HTMLElement>("#postDownloadSection").hidden = isAndroid;
  }

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
      max_parallel: Math.min(16, Math.max(1, Number($<HTMLInputElement>("#setMaxParallel").value) || 3)),
      max_connections: Math.min(16, Math.max(1, Number($<HTMLInputElement>("#setParallel").value) || 8)),
      speed_limit_kbps: Math.max(0, Number($<HTMLInputElement>("#setSpeed").value) || 0),
      post_download_action: $<HTMLSelectElement>("#setPostAction")
        .value as PostDownloadAction,
    };
    try {
      await api.setSettings(settings);
      settingsMsg.textContent = "تنظیمات ذخیره شد ✓";
    } catch (e) {
      settingsMsg.textContent = "خطا در ذخیره: " + String((e as Error)?.message || e);
    }
  });

  /* ── شمارش معکوس پس از دانلود (۰٫۶٫۳) ── */
  const powerModal = $<HTMLElement>("#powerModal");
  const powerText = $<HTMLElement>("#powerText");
  const powerCount = $<HTMLElement>("#powerCount");
  const POWER_LABELS: Record<string, string> = {
    shutdown: "خاموش می‌شود",
    sleep: "به خواب می‌رود",
    restart: "ری‌استارت می‌شود",
  };
  // نسل شمارشِ در حال نمایش؛ رویدادهای نسل ناهم‌خوان نادیده گرفته
  // می‌شوند تا لغو شمارش قدیمی، مودال شمارش تازه را نپوشاند.
  let powerRun: number | null = null;
  function hidePowerModal(): void {
    powerModal.hidden = true;
    powerRun = null;
  }
  void onPostDownloadCountdown((p) => {
    powerRun = p.run;
    powerModal.hidden = false;
    powerText.textContent = `سیستم تا ${faNum(p.remaining)} ثانیه‌ی دیگر ${POWER_LABELS[p.action] || ""}؛ برای لغو، دکمه‌ی زیر را بزن.`;
    powerCount.textContent = faNum(p.remaining);
  }).catch(() => undefined);
  void onPostDownloadCancelled((p) => {
    if (powerRun === p.run) hidePowerModal();
  }).catch(() => undefined);
  void onPostDownloadExecuted((p) => {
    if (powerRun === p.run) {
      hidePowerModal();
      toast("حالت پس از دانلود در حال اجراست.");
    }
  }).catch(() => undefined);
  void onPostDownloadFailed((p) => {
    if (powerRun === p.run) {
      hidePowerModal();
      toast("اجرای حالت پس از دانلود ناموفق بود: " + p.error, true);
    }
  }).catch(() => undefined);
  $("#powerCancelBtn").addEventListener("click", () => {
    hidePowerModal();
    void api.cancelPostDownload().catch(() => undefined);
  });

  /* ── چرخه‌ی به‌روزرسانی: رویداد بک‌اند + پول ۱ ثانیه‌ای ── */
  void onProgressSnapshot(applySnapshot).catch(() => undefined);
  // تشخیص سکو برای UI اندروید (پنهان‌کردن انتخاب پوشه، دکمه‌های
  // باز/اشتراک در ردیف‌ها) و رندر دوباره‌ی فهرست با آن.
  void api.platform().then((p) => {
    isAndroid = p === "android";
    applyAndroidUi();
    void refresh();
  }).catch(() => undefined);
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
