## What / why
Closes #

## How to test
1.
2.

## Screenshot (UI PRs)

## Checklist
- [ ] Follows docs/CONTRACT.md types and event names
- [ ] Every subprocess goes through `Runner::spawn` (env scrubbed)
- [ ] No credential file paths anywhere (grep for `.claude`, `.codex`, `.gemini`, `.grok`, `auth.json`)
- [ ] No vendor binary modified or vendored; official installer only
- [ ] Errors reach the UI; no silent failures
- [ ] Tests for parsers and state changes
- [ ] UI: 3 controls or fewer per screen; works at widget size
- [ ] Tested on macOS and Windows, or says which one is untested
- [ ] No secrets, tokens, or personal paths in the diff
- [ ] Under 400 changed lines, or the PR says why not
