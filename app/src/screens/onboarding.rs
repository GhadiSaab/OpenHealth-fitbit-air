//! First run: explore the demo, import history, or set up the device later.

use crate::state::Route;
use crate::ui::icons::Icon;
use crate::ui::widgets::use_app;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn Onboarding() -> impl IntoView {
    let app = use_app();
    let finish = move |r: Route| {
        app.update_settings(|s| s.onboarded = true);
        app.go(r);
    };
    let demo = move || {
        spawn_local(async move {
            match app.open_demo().await {
                Ok(()) => finish(Route::Today),
                Err(e) => app.error(format!("Couldn't create the demo: {e}")),
            }
        });
    };
    let import = move || {
        spawn_local(async move {
            if let Err(e) = app.switch_db(airspace_core::DbKind::Personal).await {
                app.error(e);
            }
            finish(Route::Import);
        });
    };
    let later = move || {
        spawn_local(async move {
            let _ = app.switch_db(airspace_core::DbKind::Personal).await;
            finish(Route::Today);
        });
    };
    view! {
        <div class="onboarding">
            <div class="brand-mark" aria-hidden="true"><span></span></div>
            <h1 id="screen-title" tabindex="-1">"Airspace"</h1>
            <p class="lede">"Your fitness data, understandable and yours. Everything stays on this device — no account, no subscription, no cloud."</p>
            <div class="choice-list">
                <button class="choice" on:click=move |_| demo()>
                    <span class="choice-icon tone-sleep"><Icon name="explore" /></span>
                    <span class="choice-text"><span class="row-title">"Explore the demo"</span><span class="row-hint">"30 days of simulated data, kept in its own database."</span></span>
                    <Icon name="right" />
                </button>
                <button class="choice" on:click=move |_| import()>
                    <span class="choice-icon tone-activity"><Icon name="upload" /></span>
                    <span class="choice-text"><span class="row-title">"Import my history"</span><span class="row-hint">"Preview a JSON or CSV file before anything is saved."</span></span>
                    <Icon name="right" />
                </button>
                <button class="choice" on:click=move |_| later()>
                    <span class="choice-icon tone-heart"><Icon name="bluetooth" /></span>
                    <span class="choice-text"><span class="row-title">"Set up my device later"</span><span class="row-hint">"Start with an empty journal. Bluetooth recording comes in a later version."</span></span>
                    <Icon name="right" />
                </button>
            </div>
            <p class="fineprint">"Airspace is not a medical device. It shows what your tracker recorded and never diagnoses."</p>
        </div>
    }
}
