use crate::command::OrderCommand;
use crate::engine::{MatchingEngine, OrderAcceptError};
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationError {
    Fenced,
    QueueFull,
    Disconnected,
    Reject(OrderAcceptError),
}

#[derive(Debug)]
pub struct Sequencer {
    tx: Sender<OrderCommand>,
    rx: Receiver<OrderCommand>,
    generation: Generation,
}

impl Sequencer {
    pub fn new(capacity: usize, generation: Generation) -> Self {
        let (tx, rx) = bounded(capacity);
        Self { tx, rx, generation }
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn submit(
        &self,
        generation: Generation,
        command: OrderCommand,
    ) -> Result<(), ReplicationError> {
        if generation != self.generation {
            return Err(ReplicationError::Fenced);
        }
        self.tx.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => ReplicationError::QueueFull,
            TrySendError::Disconnected(_) => ReplicationError::Disconnected,
        })
    }

    pub fn drain_into(
        &self,
        engine: &mut MatchingEngine,
        standby: Option<&mut HotStandby>,
    ) -> Result<usize, ReplicationError> {
        let mut processed = 0;
        let mut standby = standby;
        while let Ok(command) = self.rx.try_recv() {
            let accepted = engine
                .accept_command(&command)
                .map_err(ReplicationError::Reject)?;
            engine.process_order(accepted.pool_index);
            if let Some(node) = standby.as_deref_mut() {
                node.apply(self.generation, command)?;
            }
            processed += 1;
        }
        Ok(processed)
    }
}

pub struct HotStandby {
    engine: MatchingEngine,
    generation: Generation,
    applied: u64,
}

impl HotStandby {
    pub fn new(generation: Generation) -> Self {
        Self {
            engine: MatchingEngine::new(),
            generation,
            applied: 0,
        }
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn applied(&self) -> u64 {
        self.applied
    }

    pub fn promote(&mut self, new_generation: Generation) {
        self.generation = new_generation;
    }

    pub fn apply(
        &mut self,
        generation: Generation,
        command: OrderCommand,
    ) -> Result<(), ReplicationError> {
        if generation != self.generation {
            return Err(ReplicationError::Fenced);
        }
        let accepted = self
            .engine
            .accept_command(&command)
            .map_err(ReplicationError::Reject)?;
        self.engine.process_order(accepted.pool_index);
        self.applied += 1;
        Ok(())
    }

    pub fn engine(&self) -> &MatchingEngine {
        &self.engine
    }

    pub fn into_engine(self) -> MatchingEngine {
        self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{NewOrder, OrderSide};
    use crate::order::ClientOrderId;

    fn order(id: u64) -> OrderCommand {
        OrderCommand::New(NewOrder {
            client_order_id: ClientOrderId(id),
            account_id: 7,
            instrument_id: 0,
            side: OrderSide::Buy,
            price: 100,
            quantity: 1,
            client_timestamp: id,
        })
    }

    #[test]
    fn bounded_ingress_backpressures() {
        let sequencer = Sequencer::new(1, Generation(1));
        assert!(sequencer.submit(Generation(1), order(1)).is_ok());
        assert_eq!(
            sequencer.submit(Generation(1), order(2)),
            Err(ReplicationError::QueueFull)
        );
    }

    #[test]
    fn stale_generation_is_fenced() {
        let sequencer = Sequencer::new(4, Generation(9));
        assert_eq!(
            sequencer.submit(Generation(8), order(1)),
            Err(ReplicationError::Fenced)
        );
    }

    #[test]
    fn standby_tracks_primary_commands() {
        let sequencer = Sequencer::new(4, Generation(1));
        let mut primary = MatchingEngine::new();
        let mut standby = HotStandby::new(Generation(1));
        sequencer.submit(Generation(1), order(1)).unwrap();
        sequencer.submit(Generation(1), order(2)).unwrap();
        assert_eq!(
            sequencer
                .drain_into(&mut primary, Some(&mut standby))
                .unwrap(),
            2
        );
        assert_eq!(standby.applied(), 2);
        assert_eq!(standby.engine().book(0).unwrap().order_map.len(), 2);
        assert_eq!(primary.book(0).unwrap().order_map.len(), 2);
    }
}
