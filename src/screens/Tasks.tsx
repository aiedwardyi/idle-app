import type { EngineChoice, Task } from "../types";
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

const CHOICES: { value: string; label: string; dot: string | null }[] = [
  { value: "auto", label: "Auto", dot: null },
  ...ENGINE_ORDER.map((engine) => ({
    value: engine,
    label: ENGINE_LABEL[engine],
    dot: ENGINE_DOT[engine],
  })),
];

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
  onEngine: (id: string, engine: EngineChoice) => void;
  onPriority: (id: string, priority: Priority) => void;
  onRemove: (id: string) => void;
  onSort: (sort: Sort) => void;
};

export function Tasks({
  tasks,
  loading,
  priorities,
  sort,
  onEngine,
  onPriority,
  onRemove,
  onSort,
}: Props) {
  const bar = (
    <div className="sortbar" role="group" aria-label="Sort queue">
      {SORTS.map((option) => (
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

  if (loading || tasks.length === 0) {
    return (
      <div className="stack">
        {bar}
        <p className="empty">{loading ? "Loading…" : "Nothing queued."}</p>
      </div>
    );
  }

  return (
    <div className="stack">
      {bar}
      {sortTasks(tasks, sort, priorities).map((task) => {
        const value = choiceValue(task.engine);
        const dot = CHOICES.find((choice) => choice.value === value)?.dot;
        const priority = priorities[task.id] ?? DEFAULT_PRIORITY;

        return (
          <div className="task" key={task.id} data-priority={priority}>
            <span className="taskbody">
              <b>{task.prompt}</b>
              <span className="meta">
                <span className="dot" data-status={task.status} />
                {task.status} · {task.size.toUpperCase()} ·{" "}
                {folderName(task.folder)}
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
