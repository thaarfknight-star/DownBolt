const PLUGIN_IDENTIFIER: &str = "ir.downbolt.plugin.foreground";

fn main() {
    tauri_plugin::Builder::new(&["sync_notification", "stop_notification"])
        .android_path("android")
        .build();
}
