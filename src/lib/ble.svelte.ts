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
