use std::collections::BTreeMap;

use crate::command::{OrderCommand, OrderSide};

pub type Cash = i128;
pub type Quantity = u64;
pub type Notional = i128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Active,
    Frozen,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskLimits {
    pub max_order_quantity: Quantity,
    pub max_order_notional: Notional,
    pub max_gross_notional: Notional,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_order_quantity: u64::MAX,
            max_order_notional: i128::MAX,
            max_gross_notional: i128::MAX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub account_id: u32,
    pub status: AccountStatus,
    pub cash: Cash,
    pub reserved_cash: Cash,
    pub positions: BTreeMap<u16, Quantity>,
    pub reserved_positions: BTreeMap<u16, Quantity>,
    pub limits: RiskLimits,
}

impl Account {
    pub fn new(account_id: u32) -> Self {
        Self {
            account_id,
            status: AccountStatus::Active,
            cash: 0,
            reserved_cash: 0,
            positions: BTreeMap::new(),
            reserved_positions: BTreeMap::new(),
            limits: RiskLimits::default(),
        }
    }

    pub fn available_cash(&self) -> Cash {
        self.cash.saturating_sub(self.reserved_cash)
    }

    pub fn available_position(&self, instrument_id: u16) -> Quantity {
        self.positions
            .get(&instrument_id)
            .copied()
            .unwrap_or_default()
            .saturating_sub(
                self.reserved_positions
                    .get(&instrument_id)
                    .copied()
                    .unwrap_or_default(),
            )
    }

    pub fn position(&self, instrument_id: u16) -> Quantity {
        self.positions
            .get(&instrument_id)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskError {
    UnknownAccount,
    AccountInactive,
    InvalidPrice,
    InvalidQuantity,
    QuantityLimit,
    NotionalLimit,
    InsufficientCash,
    InsufficientPosition,
    GrossExposureLimit,
    ArithmeticOverflow,
    ReservationMissing,
    DuplicateReservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reservation {
    pub account_id: u32,
    pub instrument_id: u16,
    pub client_order_id: u64,
    pub side: OrderSide,
    pub price: u32,
    pub quantity: u32,
    pub reserved_value: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskFill {
    pub buyer_account: u32,
    pub seller_account: u32,
    pub instrument_id: u16,
    pub buyer_client_order_id: u64,
    pub seller_client_order_id: u64,
    pub price: u32,
    pub quantity: u32,
}

#[derive(Debug, Default)]
pub struct RiskEngine {
    accounts: BTreeMap<u32, Account>,
    reservations: BTreeMap<(u32, u16, u64), Reservation>,
}

impl RiskEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_account(&mut self, account_id: u32) -> Result<(), RiskError> {
        if self.accounts.contains_key(&account_id) {
            return Err(RiskError::DuplicateReservation);
        }
        self.accounts.insert(account_id, Account::new(account_id));
        Ok(())
    }

    pub fn account(&self, account_id: u32) -> Option<&Account> {
        self.accounts.get(&account_id)
    }

    pub fn account_mut(&mut self, account_id: u32) -> Option<&mut Account> {
        self.accounts.get_mut(&account_id)
    }

    pub fn fund(&mut self, account_id: u32, amount: Cash) -> Result<(), RiskError> {
        if amount < 0 {
            return Err(RiskError::InvalidPrice);
        }
        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;
        account.cash = account
            .cash
            .checked_add(amount)
            .ok_or(RiskError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn credit_position(
        &mut self,
        account_id: u32,
        instrument_id: u16,
        quantity: Quantity,
    ) -> Result<(), RiskError> {
        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;
        let current = account.position(instrument_id);
        let next = current
            .checked_add(quantity)
            .ok_or(RiskError::ArithmeticOverflow)?;
        account.positions.insert(instrument_id, next);
        Ok(())
    }

    pub fn set_limits(&mut self, account_id: u32, limits: RiskLimits) -> Result<(), RiskError> {
        if limits.max_order_notional < 0 || limits.max_gross_notional < 0 {
            return Err(RiskError::InvalidPrice);
        }
        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;
        account.limits = limits;
        Ok(())
    }

    pub fn set_status(&mut self, account_id: u32, status: AccountStatus) -> Result<(), RiskError> {
        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;
        account.status = status;
        Ok(())
    }

    pub fn admit(&mut self, command: &OrderCommand) -> Result<Option<Reservation>, RiskError> {
        match command {
            OrderCommand::New(order) => self.reserve(
                order.account_id,
                order.instrument_id,
                order.client_order_id.0,
                order.side,
                order.price,
                order.quantity,
            ),
            OrderCommand::Cancel(order) => {
                self.cancel_reservation(
                    order.account_id,
                    order.instrument_id,
                    order.client_order_id.0,
                )?;
                Ok(None)
            }
            OrderCommand::Replace(order) => {
                self.cancel_reservation(
                    order.account_id,
                    order.instrument_id,
                    order.target_client_order_id.0,
                )?;
                self.reserve(
                    order.account_id,
                    order.instrument_id,
                    order.new_client_order_id.0,
                    order.side,
                    order.price,
                    order.quantity,
                )
            }
        }
    }

    pub fn reserve(
        &mut self,
        account_id: u32,
        instrument_id: u16,
        client_order_id: u64,
        side: OrderSide,
        price: u32,
        quantity: u32,
    ) -> Result<Option<Reservation>, RiskError> {
        if price == 0 {
            return Err(RiskError::InvalidPrice);
        }
        if quantity == 0 {
            return Err(RiskError::InvalidQuantity);
        }
        let key = (account_id, instrument_id, client_order_id);
        if self.reservations.contains_key(&key) {
            return Err(RiskError::DuplicateReservation);
        }

        let notional = (price as i128)
            .checked_mul(quantity as i128)
            .ok_or(RiskError::ArithmeticOverflow)?;

        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;

        if account.status != AccountStatus::Active {
            return Err(RiskError::AccountInactive);
        }
        if quantity as u64 > account.limits.max_order_quantity {
            return Err(RiskError::QuantityLimit);
        }
        if notional > account.limits.max_order_notional {
            return Err(RiskError::NotionalLimit);
        }

        match side {
            OrderSide::Buy => {
                if account.available_cash() < notional {
                    return Err(RiskError::InsufficientCash);
                }
                account.reserved_cash = account
                    .reserved_cash
                    .checked_add(notional)
                    .ok_or(RiskError::ArithmeticOverflow)?;
            }
            OrderSide::Sell => {
                if account.available_position(instrument_id) < quantity as u64 {
                    return Err(RiskError::InsufficientPosition);
                }
                let current = account
                    .reserved_positions
                    .get(&instrument_id)
                    .copied()
                    .unwrap_or_default();
                account
                    .reserved_positions
                    .insert(instrument_id, current + quantity as u64);
            }
        }

        let reservation = Reservation {
            account_id,
            instrument_id,
            client_order_id,
            side,
            price,
            quantity,
            reserved_value: notional,
        };
        self.reservations.insert(key, reservation);
        Ok(Some(reservation))
    }

    pub fn cancel_reservation(
        &mut self,
        account_id: u32,
        instrument_id: u16,
        client_order_id: u64,
    ) -> Result<Reservation, RiskError> {
        let key = (account_id, instrument_id, client_order_id);
        let reservation = self
            .reservations
            .remove(&key)
            .ok_or(RiskError::ReservationMissing)?;
        let account = self
            .accounts
            .get_mut(&account_id)
            .ok_or(RiskError::UnknownAccount)?;

        match reservation.side {
            OrderSide::Buy => {
                account.reserved_cash = account
                    .reserved_cash
                    .checked_sub(reservation.reserved_value)
                    .ok_or(RiskError::ArithmeticOverflow)?;
            }
            OrderSide::Sell => {
                let current = account
                    .reserved_positions
                    .get(&instrument_id)
                    .copied()
                    .unwrap_or_default();
                let next = current
                    .checked_sub(reservation.quantity as u64)
                    .ok_or(RiskError::ArithmeticOverflow)?;
                if next == 0 {
                    account.reserved_positions.remove(&instrument_id);
                } else {
                    account.reserved_positions.insert(instrument_id, next);
                }
            }
        }
        Ok(reservation)
    }

    pub fn apply_fill(&mut self, fill: RiskFill) -> Result<(), RiskError> {
        let buyer_account = fill.buyer_account;
        let seller_account = fill.seller_account;
        let instrument_id = fill.instrument_id;
        let buyer_client_order_id = fill.buyer_client_order_id;
        let seller_client_order_id = fill.seller_client_order_id;
        let price = fill.price;
        let quantity = fill.quantity;
        if price == 0 {
            return Err(RiskError::InvalidPrice);
        }
        if quantity == 0 {
            return Err(RiskError::InvalidQuantity);
        }

        let buyer_key = (buyer_account, instrument_id, buyer_client_order_id);
        let seller_key = (seller_account, instrument_id, seller_client_order_id);
        let buyer_reservation = self
            .reservations
            .get(&buyer_key)
            .copied()
            .ok_or(RiskError::ReservationMissing)?;
        let seller_reservation = self
            .reservations
            .get(&seller_key)
            .copied()
            .ok_or(RiskError::ReservationMissing)?;

        let notional = (price as i128)
            .checked_mul(quantity as i128)
            .ok_or(RiskError::ArithmeticOverflow)?;
        let reserved_buy = (buyer_reservation.price as i128)
            .checked_mul(quantity as i128)
            .ok_or(RiskError::ArithmeticOverflow)?;

        {
            let buyer = self
                .accounts
                .get_mut(&buyer_account)
                .ok_or(RiskError::UnknownAccount)?;
            if buyer.available_cash() + reserved_buy < notional {
                return Err(RiskError::InsufficientCash);
            }
            buyer.cash = buyer
                .cash
                .checked_sub(notional)
                .ok_or(RiskError::ArithmeticOverflow)?;
            buyer.reserved_cash = buyer
                .reserved_cash
                .checked_sub(reserved_buy)
                .ok_or(RiskError::ArithmeticOverflow)?;
            let current = buyer.position(instrument_id);
            buyer.positions.insert(
                instrument_id,
                current
                    .checked_add(quantity as u64)
                    .ok_or(RiskError::ArithmeticOverflow)?,
            );
        }

        {
            let seller = self
                .accounts
                .get_mut(&seller_account)
                .ok_or(RiskError::UnknownAccount)?;
            let available = seller.available_position(instrument_id);
            if available < quantity as u64 {
                return Err(RiskError::InsufficientPosition);
            }
            let current_position = seller.position(instrument_id);
            seller.positions.insert(
                instrument_id,
                current_position
                    .checked_sub(quantity as u64)
                    .ok_or(RiskError::ArithmeticOverflow)?,
            );
            let reserved = seller
                .reserved_positions
                .get(&instrument_id)
                .copied()
                .unwrap_or_default();
            let next_reserved = reserved
                .checked_sub(quantity as u64)
                .ok_or(RiskError::ArithmeticOverflow)?;
            if next_reserved == 0 {
                seller.reserved_positions.remove(&instrument_id);
            } else {
                seller
                    .reserved_positions
                    .insert(instrument_id, next_reserved);
            }
            seller.cash = seller
                .cash
                .checked_add(notional)
                .ok_or(RiskError::ArithmeticOverflow)?;
        }

        if buyer_reservation.quantity == quantity {
            self.reservations.remove(&buyer_key);
        } else {
            let mut updated = buyer_reservation;
            updated.quantity -= quantity;
            updated.reserved_value = updated
                .price
                .checked_mul(updated.quantity)
                .map(|v| v as i128)
                .ok_or(RiskError::ArithmeticOverflow)?;
            self.reservations.insert(buyer_key, updated);
        }

        if seller_reservation.quantity == quantity {
            self.reservations.remove(&seller_key);
        } else {
            let mut updated = seller_reservation;
            updated.quantity -= quantity;
            updated.reserved_value = updated.quantity as i128;
            self.reservations.insert(seller_key, updated);
        }

        Ok(())
    }

    pub fn reservation(
        &self,
        account_id: u32,
        instrument_id: u16,
        client_order_id: u64,
    ) -> Option<Reservation> {
        self.reservations
            .get(&(account_id, instrument_id, client_order_id))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CancelOrder, NewOrder};
    use crate::order::ClientOrderId;

    fn new_order(
        account_id: u32,
        id: u64,
        side: OrderSide,
        price: u32,
        quantity: u32,
    ) -> OrderCommand {
        OrderCommand::New(NewOrder {
            client_order_id: ClientOrderId(id),
            account_id,
            instrument_id: 0,
            side,
            price,
            quantity,
            client_timestamp: id,
        })
    }

    #[test]
    fn buy_reserves_cash_deterministically() {
        let mut risk = RiskEngine::new();
        risk.create_account(1).unwrap();
        risk.fund(1, 100_000).unwrap();

        let reservation = risk
            .admit(&new_order(1, 7, OrderSide::Buy, 100, 500))
            .unwrap()
            .unwrap();

        assert_eq!(reservation.reserved_value, 50_000);
        assert_eq!(risk.account(1).unwrap().available_cash(), 50_000);
    }

    #[test]
    fn sell_requires_available_position() {
        let mut risk = RiskEngine::new();
        risk.create_account(1).unwrap();
        risk.credit_position(1, 0, 100).unwrap();

        assert_eq!(
            risk.admit(&new_order(1, 7, OrderSide::Sell, 100, 101)),
            Err(RiskError::InsufficientPosition)
        );
    }

    #[test]
    fn frozen_account_cannot_submit() {
        let mut risk = RiskEngine::new();
        risk.create_account(1).unwrap();
        risk.fund(1, 1000).unwrap();
        risk.set_status(1, AccountStatus::Frozen).unwrap();

        assert_eq!(
            risk.admit(&new_order(1, 1, OrderSide::Buy, 10, 1)),
            Err(RiskError::AccountInactive)
        );
    }

    #[test]
    fn cancel_releases_reservation() {
        let mut risk = RiskEngine::new();
        risk.create_account(1).unwrap();
        risk.fund(1, 1000).unwrap();
        risk.admit(&new_order(1, 1, OrderSide::Buy, 10, 10))
            .unwrap();

        risk.admit(&OrderCommand::Cancel(CancelOrder {
            account_id: 1,
            instrument_id: 0,
            client_order_id: ClientOrderId(1),
        }))
        .unwrap();

        assert_eq!(risk.account(1).unwrap().available_cash(), 1000);
        assert!(risk.reservation(1, 0, 1).is_none());
    }

    #[test]
    fn trade_moves_cash_and_position() {
        let mut risk = RiskEngine::new();
        risk.create_account(1).unwrap();
        risk.create_account(2).unwrap();
        risk.fund(1, 10_000).unwrap();
        risk.credit_position(2, 0, 100).unwrap();
        risk.admit(&new_order(1, 1, OrderSide::Buy, 120, 50))
            .unwrap();
        risk.admit(&new_order(2, 2, OrderSide::Sell, 100, 50))
            .unwrap();

        risk.apply_fill(RiskFill {
            buyer_account: 1,
            seller_account: 2,
            instrument_id: 0,
            buyer_client_order_id: 1,
            seller_client_order_id: 2,
            price: 100,
            quantity: 50,
        })
        .unwrap();

        assert_eq!(risk.account(1).unwrap().cash, 5000);
        assert_eq!(risk.account(1).unwrap().position(0), 50);
        assert_eq!(risk.account(2).unwrap().cash, 5000);
        assert_eq!(risk.account(2).unwrap().position(0), 50);
        assert!(risk.reservation(1, 0, 1).is_none());
        assert!(risk.reservation(2, 0, 2).is_none());
    }
}
