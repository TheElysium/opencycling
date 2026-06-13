import { invoke } from '@tauri-apps/api/core';

export type StravaStatus = {
  connected: boolean;
  athlete_id: number | null;
  athlete_name: string | null;
  auto_upload: boolean;
};

export function stravaStatus(): Promise<StravaStatus> {
  return invoke<StravaStatus>('strava_status');
}

export function stravaConnect(): Promise<StravaStatus> {
  return invoke<StravaStatus>('strava_connect');
}

export function stravaDisconnect(): Promise<void> {
  return invoke('strava_disconnect');
}

export function stravaSetAutoUpload(enabled: boolean): Promise<void> {
  return invoke('strava_set_auto_upload', { enabled });
}

/**
 * Uploads a session to Strava. With `force`, bypasses the dedup guard and
 * creates a new activity (use when the original was deleted on Strava).
 * Returns the created Strava activity id.
 */
export function uploadSessionToStrava(sessionId: number, force = false): Promise<number> {
  return invoke<number>('upload_session_to_strava', { sessionId, force });
}

export function activityUrl(activityId: number): string {
  return `https://www.strava.com/activities/${activityId}`;
}
