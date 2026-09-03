# Claude adapter fixtures

Replayed through `fake_cli` by `tests/claude.rs`. No test needs a real
`claude` binary or a network.

Captured on Windows 11 with `claude` 2.1.259 (native install) using:

    claude -p --output-format stream-json --verbose --permission-mode acceptEdits --permission-prompts none -- "<prompt>"

Scrubbing applied to every real capture: the task folder became
`C:\work\task`, session ids were replaced with a fixed placeholder, hook
outputs were replaced with `[hook output scrubbed]`, the `system/init` line
was trimmed to a short generic tool and plugin list, and one vendor em dash
in a tool result was normalized to a hyphen. No credential paths, emails, or
org ids remain.

| File                           | Origin                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------ |
| `run_success.jsonl`            | Real. Task wrote `hello.txt` via the Write tool and replied `done`. Exit 0.                       |
| `run_unrecognized_event.jsonl` | Real stream above plus one copy of the `rate_limit_event` line with `type` renamed to `usage_forecast`. |
| `run_error_result.jsonl`       | Real. `--model bogus-model-xyz`, so `result.is_error` is true with zero usage. Exit 1.            |
| `run_limit_hit.synthetic.jsonl`| SYNTHETIC. Built from a public 2.1.62 issue log (`rate_limit_event` rejected, `assistant` with `error: rate_limit`, `result` is_error) plus the documented optional `resetsAt` (unix seconds). Replace with a real capture when a limit is actually hit. |
| `auth_status_signed_out.json`  | Real output of `claude auth status` while the CLI reported signed out. Exit 1.                    |
| `auth_status_signed_in.json`   | Real output of `claude auth status` while signed in, identifiers replaced. Exit 0.                |
