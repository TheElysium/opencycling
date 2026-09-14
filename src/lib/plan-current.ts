import type { TrainingPlan } from '$lib/bindings';
import { planEndDate } from './plan-date';

/** The single active plan covering `today` (ISO date), if any; the backend forbids
 *  overlapping active plans (src-tauri/src/plan/schedule.rs, check_no_overlap). */
export function currentPlan(plans: TrainingPlan[], today: string): TrainingPlan | null {
  return (
    plans.find(
      (p) => p.archived_at === null && p.start_date <= today && today <= planEndDate(p.start_date, p.weeks),
    ) ?? null
  );
}
