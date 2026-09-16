import { goto } from '$app/navigation';
import { commands } from './bindings';
import type { FlatBlock, ParsedWorkout, PlanEntryView } from './bindings';
import { getSettings } from './settings';
import { indexByFileName } from './plan-load';
import { resolvePlanStart } from './plan-start';
import { session } from './session.svelte';

type Snapshot = { ftp: number; aero: boolean; workouts: ParsedWorkout[]; flats: FlatBlock[][] };

// Best-effort: a rider without a configured/scannable library still gets a usable page.
async function fetchLibrary(prev: Snapshot): Promise<Snapshot> {
  try {
    const s = await getSettings();
    const lib = s.workout_path
      ? await commands.listWorkoutsCmd(s.workout_path, s.ftp_w)
      : { workouts: [], flats: [] };
    return { ftp: s.ftp_w, aero: s.aero_enabled, workouts: lib.workouts, flats: lib.flats };
  } catch {
    return { ftp: prev.ftp, aero: prev.aero, workouts: [], flats: [] };
  }
}

// The workout library (settings FTP/aero + scanned .zwo files), shared by every
// page that lists or starts a plan entry (Home, /plans, /plans/[id]).
class LibraryStore {
  ftp      = $state(0);
  aero     = $state(false);
  workouts = $state<ParsedWorkout[]>([]);
  flats    = $state<FlatBlock[][]>([]);
  loading  = $state(true);
  #latest = 0;

  get index(): Map<string, ParsedWorkout> {
    return indexByFileName(this.workouts);
  }

  // Pages mounting back to back fire overlapping loads; only the latest one may write.
  async load(): Promise<void> {
    const id = ++this.#latest;
    this.loading = true;
    const next = await fetchLibrary(this);
    if (id !== this.#latest) return;
    this.ftp = next.ftp;
    this.aero = next.aero;
    this.workouts = next.workouts;
    this.flats = next.flats;
    this.loading = false;
  }

  // Shared start flow: resolve the entry against the loaded library, arm the
  // session, and navigate. Returns the reason it could not start, or null on success.
  async start(entry: PlanEntryView): Promise<string | null> {
    const result = resolvePlanStart(entry, this.index, this.ftp);
    if (!result.ok) return result.error;
    session.prepare(result.workout, result.ftpW, this.aero, entry.entry_id);
    await goto('/session');
    return null;
  }
}

export const library = new LibraryStore();
