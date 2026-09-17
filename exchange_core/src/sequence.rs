/// Canonical exchange sequence assigned to every accepted command.
///
/// Sequence numbers are the exchange's ordering authority. Client timestamps
/// are metadata only and must never determine matching priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct SequenceNumber(pub u64);

impl SequenceNumber {
    pub const FIRST: Self = Self(1);

    #[inline(always)]
    pub const fn get(self) -> u64 {
        self.0
    }
}
