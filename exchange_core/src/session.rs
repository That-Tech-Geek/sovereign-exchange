#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessageSeq(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryRequest {
    pub begin: MessageSeq,
    pub end: MessageSeq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    NotEstablished,
    DuplicateOrOld {
        expected: MessageSeq,
        received: MessageSeq,
    },
    Gap {
        expected: MessageSeq,
        received: MessageSeq,
    },
    InvalidRecoveryRange,
}

#[derive(Debug)]
pub struct SessionState {
    next_in: MessageSeq,
    next_out: MessageSeq,
    established: bool,
    heartbeat_interval_ticks: u64,
    last_activity_tick: u64,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            next_in: MessageSeq(1),
            next_out: MessageSeq(1),
            established: false,
            heartbeat_interval_ticks: 1000,
            last_activity_tick: 0,
        }
    }

    pub fn establish(
        &mut self,
        next_in: MessageSeq,
        next_out: MessageSeq,
        heartbeat_interval_ticks: u64,
    ) {
        self.next_in = next_in;
        self.next_out = next_out;
        self.heartbeat_interval_ticks = heartbeat_interval_ticks;
        self.last_activity_tick = 0;
        self.established = true;
    }

    pub fn next_in(&self) -> MessageSeq {
        self.next_in
    }
    pub fn next_out(&self) -> MessageSeq {
        self.next_out
    }

    pub fn allocate_outbound(&mut self) -> Result<MessageSeq, SessionError> {
        if !self.established {
            return Err(SessionError::NotEstablished);
        }
        let seq = self.next_out;
        self.next_out = MessageSeq(
            self.next_out
                .0
                .checked_add(1)
                .ok_or(SessionError::InvalidRecoveryRange)?,
        );
        Ok(seq)
    }

    pub fn receive(&mut self, sequence: MessageSeq) -> Result<(), SessionError> {
        if !self.established {
            return Err(SessionError::NotEstablished);
        }
        if sequence < self.next_in {
            return Err(SessionError::DuplicateOrOld {
                expected: self.next_in,
                received: sequence,
            });
        }
        if sequence > self.next_in {
            return Err(SessionError::Gap {
                expected: self.next_in,
                received: sequence,
            });
        }
        self.next_in = MessageSeq(
            self.next_in
                .0
                .checked_add(1)
                .ok_or(SessionError::InvalidRecoveryRange)?,
        );
        Ok(())
    }

    pub fn recovery_request(&self, received: MessageSeq) -> Result<RecoveryRequest, SessionError> {
        if !self.established {
            return Err(SessionError::NotEstablished);
        }
        if received <= self.next_in {
            return Err(SessionError::InvalidRecoveryRange);
        }
        Ok(RecoveryRequest {
            begin: self.next_in,
            end: MessageSeq(received.0 - 1),
        })
    }

    pub fn heartbeat_due(&self, now_tick: u64) -> bool {
        now_tick.saturating_sub(self.last_activity_tick) >= self.heartbeat_interval_ticks
    }

    pub fn note_activity(&mut self, now_tick: u64) {
        self.last_activity_tick = now_tick;
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_session_detects_and_recovers_gaps() {
        let mut session = SessionState::new();
        session.establish(MessageSeq(1), MessageSeq(1), 10);
        assert_eq!(
            session.receive(MessageSeq(2)),
            Err(SessionError::Gap {
                expected: MessageSeq(1),
                received: MessageSeq(2)
            })
        );
        assert_eq!(
            session.recovery_request(MessageSeq(4)).unwrap(),
            RecoveryRequest {
                begin: MessageSeq(1),
                end: MessageSeq(3)
            }
        );
        session.receive(MessageSeq(1)).unwrap();
        session.receive(MessageSeq(2)).unwrap();
        session.receive(MessageSeq(3)).unwrap();
        assert_eq!(session.next_in(), MessageSeq(4));
    }

    #[test]
    fn outbound_sequence_is_monotonic() {
        let mut session = SessionState::new();
        session.establish(MessageSeq(1), MessageSeq(1), 10);
        assert_eq!(session.allocate_outbound().unwrap(), MessageSeq(1));
        assert_eq!(session.allocate_outbound().unwrap(), MessageSeq(2));
    }

    #[test]
    fn heartbeat_uses_monotonic_transport_time() {
        let mut session = SessionState::new();
        session.establish(MessageSeq(1), MessageSeq(1), 10);
        assert!(!session.heartbeat_due(9));
        assert!(session.heartbeat_due(10));
        session.note_activity(10);
        assert!(!session.heartbeat_due(19));
    }
}
