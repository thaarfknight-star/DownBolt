// chart.ts — نمودار سرعت زنده (مجموع سرعت همه‌ی دانلودها) روی کانوس
// نمونه‌ها حلقه‌ای نگه داشته می‌شوند (پیش‌فرض ۶۰ ثانیه‌ی اخیر) و خط نئونی با پرشدگی گرادیانی رسم می‌شود.

export class SpeedChart {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private samples: { t: number; v: number }[] = [];
  private readonly windowMs: number;

  constructor(canvas: HTMLCanvasElement, windowSeconds = 60) {
    this.canvas = canvas;
    this.windowMs = windowSeconds * 1000;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas 2d context unavailable");
    this.ctx = ctx;
    this.resize();
  }

  resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = Math.max(1, Math.round(rect.width * dpr));
    this.canvas.height = Math.max(1, Math.round(rect.height * dpr));
    this.draw();
  }

  /** افزودن نمونه‌ی سرعت (بایت‌برثانیه) با زمان فعلی و رسم دوباره */
  push(totalSpeedBps: number): void {
    const now = Date.now();
    this.samples.push({ t: now, v: Math.max(0, totalSpeedBps) });
    const cutoff = now - this.windowMs;
    while (this.samples.length > 1 && this.samples[0].t < cutoff) this.samples.shift();
    this.draw();
  }

  private draw(): void {
    const { ctx, canvas } = this;
    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    // خطوط راهنمای افقی
    ctx.strokeStyle = "rgba(120, 160, 220, 0.10)";
    ctx.lineWidth = 1;
    for (let i = 1; i <= 3; i++) {
      const y = (H / 4) * i;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(W, y);
      ctx.stroke();
    }

    if (this.samples.length < 2) return;

    const now = Date.now();
    const t0 = now - this.windowMs;
    const max = Math.max(...this.samples.map((s) => s.v), 1024); // حداقل مقیاس ۱KB/s
    const pad = H * 0.12;
    const x = (t: number) => ((t - t0) / this.windowMs) * W;
    const y = (v: number) => H - pad - (v / max) * (H - pad * 2);

    // پرشدگی زیر منحنی
    const fill = ctx.createLinearGradient(0, 0, 0, H);
    fill.addColorStop(0, "rgba(34, 211, 238, 0.35)");
    fill.addColorStop(1, "rgba(34, 211, 238, 0.02)");
    ctx.beginPath();
    ctx.moveTo(x(this.samples[0].t), H);
    for (const s of this.samples) ctx.lineTo(x(s.t), y(s.v));
    ctx.lineTo(x(this.samples[this.samples.length - 1].t), H);
    ctx.closePath();
    ctx.fillStyle = fill;
    ctx.fill();

    // خط اصلی نئونی
    const line = ctx.createLinearGradient(0, 0, W, 0);
    line.addColorStop(0, "#22d3ee");
    line.addColorStop(1, "#818cf8");
    ctx.beginPath();
    this.samples.forEach((s, i) => {
      if (i === 0) ctx.moveTo(x(s.t), y(s.v));
      else ctx.lineTo(x(s.t), y(s.v));
    });
    ctx.strokeStyle = line;
    ctx.lineWidth = Math.max(1.5, (window.devicePixelRatio || 1) * 1.4);
    ctx.shadowColor = "rgba(34, 211, 238, 0.65)";
    ctx.shadowBlur = 8;
    ctx.stroke();
    ctx.shadowBlur = 0;

    // نقطه‌ی انتهایی (آخرین سرعت)
    const last = this.samples[this.samples.length - 1];
    ctx.beginPath();
    ctx.arc(x(last.t), y(last.v), Math.max(2.5, (window.devicePixelRatio || 1) * 2.2), 0, Math.PI * 2);
    ctx.fillStyle = "#a5f3fc";
    ctx.fill();
  }
}
