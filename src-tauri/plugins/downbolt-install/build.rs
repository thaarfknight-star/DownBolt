const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.install";

fn main() {
    tauri_plugin::Builder::new(&[
        "install_apk",
        "publish_to_downloads",
        "delete_published",
        "open_file",
        "share_file",
        "pick_folder",
        "copy_to_folder",
        "delete_document",
    ])
    .android_path("android")
    .build();
}
