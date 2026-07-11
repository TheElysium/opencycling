import { commands } from './bindings';
import type { Settings } from './bindings';

// Generated from the Rust struct (src-tauri/src/db/types.rs).
export type { Settings } from './bindings';

export function getSettings(): Promise<Settings> {
  return commands.getSettings();
}

export async function updateSettings(settings: Settings): Promise<void> {
  await commands.updateSettings(settings);
}
