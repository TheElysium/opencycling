import { describe, it, expect } from 'vitest';
import { shouldLinkPlanEntry, type LinkGate } from './plan-link';

function gate(pendingEntryId: number | null, linkedSessionId: number | null = null): LinkGate {
  return { pendingEntryId, linkedSessionId };
}

describe('shouldLinkPlanEntry', () => {
  it('never links when there is no pending plan entry', () => {
    expect(shouldLinkPlanEntry(gate(null), 'Finished', 42)).toBe(false);
    expect(shouldLinkPlanEntry(gate(null), 'Running', null)).toBe(false);
  });

  it('never links while the session is not Finished', () => {
    expect(shouldLinkPlanEntry(gate(7), 'Running', 42)).toBe(false);
    expect(shouldLinkPlanEntry(gate(7), 'WaitingForRider', 42)).toBe(false);
    expect(shouldLinkPlanEntry(gate(7), 'Paused', 42)).toBe(false);
  });

  it('never links on a Finished tick whose session_id is not known yet', () => {
    expect(shouldLinkPlanEntry(gate(7), 'Finished', null)).toBe(false);
  });

  it('links once Finished carries a fresh session_id not yet recorded', () => {
    expect(shouldLinkPlanEntry(gate(7), 'Finished', 42)).toBe(true);
  });

  it('links a new session_id even if a different one was already linked', () => {
    expect(shouldLinkPlanEntry(gate(7, 41), 'Finished', 42)).toBe(true);
  });

  it('does not link again once the same session_id is already recorded', () => {
    expect(shouldLinkPlanEntry(gate(7, 42), 'Finished', 42)).toBe(false);
  });
});
