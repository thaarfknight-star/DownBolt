// جلوگیری از باز شدن کنسول کنار پنجره‌ی برنامه در بیلد ریلیز ویندوز
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    download_manager_lib::run();
}
