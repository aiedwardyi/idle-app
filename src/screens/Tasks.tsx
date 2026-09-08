import { useMemo, useState } from "react";
import type { EngineChoice, Task, TaskSize } from "../types";
import { ENGINE_DOT, ENGINE_LABEL, ENGINE_ORDER } from "../lib/engines";
import { folderName } from "../lib/paths";
import {
  DEFAULT_PRIORITY,
  PRIORITIES,
  PRIORITY_BARS,
  PRIORITY_LABEL,
  type Priority,
} from "../lib/priority";
import { Icon } from "../components/Icon";
import { SORTS, SORT_LABEL, sortTasks, type Sort } from "../lib/sort";
import type { Filters } from "../lib/filters";

const CHOICES: { value: string; label: string; dot: string | null }[] = [
  { value: "auto", label: "Auto", dot: null },
  ...ENGINE_ORDER.map((engine) => ({
    value: engine,
    label: ENGINE_LABEL[engine],
    dot: ENGINE_DOT[engine],
  })),
];

const SIZES: TaskSize[] = ["s", "m", "l"];

function choiceValue(engine: EngineChoice): string {
  return engine.type === "auto" ? "auto" : engine.engine;
}

function toChoice(value: string): EngineChoice {
  if (value === "auto") return { type: "auto" };
  const engine = ENGINE_ORDER.find((id) => id === value);
  return engine ? { type: "fixed", engine } : { type: "auto" };
}

type Props = {
  tasks: Task[];
  /** First load has not returned yet. Distinct from an empty queue. */
  loading: boolean;
  priorities: Record<string, Priority>;
  sort: Sort;
  pro: boolean;
  /** Task ids in the order the user dragged them, for the manual sort. */
  order: string[];
  filters: Filters;
  /** Task id -> run id, for the ones this window has in flight. */
  running: Record<string, string>;
  onFilters: (filters: Filters) => void;
  onRun: (task: Task) => void;
  onStopTask: (runId: string) => void;
  onEngine: (id: string, engine: EngineChoice) => void;
  onPriority: (id: string, priority: Priority) => void;
  onSize: (id: string, size: TaskSize) => void;
  onRemove: (id: string) => void;
  onRemoveMany: (ids: string[]) => void;
  onEngineMany: (ids: string[], engine: EngineChoice) => void;
  onReorder: (ids: string[]) => void;
  onSort: (sort: Sort) => void;
};

export function Tasks({
  tasks,
  loading,
  priorities,
  sort,
  pro,
  order,
  filters,
  running,
  onFilters,
  onRun,
  onStopTask,
  onEngine,
  onPriority,
  onSize,
  onRemove,
  onRemoveMany,
  onEngineMany,
  onReorder,
  onSort,
}: Props) {
  // Selection and drag are transient: nothing to remember once you leave.
  const [picked, setPicked] = useState<string[]>([]);
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);

  // Manual only makes sense once you can drag, so it is a pro-only option.
  const sorts = pro ? SORTS : SORTS.filter((option) => option !== "manual");

  const folders = useMemo(
    () => [...new Set(tasks.map((task) => task.folder))],
    [tasks],
  );
  const statuses = useMemo(
    () => [...new Set(tasks.map((task) => task.status))],
    [tasks],
  );

  const shown = useMemo(() => {
    const kept = tasks.filter(
      (task) =>
        (filters.engine === null ||
          choiceValue(task.engine) === filters.engine) &&
        (filters.status === null || task.status === filters.status) &&
        (filters.folder === null || task.folder === filters.folder),
    );
    return sortTasks(kept, sort, priorities, order);
  }, [tasks, filters, sort, priorities, order]);

  const toggle = (id: string) =>
    setPicked((current) =>
      current.includes(id)
        ? current.filter((other) => other !== id)
        : [...current, id],
    );

  const drop = (targetId: string) => {
    const ids = shown.map((task) => task.id);
    if (dragging === null || dragging === targetId) return;
    const from = ids.indexOf(dragging);
    const to = ids.indexOf(targetId);
    if (from === -1 || to === -1) return;
    ids.splice(to, 0, ...ids.splice(from, 1));
    onReorder(ids);
    setDragging(null);
    setOver(null);
  };

  const bar = (
    <div className="sortbar" role="group" aria-label="Sort queue">
      {sorts.map((option) => (
        <button
          key={option}
          type="button"
          aria-pressed={sort === option}
          onClick={() => onSort(option)}
        >
          {SORT_LABEL[option]}
        </button>
      ))}
    </div>
  );

  const chip = (
    key: string,
    label: string,
    on: boolean,
    dot: string | null,
    press: () => void,
  ) => (
    <button
      key={key}
      type="button"
      className="chip"
      aria-pressed={on}
      onClick={press}
    >
      {dot !== null && (
        <span className="dot" style={{ background: dot }} aria-hidden="true" />
      )}
      {label}
    </button>
  );

  // Only the axes that have something to choose between: a lone folder is not
  // a filter, it is noise.
  const chips = pro && (
    <div className="chiprow" role="group" aria-label="Filter queue">
      {CHOICES.filter((choice) =>
        tasks.some((task) => choiceValue(task.engine) === choice.value),
      ).map((choice) =>
        chip(
          `e-${choice.value}`,
          choice.label,
          filters.engine === choice.value,
          choice.dot,
          () =>
            onFilters({
              ...filters,
              engine: filters.engine === choice.value ? null : choice.value,
            }),
        ),
      )}
      {statuses.length > 1 &&
        statuses.map((status) =>
          chip(`s-${status}`, status, filters.status === status, null, () =>
            onFilters({
              ...filters,
              status: filters.status === status ? null : status,
            }),
          ),
        )}
      {folders.length > 1 &&
        folders.map((folder) =>
          chip(
            `f-${folder}`,
            folderName(folder),
            filters.folder === folder,
            null,
            () =>
              onFilters({
                ...filters,
                folder: filters.folder === folder ? null : folder,
              }),
          ),
        )}
    </div>
  );

  const bulk = pro && picked.length > 0 && (
    <div className="bulkbar">
      <span className="count">{picked.length} selected</span>
      <select
        aria-label="Engine for selected"
        value=""
        onChange={(event) => {
          onEngineMany(picked, toChoice(event.target.value));
          setPicked([]);
        }}
      >
        <option value="">Engine…</option>
        {CHOICES.map((choice) => (
          <option key={choice.value} value={choice.value}>
            {choice.label}
          </option>
        ))}
      </select>
      <button
        type="button"
        className="linkbtn"
        aria-label="Remove selected"
        onClick={() => {
          onRemoveMany(picked);
          setPicked([]);
        }}
      >
        Remove
      </button>
    </div>
  );

  if (loading || shown.length === 0) {
    return (
      <div className="stack">
        {bar}
        {chips}
        <p className="empty">
          {loading
            ? "Loading…"
            : tasks.length === 0
              ? "Nothing queued."
              : "Nothing matches those filters."}
        </p>
      </div>
    );
  }

  return (
    <div className="stack">
      {bar}
      {chips}
      {bulk}
      {shown.map((task) => {
        const value = choiceValue(task.engine);
        const dot = CHOICES.find((choice) => choice.value === value)?.dot;
        const priority = priorities[task.id] ?? DEFAULT_PRIORITY;
        const draggable = pro && sort === "manual";
        const runId = running[task.id];

        return (
          <div
            className="task"
            key={task.id}
            data-priority={priority}
            data-dragging={dragging === task.id}
            data-dropping={over === task.id}
            draggable={draggable}
            onDragStart={() => setDragging(task.id)}
            onDragEnd={() => {
              setDragging(null);
              setOver(null);
            }}
            onDragOver={(event) => {
              if (!draggable) return;
              event.preventDefault();
              setOver(task.id);
            }}
            onDrop={(event) => {
              event.preventDefault();
              drop(task.id);
            }}
          >
            {pro && (
              <input
                type="checkbox"
                className="taskpick"
                checked={picked.includes(task.id)}
                aria-label={`Select ${task.prompt}`}
                onChange={() => toggle(task.id)}
              />
            )}
            {draggable && (
              <span className="grip" aria-hidden="true">
                ⠿
              </span>
            )}

            {/* Work starts here, on a task, because run_now takes a task.
                Once it is running the same button stops it — the run this
                window started is the one thing it can honestly cancel. A
                task left `running` by some other window has neither. */}
            <button
              type="button"
              className="taskplay"
              data-running={runId !== undefined}
              disabled={runId === undefined && task.status !== "queued"}
              aria-label={
                runId !== undefined
                  ? `Stop ${task.prompt}`
                  : task.status === "queued"
                    ? `Run ${task.prompt}`
                    : `${task.prompt} is already ${task.status}`
              }
              onClick={() =>
                runId === undefined ? onRun(task) : onStopTask(runId)
              }
            >
              <Icon name={runId === undefined ? "play" : "pause"} size={10} />
            </button>

            <span className="taskbody">
              <b>{task.prompt}</b>
              <span className="meta">
                <span className="dot" data-status={task.status} />
                {task.status} ·{" "}
                {pro ? (
                  <select
                    className="winpick"
                    aria-label={`Size for ${task.prompt}`}
                    value={task.size}
                    onChange={(event) =>
                      onSize(task.id, event.target.value as TaskSize)
                    }
                  >
                    {SIZES.map((size) => (
                      <option key={size} value={size}>
                        {size.toUpperCase()}
                      </option>
                    ))}
                  </select>
                ) : (
                  task.size.toUpperCase()
                )}{" "}
                · {folderName(task.folder)}
              </span>
            </span>

            {/* Engine over priority, stacked at the right edge. Priority is
                ordinal, so it is encoded by how many bars are lit rather than
                by hue — state and engine already spend the colour budget. */}
            <span className="picks">
              <label className="pick engine">
                <span
                  className="dot"
                  style={{ background: dot ?? "var(--w-ink-3)" }}
                />
                <select
                  aria-label={`Engine for ${task.prompt}`}
                  value={value}
                  onChange={(event) =>
                    onEngine(task.id, toChoice(event.target.value))
                  }
                >
                  {CHOICES.map((choice) => (
                    <option key={choice.value} value={choice.value}>
                      {choice.label}
                    </option>
                  ))}
                </select>
              </label>

              <label className="pick prio" data-bars={PRIORITY_BARS[priority]}>
                <Icon name="levels" size={12} />
                <select
                  aria-label={`Priority for ${task.prompt}`}
                  value={priority}
                  onChange={(event) =>
                    onPriority(task.id, event.target.value as Priority)
                  }
                >
                  {PRIORITIES.map((option) => (
                    <option key={option} value={option}>
                      {PRIORITY_LABEL[option]}
                    </option>
                  ))}
                </select>
              </label>
            </span>

            <button
              type="button"
              className="taskdrop"
              aria-label={`Remove ${task.prompt}`}
              onClick={() => onRemove(task.id)}
            >
              <svg
                viewBox="0 0 24 24"
                width={11}
                height={11}
                aria-hidden="true"
                fill="none"
                stroke="currentColor"
                strokeWidth={2.6}
                strokeLinecap="round"
              >
                <path d="M6 6l12 12M18 6L6 18" />
              </svg>
            </button>
          </div>
        );
      })}
    </div>
  );
}
