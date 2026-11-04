//! App shell: boot, routing, theming and global overlays.

use crate::platform::{self, ls_get, ls_set, now_ms};
use crate::screens::{coach, explore, journal, onboarding, profile, today};
use crate::state::{App, LS_DAY, LS_ROUTE, Route};
use crate::ui::widgets::{BusyOverlay, SheetHost, TabBar, ToastHost};
use airspace_core::settings::{MotionPref, Theme};
use airspace_core::time;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, closure::Closure};

fn listen(target: &web_sys::EventTarget, event: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

#[component]
pub fn Shell() -> impl IntoView {
    let app = App::new();
    provide_context(app);
    let win = platform::window();

    // Theme and motion preferences.
    Effect::new(move |_| {
        let s = app.settings.get();
        let dark = match s.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => platform::media_matches("(prefers-color-scheme: dark)"),
        };
        platform::set_root_attr("data-theme", if dark { "dark" } else { "light" });
        platform::set_theme_color(if dark { "#121314" } else { "#F6F3EC" });
        let reduced = s.motion == MotionPref::Reduced
            || platform::media_matches("(prefers-reduced-motion: reduce)");
        platform::set_root_attr("data-motion", if reduced { "reduced" } else { "full" });
    });

    // Routing: keep the URL hash in sync so Back works (browser and Android).
    Effect::new(move |_| {
        let r = app.route.get();
        let h = r.to_hash();
        let loc = platform::window().location();
        if loc.hash().unwrap_or_default() != h {
            let _ = loc.set_hash(&h);
        }
        if r != Route::Onboarding {
            ls_set(LS_ROUTE, &h);
        }
        platform::scroll_top();
        // Move focus to the new screen's heading for screen-reader users.
        set_timeout(
            || platform::focus_by_id("screen-title"),
            std::time::Duration::from_millis(30),
        );
    });
    listen(win.as_ref(), "hashchange", move |_| {
        if let Some(r) = Route::from_hash(&platform::window().location().hash().unwrap_or_default())
        {
            if r != app.route.get_untracked() {
                app.route.set(r);
            }
        }
    });
    Effect::new(move |_| ls_set(LS_DAY, &time::day_key(app.day.get())));

    // Clock, connectivity and viewport.
    {
        let tick = Closure::<dyn FnMut()>::new(move || app.now.set(now_ms()));
        let _ = win.set_interval_with_callback_and_timeout_and_arguments_0(
            tick.as_ref().unchecked_ref(),
            30_000,
        );
        tick.forget();
    }
    listen(win.as_ref(), "online", move |_| app.online.set(true));
    listen(win.as_ref(), "offline", move |_| app.online.set(false));
    listen(win.as_ref(), "resize", move |_| {
        app.vw.set(
            platform::window()
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(390.0),
        );
    });
    if let Ok(mq) = win.match_media("(prefers-color-scheme: dark)") {
        if let Some(mq) = mq {
            listen(mq.as_ref(), "change", move |_| app.settings.update(|_| {}));
        }
    }

    // Snapshot polling is presentation only; native recording survives this WebView.
    if platform::is_android() {
        let tick = Closure::<dyn FnMut()>::new(move || {
            spawn_local(async move {
                app.refresh_live().await;
            });
        });
        if let Ok(id) = win.set_interval_with_callback_and_timeout_and_arguments_0(
            tick.as_ref().unchecked_ref(),
            3000,
        ) {
            let tick = StoredValue::new_local(tick);
            on_cleanup(move || {
                platform::window().clear_interval_with_handle(id);
                tick.dispose();
            });
        }
    }
    // Boot.
    spawn_local(async move {
        if let Err(e) = app.load().await {
            app.error(format!("Couldn't open local storage: {e}"));
        }
        if crate::platform::is_tauri() {
            if let Ok(has) = crate::platform::invoke::<bool>("ai_key_status", ()).await {
                app.ai_native_key.set(has);
            }
        }
        // Keep the demo current if it is the active database.
        if app.db.get_untracked() == airspace_core::DbKind::Demo {
            if let Err(e) = app.open_demo().await {
                app.error(e);
            }
        }
        let start = if !app.settings.with_untracked(|s| s.onboarded) {
            Route::Onboarding
        } else {
            let h = platform::window().location().hash().unwrap_or_default();
            Route::from_hash(&h)
                .or_else(|| ls_get(LS_ROUTE).and_then(|h| Route::from_hash(&h)))
                .filter(|r| *r != Route::Onboarding)
                .unwrap_or(Route::Today)
        };
        app.route.set(start);
        app.ready.set(true);
    });
    platform::register_service_worker();

    let screen = move || {
        if !app.ready.get() {
            return view! { <div class="boot" aria-busy="true"><div class="spinner" aria-hidden="true"></div><p class="sr-only">"Loading"</p></div> }.into_any();
        }
        match app.route.get() {
            Route::Onboarding => view! { <onboarding::Onboarding /> }.into_any(),
            Route::Today => view! { <today::Today /> }.into_any(),
            Route::Explore => view! { <explore::Explore /> }.into_any(),
            Route::Metric(m, r) => view! { <explore::MetricDetail metric=m range=r /> }.into_any(),
            Route::Journal => view! { <journal::Journal /> }.into_any(),
            Route::Coach => view! { <coach::Coach /> }.into_any(),
            Route::AiSettings => view! { <coach::AiSettings /> }.into_any(),
            Route::Profile => view! { <profile::Profile /> }.into_any(),
            Route::Device => view! { <profile::Device /> }.into_any(),
            Route::Data => view! { <profile::DataScreen /> }.into_any(),
            Route::Import => view! { <profile::ImportScreen /> }.into_any(),
            Route::Settings => view! { <profile::SettingsScreen /> }.into_any(),
            Route::About => view! { <profile::About /> }.into_any(),
        }
    };
    let show_tabs = move || app.ready.get() && app.route.get() != Route::Onboarding;

    view! {
        <a class="skip-link" href="#main">"Skip to content"</a>
        <div class="app" class:with-tabs=show_tabs>
            {move || show_tabs().then(|| view! { <TabBar /> })}
            <main id="main" class="main">
                {move || (!app.online.get()).then(|| view! { <div class="offline-note" role="status">"Offline. Everything except the optional AI coach works without a connection."</div> })}
                {screen}
            </main>
        </div>
        <SheetHost />
        <ToastHost />
        <BusyOverlay />
    }
}
