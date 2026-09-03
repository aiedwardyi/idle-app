/**
 * Last segment of an absolute path, for either separator.
 *
 * `Task.folder` comes from whichever OS the app is running on, so splitting on
 * "/" alone returns the whole of "C:\Users\you\code\ledger" on Windows. Falls
 * back to the input when there is no segment to take.
 */
export function folderName(folder: string): string {
  const segments = folder.split(/[\\/]+/).filter((part) => part.length > 0);
  return segments.length > 0 ? segments[segments.length - 1] : folder;
}
