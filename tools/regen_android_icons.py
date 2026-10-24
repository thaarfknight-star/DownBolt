#!/usr/bin/env python3
"""بازتولید آیکن‌های لانچر اندروید DownBolt از لوگوی اصلی (branding/logo.png).

مشکل قبلی: فورگراند یک عکس تمام‌قاب بود و صاعقه تا لبه‌ها می‌رسید؛ ماسک‌های
adaptive icon (دایره/اسکوئرکل سامسونگ) نوک‌هایش را می‌بریدند.

روش این اسکریپت:
  ۱) از عکس اصلی (صاعقه‌ی نئونی روی پس‌زمینه‌ی سرمه‌ای تیره) با آستانه‌ی
     روشنایی یک matte آلفا استخراج می‌شود — خود صاعقه مات و درخشان است،
     پس‌زمینه تقریباً مشکی-سرمه‌ای و جدا شدنی.
  ۲) رنگ پس‌زمینه از گوشه‌های عکس نمونه‌برداری و در values/ic_launcher_background.xml
     می‌نشیند تا هاله‌ی نرم نور صاعقه روی همان رنگ بنشیند و درز دیده نشود.
  ۳) فورگراند هر density طوری ساخته می‌شود که قابِ هسته‌ی صاعقه (آلفای بالای
     ۵۰٪) حداکثر ۶۰٪ بوم 108dp را بگیرد و وسط‌چین باشد — داخل ناحیه‌ی امن
     هر سه ماسک رایج.
  ۴) آیکن‌های legacy (ic_launcher / ic_launcher_round) هم با همان قاب‌بندی
     روی پس‌زمینه‌ی سرمه‌ای ساخته می‌شوند.

اجرا: python3 tools/regen_android_icons.py
خروجی: src-tauri/icons/android/ (همان جایی که CI روی قالب Tauri می‌نشاند)
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
MASTER = ROOT / "branding" / "logo.png"
OUT = ROOT / "src-tauri" / "icons" / "android"

# density → اندازه‌ی فورگراند (108dp) و آیکن legacy (48dp)
DENSITIES = {
    "mdpi": (108, 48),
    "hdpi": (162, 72),
    "xhdpi": (216, 96),
    "xxhdpi": (324, 144),
    "xxxhdpi": (432, 192),
}
FG_FIT = 0.60      # سقف قاب هسته‌ی صاعقه در فورگراند (از بوم)
LEGACY_FIT = 0.64  # سقف قاب در آیکن legacy


def extract_bolt(master: Image.Image) -> Image.Image:
    """استخراج صاعقه با کانال آلفا از روی روشنایی (پس‌زمینه تیره است)."""
    rgb = master.convert("RGB")
    px = rgb.load()
    w, h = rgb.size
    out = Image.new("RGBA", (w, h))
    opx = out.load()
    LO, HI = 38.0, 120.0  # بازه‌ی گذار آلفا (نرم، برای هاله‌ی نور) — کف ۳۸ گرادیانِ کم‌نور پس‌زمینه را هم کامل حذف می‌کند تا شبح مربعی نماند
    for y in range(h):
        for x in range(w):
            r, g, b = px[x, y]
            lum = max(r, g, b)
            if lum <= LO:
                a = 0
            elif lum >= HI:
                a = 255
            else:
                a = int(255 * (lum - LO) / (HI - LO))
            opx[x, y] = (r, g, b, a)
    return out


def sample_bg_color(master: Image.Image) -> tuple[int, int, int]:
    """میانگین رنگ چهار گوشه‌ی عکس (پس‌زمینه‌ی خالص)."""
    rgb = master.convert("RGB")
    w, h = rgb.size
    s = 40
    regions = [(0, 0), (w - s, 0), (0, h - s), (w - s, h - s)]
    tot = [0, 0, 0]
    n = 0
    px = rgb.load()
    for rx, ry in regions:
        for y in range(ry, ry + s):
            for x in range(rx, rx + s):
                r, g, b = px[x, y]
                tot[0] += r
                tot[1] += g
                tot[2] += b
                n += 1
    return (tot[0] // n, tot[1] // n, tot[2] // n)


def core_bbox(img: Image.Image) -> tuple[int, int, int, int]:
    """قاب هسته‌ی صاعقه از روی صدک‌های مختصات پیکسل‌های پرآلفا.

    برق‌ها و ذره‌های پراکنده‌ی پس‌زمینه پیکسلِ پرآلفا دارند ولی در برابر توده‌ی
    صاعقه ناچیزند؛ صدک ۰٫۱ و ۹۹٫۹ در هر محور، قاب واقعی بدنه را می‌دهد و
    نوک‌های تیز هم (که توده‌ی کافی دارند) گم نمی‌شوند.
    """
    import numpy as np

    a = np.asarray(img.getchannel("A"))
    ys, xs = np.nonzero(a > 128)
    x0, x1 = np.percentile(xs, [0.1, 99.9])
    y0, y1 = np.percentile(ys, [0.1, 99.9])
    return (int(x0), int(y0), int(x1) + 1, int(y1) + 1)


def fit_paste(canvas: Image.Image, bolt: Image.Image, bbox, fit: float) -> None:
    """چسباندن صاعقه وسط بوم طوری که قاب هسته‌اش حداکثر fit بوم شود."""
    S = canvas.size[0]
    bw, bh = bbox[2] - bbox[0], bbox[3] - bbox[1]
    scale = (fit * S) / max(bw, bh)
    new_size = (round(bolt.size[0] * scale), round(bolt.size[1] * scale))
    resized = bolt.resize(new_size, Image.LANCZOS)
    # آفست طوری که مرکز قاب هسته دقیقاً مرکز بوم شود
    cx = (bbox[0] + bbox[2]) / 2 * scale
    cy = (bbox[1] + bbox[3]) / 2 * scale
    canvas.alpha_composite(resized, (round(S / 2 - cx), round(S / 2 - cy)))


def main() -> None:
    master = Image.open(MASTER)
    bolt = extract_bolt(master)
    bbox = core_bbox(bolt)
    bg = sample_bg_color(master)
    print(f"master={master.size} core_bbox={bbox} bg=#{bg[0]:02X}{bg[1]:02X}{bg[2]:02X}")

    for name, (fg_size, legacy_size) in DENSITIES.items():
        d = OUT / f"mipmap-{name}"
        d.mkdir(parents=True, exist_ok=True)
        # فورگراند adaptive: شفاف، صاعقه در ناحیه‌ی امن
        fg = Image.new("RGBA", (fg_size, fg_size), (0, 0, 0, 0))
        fit_paste(fg, bolt, bbox, FG_FIT)
        fg.save(d / "ic_launcher_foreground.png")
        # legacy مربعی: پس‌زمینه‌ی سرمه‌ای + صاعقه
        sq = Image.new("RGBA", (legacy_size, legacy_size), bg + (255,))
        sq2 = Image.new("RGBA", (legacy_size, legacy_size), (0, 0, 0, 0))
        fit_paste(sq2, bolt, bbox, LEGACY_FIT)
        sq.alpha_composite(sq2)
        sq.save(d / "ic_launcher.png")
        # legacy گرد: همان ترکیب، برش دایره‌ای
        rd = Image.new("RGBA", (legacy_size, legacy_size), (0, 0, 0, 0))
        rd.alpha_composite(sq)
        mask = Image.new("L", (legacy_size, legacy_size), 0)
        from PIL import ImageDraw

        ImageDraw.Draw(mask).ellipse([0, 0, legacy_size - 1, legacy_size - 1], fill=255)
        rd.putalpha(mask)
        rd.save(d / "ic_launcher_round.png")
        print(f"{name}: fg={fg_size} legacy={legacy_size} ✓")

    # رنگ پس‌زمینه‌ی adaptive
    (OUT / "values").mkdir(parents=True, exist_ok=True)
    (OUT / "values" / "ic_launcher_background.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n'
        "<resources>\n"
        f'  <color name="ic_launcher_background">#{bg[0]:02X}{bg[1]:02X}{bg[2]:02X}</color>\n'
        "</resources>\n",
        encoding="utf-8",
    )
    print("background xml ✓")


if __name__ == "__main__":
    main()
