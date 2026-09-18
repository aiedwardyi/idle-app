import { useState } from "react";
import type { Run } from "../types";
import {
  latestRun,
  resultDuration,
  resultState,
  resultWord,
} from "../lib/results";

type Props = {
  /** Every run recorded against this task, newest attempt included. */
  runs: Run[];
  prompt: string;
  /** Live transcript tails keyed by run id. Empty for runs that predate it. */
  tails: Record<string, string[]>;
};

/**
 * What happened after this task ran — whenever that was.
 *
 * It is a band across the foot of the task card rather than a screen of its
 * own, because nothing in the app knows when the user will come back: they
 * might queue three tasks and look in ten minutes or after a night's sleep.
 * The answer therefore lives where the task lives. A card with no runs has no
 * band at all, so a fresh queue looks exactly as it did.
 *
 * Amber for work still going, green for a clean exit, red for anything that
 * wants a person. The word sits beside the colour (rule 9) and the transcript
 * tail waits behind the click, so the default view gains no controls.
 *
 * Undo is drawn and disabled: it needs the workspace guard, which is not
 * built. Settling its shape now means wiring it later moves nothing.
 */
export function TaskResult({ runs, prompt, tails }: Props) {
  const [open, setOpen] = useState(false);
  const run = latestRun(runs);
  if (run === null) return null;

  const state = resultState(run);
  const duration = resultDuration(run);
  const lines = tails[run.id] ?? [];

  return (
    <>
      <button
        type="button"
        className="taskfoot"
        data-state={state}
        aria-expanded={open}
        aria-label={`${prompt} — ${resultWord(run)}. Details`}
        onClick={() => setOpen((shown) => !shown)}
      >
        <span className="dot" data-state={state} />
        <span className="tfword">
          {resultWord(run)}
          {duration !== null && ` · ${duration}`}
        </span>
        <span className="tfhint">{open ? "hide" : "details"}</span>
      </button>

      {open && (
        <div className="taskdetail">
          {lines.length > 0 ? (
            <pre className="restail">{lines.join("\n")}</pre>
          ) : (
            <p className="resempty">
              No output kept — this run finished before the window opened.
            </p>
          )}
          <button
            type="button"
            className="linkbtn"
            disabled
            title="Undo needs the workspace guard, which is not built yet"
            aria-label={`Undo ${prompt}`}
          >
            Undo
          </button>
        </div>
      )}
    </>
  );
}
