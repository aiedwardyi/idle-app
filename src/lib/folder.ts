import { homeDir } from "@tauri-apps/api/path";

/**
 * `Task.folder` is an absolute path per the contract. Checked rather than
 * assumed: the value reaches the store through add_task, and a relative path
 * there is a data error nobody sees again.
 */
export function isAbsolute(path: string): boolean {
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path);
}

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
