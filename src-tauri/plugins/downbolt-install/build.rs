const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.install";

fn main() {
    tauri_plugin::Builder::new(&[
        "install_apk",
        "publish_to_downloads",
        "delete_published",
        "open_file",
        "share_file",
    ])
    .android_path("android")
    .build();
}
