use std::collections::BTreeMap;

pub type AccountId = u32;
pub type TradeId = u64;
pub type Sequence = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerKind {
    TradeCash,
    TradeSecurity,
    Fee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub trade_id: TradeId,
    pub sequence: Sequence,
    pub debit_account: AccountId,
    pub credit_account: AccountId,
    pub asset: AssetId,
    pub amount: i128,
    pub kind: LedgerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerEntry {
    pub entry_id: u64,
    pub trade_id: TradeId,
    pub sequence: Sequence,
    pub debit_account: AccountId,
    pub credit_account: AccountId,
    pub asset: AssetId,
    pub amount: i128,
    pub kind: LedgerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    InvalidAmount,
    SelfTransfer,
    InsufficientBalance,
    ArithmeticOverflow,
    DuplicateTrade,
    UnknownTrade,
}

#[derive(Debug, Default)]
pub struct DoubleEntryLedger {
    balances: BTreeMap<(AccountId, AssetId), i128>,
    entries: Vec<LedgerEntry>,
    next_entry_id: u64,
}

impl DoubleEntryLedger {
    pub fn new() -> Self {
        Self {
            balances: BTreeMap::new(),
            entries: Vec::new(),
            next_entry_id: 1,
        }
    }

    pub fn seed(
        &mut self,
        account_id: AccountId,
        asset: AssetId,
        amount: i128,
    ) -> Result<(), LedgerError> {
        if amount < 0 {
            return Err(LedgerError::InvalidAmount);
        }
        let balance = self.balances.entry((account_id, asset)).or_default();
        *balance = balance
            .checked_add(amount)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn balance(&self, account_id: AccountId, asset: AssetId) -> i128 {
        self.balances
            .get(&(account_id, asset))
            .copied()
            .unwrap_or_default()
    }

    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    pub fn post_batch(&mut self, transfers: &[Transfer]) -> Result<(), LedgerError> {
        if transfers.is_empty() {
            return Ok(());
        }

        let mut deltas: BTreeMap<(AccountId, AssetId), i128> = BTreeMap::new();
        for transfer in transfers {
            if transfer.amount <= 0 {
                return Err(LedgerError::InvalidAmount);
            }
            if transfer.debit_account == transfer.credit_account {
                return Err(LedgerError::SelfTransfer);
            }

            let debit = deltas
                .entry((transfer.debit_account, transfer.asset))
                .or_default();
            *debit = debit
                .checked_sub(transfer.amount)
                .ok_or(LedgerError::ArithmeticOverflow)?;

            let credit = deltas
                .entry((transfer.credit_account, transfer.asset))
                .or_default();
            *credit = credit
                .checked_add(transfer.amount)
                .ok_or(LedgerError::ArithmeticOverflow)?;
        }

        for ((account, asset), delta) in &deltas {
            let current = self.balance(*account, *asset);
            let next = current
                .checked_add(*delta)
                .ok_or(LedgerError::ArithmeticOverflow)?;
            if next < 0 {
                return Err(LedgerError::InsufficientBalance);
            }
        }

        for transfer in transfers {
            let debit_key = (transfer.debit_account, transfer.asset);
            let credit_key = (transfer.credit_account, transfer.asset);
            let debit = self.balances.entry(debit_key).or_default();
            *debit = debit
                .checked_sub(transfer.amount)
                .ok_or(LedgerError::ArithmeticOverflow)?;
            let credit = self.balances.entry(credit_key).or_default();
            *credit = credit
                .checked_add(transfer.amount)
                .ok_or(LedgerError::ArithmeticOverflow)?;

            let entry = LedgerEntry {
                entry_id: self.next_entry_id,
                trade_id: transfer.trade_id,
                sequence: transfer.sequence,
                debit_account: transfer.debit_account,
                credit_account: transfer.credit_account,
                asset: transfer.asset,
                amount: transfer.amount,
                kind: transfer.kind,
            };
            self.next_entry_id = self
                .next_entry_id
                .checked_add(1)
                .ok_or(LedgerError::ArithmeticOverflow)?;
            self.entries.push(entry);
        }

        Ok(())
    }

    pub fn asset_total(&self, asset: AssetId) -> i128 {
        self.balances
            .iter()
            .filter(|((_, current_asset), _)| *current_asset == asset)
            .map(|(_, balance)| *balance)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeRole {
    Maker,
    Taker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSchedule {
    pub maker_bps: u32,
    pub taker_bps: u32,
}

impl FeeSchedule {
    pub fn new(maker_bps: u32, taker_bps: u32) -> Self {
        Self {
            maker_bps,
            taker_bps,
        }
    }

    pub fn fee(&self, notional: i128, role: FeeRole) -> Result<i128, LedgerError> {
        if notional < 0 {
            return Err(LedgerError::InvalidAmount);
        }
        let bps = match role {
            FeeRole::Maker => self.maker_bps,
            FeeRole::Taker => self.taker_bps,
        };
        let product = notional
            .checked_mul(bps as i128)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        Ok(product / 10_000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeSettlement {
    pub trade_id: TradeId,
    pub sequence: Sequence,
    pub buyer_account: AccountId,
    pub seller_account: AccountId,
    pub instrument_asset: AssetId,
    pub cash_asset: AssetId,
    pub price: u32,
    pub quantity: u32,
    pub buyer_fee: i128,
    pub seller_fee: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementState {
    Pending,
    Settled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementRecord {
    pub trade_id: TradeId,
    pub state: SettlementState,
    pub error: Option<LedgerError>,
}

#[derive(Debug)]
pub struct SettlementEngine {
    pub ledger: DoubleEntryLedger,
    settlements: BTreeMap<TradeId, SettlementRecord>,
}

impl SettlementEngine {
    pub fn new(ledger: DoubleEntryLedger) -> Self {
        Self {
            ledger,
            settlements: BTreeMap::new(),
        }
    }

    pub fn settlement(&self, trade_id: TradeId) -> Option<SettlementRecord> {
        self.settlements.get(&trade_id).copied()
    }

    pub fn settle_trade(
        &mut self,
        trade: TradeSettlement,
        fee_schedule: FeeSchedule,
        fee_account: AccountId,
    ) -> Result<SettlementRecord, LedgerError> {
        if self.settlements.contains_key(&trade.trade_id) {
            return Err(LedgerError::DuplicateTrade);
        }

        let notional = (trade.price as i128)
            .checked_mul(trade.quantity as i128)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        let buyer_fee = fee_schedule.fee(notional, FeeRole::Taker)?;
        let seller_fee = fee_schedule.fee(notional, FeeRole::Maker)?;

        let transfers = [
            Transfer {
                trade_id: trade.trade_id,
                sequence: trade.sequence,
                debit_account: trade.buyer_account,
                credit_account: trade.seller_account,
                asset: trade.cash_asset,
                amount: notional,
                kind: LedgerKind::TradeCash,
            },
            Transfer {
                trade_id: trade.trade_id,
                sequence: trade.sequence,
                debit_account: trade.seller_account,
                credit_account: trade.buyer_account,
                asset: trade.instrument_asset,
                amount: trade.quantity as i128,
                kind: LedgerKind::TradeSecurity,
            },
            Transfer {
                trade_id: trade.trade_id,
                sequence: trade.sequence,
                debit_account: trade.buyer_account,
                credit_account: fee_account,
                asset: trade.cash_asset,
                amount: buyer_fee,
                kind: LedgerKind::Fee,
            },
            Transfer {
                trade_id: trade.trade_id,
                sequence: trade.sequence,
                debit_account: trade.seller_account,
                credit_account: fee_account,
                asset: trade.cash_asset,
                amount: seller_fee,
                kind: LedgerKind::Fee,
            },
        ];

        let transfers = transfers
            .iter()
            .copied()
            .filter(|transfer| transfer.amount > 0)
            .collect::<Vec<_>>();

        if let Err(error) = self.ledger.post_batch(&transfers) {
            let record = SettlementRecord {
                trade_id: trade.trade_id,
                state: SettlementState::Failed,
                error: Some(error),
            };
            self.settlements.insert(trade.trade_id, record);
            return Err(error);
        }

        let record = SettlementRecord {
            trade_id: trade.trade_id,
            state: SettlementState::Settled,
            error: None,
        };
        self.settlements.insert(trade.trade_id, record);
        Ok(record)
    }

    pub fn reconcile_asset(&self, asset: AssetId, expected_total: i128) -> bool {
        self.ledger.asset_total(asset) == expected_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASH: AssetId = AssetId(0);
    const STOCK: AssetId = AssetId(1000);
    const FEE: AccountId = 999;

    fn funded_ledger() -> DoubleEntryLedger {
        let mut ledger = DoubleEntryLedger::new();
        ledger.seed(1, CASH, 100_000).unwrap();
        ledger.seed(2, CASH, 100_000).unwrap();
        ledger.seed(2, STOCK, 1_000).unwrap();
        ledger
    }

    #[test]
    fn ledger_is_zero_sum_per_asset_after_transfer() {
        let mut ledger = funded_ledger();
        let before_cash = ledger.asset_total(CASH);
        let before_stock = ledger.asset_total(STOCK);

        ledger
            .post_batch(&[
                Transfer {
                    trade_id: 1,
                    sequence: 1,
                    debit_account: 1,
                    credit_account: 2,
                    asset: CASH,
                    amount: 10_000,
                    kind: LedgerKind::TradeCash,
                },
                Transfer {
                    trade_id: 1,
                    sequence: 1,
                    debit_account: 2,
                    credit_account: 1,
                    asset: STOCK,
                    amount: 100,
                    kind: LedgerKind::TradeSecurity,
                },
            ])
            .unwrap();

        assert_eq!(ledger.asset_total(CASH), before_cash);
        assert_eq!(ledger.asset_total(STOCK), before_stock);
    }

    #[test]
    fn batch_is_atomic_on_insufficient_balance() {
        let mut ledger = funded_ledger();
        let before = ledger.balance(1, CASH);

        let result = ledger.post_batch(&[
            Transfer {
                trade_id: 2,
                sequence: 2,
                debit_account: 1,
                credit_account: 2,
                asset: CASH,
                amount: 10_000,
                kind: LedgerKind::TradeCash,
            },
            Transfer {
                trade_id: 2,
                sequence: 2,
                debit_account: 1,
                credit_account: 2,
                asset: CASH,
                amount: 1_000_000,
                kind: LedgerKind::TradeCash,
            },
        ]);

        assert_eq!(result, Err(LedgerError::InsufficientBalance));
        assert_eq!(ledger.balance(1, CASH), before);
        assert!(ledger.entries().is_empty());
    }

    #[test]
    fn fee_calculation_is_integer_and_deterministic() {
        let schedule = FeeSchedule::new(1, 25);
        assert_eq!(schedule.fee(10_000, FeeRole::Taker).unwrap(), 25);
        assert_eq!(schedule.fee(10_000, FeeRole::Maker).unwrap(), 1);
    }

    #[test]
    fn trade_settlement_moves_cash_security_and_fees_atomically() {
        let mut engine = SettlementEngine::new(funded_ledger());

        let record = engine
            .settle_trade(
                TradeSettlement {
                    trade_id: 7,
                    sequence: 7,
                    buyer_account: 1,
                    seller_account: 2,
                    instrument_asset: STOCK,
                    cash_asset: CASH,
                    price: 100,
                    quantity: 100,
                    buyer_fee: 0,
                    seller_fee: 0,
                },
                FeeSchedule::new(10, 20),
                FEE,
            )
            .unwrap();

        assert_eq!(record.state, SettlementState::Settled);
        assert_eq!(engine.ledger.balance(1, CASH), 89_980);
        assert_eq!(engine.ledger.balance(2, CASH), 109_990);
        assert_eq!(engine.ledger.balance(1, STOCK), 100);
        assert_eq!(engine.ledger.balance(2, STOCK), 900);
        assert_eq!(engine.ledger.balance(FEE, CASH), 30);
        assert_eq!(engine.ledger.asset_total(CASH), 200_000);
        assert_eq!(engine.ledger.asset_total(STOCK), 1_000);
    }

    #[test]
    fn duplicate_trade_is_rejected() {
        let mut engine = SettlementEngine::new(funded_ledger());
        let trade = TradeSettlement {
            trade_id: 8,
            sequence: 8,
            buyer_account: 1,
            seller_account: 2,
            instrument_asset: STOCK,
            cash_asset: CASH,
            price: 100,
            quantity: 10,
            buyer_fee: 0,
            seller_fee: 0,
        };
        engine
            .settle_trade(trade, FeeSchedule::new(0, 0), FEE)
            .unwrap();
        assert_eq!(
            engine.settle_trade(trade, FeeSchedule::new(0, 0), FEE),
            Err(LedgerError::DuplicateTrade)
        );
    }
}
