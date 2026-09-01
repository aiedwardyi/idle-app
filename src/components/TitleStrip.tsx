import { Icon } from "./Icon";
import type { Screen } from "../lib/screens";

type Props = {
  title: string;
  status: string;
  screen: Screen;
  onOpen: (screen: Screen) => void;
};

export function TitleStrip({ title, status, screen, onOpen }: Props) {
  return (
    <div className="strip" data-tauri-drag-region>
      <span className="brand">
        <b>{title}</b>
        <span>{status}</span>
      </span>
      <span className="actions">
        <button
          type="button"
          className="iconbtn"
          aria-pressed={screen === "tasks"}
          aria-label="Task queue"
          onClick={() => onOpen("tasks")}
        >
          <Icon name="queue" />
        </button>
        <button
          type="button"
          className="iconbtn"
          aria-pressed={screen === "settings"}
          aria-label="Settings"
          onClick={() => onOpen("settings")}
        >
          <Icon name="settings" />
        </button>
      </span>
    </div>
  );
}
