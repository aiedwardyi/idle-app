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
  priorities: Record<string, Priority>;
  onEngine: (id: string, engine: EngineChoice) => void;
  onPriority: (id: string, priority: Priority) => void;
};

export function Tasks({ tasks, priorities, onEngine, onPriority }: Props) {
  if (tasks.length === 0) {
    return <p className="empty">Nothing queued.</p>;
  }

  return (
    <div className="stack">
      {tasks.map((task) => {
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
          </div>
        );
      })}
    </div>
  );
}
