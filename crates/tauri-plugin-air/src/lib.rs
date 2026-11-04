use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};
#[cfg(target_os = "android")]
pub struct Air<R: Runtime>(tauri::plugin::PluginHandle<R>);
#[cfg(not(target_os = "android"))]
pub struct Air<R: Runtime>(std::marker::PhantomData<fn() -> R>);
impl<R: Runtime> Air<R> {
    pub fn call(
        &self,
        method: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        #[cfg(target_os = "android")]
        {
            self.0.run_mobile_plugin(method, payload).map_err(|_| {
                "Air connection command failed. Check the device screen and permissions.".into()
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (method, payload);
            Err("Live recording is available in the Android APK.".into())
        }
    }
}
pub trait AirExt<R: Runtime> {
    fn air(&self) -> &Air<R>;
}
impl<R: Runtime, T: Manager<R>> AirExt<R> for T {
    fn air(&self) -> &Air<R> {
        self.state::<Air<R>>().inner()
    }
}
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("air")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let air = Air(_api.register_android_plugin("app.airspace.air", "AirPlugin")?);
            #[cfg(not(target_os = "android"))]
            let air = Air(std::marker::PhantomData);
            app.manage(air);
            Ok(())
        })
        .build()
}
