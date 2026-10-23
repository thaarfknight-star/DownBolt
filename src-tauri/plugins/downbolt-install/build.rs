const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.install";

fn main() {
    tauri_plugin::Builder::new(&["install_apk"])
        .android_path("android")
        .build();
}
