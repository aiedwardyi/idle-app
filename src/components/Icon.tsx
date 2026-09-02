import type { ReactElement } from "react";

type IconName =
  | "play"
  | "pause"
  | "meters"
  | "queue"
  | "settings"
  | "ok"
  | "warn"
  | "near"
  | "blocked"
  | "unknown";

const PATHS: Record<IconName, ReactElement> = {
  play: <path d="M8 5.14v13.72L19 12z" fill="currentColor" />,
  pause: <path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor" />,
  // three bars of different length — the meters screen, at a glance
  meters: <path d="M4 7h16M4 12h11M4 17h6" />,
  queue: <path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01" />,
  // sliders, not a gear: gear teeth turn to mush below about 20px
  settings: <path d="M4 8h9M17 8h3M4 16h3M11 16h9M15 5.5v5M8 13.5v5" />,
  ok: <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM8 12.2l2.6 2.6L16 9.4" />,
  warn: <path d="M12 3l9 17H3zM12 9v5M12 17h.01" />,
  near: <path d="M3.5 17a8.5 8.5 0 1 1 17 0M12 12l4.5-3.6" />,
  blocked: <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM5.6 5.6l12.8 12.8" />,
  unknown: (
    <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM9.6 9.3a2.5 2.5 0 1 1 3.6 2.3c-.7.4-1.2 1-1.2 1.8v.2M12 17.2h.01" />
  ),
};

const FILLED: IconName[] = ["play", "pause"];

export function Icon({
  name,
  size = 14,
  strokeWidth = 2.2,
}: {
  name: IconName;
  size?: number;
  strokeWidth?: number;
}) {
  const filled = FILLED.includes(name);
  return (
    <svg
      viewBox="0 0 24 24"
      data-icon={name}
      width={size}
      height={size}
      aria-hidden="true"
      fill="none"
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {PATHS[name]}
    </svg>
  );
}
