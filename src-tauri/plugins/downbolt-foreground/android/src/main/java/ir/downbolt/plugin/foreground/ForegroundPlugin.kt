package ir.downbolt.plugin.foreground

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class SyncArgs {
    var activeCount: Int = 0
    var totalSpeed: Long = 0L
}

/**
 * پلاگین سرویس foreground دانلودها (فقط اندروید).
 *
 * جهت فراخوانی فقط Rust → Kotlin است: سمت Rust با sync/stop وضعیت را
 * هم‌گام می‌کند. اکشن‌های نوتیفیکیشن (شروع همه / توقف همه / خروج) از
 * راه BroadcastReceiver به فایل صندوق‌پستی dataDir/fg_action.json
 * می‌رسند و نخ Rust در lib.rs آن را می‌خواند — چون Receiver به
 * AppHandle دسترسی ندارد.
 */
@TauriPlugin
class ForegroundPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun syncNotification(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SyncArgs::class.java)
            requestNotificationPermissionIfNeeded()
            val intent = Intent(activity, DownloadForegroundService::class.java).apply {
                action = DownloadForegroundService.ACTION_SYNC
                putExtra(DownloadForegroundService.EXTRA_ACTIVE_COUNT, args.activeCount)
                putExtra(DownloadForegroundService.EXTRA_TOTAL_SPEED, args.totalSpeed)
            }
            // شروع از بک‌گراند در اندروید ۱۲+ ممنوع است؛ فراخواننده‌ی
            // Rust فقط وقتی سرویس در حال اجراست یا برنامه در پیش‌زمینه
            // است sync می‌زند، ولی خطا هم به‌هرحال reject می‌شود تا
            // سمت Rust بی‌صدا قورتش ندهد.
            ContextCompat.startForegroundService(activity, intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "foreground sync failed")
        }
    }

    @Command
    fun stopNotification(invoke: Invoke) {
        try {
            // stopService وقتی سرویس در حال اجرا نیست بی‌اثر است، پس
            // برخلاف startService با ACTION_STOP، فلشِ شروع/توقف ندارد.
            // خود سرویس در onDestroy نوتیفیکیشن را برمی‌دارد.
            activity.stopService(Intent(activity, DownloadForegroundService::class.java))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "foreground stop failed")
        }
    }

    /**
     * درخواست بهترین‌تلاش POST_NOTIFICATIONS (اندروید ۱۳+). نتیجه‌اش
     * این‌جا مشاهده نمی‌شود؛ اگر کاربر بدهد، sync بعدی که سمت Rust با
     * هر اسنپ‌شات/تغییر وضعیت تکرار می‌شود، نوتیفیکیشن را نشان می‌دهد.
     * اگر ندهد، سرویس باز هم بالا می‌ماند (پروسه زنده می‌ماند) و فقط
     * نوتیفیکیشن دیده نمی‌شود.
     */
    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (ContextCompat.checkSelfPermission(
                activity, Manifest.permission.POST_NOTIFICATIONS
            ) == PackageManager.PERMISSION_GRANTED
        ) return
        try {
            ActivityCompat.requestPermissions(
                activity, arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001
            )
        } catch (_: Exception) {
            // درخواست مجوز نباید sync را بیندازد.
        }
    }
}
