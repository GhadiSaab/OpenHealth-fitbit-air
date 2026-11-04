use airspace_core::{
    Metric, Origin, SampleKind,
    live_ingest::{RawLive, normalize},
};
fn row() -> RawLive {
    serde_json::from_value(serde_json::json!({"session":"test-session","seq":1,"received_at":1793820000000_i64,"utc_offset_min":60,"fields":{"1":1793820000,"2":1234,"3":19085,"4":38,"7":75,"8":2}})).unwrap()
}
#[test]
fn units_counters_and_replay_ids() {
    let a = normalize(&row()).unwrap();
    let b = normalize(&row()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 4);
    for m in &a {
        assert_eq!(m.prov.origin, Origin::BluetoothLive);
        assert!(!m.prov.demo);
        assert!(m.prov.provisional);
    }
    let d = a.iter().find(|m| m.metric == Metric::Distance).unwrap();
    assert_eq!(d.value, 19.085);
    assert_eq!(d.kind, SampleKind::CumulativeCounter);
}
#[test]
fn zero_heart_rate_is_not_a_measurement() {
    let mut r = row();
    r.fields.insert(7, 0);
    assert!(
        normalize(&r)
            .unwrap()
            .iter()
            .all(|m| m.metric != Metric::HeartRate)
    );
}
#[test]
fn unknown_fields_never_become_advanced_metrics() {
    let mut r = row();
    r.fields.insert(10, 97);
    assert!(normalize(&r).unwrap().iter().all(|m| matches!(
        m.metric,
        Metric::Steps | Metric::Distance | Metric::Calories | Metric::HeartRate
    )));
}
#[test]
fn invalid_clock_uses_receive_time() {
    let mut r = row();
    r.fields.insert(1, u64::MAX);
    let a = normalize(&r).unwrap();
    assert!(a.iter().all(|m| m.start == r.received_at));
}
#[test]
fn invalid_envelopes_rejected() {
    let mut r = row();
    r.session.clear();
    assert!(normalize(&r).is_err());
}
#[test]
fn counter_deltas_do_not_bridge_recording_sessions() {
    let a=row();let mut b=row();b.session="next-session".into();b.seq=2;b.received_at+=60_000;b.fields.insert(1,1793820060);b.fields.insert(2,1244);
    let mut ms=normalize(&a).unwrap();ms.extend(normalize(&b).unwrap());
    let ctx=airspace_core::aggregate::SummaryContext{tz:chrono_tz::UTC,now:b.received_at,demo:false};
    let steps=airspace_core::aggregate::summarize(&ms,&ctx).into_iter().find(|s|s.metric==Metric::Steps).unwrap();
    assert_eq!(steps.value.unwrap_or(0.0),0.0);
}
#[test]
fn counter_deltas_do_not_bridge_link_segments() {
    let a=row();let mut value=serde_json::to_value(row()).unwrap();value["segment"]=serde_json::json!(1);value["seq"]=serde_json::json!(2);value["received_at"]=serde_json::json!(1793820060000_i64);value["fields"]["1"]=serde_json::json!(1793820060_u64);value["fields"]["2"]=serde_json::json!(1244);
    let b:RawLive=serde_json::from_value(value).unwrap();let mut ms=normalize(&a).unwrap();ms.extend(normalize(&b).unwrap());
    let ctx=airspace_core::aggregate::SummaryContext{tz:chrono_tz::UTC,now:b.received_at,demo:false};
    let steps=airspace_core::aggregate::summarize(&ms,&ctx).into_iter().find(|s|s.metric==Metric::Steps).unwrap();
    assert_eq!(steps.value.unwrap_or(0.0),0.0);
}
