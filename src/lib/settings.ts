import { invoke } from '@tauri-apps/api/core';

export type Settings = {
  ftp_w: number;
  max_hr_bpm: number;
  workout_path: string;
  strava_proxy_url: string;
};

export function getSettings(): Promise<Settings> {
  return invoke<Settings>('get_settings');
}

export function updateSettings(settings: Settings): Promise<void> {
  return invoke('update_settings', { settings });
}
