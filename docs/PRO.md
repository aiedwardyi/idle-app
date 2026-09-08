# Pro mode

Two people wanted two different apps: one as simple as possible, one with a
lot of control. Pro mode is how both ship. It is a single boolean in
`Preferences`, off by default, and it decides how much of the app exists.

## Off

Meters and queue, two tabs, no settings screen. Every pro preference is still
stored but none of it is read: `App` passes the shipped defaults instead, and
the root element carries none of the `data-*` attributes the pro CSS keys off.
Turning pro off therefore gives back exactly the app that shipped — not the app
with the extras hidden — and turning it back on restores the user's choices
without them having to be re-picked.

That is a property worth keeping, and `src/App.test.tsx` asserts it directly:
shape attributes come off the root, hidden engines come back, reordered engines
return to the contract's order, and the severity thresholds return to 70/88.

## On

The settings tab appears, with three collapsible groups.

| Group              | What it holds                                                                                                                                            |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Colour & Theme** | always on top, mode, auto dark, accent (presets + a custom hex), theme, opacity, density, font, corners, bar style, window size, meter footer, bars only |
| **Tasks**          | default sort (including manual), show history, saved templates                                                                                           |
| **LLMs**           | per-engine on/off, order, default window, and the tight / near-limit thresholds                                                                          |

The queue grows filter chips, checkboxes with bulk engine and remove, a
per-task size picker, and drag-to-reorder under the manual sort. The composer
grows saved prompts and a per-task folder.

## How it is wired

- **Variants** are root attributes (`data-density`, `data-font`, `data-radius`,
  `data-bar`, `data-bars-only`) and every pro style is written as a selector
  that only matches when one is present. No attribute, shipped look.
- **Values** are inline custom properties: `--w-opacity` and `--accent`. The
  custom accent is the only stored string that reaches CSS, so
  `loadPreferences` accepts nothing but `#rrggbb`.
- **Corner presets** redefine `--w-radius`, and `tokens.css` is unlayered, so
  those two rules sit outside `@layer` on purpose — a layered redeclaration
  loses the cascade to an unlayered one whatever its specificity.
- **Window calls** (`setAlwaysOnTop`, `setSize`) go through `src/lib/window.ts`,
  which swallows failure so the same build runs under `vite dev` and jsdom.
  `setSize` needs `core:window:allow-set-size` in the capability file.

## Starting work

`run_now` takes one task and returns one Run, so that is what the UI offers: a
play button on each task row, in both modes. There is no scheduler — nothing
picks a next task — so a finished run leaves the engine idle until someone
presses play again. That is the truth about what the app does today, and the
meter row's transport says the same thing: it is a **stop** button, live only
while that engine has a process running, disabled and labelled idle otherwise.
A play button there would imply a queue runner that does not exist yet.

A run's terminal state belongs to the store: the backend flips the task to
done, failed or discarded after the process exits, so `finished` and `error`
events trigger a re-read of `list_tasks` rather than an optimistic guess.

This is also the answer to "why are all the meters blank?" — `meter_state` is
seeded one row per engine per window with zeros and NULLs, and only a real run
fills it in. `usedPct` returns null when neither `remainingPct` nor
`capacityEst` is known, and the row says "no estimate" instead of inventing a
number.

## Rules this bends, on purpose

- `CLAUDE.md` caps a screen at three controls. The default view now has exactly
  three — the pro switch and two tabs — which is what pays for the settings
  screen having many. Simple is the default; depth is opt-in.
- `docs/COLOR.md` rule 9 says a status never rests on colour alone. **Bars only**
  hides the footer, which leaves the fill colour carrying severity by itself.
  It is off by default and the reset countdown is still spelled out.
