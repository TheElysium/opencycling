use crate::session::types::{
    FinishedState, PausedState, RampingState, RunningState, Session, State, StateKind,
    WaitingForRiderState,
};

pub const TICK_S: u32 = 1;
const CADENCE_START: u16 = 30;
const POWER_START: i16 = 30;
/// Sustained pedaling (in ticks) required to auto-resume a stall pause.
const RESUME_PEDAL_S: u32 = 3;
/// Duration of the post-resume ERG ramp back to the block target.
pub const RAMP_S: u32 = 15;
/// Never send an ERG target below this during a ramp: keeps the trainer neutral
/// enough to spin freely without going fully slack.
const RAMP_FLOOR_MIN_W: u16 = 30;

/// ERG floor used while paused and as the ramp starting point: half the block
/// target, clamped so it stays spinable and never exceeds the target itself.
pub fn ramp_floor_w(target_w: u16) -> u16 {
    (target_w / 2).max(RAMP_FLOOR_MIN_W).min(target_w)
}

fn pedaling(session: &Session) -> bool {
    session.last_cadence_rpm.is_some_and(|c| c >= CADENCE_START)
        || session.last_power_w.is_some_and(|p| p >= POWER_START)
}

impl State for WaitingForRiderState {
    fn kind(&self) -> StateKind {
        StateKind::WaitingForRider
    }
    fn tick(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        if pedaling(session) {
            Box::new(RunningState { no_pedal_s: 0 })
        } else {
            self
        }
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn stop(self: Box<Self>) -> Box<dyn State> {
        Box::new(FinishedState)
    }
    fn skip(self: Box<Self>, _session: &mut Session) -> Box<dyn State> {
        self
    }
    fn device_lost(self: Box<Self>) -> Box<dyn State> {
        // No workout timer is running yet, so there is nothing to pause. The rider
        // simply has not started; the blocking modal (frontend) still shows while the
        // trainer reconnects. Stay put.
        self
    }
    fn device_reconnected(self: Box<Self>) -> Box<dyn State> {
        self
    }
}

impl State for RunningState {
    fn kind(&self) -> StateKind {
        StateKind::Running
    }
    fn tick(mut self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        if pedaling(session) {
            self.no_pedal_s = 0;
        } else {
            self.no_pedal_s += TICK_S;
        }
        // Stall: the rider stopped pedaling. Freeze the clock on the triggering
        // tick and pause; the actor drops the ERG target to the ramp floor.
        if self.no_pedal_s >= session.stall_timeout_s as u32 {
            return Box::new(PausedState {
                by_dropout: false,
                by_stall: true,
                resume_pedal_s: 0,
            });
        }

        session.total_elapsed_s += TICK_S;
        session.total_active_s += TICK_S;
        session.current_block_elapsed_s += TICK_S;

        let Some(block_duration_s) = session.current_block().map(|b| b.duration_s) else {
            return Box::new(FinishedState);
        };
        if session.current_block_elapsed_s >= block_duration_s {
            session.advance_block()
        }
        if session.is_finished() {
            return Box::new(FinishedState);
        }
        session.last_target_w = session.compute_target_w();
        self
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        })
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn stop(self: Box<Self>) -> Box<dyn State> {
        Box::new(FinishedState)
    }
    fn skip(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        session.skip_block();
        if session.is_finished() {
            return Box::new(FinishedState);
        }
        self
    }
    fn device_lost(self: Box<Self>) -> Box<dyn State> {
        Box::new(PausedState {
            by_dropout: true,
            by_stall: false,
            resume_pedal_s: 0,
        })
    }
    fn device_reconnected(self: Box<Self>) -> Box<dyn State> {
        self
    }
}

impl State for PausedState {
    fn kind(&self) -> StateKind {
        StateKind::Paused
    }
    fn paused_by_stall(&self) -> bool {
        self.by_stall
    }
    fn tick(mut self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        // Only a stall pause auto-resumes on pedaling; a manual or dropout pause
        // needs an explicit resume (or a trainer reconnect for dropout).
        if !self.by_stall {
            return self;
        }
        if pedaling(session) {
            self.resume_pedal_s += TICK_S;
            if self.resume_pedal_s >= RESUME_PEDAL_S {
                return Box::new(RampingState {
                    ramp_elapsed_s: 0,
                    no_pedal_s: 0,
                });
            }
        } else {
            self.resume_pedal_s = 0;
        }
        self
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        // Every resume goes through the ramp so the ERG target comes back up
        // progressively instead of jumping straight to the block target.
        Box::new(RampingState {
            ramp_elapsed_s: 0,
            no_pedal_s: 0,
        })
    }
    fn stop(self: Box<Self>) -> Box<dyn State> {
        Box::new(FinishedState)
    }
    fn skip(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        session.skip_block();
        if session.is_finished() {
            return Box::new(FinishedState);
        }
        self
    }
    fn device_lost(self: Box<Self>) -> Box<dyn State> {
        // Already paused. Do NOT promote a manual pause (by_dropout: false) into a
        // dropout pause, otherwise a later reconnect would resume a session the rider
        // deliberately paused. Keep the existing flag untouched.
        self
    }
    fn device_reconnected(self: Box<Self>) -> Box<dyn State> {
        if self.by_dropout {
            Box::new(RampingState {
                ramp_elapsed_s: 0,
                no_pedal_s: 0,
            })
        } else {
            self
        }
    }
}

impl State for RampingState {
    fn kind(&self) -> StateKind {
        StateKind::Ramping
    }
    fn tick(mut self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        // Same stall rule as Running: without pedaling the ramp is frozen (clock and
        // target hold) instead of pushing resistance up on empty legs, and after the
        // timeout the session pauses so the usual auto-resume/ramp cycle applies.
        if pedaling(session) {
            self.no_pedal_s = 0;
        } else {
            self.no_pedal_s += TICK_S;
            if self.no_pedal_s >= session.stall_timeout_s as u32 {
                return Box::new(PausedState {
                    by_dropout: false,
                    by_stall: true,
                    resume_pedal_s: 0,
                });
            }
            return self;
        }

        self.ramp_elapsed_s += TICK_S;
        if self.ramp_elapsed_s >= RAMP_S {
            return Box::new(RunningState { no_pedal_s: 0 });
        }
        // The block clock is frozen during the ramp, so compute_target_w is stable
        // and the ramp is a plain floor -> target interpolation.
        if let Some(target) = session.compute_target_w() {
            let floor = ramp_floor_w(target);
            let ramped = floor as i32
                + (target as i32 - floor as i32) * self.ramp_elapsed_s as i32 / RAMP_S as i32;
            session.last_target_w = Some(ramped.max(0) as u16);
        }
        self
    }
    fn ramp_remaining_s(&self) -> Option<u32> {
        Some(RAMP_S.saturating_sub(self.ramp_elapsed_s))
    }
    fn paused_by_stall(&self) -> bool {
        false
    }
    fn ramp_stalled(&self) -> bool {
        self.no_pedal_s > 0
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        })
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn stop(self: Box<Self>) -> Box<dyn State> {
        Box::new(FinishedState)
    }
    fn skip(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        // The rider is on the bike and actively skipping; no ramp needed for the
        // next block.
        session.skip_block();
        if session.is_finished() {
            return Box::new(FinishedState);
        }
        Box::new(RunningState { no_pedal_s: 0 })
    }
    fn device_lost(self: Box<Self>) -> Box<dyn State> {
        Box::new(PausedState {
            by_dropout: true,
            by_stall: false,
            resume_pedal_s: 0,
        })
    }
    fn device_reconnected(self: Box<Self>) -> Box<dyn State> {
        self
    }
}

impl State for FinishedState {
    fn kind(&self) -> StateKind {
        StateKind::Finished
    }
    fn tick(self: Box<Self>, _session: &mut Session) -> Box<dyn State> {
        self
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn stop(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn skip(self: Box<Self>, _session: &mut Session) -> Box<dyn State> {
        self
    }
    fn device_lost(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn device_reconnected(self: Box<Self>) -> Box<dyn State> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::types::FlatBlock;

    fn steady(duration_s: u32, watts: u16) -> FlatBlock {
        FlatBlock {
            duration_s,
            power_start_w: watts,
            power_end_w: watts,
            cadence_rpm: None,
            label: String::new(),
        }
    }

    fn ramp(duration_s: u32, start_w: u16, end_w: u16) -> FlatBlock {
        FlatBlock {
            duration_s,
            power_start_w: start_w,
            power_end_w: end_w,
            cadence_rpm: None,
            label: String::new(),
        }
    }

    fn session_with(blocks: Vec<FlatBlock>) -> Session {
        Session {
            blocks,
            ftp_w: 200,
            stall_timeout_s: 5,
            total_elapsed_s: 0,
            total_active_s: 0,
            current_block_idx: 0,
            current_block_elapsed_s: 0,
            last_target_w: None,
            last_cadence_rpm: None,
            last_power_w: None,
            workout_name: None,
            workout_author: None,
            workout_description: None,
            is_ftp_test: false,
        }
    }

    // --- Running.tick: counters + target ---

    #[test]
    fn running_tick_increments_counters() {
        let mut s = session_with(vec![steady(60, 150)]);
        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.tick(&mut s);

        assert_eq!(s.total_elapsed_s, 1);
        assert_eq!(s.total_active_s, 1);
        assert_eq!(s.current_block_elapsed_s, 1);
        assert_eq!(s.current_block_idx, 0);
        assert_eq!(s.last_target_w, Some(150));
        assert_eq!(next.kind(), StateKind::Running);
    }

    #[test]
    fn running_tick_advances_to_next_block_when_duration_reached() {
        // Eager transition: at elapsed=59, tick brings it to 60 -> idx advances.
        let mut s = session_with(vec![steady(60, 150), steady(60, 200)]);
        s.current_block_elapsed_s = 59;
        s.total_elapsed_s = 59;
        s.total_active_s = 59;

        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.tick(&mut s);

        assert_eq!(s.current_block_idx, 1);
        assert_eq!(s.current_block_elapsed_s, 0);
        assert_eq!(s.total_elapsed_s, 60);
        assert_eq!(s.total_active_s, 60);
        assert_eq!(s.last_target_w, Some(200));
        assert_eq!(next.kind(), StateKind::Running);
    }

    #[test]
    fn running_tick_finishes_on_last_block() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.current_block_elapsed_s = 59;
        s.total_elapsed_s = 59;
        s.total_active_s = 59;

        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.tick(&mut s);

        assert_eq!(s.total_elapsed_s, 60);
        assert_eq!(next.kind(), StateKind::Finished);
    }

    #[test]
    fn running_tick_ramp_interpolates_target() {
        // Ramp 100->200W over 60s. At t=30 (after tick from 29), expected target = 150.
        let mut s = session_with(vec![ramp(60, 100, 200)]);
        s.current_block_elapsed_s = 29;
        s.total_elapsed_s = 29;
        s.total_active_s = 29;

        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let _ = st.tick(&mut s);

        assert_eq!(s.last_target_w, Some(150));
    }

    // --- Paused.tick: no mutation ---

    #[test]
    fn paused_tick_does_not_mutate_session() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.current_block_elapsed_s = 10;
        s.total_elapsed_s = 10;
        s.total_active_s = 10;
        s.last_target_w = Some(150);

        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        let next = st.tick(&mut s);

        assert_eq!(s.current_block_idx, 0);
        assert_eq!(s.current_block_elapsed_s, 10);
        assert_eq!(s.total_elapsed_s, 10);
        assert_eq!(s.total_active_s, 10);
        assert_eq!(s.last_target_w, Some(150));
        assert_eq!(next.kind(), StateKind::Paused);
    }

    // --- skip: bump remaining time, idx++ ---

    #[test]
    fn running_skip_jumps_to_next_block_and_bumps_elapsed() {
        let mut s = session_with(vec![steady(60, 150), steady(60, 200)]);
        s.current_block_elapsed_s = 10;
        s.total_elapsed_s = 10;
        s.total_active_s = 10;

        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.skip(&mut s);

        assert_eq!(s.current_block_idx, 1);
        assert_eq!(s.current_block_elapsed_s, 0);
        assert_eq!(s.total_elapsed_s, 60); // bumped by 50 (remainder of block 1)
        assert_eq!(s.total_active_s, 10); // active never moves on skip
        assert_eq!(next.kind(), StateKind::Running);
    }

    #[test]
    fn running_skip_on_last_block_finishes() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.current_block_elapsed_s = 10;
        s.total_elapsed_s = 10;
        s.total_active_s = 10;

        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.skip(&mut s);

        assert_eq!(s.total_elapsed_s, 60);
        assert_eq!(s.total_active_s, 10);
        assert_eq!(next.kind(), StateKind::Finished);
    }

    #[test]
    fn paused_skip_jumps_but_stays_paused() {
        let mut s = session_with(vec![steady(60, 150), steady(60, 200)]);
        s.current_block_elapsed_s = 10;
        s.total_elapsed_s = 10;
        s.total_active_s = 10;

        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        let next = st.skip(&mut s);

        assert_eq!(s.current_block_idx, 1);
        assert_eq!(s.current_block_elapsed_s, 0);
        assert_eq!(s.total_elapsed_s, 60);
        assert_eq!(s.total_active_s, 10);
        assert_eq!(next.kind(), StateKind::Paused);
    }

    // --- Minimal anchoring of the transition table ---

    #[test]
    fn waiting_tick_stays_waiting_when_not_pedaling() {
        let mut s = session_with(vec![steady(60, 150)]);
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.tick(&mut s).kind(), StateKind::WaitingForRider);
    }

    #[test]
    fn waiting_tick_becomes_running_when_pedaling() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.last_cadence_rpm = Some(50);
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.tick(&mut s).kind(), StateKind::Running);
    }

    #[test]
    fn stop_from_anywhere_becomes_finished() {
        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        assert_eq!(st.stop().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        assert_eq!(st.stop().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 0,
            no_pedal_s: 0,
        });
        assert_eq!(st.stop().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.stop().kind(), StateKind::Finished);
    }

    // --- device_lost / device_reconnected: trainer dropout transitions ---

    #[test]
    fn running_device_lost_pauses_by_dropout() {
        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let next = st.device_lost();
        assert_eq!(next.kind(), StateKind::Paused);
        // A dropout pause must auto-resume (into a ramp) on reconnect.
        assert_eq!(next.device_reconnected().kind(), StateKind::Ramping);
    }

    #[test]
    fn dropout_paused_resumes_on_reconnect() {
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: true,
            by_stall: false,
            resume_pedal_s: 0,
        });
        assert_eq!(st.device_reconnected().kind(), StateKind::Ramping);
    }

    #[test]
    fn manual_pause_device_lost_keeps_by_dropout_false() {
        // Rider paused manually, then the trainer drops: the pause must NOT become a
        // dropout pause, so a later reconnect does not resume the session.
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        let after_lost = st.device_lost();
        assert_eq!(after_lost.kind(), StateKind::Paused);
        // Reconnect must leave a manually-paused session paused.
        assert_eq!(after_lost.device_reconnected().kind(), StateKind::Paused);
    }

    #[test]
    fn manual_resume_clears_dropout_flag() {
        // A manual resume of a dropout pause yields Ramping; a subsequent reconnect
        // event is a harmless no-op (already past Paused).
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: true,
            by_stall: false,
            resume_pedal_s: 0,
        });
        assert_eq!(st.resume().kind(), StateKind::Ramping);
    }

    #[test]
    fn waiting_device_lost_stays_waiting() {
        // Trainer drops before the rider started pedaling: no timer to pause.
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.device_lost().kind(), StateKind::WaitingForRider);
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.device_reconnected().kind(), StateKind::WaitingForRider);
    }

    #[test]
    fn finished_device_lost_and_reconnected_are_noops() {
        let st: Box<dyn State> = Box::new(FinishedState);
        assert_eq!(st.device_lost().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(FinishedState);
        assert_eq!(st.device_reconnected().kind(), StateKind::Finished);
    }

    // --- Stall auto-pause: rider stops pedaling mid-run ---

    #[test]
    fn running_pauses_by_stall_after_timeout_without_pedaling() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.stall_timeout_s = 2;
        let mut st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Running);
        assert_eq!(s.total_elapsed_s, 1);
        // The triggering tick freezes the clock instead of advancing it.
        st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Paused);
        assert_eq!(s.total_elapsed_s, 1);
        assert_eq!(s.current_block_elapsed_s, 1);
    }

    #[test]
    fn running_pedaling_resets_stall_counter() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.stall_timeout_s = 2;
        let mut st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        st = st.tick(&mut s); // no pedal: 1
        s.last_cadence_rpm = Some(80);
        st = st.tick(&mut s); // pedaling: counter reset, clock advances
        s.last_cadence_rpm = None;
        st = st.tick(&mut s); // no pedal: 1 again
        assert_eq!(st.kind(), StateKind::Running);
        assert_eq!(s.total_elapsed_s, 3);
    }

    #[test]
    fn stall_pause_auto_resumes_after_sustained_pedaling() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.stall_timeout_s = 1;
        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 0 });
        let mut st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Paused);
        assert!(st.paused_by_stall());
        // Not pedaling: stays paused.
        st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Paused);
        // Sustained pedaling: 3 ticks to auto-resume into a ramp.
        s.last_cadence_rpm = Some(80);
        for _ in 0..2 {
            st = st.tick(&mut s);
            assert_eq!(st.kind(), StateKind::Paused);
        }
        st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Ramping);
    }

    #[test]
    fn manual_pause_tick_never_auto_resumes_even_while_pedaling() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.last_cadence_rpm = Some(80);
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        assert!(!st.paused_by_stall());
        assert_eq!(st.tick(&mut s).kind(), StateKind::Paused);
    }

    // --- Ramp: stalled feedback while frozen ---

    #[test]
    fn ramp_is_not_stalled_while_pedaling() {
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 3,
            no_pedal_s: 0,
        });
        assert!(!st.ramp_stalled());
    }

    #[test]
    fn ramp_reports_stalled_once_pedaling_stops() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.stall_timeout_s = 5;
        s.last_cadence_rpm = None; // not pedaling
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 3,
            no_pedal_s: 0,
        });
        let st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Ramping);
        assert!(st.ramp_stalled());
    }

    #[test]
    fn non_ramping_states_are_never_stalled() {
        let st: Box<dyn State> = Box::new(RunningState { no_pedal_s: 2 });
        assert!(!st.ramp_stalled());
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: true,
            resume_pedal_s: 0,
        });
        assert!(!st.ramp_stalled());
    }

    // --- Ramp: progressive ERG target with frozen clock ---

    #[test]
    fn resume_from_paused_enters_ramp_with_frozen_clock() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.current_block_elapsed_s = 20;
        s.total_elapsed_s = 20;
        s.total_active_s = 20;
        s.last_target_w = Some(150);
        s.last_cadence_rpm = Some(80);
        let st: Box<dyn State> = Box::new(PausedState {
            by_dropout: false,
            by_stall: false,
            resume_pedal_s: 0,
        });
        let st = st.resume();
        assert_eq!(st.kind(), StateKind::Ramping);
        assert_eq!(st.ramp_remaining_s(), Some(RAMP_S));
        // Floor of 150 is 75; one tick in: 75 + 75/15 = 80.
        let st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Ramping);
        assert_eq!(st.ramp_remaining_s(), Some(RAMP_S - 1));
        assert_eq!(s.last_target_w, Some(80));
        assert_eq!(s.total_elapsed_s, 20);
        assert_eq!(s.total_active_s, 20);
        assert_eq!(s.current_block_elapsed_s, 20);
    }

    #[test]
    fn ramp_tick_progresses_linearly_toward_block_target() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.last_target_w = Some(150);
        s.last_cadence_rpm = Some(80);
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 0,
            no_pedal_s: 0,
        });
        // floor(150) = 75, +5 W per tick over the 15-tick ramp.
        st.tick(&mut s);
        assert_eq!(s.last_target_w, Some(80));
        // 14th tick: 75 + 75*14/15 = 145, one tick left.
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 13,
            no_pedal_s: 0,
        });
        assert_eq!(st.ramp_remaining_s(), Some(2));
        st.tick(&mut s);
        assert_eq!(s.last_target_w, Some(145));
    }

    #[test]
    fn ramp_completes_into_running_which_writes_block_target() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.last_target_w = Some(150);
        s.last_cadence_rpm = Some(80);
        let mut st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 0,
            no_pedal_s: 0,
        });
        for _ in 0..RAMP_S {
            st = st.tick(&mut s);
        }
        assert_eq!(st.kind(), StateKind::Running);
        // The next running tick recomputes and stores the full block target.
        st.tick(&mut s);
        assert_eq!(s.last_target_w, Some(150));
    }

    #[test]
    fn ramp_skip_goes_straight_to_running() {
        let mut s = session_with(vec![steady(60, 150), steady(60, 200)]);
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 5,
            no_pedal_s: 0,
        });
        let next = st.skip(&mut s);
        assert_eq!(s.current_block_idx, 1);
        assert_eq!(next.kind(), StateKind::Running);
    }

    #[test]
    fn ramp_freezes_target_while_not_pedaling_then_pauses_by_stall() {
        let mut s = session_with(vec![steady(60, 150)]);
        s.stall_timeout_s = 2;
        s.last_cadence_rpm = Some(80);
        let mut st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 0,
            no_pedal_s: 0,
        });
        st = st.tick(&mut s);
        assert_eq!(s.last_target_w, Some(80));
        s.last_cadence_rpm = None;
        // Not pedaling: ramp progress and target are held.
        st = st.tick(&mut s);
        assert_eq!(st.ramp_remaining_s(), Some(RAMP_S - 1));
        assert_eq!(s.last_target_w, Some(80));
        // After the stall timeout the session pauses; the clock stayed frozen.
        st = st.tick(&mut s);
        assert_eq!(st.kind(), StateKind::Paused);
        assert!(st.paused_by_stall());
        assert_eq!(s.total_elapsed_s, 0);
    }

    #[test]
    fn ramp_device_lost_pauses_then_reconnect_restarts_ramp() {
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 7,
            no_pedal_s: 0,
        });
        let next = st.device_lost();
        assert_eq!(next.kind(), StateKind::Paused);
        assert_eq!(next.device_reconnected().kind(), StateKind::Ramping);
    }

    #[test]
    fn pause_during_ramp_then_resume_restarts_ramp() {
        let st: Box<dyn State> = Box::new(RampingState {
            ramp_elapsed_s: 7,
            no_pedal_s: 0,
        });
        let next = st.pause();
        assert_eq!(next.kind(), StateKind::Paused);
        let next = next.resume();
        assert_eq!(next.kind(), StateKind::Ramping);
        assert_eq!(next.ramp_remaining_s(), Some(RAMP_S));
    }

    #[test]
    fn ramp_floor_is_clamped() {
        assert_eq!(ramp_floor_w(200), 100);
        assert_eq!(ramp_floor_w(40), 30); // minimum spinable floor
        assert_eq!(ramp_floor_w(20), 20); // never above the block target
    }
}
