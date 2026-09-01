use crate::contract::{DetectInfo, EngineId, LimitWindow, RunEvent, Task};
use futures::stream::BoxStream;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("detect failed: {0}")]
    Detect(String),
    #[error("install failed: {0}")]
    Install(String),
    #[error("login failed: {0}")]
    Login(String),
    #[error("run failed: {0}")]
    Run(String),
}

pub struct RunCtx {
    pub run_id: String,
    pub cwd: String,
    pub timeout_secs: u64,
}

pub trait Engine: Send + Sync {
    /// Stable identifier. No I/O.
    fn id(&self) -> EngineId;

    /// Reads exit codes only, never files.
    fn detect(&self) -> Result<DetectInfo>;

    /// Runs the vendor installer unmodified.
    fn install(&self) -> Result<()>;

    /// Runs the vendor login command unmodified.
    fn login(&self) -> Result<()>;

    /// Goes through Runner::spawn (to come).
    fn run(&self, task: &Task, ctx: RunCtx) -> Result<BoxStream<'static, RunEvent>>;

    /// Default limit windows for this engine. No vendor query.
    fn windows(&self) -> Vec<LimitWindow>;
}
