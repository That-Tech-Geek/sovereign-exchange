//! Instrument definitions and registry for the sovereign exchange.
//!
//! The exchange supports up to 196 sovereigns with one spot and one futures
//! instrument per sovereign, for a maximum of 392 instruments.

use crate::constants::{MAX_INSTRUMENTS, MAX_SOVEREIGNS};

/// Market type for a sovereign instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MarketType {
    Spot = 0,
    Future = 1,
}

/// Lifecycle state of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InstrumentStatus {
    PreOpen = 0,
    Open = 1,
    Halted = 2,
    Closed = 3,
    Expired = 4,
    Settled = 5,
}

/// Static metadata needed to identify and validate an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instrument {
    pub id: u16,
    pub sovereign_id: u16,
    pub market_type: MarketType,
    pub symbol: &'static str,
    pub currency: &'static str,
    pub tick_size: u32,
    pub base_price: u32,
    pub status: InstrumentStatus,
}

impl Instrument {
    #[inline(always)]
    pub const fn spot(
        id: u16,
        sovereign_id: u16,
        symbol: &'static str,
        currency: &'static str,
        tick_size: u32,
        base_price: u32,
    ) -> Self {
        Self {
            id,
            sovereign_id,
            market_type: MarketType::Spot,
            symbol,
            currency,
            tick_size,
            base_price,
            status: InstrumentStatus::PreOpen,
        }
    }

    #[inline(always)]
    pub const fn future(
        id: u16,
        sovereign_id: u16,
        symbol: &'static str,
        currency: &'static str,
        tick_size: u32,
        base_price: u32,
    ) -> Self {
        Self {
            id,
            sovereign_id,
            market_type: MarketType::Future,
            symbol,
            currency,
            tick_size,
            base_price,
            status: InstrumentStatus::PreOpen,
        }
    }
}

/// Deterministic instrument ID mapping.
///
/// For sovereign N:
/// - spot   = 2 * N
/// - future = 2 * N + 1
#[inline(always)]
pub const fn spot_instrument_id(sovereign_id: u16) -> u16 {
    sovereign_id.saturating_mul(2)
}

#[inline(always)]
pub const fn future_instrument_id(sovereign_id: u16) -> u16 {
    sovereign_id.saturating_mul(2).saturating_add(1)
}

/// Fixed-capacity registry. Empty slots are valid because the exchange can
/// support the full 392-instrument universe without requiring all metadata to
/// be compiled into the binary at this stage.
pub struct InstrumentRegistry {
    instruments: Vec<Option<Instrument>>,
}

impl InstrumentRegistry {
    pub fn new() -> Self {
        Self {
            instruments: vec![None; MAX_INSTRUMENTS],
        }
    }

    /// Build a registry from the sovereign metadata currently present in the
    /// repository. Additional sovereigns can be registered without changing
    /// the matching architecture.
    pub fn from_sovereigns(sovereigns: &[SovereignDefinition]) -> Self {
        assert!(sovereigns.len() <= MAX_SOVEREIGNS);

        let mut registry = Self::new();
        for sovereign in sovereigns {
            registry.register_pair(*sovereign);
        }
        registry
    }

    #[inline]
    pub fn register_pair(&mut self, sovereign: SovereignDefinition) {
        assert!((sovereign.id as usize) < MAX_SOVEREIGNS);

        let spot_id = spot_instrument_id(sovereign.id);
        let future_id = future_instrument_id(sovereign.id);

        self.register(Instrument::spot(
            spot_id,
            sovereign.id,
            sovereign.symbol,
            sovereign.currency,
            sovereign.tick_size,
            sovereign.base_price,
        ));

        self.register(Instrument::future(
            future_id,
            sovereign.id,
            sovereign.symbol,
            sovereign.currency,
            sovereign.tick_size,
            sovereign.base_price,
        ));
    }

    #[inline]
    pub fn register(&mut self, instrument: Instrument) {
        assert!((instrument.id as usize) < MAX_INSTRUMENTS);
        self.instruments[instrument.id as usize] = Some(instrument);
    }

    #[inline(always)]
    pub fn get(&self, id: u16) -> Option<&Instrument> {
        self.instruments.get(id as usize).and_then(Option::as_ref)
    }

    #[inline(always)]
    pub fn contains(&self, id: u16) -> bool {
        self.get(id).is_some()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.instruments
            .iter()
            .filter(|instrument| instrument.is_some())
            .count()
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        MAX_INSTRUMENTS
    }
}

impl Default for InstrumentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Existing country metadata adapted into a sovereign-level definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SovereignDefinition {
    pub id: u16,
    pub symbol: &'static str,
    pub country_name: &'static str,
    pub currency: &'static str,
    pub tick_size: u32,
    pub base_price: u32,
    pub total_shares: u64,
}

/// Seed sovereign universe currently represented in the repository.
///
/// This intentionally remains the existing 12-country dataset. The registry
/// capacity is 196 sovereigns / 392 instruments; adding sovereign metadata is
/// a separate data task and does not require another engine architecture.
pub const SOVEREIGNS: &[SovereignDefinition] = &[
    SovereignDefinition {
        id: 0,
        symbol: "USA",
        country_name: "United States",
        currency: "USD",
        tick_size: 1,
        base_price: 34500,
        total_shares: 10_000_000_000,
    },
    SovereignDefinition {
        id: 1,
        symbol: "GER",
        country_name: "Germany",
        currency: "EUR",
        tick_size: 1,
        base_price: 18200,
        total_shares: 4_500_000_000,
    },
    SovereignDefinition {
        id: 2,
        symbol: "JPN",
        country_name: "Japan",
        currency: "JPY",
        tick_size: 1,
        base_price: 22400,
        total_shares: 5_200_000_000,
    },
    SovereignDefinition {
        id: 3,
        symbol: "GBR",
        country_name: "United Kingdom",
        currency: "GBP",
        tick_size: 1,
        base_price: 16800,
        total_shares: 3_800_000_000,
    },
    SovereignDefinition {
        id: 4,
        symbol: "FRA",
        country_name: "France",
        currency: "EUR",
        tick_size: 1,
        base_price: 15900,
        total_shares: 3_600_000_000,
    },
    SovereignDefinition {
        id: 5,
        symbol: "CAN",
        country_name: "Canada",
        currency: "CAD",
        tick_size: 1,
        base_price: 14200,
        total_shares: 2_900_000_000,
    },
    SovereignDefinition {
        id: 6,
        symbol: "AUS",
        country_name: "Australia",
        currency: "AUD",
        tick_size: 1,
        base_price: 13500,
        total_shares: 2_700_000_000,
    },
    SovereignDefinition {
        id: 7,
        symbol: "CHE",
        country_name: "Switzerland",
        currency: "CHF",
        tick_size: 1,
        base_price: 28900,
        total_shares: 1_900_000_000,
    },
    SovereignDefinition {
        id: 8,
        symbol: "IND",
        country_name: "India",
        currency: "INR",
        tick_size: 1,
        base_price: 11200,
        total_shares: 8_400_000_000,
    },
    SovereignDefinition {
        id: 9,
        symbol: "BRA",
        country_name: "Brazil",
        currency: "BRL",
        tick_size: 1,
        base_price: 9800,
        total_shares: 3_100_000_000,
    },
    SovereignDefinition {
        id: 10,
        symbol: "SGP",
        country_name: "Singapore",
        currency: "SGD",
        tick_size: 1,
        base_price: 21500,
        total_shares: 1_200_000_000,
    },
    SovereignDefinition {
        id: 11,
        symbol: "KOR",
        country_name: "South Korea",
        currency: "KRW",
        tick_size: 1,
        base_price: 14700,
        total_shares: 2_500_000_000,
    },
];
