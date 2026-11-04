//! Provider boundaries and their honest status.
//!
//! | Provider                      | This build                                  |
//! |-------------------------------|---------------------------------------------|
//! | `DemoDataProvider`            | Working: deterministic simulated dataset    |
//! | `HistoryImportProvider`       | Working: JSON/CSV import with preview       |
//! | `BluetoothLiveProvider`       | Not connected (interface only)              |
//! | `OptionalHistorySyncProvider` | Not connected (Google sign-in deferred)     |
//! | `AIProvider`                  | Optional; works only with the user's key    |

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    DemoData,
    HistoryImport,
    BluetoothLive,
    OptionalHistorySync,
    Ai,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Ready to use without further setup.
    Available,
    /// Configured and reachable.
    Connected,
    /// Exists as an interface, but there is nothing to connect to yet.
    NotConnected { reason: String },
    /// Needs the user to configure something first.
    NeedsSetup { what: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub name: String,
    pub description: String,
    pub status: ProviderStatus,
}

/// Static descriptors; the UI overlays runtime state (e.g. AI configured).
pub fn descriptors(ai_configured: bool) -> Vec<ProviderDescriptor> {
    vec![
        ProviderDescriptor {
            id: ProviderId::DemoData,
            name: "Demo data".into(),
            description: "A simulated 30-day dataset kept in its own database.".into(),
            status: ProviderStatus::Available,
        },
        ProviderDescriptor {
            id: ProviderId::HistoryImport,
            name: "History import".into(),
            description: "Import JSON or CSV files in the Airspace import schema. Files stay on this device.".into(),
            status: ProviderStatus::Available,
        },
        ProviderDescriptor {
            id: ProviderId::BluetoothLive,
            name: "Bluetooth live recording".into(),
            description: "Live heart rate, steps, distance and calories from your tracker.".into(),
            status: ProviderStatus::NotConnected {
                reason: "Not included in this version. It will run as a native Android service; see the integration guide.".into(),
            },
        },
        ProviderDescriptor {
            id: ProviderId::OptionalHistorySync,
            name: "History sync".into(),
            description: "Optional download of history processed by Google's services.".into(),
            status: ProviderStatus::NotConnected { reason: "Google sign-in is not part of this version.".into() },
        },
        ProviderDescriptor {
            id: ProviderId::Ai,
            name: "AI coach".into(),
            description: "Optional. Uses your own API key with the provider you choose.".into(),
            status: if ai_configured {
                ProviderStatus::Connected
            } else {
                ProviderStatus::NeedsSetup { what: "Add a provider and API key in Coach settings.".into() }
            },
        },
    ]
}

/// The history sync provider in this build.
pub struct NotConnectedHistorySync;

impl NotConnectedHistorySync {
    pub fn status(&self) -> ProviderStatus {
        ProviderStatus::NotConnected { reason: "Google sign-in is not part of this version.".into() }
    }
}
