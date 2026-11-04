//! Application state (fine-grained signals) and the data pipeline.
//!
//! Raw measurements live only in the repository; the UI keeps the derived
//! daily summaries in memory and loads intraday samples on demand.

use crate::platform::{self, ls_get, ls_get_json, ls_set, ls_set_json, now_ms};
use crate::repo;
use airspace_core::aggregate::{self, DaySummary, SummaryContext, SummaryIndex};
use airspace_core::ai::AiConfig;
use airspace_core::backup::{self, Backup};
use airspace_core::demo::{self, DemoDevice};
use airspace_core::journal::JournalEntry;
use airspace_core::settings::Settings;
use airspace_core::time::{self, DAY_MS};
use airspace_core::{DbKind, ImportBatch, Measurement, Metric};
use chrono::NaiveDate;
use chrono_tz::Tz;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Range {
    Day,
    Week,
    Month,
    Year,
}

impl Range {
    pub fn key(self) -> &'static str {
        match self {
            Range::Day => "d",
            Range::Week => "w",
            Range::Month => "m",
            Range::Year => "y",
        }
    }
    pub fn from_key(s: &str) -> Option<Range> {
        Some(match s {
            "d" => Range::Day,
            "w" => Range::Week,
            "m" => Range::Month,
            "y" => Range::Year,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    Onboarding,
    Today,
    Explore,
    Metric(Metric, Range),
    Journal,
    Coach,
    Profile,
    Device,
    Data,
    Import,
    Settings,
    AiSettings,
    About,
}

impl Route {
    pub fn tab(self) -> Option<u8> {
        Some(match self {
            Route::Today => 0,
            Route::Explore | Route::Metric(..) => 1,
            Route::Journal => 2,
            Route::Coach | Route::AiSettings => 3,
            _ => return None,
        })
    }
    pub fn parent(self) -> Option<Route> {
        match self {
            Route::Metric(..) => Some(Route::Explore),
            Route::Device | Route::Data | Route::Settings | Route::About => Some(Route::Profile),
            Route::Import => Some(Route::Data),
            Route::AiSettings => Some(Route::Coach),
            Route::Profile => Some(Route::Today),
            _ => None,
        }
    }
    pub fn to_hash(self) -> String {
        match self {
            Route::Onboarding => "#/welcome".into(),
            Route::Today => "#/today".into(),
            Route::Explore => "#/explore".into(),
            Route::Metric(m, r) => format!("#/explore/{}/{}", m.key(), r.key()),
            Route::Journal => "#/journal".into(),
            Route::Coach => "#/coach".into(),
            Route::Profile => "#/profile".into(),
            Route::Device => "#/profile/device".into(),
            Route::Data => "#/profile/data".into(),
            Route::Import => "#/profile/data/import".into(),
            Route::Settings => "#/profile/settings".into(),
            Route::AiSettings => "#/coach/settings".into(),
            Route::About => "#/profile/about".into(),
        }
    }
    pub fn from_hash(h: &str) -> Option<Route> {
        let p: Vec<&str> = h
            .trim_start_matches('#')
            .trim_start_matches('/')
            .split('/')
            .collect();
        Some(match p.as_slice() {
            ["welcome"] => Route::Onboarding,
            ["today"] | [""] => Route::Today,
            ["explore"] => Route::Explore,
            ["explore", m, r] => Route::Metric(Metric::from_key(m)?, Range::from_key(r)?),
            ["explore", m] => Route::Metric(Metric::from_key(m)?, Range::Week),
            ["journal"] => Route::Journal,
            ["coach"] => Route::Coach,
            ["coach", "settings"] => Route::AiSettings,
            ["profile"] => Route::Profile,
            ["profile", "device"] => Route::Device,
            ["profile", "data"] => Route::Data,
            ["profile", "data", "import"] => Route::Import,
            ["profile", "settings"] => Route::Settings,
            ["profile", "about"] => Route::About,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub error: bool,
}

/// What a bottom sheet is showing.
#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    /// Provenance and method for a day's summary.
    Provenance(Metric, NaiveDate),
    /// Freshness / connection explanation for Today.
    Status,
}

#[derive(Clone, Copy)]
pub struct App {
    pub settings: RwSignal<Settings>,
    pub db: RwSignal<DbKind>,
    pub summaries: RwSignal<SummaryIndex>,
    pub journal: RwSignal<Vec<JournalEntry>>,
    pub batches: RwSignal<Vec<ImportBatch>>,
    pub route: RwSignal<Route>,
    pub day: RwSignal<NaiveDate>,
    pub ready: RwSignal<bool>,
    pub busy: RwSignal<Option<String>>,
    pub toast: RwSignal<Option<Toast>>,
    /// Bumped after every write, so on-demand loads refresh.
    pub data_rev: RwSignal<u64>,
    pub demo_device: RwSignal<Option<DemoDevice>>,
    pub live: RwSignal<serde_json::Value>,
    pub live_busy: RwSignal<bool>,
    pub live_loaded_at: RwSignal<i64>,
    pub sheet: RwSignal<Option<Sheet>>,
    pub ai: RwSignal<Option<AiConfig>>,
    /// Browser only: the AI key, kept in memory and never persisted.
    pub ai_key_mem: RwSignal<Option<String>>,
    /// Android: whether a key is stored in the Keystore-backed vault.
    pub ai_native_key: RwSignal<bool>,
    pub online: RwSignal<bool>,
    /// Current time, refreshed every 30 s for "x min ago" labels.
    pub now: RwSignal<i64>,
    pub measurement_count: RwSignal<u32>,
    /// Exact data shown to the user before the last AI request.
    pub last_ai_context: RwSignal<Option<String>>,
    /// Viewport width in CSS px (for chart sizing).
    pub vw: RwSignal<f64>,
}

pub const LS_SETTINGS: &str = "airspace.settings.v1";
pub const LS_DB: &str = "airspace.db";
pub const LS_ROUTE: &str = "airspace.route";
pub const LS_DAY: &str = "airspace.day";
pub const LS_AI: &str = "airspace.ai.v1";

impl App {
    pub fn new() -> Self {
        let mut settings: Settings = ls_get_json(LS_SETTINGS).unwrap_or_default();
        if settings.timezone == "UTC" && ls_get(LS_SETTINGS).is_none() {
            settings.timezone = platform::detect_timezone();
        }
        let tz = time::parse_tz(&settings.timezone);
        let today = time::local_day_in(now_ms(), &tz);
        let db = match ls_get(LS_DB).as_deref() {
            Some("demo") => DbKind::Demo,
            _ => DbKind::Personal,
        };
        let day = ls_get(LS_DAY)
            .as_deref()
            .and_then(time::parse_day)
            .filter(|d| *d <= today)
            .unwrap_or(today);
        App {
            settings: RwSignal::new(settings),
            db: RwSignal::new(db),
            summaries: RwSignal::new(SummaryIndex::default()),
            journal: RwSignal::new(vec![]),
            batches: RwSignal::new(vec![]),
            route: RwSignal::new(Route::Today),
            day: RwSignal::new(day),
            ready: RwSignal::new(false),
            busy: RwSignal::new(None),
            toast: RwSignal::new(None),
            data_rev: RwSignal::new(0),
            demo_device: RwSignal::new(None),
            live: RwSignal::new(
                serde_json::json!({"state":"unavailable","recording":false,"configured":false}),
            ),
            live_busy: RwSignal::new(false),
            live_loaded_at: RwSignal::new(0),
            sheet: RwSignal::new(None),
            ai: RwSignal::new(ls_get_json(LS_AI)),
            ai_key_mem: RwSignal::new(None),
            ai_native_key: RwSignal::new(false),
            online: RwSignal::new(platform::is_online()),
            now: RwSignal::new(now_ms()),
            measurement_count: RwSignal::new(0),
            last_ai_context: RwSignal::new(None),
            vw: RwSignal::new(
                platform::window()
                    .inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(390.0),
            ),
        }
    }

    pub async fn refresh_live(&self) {
        if !platform::is_android() || self.live_busy.get_untracked() {
            return;
        }
        self.live_busy.set(true);
        let result = platform::invoke::<serde_json::Value>(
            "live_action",
            serde_json::json!({"action":"status"}),
        )
        .await;
        if let Ok(status) = result {
            self.live.set(status);
            if self.db.get_untracked() == DbKind::Personal
                && now_ms() - self.live_loaded_at.get_untracked() > 15_000
            {
                self.live_loaded_at.set(now_ms());
                if let Err(e) = self.load().await {
                    self.error(e);
                }
            }
        }
        self.live_busy.set(false);
    }
    pub async fn live_command(&self, action: &str) {
        if self.live_busy.get_untracked() {
            return;
        }
        self.live_busy.set(true);
        match platform::invoke::<serde_json::Value>(
            "live_action",
            serde_json::json!({"action":action}),
        )
        .await
        {
            Ok(status) => {
                self.live.set(status);
            }
            Err(e) => self.error(e),
        }
        self.live_busy.set(false);
    }
    pub fn live_connected(&self) -> bool {
        self.db.get() == DbKind::Personal
            && self
                .live
                .with(|s| s["state"] == "connected" && s["recording"] == true)
    }
    pub fn live_text(&self) -> String {
        self.live.with(|s| {
            s.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Import your device credentials to connect your Air")
                .to_string()
        })
    }
    pub fn tz(&self) -> Tz {
        self.settings.with(|s| time::parse_tz(&s.timezone))
    }
    pub fn tz_untracked(&self) -> Tz {
        self.settings
            .with_untracked(|s| time::parse_tz(&s.timezone))
    }
    pub fn today(&self) -> NaiveDate {
        time::local_day_in(self.now.get(), &self.tz())
    }
    pub fn is_demo(&self) -> bool {
        self.db.get() == DbKind::Demo
    }

    pub fn go(&self, r: Route) {
        self.route.set(r);
    }

    pub fn back(&self) {
        let r = self.route.get_untracked();
        if let Some(p) = r.parent() {
            let _ = platform::window().history().map(|h| h.back());
            // Fallback when there is no history entry (e.g. deep link).
            let app = *self;
            set_timeout(
                move || {
                    if app.route.get_untracked() == r {
                        app.route.set(p);
                    }
                },
                std::time::Duration::from_millis(120),
            );
        }
    }

    pub fn notify(&self, msg: impl Into<String>) {
        self.toast.set(Some(Toast {
            id: now_ms() as u64,
            message: msg.into(),
            error: false,
        }));
    }
    pub fn error(&self, msg: impl Into<String>) {
        self.toast.set(Some(Toast {
            id: now_ms() as u64,
            message: msg.into(),
            error: true,
        }));
    }

    pub fn save_settings(&self) {
        self.settings
            .with_untracked(|s| ls_set_json(LS_SETTINGS, s));
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        self.settings.update(f);
        self.save_settings();
    }

    pub fn haptic(&self) {
        platform::haptic(self.settings.with_untracked(|s| s.haptics));
    }

    fn ctx(&self) -> SummaryContext {
        SummaryContext {
            tz: self.tz_untracked(),
            now: now_ms(),
            demo: self.db.get_untracked() == DbKind::Demo,
        }
    }

    /// Loads summaries, journal and metadata for the active database.
    pub async fn load(&self) -> Result<(), String> {
        let k = self.db.get_untracked();
        repo::open(k).await?;
        let sums = repo::summaries(k).await?;
        let journal = repo::journal(k).await?;
        let batches = repo::batches(k).await.unwrap_or_default();
        let count = repo::count(k).await.unwrap_or(0);
        let dev = if k == DbKind::Demo {
            repo::meta_get(k, "demo_device")
                .await
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        };
        // Recompute summaries calculated by an older version or another time zone.
        let tz_key = self.settings.with_untracked(|s| s.timezone.clone());
        let stale = sums
            .iter()
            .any(|s| s.calc_version != aggregate::CALC_VERSION)
            || repo::meta_get(k, "summary_tz")
                .await
                .ok()
                .flatten()
                .map(|t| t != tz_key)
                .unwrap_or(false);
        self.summaries.set(SummaryIndex::from_vec(sums));
        self.journal.set(journal);
        self.batches.set(batches);
        self.demo_device.set(dev);
        self.measurement_count.set(count);
        self.data_rev.update(|r| *r += 1);
        if stale && count > 0 {
            self.recompute_all().await?;
        }
        Ok(())
    }

    pub async fn switch_db(&self, k: DbKind) -> Result<(), String> {
        self.db.set(k);
        ls_set(LS_DB, k.key());
        self.load().await?;
        self.day.set(self.today_untracked());
        Ok(())
    }

    pub fn today_untracked(&self) -> NaiveDate {
        time::local_day_in(now_ms(), &self.tz_untracked())
    }

    /// Writes the demo dataset for today into the demo database.
    pub async fn generate_demo(&self) -> Result<(), String> {
        self.busy.set(Some("Preparing demo data…".into()));
        let k = DbKind::Demo;
        repo::open(k).await?;
        repo::clear(k).await?;
        let tz = self.tz_untracked();
        let now = now_ms();
        let today = time::local_day_in(now, &tz);
        let ds = demo::generate(today, &tz, now);
        repo::put_measurements(k, &ds.measurements).await?;
        let sums = aggregate::summarize(
            &ds.measurements,
            &SummaryContext {
                tz,
                now,
                demo: true,
            },
        );
        repo::put_summaries(k, &sums).await?;
        repo::put_journal(k, &ds.journal).await?;
        repo::meta_set(
            k,
            "demo_device",
            &serde_json::to_string(&ds.device).unwrap_or_default(),
        )
        .await?;
        repo::meta_set(k, "demo_day", &time::day_key(today)).await?;
        repo::meta_set(
            k,
            "summary_tz",
            &self.settings.with_untracked(|s| s.timezone.clone()),
        )
        .await?;
        self.busy.set(None);
        Ok(())
    }

    /// Opens the demo, creating or refreshing it so "today" has data.
    pub async fn open_demo(&self) -> Result<(), String> {
        repo::open(DbKind::Demo).await?;
        let day = repo::meta_get(DbKind::Demo, "demo_day")
            .await
            .ok()
            .flatten();
        if day.as_deref() != Some(time::day_key(self.today_untracked()).as_str()) {
            let refreshed = day.is_some();
            self.generate_demo().await?;
            if refreshed {
                self.notify("Demo data was regenerated so it ends today.");
            }
        }
        self.switch_db(DbKind::Demo).await
    }

    /// Stores measurements and recomputes the affected daily summaries.
    pub async fn ingest(&self, items: Vec<Measurement>) -> Result<(), String> {
        if items.is_empty() {
            return Ok(());
        }
        let k = self.db.get_untracked();
        repo::put_measurements(k, &items).await?;
        self.recompute_for(&items).await?;
        self.measurement_count
            .set(repo::count(k).await.unwrap_or(0));
        Ok(())
    }

    async fn recompute_for(&self, changed: &[Measurement]) -> Result<(), String> {
        let k = self.db.get_untracked();
        let ctx = self.ctx();
        let affected: BTreeSet<(Metric, NaiveDate)> = changed
            .iter()
            .map(|m| (m.metric, aggregate::day_of(m, &ctx.tz)))
            .collect();
        let metrics: HashSet<Metric> = affected.iter().map(|(m, _)| *m).collect();
        let (Some(first), Some(last)) = (
            affected.iter().map(|(_, d)| *d).min(),
            affected.iter().map(|(_, d)| *d).max(),
        ) else {
            return Ok(());
        };
        // Load a margin around the affected days: sessions and offsets can cross midnight.
        let from = time::day_start(first, &ctx.tz) - 2 * DAY_MS;
        let to = time::day_start(last, &ctx.tz) + 3 * DAY_MS;
        let mut fresh = vec![];
        for m in metrics {
            let ms = repo::measurements(k, Some(m), from, to).await?;
            fresh.extend(aggregate::summarize(&ms, &ctx).into_iter().filter(|s| {
                time::parse_day(&s.day)
                    .map(|d| affected.contains(&(m, d)))
                    .unwrap_or(false)
            }));
        }
        let have: HashSet<(Metric, NaiveDate)> = fresh
            .iter()
            .filter_map(|s| Some((s.metric, time::parse_day(&s.day)?)))
            .collect();
        let gone: Vec<(Metric, NaiveDate)> = affected
            .iter()
            .filter(|x| !have.contains(x))
            .copied()
            .collect();
        repo::put_summaries(k, &fresh).await?;
        repo::delete_summaries(
            k,
            &gone
                .iter()
                .map(|(m, d)| aggregate::summary_key(*m, &time::day_key(*d)))
                .collect::<Vec<_>>(),
        )
        .await?;
        self.summaries.update(|idx| {
            idx.remove(&gone);
            idx.extend(fresh);
        });
        self.data_rev.update(|r| *r += 1);
        Ok(())
    }

    /// Recomputes every summary (after a time-zone change or upgrade).
    pub async fn recompute_all(&self) -> Result<(), String> {
        self.busy.set(Some("Recalculating daily summaries…".into()));
        let k = self.db.get_untracked();
        let all = repo::all_measurements(k).await?;
        let sums = aggregate::summarize(&all, &self.ctx());
        let old: Vec<String> = self
            .summaries
            .with_untracked(|i| i.map.values().map(DaySummary::key).collect());
        repo::delete_summaries(k, &old).await?;
        repo::put_summaries(k, &sums).await?;
        repo::meta_set(
            k,
            "summary_tz",
            &self.settings.with_untracked(|s| s.timezone.clone()),
        )
        .await?;
        self.summaries.set(SummaryIndex::from_vec(sums));
        self.data_rev.update(|r| *r += 1);
        self.busy.set(None);
        Ok(())
    }

    pub async fn save_journal(&self, mut e: JournalEntry) -> Result<(), String> {
        let k = self.db.get_untracked();
        e.demo = k == DbKind::Demo;
        e.updated_at = now_ms();
        if e.is_empty() {
            repo::delete_journal(k, &e.id).await?;
            self.journal.update(|j| j.retain(|x| x.id != e.id));
        } else {
            repo::put_journal(k, std::slice::from_ref(&e)).await?;
            self.journal.update(|j| {
                j.retain(|x| x.id != e.id);
                j.push(e);
                j.sort_by(|a, b| a.day.cmp(&b.day));
            });
        }
        Ok(())
    }

    pub async fn delete_all(&self, k: DbKind) -> Result<(), String> {
        repo::clear(k).await?;
        if self.db.get_untracked() == k {
            self.summaries.set(SummaryIndex::default());
            self.journal.set(vec![]);
            self.batches.set(vec![]);
            self.demo_device.set(None);
            self.measurement_count.set(0);
            self.data_rev.update(|r| *r += 1);
        }
        Ok(())
    }

    pub async fn backup_json(&self) -> Result<String, String> {
        let k = self.db.get_untracked();
        let b = Backup::new(
            k,
            now_ms(),
            self.settings.get_untracked(),
            self.ai.get_untracked(),
            repo::all_measurements(k).await?,
            repo::journal(k).await?,
            repo::batches(k).await.unwrap_or_default(),
        );
        Ok(b.to_json())
    }

    pub async fn metric_csv(&self, m: Metric) -> Result<String, String> {
        let k = self.db.get_untracked();
        let items = repo::measurements(k, Some(m), i64::MIN / 4, i64::MAX / 4).await?;
        Ok(backup::metric_csv(m, &items, &self.tz_untracked()))
    }

    /// Restores a backup into the active database.
    pub async fn restore(&self, b: Backup, replace: bool) -> Result<usize, String> {
        let k = self.db.get_untracked();
        let backup_is_demo =
            b.database == DbKind::Demo || b.measurements.iter().any(|m| m.prov.demo);
        if k == DbKind::Personal && backup_is_demo {
            return Err(
                "This backup contains demo data and can't be restored into your personal database."
                    .into(),
            );
        }
        if k == DbKind::Demo && !backup_is_demo {
            return Err("Switch to your personal data before restoring a personal backup.".into());
        }
        if replace {
            repo::clear(k).await?;
        }
        let existing: HashSet<String> = if replace {
            HashSet::new()
        } else {
            repo::all_measurements(k)
                .await?
                .into_iter()
                .map(|m| m.id)
                .collect()
        };
        let new: Vec<Measurement> = b
            .measurements
            .into_iter()
            .filter(|m| !existing.contains(&m.id))
            .collect();
        let n = new.len();
        repo::put_measurements(k, &new).await?;
        repo::put_journal(k, &b.journal).await?;
        for batch in &b.batches {
            repo::put_batch(k, batch).await?;
        }
        if replace {
            let mut s = b.settings;
            s.onboarded = true;
            self.settings.set(s);
            self.save_settings();
        }
        self.load().await?;
        self.recompute_all().await?;
        Ok(n)
    }
}

/// A signal filled asynchronously whenever its key changes. Late results
/// from superseded keys are discarded.
pub fn async_signal<K, T, Fut>(
    key: impl Fn() -> K + 'static,
    load: impl Fn(K) -> Fut + 'static,
) -> RwSignal<Option<T>>
where
    K: Clone + PartialEq + Send + Sync + 'static,
    T: Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + 'static,
{
    let out = RwSignal::new(None);
    let seq = StoredValue::new(0u64);
    let load = std::rc::Rc::new(load);
    Effect::new(move |_| {
        let k = key();
        seq.update_value(|s| *s += 1);
        let mine = seq.get_value();
        let load = load.clone();
        spawn_local(async move {
            let v = load(k).await;
            if seq.get_value() == mine {
                out.set(Some(v));
            }
        });
    });
    out
}

/// Measurements for one metric over a set of local days.
pub async fn load_range(
    app: App,
    metric: Metric,
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<Measurement> {
    let tz = app.tz_untracked();
    let a = time::day_start(from, &tz) - DAY_MS;
    let b = time::day_start(to, &tz) + 2 * DAY_MS;
    let ms = repo::measurements(app.db.get_untracked(), Some(metric), a, b)
        .await
        .unwrap_or_default();
    ms.into_iter()
        .filter(|m| m.is_active())
        .filter(|m| {
            let d = aggregate::day_of(m, &tz);
            d >= from && d <= to
        })
        .collect()
}
