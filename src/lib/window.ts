/**
 * Always-on-top is a real window call, so it only works inside Tauri. The
 * import is dynamic and the failure is swallowed on purpose: the same build
 * runs in `vite dev` and in jsdom under test, where there is no window to
 * pin. Requires core:window:allow-set-always-on-top in the capability file.
 */
export async function applyAlwaysOnTop(value: boolean): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setAlwaysOnTop(value);
  } catch {
    // Not running under Tauri.
  }
}

/**
 * Size presets. Same swallow-the-failure reasoning as above; requires
 * core:window:allow-set-size in the capability file.
 */
export async function applySize(width: number, height: number): Promise<void> {
  try {
    const { getCurrentWindow, LogicalSize } =
      await import("@tauri-apps/api/window");
    await getCurrentWindow().setSize(new LogicalSize(width, height));
  } catch {
    // Not running under Tauri.
  }
}
