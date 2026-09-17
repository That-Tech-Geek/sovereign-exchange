//! Ticker registry and metadata management for country shares.

use crate::constants::MAX_TICKERS;

#[derive(Debug, Clone, Copy)]
pub struct CountryTicker {
    pub id: u16,
    pub symbol: &'static str,
    pub country_name: &'static str,
    pub currency: &'static str,
    pub tick_size: u32,  // e.g. 1 cent = 100
    pub base_price: u32, // initial scaled price (e.g. 10000 = $100.00)
    pub total_shares: u64,
}

pub static TICKERS: &[CountryTicker] = &[
    CountryTicker {
        id: 0,
        symbol: "USA",
        country_name: "United States",
        currency: "USD",
        tick_size: 1,
        base_price: 34500,
        total_shares: 10_000_000_000,
    },
    CountryTicker {
        id: 1,
        symbol: "GER",
        country_name: "Germany",
        currency: "EUR",
        tick_size: 1,
        base_price: 18200,
        total_shares: 4_500_000_000,
    },
    CountryTicker {
        id: 2,
        symbol: "JPN",
        country_name: "Japan",
        currency: "JPY",
        tick_size: 1,
        base_price: 22400,
        total_shares: 5_200_000_000,
    },
    CountryTicker {
        id: 3,
        symbol: "GBR",
        country_name: "United Kingdom",
        currency: "GBP",
        tick_size: 1,
        base_price: 16800,
        total_shares: 3_800_000_000,
    },
    CountryTicker {
        id: 4,
        symbol: "FRA",
        country_name: "France",
        currency: "EUR",
        tick_size: 1,
        base_price: 15900,
        total_shares: 3_600_000_000,
    },
    CountryTicker {
        id: 5,
        symbol: "CAN",
        country_name: "Canada",
        currency: "CAD",
        tick_size: 1,
        base_price: 14200,
        total_shares: 2_900_000_000,
    },
    CountryTicker {
        id: 6,
        symbol: "AUS",
        country_name: "Australia",
        currency: "AUD",
        tick_size: 1,
        base_price: 13500,
        total_shares: 2_700_000_000,
    },
    CountryTicker {
        id: 7,
        symbol: "CHE",
        country_name: "Switzerland",
        currency: "CHF",
        tick_size: 1,
        base_price: 28900,
        total_shares: 1_900_000_000,
    },
    CountryTicker {
        id: 8,
        symbol: "IND",
        country_name: "India",
        currency: "INR",
        tick_size: 1,
        base_price: 11200,
        total_shares: 8_400_000_000,
    },
    CountryTicker {
        id: 9,
        symbol: "BRA",
        country_name: "Brazil",
        currency: "BRL",
        tick_size: 1,
        base_price: 9800,
        total_shares: 3_100_000_000,
    },
    CountryTicker {
        id: 10,
        symbol: "SGP",
        country_name: "Singapore",
        currency: "SGD",
        tick_size: 1,
        base_price: 21500,
        total_shares: 1_200_000_000,
    },
    CountryTicker {
        id: 11,
        symbol: "KOR",
        country_name: "South Korea",
        currency: "KRW",
        tick_size: 1,
        base_price: 14700,
        total_shares: 2_500_000_000,
    },
];

pub struct TickerRegistry;

impl TickerRegistry {
    /// Resolve a ticker ID by country symbol.
    #[inline(always)]
    pub fn resolve_symbol(symbol: &str) -> Option<u16> {
        for ticker in TICKERS {
            if ticker.symbol.eq_ignore_ascii_case(symbol) {
                return Some(ticker.id);
            }
        }
        None
    }

    /// Retrieve country ticker metadata by ID.
    #[inline(always)]
    pub fn get(id: u16) -> Option<&'static CountryTicker> {
        TICKERS.iter().find(|t| t.id == id)
    }

    /// Return total registered tickers count.
    #[inline(always)]
    pub fn count() -> usize {
        std::cmp::min(TICKERS.len(), MAX_TICKERS)
    }
}
