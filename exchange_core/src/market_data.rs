use crate::book::OrderBook;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthLevel {
    pub price: u32,
    pub quantity: u32,
    pub order_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthSnapshot {
    pub instrument_id: u16,
    pub sequence: u64,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketDataEvent {
    Snapshot(DepthSnapshot),
    Delta {
        instrument_id: u16,
        sequence: u64,
        bids: Vec<DepthLevel>,
        asks: Vec<DepthLevel>,
    },
}

impl MarketDataEvent {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::Snapshot(snapshot) => snapshot.sequence,
            Self::Delta { sequence, .. } => *sequence,
        }
    }

    pub fn instrument_id(&self) -> u16 {
        match self {
            Self::Snapshot(snapshot) => snapshot.instrument_id,
            Self::Delta { instrument_id, .. } => *instrument_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataError {
    Gap { expected: u64, received: u64 },
    OutOfOrder { expected: u64, received: u64 },
}

#[derive(Debug)]
pub struct MarketDataPublisher {
    next_sequence: u64,
    subscribers: Vec<VecDeque<MarketDataEvent>>,
    capacity: usize,
}

impl MarketDataPublisher {
    pub fn new(subscriber_capacity: usize) -> Self {
        assert!(subscriber_capacity > 0);
        Self {
            next_sequence: 1,
            subscribers: Vec::new(),
            capacity: subscriber_capacity,
        }
    }

    pub fn subscribe(&mut self) -> usize {
        let id = self.subscribers.len();
        self.subscribers
            .push(VecDeque::with_capacity(self.capacity));
        id
    }

    pub fn publish_delta(
        &mut self,
        instrument_id: u16,
        bids: Vec<DepthLevel>,
        asks: Vec<DepthLevel>,
    ) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let event = MarketDataEvent::Delta {
            instrument_id,
            sequence,
            bids,
            asks,
        };
        for queue in &mut self.subscribers {
            if queue.len() == self.capacity {
                queue.pop_front();
            }
            queue.push_back(event.clone());
        }
        sequence
    }

    pub fn publish_snapshot(&mut self, snapshot: DepthSnapshot) -> u64 {
        let sequence = snapshot.sequence;
        self.next_sequence = self.next_sequence.max(sequence.saturating_add(1));
        let event = MarketDataEvent::Snapshot(snapshot);
        for queue in &mut self.subscribers {
            if queue.len() == self.capacity {
                queue.pop_front();
            }
            queue.push_back(event.clone());
        }
        sequence
    }

    pub fn poll(
        &mut self,
        subscriber_id: usize,
        expected_sequence: u64,
    ) -> Result<Option<MarketDataEvent>, MarketDataError> {
        let queue = self
            .subscribers
            .get_mut(subscriber_id)
            .expect("invalid market-data subscriber");
        let Some(event) = queue.pop_front() else {
            return Ok(None);
        };
        let received = event.sequence();
        if received > expected_sequence {
            queue.push_front(event);
            return Err(MarketDataError::Gap {
                expected: expected_sequence,
                received,
            });
        }
        if received < expected_sequence {
            return Err(MarketDataError::OutOfOrder {
                expected: expected_sequence,
                received,
            });
        }
        Ok(Some(event))
    }

    pub fn queue_depth(&self, subscriber_id: usize) -> usize {
        self.subscribers
            .get(subscriber_id)
            .map(VecDeque::len)
            .unwrap_or(0)
    }
}

pub fn full_depth_snapshot(instrument_id: u16, sequence: u64, book: &OrderBook) -> DepthSnapshot {
    let bids = book
        .bids
        .iter()
        .rev()
        .map(|(&price, level)| DepthLevel {
            price,
            quantity: level.volume,
            order_count: level.order_count,
        })
        .collect();
    let asks = book
        .asks
        .iter()
        .map(|(&price, level)| DepthLevel {
            price,
            quantity: level.volume,
            order_count: level.order_count,
        })
        .collect();
    DepthSnapshot {
        instrument_id,
        sequence,
        bids,
        asks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_dropped_sequence_and_preserves_event() {
        let mut publisher = MarketDataPublisher::new(2);
        let subscriber = publisher.subscribe();
        publisher.publish_delta(0, vec![], vec![]);
        publisher.publish_delta(0, vec![], vec![]);
        publisher.publish_delta(0, vec![], vec![]);

        assert_eq!(
            publisher.poll(subscriber, 1),
            Err(MarketDataError::Gap {
                expected: 1,
                received: 2
            })
        );
        assert_eq!(publisher.queue_depth(subscriber), 2);
    }

    #[test]
    fn full_depth_is_price_time_aggregated() {
        let mut book = OrderBook::new();
        let mut pool = crate::pool::OrderPool::new();
        let idx = pool.allocate().unwrap();
        pool.data[idx as usize].side = 0;
        pool.data[idx as usize].price = 100;
        pool.data[idx as usize].remaining = 7;
        pool.data[idx as usize].account_id = 1;
        pool.data[idx as usize].client_order_id = 1;
        book.insert_limit(idx, &mut pool);

        let snapshot = full_depth_snapshot(0, 9, &book);
        assert_eq!(snapshot.sequence, 9);
        assert_eq!(snapshot.bids[0].price, 100);
        assert_eq!(snapshot.bids[0].quantity, 7);
        assert_eq!(snapshot.bids[0].order_count, 1);
    }
}
