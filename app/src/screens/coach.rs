//! Coach: local insights (always available) and an optional AI coach that
//! uses the user's own provider and key, with explicit consent per request.

use crate::platform::{self, is_tauri};
use crate::state::{LS_AI, Route};
use crate::ui::format::day_short;
use crate::ui::icons::{Icon, metric_icon};
use crate::ui::widgets::{DemoBanner, Header, Section, use_app};
use airspace_core::ai::{self, AiConfig, AiProviderKind, ChatTurn, ConsentSelection, HttpSpec};
use airspace_core::catalog;
use airspace_core::insights::{self, Direction};
use airspace_core::time;
use airspace_core::Metric;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, Debug, PartialEq)]
struct Turn {
    question: String,
    answer: Option<Result<String, String>>,
    window: (String, String),
    shared: Vec<String>,
    provider: String,
}

#[component]
pub fn Coach() -> impl IntoView {
    view! {
        <Header title="Coach" />
        <DemoBanner />
        <LocalInsights />
        <AiCoach />
    }
}

#[component]
fn LocalInsights() -> impl IntoView {
    let app = use_app();
    view! {
        <Section title="Insights">
            <p class="muted">"Calculated on this device from your records. No AI and no network involved."</p>
            {move || {
                let today = app.today();
                let tz = app.tz();
                let s = app.settings.get();
                let list = app.summaries.with(|i| insights::generate(i, today, &tz, &s));
                if list.is_empty() {
                    return view! { <p>"Insights appear once there's at least a week of data. Each one needs enough recorded days to be meaningful."</p> }.into_any();
                }
                view! {
                    <ul class="insight-list">
                        {list.into_iter().map(|ins| {
                            let m = ins.metric;
                            let arrow = match ins.direction { Direction::Up => "↑", Direction::Down => "↓", Direction::Steady => "→", Direction::Info => "" };
                            let tone = catalog::info(m).tone.css();
                            view! {
                                <li class=format!("insight tone-{tone}")>
                                    <div class="insight-head">
                                        <span class="metric-icon"><Icon name=metric_icon(m) size=18 /></span>
                                        <h3>{ins.title.clone()}</h3>
                                        {(!arrow.is_empty()).then(|| view! { <span class="trend-arrow" aria-hidden="true">{arrow}</span> })}
                                    </div>
                                    <p>{ins.body.clone()}</p>
                                    <p class="muted tabular">
                                        {format!("Window {}–{}: {} of {} days recorded.", short(&ins.window.from), short(&ins.window.to), ins.window.days_with_data, ins.window.days)}
                                        {ins.baseline.as_ref().map(|b| format!(" Compared with {}–{}: {} of {} days.", short(&b.from), short(&b.to), b.days_with_data, b.days))}
                                    </p>
                                    <button class="link" on:click=move |_| app.go(Route::Metric(m, crate::state::Range::Month))>"See chart"</button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
            <p class="fineprint">"Airspace doesn't calculate readiness, sleep or recovery scores. Those imply a validated model; these are plain comparisons with your own history."</p>
        </Section>
    }
}

fn short(day: &str) -> String {
    time::parse_day(day).map(day_short).unwrap_or_else(|| day.to_string())
}

fn key_available(app: &crate::state::App) -> bool {
    if is_tauri() { app.ai_native_key.get() } else { app.ai_key_mem.get().is_some() }
}

#[component]
fn AiCoach() -> impl IntoView {
    let app = use_app();
    let turns = RwSignal::new(Vec::<Turn>::new());
    let pending = RwSignal::new(None::<String>);
    let question = RwSignal::new(String::new());
    view! {
        <Section title="AI coach (optional)">
            {move || match app.ai.get() {
                None => view! {
                    <p>"Connect your own AI provider to ask questions about your trends or get a weekly review. It's entirely optional; everything else works without it."</p>
                    <ul class="plain checklist">
                        <li>"You bring your own API key. Airspace includes no key and makes no hidden AI calls."</li>
                        <li>"Before anything is sent, you see exactly which dates and measurements will be shared."</li>
                        <li>"Only compact daily summaries are sent, never raw samples."</li>
                    </ul>
                    <button class="btn primary" on:click=move |_| app.go(Route::AiSettings)>"Set up AI coach"</button>
                }.into_any(),
                Some(cfg) => {
                    let ready = key_available(&app);
                    view! {
                        <div class="ai-status">
                            <span>{format!("{} · {}", cfg.provider.label(), cfg.model)}</span>
                            <button class="link" on:click=move |_| app.go(Route::AiSettings)>"Settings"</button>
                        </div>
                        {(!ready).then(|| view! {
                            <p class="notice">{if is_tauri() { "Add your API key in settings to use the coach." } else { "Enter your API key in settings. In the browser it's kept in memory only, so it's needed again after reloading." }}</p>
                        })}
                        <div class="quick-actions">
                            <button class="chip" disabled=!ready on:click=move |_| pending.set(Some(ai::WEEKLY_REVIEW_PROMPT.to_string()))>"Weekly review"</button>
                            <button class="chip" disabled=!ready on:click=move |_| pending.set(Some("Explain what my resting heart rate and HRV measurements mean, and how mine have looked over this period.".into()))>"Explain my measurements"</button>
                            <button class="chip" disabled=!ready on:click=move |_| pending.set(Some("Suggest one or two modest, specific habits for sleep or activity based on this data.".into()))>"Suggest a habit"</button>
                        </div>
                        <form class="ask" on:submit=move |ev| { ev.prevent_default(); let q = question.get_untracked(); if !q.trim().is_empty() { pending.set(Some(q)); } }>
                            <label class="sr-only" for="ask">"Your question"</label>
                            <textarea id="ask" rows="2" maxlength="1000" placeholder="Ask about your recorded trends…" disabled=!ready
                                prop:value=move || question.get() on:input=move |ev| question.set(event_target_value(&ev))></textarea>
                            <button class="btn primary" type="submit" disabled=move || !ready || question.get().trim().is_empty()><Icon name="send" size=18 />"Review & send"</button>
                        </form>
                    }.into_any()
                }
            }}
            {move || pending.get().map(|q| view! { <Consent question=q pending=pending turns=turns on_sent=Callback::new(move |_| question.set(String::new())) /> })}
            <Conversation turns=turns />
        </Section>
    }
}

#[component]
fn Consent(question: String, pending: RwSignal<Option<String>>, turns: RwSignal<Vec<Turn>>, on_sent: Callback<()>) -> impl IntoView {
    let app = use_app();
    let today = app.today_untracked();
    let (f, t) = ai::default_window(today);
    let from = RwSignal::new(time::day_key(f));
    let to = RwSignal::new(time::day_key(t));
    let available: Vec<Metric> = app.summaries.with_untracked(|i| Metric::ALL.into_iter().filter(|m| i.has_metric(*m)).collect());
    let metrics = RwSignal::new(available.iter().copied().filter(|m| *m != Metric::HeartRateZones).collect::<Vec<_>>());
    let include_journal = RwSignal::new(false);
    let show_json = RwSignal::new(false);
    let sel = move || ConsentSelection { from: from.get(), to: to.get(), metrics: metrics.get(), include_journal: include_journal.get() };
    let ctx = Memo::new(move |_| {
        let s = sel();
        let settings = app.settings.get();
        app.summaries.with(|i| app.journal.with(|j| ai::build_context(i, j, &s, &settings)))
    });
    let q = question.clone();
    let send = move || {
        let c = ctx.get_untracked();
        let s = sel();
        let cfg = app.ai.get_untracked().unwrap();
        let mut history: Vec<ChatTurn> = vec![];
        for t in turns.get_untracked() {
            if let Some(Ok(a)) = &t.answer {
                history.push(ChatTurn { role: "user".into(), content: t.question.clone() });
                history.push(ChatTurn { role: "assistant".into(), content: a.clone() });
            }
        }
        history.push(ChatTurn { role: "user".into(), content: ai::user_message(&c, &q) });
        let idx = turns.with_untracked(|t| t.len());
        turns.update(|t| t.push(Turn { question: q.clone(), answer: None, window: (s.from.clone(), s.to.clone()), shared: c.lines.clone(), provider: format!("{} · {}", cfg.provider.label(), cfg.model) }));
        app.last_ai_context.set(Some(c.json.clone()));
        pending.set(None);
        on_sent.run(());
        spawn_local(async move {
            let r = send_chat(&app, &cfg, &history).await;
            turns.update(|t| if let Some(x) = t.get_mut(idx) { x.answer = Some(r) });
        });
    };
    view! {
        <div class="consent" role="region" aria-label="Review what will be shared">
            <h3>"Review before sending"</h3>
            <p>{move || format!("Your question and the data below will be sent to {}. That provider's terms and privacy policy apply to it.", app.ai.get().map(|c| c.provider.label()).unwrap_or_default())}</p>
            <blockquote class="q">{question.clone()}</blockquote>
            <div class="field-row">
                <label class="field"><span class="field-label">"From"</span>
                    <input type="date" prop:value=move || from.get() max=move || to.get() on:change=move |ev| from.set(event_target_value(&ev)) /></label>
                <label class="field"><span class="field-label">"To"</span>
                    <input type="date" prop:value=move || to.get() min=move || from.get() max=time::day_key(today) on:change=move |ev| to.set(event_target_value(&ev)) /></label>
            </div>
            <fieldset>
                <legend class="field-label">"Measurements to include"</legend>
                <div class="chip-wrap">
                    {available.into_iter().map(|m| {
                        let on = move || metrics.with(|v| v.contains(&m));
                        view! {
                            <button class="chip toggle" aria-pressed=move || if on() { "true" } else { "false" }
                                on:click=move |_| metrics.update(|v| if v.contains(&m) { v.retain(|x| *x != m) } else { v.push(m) })>
                                {catalog::info(m).short}
                            </button>
                        }
                    }).collect_view()}
                </div>
                <label class="check-row"><input type="checkbox" prop:checked=move || include_journal.get() on:change=move |_| include_journal.update(|v| *v = !*v) />
                    <span>"Include journal notes and tags"</span></label>
            </fieldset>
            <div class="shared-summary">
                <p class="field-label">"Will be shared"</p>
                {move || {
                    let c = ctx.get();
                    view! {
                        <ul class="plain">{c.lines.iter().map(|l| view! { <li>{l.clone()}</li> }).collect_view()}</ul>
                        <p class="muted tabular">{format!("{} days with data · {:.1} KB of daily summaries · no raw samples", c.days_with_data, c.bytes as f64 / 1024.0)}</p>
                    }
                }}
                <button class="link" aria-expanded=move || if show_json.get() { "true" } else { "false" } on:click=move |_| show_json.update(|v| *v = !*v)>
                    {move || if show_json.get() { "Hide exact data" } else { "Show exact data" }}
                </button>
                {move || show_json.get().then(|| view! { <pre class="json" tabindex="0">{pretty(&ctx.get().json)}</pre> })}
            </div>
            <div class="btn-row">
                <button class="btn primary" disabled=move || ctx.with(|c| c.days_with_data == 0) on:click=move |_| send()>"Share and ask"</button>
                <button class="btn quiet" on:click=move |_| pending.set(None)>"Cancel"</button>
            </div>
        </div>
    }
}

fn pretty(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| json.to_string())
}

async fn send_chat(app: &crate::state::App, cfg: &AiConfig, turns: &[ChatTurn]) -> Result<String, String> {
    if !platform::is_online() {
        return Err("You're offline. The AI coach needs a connection; everything else works offline.".into());
    }
    let adapter = ai::provider(cfg.provider);
    let spec = adapter.chat_request(cfg, ai::SYSTEM_PROMPT, turns, !is_tauri());
    let (status, body) = execute(app, cfg.provider, spec).await?;
    if (200..300).contains(&status) {
        adapter.parse_response(&body)
    } else {
        Err(ai::describe_http_error(status, &body))
    }
}

/// Sends a request. On Android the native layer attaches the key from the
/// vault; in the browser the in-memory key is attached here.
pub async fn execute(app: &crate::state::App, provider: AiProviderKind, spec: HttpSpec) -> Result<(u16, String), String> {
    if is_tauri() {
        let r: (u16, String) = platform::invoke("ai_request", serde_json::json!({"provider": provider, "spec": spec})).await?;
        Ok(r)
    } else {
        let key = app.ai_key_mem.get_untracked().ok_or("No API key in this session.")?;
        let mut headers = spec.headers.clone();
        headers.push(ai::provider(provider).auth_header(&key));
        platform::http(&spec.method, &spec.url, &headers, spec.body.as_deref()).await
    }
}

#[component]
fn Conversation(turns: RwSignal<Vec<Turn>>) -> impl IntoView {
    view! {
        {move || {
            let list = turns.get();
            (!list.is_empty()).then(|| view! {
                <ol class="conversation" aria-live="polite">
                    {list.into_iter().map(|t| view! {
                        <li class="turn">
                            <p class="you"><span class="who">"You"</span>{t.question.clone()}</p>
                            <div class="ai-answer">
                                <p class="ai-label"><Icon name="coach" size=16 />{format!("AI-generated · {}", t.provider)}</p>
                                {match t.answer.clone() {
                                    None => view! { <p class="muted" aria-busy="true">"Waiting for the provider…"</p> }.into_any(),
                                    Some(Ok(a)) => view! { <div class="ai-text">{a.split("\n\n").map(|p| view! { <p>{p.to_string()}</p> }).collect_view()}</div> }.into_any(),
                                    Some(Err(e)) => view! { <p class="error-text" role="alert">{e}</p> }.into_any(),
                                }}
                                <p class="ai-foot muted">{format!("Based on {} to {}: {}. AI can be wrong; it isn't medical advice.", short(&t.window.0), short(&t.window.1), t.shared.join(", "))}</p>
                            </div>
                        </li>
                    }).collect_view()}
                </ol>
                <button class="link" on:click=move |_| turns.set(vec![])>"Clear conversation"</button>
                <p class="fineprint">"Conversations are kept in memory only and disappear when you close the app."</p>
            })
        }}
    }
}

#[component]
pub fn AiSettings() -> impl IntoView {
    let app = use_app();
    let existing = app.ai.get_untracked();
    let provider = RwSignal::new(existing.as_ref().map(|c| c.provider).unwrap_or(AiProviderKind::Anthropic));
    let endpoint = RwSignal::new(existing.as_ref().map(|c| c.endpoint.clone()).unwrap_or_else(|| ai::ANTHROPIC_DEFAULT_ENDPOINT.into()));
    let model = RwSignal::new(existing.as_ref().map(|c| c.model.clone()).unwrap_or_else(|| ai::ANTHROPIC_DEFAULT_MODEL.into()));
    let key = RwSignal::new(String::new());
    let status = RwSignal::new(None::<Result<String, String>>);
    let testing = RwSignal::new(false);
    let cfg = move || AiConfig { provider: provider.get(), endpoint: endpoint.get().trim().to_string(), model: model.get().trim().to_string() };
    let validation = move || cfg().validate().err();
    let has_key = move || key_available(&app);

    let save = move || {
        let c = cfg();
        if let Err(e) = c.validate() {
            status.set(Some(Err(e)));
            return;
        }
        let k = key.get_untracked();
        spawn_local(async move {
            if !k.trim().is_empty() {
                if is_tauri() {
                    if let Err(e) = platform::invoke::<()>("ai_key_set", serde_json::json!({"key": k.trim(), "endpoint": c.endpoint.clone()})).await {
                        status.set(Some(Err(format!("Couldn't store the key securely: {e}"))));
                        return;
                    }
                    app.ai_native_key.set(true);
                } else {
                    app.ai_key_mem.set(Some(k.trim().to_string()));
                }
            }
            else if is_tauri() && app.ai_native_key.get_untracked() {
                // A stored key stays bound to the endpoint it was entered for.
                let bound: Option<String> = platform::invoke("ai_key_endpoint", ()).await.unwrap_or(None);
                if bound.map(|b| b.trim().trim_end_matches('/') != c.endpoint.trim().trim_end_matches('/')).unwrap_or(false) {
                    status.set(Some(Err("Your stored key was saved for a different endpoint. Enter the key again to use it with this one.".into())));
                    return;
                }
            }
            key.set(String::new());
            platform::ls_set_json(LS_AI, &c);
            app.ai.set(Some(c));
            app.haptic();
            status.set(Some(Ok("Saved.".into())));
        });
    };
    let test = move || {
        let c = cfg();
        if let Err(e) = c.validate() {
            status.set(Some(Err(e)));
            return;
        }
        testing.set(true);
        let typed = key.get_untracked();
        spawn_local(async move {
            let r = if !is_tauri() && !typed.trim().is_empty() {
                let spec = ai::test_request(&c, true);
                let mut h = spec.headers.clone();
                h.push(ai::auth_header(c.provider, typed.trim()));
                platform::http(&spec.method, &spec.url, &h, None).await
            } else if is_tauri() && !typed.trim().is_empty() {
                Err("Save the key first, then test.".into())
            } else {
                execute(&app, c.provider, ai::test_request(&c, !is_tauri())).await
            };
            status.set(Some(match r {
                Ok((s, _)) if (200..300).contains(&s) => Ok("Connected. The provider accepted the key. No health data was sent.".into()),
                Ok((s, b)) => Err(ai::describe_http_error(s, &b)),
                Err(e) => Err(e),
            }));
            testing.set(false);
        });
    };
    let delete_key = move || {
        spawn_local(async move {
            if is_tauri() {
                let _ = platform::invoke::<()>("ai_key_delete", ()).await;
                app.ai_native_key.set(false);
            }
            app.ai_key_mem.set(None);
            status.set(Some(Ok("Key deleted.".into())));
        });
    };
    let disconnect = move || {
        delete_key();
        platform::ls_remove(LS_AI);
        app.ai.set(None);
        app.go(Route::Coach);
        app.notify("AI coach disconnected and key deleted.");
    };

    view! {
        <Header title="AI provider" back=true />
        <Section title="Provider">
            <div class="radio-cards" role="radiogroup" aria-label="Provider">
                {[(AiProviderKind::Anthropic, "Uses the Anthropic Messages API (documented adapter)."), (AiProviderKind::OpenAiCompatible, "For services implementing the OpenAI chat-completions format. Compatibility varies by service.")].into_iter().map(|(p, d)| {
                    let on = move || provider.get() == p;
                    view! {
                        <button role="radio" class="radio-card" aria-checked=move || if on() { "true" } else { "false" } tabindex=move || if on() { "0" } else { "-1" }
                            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                if matches!(ev.key().as_str(), "ArrowDown" | "ArrowUp" | "ArrowLeft" | "ArrowRight") {
                                    ev.prevent_default();
                                    let next = if provider.get_untracked() == AiProviderKind::Anthropic { AiProviderKind::OpenAiCompatible } else { AiProviderKind::Anthropic };
                                    provider.set(next);
                                    let def = AiConfig::default_for(next);
                                    endpoint.set(def.endpoint);
                                    model.set(def.model);
                                    set_timeout(|| platform::focus_selector(".radio-card[aria-checked=true]"), std::time::Duration::from_millis(20));
                                }
                            }
                            on:click=move |_| {
                            if provider.get_untracked() != p {
                                provider.set(p);
                                let def = AiConfig::default_for(p);
                                endpoint.set(def.endpoint);
                                model.set(def.model);
                            }
                        }>
                            <span class="row-title">{p.label()}</span>
                            <span class="row-hint">{d}</span>
                        </button>
                    }
                }).collect_view()}
            </div>
            <label class="field"><span class="field-label">"HTTPS endpoint"</span>
                <input type="url" inputmode="url" autocomplete="off" spellcheck="false" placeholder="https://api.example.com/v1"
                    prop:value=move || endpoint.get() on:input=move |ev| endpoint.set(event_target_value(&ev)) /></label>
            <label class="field"><span class="field-label">"Model"</span>
                <input type="text" autocomplete="off" spellcheck="false" list="model-suggestions" prop:value=move || model.get() on:input=move |ev| model.set(event_target_value(&ev)) /></label>
            <datalist id="model-suggestions">
                {move || (provider.get() == AiProviderKind::Anthropic).then(|| view! {
                    <option value="claude-opus-5-5"></option><option value="claude-sonnet-5-5"></option><option value="claude-haiku-5-5"></option>
                })}
            </datalist>
            <label class="field"><span class="field-label">"API key"</span>
                <input type="password" autocomplete="off" spellcheck="false" placeholder=move || if has_key() { "A key is stored — enter a new one to replace it" } else { "Paste your key" }
                    prop:value=move || key.get() on:input=move |ev| key.set(event_target_value(&ev)) /></label>
            <p class="fineprint">{if is_tauri() {
                "On Android the key is stored with Android Keystore-backed encryption and only used by the app's native networking. It is never included in backups, exports or logs."
            } else {
                "In the browser the key is kept in memory only for this session. It is never written to storage, included in backups, or cached by the service worker."
            }}</p>
            {move || validation().map(|e| view! { <p class="error-text" role="alert">{e}</p> })}
            <div class="btn-row">
                <button class="btn primary" disabled=move || validation().is_some() on:click=move |_| save()>"Save"</button>
                <button class="btn" disabled=move || validation().is_some() || testing.get() on:click=move |_| test()>{move || if testing.get() { "Testing…" } else { "Test connection" }}</button>
            </div>
            {move || status.get().map(|s| match s {
                Ok(m) => view! { <p class="ok-text" role="status">{m}</p> }.into_any(),
                Err(e) => view! { <p class="error-text" role="alert">{e}</p> }.into_any(),
            })}
        </Section>
        {(!is_tauri()).then(|| view! {
            <Section title="Using AI from a browser">
                <p class="muted">"Browsers only allow a request to a provider that explicitly permits it (CORS). Anthropic supports direct browser access; many OpenAI-compatible services don't. If a request is blocked, use the Android app, which sends requests natively. Airspace never routes your data through a proxy."</p>
            </Section>
        })}
        <Section title="Remove">
            <div class="btn-row">
                <button class="btn quiet" disabled=move || !has_key() on:click=move |_| delete_key()><Icon name="trash" size=18 />"Delete key"</button>
                <button class="btn danger" disabled=move || app.ai.get().is_none() on:click=move |_| disconnect()>"Disconnect AI coach"</button>
            </div>
        </Section>
        <p class="fineprint pad">{move || format!("Last data shared this session: {}", if app.last_ai_context.get().is_some() { "see the conversation in Coach" } else { "none" })}</p>
    }
}
