import { useState, type KeyboardEvent } from "react";

/**
 * Queues a task from typed text. Shaped like the real call — add_task takes
 * { prompt, folder, size, engine } — but there is no handler yet, so what this
 * produces lives in memory for the session. Deliberately not persisted to
 * localStorage: tasks are app data and belong in the SQLite store, and a
 * shadow copy here would diverge from it the moment the real one arrives.
 */
type Props = {
  folder: string;
  onSubmit: (prompt: string) => void;
};

export function Composer({ folder, onSubmit }: Props) {
  const [text, setText] = useState("");
  const ready = text.trim().length > 0;

  const send = () => {
    if (!ready) return;
    onSubmit(text.trim());
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
      <span className="composer-hint">runs in {folder}</span>
    </form>
  );
}
