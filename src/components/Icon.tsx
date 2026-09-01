import type { ReactElement } from "react";

type IconName =
  | "play"
  | "pause"
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
  queue: <path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01" />,
  settings: (
    <path d="M12 3a2 2 0 0 1 2 2v.4a1.7 1.7 0 0 0 2.6 1.1l.3-.2a2 2 0 0 1 2.6 3l-.3.3a1.7 1.7 0 0 0 1 2.9H21a2 2 0 0 1 0 4h-.4a1.7 1.7 0 0 0-1 2.9l.3.3a2 2 0 0 1-2.6 3l-.3-.2a1.7 1.7 0 0 0-2.6 1.1V21a2 2 0 0 1-4 0v-.4a1.7 1.7 0 0 0-2.6-1.1l-.3.2a2 2 0 0 1-2.6-3l.3-.3a1.7 1.7 0 0 0-1-2.9H3a2 2 0 0 1 0-4h.4a1.7 1.7 0 0 0 1-2.9l-.3-.3a2 2 0 0 1 2.6-3l.3.2A1.7 1.7 0 0 0 9.6 5.4V5a2 2 0 0 1 2.4-2z" />
  ),
  ok: <path d="M5 13l4 4L19 7" />,
  warn: <path d="M12 3l9 17H3zM12 9v5M12 17h.01" />,
  near: <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM12 7v5l3 2" />,
  blocked: <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM5.6 5.6l12.8 12.8" />,
  unknown: (
    <path d="M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM9.6 9.3a2.5 2.5 0 1 1 3.6 2.3c-.7.4-1.2 1-1.2 1.8v.2M12 17.2h.01" />
  ),
};

const FILLED: IconName[] = ["play", "pause"];

export function Icon({ name, size = 14 }: { name: IconName; size?: number }) {
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
      strokeWidth={2.2}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {PATHS[name]}
    </svg>
  );
}
