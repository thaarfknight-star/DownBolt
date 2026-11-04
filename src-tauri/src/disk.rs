//! فضای دیسکِ مسیر دانلود — برای پنجره‌ی تأیید پیش از دانلود (۰٫۶٫۹).
//!
//! فقط خواندنِ وضعیتِ فایل‌سیستم است و به Tauri وابسته نیست تا روی هر
//! سکوی بیلد (ویندوز/اندروید) کامپایل شود. مسیرِ هنوز ساخته‌نشده با
//! بالا رفتن تا نزدیک‌ترین جدِ موجود، به حجمِ همان درایو می‌رسد.

use std::path::Path;

/// (فضای خالی، حجم کل) به بایت برای درایوِ دربردارنده‌ی `dir`؛
/// هر کدام که به دست نیاید None است (دیالوگ «نامشخص» نشان می‌دهد).
pub fn free_space(dir: &Path) -> (Option<u64>, Option<u64>) {
    let mut cur = Some(dir);
    let existing = loop {
        match cur {
            None => break None,
            Some(p) if p.exists() => break Some(p),
            Some(p) => cur = p.parent(),
        }
    };
    let path = match existing {
        Some(p) => p,
        None => return (None, None),
    };
    #[cfg(unix)]
    {
        return free_space_unix(path);
    }
    #[cfg(windows)]
    {
        return free_space_windows(path);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        (None, None)
    }
}

#[cfg(unix)]
fn free_space_unix(path: &Path) -> (Option<u64>, Option<u64>) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let cstr = match CString::new(path.as_os_str().as_bytes()) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(cstr.as_ptr(), &mut vfs) } != 0 {
        return (None, None);
    }
    let block = vfs.f_frsize as u64;
    // f_bavail یعنی فضای در دسترسِ کاربرِ عادی — همان عددی که برای
    // «جا می‌شود یا نه» درست است، نه f_bfree (شامل رزروِ ریشه).
    let free = (vfs.f_bavail as u64).saturating_mul(block);
    let total = (vfs.f_blocks as u64).saturating_mul(block);
    (Some(free), Some(total))
}

#[cfg(windows)]
fn free_space_windows(path: &Path) -> (Option<u64>, Option<u64>) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut free: u64 = 0;
    let mut total: u64 = 0;
    // در windows-sys 0.59، ‏PCWSTR فقط یک نام مستعار برای *const u16 است
    // (سازنده‌ی تاپلی ندارد)؛ اشاره‌گر خام مستقیم همان نوع است.
    let ok: BOOL =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
    if ok == 0 {
        (None, None)
    } else {
        (Some(free), Some(total))
    }
}
