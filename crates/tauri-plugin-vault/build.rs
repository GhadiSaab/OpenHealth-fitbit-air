// No commands are exposed to the webview: the app's own Rust commands call
// this plugin's Rust API, so secrets never round-trip through JavaScript.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
