//! Original line icons (24×24, 1.75 stroke). Decorative by default.

use leptos::prelude::*;

fn paths(name: &str) -> &'static [&'static str] {
    match name {
        "today" => &["M12 3v2", "M12 19v2", "M4.2 4.2l1.4 1.4", "M18.4 18.4l1.4 1.4", "M3 12h2", "M19 12h2", "M4.2 19.8l1.4-1.4", "M18.4 5.6l1.4-1.4", "M12 8a4 4 0 1 0 0 8a4 4 0 0 0 0-8z"],
        "explore" => &["M4 19V9", "M10 19V5", "M16 19v-7", "M22 19H2"],
        "journal" => &["M6 3h11a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6z", "M6 3v18", "M10 8h5", "M10 12h5"],
        "coach" => &["M12 3l1.8 4.6L18.5 9l-4.7 1.8L12 15.5l-1.8-4.7L5.5 9l4.7-1.4z", "M18 15l.9 2.1L21 18l-2.1.9L18 21l-.9-2.1L15 18l2.1-.9z"],
        "device" => &["M9 3h6l1 4H8z", "M8 17l1 4h6l1-4", "M8 7h8a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2z"],
        "back" => &["M15 5l-7 7l7 7"],
        "left" => &["M14 6l-6 6l6 6"],
        "right" => &["M10 6l6 6l-6 6"],
        "down" => &["M6 10l6 6l6-6"],
        "close" => &["M6 6l12 12", "M18 6L6 18"],
        "check" => &["M5 12.5l4.5 4.5L19 7.5"],
        "plus" => &["M12 5v14", "M5 12h14"],
        "info" => &["M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18z", "M12 11v5", "M12 7.5v.5"],
        "alert" => &["M12 4l9 16H3z", "M12 10v4", "M12 17v.5"],
        "download" => &["M12 4v11", "M7 10l5 5l5-5", "M5 20h14"],
        "upload" => &["M12 20V9", "M7 14l5-5l5 5", "M5 4h14"],
        "trash" => &["M4 7h16", "M9 7V4h6v3", "M6 7l1 13h10l1-13"],
        "bluetooth" => &["M7 7l10 10l-5 4V3l5 4L7 17"],
        "heart" => &["M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z"],
        "moon" => &["M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"],
        "steps" => &["M8 4c1.7 0 2.5 1.6 2.5 4S9.5 12 8 12s-2-1.8-2-4s.3-4 2-4z", "M6.5 14h3l-.5 3a1.5 1.5 0 0 1-3 0z", "M16 8c1.7 0 2 1.6 2 4s-.5 4-2 4s-2.5-1.6-2.5-4S14.3 8 16 8z", "M14.5 18h3l-.5 2a1.5 1.5 0 0 1-3 0z"],
        "edit" => &["M4 20h4L19 9l-4-4L4 16z", "M13.5 6.5l4 4"],
        "star" => &["M12 4l2.4 5l5.4.6l-4 3.7l1.1 5.4L12 16l-4.9 2.7l1.1-5.4l-4-3.7l5.4-.6z"],
        "table" => &["M4 5h16v14H4z", "M4 10h16", "M4 15h16", "M10 5v14"],
        "chart" => &["M4 18l5-6l4 3l7-9"],
        "settings" => &["M12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6z", "M19 12a7 7 0 0 0-.1-1.2l2-1.6l-2-3.4l-2.4 1a7 7 0 0 0-2-1.2L14 3h-4l-.5 2.6a7 7 0 0 0-2 1.2l-2.4-1l-2 3.4l2 1.6A7 7 0 0 0 5 12a7 7 0 0 0 .1 1.2l-2 1.6l2 3.4l2.4-1a7 7 0 0 0 2 1.2L10 21h4l.5-2.6a7 7 0 0 0 2-1.2l2.4 1l2-3.4l-2-1.6A7 7 0 0 0 19 12z"],
        "database" => &["M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3s-3.6 3-8 3s-8-1.3-8-3z", "M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6", "M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"],
        "lock" => &["M6 11h12v9H6z", "M8.5 11V8a3.5 3.5 0 0 1 7 0v3"],
        "send" => &["M4 12l16-8l-6 16l-2.5-6.5z"],
        "refresh" => &["M20 11a8 8 0 0 0-14.3-4.5L4 8", "M4 4v4h4", "M4 13a8 8 0 0 0 14.3 4.5L20 16", "M20 20v-4h-4"],
        "calendar" => &["M4 6h16v14H4z", "M4 10h16", "M8 3v5", "M16 3v5"],
        "wind" => &["M3 9h11a3 3 0 1 0-3-3", "M3 15h15a3 3 0 1 1-3 3", "M3 12h7"],
        "drop" => &["M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"],
        "wave" => &["M2 12h4l2-5l3 10l3-8l2 3h6"],
        "thermo" => &["M10 4a2 2 0 0 1 4 0v9.5a4 4 0 1 1-4 0z", "M12 10v6"],
        "flame" => &["M12 21a6 6 0 0 0 6-6c0-4-3-6-4-9c-1 2-2 3-3.5 3.5C9 8 9 6 9 5c-2 2-3 5-3 8a6 6 0 0 0 6 8z"],
        "route" => &["M5 19a2 2 0 1 0 0-4a2 2 0 0 0 0 4z", "M19 9a2 2 0 1 0 0-4a2 2 0 0 0 0 4z", "M7 17h6a3 3 0 0 0 0-6H11a3 3 0 0 1 0-6h6"],
        "zones" => &["M3 18a9 9 0 0 1 18 0", "M7 18a5 5 0 0 1 10 0", "M12 18l3-5"],
        _ => &["M12 12h.01"],
    }
}

#[component]
pub fn Icon(name: &'static str, #[prop(optional)] size: Option<u32>, #[prop(optional)] label: Option<&'static str>) -> impl IntoView {
    let s = size.unwrap_or(22);
    view! {
        <svg
            class="icon"
            width=s
            height=s
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.75"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden=if label.is_some() { "false" } else { "true" }
            role=label.map(|_| "img")
            aria-label=label
            focusable="false"
        >
            {paths(name).iter().map(|d| view! { <path d=*d /> }).collect_view()}
        </svg>
    }
}

pub fn metric_icon(m: airspace_core::Metric) -> &'static str {
    use airspace_core::Metric::*;
    match m {
        HeartRate | RestingHeartRate => "heart",
        HeartRateZones => "zones",
        Steps => "steps",
        Sleep => "moon",
        Distance => "route",
        Calories => "flame",
        Spo2 => "drop",
        Hrv => "wave",
        SkinTemperature => "thermo",
        RespiratoryRate => "wind",
    }
}
