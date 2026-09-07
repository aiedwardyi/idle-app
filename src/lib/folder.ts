import { homeDir } from "@tauri-apps/api/path";

/**
 * `Task.folder` must be an absolute path, so it cannot be invented. The newest
 * task's folder is the best guess for the next one; on a fresh install there is
 * no task to inherit from, so fall back to the home directory rather than
 * guessing a project path. A folder picker belongs in a later PR.
 */
export async function defaultFolder(
  newest: string | undefined,
): Promise<string> {
  if (newest !== undefined && newest.length > 0) return newest;
  try {
    return await homeDir();
  } catch {
    return "";
  }
}
