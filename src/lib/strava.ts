import { commands } from './bindings';
import type { StravaStatus } from './bindings';

// Generated from the Rust struct (src-tauri/src/strava/types.rs).
export type { StravaStatus } from './bindings';

export function stravaStatus(): Promise<StravaStatus> {
  return commands.stravaStatus();
}

export function stravaConnect(): Promise<StravaStatus> {
  return commands.stravaConnect();
}

export async function stravaDisconnect(): Promise<void> {
  await commands.stravaDisconnect();
}

export async function stravaSetAutoUpload(enabled: boolean): Promise<void> {
  await commands.stravaSetAutoUpload(enabled);
}

/**
 * Uploads a session to Strava. With `force`, bypasses the dedup guard and
 * creates a new activity (use when the original was deleted on Strava).
 * Returns the created Strava activity id.
 */
export function uploadSessionToStrava(sessionId: number, force = false): Promise<number> {
  return commands.uploadSessionToStrava(sessionId, force);
}

export function activityUrl(activityId: number): string {
  return `https://www.strava.com/activities/${activityId}`;
}
