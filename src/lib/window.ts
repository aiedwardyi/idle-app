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
