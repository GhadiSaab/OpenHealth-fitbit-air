//! Secret storage for Airspace.
//!
//! * **Android**: values are encrypted with AES-256-GCM using a key that
//!   lives in the Android Keystore (non-exportable), and the ciphertext is
//!   kept in app-private SharedPreferences. See `android/src/main/java/VaultPlugin.kt`.
//! * **Desktop** (development builds only): memory only, never written to disk.
//!
//! Values are never logged.

use serde::{Deserialize, Serialize};
use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("secure storage is unavailable: {0}")]
    Unavailable(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg_attr(desktop, allow(dead_code))]
#[derive(Serialize)]
struct SetPayload<'a> {
    key: &'a str,
    value: &'a str,
}

#[cfg_attr(desktop, allow(dead_code))]
#[derive(Serialize)]
struct KeyPayload<'a> {
    key: &'a str,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GetResponse {
    found: bool,
    #[serde(default)]
    value: String,
}

#[cfg(mobile)]
mod imp {
    use super::*;
    use tauri::plugin::{PluginApi, PluginHandle};

    pub struct Vault<R: Runtime>(PluginHandle<R>);

    pub fn init<R: Runtime, C: serde::de::DeserializeOwned>(api: PluginApi<R, C>) -> std::result::Result<Vault<R>, Box<dyn std::error::Error>> {
        #[cfg(target_os = "android")]
        let handle = api.register_android_plugin("app.airspace.vault", "VaultPlugin")?;
        #[cfg(target_os = "ios")]
        let handle: PluginHandle<R> = return Err("iOS is not supported".into());
        Ok(Vault(handle))
    }

    impl<R: Runtime> Vault<R> {
        pub fn set(&self, key: &str, value: &str) -> Result<()> {
            self.0.run_mobile_plugin::<serde_json::Value>("set", SetPayload { key, value }).map(|_| ()).map_err(|_| Error::Unavailable("could not store the value".into()))
        }
        pub fn get(&self, key: &str) -> Result<Option<String>> {
            let r: GetResponse = self.0.run_mobile_plugin("get", KeyPayload { key }).map_err(|_| Error::Unavailable("could not read the value".into()))?;
            Ok(r.found.then_some(r.value))
        }
        pub fn delete(&self, key: &str) -> Result<()> {
            self.0.run_mobile_plugin::<serde_json::Value>("delete", KeyPayload { key }).map(|_| ()).map_err(|_| Error::Unavailable("could not delete the value".into()))
        }
        pub fn is_persistent(&self) -> bool {
            true
        }
    }
}

#[cfg(desktop)]
mod imp {
    use super::*;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::sync::Mutex;

    pub struct Vault<R: Runtime>(Mutex<HashMap<String, String>>, PhantomData<fn() -> R>);

    pub fn init<R: Runtime>() -> Vault<R> {
        Vault(Mutex::new(HashMap::new()), PhantomData)
    }

    impl<R: Runtime> Vault<R> {
        pub fn set(&self, key: &str, value: &str) -> Result<()> {
            self.0.lock().map_err(|_| Error::Unavailable("lock".into()))?.insert(key.into(), value.into());
            Ok(())
        }
        pub fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.0.lock().map_err(|_| Error::Unavailable("lock".into()))?.get(key).cloned())
        }
        pub fn delete(&self, key: &str) -> Result<()> {
            self.0.lock().map_err(|_| Error::Unavailable("lock".into()))?.remove(key);
            Ok(())
        }
        pub fn is_persistent(&self) -> bool {
            false
        }
    }
}

pub use imp::Vault;

pub trait VaultExt<R: Runtime> {
    fn vault(&self) -> &Vault<R>;
}

impl<R: Runtime, T: Manager<R>> VaultExt<R> for T {
    fn vault(&self) -> &Vault<R> {
        self.state::<Vault<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("vault")
        .setup(|app, _api| {
            #[cfg(mobile)]
            let v = imp::init(_api)?;
            #[cfg(desktop)]
            let v = imp::init::<R>();
            app.manage(v);
            Ok(())
        })
        .build()
}
