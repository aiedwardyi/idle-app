use crate::contract::{
    default_windows, EngineId, LimitWindowKind, MeterSource, MeterState, RunEvent, Usage,
};
use crate::engines::claude::rfc3339_from_unix;

pub fn seed(engine: EngineId, kind: LimitWindowKind) -> Option<MeterState> {
    if !known_window(engine, kind) {
        return None;
    }
    Some(MeterState {
        engine,
        window: kind,
        used: Usage::default(),
        capacity_est: None,
        calibrated: false,
        remaining_pct: None,
        resets_at: None,
        source: MeterSource::None,
        observed_at: None,
    })
}

/// RFC3339 UTC `YYYY-MM-DDTHH:MM:SSZ` compares lexicographically.
pub fn refresh(mut row: MeterState, now: &str) -> MeterState {
    let Some(resets_at) = row.resets_at.as_deref() else {
        return row;
    };
    if now < resets_at {
        return row;
    }
    row.used = Usage::default();
    row.resets_at = None;
    row.remaining_pct = if row.source != MeterSource::None {
        Some(100.0)
    } else {
        None
    };
    row
}

pub fn apply(row: MeterState, engine: EngineId, event: &RunEvent, now: &str) -> MeterState {
    let row = refresh(row, now);
    let row = apply_event(row, engine, event, now);
    refresh(row, now)
}

fn apply_event(row: MeterState, engine: EngineId, event: &RunEvent, now: &str) -> MeterState {
    match event {
        RunEvent::WindowReading {
            window,
            utilization,
            resets_at,
            ..
        } => {
            if !utilization.is_finite() || !known_window(engine, *window) || row.window != *window {
                return row;
            }
            let util = utilization.clamp(0.0, 1.0);
            MeterState {
                remaining_pct: Some(100.0 * (1.0 - util)),
                // None is no new information, never erase a known deadline.
                resets_at: resets_at.clone().or_else(|| row.resets_at.clone()),
                source: MeterSource::Vendor,
                calibrated: true,
                observed_at: Some(now.to_string()),
                ..row
            }
        }
        RunEvent::Usage {
            input,
            output,
            cache,
            ..
        } => {
            let used = Usage {
                input: row.used.input.saturating_add(*input),
                output: row.used.output.saturating_add(*output),
                cache: row.used.cache.saturating_add(*cache),
            };
            let (remaining_pct, source) = if row.source == MeterSource::Vendor {
                (row.remaining_pct, row.source)
            } else if let Some(cap) = row.capacity_est.filter(|&c| c > 0) {
                let total = used
                    .input
                    .saturating_add(used.output)
                    .saturating_add(used.cache) as f64;
                let remaining = (100.0 * (1.0 - total / cap as f64)).clamp(0.0, 100.0);
                (Some(remaining), MeterSource::Estimate)
            } else {
                (row.remaining_pct, row.source)
            };
            MeterState {
                used,
                remaining_pct,
                source,
                ..row
            }
        }
        RunEvent::Started { .. } => {
            // Vendor rows only take a deadline from the vendor; guessing one is forbidden.
            if row.resets_at.is_none() && row.source != MeterSource::Vendor {
                if let Some(hours) = window_hours(engine, row.window) {
                    return MeterState {
                        resets_at: Some(add_hours(now, hours)),
                        ..row
                    };
                }
            }
            row
        }
        RunEvent::LimitHit { .. }
        | RunEvent::Output { .. }
        | RunEvent::Error { .. }
        | RunEvent::Finished { .. } => row,
    }
}

fn known_window(engine: EngineId, kind: LimitWindowKind) -> bool {
    default_windows(engine).iter().any(|w| w.kind == kind)
}

fn window_hours(engine: EngineId, kind: LimitWindowKind) -> Option<u32> {
    default_windows(engine)
        .iter()
        .find(|w| w.kind == kind)
        .map(|w| w.hours)
}

fn add_hours(now: &str, hours: u32) -> String {
    match unix_secs(now) {
        Some(secs) => rfc3339_from_unix(secs.saturating_add(u64::from(hours) * 3600)),
        None => now.to_string(),
    }
}

fn unix_secs(ts: &str) -> Option<u64> {
    if ts.len() != 20 {
        return None;
    }
    let b = ts.as_bytes();
    if b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let year: i64 = ts[0..4].parse().ok()?;
    let month: u32 = ts[5..7].parse().ok()?;
    let day: u32 = ts[8..10].parse().ok()?;
    let hour: u64 = ts[11..13].parse().ok()?;
    let min: u64 = ts[14..16].parse().ok()?;
    let sec: u64 = ts[17..19].parse().ok()?;
    if !(1..=12).contains(&month) || day == 0 || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hour * 3600 + min * 60 + sec)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * u64::from(mp) + 2) / 5 + u64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-04T00:00:00Z";

    fn claude_5h() -> MeterState {
        seed(EngineId::Claude, LimitWindowKind::FiveHour).unwrap()
    }

    fn claude_weekly() -> MeterState {
        seed(EngineId::Claude, LimitWindowKind::Weekly).unwrap()
    }

    fn reading(kind: LimitWindowKind, utilization: f64, resets: Option<&str>) -> RunEvent {
        RunEvent::WindowReading {
            run_id: "r1".into(),
            window: kind,
            utilization,
            resets_at: resets.map(str::to_string),
        }
    }

    fn usage(input: u64, output: u64, cache: u64) -> RunEvent {
        RunEvent::Usage {
            run_id: "r1".into(),
            input,
            output,
            cache,
        }
    }

    fn started() -> RunEvent {
        RunEvent::Started {
            run_id: "r1".into(),
        }
    }

    fn apply_claude(row: MeterState, event: &RunEvent) -> MeterState {
        apply(row, EngineId::Claude, event, NOW)
    }

    #[test]
    fn r1_window_reading_sets_vendor_fill() {
        let row = apply_claude(
            claude_5h(),
            &reading(
                LimitWindowKind::FiveHour,
                0.25,
                Some("2026-09-04T05:00:00Z"),
            ),
        );
        assert_eq!(row.remaining_pct, Some(75.0));
        assert_eq!(row.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        assert_eq!(row.source, MeterSource::Vendor);
        assert!(row.calibrated);
        assert_eq!(row.observed_at.as_deref(), Some(NOW));
        assert_eq!(row.capacity_est, None);
        assert_eq!(row.used, Usage::default());
    }

    #[test]
    fn r2_usage_adds_tokens_and_estimates_only_without_vendor() {
        let added = usage(10, 20, 30);
        let vendor = MeterState {
            remaining_pct: Some(75.0),
            source: MeterSource::Vendor,
            ..claude_5h()
        };
        let after_vendor = apply_claude(vendor, &added);
        assert_eq!(
            after_vendor.used,
            Usage {
                input: 10,
                output: 20,
                cache: 30
            }
        );
        assert_eq!(after_vendor.remaining_pct, Some(75.0));
        assert_eq!(after_vendor.source, MeterSource::Vendor);

        let estimated = MeterState {
            capacity_est: Some(200),
            ..claude_5h()
        };
        let after_est = apply_claude(estimated, &added);
        assert_eq!(after_est.remaining_pct, Some(70.0));
        assert_eq!(after_est.source, MeterSource::Estimate);

        let none = apply_claude(claude_5h(), &added);
        assert_eq!(
            none.used,
            Usage {
                input: 10,
                output: 20,
                cache: 30
            }
        );
        assert_eq!(none.remaining_pct, None);
        assert_eq!(none.source, MeterSource::None);
    }

    #[test]
    fn r1_reading_without_resets_at_keeps_the_deadline() {
        let row = MeterState {
            remaining_pct: Some(50.0),
            resets_at: Some("2026-09-04T05:00:00Z".into()),
            source: MeterSource::Vendor,
            ..claude_5h()
        };
        let after = apply_claude(row, &reading(LimitWindowKind::FiveHour, 0.25, None));
        assert_eq!(after.remaining_pct, Some(75.0));
        assert_eq!(after.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        assert_eq!(after.source, MeterSource::Vendor);
    }

    #[test]
    fn r2_zero_capacity_stays_unknown() {
        let row = MeterState {
            capacity_est: Some(0),
            ..claude_5h()
        };
        let after = apply_claude(row, &usage(10, 20, 30));
        assert_eq!(
            after.used,
            Usage {
                input: 10,
                output: 20,
                cache: 30
            }
        );
        assert_eq!(after.remaining_pct, None);
        assert_eq!(after.source, MeterSource::None);
        assert_eq!(after.capacity_est, Some(0));
    }

    #[test]
    fn r3_started_sets_resets_at_from_window_hours() {
        let fresh = apply_claude(claude_5h(), &started());
        assert_eq!(fresh.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        let weekly = apply_claude(claude_weekly(), &started());
        assert_eq!(weekly.resets_at.as_deref(), Some("2026-09-11T00:00:00Z"));
    }

    #[test]
    fn r4_limit_hit_leaves_every_row_identical() {
        let rows = [claude_5h(), claude_weekly()];
        for window in [Some(LimitWindowKind::FiveHour), None] {
            let event = RunEvent::LimitHit {
                run_id: "r1".into(),
                window,
                resets_at: Some("2026-09-04T05:00:00Z".into()),
            };
            for row in &rows {
                assert_eq!(apply_claude(row.clone(), &event), *row);
            }
        }
    }

    #[test]
    fn r5_output_error_finished_are_identity() {
        let row = claude_5h();
        let events = [
            RunEvent::Output {
                run_id: "r1".into(),
                line: "hi".into(),
            },
            RunEvent::Error {
                run_id: "r1".into(),
                message: "no".into(),
            },
            RunEvent::Finished {
                run_id: "r1".into(),
                ok: true,
            },
        ];
        for event in events {
            assert_eq!(apply_claude(row.clone(), &event), row);
        }
    }

    #[test]
    fn r6_refresh_rolls_used_and_resets_at() {
        let row = MeterState {
            used: Usage {
                input: 1,
                output: 2,
                cache: 3,
            },
            remaining_pct: Some(40.0),
            resets_at: Some("2026-09-04T05:00:00Z".into()),
            source: MeterSource::Estimate,
            calibrated: true,
            observed_at: Some("2026-09-04T01:00:00Z".into()),
            ..claude_5h()
        };
        let rolled = refresh(row.clone(), "2026-09-04T06:00:00Z");
        assert_eq!(rolled.used, Usage::default());
        assert_eq!(rolled.resets_at, None);
        assert_eq!(rolled.remaining_pct, Some(100.0));
        assert_eq!(rolled.source, row.source);
        assert_eq!(rolled.calibrated, row.calibrated);
        assert_eq!(rolled.observed_at, row.observed_at);
        assert_eq!(refresh(row.clone(), NOW), row);
    }

    #[test]
    fn r7_last_applied_reading_wins() {
        let first = reading(LimitWindowKind::FiveHour, 0.25, None);
        let second = reading(LimitWindowKind::FiveHour, 0.5, None);
        let a = apply_claude(apply_claude(claude_5h(), &first), &second);
        let b = apply_claude(apply_claude(claude_5h(), &second), &first);
        assert_eq!(a.remaining_pct, Some(50.0));
        assert_eq!(b.remaining_pct, Some(75.0));
    }

    #[test]
    fn r8_refresh_is_silent_when_the_window_is_still_open() {
        let row = MeterState {
            used: Usage {
                input: 9,
                output: 0,
                cache: 0,
            },
            resets_at: Some("2026-09-04T05:00:00Z".into()),
            remaining_pct: Some(40.0),
            source: MeterSource::Estimate,
            ..claude_5h()
        };
        assert_eq!(refresh(row.clone(), NOW), row);
        assert_eq!(apply_claude(row.clone(), &started()), row);
    }

    #[test]
    fn rollover_at_exactly_resets_at() {
        let row = MeterState {
            used: Usage {
                input: 4,
                output: 0,
                cache: 0,
            },
            remaining_pct: Some(10.0),
            resets_at: Some("2026-09-04T05:00:00Z".into()),
            source: MeterSource::Estimate,
            ..claude_5h()
        };
        let rolled = refresh(row, "2026-09-04T05:00:00Z");
        assert_eq!(rolled.used, Usage::default());
        assert_eq!(rolled.resets_at, None);
        assert_eq!(rolled.remaining_pct, Some(100.0));
    }

    #[test]
    fn vendor_rollover_stays_vendor_at_100() {
        let row = MeterState {
            remaining_pct: Some(12.0),
            resets_at: Some("2026-09-04T05:00:00Z".into()),
            source: MeterSource::Vendor,
            calibrated: true,
            ..claude_5h()
        };
        let rolled = refresh(row, "2026-09-04T05:00:00Z");
        assert_eq!(rolled.remaining_pct, Some(100.0));
        assert_eq!(rolled.source, MeterSource::Vendor);
        assert_eq!(rolled.resets_at, None);
    }

    #[test]
    fn usage_after_vendor_reading_changes_used_not_remaining() {
        let read = apply_claude(
            claude_5h(),
            &reading(
                LimitWindowKind::FiveHour,
                0.25,
                Some("2026-09-04T05:00:00Z"),
            ),
        );
        let after = apply_claude(read, &usage(1, 2, 3));
        assert_eq!(
            after.used,
            Usage {
                input: 1,
                output: 2,
                cache: 3
            }
        );
        assert_eq!(after.remaining_pct, Some(75.0));
        assert_eq!(after.source, MeterSource::Vendor);
    }

    #[test]
    fn daily_reading_on_claude_is_ignored_and_creates_nothing() {
        assert!(seed(EngineId::Claude, LimitWindowKind::Daily).is_none());
        let row = claude_5h();
        let event = reading(LimitWindowKind::Daily, 0.5, Some("2026-09-05T00:00:00Z"));
        assert_eq!(apply_claude(row.clone(), &event), row);
    }

    #[test]
    fn utilization_above_one_clamps_to_zero_remaining() {
        let row = apply_claude(
            claude_5h(),
            &reading(LimitWindowKind::FiveHour, 1.3, Some("2026-09-04T05:00:00Z")),
        );
        assert_eq!(row.remaining_pct, Some(0.0));
        assert_eq!(row.source, MeterSource::Vendor);
    }

    #[test]
    fn nan_and_infinite_utilization_are_ignored() {
        let row = claude_5h();
        for util in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                apply_claude(row.clone(), &reading(LimitWindowKind::FiveHour, util, None)),
                row
            );
        }
    }

    #[test]
    fn started_on_a_vendor_row_does_nothing() {
        let vendor = MeterState {
            source: MeterSource::Vendor,
            remaining_pct: Some(40.0),
            ..claude_5h()
        };
        assert_eq!(apply_claude(vendor.clone(), &started()), vendor);
        let fresh = apply_claude(claude_5h(), &started());
        assert_eq!(fresh.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
    }

    #[test]
    fn past_reading_rolls_straight_to_100() {
        let row = apply_claude(
            claude_5h(),
            &reading(
                LimitWindowKind::FiveHour,
                0.25,
                Some("2026-09-03T00:00:00Z"),
            ),
        );
        assert_eq!(row.remaining_pct, Some(100.0));
        assert_eq!(row.resets_at, None);
        assert_eq!(row.source, MeterSource::Vendor);
    }
}
