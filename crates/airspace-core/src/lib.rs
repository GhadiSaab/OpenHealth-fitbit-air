//! # airspace-core
//!
//! Platform-independent logic for Airspace, a private, local-first fitness
//! companion. Compiles to WebAssembly (PWA / Android WebView UI) and to
//! native code (the Tauri shell). Contains no I/O and no UI.

pub mod aggregate;
pub mod ai;
pub mod backup;
pub mod bluetooth;
pub mod catalog;
pub mod dedupe;
pub mod demo;
pub mod import;
pub mod insights;
pub mod journal;
pub mod live_ingest;
pub mod model;
pub mod providers;
pub mod series;
pub mod settings;
pub mod time;

pub use model::*;

/// Current schema version of the local database (both IndexedDB and SQLite).
pub const DB_SCHEMA_VERSION: u32 = 1;
