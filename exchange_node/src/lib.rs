#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = 64;
pub const FRAME_BYTES: usize = 56;
pub const MAGIC: u32 = 0x5345_5832;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageKind {
    Order = 1,
    Execution = 2,
    Ack = 3,
    GapRequest = 4,
    Heartbeat = 5,
    SnapshotHeader = 6,
}

impl TryFrom<u8> for MessageKind {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Order),
            2 => Ok(Self::Execution),
            3 => Ok(Self::Ack),
            4 => Ok(Self::GapRequest),
            5 => Ok(Self::Heartbeat),
            6 => Ok(Self::SnapshotHeader),
            _ => Err(ProtocolError::UnknownKind),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownKind,
    SequenceGap,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub kind: MessageKind,
    pub sender: u16,
    pub epoch: u64,
    pub sequence: u64,
    pub instrument: u16,
    pub payload: [u8; 24],
}

impl Frame {
    pub fn encode(self) -> [u8; FRAME_BYTES] {
        let mut out = [0u8; FRAME_BYTES];
        out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        out[4] = PROTOCOL_VERSION;
        out[5] = self.kind as u8;
        out[6..8].copy_from_slice(&self.sender.to_le_bytes());
        out[8..16].copy_from_slice(&self.epoch.to_le_bytes());
        out[16..24].copy_from_slice(&self.sequence.to_le_bytes());
        out[24..26].copy_from_slice(&self.instrument.to_le_bytes());
        out[32..56].copy_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != FRAME_BYTES {
            return Err(ProtocolError::InvalidLength);
        }
        if u32::from_le_bytes(bytes[0..4].try_into().unwrap()) != MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }
        if bytes[4] != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        Ok(Self {
            kind: MessageKind::try_from(bytes[5])?,
            sender: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            epoch: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            sequence: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            instrument: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
            payload: bytes[32..56].try_into().unwrap(),
        })
    }
}

#[derive(Debug, Default)]
pub struct PeerSequencer {
    next: HashMap<u16, u64>,
}

impl PeerSequencer {
    pub fn observe(&mut self, sender: u16, sequence: u64) -> Result<(), ProtocolError> {
        let expected = self.next.entry(sender).or_insert(1);
        match sequence.cmp(expected) {
            std::cmp::Ordering::Less => Err(ProtocolError::Duplicate),
            std::cmp::Ordering::Greater => Err(ProtocolError::SequenceGap),
            std::cmp::Ordering::Equal => {
                *expected = expected.saturating_add(1);
                Ok(())
            }
        }
    }
}

pub mod quic;

pub fn transport_timeout() -> Duration {
    Duration::from_millis(250)
}

#[derive(Debug, Default)]
pub struct PeerIngress {
    epochs: HashMap<u16, u64>,
    next: HashMap<u16, u64>,
    pending: HashMap<u16, BTreeMap<u64, Frame>>,
}

impl PeerIngress {
    pub fn observe(&mut self, frame: Frame) -> Result<(), ProtocolError> {
        let _ = self.ingest(frame)?;
        Ok(())
    }

    pub fn ingest(&mut self, frame: Frame) -> Result<Vec<Frame>, ProtocolError> {
        let current_epoch = self.epochs.entry(frame.sender).or_insert(frame.epoch);
        if frame.epoch < *current_epoch {
            return Err(ProtocolError::Duplicate);
        }
        if frame.epoch > *current_epoch {
            *current_epoch = frame.epoch;
            self.next.insert(frame.sender, 1);
            self.pending.remove(&frame.sender);
        }

        let expected = *self.next.entry(frame.sender).or_insert(1);
        if frame.sequence < expected {
            return Err(ProtocolError::Duplicate);
        }
        if frame.sequence > expected {
            self.pending
                .entry(frame.sender)
                .or_default()
                .entry(frame.sequence)
                .or_insert(frame);
            return Err(ProtocolError::SequenceGap);
        }

        let mut committed = vec![frame];
        self.next.insert(frame.sender, expected + 1);
        if let Some(buffer) = self.pending.get_mut(&frame.sender) {
            loop {
                let expected = *self.next.get(&frame.sender).unwrap_or(&1);
                let Some(next_frame) = buffer.remove(&expected) else {
                    break;
                };
                self.next.insert(frame.sender, expected + 1);
                committed.push(next_frame);
            }
        }
        if self
            .pending
            .get(&frame.sender)
            .is_some_and(BTreeMap::is_empty)
        {
            self.pending.remove(&frame.sender);
        }
        Ok(committed)
    }
}

