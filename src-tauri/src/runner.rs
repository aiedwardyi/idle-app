//! The single chokepoint every subprocess in this app passes through.
//!
//! Every adapter calls [`Runner::spawn`] and nothing else. The env scrub
//! lives here and only here: if it lived in the adapters, one of them would
//! eventually forget it and a user would get an API bill for usage they
//! thought was on their subscription. This is also why `Command::new` must
//! never appear outside this file.
//!
//! The runner is generic over the engine. It parses stdout as JSON lines and
//! emits contract [`RunEvent`] values; per-engine field mapping belongs in
//! the adapters.

use crate::contract::{ExitReason, RunEvent, SCRUBBED_ENV_VARS};
use crate::engines::RunCtx;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::ffi::OsStr;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

pub struct Runner;

/// Handle to a running child: the event stream, a kill switch, and the
/// final exit reason.
pub struct RunHandle {
    events: Option<BoxStream<'static, RunEvent>>,
    kill_tx: Option<oneshot::Sender<()>>,
    done_rx: Option<oneshot::Receiver<ExitReason>>,
    pid: Option<u32>,
}

impl RunHandle {
    /// The [`RunEvent`] stream for this run. Yields `started`, then one
    /// event per stdout/stderr line, then exactly one terminal `finished`.
    /// Callable once.
    pub fn take_events(&mut self) -> BoxStream<'static, RunEvent> {
        self.events.take().expect("events stream already taken")
    }

    /// Stop the running child on demand. Idempotent; the run ends with
    /// [`ExitReason::Cancelled`].
    pub fn kill(&mut self) {
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Final exit reason. Resolves after the child has exited and the event
    /// stream has ended. Call at most once.
    pub async fn wait(&mut self) -> ExitReason {
        let rx = self.done_rx.take().expect("wait already called");
        rx.await
            .expect("runner task dropped without sending exit reason")
    }

    /// OS pid of the child, used by tests to assert the process actually died.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }
}

impl Runner {
    /// Launch `program` with `args` as a headless subprocess.
    ///
    /// - cwd is `ctx.cwd`, never the app's own cwd.
    /// - Every var in [`SCRUBBED_ENV_VARS`] is stripped from the child env.
    /// - stdin is null; stdout and stderr are piped and read concurrently.
    /// - The child is killed when `ctx.timeout_secs` elapses.
    /// - Every emitted event carries `ctx.run_id`, because up to four runs
    ///   share one global event channel.
    pub fn spawn(
        program: impl AsRef<OsStr>,
        args: &[impl AsRef<OsStr>],
        ctx: &RunCtx,
    ) -> std::io::Result<RunHandle> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(&ctx.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for var in SCRUBBED_ENV_VARS {
            cmd.env_remove(var);
        }
        let child = cmd.spawn()?;
        let pid = child.id();

        // Unbounded: current engines emit a thin JSON-lines stream, so there
        // is no backpressure. Benchmark before attaching a high-volume adapter.
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (kill_tx, kill_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();

        tokio::spawn(drive(
            child,
            ctx.run_id.clone(),
            Duration::from_secs(ctx.timeout_secs),
            event_tx,
            kill_rx,
            done_tx,
        ));

        let events = futures::stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        })
        .boxed();

        Ok(RunHandle {
            events: Some(events),
            kill_tx: Some(kill_tx),
            done_rx: Some(done_rx),
            pid,
        })
    }
}

/// Own the child from spawn to exit: pump stdout and stderr, apply the
/// timeout, honor the kill switch, and report the final exit reason.
async fn drive(
    mut child: Child,
    run_id: String,
    timeout: Duration,
    tx: mpsc::UnboundedSender<RunEvent>,
    mut kill_rx: oneshot::Receiver<()>,
    done_tx: oneshot::Sender<ExitReason>,
) {
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let _ = tx.send(RunEvent::Started {
        run_id: run_id.clone(),
    });

    // Drain stderr concurrently with stdout so a child that fills one pipe
    // while we block on the other cannot deadlock. stderr lines surface as
    // Error events: loud and traceable, per the malformed line rationale.
    let stderr_tx = tx.clone();
    let stderr_run_id = run_id.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_tx.send(RunEvent::Error {
                run_id: stderr_run_id.clone(),
                message: format!("stderr: {}", line.trim_end_matches('\r')),
            });
        }
    });

    let deadline = tokio::time::Instant::now() + timeout;
    let mut lines = BufReader::new(stdout).lines();
    // Set when the child is stopped early; overrides the exit status.
    let mut early_exit: Option<ExitReason> = None;
    let mut kill_seen = false;
    let mut stdout_done = false;

    // Keep pumping stdout until EOF even after a kill, so buffered output
    // is not lost. After stdout EOF, stay in this loop: only stop reading
    // stdout, not the lifecycle. A child that closes stdout and then blocks
    // must still be killable and must still hit the deadline.
    // Lines::next_line is cancel safe, so a select loop does not drop
    // partial reads.
    let status = loop {
        tokio::select! {
            line = lines.next_line(), if !stdout_done => match line {
                Ok(Some(line)) => emit_line(&tx, &run_id, &line),
                Ok(None) | Err(_) => stdout_done = true,
            },
            _kill = &mut kill_rx, if !kill_seen && early_exit.is_none() => {
                kill_seen = true;
                // Ok: explicit kill(). Err: RunHandle was dropped. drive owns
                // the Child, so kill_on_drop cannot fire from the handle;
                // treat either as cancellation and stop the child.
                early_exit = Some(ExitReason::Cancelled);
                stop_child(&mut child);
            },
            _ = tokio::time::sleep_until(deadline), if early_exit.is_none() => {
                early_exit = Some(ExitReason::Timeout);
                stop_child(&mut child);
            },
            status = child.wait() => break status,
        }
    };
    let _ = stderr_task.await;

    let reason = early_exit.unwrap_or(match &status {
        Ok(status) if status.success() => ExitReason::Ok,
        _ => ExitReason::Failed,
    });
    let _ = tx.send(RunEvent::Finished {
        run_id,
        ok: reason == ExitReason::Ok,
    });
    let _ = done_tx.send(reason);
}

/// Kill the child process.
///
/// Windows landmine: this is TerminateProcess on the direct child only, so
/// grandchildren survive. The engine CLIs run as the direct child here, so
/// this is acceptable for now; a Win32 job object is the full fix if an
/// engine ever spawns its own long-lived children.
fn stop_child(child: &mut Child) {
    let _ = child.start_kill();
}

/// Turn one stdout line into one RunEvent.
///
/// Malformed line policy (fixed): a line that fails to parse as JSON emits
/// an Error event carrying the raw line, and the run continues. Never skip
/// silently, never abort: the meter is fed by these events, and a silent
/// skip means the meter drifts with no trace of why.
fn emit_line(tx: &mpsc::UnboundedSender<RunEvent>, run_id: &str, raw: &str) {
    // Windows children end lines with \r\n; trim the \r before parsing.
    let line = raw.trim_end_matches('\r');
    let event = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(_) => RunEvent::Output {
            run_id: run_id.to_string(),
            line: line.to_string(),
        },
        Err(err) => RunEvent::Error {
            run_id: run_id.to_string(),
            message: format!("{line}: {err}"),
        },
    };
    let _ = tx.send(event);
}
