import { useEffect, useState } from "react";
import type { MeterState } from "../types";
import { getMeters, listenMeterUpdate } from "../types/ipc";
import { message } from "../lib/errors";

/**
 * Meters from the store, kept current by the `meter_update` channel. PR-08
 * builds the bar itself; this is the read that stops the screen showing
 * invented numbers in the meantime.
 */
export function useMeters() {
  const [meters, setMeters] = useState<MeterState[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const initial = await getMeters();
        if (live) setMeters(initial);
      } catch (caught) {
        if (live) {
          setError(message(caught));
          setMeters([]);
        }
      }
      try {
        const stop = await listenMeterUpdate((update) => {
          // One meter per (engine, window): replace that pair, keep the rest.
          setMeters((current) => {
            const rest = (current ?? []).filter(
              (m) =>
                !(m.engine === update.engine && m.window === update.window),
            );
            return [...rest, update];
          });
        });
        if (live) unlisten = stop;
        else stop();
      } catch {
        // No event channel means no live updates; the initial read still stands.
      }
    })();

    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  return { meters, error };
}
