// dropbox.ts — جعبه‌ی شناور دراپ DownBolt (۰٫۶٫۱۳، مثل FDM).
//
// این پنجره‌ی کوچکِ همیشه‌روی‌صفحه فقط یک کار می‌کند: لینکِ رها‌شده را
// می‌گیرد و با کامند forward_dropbox_link به پنجره‌ی اصلی می‌فرستد تا همان
// جریان تأیید دانلودِ معمولی (حجم، مسیر، فضای درایو) اجرا شود. خودِ جعبه
// دانلودی شروع نمی‌کند و پنجره‌ی تأیید را هم نشان نمی‌دهد.
// جابه‌جایی جعبه با کشیدنِ نوار بالاست (data-tauri-drag-region)؛ دکمه‌ی ×
// آن را می‌بندد و انتخابِ «خاموش» را هم ذخیره می‌کند (فعال‌سازی دوباره از
// تنظیمات پنجره‌ی اصلی).

import { api } from "../lib/api";
import { looksLikeUrl } from "./app";

export function mountDropbox(root: HTMLElement): void {
  document.body.classList.add("dropbox-mode");
  root.innerHTML = `
  <div class="dropbox">
    <div class="dropbox-handle" data-tauri-drag-region>
      <span>DownBolt</span>
      <span class="dropbox-handle-hint">بکشید تا جابه‌جا شود</span>
    </div>
    <button class="dropbox-close" type="button" title="بستن جعبه (فعال‌سازی دوباره از تنظیمات)">×</button>
    <div class="dropbox-zone">
      <div class="dropbox-ic">⬇</div>
      <div class="dropbox-text">لینک را اینجا رها کنید</div>
      <div class="dropbox-hint">پنجره‌ی تأیید دانلود باز می‌شود</div>
      <div class="dropbox-state"></div>
    </div>
  </div>`;

  const $ = <T extends HTMLElement>(sel: string) => root.querySelector<T>(sel)!;
  const zone = $(".dropbox-zone");
  const stateEl = $(".dropbox-state");
  let depth = 0;
  let stateTimer: number | undefined;

  function showState(msg: string): void {
    stateEl.textContent = msg;
    window.clearTimeout(stateTimer);
    stateTimer = window.setTimeout(() => {
      stateEl.textContent = "";
    }, 3000);
  }

  function carriesLink(e: DragEvent): boolean {
    const types: readonly string[] = e.dataTransfer?.types ?? [];
    return (
      types.includes("text/uri-list") ||
      types.includes("text/plain") ||
      types.includes("text/html")
    );
  }

  window.addEventListener("dragenter", (e) => {
    if (!carriesLink(e)) return;
    e.preventDefault();
    depth++;
    zone.classList.add("drag-over");
  });
  window.addEventListener("dragover", (e) => {
    if (!carriesLink(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  });
  window.addEventListener("dragleave", () => {
    depth = Math.max(0, depth - 1);
    if (depth === 0) zone.classList.remove("drag-over");
  });
  window.addEventListener("dragend", () => {
    depth = 0;
    zone.classList.remove("drag-over");
  });
  window.addEventListener("drop", (e) => {
    depth = 0;
    zone.classList.remove("drag-over");
    if (!carriesLink(e)) return;
    e.preventDefault();
    const dt = e.dataTransfer;
    if (!dt || dt.files.length > 0) return; // فقط لینک؛ دراپِ فایل نادیده گرفته می‌شود
    // اگر مرورگر فقط html داده بود، href لینک را از همان html دربیاور.
    const htmlHrefs: string[] = [];
    try {
      const html = dt.getData("text/html");
      if (html) {
        const doc = new DOMParser().parseFromString(html, "text/html");
        doc.querySelectorAll("a[href]").forEach((a) => {
          const href = a.getAttribute("href");
          if (href) htmlHrefs.push(href);
        });
      }
    } catch {
      // نبودِ html یعنی همان دو منبعِ قبلی کافی‌اند.
    }
    const lines = [
      ...dt.getData("text/uri-list").split(/\r?\n/),
      ...dt.getData("text/plain").split(/\r?\n/),
      ...htmlHrefs,
    ]
      .map((s) => s.trim())
      .filter((s) => s !== "" && !s.startsWith("#"));
    const urls = [...new Set(lines.filter(looksLikeUrl))];
    if (urls.length === 0) {
      if (lines.length > 0) showState("این لینک معتبر به نظر نمی‌رسد.");
      return;
    }
    showState("دریافت شد ✓");
    void (async () => {
      for (const url of urls) {
        try {
          await api.forwardDropboxLink(url);
        } catch {
          // پنجره‌ی اصلی در دسترس نیست؛ لینک بعدی باز هم فرستاده می‌شود.
        }
      }
    })();
  });

  $(".dropbox-close").addEventListener("click", () => {
    void api.setDropboxVisible(false).catch(() => undefined);
  });
}
