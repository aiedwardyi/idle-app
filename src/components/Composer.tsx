import { useState, type KeyboardEvent } from "react";
import { isAbsolute } from "../lib/folder";
import { folderName } from "../lib/paths";

/**
 * Queues a task from typed text, shaped like the real call — add_task takes
 * { prompt, folder, size, engine }.
 *
 * Pro mode adds two things above the box: saved prompts, and a folder this
 * task runs in instead of the inherited one. Both are per-task decisions, so
 * they live here rather than in settings.
 */
type Props = {
  /** `undefined` while the default folder is still resolving. */
  folder: string | undefined;
  pro?: boolean;
  templates?: string[];
  onSubmit: (prompt: string, folder: string) => void;
  onSaveTemplate?: (prompt: string) => void;
  onDeleteTemplate?: (prompt: string) => void;
};

export function Composer({
  folder,
  pro = false,
  templates = [],
  onSubmit,
  onSaveTemplate = () => {},
  onDeleteTemplate = () => {},
}: Props) {
  const [text, setText] = useState("");
  const [override, setOverride] = useState<string | null>(null);
  const [editingFolder, setEditingFolder] = useState(false);

  const target = override ?? folder;
  // A task cannot be queued without an absolute folder to run it in.
  const ready =
    text.trim().length > 0 && target !== undefined && isAbsolute(target);

  const send = () => {
    if (!ready || target === undefined) return;
    onSubmit(text.trim(), target);
    setText("");
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter sends, Shift+Enter is a newline — the convention people already
    // have in their fingers from every chat box.
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  };

  return (
    <form
      className="composer"
      onSubmit={(event) => {
        event.preventDefault();
        send();
      }}
    >
      {pro && (
        <>
          <span className="cbar">
            {templates.map((template) => (
              <span className="tmpl" key={template}>
                <button
                  type="button"
                  className="tmplname"
                  aria-label={`Use template ${template}`}
                  onClick={() => setText(template)}
                >
                  {template}
                </button>
                <button
                  type="button"
                  className="tmplx"
                  aria-label={`Forget template ${template}`}
                  onClick={() => onDeleteTemplate(template)}
                >
                  ×
                </button>
              </span>
            ))}
            <button
              type="button"
              className="linkbtn"
              disabled={text.trim().length === 0}
              aria-label="Save as template"
              onClick={() => onSaveTemplate(text.trim())}
            >
              + save
            </button>
            <span className="grow" />
            <button
              type="button"
              className="linkbtn"
              aria-label="Change folder"
              onClick={() => setEditingFolder((open) => !open)}
            >
              {target === undefined ? "folder…" : folderName(target)}
            </button>
          </span>

          {editingFolder && (
            <input
              className="folderin"
              value={override ?? folder ?? ""}
              placeholder="/absolute/path"
              aria-label="Task folder"
              onChange={(event) => setOverride(event.target.value)}
            />
          )}
        </>
      )}

      <textarea
        className="composer-input"
        rows={1}
        value={text}
        placeholder="Queue a task…"
        aria-label="New task"
        onChange={(event) => setText(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <button
        type="submit"
        className="composer-send"
        disabled={!ready}
        aria-label="Add to queue"
      >
        <svg
          viewBox="0 0 24 24"
          width={14}
          height={14}
          aria-hidden="true"
          fill="none"
          stroke="currentColor"
          strokeWidth={2.4}
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M4 12h14M12 5l7 7-7 7" />
        </svg>
      </button>
      <span className="composer-hint">
        {target === undefined
          ? "finding a folder…"
          : isAbsolute(target)
            ? `runs in ${target}`
            : "folder must be an absolute path"}
      </span>
    </form>
  );
}
