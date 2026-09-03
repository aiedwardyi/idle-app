//! Engine adapters: one module per vendor CLI, all behind [`Engine`].
//!
//! An adapter owns three things and nothing else: how to detect its binary,
//! how to launch a headless run, and how to translate that CLI's stdout
//! into contract [`RunEvent`]s. Process plumbing (env scrub, cwd, timeout,
//! kill) lives in the Runner; the shared pump in [`EngineRun`] lives here so
//! every adapter gets identical stream semantics from one implementation.

use crate::contract::{DetectInfo, EngineId, ExitReason, LimitWindow, RunEvent, Task};
use crate::runner::RunHandle;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

pub mod claude;

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
    // Task.folder stays String: it crosses the IPC wire and must stay serde/ts-rs friendly.
    pub cwd: PathBuf,
    pub timeout_secs: u64,
}

#[async_trait]
pub trait Engine: Send + Sync {
    /// Stable identifier. No I/O.
    fn id(&self) -> EngineId;

    /// Reads exit codes only, never files. Async so a CLI probe does not block the runtime.
    async fn detect(&self) -> Result<DetectInfo>;

    /// Runs the vendor installer unmodified. Async so install does not block the runtime.
    async fn install(&self) -> Result<()>;

    /// Runs the vendor login command unmodified. Async so login does not block the runtime.
    async fn login(&self) -> Result<()>;

    /// Launches the task through Runner::spawn. Returns the run handle: the
    /// event stream plus kill and the final ExitReason. Must be called from
    /// inside a tokio runtime.
    fn run(&self, task: &Task, ctx: RunCtx) -> Result<EngineRun>;

    /// Default limit windows for this engine. No vendor query.
    fn windows(&self) -> Vec<LimitWindow>;
}

/// Every engine adapter the app knows about, in contract order. Codex,
/// Antigravity and Grok join here as their adapters land.
pub fn registry() -> Vec<Box<dyn Engine>> {
    vec![Box::new(claude::ClaudeEngine::new())]
}

/// Per-engine translation of the Runner's stream. The pump in
/// [`EngineRun::from_handle`] feeds it every stdout line the Runner parsed as
/// JSON and asks it once, at the end, to settle the exit reason.
///
/// Runner `Started` and `Error` events (stderr lines, malformed JSON) pass
/// through untouched; the Runner's `Finished` is replaced by the mapper's
/// closing events plus one `Finished` that agrees with the final reason.
pub trait EventMapper: Send + 'static {
    /// One well-formed JSON line from stdout. Return every event it implies,
    /// in order. Unrecognised input must come back as an `Error` carrying the
    /// raw line, never as nothing.
    fn map_line(&mut self, run_id: &str, line: &str) -> Vec<RunEvent>;

    /// Called once after the child has exited with the Runner's reason.
    /// Returns any closing events (for example `LimitHit`) and the final
    /// reason the run should report.
    fn finish(&mut self, run_id: &str, runner_reason: ExitReason) -> (Vec<RunEvent>, ExitReason);
}

/// A running task. Mirrors the Runner's `RunHandle` on purpose so the
/// scheduler learns one shape: `take_events` once, `kill` at will, `wait`
/// for the reason. Dropping it cancels the child.
pub struct EngineRun {
    events: Option<BoxStream<'static, RunEvent>>,
    kill_tx: Option<oneshot::Sender<()>>,
    done_rx: Option<oneshot::Receiver<ExitReason>>,
}

impl EngineRun {
    /// Wrap a Runner handle with a mapper. The pump task owns the handle so
    /// the child lives exactly as long as this run does.
    pub fn from_handle(handle: RunHandle, run_id: String, mapper: impl EventMapper) -> Self {
        // Unbounded, matching the Runner's channel one hop upstream: the
        // mapping is about one event per line, so the backlog is the
        // Runner's plus a handful. Bounding here would either drop events
        // (invariant: nothing is dropped silently) or move the same backlog
        // into the Runner. Revisit together with runner.rs if an adapter
        // ever streams high volume.
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (kill_tx, kill_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        tokio::spawn(pump(handle, run_id, mapper, event_tx, kill_rx, done_tx));
        let events = futures::stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        })
        .boxed();
        Self {
            events: Some(events),
            kill_tx: Some(kill_tx),
            done_rx: Some(done_rx),
        }
    }

    /// The event stream: `started`, then the adapter's events, then exactly
    /// one `finished`.
    ///
    /// # Panics
    ///
    /// Panics on a second call. The stream is owned once, like the Runner's
    /// `RunHandle::take_events`.
    pub fn take_events(&mut self) -> BoxStream<'static, RunEvent> {
        self.events.take().expect("events stream already taken")
    }

    /// Stop the child. Idempotent; the run ends with `Cancelled`.
    pub fn kill(&mut self) {
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Final exit reason. Resolves after the stream has ended, whether or
    /// not anyone drained it.
    ///
    /// # Panics
    ///
    /// Panics on a second call: the reason is delivered once, like the
    /// Runner's `RunHandle::wait`. Keep the value if you need it twice.
    pub async fn wait(&mut self) -> ExitReason {
        let rx = self.done_rx.take().expect("wait already called");
        rx.await.expect("pump dropped without sending exit reason")
    }
}

async fn pump(
    mut handle: RunHandle,
    run_id: String,
    mut mapper: impl EventMapper,
    tx: mpsc::UnboundedSender<RunEvent>,
    mut kill_rx: oneshot::Receiver<()>,
    done_tx: oneshot::Sender<ExitReason>,
) {
    let mut events = handle.take_events();
    let mut kill_seen = false;
    loop {
        tokio::select! {
            event = events.next() => match event {
                Some(RunEvent::Finished { .. }) | None => break,
                Some(RunEvent::Output { line, .. }) => {
                    for mapped in mapper.map_line(&run_id, &line) {
                        let _ = tx.send(mapped);
                    }
                }
                Some(other) => {
                    let _ = tx.send(other);
                }
            },
            // Ok: explicit kill(). Err: the EngineRun was dropped. Both stop
            // the child; the Runner reports Cancelled either way.
            _ = &mut kill_rx, if !kill_seen => {
                kill_seen = true;
                handle.kill();
            }
        }
    }
    let runner_reason = handle.wait().await;
    let (closing, reason) = mapper.finish(&run_id, runner_reason);
    for event in closing {
        let _ = tx.send(event);
    }
    let _ = tx.send(RunEvent::Finished {
        run_id,
        ok: reason == ExitReason::Ok,
    });
    let _ = done_tx.send(reason);
}
