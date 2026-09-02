import type { EngineChoice, Task } from "../types";
import { ENGINE_DOT, ENGINE_LABEL, ENGINE_ORDER } from "../lib/engines";
import { folderName } from "../lib/paths";

/** "auto" plus one option per engine, matching EngineChoice on the wire. */
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
  onEngine: (id: string, engine: EngineChoice) => void;
};

export function Tasks({ tasks, onEngine }: Props) {
  if (tasks.length === 0) {
    return <p className="empty">Nothing queued.</p>;
  }

  return (
    <div className="stack">
      {tasks.map((task) => {
        const value = choiceValue(task.engine);
        const dot = CHOICES.find((choice) => choice.value === value)?.dot;

        return (
          <div className="task" key={task.id}>
            <label className="enginepick">
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

            <span className="taskbody">
              <b>{task.prompt}</b>
              <span className="meta">
                <span className="dot" data-status={task.status} />
                {task.status} · {task.size.toUpperCase()} ·{" "}
                {folderName(task.folder)}
              </span>
            </span>
          </div>
        );
      })}
    </div>
  );
}
