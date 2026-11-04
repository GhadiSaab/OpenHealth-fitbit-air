//! Journal: notes and tags per day, goals, reminders, and cautious
//! comparisons between tags and recorded metrics.

use crate::platform;
use crate::state::Route;
use crate::ui::format::{self, day_short};
use crate::ui::icons::Icon;
use crate::ui::widgets::{DemoBanner, Header, Section, Switch, use_app};
use airspace_core::catalog;
use airspace_core::journal::{ASSOCIATION_METRICS, JournalEntry, SUGGESTED_TAGS, associate, normalize_tag, tag_counts};
use airspace_core::time;
use airspace_core::{Metric, Unit};
use chrono::{Datelike, Duration};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn Journal() -> impl IntoView {
    view! {
        <Header title="Journal" />
        <DemoBanner />
        <Calendar />
        <DayEditor />
        <Goals />
        <Reminders />
        <Associations />
    }
}

#[component]
fn Calendar() -> impl IntoView {
    let app = use_app();
    let month = RwSignal::new(time::month_start(app.day.get_untracked()));
    Effect::new(move |_| {
        let d = app.day.get();
        if time::month_start(d) != month.get_untracked() {
            month.set(time::month_start(d));
        }
    });
    view! {
        <section class="section calendar" aria-label="Calendar">
            <div class="cal-head">
                <button class="icon-btn" aria-label="Previous month" on:click=move |_| month.update(|m| *m = time::add_months(*m, -1))><Icon name="left" /></button>
                <h2 class="tabular" aria-live="polite">{move || month.get().format("%B %Y").to_string()}</h2>
                <button class="icon-btn" aria-label="Next month" disabled={move || month.get() >= time::month_start(app.today())} on:click=move |_| month.update(|m| *m = time::add_months(*m, 1))><Icon name="right" /></button>
            </div>
            <div class="cal-grid" role="grid">
                <div class="cal-row" role="row">
                    {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].into_iter().map(|d| view! { <span class="cal-dow" role="columnheader" aria-label=d>{&d[..1]}</span> }).collect_view()}
                </div>
                {move || {
                    let m = month.get();
                    let first = time::week_start(m);
                    let today = app.today();
                    let goal = app.settings.with(|s| s.step_goal as f64);
                    let weeks = (0..6).map(|w| first + Duration::days(7 * w)).filter(|w| w.month() == m.month() || (*w + Duration::days(6)).month() == m.month()).collect::<Vec<_>>();
                    weeks.into_iter().map(|w| view! {
                        <div class="cal-row" role="row">
                            {(0..7).map(|i| {
                                let d = w + Duration::days(i);
                                let in_month = d.month() == m.month();
                                let future = d > today;
                                let entry = app.journal.with(|j| j.iter().find(|e| e.day == time::day_key(d)).cloned());
                                let steps = app.summaries.with(|s| s.value(Metric::Steps, d));
                                let mut label = d.format("%A %-d %B").to_string();
                                if let Some(e) = &entry {
                                    if !e.note.is_empty() { label.push_str(", has a note"); }
                                    if !e.tags.is_empty() { label.push_str(&format!(", tags: {}", e.tags.join(", "))); }
                                }
                                match steps {
                                    Some(v) if v >= goal => label.push_str(", step goal reached"),
                                    Some(_) => label.push_str(", steps recorded"),
                                    None => {}
                                }
                                let selected = move || app.day.get() == d;
                                view! {
                                    <button
                                        role="gridcell"
                                        class="cal-day"
                                        class:out=!in_month
                                        class:sel=selected
                                        class:today=d == today
                                        disabled=future
                                        aria-selected=move || if selected() { "true" } else { "false" }
                                        aria-label=label
                                        on:click=move |_| app.day.set(d)
                                    >
                                        <span class="num tabular">{d.day()}</span>
                                        <span class="marks" aria-hidden="true">
                                            {match steps { Some(v) if v >= goal => Some(view! { <span class="mk goal"></span> }.into_any()), Some(_) => Some(view! { <span class="mk data"></span> }.into_any()), None => None }}
                                            {entry.map(|_| view! { <span class="mk note"></span> })}
                                        </span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    }).collect_view()
                }}
            </div>
            <p class="cal-legend muted"><span class="mk goal"></span>" step goal reached "<span class="mk data"></span>" steps recorded "<span class="mk note"></span>" journal entry"</p>
        </section>
    }
}

#[component]
fn DayEditor() -> impl IntoView {
    let app = use_app();
    let draft = RwSignal::new(JournalEntry::new(app.day.get_untracked(), platform::now_ms()));
    let dirty = RwSignal::new(false);
    let saving = RwSignal::new(false);
    let custom = RwSignal::new(String::new());
    Effect::new(move |_| {
        let d = app.day.get();
        // Changing day with unsaved edits saves them rather than dropping them.
        if dirty.get_untracked() {
            let prev = draft.get_untracked();
            spawn_local(async move {
                match app.save_journal(prev.clone()).await {
                    Ok(()) => app.notify(format!("Saved your entry for {}", time::parse_day(&prev.day).map(day_short).unwrap_or(prev.day.clone()))),
                    Err(e) => app.error(format!("Couldn't save: {e}")),
                }
            });
        }
        let e = app.journal.with_untracked(|j| j.iter().find(|e| e.day == time::day_key(d)).cloned()).unwrap_or_else(|| JournalEntry::new(d, platform::now_ms()));
        draft.set(e);
        dirty.set(false);
    });
    let all_tags = move || {
        let mut t: Vec<String> = SUGGESTED_TAGS.iter().map(|s| s.to_string()).collect();
        for (tag, _) in app.journal.with(|j| tag_counts(j)) {
            if !t.contains(&tag) {
                t.push(tag);
            }
        }
        for tag in draft.with(|d| d.tags.clone()) {
            if !t.contains(&tag) {
                t.push(tag);
            }
        }
        t
    };
    let save = move || {
        saving.set(true);
        let e = draft.get_untracked();
        spawn_local(async move {
            match app.save_journal(e).await {
                Ok(()) => {
                    dirty.set(false);
                    app.haptic();
                    app.notify("Journal saved");
                }
                Err(err) => app.error(format!("Couldn't save: {err}")),
            }
            saving.set(false);
        });
    };
    let add_custom = move || {
        if let Some(t) = normalize_tag(&custom.get_untracked()) {
            draft.update(|d| if !d.tags.contains(&t) { d.tags.push(t) });
            dirty.set(true);
        }
        custom.set(String::new());
    };
    view! {
        <Section title="Entry">
            <p class="editor-day">{move || app.day.get().format("%A, %-d %B %Y").to_string()}</p>
            <DayFacts />
            <label class="field">
                <span class="field-label">"Note"</span>
                <textarea
                    rows="3"
                    maxlength="2000"
                    placeholder="How was the day? Anything worth remembering?"
                    prop:value=move || draft.with(|d| d.note.clone())
                    on:input=move |ev| { draft.update(|d| d.note = event_target_value(&ev)); dirty.set(true); }
                ></textarea>
            </label>
            <fieldset class="tags">
                <legend class="field-label">"Tags"</legend>
                <div class="chip-wrap">
                    {move || all_tags().into_iter().map(|t| {
                        let t2 = t.clone();
                        let on = move || draft.with(|d| d.tags.contains(&t2));
                        let t3 = t.clone();
                        view! {
                            <button class="chip toggle" aria-pressed=move || if on() { "true" } else { "false" } on:click=move |_| {
                                draft.update(|d| if d.tags.contains(&t3) { d.tags.retain(|x| x != &t3) } else { d.tags.push(t3.clone()) });
                                dirty.set(true);
                            }>{t}</button>
                        }
                    }).collect_view()}
                </div>
                <div class="inline-add">
                    <label class="sr-only" for="custom-tag">"New tag"</label>
                    <input id="custom-tag" type="text" maxlength="32" placeholder="Add a tag" prop:value=move || custom.get()
                        on:input=move |ev| custom.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| { if ev.key() == "Enter" { ev.prevent_default(); add_custom() } } />
                    <button class="btn small" on:click=move |_| add_custom() disabled=move || custom.get().trim().is_empty()><Icon name="plus" size=18 />"Add"</button>
                </div>
            </fieldset>
            <div class="btn-row">
                <button class="btn primary" disabled=move || !dirty.get() || saving.get() on:click=move |_| save()>
                    {move || if saving.get() { "Saving…" } else if dirty.get() { "Save entry" } else { "Saved" }}
                </button>
                {move || draft.with(|d| !d.is_empty()).then(|| view! {
                    <button class="btn quiet" on:click=move |_| { draft.update(|d| { d.note.clear(); d.tags.clear(); }); dirty.set(true); save(); }>"Clear entry"</button>
                })}
            </div>
            <p class="fineprint">"Notes stay on this device. They're only shared with an AI provider if you explicitly include them."</p>
        </Section>
    }
}

#[component]
fn DayFacts() -> impl IntoView {
    let app = use_app();
    move || {
        let d = app.day.get();
        let s = app.settings.get();
        let facts: Vec<String> = app.summaries.with(|i| {
            [Metric::Steps, Metric::Sleep, Metric::RestingHeartRate, Metric::Hrv]
                .into_iter()
                .filter_map(|m| i.value(m, d).map(|v| format!("{}: {}", catalog::info(m).short, if m == Metric::Sleep { time::fmt_minutes(v) } else { format::value(m, v, &s) })))
                .collect()
        });
        if facts.is_empty() {
            view! { <p class="muted">"No measurements recorded for this day."</p> }.into_any()
        } else {
            view! { <p class="facts-line tabular">{facts.join(" · ")}</p> }.into_any()
        }
    }
}

#[component]
fn Goals() -> impl IntoView {
    let app = use_app();
    let set_steps = move |v: u32| app.update_settings(|s| s.step_goal = v.clamp(500, 50_000));
    view! {
        <Section title="Goals">
            <div class="row">
                <div class="row-text"><span class="row-title">"Daily steps"</span><span class="row-hint">"Shown on Today and in Explore."</span></div>
                <div class="stepper" role="group" aria-label="Daily step goal">
                    <button class="icon-btn" aria-label="Decrease by 500" on:click=move |_| set_steps(app.settings.get_untracked().step_goal.saturating_sub(500))>"−"</button>
                    <input type="number" inputmode="numeric" min="500" max="50000" step="500" aria-label="Steps"
                        prop:value=move || app.settings.with(|s| s.step_goal.to_string())
                        on:change=move |ev| { if let Ok(v) = event_target_value(&ev).parse::<u32>() { set_steps(v) } } />
                    <button class="icon-btn" aria-label="Increase by 500" on:click=move |_| set_steps(app.settings.get_untracked().step_goal + 500)>"+"</button>
                </div>
            </div>
            <div class="row">
                <div class="row-text"><span class="row-title">"Sleep duration"</span><span class="row-hint">"Time asleep, not time in bed."</span></div>
                <select aria-label="Sleep goal" on:change=move |ev| { if let Ok(v) = event_target_value(&ev).parse::<u32>() { app.update_settings(|s| s.sleep_goal_min = v) } }>
                    {(0..=16).map(|i| {
                        let m = 300 + i * 15;
                        view! { <option value=m.to_string() selected=move || app.settings.with(|s| s.sleep_goal_min == m)>{time::fmt_minutes(m as f64)}</option> }
                    }).collect_view()}
                </select>
            </div>
        </Section>
    }
}

#[component]
fn Reminders() -> impl IntoView {
    let app = use_app();
    let native = platform::is_android();
    let apply = move |enabled: bool, at: Option<String>| {
        app.update_settings(|s| {
            s.reminders.enabled = enabled;
            if at.is_some() {
                s.reminders.journal_time = at.clone();
            }
        });
        let time = app.settings.with_untracked(|s| s.reminders.journal_time.clone()).unwrap_or_else(|| "21:00".into());
        spawn_local(async move {
            let r: Result<(), String> = platform::invoke("reminders_set", serde_json::json!({"enabled": enabled, "time": time})).await;
            match r {
                Ok(()) => app.notify(if enabled { format!("Reminder set for {time} each day") } else { "Reminders turned off".into() }),
                Err(e) => {
                    app.update_settings(|s| s.reminders.enabled = false);
                    app.error(e);
                }
            }
        });
    };
    view! {
        <Section title="Reminders">
            {if native {
                view! {
                    <Switch label="Evening journal reminder" hint="Off unless you turn it on. One quiet notification a day."
                        checked=Signal::derive(move || app.settings.with(|s| s.reminders.enabled))
                        on_change=Callback::new(move |v| apply(v, None)) />
                    {move || app.settings.with(|s| s.reminders.enabled).then(|| view! {
                        <label class="row">
                            <span class="row-title">"Time"</span>
                            <input type="time" prop:value=move || app.settings.with(|s| s.reminders.journal_time.clone().unwrap_or_default())
                                on:change=move |ev| apply(true, Some(event_target_value(&ev))) />
                        </label>
                    })}
                }.into_any()
            } else {
                view! {
                    <p class="muted">"Reminders are available in the Android app. Browsers can't reliably deliver scheduled notifications while a page is closed, so the web version doesn't offer them."</p>
                }.into_any()
            }}
        </Section>
    }
}

#[component]
fn Associations() -> impl IntoView {
    let app = use_app();
    view! {
        <Section title="Tags and your measurements">
            <p class="muted">"Compares days with a tag to days without it, in your own records. An association is not proof that one thing caused the other — many factors change together."</p>
            {move || {
                let today = app.today();
                let journal = app.journal.get();
                let s = app.settings.get();
                let from = app.summaries.with(|i| i.first_day()).unwrap_or(today).max(today - Duration::days(180));
                let tags: Vec<(String, usize)> = tag_counts(&journal).into_iter().take(6).collect();
                if tags.is_empty() {
                    return view! { <p>"Tag a few days to see comparisons here."</p> }.into_any();
                }
                let rows: Vec<_> = app.summaries.with(|idx| {
                    tags.iter().flat_map(|(t, _)| ASSOCIATION_METRICS.iter().filter_map(|(m, lag)| associate(&journal, idx, t, *m, *lag, from, today))).collect()
                });
                let insufficient: Vec<String> = tags.iter().filter(|(t, _)| !rows.iter().any(|a| &a.tag == t)).map(|(t, n)| format!("{t} ({n} day{})", if *n == 1 { "" } else { "s" })).collect();
                // Show the clearest differences; summarise the rest in one line.
                let small = |a: &airspace_core::journal::Association| {
                    let d = a.diff().abs();
                    match a.metric.canonical_unit() { Unit::Steps => d < 500.0, Unit::Minutes => d < 15.0, Unit::Bpm => d < 1.5, Unit::Milliseconds => d < 3.0, Unit::CelsiusDelta => d < 0.15, _ => true }
                };
                let rel = |a: &airspace_core::journal::Association| a.diff().abs() / a.other_mean.abs().max(1e-6);
                let (mut notable, same): (Vec<_>, Vec<_>) = rows.into_iter().partition(|a| !small(a));
                notable.sort_by(|a, b| rel(b).partial_cmp(&rel(a)).unwrap_or(std::cmp::Ordering::Equal));
                let hidden = notable.len().saturating_sub(5);
                notable.truncate(5);
                let same_txt: Vec<String> = same.iter().map(|a| format!("{} and {}", a.tag, catalog::info(a.metric).short.to_lowercase())).collect();
                view! {
                    {notable.is_empty().then(|| view! { <p>"No clear differences between tagged and untagged days yet."</p> })}
                    <ul class="assoc-list">
                        {notable.into_iter().map(|a| {
                            let fmtv = |v: f64| if a.metric == Metric::Sleep { time::fmt_minutes(v) } else { format::value(a.metric, v, &s) };
                            let name = catalog::info(a.metric).title;
                            let text = if a.lag_days == 0 {
                                format!("{name} averaged {} on days tagged “{}”, compared with {} on other days.", fmtv(a.tagged_mean), a.tag, fmtv(a.other_mean))
                            } else {
                                format!("{name} the following night or day averaged {} after days tagged “{}”, compared with {} after other days.", fmtv(a.tagged_mean), a.tag, fmtv(a.other_mean))
                            };
                            let m = a.metric;
                            view! {
                                <li>
                                    <p>{text}</p>
                                    <p class="muted tabular">{format!("{} tagged days vs {} other days, {} to {}.", a.tagged_n, a.other_n, day_short(a.window.0), day_short(a.window.1))}</p>
                                    <button class="link" on:click=move |_| app.go(Route::Metric(m, crate::state::Range::Month))>"See chart"</button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                    {(hidden > 0).then(|| view! { <p class="muted">{format!("{hidden} smaller differences not shown.")}</p> })}
                    {(!same_txt.is_empty()).then(|| view! { <p class="muted">{format!("About the same on tagged and untagged days: {}.", same_txt.join("; "))}</p> })}
                    {(!insufficient.is_empty()).then(|| view! {
                        <p class="muted">{format!("Not enough data yet for: {}. Comparisons need at least 3 tagged days and 5 other days with measurements.", insufficient.join(", "))}</p>
                    })}
                }.into_any()
            }}
        </Section>
    }
}
