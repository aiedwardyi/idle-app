/** The widget has three screens; the shell owns which one is showing. */
export type Screen = "widget" | "tasks" | "settings";

export const SCREEN_TITLE: Record<Screen, string> = {
  widget: "Idle",
  tasks: "Queue",
  settings: "Settings",
};
