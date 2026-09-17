//! Backwards-compatible country metadata facade.
//!
//! The exchange core now uses `instrument` as the authoritative market model.
//! This module remains as a compatibility layer for existing callers while the
//! codebase migrates away from ticker terminology.

use crate::instrument::{SovereignDefinition, SOVEREIGNS};

pub type CountryTicker = SovereignDefinition;
pub static TICKERS: &[CountryTicker] = SOVEREIGNS;

pub struct TickerRegistry;

impl TickerRegistry {
    #[inline(always)]
    pub fn resolve_symbol(symbol: &str) -> Option<u16> {
        TICKERS
            .iter()
            .find(|ticker| ticker.symbol.eq_ignore_ascii_case(symbol))
            .map(|ticker| ticker.id)
    }

    #[inline(always)]
    pub fn get(id: u16) -> Option<&'static CountryTicker> {
        TICKERS.iter().find(|ticker| ticker.id == id)
    }

    #[inline(always)]
    pub fn count() -> usize {
        TICKERS.len()
    }
}
