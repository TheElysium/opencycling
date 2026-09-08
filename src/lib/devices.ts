import type { DeviceInfo } from './bindings';

type KnownDevice = { id: string; name: string } | null;

export function matchKnownDevice(
  devices: DeviceInfo[],
  known: KnownDevice,
): DeviceInfo | null {
  if (known === null || devices.length === 0) return null;
  return (
    devices.find((d) => d.id === known.id) ??
    devices.find((d) => d.name === known.name) ??
    null
  );
}

export function autoConnectCandidates(
  devices: DeviceInfo[],
  known: {
    trainer: KnownDevice;
    hrm: KnownDevice;
  },
): { trainer: DeviceInfo | null; hrm: DeviceInfo | null } {
  return {
    trainer: matchKnownDevice(
      devices.filter((d) => d.kind === 'Trainer'),
      known.trainer,
    ),
    hrm: matchKnownDevice(
      devices.filter((d) => d.kind === 'Hrm'),
      known.hrm,
    ),
  };
}
