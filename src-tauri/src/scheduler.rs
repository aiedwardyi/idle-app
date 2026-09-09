use crate::contract::*;
use chrono::{DateTime, FixedOffset, Local, Timelike};

pub const ENGINES: [EngineId; 4] = [
    EngineId::Claude,
    EngineId::Codex,
    EngineId::Antigravity,
    EngineId::Grok,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idle {
    Minutes(u32),
    Unknown,
}

pub struct Snapshot {
    pub tasks: Vec<Task>,
    pub meters: Vec<MeterState>,
    pub active: Vec<EngineId>,
    pub detect: Vec<EngineStatus>,
    pub cooldowns: Vec<(EngineId, String)>,
    pub schedule: Schedule,
    pub now: DateTime<FixedOffset>,
    pub idle: Idle,
}

pub struct Decision {
    pub starts: Vec<String>,
    pub statuses: Vec<SchedulerStatus>,
}

pub fn resolve(engine: &EngineChoice) -> EngineId {
    match engine {
        EngineChoice::Auto => EngineId::Claude,
        EngineChoice::Fixed(id) => *id,
    }
}

pub fn quiet_hours(schedule: &Schedule, minute: u16) -> bool {
    let (Some(start), Some(end)) = (
        minute_of_day(&schedule.quiet_start),
        minute_of_day(&schedule.quiet_end),
    ) else {
        return false;
    };
    schedule.enabled
        && if start == end {
            true
        } else if start < end {
            minute >= start && minute < end
        } else {
            minute >= start || minute < end
        }
}

pub fn decide(s: &Snapshot) -> Decision {
    let mut result = Decision {
        starts: Vec::new(),
        statuses: Vec::new(),
    };
    for engine in ENGINES {
        let head = s
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Queued && resolve(&t.engine) == engine)
            .min_by_key(|t| (&t.created_at, &t.id));
        let until = s
            .cooldowns
            .iter()
            .filter(|(e, _)| *e == engine)
            .filter_map(|(_, until)| {
                DateTime::parse_from_rfc3339(until)
                    .ok()
                    .map(|date| (date, until))
            })
            .filter(|(date, _)| *date > s.now)
            .max_by_key(|(date, _)| *date)
            .map(|(_, text)| text.clone());
        let (state, reason) = if s.active.contains(&engine) {
            (SchedulerState::Running, SchedulerReason::Busy)
        } else if !s.schedule.enabled {
            (SchedulerState::Off, SchedulerReason::NoTasks)
        } else if until.is_some() {
            (SchedulerState::Paused, SchedulerReason::Cooldown)
        } else if let Some(task) = head {
            let margin = SIZE_MARGINS[match task.size {
                TaskSize::S => 0,
                TaskSize::M => 1,
                TaskSize::L => 2,
            }];
            let reason = if !s
                .detect
                .iter()
                .any(|d| d.engine == engine && d.detect.installed && d.detect.signed_in)
            {
                SchedulerReason::EngineUnavailable
            } else if !quiet_hours(&s.schedule, (s.now.hour() * 60 + s.now.minute()) as u16) {
                SchedulerReason::QuietHours
            } else if matches!(s.idle, Idle::Minutes(n) if n < u32::from(s.schedule.idle_minutes)) {
                SchedulerReason::NotIdle
            } else if s.active.len() + result.starts.len() >= usize::from(s.schedule.max_concurrent)
            {
                SchedulerReason::Busy
            } else if s
                .meters
                .iter()
                .filter(|m| m.engine == engine && m.source != MeterSource::None)
                .any(|m| {
                    m.remaining_pct.is_some_and(|p| {
                        !p.is_finite() || p < f64::from(s.schedule.reserve_pct) + f64::from(margin)
                    })
                })
            {
                SchedulerReason::Reserve
            } else {
                result.starts.push(task.id.clone());
                SchedulerReason::NoTasks
            };
            (SchedulerState::Waiting, reason)
        } else {
            (SchedulerState::Waiting, SchedulerReason::NoTasks)
        };
        result.statuses.push(SchedulerStatus {
            engine,
            state,
            reason,
            until: if state == SchedulerState::Paused {
                until
            } else {
                None
            },
        });
    }
    result
}

pub fn local_now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

#[cfg(windows)]
pub fn idle_probe() -> Idle {
    use windows_sys::Win32::{
        System::SystemInformation::GetTickCount,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return Idle::Unknown;
        }
        Idle::Minutes(GetTickCount().wrapping_sub(info.dwTime) / 60_000)
    }
}

#[cfg(not(windows))]
pub fn idle_probe() -> Idle {
    // TODO: macOS CGEventSourceSecondsSinceLastEvent after adding a macOS CI leg.
    Idle::Unknown
}
