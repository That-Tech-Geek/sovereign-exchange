use crate::command::OrderCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommitIndex(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicatedEntry {
    pub index: CommitIndex,
    pub generation: u64,
    pub command: OrderCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuorumError {
    InvalidClusterSize,
    StaleGeneration,
    OutOfOrder {
        expected: CommitIndex,
        received: CommitIndex,
    },
    DuplicateAck,
}

#[derive(Debug)]
pub struct ReplicationLog {
    generation: u64,
    next_index: u64,
    entries: Vec<ReplicatedEntry>,
}

impl ReplicationLog {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            next_index: 1,
            entries: Vec::new(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn last_index(&self) -> CommitIndex {
        CommitIndex(self.next_index.saturating_sub(1))
    }

    pub fn append(
        &mut self,
        generation: u64,
        command: OrderCommand,
    ) -> Result<ReplicatedEntry, QuorumError> {
        if generation != self.generation {
            return Err(QuorumError::StaleGeneration);
        }
        let entry = ReplicatedEntry {
            index: CommitIndex(self.next_index),
            generation,
            command,
        };
        self.next_index = self
            .next_index
            .checked_add(1)
            .expect("replication index exhausted");
        self.entries.push(entry);
        Ok(entry)
    }

    pub fn get(&self, index: CommitIndex) -> Option<ReplicatedEntry> {
        index
            .0
            .checked_sub(1)
            .and_then(|i| self.entries.get(i as usize))
            .copied()
    }
}

#[derive(Debug)]
pub struct QuorumTracker {
    cluster_size: usize,
    committed: CommitIndex,
    acked: Vec<bool>,
    generation: u64,
}

impl QuorumTracker {
    pub fn new(cluster_size: usize, generation: u64) -> Result<Self, QuorumError> {
        if cluster_size == 0 || cluster_size.is_multiple_of(2) {
            return Err(QuorumError::InvalidClusterSize);
        }
        Ok(Self {
            cluster_size,
            committed: CommitIndex(0),
            acked: vec![false; cluster_size],
            generation,
        })
    }

    pub fn quorum(&self) -> usize {
        self.cluster_size / 2 + 1
    }
    pub fn committed(&self) -> CommitIndex {
        self.committed
    }

    pub fn ack(
        &mut self,
        generation: u64,
        index: CommitIndex,
        replica: usize,
    ) -> Result<bool, QuorumError> {
        if generation != self.generation {
            return Err(QuorumError::StaleGeneration);
        }
        if index.0 != self.committed.0 + 1 {
            return Err(QuorumError::OutOfOrder {
                expected: CommitIndex(self.committed.0 + 1),
                received: index,
            });
        }
        if replica >= self.cluster_size || self.acked[replica] {
            return Err(QuorumError::DuplicateAck);
        }
        self.acked[replica] = true;
        if self.acked.iter().filter(|acked| **acked).count() >= self.quorum() {
            self.committed = index;
            self.acked.fill(false);
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{NewOrder, OrderSide};
    use crate::order::ClientOrderId;

    fn command(id: u64) -> OrderCommand {
        OrderCommand::New(NewOrder {
            client_order_id: ClientOrderId(id),
            account_id: 1,
            instrument_id: 0,
            side: OrderSide::Buy,
            price: 100,
            quantity: 1,
            client_timestamp: id,
        })
    }

    #[test]
    fn majority_commit_requires_two_of_three() {
        let mut log = ReplicationLog::new(1);
        let entry = log.append(1, command(1)).unwrap();
        let mut quorum = QuorumTracker::new(3, 1).unwrap();
        assert!(!quorum.ack(1, entry.index, 0).unwrap());
        assert!(quorum.ack(1, entry.index, 1).unwrap());
        assert_eq!(quorum.committed(), entry.index);
    }

    #[test]
    fn stale_leader_generation_is_rejected() {
        let mut log = ReplicationLog::new(7);
        assert_eq!(log.append(6, command(1)), Err(QuorumError::StaleGeneration));
    }

    #[test]
    fn duplicate_ack_cannot_inflate_quorum() {
        let mut quorum = QuorumTracker::new(3, 1).unwrap();
        assert!(!quorum.ack(1, CommitIndex(1), 0).unwrap());
        assert_eq!(
            quorum.ack(1, CommitIndex(1), 0),
            Err(QuorumError::DuplicateAck)
        );
    }
}
