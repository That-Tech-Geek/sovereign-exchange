//! Explicit market lifecycle state machine.
//!
//! PR10 defines lifecycle semantics without yet coupling them to the matcher
//! hot path. The lifecycle authority is intentionally separate so operational
//! controls can be journaled and tested before admission enforcement is wired
//! into the engine.

use crate::constants::MAX_INSTRUMENTS;
use crate::instrument::InstrumentStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidInstrument(u16),
    InvalidTransition { from: InstrumentStatus, to: InstrumentStatus },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradingSession {
    pub session_id: u64,
    pub opens_at: u64,
    pub closes_at: u64,
}

impl TradingSession {
    pub const fn contains(&self, timestamp: u64) -> bool {
        timestamp >= self.opens_at && timestamp < self.closes_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleTransition {
    pub instrument_id: u16,
    pub from: InstrumentStatus,
    pub to: InstrumentStatus,
    pub effective_sequence: u64,
    pub effective_timestamp: u64,
}

pub struct MarketLifecycle {
    statuses: [InstrumentStatus; MAX_INSTRUMENTS],
    sessions: [Option<TradingSession>; MAX_INSTRUMENTS],
}

impl MarketLifecycle {
    pub fn new() -> Self {
        Self {
            statuses: [InstrumentStatus::PreOpen; MAX_INSTRUMENTS],
            sessions: [None; MAX_INSTRUMENTS],
        }
    }

    #[inline(always)]
    pub fn status(&self, instrument_id: u16) -> Option<InstrumentStatus> {
        self.statuses.get(instrument_id as usize).copied()
    }

    #[inline(always)]
    pub fn session(&self, instrument_id: u16) -> Option<TradingSession> {
        self.sessions.get(instrument_id as usize).copied().flatten()
    }

    pub fn set_session(&mut self, instrument_id: u16, session: TradingSession) -> Result<(), LifecycleError> {
        let slot = self.slot_mut(instrument_id)?;
        *slot = self.statuses[instrument_id as usize];
        self.sessions[instrument_id as usize] = Some(session);
        Ok(())
    }

    pub fn transition(&mut self, instrument_id: u16, to: InstrumentStatus, effective_sequence: u64, effective_timestamp: u64) -> Result<LifecycleTransition, LifecycleError> {
        let from = self.status(instrument_id).ok_or(LifecycleError::InvalidInstrument(instrument_id))?;
        if !valid_transition(from, to) {
            return Err(LifecycleError::InvalidTransition { from, to });
        }
        self.statuses[instrument_id as usize] = to;
        Ok(LifecycleTransition { instrument_id, from, to, effective_sequence, effective_timestamp })
    }

    #[inline(always)]
    pub fn accepts_orders(&self, instrument_id: u16) -> bool {
        matches!(self.status(instrument_id), Some(InstrumentStatus::Open))
    }

    #[inline(always)]
    pub fn can_match(&self, instrument_id: u16) -> bool {
        self.accepts_orders(instrument_id)
    }

    #[inline(always)]
    pub fn is_terminal(&self, instrument_id: u16) -> bool {
        matches!(self.status(instrument_id), Some(InstrumentStatus::Closed | InstrumentStatus::Expired | InstrumentStatus::Settled))
    }

    fn slot_mut(&mut self, instrument_id: u16) -> Result<&mut InstrumentStatus, LifecycleError> {
        self.statuses.get_mut(instrument_id as usize).ok_or(LifecycleError::InvalidInstrument(instrument_id))
    }
}

impl Default for MarketLifecycle { fn default() -> Self { Self::new() } }

fn valid_transition(from: InstrumentStatus, to: InstrumentStatus) -> bool {
    use InstrumentStatus::*;
    match (from, to) {
        (PreOpen, Open) => true,
        (PreOpen, Halted) => true,
        (Open, Halted) => true,
        (Halted, Open) => true,
        (Open, Closed) => true,
        (Halted, Closed) => true,
        (Closed, Expired) => true,
        (Closed, Settled) => true,
        (Expired, Settled) => true,
        (PreOpen, Closed) => true,
        (a, b) if a == b => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_has_explicit_state_transitions() {
        let mut lifecycle = MarketLifecycle::new();
        assert_eq!(lifecycle.status(7), Some(InstrumentStatus::PreOpen));
        assert!(!lifecycle.accepts_orders(7));
        lifecycle.transition(7, InstrumentStatus::Open, 10, 100).unwrap();
        assert!(lifecycle.accepts_orders(7));
        lifecycle.transition(7, InstrumentStatus::Halted, 11, 110).unwrap();
        assert!(!lifecycle.accepts_orders(7));
        lifecycle.transition(7, InstrumentStatus::Open, 12, 120).unwrap();
        lifecycle.transition(7, InstrumentStatus::Closed, 13, 130).unwrap();
        assert!(lifecycle.is_terminal(7));
    }

    #[test]
    fn invalid_transition_fails_closed() {
        let mut lifecycle = MarketLifecycle::new();
        let err = lifecycle.transition(3, InstrumentStatus::Settled, 1, 1).unwrap_err();
        assert!(matches!(err, LifecycleError::InvalidTransition { .. }));
    }

    #[test]
    fn sessions_are_half_open() {
        let mut lifecycle = MarketLifecycle::new();
        lifecycle.set_session(2, TradingSession { session_id: 1, opens_at: 100, closes_at: 200 }).unwrap();
        assert!(!lifecycle.session(2).unwrap().contains(99));
        assert!(lifecycle.session(2).unwrap().contains(100));
        assert!(!lifecycle.session(2).unwrap().contains(200));
    }
}
