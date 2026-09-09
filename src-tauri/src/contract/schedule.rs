use super::EngineId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const QUIET_START: &str = "23:00";
pub const QUIET_END: &str = "07:00";
pub const RESERVE_PCT: u8 = 25;
pub const IDLE_MINUTES: u8 = 10;
pub const MAX_CONCURRENT: u8 = 2;
pub const SIZE_MARGINS: [u8; 3] = [5, 15, 30];
pub const COOLDOWN_MINUTES: i64 = 60;
pub const MAX_LIMIT_HITS: usize = 3;
pub const TICK_SECS: u64 = 5;
pub const DETECT_CACHE_SECS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub enabled: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    pub reserve_pct: u8,
    pub idle_minutes: u8,
    pub max_concurrent: u8,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: false,
            quiet_start: QUIET_START.into(),
            quiet_end: QUIET_END.into(),
            reserve_pct: RESERVE_PCT,
            idle_minutes: IDLE_MINUTES,
            max_concurrent: MAX_CONCURRENT,
        }
    }
}

impl Schedule {
    pub fn validate(&self) -> Result<(), String> {
        if minute_of_day(&self.quiet_start).is_none() || minute_of_day(&self.quiet_end).is_none() {
            return Err("quiet hours must be HH:MM".into());
        }
        if self.reserve_pct > 95
            || self.idle_minutes > 120
            || !(1..=4).contains(&self.max_concurrent)
        {
            return Err("reservePct must be 0..95, idleMinutes 0..120, maxConcurrent 1..4".into());
        }
        Ok(())
    }
}

pub fn minute_of_day(value: &str) -> Option<u16> {
    let b = value.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![b[0], b[1], b[3], b[4]].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let hour = (b[0] - b'0') as u16 * 10 + (b[1] - b'0') as u16;
    let minute = (b[3] - b'0') as u16 * 10 + (b[4] - b'0') as u16;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SchedulerState {
    Off,
    Waiting,
    Running,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SchedulerReason {
    QuietHours,
    NotIdle,
    Busy,
    Reserve,
    Cooldown,
    NoTasks,
    EngineUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStatus {
    pub engine: EngineId,
    pub state: SchedulerState,
    pub reason: SchedulerReason,
    pub until: Option<String>,
}
