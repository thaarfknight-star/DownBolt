package ir.downbolt.plugin.foreground

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat

/**
 * سرویس foreground دانلودها: تا وقتی دانلودی فعال است بالا می‌ماند تا
 * سیستم پروسه را نکشد و دانلود در بک‌گراند کامل شود. نوتیفیکیشن دائمی
 * (ongoing) با خلاصه‌ی پیشرفت + سه اکشن نشان می‌دهد. START_NOT_STICKY
 * است: اگر سیستم سرویس را بکشد، بدون برنامه برنمی‌گردد (وضعیت Rust
 * هم با مرگ پروسه رفته است).
 */
class DownloadForegroundService : Service() {

    companion object {
        const val ACTION_SYNC = "ir.downbolt.plugin.foreground.SYNC"
        const val EXTRA_ACTIVE_COUNT = "activeCount"
        const val EXTRA_TOTAL_SPEED = "totalSpeed"
        const val ACTION_PAUSE_ALL = "ir.downbolt.plugin.foreground.PAUSE_ALL"
        const val ACTION_RESUME_ALL = "ir.downbolt.plugin.foreground.RESUME_ALL"
        const val ACTION_EXIT = "ir.downbolt.plugin.foreground.EXIT"
        const val NOTIF_ID = 1001
        const val CHANNEL_ID = "downbolt_downloads"
    }

    override fun onCreate() {
        super.onCreate()
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_SYNC) {
            val active = intent.getIntExtra(EXTRA_ACTIVE_COUNT, 0)
            // نگهبان تدافعی: sync با صفر فعال یعنی سرویس باید برود.
            if (active <= 0) {
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                return START_NOT_STICKY
            }
            val speed = intent.getLongExtra(EXTRA_TOTAL_SPEED, 0L)
            val notification = buildNotification(active, speed)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(
                    NOTIF_ID, notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
                )
            } else {
                startForeground(NOTIF_ID, notification)
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        try {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } catch (_: Exception) {
        }
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(
            CHANNEL_ID, "دانلودها", NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "پیشرفت دانلودهای فعال DownBolt"
            setSound(null, null)
            enableVibration(false)
        }
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(activeCount: Int, totalSpeed: Long): Notification {
        // ضربه روی خود نوتیفیکیشن: برگرداندن برنامه به جلو، بدون
        // ساختن Activity تازه (لانچرِ بسته از PackageManager).
        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)?.apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        }
        val contentIntent = launchIntent?.let {
            PendingIntent.getActivity(
                this, 0, it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }
        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle("DownBolt — در حال دانلود")
            .setContentText("$activeCount دانلود فعال • سرعت ${formatSpeed(totalSpeed)}")
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(contentIntent)
            .addAction(actionButton(1, "خروج از برنامه", ACTION_EXIT))
            .addAction(actionButton(2, "شروع همه", ACTION_RESUME_ALL))
            .addAction(actionButton(3, "توقف همه", ACTION_PAUSE_ALL))
        return builder.build()
    }

    private fun actionButton(
        requestCode: Int, title: String, action: String
    ): NotificationCompat.Action {
        val intent = Intent(this, DownloadActionReceiver::class.java).apply {
            this.action = action
        }
        val pending = PendingIntent.getBroadcast(
            this, requestCode, intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        return NotificationCompat.Action.Builder(0, title, pending).build()
    }

    private fun formatSpeed(bytesPerSec: Long): String {
        return when {
            bytesPerSec < 1024 -> "$bytesPerSec بایت/ثانیه"
            bytesPerSec < 1024 * 1024 -> "%.1f کیلوبایت/ثانیه".format(bytesPerSec / 1024.0)
            else -> "%.1f مگابایت/ثانیه".format(bytesPerSec / 1024.0 / 1024.0)
        }
    }
}
