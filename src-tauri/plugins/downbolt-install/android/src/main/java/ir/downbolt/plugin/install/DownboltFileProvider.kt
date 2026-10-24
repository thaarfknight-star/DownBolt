package ir.downbolt.plugin.install

import androidx.core.content.FileProvider

/**
 * زیرکلاس خالی FileProvider برای پلاگین.
 *
 * چرا کلاس جدا لازم است: مرج مانیفست اندروید providerها را با android:name
 * (نام کلاس) کلید می‌زند، نه با authority — پس provider پلاگین با همان کلاس
 * FileProvider و authority متفاوت هم با provider قالب اپ تضاد می‌گرفت (بیلد
 * 0.6.1 دو بار با خطای مرج مرد). با نام کلاس متفاوت، دو provider دو ورودی
 * جدایند و مرج دیگر تضادی ندارد.
 */
class DownboltFileProvider : FileProvider()
