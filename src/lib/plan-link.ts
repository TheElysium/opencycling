// Decides, on each session_metrics tick, whether to call link_plan_entry_session_cmd.
// Pure so the "exactly once per finished ride" rule is unit-tested without a Svelte store.

import type { SessionMetrics } from './bindings';

export interface LinkGate {
  /** Plan entry id waiting to be linked once the ride finishes, or null if this session is not tied to a plan entry. */
  pendingEntryId: number | null;
  /** session_id already linked for pendingEntryId in this run, to avoid calling the command twice for the same finished ride. */
  linkedSessionId: number | null;
}

// `session_metrics` fires every second while Finished, so this can't just check
// `state === 'Finished'`: it also guards an unknown session_id and a session_id already linked.
export function shouldLinkPlanEntry(
  gate: LinkGate,
  state: SessionMetrics['state'],
  sessionId: number | null
): boolean {
  if (gate.pendingEntryId == null) return false;
  if (state !== 'Finished') return false;
  if (sessionId == null) return false;
  return sessionId !== gate.linkedSessionId;
}
