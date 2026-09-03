//! Claude Code adapter. The template the other three engines copy.
//!
//! Verified against `claude` 2.1.259 (native install) on Windows. Flags used
//! for a run:
//!
//! ```text
//! claude -p --output-format stream-json --verbose \
//!        --permission-mode acceptEdits --permission-prompts none -- <prompt>
//! ```
//!
//! `acceptEdits` auto-accepts file edits inside the working directory and
//! nothing broader; `--permission-prompts none` denies anything that would
//! have asked. `bypassPermissions` and the two `dangerously-skip` flags are
//! banned and a test asserts they never appear. `--verbose` is no longer
//! required for stream-json on this version but older ones needed it and it
//! is harmless, so it stays. The prompt goes after `--` as one argv entry:
//! the Runner nulls stdin, and `--` keeps a prompt that starts with `-` from
//! being read as a flag.
//!
//! Stream policy: every stdout line the CLI prints reaches the caller exactly
//! once. Recognised types (`system`, `assistant`, `user`, `rate_limit_event`,
//! `result`) pass through verbatim as `Output`, so nothing is lost and the
//! raw text is always available next to any derived event. Any other type
//! becomes an `Error` carrying the raw line, never a silent drop.
//!
//! Detect reads exit codes and stdout only. `--version` gives the version,
//! `auth status` exits 0 signed in and 1 signed out. Both cost zero tokens.
//! The adapter never looks at any file the CLI keeps.

use super::{Engine, EngineError, EngineRun, EventMapper, Result, RunCtx};
use crate::contract::{
    default_windows, DetectInfo, EngineId, ExitReason, LimitWindow, RunEvent, Task,
};
use crate::runner::{RunHandle, Runner};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::io::ErrorKind;
use std::path::PathBuf;

/// What the vendor puts on PATH.
const DEFAULT_PROGRAM: &str = "claude";
/// Probes are quick; the bound only guards a hung binary.
const PROBE_TIMEOUT_SECS: u64 = 60;

pub struct ClaudeEngine {
    program: OsString,
}

impl Default for ClaudeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeEngine {
    /// Launches `claude` from PATH, falling back to the vendor's documented
    /// native install location when PATH does not have it (GUI launches on
    /// macOS routinely miss `~/.local/bin`).
    pub fn new() -> Self {
        Self::with_program(DEFAULT_PROGRAM)
    }

    /// Launch a specific binary instead. Tests point this at `fake_cli`.
    pub fn with_program(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
        }
    }

    /// Programs to try, in order. The native executable wins over anything
    /// else because `Command` resolves `claude` to `claude.exe` on Windows
    /// and never to an npm `.cmd` shim.
    fn candidates(&self) -> Vec<OsString> {
        let mut out = vec![self.program.clone()];
        if self.program == DEFAULT_PROGRAM {
            if let Some(path) = native_install_path() {
                out.push(path.into_os_string());
            }
        }
        out
    }

    /// The only way this adapter starts a process: Runner::spawn on the
    /// first candidate that exists. NotFound moves to the next candidate;
    /// any other error is returned as-is.
    fn spawn(&self, args: &[impl AsRef<OsStr>], ctx: &RunCtx) -> std::io::Result<RunHandle> {
        let candidates = self.candidates();
        let mut last = None;
        for program in &candidates {
            match Runner::spawn(program, args, ctx) {
                Err(err) if err.kind() == ErrorKind::NotFound => last = Some(err),
                other => return other,
            }
        }
        Err(last.expect("at least one candidate"))
    }

    /// Run a short probe to completion and keep its stdout lines and exit
    /// reason. Plain text lines come back from the Runner as `Error` events
    /// (its malformed JSON policy), so they are recovered here.
    async fn probe(&self, args: &[&str], run_id: &str) -> std::io::Result<Probe> {
        let ctx = RunCtx {
            run_id: run_id.to_string(),
            cwd: std::env::temp_dir(),
            timeout_secs: PROBE_TIMEOUT_SECS,
        };
        let mut handle = self.spawn(args, &ctx)?;
        let mut lines = Vec::new();
        let mut events = handle.take_events();
        while let Some(event) = events.next().await {
            match event {
                RunEvent::Output { line, .. } => lines.push(line),
                RunEvent::Error { message, .. } => lines.extend(raw_stdout_line(&message)),
                _ => {}
            }
        }
        let reason = handle.wait().await;
        Ok(Probe { lines, reason })
    }
}

struct Probe {
    lines: Vec<String>,
    reason: ExitReason,
}

#[async_trait]
impl Engine for ClaudeEngine {
    fn id(&self) -> EngineId {
        EngineId::Claude
    }

    async fn detect(&self) -> Result<DetectInfo> {
        let version = match self.probe(&["--version"], "claude-detect-version").await {
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(DetectInfo {
                    installed: false,
                    version: None,
                    signed_in: false,
                })
            }
            Err(err) => {
                return Err(EngineError::Detect(format!(
                    "could not launch claude: {err}"
                )))
            }
            Ok(probe) if probe.reason == ExitReason::Ok => parse_version(&probe.lines),
            Ok(_) => None,
        };
        let auth = self
            .probe(&["auth", "status"], "claude-detect-auth")
            .await
            .map_err(|err| {
                EngineError::Detect(format!("could not run claude auth status: {err}"))
            })?;
        Ok(DetectInfo {
            installed: true,
            version,
            signed_in: auth.reason == ExitReason::Ok,
        })
    }

    async fn install(&self) -> Result<()> {
        Err(EngineError::Install(
            "not implemented until PR-19; install Claude Code with the vendor's official installer"
                .into(),
        ))
    }

    async fn login(&self) -> Result<()> {
        Err(EngineError::Login(
            "not implemented until PR-19; run `claude auth login` in a terminal".into(),
        ))
    }

    fn run(&self, task: &Task, ctx: RunCtx) -> Result<EngineRun> {
        // Task.size is ignored on purpose: the scheduler maps size to an
        // effort level in PR-14. cwd comes from ctx, which the scheduler sets
        // to Task.folder; that folder is the edit boundary acceptEdits sees.
        let args = run_args(&task.prompt);
        let handle = self
            .spawn(&args, &ctx)
            .map_err(|err| EngineError::Run(format!("could not launch claude: {err}")))?;
        Ok(EngineRun::from_handle(
            handle,
            ctx.run_id,
            ClaudeStream::default(),
        ))
    }

    fn windows(&self) -> Vec<LimitWindow> {
        default_windows(EngineId::Claude)
    }
}

/// Headless run flags. See the module docs for why each one is here.
fn run_args(prompt: &str) -> Vec<OsString> {
    [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--permission-mode",
        "acceptEdits",
        "--permission-prompts",
        "none",
        "--",
        prompt,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

/// `<home>/.local/bin/claude[.exe]`, the vendor's native install target.
/// Built from the home env var only; nothing is read from disk here.
fn native_install_path() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    let mut path = PathBuf::from(home)
        .join(".local")
        .join("bin")
        .join("claude");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    Some(path)
}

/// `2.1.259 (Claude Code)` -> `2.1.259`.
fn parse_version(lines: &[String]) -> Option<String> {
    let first = lines.first()?.split_whitespace().next()?;
    first
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| first.to_string())
}

/// Recover a plain text stdout line from the Runner's `Error` message. The
/// Runner formats a non-JSON stdout line as `"{line}: {err}"` and a stderr
/// line as `"stderr: {line}"`; only the former is stdout.
fn raw_stdout_line(message: &str) -> Option<String> {
    if message.starts_with("stderr") || message.starts_with("stdout read failed") {
        return None;
    }
    message.rsplit_once(": ").map(|(line, _)| line.to_string())
}

/// Translates one Claude stream-json run. See the module docs for the
/// pass-through policy; the decisions that need a paragraph are inline.
#[derive(Default)]
struct ClaudeStream {
    limit_hit: bool,
    resets_at: Option<String>,
    error_seen: bool,
}

impl EventMapper for ClaudeStream {
    fn map_line(&mut self, run_id: &str, line: &str) -> Vec<RunEvent> {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(err) => return vec![error(run_id, format!("{line}: {err}"))],
        };
        let output = RunEvent::Output {
            run_id: run_id.to_string(),
            line: line.to_string(),
        };
        match value["type"].as_str() {
            Some("system") | Some("user") => vec![output],
            Some("assistant") => {
                // A rejected request comes back as a synthetic assistant
                // message tagged with the error kind.
                if value["error"].as_str() == Some("rate_limit") {
                    self.limit_hit = true;
                }
                vec![output]
            }
            Some("rate_limit_event") => {
                // Emitted on every run with status allowed, allowed_warning
                // or rejected. Only rejected is a hit. resetsAt is optional
                // and has been seen absent on a rejection, so it is kept as a
                // hint and the result text is a second source below.
                let info = &value["rate_limit_info"];
                if info["status"].as_str() == Some("rejected") {
                    self.limit_hit = true;
                    if let Some(ts) = epoch_to_rfc3339(&info["resetsAt"]) {
                        self.resets_at = Some(ts);
                    }
                }
                vec![output]
            }
            Some("result") => {
                let mut events = vec![output];
                // Usage source of truth: result.usage, once. Per-message
                // assistant usage overlaps it and its output_tokens are
                // streaming partials (observed 17 + 1 against a result total
                // of 118), so summing messages would both double count and
                // under count. cache is creation + read.
                match &value["usage"] {
                    Value::Object(usage) => events.push(RunEvent::Usage {
                        run_id: run_id.to_string(),
                        input: u64_of(usage.get("input_tokens")),
                        output: u64_of(usage.get("output_tokens")),
                        cache: u64_of(usage.get("cache_creation_input_tokens"))
                            + u64_of(usage.get("cache_read_input_tokens")),
                    }),
                    _ => events.push(error(run_id, format!("result without usage: {line}"))),
                }
                if value["is_error"].as_bool().unwrap_or(false) {
                    let text = value["result"].as_str().unwrap_or("");
                    if self.limit_hit || is_limit_text(text) {
                        // Older builds put the reset time in the text as
                        // `...|<epoch>`; use it only if the event had none.
                        self.limit_hit = true;
                        if self.resets_at.is_none() {
                            self.resets_at = epoch_from_text(text);
                        }
                    } else {
                        self.error_seen = true;
                        events.push(error(run_id, format!("claude reported an error: {text}")));
                    }
                }
                events
            }
            _ => vec![error(run_id, format!("unrecognized claude event: {line}"))],
        }
    }

    fn finish(&mut self, run_id: &str, runner_reason: ExitReason) -> (Vec<RunEvent>, ExitReason) {
        if self.limit_hit {
            // A limit is not an error: LimitHit, then Finished, reason LimitHit.
            let hit = RunEvent::LimitHit {
                run_id: run_id.to_string(),
                resets_at: self.resets_at.take(),
            };
            return (vec![hit], ExitReason::LimitHit);
        }
        let reason = if self.error_seen && runner_reason == ExitReason::Ok {
            // The CLI said is_error; a zero exit code does not outrank that.
            ExitReason::Failed
        } else {
            runner_reason
        };
        (Vec::new(), reason)
    }
}

fn error(run_id: &str, message: String) -> RunEvent {
    RunEvent::Error {
        run_id: run_id.to_string(),
        message,
    }
}

fn u64_of(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or(0)
}

/// Phrases the CLI has used for a subscription limit, past and present.
fn is_limit_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "rate limit reached",
        "usage limit reached",
        "hit your limit",
    ]
    .iter()
    .any(|phrase| lower.contains(phrase))
}

/// `Claude AI usage limit reached|1751702400` -> the epoch after the bar.
fn epoch_from_text(text: &str) -> Option<String> {
    let (_, tail) = text.rsplit_once('|')?;
    let secs: u64 = tail.trim().parse().ok()?;
    Some(rfc3339_from_unix(secs))
}

/// A JSON number of unix seconds (observed) or milliseconds (documented in
/// one SDK) to RFC3339. Anything past year 5000 in seconds is really ms.
fn epoch_to_rfc3339(value: &Value) -> Option<String> {
    let raw = value.as_f64()?;
    if raw <= 0.0 {
        return None;
    }
    let secs = if raw > 100_000_000_000.0 {
        raw / 1000.0
    } else {
        raw
    };
    Some(rfc3339_from_unix(secs as u64))
}

/// Unix seconds to `YYYY-MM-DDTHH:MM:SSZ` without a date crate.
/// Civil-from-days per Howard Hinnant's algorithm.
fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_rfc3339_handles_epoch_leap_day_and_observed_reset() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_from_unix(1_788_403_800), "2026-09-03T02:50:00Z");
    }

    #[test]
    fn epoch_field_accepts_seconds_or_milliseconds_and_rejects_junk() {
        let s = epoch_to_rfc3339(&serde_json::json!(1_788_403_800));
        assert_eq!(s.as_deref(), Some("2026-09-03T02:50:00Z"));
        let ms = epoch_to_rfc3339(&serde_json::json!(1_788_403_800_000u64));
        assert_eq!(ms, s);
        assert_eq!(epoch_to_rfc3339(&serde_json::json!(null)), None);
        assert_eq!(epoch_to_rfc3339(&serde_json::json!("soon")), None);
        assert_eq!(epoch_to_rfc3339(&serde_json::json!(0)), None);
    }

    #[test]
    fn reset_time_embedded_in_text_is_parsed() {
        assert_eq!(
            epoch_from_text("Claude AI usage limit reached|1788403800").as_deref(),
            Some("2026-09-03T02:50:00Z")
        );
        assert_eq!(epoch_from_text("API Error: Rate limit reached"), None);
        assert_eq!(epoch_from_text("x|not-a-number"), None);
    }

    #[test]
    fn limit_phrases_match_case_insensitively() {
        assert!(is_limit_text("API Error: Rate limit reached"));
        assert!(is_limit_text("Claude AI usage limit reached|1"));
        assert!(is_limit_text("You've hit your limit"));
        assert!(!is_limit_text("Not logged in"));
    }

    #[test]
    fn version_is_first_token_when_numeric() {
        assert_eq!(
            parse_version(&["2.1.259 (Claude Code)".to_string()]).as_deref(),
            Some("2.1.259")
        );
        assert_eq!(parse_version(&["error: unknown option".to_string()]), None);
        assert_eq!(parse_version(&[]), None);
    }

    #[test]
    fn raw_stdout_line_is_recovered_from_runner_error_only_for_stdout() {
        assert_eq!(
            raw_stdout_line("2.1.259 (Claude Code): expected value at line 1 column 1").as_deref(),
            Some("2.1.259 (Claude Code)")
        );
        assert_eq!(
            raw_stdout_line("  \"loggedIn\": true,: expected value at line 1 column 3").as_deref(),
            Some("  \"loggedIn\": true,")
        );
        assert_eq!(raw_stdout_line("stderr: warning"), None);
        assert_eq!(raw_stdout_line("stdout read failed: boom"), None);
    }

    #[test]
    fn run_args_use_narrow_permissions_and_separate_prompt() {
        let args = run_args("-looks like a flag");
        let args: Vec<&str> = args.iter().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args[0], "-p");
        assert_eq!(&args[args.len() - 2..], &["--", "-looks like a flag"]);
        assert!(!args.iter().any(|a| a.contains("dangerously")));
        assert!(!args.iter().any(|a| a.contains("bypassPermissions")));
    }

    #[test]
    fn native_install_path_is_home_local_bin() {
        let path = native_install_path().expect("home is set in tests");
        let text = path.to_string_lossy();
        assert!(text.ends_with(if cfg!(windows) {
            "\\.local\\bin\\claude.exe"
        } else {
            "/.local/bin/claude"
        }));
    }

    #[test]
    fn mapper_emits_usage_once_and_flags_unknown_types() {
        let mut stream = ClaudeStream::default();
        let result = r#"{"type":"result","is_error":false,"result":"ok","usage":{"input_tokens":4,"output_tokens":118,"cache_creation_input_tokens":21986,"cache_read_input_tokens":45376}}"#;
        let events = stream.map_line("r", result);
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1],
            RunEvent::Usage {
                run_id: "r".into(),
                input: 4,
                output: 118,
                cache: 67_362,
            }
        );
        let odd = stream.map_line("r", r#"{"type":"usage_forecast"}"#);
        assert_eq!(
            odd,
            vec![RunEvent::Error {
                run_id: "r".into(),
                message: r#"unrecognized claude event: {"type":"usage_forecast"}"#.into(),
            }]
        );
        assert_eq!(stream.finish("r", ExitReason::Ok), (vec![], ExitReason::Ok));
    }

    #[test]
    fn mapper_turns_text_only_limit_into_limit_hit_with_text_reset() {
        let mut stream = ClaudeStream::default();
        let result = r#"{"type":"result","is_error":true,"result":"Claude AI usage limit reached|1788403800","usage":{"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}"#;
        let events = stream.map_line("r", result);
        assert_eq!(
            events.len(),
            2,
            "output and zero usage, no error: {events:?}"
        );
        let (closing, reason) = stream.finish("r", ExitReason::Failed);
        assert_eq!(reason, ExitReason::LimitHit);
        assert_eq!(
            closing,
            vec![RunEvent::LimitHit {
                run_id: "r".into(),
                resets_at: Some("2026-09-03T02:50:00Z".into()),
            }]
        );
    }

    #[test]
    fn mapper_reports_missing_usage_instead_of_inventing_zero() {
        let mut stream = ClaudeStream::default();
        let events = stream.map_line("r", r#"{"type":"result","is_error":false}"#);
        assert!(
            matches!(&events[1], RunEvent::Error { message, .. } if message.starts_with("result without usage"))
        );
    }
}
