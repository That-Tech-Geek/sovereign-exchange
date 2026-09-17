use crate::constants::MAX_INSTRUMENTS;
use crate::instrument::InstrumentStatus;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidInstrument(u16),
    InvalidTransition {
        from: InstrumentStatus,
        to: InstrumentStatus,
    },
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
    pub fn status(&self, id: u16) -> Option<InstrumentStatus> {
        self.statuses.get(id as usize).copied()
    }
    pub fn session(&self, id: u16) -> Option<TradingSession> {
        self.sessions.get(id as usize).copied().flatten()
    }
    pub fn set_session(&mut self, id: u16, s: TradingSession) -> Result<(), LifecycleError> {
        self.sessions
            .get_mut(id as usize)
            .ok_or(LifecycleError::InvalidInstrument(id))?
            .replace(s);
        Ok(())
    }
    pub fn transition(
        &mut self,
        id: u16,
        to: InstrumentStatus,
        seq: u64,
        ts: u64,
    ) -> Result<LifecycleTransition, LifecycleError> {
        let from = self
            .status(id)
            .ok_or(LifecycleError::InvalidInstrument(id))?;
        if !valid_transition(from, to) {
            return Err(LifecycleError::InvalidTransition { from, to });
        }
        self.statuses[id as usize] = to;
        Ok(LifecycleTransition {
            instrument_id: id,
            from,
            to,
            effective_sequence: seq,
            effective_timestamp: ts,
        })
    }
    pub fn accepts_orders(&self, id: u16) -> bool {
        matches!(self.status(id), Some(InstrumentStatus::Open))
    }
    pub fn can_match(&self, id: u16) -> bool {
        self.accepts_orders(id)
    }
    pub fn is_terminal(&self, id: u16) -> bool {
        matches!(
            self.status(id),
            Some(InstrumentStatus::Closed | InstrumentStatus::Expired | InstrumentStatus::Settled)
        )
    }
}
impl Default for MarketLifecycle {
    fn default() -> Self {
        Self::new()
    }
}
fn valid_transition(from: InstrumentStatus, to: InstrumentStatus) -> bool {
    use InstrumentStatus::*;
    match (from, to) {
        (PreOpen, Open)
        | (PreOpen, Halted)
        | (PreOpen, Closed)
        | (Open, Halted)
        | (Open, Closed)
        | (Halted, Open)
        | (Halted, Closed)
        | (Closed, Expired)
        | (Closed, Settled)
        | (Expired, Settled) => true,
        (a, b) if a == b => true,
        _ => false,
    }
}
