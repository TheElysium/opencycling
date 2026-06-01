import { invoke } from '@tauri-apps/api/core';

export type DeviceKind = 'Trainer' | 'Hrm';

export type DeviceStatus = 'scanning' | 'not_found' | 'detected' | 'connecting' | 'connected' | 'disconnected' | 'error';

export type BleMetrics = {
  power_w: number | null;
  hr_bpm: number | null;
  cadence_rpm: number | null;
};

class BleState {
  trainerStatus = $state<DeviceStatus>('scanning');
  hrmStatus     = $state<DeviceStatus>('scanning');
  trainerName   = $state<string | null>(null);
  hrmName       = $state<string | null>(null);
  metrics       = $state<BleMetrics | null>(null);
  trainerError  = $state<string | null>(null);
  hrmError      = $state<string | null>(null);
}

export const ble = new BleState();

export async function disconnectDevice(kind: DeviceKind): Promise<void> {
  // TODO(rust): expose `disconnect_trainer` / `disconnect_hrm` commands in src-tauri/src/lib.rs.
  // Until then, this is a UI-side reset only — the BLE actor still holds the connection.
  try {
    await invoke(kind === 'Trainer' ? 'disconnect_trainer' : 'disconnect_hrm');
  } catch {
    // command not yet registered — fall through to local reset
  }
  if (kind === 'Trainer') {
    ble.trainerStatus = 'disconnected';
    ble.trainerName = null;
    ble.trainerError = null;
    if (ble.metrics) ble.metrics = { ...ble.metrics, power_w: null, cadence_rpm: null };
  } else {
    ble.hrmStatus = 'disconnected';
    ble.hrmName = null;
    ble.hrmError = null;
    if (ble.metrics) ble.metrics = { ...ble.metrics, hr_bpm: null };
  }
}
