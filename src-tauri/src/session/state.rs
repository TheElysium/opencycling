use crate::session::types::{
    FinishedState, PausedState, RunningState, Session, State, StateKind, WaitingForRiderState,
};

pub const TICK_S: u32 = 1;
const CADENCE_START: u16 = 30;
const POWER_START: i16 = 30;

impl State for WaitingForRiderState {
    fn kind(&self) -> StateKind {
        StateKind::WaitingForRider
    }
    fn tick(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
        let pedaling = session.last_cadence_rpm.is_some_and(|c| c >= CADENCE_START)
            || session.last_power_w.is_some_and(|p| p >= POWER_START);
        if pedaling {
            Box::new(RunningState)
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
}

impl State for RunningState {
    fn kind(&self) -> StateKind {
        StateKind::Running
    }
    fn tick(self: Box<Self>, session: &mut Session) -> Box<dyn State> {
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
        Box::new(PausedState)
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
}

impl State for PausedState {
    fn kind(&self) -> StateKind {
        StateKind::Paused
    }
    fn tick(self: Box<Self>, _session: &mut Session) -> Box<dyn State> {
        self
    }
    fn pause(self: Box<Self>) -> Box<dyn State> {
        self
    }
    fn resume(self: Box<Self>) -> Box<dyn State> {
        Box::new(RunningState)
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
        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(PausedState);
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

        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(RunningState);
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

        let st: Box<dyn State> = Box::new(PausedState);
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
        let st: Box<dyn State> = Box::new(RunningState);
        assert_eq!(st.stop().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(PausedState);
        assert_eq!(st.stop().kind(), StateKind::Finished);
        let st: Box<dyn State> = Box::new(WaitingForRiderState);
        assert_eq!(st.stop().kind(), StateKind::Finished);
    }
}
