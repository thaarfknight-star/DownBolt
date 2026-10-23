// format.ts — قالب‌بندی فارسی اعداد، حجم، سرعت و زمان برای دانلودمنیجر

const FA_D = ["۰", "۱", "۲", "۳", "۴", "۵", "۶", "۷", "۸", "۹"];

export const faNum = (v: number | string): string =>
  String(v).replace(/\d/g, (d) => FA_D[Number(d)]);

/** «۱۴۰۴» بدون جداکننده — برای اعداد صحیح ساده */
export function faInt(n: number): string {
  return faNum(String(Math.round(n)));
}

/** حجم به بایت → «۱٫۲ گیگابایت» / «۲۳ مگابایت» / «۵۱۲ کیلوبایت» / «۰ بایت» */
export function fmtBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) return "نامشخص";
  if (bytes === 0) return "۰ بایت";
  const units = ["بایت", "کیلوبایت", "مگابایت", "گیگابایت", "ترابایت"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const val = bytes / Math.pow(1024, i);
  const str = i === 0 ? String(Math.round(val)) : trimNum(val);
  return `${faNum(str)} ${units[i]}`;
}

/** سرعت به بایت‌برثانیه → «۳٫۵ مگابایت/ثانیه» */
export function fmtSpeed(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "۰ بایت/ثانیه";
  return fmtBytes(bps) + "/ثانیه";
}

/** زمان باقی‌مانده به ثانیه → «۲ ساعت و ۵ دقیقه» / «۴۰ ثانیه» / «—» */
export function fmtEta(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${faInt(s)} ثانیه`;
  const m = Math.floor(s / 60);
  if (m < 60) {
    const rs = s % 60;
    return rs > 0 ? `${faInt(m)} دقیقه و ${faInt(rs)} ثانیه` : `${faInt(m)} دقیقه`;
  }
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm > 0 ? `${faInt(h)} ساعت و ${faInt(rm)} دقیقه` : `${faInt(h)} ساعت`;
}

/** درصد با حداکثر یک رقم اعشار: «۴۵٫۵٪» */
export function fmtPct(p: number): string {
  return faNum(trimNum(p, 1)) + "٪";
}

function trimNum(v: number, digits = 2): string {
  const f = Math.pow(10, digits);
  const r = Math.round(v * f) / f;
  return Number.isInteger(r) ? String(r) : String(r).replace(".", "٫");
}
