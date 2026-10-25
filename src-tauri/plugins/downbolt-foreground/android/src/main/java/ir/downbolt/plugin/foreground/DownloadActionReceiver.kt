package ir.downbolt.plugin.foreground

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.io.File

/**
 * گیرنده‌ی اکشن‌های نوتیفیکیشن دانلودها. چون BroadcastReceiver به
 * AppHandle دسترسی ندارد، درخواست را اتمی (فایل موقت + rename) در
 * dataDir/fg_action.json می‌نویسد و نخ Rust در lib.rs هر ثانیه آن را
 * می‌خواند و اجرا می‌کند (شروع همه / توقف همه / خروج). نوشتنِ روی
 * هم، درخواست آخر را نگه می‌دارد — برای سه اکشن، همین کافی است.
 */
class DownloadActionReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        try {
            val action = when (intent.action) {
                DownloadForegroundService.ACTION_PAUSE_ALL -> "pause_all"
                DownloadForegroundService.ACTION_RESUME_ALL -> "resume_all"
                DownloadForegroundService.ACTION_EXIT -> "exit"
                else -> return
            }
            val dir: File = context.dataDir
            val tmp = File(dir, "fg_action.tmp")
            val dst = File(dir, "fg_action.json")
            tmp.writeText("{\"action\":\"$action\"}")
            // rename اتمی است (همان دایرکتوری)؛ پس سمت Rust هیچ‌وقت
            // فایل نیمه‌کاره نمی‌خواند.
            tmp.renameTo(dst)
        } catch (_: Exception) {
            // اکشن ازدست‌رفته بهتر از کرش Receiver است؛ کاربر با
            // بازکردن برنامه همان کار را می‌کند.
        }
    }
}
