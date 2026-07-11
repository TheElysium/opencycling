import { invoke } from '@tauri-apps/api/core';
import { toMessage } from '$lib/format';

export type DeviceKind = 'Trainer' | 'Hrm';

export type DeviceStatus = 'scanning' | 'not_found' | 'detected' | 'connecting' | 'connected' | 'disconnected' | 'error';

// Wire status of the backend `ble_reconnect` event.
export type ReconnectStatus = 'reconnecting' | 'reconnected' | 'failed';
// Live reconnect state per device; `null` when no reconnection is in flight.
export type ReconnectState = { status: ReconnectStatus; attempt: number } | null;

// Lowercase `device` field of the `ble_reconnect` / `ble_disconnected` events.
// Internally the store speaks `DeviceKind`; convert at the event boundary.
export type DeviceWire = 'trainer' | 'hrm';
export function kindFromWire(device: DeviceWire): DeviceKind {
  return device === 'trainer' ? 'Trainer' : 'Hrm';
}

export type BleMetrics = {
  power_w: number | null;
  hr_bpm: number | null;
  cadence_rpm: number | null;
};

// How long the "reconnected" success state lingers before the affordance clears.
const RECONNECTED_LINGER_MS = 1800;

class BleState {
  trainerStatus = $state<DeviceStatus>('scanning');
  hrmStatus     = $state<DeviceStatus>('scanning');
  trainerName   = $state<string | null>(null);
  hrmName       = $state<string | null>(null);
  metrics       = $state<BleMetrics | null>(null);
  trainerError  = $state<string | null>(null);
  hrmError      = $state<string | null>(null);

  // Reconnect affordance state. The trainer drives a blocking modal (session page);
  // the HRM drives a non-blocking toast (layout).
  trainerReconnect = $state<ReconnectState>(null);
  hrmReconnect     = $state<ReconnectState>(null);

  // Pending timer that clears the "reconnected" success state after a short linger.
  #trainerClearTimer: ReturnType<typeof setTimeout> | null = null;
  #hrmClearTimer:     ReturnType<typeof setTimeout> | null = null;

  markDisconnected(kind: DeviceKind): void {
    if (kind === 'Trainer') {
      this.trainerStatus = 'disconnected';
      if (this.metrics) this.metrics = { ...this.metrics, power_w: null, cadence_rpm: null };
    } else {
      this.hrmStatus = 'disconnected';
      if (this.metrics) this.metrics = { ...this.metrics, hr_bpm: null };
    }
  }

  setError(kind: DeviceKind, message: string): void {
    if (kind === 'Trainer') this.trainerError = message;
    else                    this.hrmError = message;
  }

  // Single ingress for the `ble_reconnect` event. Keeps the affordance state and the
  // connection dots coherent, and auto-clears the success state after a brief linger.
  applyReconnect(kind: DeviceKind, status: ReconnectStatus, attempt?: number): void {
    this.#clearTimer(kind);
    this.#setReconnect(kind, { status, attempt: attempt ?? 0 });
    if (status === 'reconnecting') this.#setStatus(kind, 'connecting');
    else if (status === 'reconnected') {
      this.#setStatus(kind, 'connected');
      this.#setError(kind, null);
      this.#setTimer(kind, setTimeout(() => this.#setReconnect(kind, null), RECONNECTED_LINGER_MS));
    } else if (status === 'failed') this.#setStatus(kind, 'error');
  }

  // The disconnect arrives slightly before the first `reconnecting` event; show the
  // affordance immediately so there is no blank gap.
  markReconnecting(kind: DeviceKind): void {
    this.applyReconnect(kind, 'reconnecting', 0);
  }

  async retryReconnect(kind: DeviceKind): Promise<void> {
    this.applyReconnect(kind, 'reconnecting', 0);
    await invoke('retry_reconnect', { kind });
  }

  // Clear the reconnect affordance for a device (e.g. when the user stops the session
  // from the failed-reconnect modal).
  clearReconnect(kind: DeviceKind): void {
    this.#clearTimer(kind);
    this.#setReconnect(kind, null);
  }

  // --- per-device field accessors: keep the two `$state` fields out of the logic ---
  #setReconnect(kind: DeviceKind, value: ReconnectState): void {
    if (kind === 'Trainer') this.trainerReconnect = value;
    else                    this.hrmReconnect = value;
  }
  #setStatus(kind: DeviceKind, status: DeviceStatus): void {
    if (kind === 'Trainer') this.trainerStatus = status;
    else                    this.hrmStatus = status;
  }
  #setError(kind: DeviceKind, message: string | null): void {
    if (kind === 'Trainer') this.trainerError = message;
    else                    this.hrmError = message;
  }
  #setTimer(kind: DeviceKind, timer: ReturnType<typeof setTimeout> | null): void {
    if (kind === 'Trainer') this.#trainerClearTimer = timer;
    else                    this.#hrmClearTimer = timer;
  }
  #clearTimer(kind: DeviceKind): void {
    const timer = kind === 'Trainer' ? this.#trainerClearTimer : this.#hrmClearTimer;
    if (timer) clearTimeout(timer);
    this.#setTimer(kind, null);
  }
}

export const ble = new BleState();

export async function disconnectDevice(kind: DeviceKind): Promise<void> {
  // The backend actually tears the device down (aborts the notif task, drops the
  // retained id so auto-reconnect cannot resurrect it). Only reset the store once it
  // resolves; on failure (e.g. trainer blocked during an active session) keep the
  // connected state and surface the error. The backend does not emit `ble_disconnected`
  // for a manual disconnect, so we update the store here.
  try {
    await invoke(kind === 'Trainer' ? 'disconnect_trainer' : 'disconnect_hrm');
  } catch (e) {
    ble.setError(kind, toMessage(e));
    return;
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
