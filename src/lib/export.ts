import { save } from '@tauri-apps/plugin-dialog';
import { commands } from './bindings';

/** Opens a save dialog and writes the session as a .tcx file. */
export async function exportSessionTcx(
  id: number,
  workoutName: string,
  startedAt: string,
): Promise<void> {
  // Strip characters that are illegal in file names on Windows/macOS/Linux.
  const safeName = workoutName.replace(/[/\\:*?"<>|]/g, '_');
  const defaultName = `${safeName}_${startedAt.slice(0, 10)}.tcx`;
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: 'TCX', extensions: ['tcx'] }],
  });
  if (!path) return; // cancelled
  await commands.exportSessionTcx(id, path);
}
