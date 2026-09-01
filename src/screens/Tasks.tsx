import type { Task } from "../types";
import { ENGINE_LABEL } from "../lib/engines";

function engineLabel(task: Task): string {
  return task.engine.type === "auto"
    ? "auto"
    : ENGINE_LABEL[task.engine.engine];
}

export function Tasks({ tasks }: { tasks: Task[] }) {
  if (tasks.length === 0) {
    return <p className="empty">Nothing queued.</p>;
  }

  return (
    <div className="stack">
      {tasks.map((task) => (
        <div className="task" key={task.id}>
          <b>{task.prompt}</b>
          <span className="meta">
            <span className="dot" data-status={task.status} />
            {task.status} · {task.size.toUpperCase()} · {engineLabel(task)} ·{" "}
            {task.folder.split("/").slice(-1)[0]}
          </span>
        </div>
      ))}
    </div>
  );
}
