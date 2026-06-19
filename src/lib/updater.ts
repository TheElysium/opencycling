import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { ask } from '@tauri-apps/plugin-dialog';

/**
 * Checks the configured updater endpoint for a newer release. When one is
 * available, asks the user before downloading, installing and relaunching.
 *
 * Note: `check()` always resolves to `null` under `tauri dev` (no installed
 * version to compare against); a real test requires a packaged build.
 */
export async function checkForUpdate(): Promise<void> {
  try {
    const update = await check();
    if (!update) return;

    const accepted = await ask(
      `Version ${update.version} is available.\n\n${update.body ?? ''}\n\nDownload and install now?`,
      { title: 'Update available', kind: 'info', okLabel: 'Update', cancelLabel: 'Later' },
    );
    if (!accepted) return;

    await update.downloadAndInstall();
    await relaunch();
  } catch (err) {
    console.error('Update check failed', err);
  }
}
