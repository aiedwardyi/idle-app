import { Icon } from "./Icon";
import { SCREEN_TITLE, type Screen } from "../lib/screens";

/**
 * Meters and queue are the app. Settings only exists in pro mode: with
 * nothing to configure there is nothing to open, and dropping the tab keeps
 * the default view at the three controls CLAUDE.md asks for.
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
  pro: boolean;
  onOpen: (screen: Screen) => void;
  onTogglePro: () => void;
};

export function TitleStrip({
  title,
  status,
  screen,
  pro,
  onOpen,
  onTogglePro,
}: Props) {
  const tabs = pro ? TABS : TABS.filter((tab) => tab.screen !== "settings");

  return (
    <div className="strip" data-tauri-drag-region>
      {/* Left, away from the tab cluster, because it is not a fourth
          destination: it changes what the other tabs contain. Shaped like
          every other toggle in the app so it reads as on/off rather than as
          a badge. */}
      <button
        type="button"
        className="proswitch"
        aria-pressed={pro}
        aria-label="Pro mode"
        title={
          pro ? "Pro mode on — settings unlocked" : "Pro mode off — simple view"
        }
        onClick={onTogglePro}
      >
        <span className="swtrack" aria-hidden="true" />
        Pro
      </button>

      <span className="brand">
        <b>{title}</b>
        <span>{status}</span>
      </span>
      <span className="actions">
        {tabs.map((tab) => (
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
