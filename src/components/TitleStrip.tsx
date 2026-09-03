import { Icon } from "./Icon";
import { SCREEN_TITLE, type Screen } from "../lib/screens";

/**
 * Three tabs, always visible, active one marked. The previous version toggled
 * the same icon to go back, which works but tells the user nothing — there was
 * no visible way back to the meters.
 */
const TABS: { screen: Screen; icon: "meters" | "queue" | "settings" }[] = [
  { screen: "widget", icon: "meters" },
  { screen: "tasks", icon: "queue" },
  { screen: "settings", icon: "settings" },
];

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
        {TABS.map((tab) => (
          <button
            key={tab.screen}
            type="button"
            className="iconbtn"
            aria-pressed={screen === tab.screen}
            aria-label={SCREEN_TITLE[tab.screen]}
            onClick={() => onOpen(tab.screen)}
          >
            <Icon name={tab.icon} />
          </button>
        ))}
      </span>
    </div>
  );
}
