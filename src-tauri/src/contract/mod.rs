use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCRUBBED_ENV_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "XAI_API_KEY",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum EngineId {
    Claude,
    Codex,
    Antigravity,
    Grok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TaskSize {
    S,
    M,
    L,
}

impl TaskSize {
    pub const fn timeout_secs(self) -> u64 {
        match self {
            TaskSize::S => 600,
            TaskSize::M => 1800,
            TaskSize::L => 3600,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", content = "engine", rename_all = "camelCase")]
pub enum EngineChoice {
    Auto,
    Fixed(EngineId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Queued,
    Running,
    Done,
    Failed,
    Discarded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub folder: String,
    pub size: TaskSize,
    pub engine: EngineChoice,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RunEvent {
    Started {
        run_id: String,
    },
    Output {
        run_id: String,
        line: String,
    },
    Usage {
        run_id: String,
        input: u64,
        output: u64,
        cache: u64,
    },
    LimitHit {
        run_id: String,
        resets_at: Option<String>,
    },
    Finished {
        run_id: String,
        ok: bool,
    },
    Error {
        run_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ExitReason {
    Ok,
    Failed,
    LimitHit,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub engine: EngineId,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_reason: Option<ExitReason>,
    pub usage: Usage,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LimitWindowKind {
    FiveHour,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub kind: LimitWindowKind,
    pub hours: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MeterState {
    pub engine: EngineId,
    pub window: LimitWindowKind,
    pub used: Usage,
    pub capacity_est: Option<u64>,
    pub calibrated: bool,
    pub remaining_pct: Option<f64>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DetectInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub signed_in: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub engine: EngineId,
    pub detect: DetectInfo,
}

pub fn default_windows(engine: EngineId) -> Vec<LimitWindow> {
    match engine {
        EngineId::Claude | EngineId::Codex => vec![
            LimitWindow {
                kind: LimitWindowKind::FiveHour,
                hours: 5,
            },
            LimitWindow {
                kind: LimitWindowKind::Weekly,
                hours: 168,
            },
        ],
        EngineId::Antigravity => vec![LimitWindow {
            kind: LimitWindowKind::Daily,
            hours: 24,
        }],
        EngineId::Grok => vec![LimitWindow {
            kind: LimitWindowKind::Weekly,
            hours: 168,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn default_windows_per_engine() {
        let claude = default_windows(EngineId::Claude);
        assert_eq!(claude.len(), 2);
        assert_eq!(claude[0].kind, LimitWindowKind::FiveHour);
        assert_eq!(claude[0].hours, 5);
        assert_eq!(claude[1].kind, LimitWindowKind::Weekly);
        assert_eq!(claude[1].hours, 168);
        assert_eq!(default_windows(EngineId::Codex), claude);
        let anti = default_windows(EngineId::Antigravity);
        assert_eq!(
            anti,
            vec![LimitWindow {
                kind: LimitWindowKind::Daily,
                hours: 24,
            }]
        );
        assert_eq!(
            default_windows(EngineId::Grok),
            vec![LimitWindow {
                kind: LimitWindowKind::Weekly,
                hours: 168,
            }]
        );
    }

    #[test]
    fn run_event_round_trip() {
        let cases = [
            (
                RunEvent::Started {
                    run_id: "r1".into(),
                },
                json!({"type": "started", "runId": "r1"}),
            ),
            (
                RunEvent::Output {
                    run_id: "r1".into(),
                    line: "hi".into(),
                },
                json!({"type": "output", "runId": "r1", "line": "hi"}),
            ),
            (
                RunEvent::Usage {
                    run_id: "r1".into(),
                    input: 1,
                    output: 2,
                    cache: 3,
                },
                json!({"type": "usage", "runId": "r1", "input": 1, "output": 2, "cache": 3}),
            ),
            (
                RunEvent::LimitHit {
                    run_id: "r1".into(),
                    resets_at: None,
                },
                json!({"type": "limitHit", "runId": "r1", "resetsAt": null}),
            ),
            (
                RunEvent::Finished {
                    run_id: "r1".into(),
                    ok: true,
                },
                json!({"type": "finished", "runId": "r1", "ok": true}),
            ),
            (
                RunEvent::Error {
                    run_id: "r1".into(),
                    message: "no".into(),
                },
                json!({"type": "error", "runId": "r1", "message": "no"}),
            ),
        ];
        for (event, expected) in cases {
            let value = to_value(&event).unwrap();
            assert_eq!(value, expected);
            let back: RunEvent = serde_json::from_value(value).unwrap();
            assert_eq!(back, event);
        }
    }

    #[test]
    fn engine_choice_round_trip() {
        let auto = to_value(EngineChoice::Auto).unwrap();
        assert_eq!(auto, json!({"type": "auto"}));
        assert_eq!(
            serde_json::from_value::<EngineChoice>(auto).unwrap(),
            EngineChoice::Auto
        );
        let fixed = to_value(EngineChoice::Fixed(EngineId::Claude)).unwrap();
        assert_eq!(fixed, json!({"type": "fixed", "engine": "claude"}));
        assert_eq!(
            serde_json::from_value::<EngineChoice>(fixed).unwrap(),
            EngineChoice::Fixed(EngineId::Claude)
        );
    }

    #[test]
    fn export_typescript_bindings() {
        use ts_rs::{Config, TS};
        let cfg = Config::from_env();
        Task::export_all(&cfg).unwrap();
        Run::export_all(&cfg).unwrap();
        RunEvent::export_all(&cfg).unwrap();
        MeterState::export_all(&cfg).unwrap();
        EngineStatus::export_all(&cfg).unwrap();
        EngineChoice::export_all(&cfg).unwrap();
        LimitWindow::export_all(&cfg).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/types/generated");
        let runevent = std::fs::read_to_string(dir.join("RunEvent.ts")).unwrap();
        assert!(
            !runevent.contains('&'),
            "RunEvent.ts must be a plain union, got {runevent}"
        );
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("ts") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lf = text.replace("\r\n", "\n");
            if lf != text {
                std::fs::write(&path, lf).unwrap();
            }
        }
    }
}
