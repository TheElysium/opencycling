import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

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
  await invoke('export_session_tcx', { id, path });
}
