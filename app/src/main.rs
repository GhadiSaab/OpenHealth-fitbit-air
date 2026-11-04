//! Airspace — a private, local-first fitness companion.

mod idb;
mod platform;
mod repo;
mod screens;
mod state;
mod ui;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(screens::shell::Shell);
}
