# idle-app

Desktop widget (Tauri 2 + React) that runs queued tasks through official AI CLIs before subscription limits reset.
Read docs/CONTRACT.md before writing code.

Rules that are never optional:
- All subprocesses go through Runner::spawn. Never call Command::new directly.
- Never read or reference credential files (~/.claude, ~/.codex, ~/.gemini, ~/.grok).
- Never modify or vendor a CLI binary. Install via the vendor's official installer only.
- No network calls from the app except the update check.
- UI screens have 3 controls or fewer.

Conventions: conventional commits, tests for every parser and state change, PRs under 400 lines.
Run tests: cargo test (src-tauri), npm test (src).

## Layout

docs/, src/types/, src-tauri/src/contract/, src-tauri/src/engines/, src-tauri/src/store/.
