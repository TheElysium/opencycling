import { describe, it, expect } from 'vitest';
import { matchKnownDevice, autoConnectCandidates } from './devices';
import type { DeviceInfo } from './bindings';

function dev(id: string, name: string, kind: DeviceInfo['kind'] = null): DeviceInfo {
  return { id, name, kind };
}

describe('matchKnownDevice', () => {
  it('returns null when known is null', () => {
    const devices = [dev('a', 'TrainerX', 'Trainer')];
    expect(matchKnownDevice(devices, null)).toBeNull();
  });

  it('returns null when devices list is empty', () => {
    expect(matchKnownDevice([], { id: 'a', name: 'TrainerX' })).toBeNull();
  });

  it('matches by id first', () => {
    const d1 = dev('id-1', 'Alpha', 'Trainer');
    const d2 = dev('id-2', 'Beta', 'Trainer');
    const result = matchKnownDevice([d1, d2], { id: 'id-2', name: 'Alpha' });
    expect(result).toBe(d2);
  });

  it('falls back to exact name match when id differs', () => {
    const d1 = dev('id-1', 'Alpha', 'Trainer');
    const d2 = dev('id-2', 'Beta', 'Trainer');
    const result = matchKnownDevice([d1, d2], { id: 'unknown', name: 'Alpha' });
    expect(result).toBe(d1);
  });

  it('returns null when neither id nor name matches', () => {
    const devices = [dev('id-1', 'Alpha', 'Trainer')];
    const result = matchKnownDevice(devices, { id: 'nope', name: 'Gamma' });
    expect(result).toBeNull();
  });
});

describe('autoConnectCandidates', () => {
  it('maps both kinds independently', () => {
    const devices = [
      dev('t-id', 'D500', 'Trainer'),
      dev('h-id', 'Polar H10', 'Hrm'),
    ];
    const result = autoConnectCandidates(devices, {
      trainer: { id: 't-id', name: 'Other' },
      hrm: { id: 'h-id', name: 'Other' },
    });
    expect(result.trainer).toBe(devices[0]);
    expect(result.hrm).toBe(devices[1]);
  });

  it('returns null for unmatched kinds', () => {
    const devices = [dev('t-id', 'D500', 'Trainer')];
    const result = autoConnectCandidates(devices, {
      trainer: { id: 'nope', name: 'nope' },
      hrm: { id: 'h-id', name: 'Polar H10' },
    });
    expect(result.trainer).toBeNull();
    expect(result.hrm).toBeNull();
  });

  it('trainer known does not match an hrm device', () => {
    const devices = [dev('id-1', 'Polar H10', 'Hrm')];
    const result = autoConnectCandidates(devices, {
      trainer: { id: 'id-1', name: 'Polar H10' },
      hrm: null,
    });
    expect(result.trainer).toBeNull();
    expect(result.hrm).toBeNull();
  });
});
