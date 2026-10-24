package ir.downbolt.plugin.install

import android.Manifest
import android.app.Activity
import android.content.ContentValues
import android.content.Intent
import android.content.pm.PackageManager
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileInputStream

@InvokeArg
class InstallApkArgs {
    lateinit var path: String
}

@InvokeArg
class PublishArgs {
    lateinit var path: String
    lateinit var displayName: String
    lateinit var mimeType: String
}

@InvokeArg
class FileArgs {
    lateinit var path: String
    var uri: String? = null
    lateinit var mimeType: String
}

@InvokeArg
class DeleteArgs {
    lateinit var uri: String
}

@TauriPlugin
class InstallPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun installApk(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(InstallApkArgs::class.java)
            val file = File(args.path)
            val uri: Uri = FileProvider.getUriForFile(
                activity,
                activity.packageName + ".downbolt-fileprovider",
                file
            )
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "install failed")
        }
    }

    /**
     * انتشار فایل کامل‌شده در پوشه‌ی Downloads عمومی، تا در فایل‌منیجر
     * دیده شود. اندروید ۱۰ به بالا: MediaStore با RELATIVE_PATH=Download
     * (بدون نیاز به مجوز ذخیره‌سازی، و بدون MANAGE_EXTERNAL_STORAGE).
     * پایین‌تر: کپی در پوشه‌ی عمومی Downloads + اسکن مدیا — فقط وقتی
     * مجوز WRITE_EXTERNAL_STORAGE از قبل داده شده باشد، وگرنه reject.
     * خروجی: URI ورودیِ منتشرشده (برای پاک‌کردنِ بعدی با deletePublished)
     * یا مسیر فایل عمومی در شاخه‌ی قدیمی.
     */
    @Command
    fun publishToDownloads(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(PublishArgs::class.java)
            val src = File(args.path)
            val outUri: String
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values = ContentValues().apply {
                    put(MediaStore.Downloads.DISPLAY_NAME, args.displayName)
                    put(MediaStore.Downloads.MIME_TYPE, args.mimeType)
                    put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
                    put(MediaStore.Downloads.IS_PENDING, 1)
                }
                val resolver = activity.contentResolver
                val uri = resolver.insert(
                    MediaStore.Downloads.EXTERNAL_CONTENT_URI, values
                ) ?: throw IllegalStateException("MediaStore insert failed")
                try {
                    resolver.openOutputStream(uri)?.use { out ->
                        FileInputStream(src).use { it.copyTo(out) }
                    } ?: throw IllegalStateException("MediaStore output stream failed")
                    values.clear()
                    values.put(MediaStore.Downloads.IS_PENDING, 0)
                    resolver.update(uri, values, null, null)
                } catch (e: Exception) {
                    // ورودیِ pendingِ ناتمام را پاک کن تا ردیف یتیم و
                    // نامرئی در MediaStore جا نماند.
                    try { resolver.delete(uri, null, null) } catch (_: Exception) {}
                    throw e
                }
                outUri = uri.toString()
            } else {
                if (ContextCompat.checkSelfPermission(
                        activity, Manifest.permission.WRITE_EXTERNAL_STORAGE
                    ) != PackageManager.PERMISSION_GRANTED
                ) {
                    throw SecurityException("WRITE_EXTERNAL_STORAGE not granted")
                }
                val dest = File(
                    Environment.getExternalStoragePublicDirectory(
                        Environment.DIRECTORY_DOWNLOADS
                    ), args.displayName
                )
                src.copyTo(dest, overwrite = true)
                MediaScannerConnection.scanFile(
                    activity, arrayOf(dest.absolutePath), arrayOf(args.mimeType), null
                )
                outUri = dest.absolutePath
            }
            invoke.resolve(JSObject().put("uri", outUri))
        } catch (e: Exception) {
            invoke.reject(e.message ?: "publish failed")
        }
    }

    /**
     * پاک‌کردن ورودیِ منتشرشده (مکمل حذف از دیسک): برای URI محتوایی،
     * حذف از MediaStore؛ برای مسیر فایل (شاخه‌ی قدیمی)، حذف فایل.
     * صفر ردیف/فایلِ ازدست‌رفته یعنی از قبل پاک شده و موفقیت است.
     */
    @Command
    fun deletePublished(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DeleteArgs::class.java)
            if (args.uri.startsWith("content://")) {
                activity.contentResolver.delete(Uri.parse(args.uri), null, null)
            } else {
                File(args.uri).delete()
            }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "delete published failed")
        }
    }

    /** باز کردن فایل با برنامه‌ی مناسب سیستم (FileProvider یا URI عمومی). */
    @Command
    fun openFile(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(FileArgs::class.java)
            val uri: Uri = fileUri(args)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, args.mimeType)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "open failed")
        }
    }

    /** اشتراک‌گذاری فایل با chooser سیستم. */
    @Command
    fun shareFile(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(FileArgs::class.java)
            val uri: Uri = fileUri(args)
            val intent = Intent(Intent.ACTION_SEND).apply {
                type = args.mimeType
                putExtra(Intent.EXTRA_STREAM, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(
                Intent.createChooser(intent, null).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
            )
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "share failed")
        }
    }

    private fun fileUri(args: FileArgs): Uri {
        val raw = args.uri
        if (raw != null && raw.startsWith("content://")) return Uri.parse(raw)
        return FileProvider.getUriForFile(
            activity,
            activity.packageName + ".downbolt-fileprovider",
            File(args.path)
        )
    }
}
