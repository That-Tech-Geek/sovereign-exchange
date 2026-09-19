use crate::command::{NewOrder, OrderCommand, OrderSide};
use crate::consensus::{NodeId, RaftNode, Term, VoteResponse};
use crate::constants::MAX_INSTRUMENTS;
use crate::instrument::{future_instrument_id, spot_instrument_id, SOVEREIGNS};
use crate::engine::MatchingEngine;
use crate::ledger::{
    AssetId, DoubleEntryLedger, FeeSchedule, LedgerKind, SettlementEngine, TradeSettlement,
    Transfer,
};
use crate::order::ClientOrderId;
use crate::protocol::{Credential, CredentialStore, GatewaySession, Role, WireFrame, WireMessage};
use crate::session::MessageSeq;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificationReport {
    pub deterministic_replay: bool,
    pub ledger_integrity: bool,
    pub consensus_fencing: bool,
    pub protocol_integrity: bool,
}

impl CertificationReport {
    pub const fn all_passed(self) -> bool {
        self.deterministic_replay
            && self.ledger_integrity
            && self.consensus_fencing
            && self.protocol_integrity
    }
}

pub fn run_smoke_certification() -> CertificationReport {
    CertificationReport {
        deterministic_replay: deterministic_replay_check() && topology_check(),
        ledger_integrity: ledger_integrity_check(),
        consensus_fencing: consensus_fencing_check(),
        protocol_integrity: protocol_integrity_check(),
    }
}

fn command_stream() -> Vec<OrderCommand> {
    let registered_instruments: Vec<u16> = SOVEREIGNS
        .iter()
        .flat_map(|sovereign| {
            [
                spot_instrument_id(sovereign.id),
                future_instrument_id(sovereign.id),
            ]
        })
        .collect();

    (1..=10_000)
        .map(|id| {
            OrderCommand::New(NewOrder {
                client_order_id: ClientOrderId(id),
                account_id: 1 + (id % 32) as u32,
                instrument_id: registered_instruments
                    [(id as usize - 1) % registered_instruments.len()],
                side: OrderSide::Buy,
                price: 100 + (id % 50) as u32,
                quantity: 1 + (id % 7) as u32,
                client_timestamp: id,
            })
        })
        .collect()
}

fn topology_check() -> bool {
    let engine = MatchingEngine::new();
    let expected_registered = SOVEREIGNS.len() * 2;

    if engine.books.len() != MAX_INSTRUMENTS
        || engine.instruments.len() != expected_registered
        || expected_registered > MAX_INSTRUMENTS
    {
        return false;
    }

    // Capacity and registration are deliberately separate contracts:
    // all 392 books exist, while only the currently defined sovereigns are
    // registered for admission.
    for id in 0..MAX_INSTRUMENTS as u16 {
        if engine.book(id).is_none() {
            return false;
        }
    }

    for sovereign in SOVEREIGNS {
        let spot = spot_instrument_id(sovereign.id);
        let future = future_instrument_id(sovereign.id);
        if !engine
            .instrument(spot)
            .is_some_and(|i| i.market_type == crate::instrument::MarketType::Spot)
            || !engine
                .instrument(future)
                .is_some_and(|i| i.market_type == crate::instrument::MarketType::Future)
        {
            return false;
        }
    }

    // An unregistered capacity slot must not alias an adjacent registered
    // instrument or become implicitly tradable.
    let first_unregistered = (expected_registered) as u16;
    engine.instrument(first_unregistered).is_none() && engine.book(first_unregistered).is_some()
}

fn deterministic_replay_check() -> bool {
    let commands = command_stream();
    let mut first = MatchingEngine::new();
    let mut second = MatchingEngine::new();

    for command in commands {
        let left = first.accept_command(&command);
        let right = second.accept_command(&command);
        if left.is_err() || right.is_err() {
            return false;
        }
        let left = left.unwrap_or_else(|_| unreachable!());
        let right = right.unwrap_or_else(|_| unreachable!());
        first.process_order(left.pool_index);
        second.process_order(right.pool_index);
    }

    first.state_fingerprint() == second.state_fingerprint()
}

fn ledger_integrity_check() -> bool {
    let cash = AssetId(0);
    let stock = AssetId(1000);
    let mut ledger = DoubleEntryLedger::new();
    if ledger.seed(1, cash, 100_000).is_err() || ledger.seed(2, stock, 1_000).is_err() {
        return false;
    }

    let before_cash = ledger.asset_total(cash);
    let before_stock = ledger.asset_total(stock);
    let transfers = [
        Transfer {
            trade_id: 1,
            sequence: 1,
            debit_account: 1,
            credit_account: 2,
            asset: cash,
            amount: 10_000,
            kind: LedgerKind::TradeCash,
        },
        Transfer {
            trade_id: 1,
            sequence: 1,
            debit_account: 2,
            credit_account: 1,
            asset: stock,
            amount: 100,
            kind: LedgerKind::TradeSecurity,
        },
    ];

    if ledger.post_batch(&transfers).is_err() {
        return false;
    }

    ledger.asset_total(cash) == before_cash && ledger.asset_total(stock) == before_stock
}

fn consensus_fencing_check() -> bool {
    let members = vec![NodeId(1), NodeId(2), NodeId(3)];
    let mut leader = match RaftNode::new(NodeId(1), members, 3, 1) {
        Ok(node) => node,
        Err(_) => return false,
    };
    leader.start_election();
    if leader
        .handle_vote_response(VoteResponse {
            term: Term(1),
            voter_id: NodeId(2),
            granted: true,
        })
        .is_err()
    {
        return false;
    }
    leader.term() == Term(1) && leader.role() == crate::consensus::Role::Leader
}

fn protocol_integrity_check() -> bool {
    let mut credentials = CredentialStore::new();
    credentials.register(Credential {
        api_key: 1,
        token: [7; 32],
        role: Role::Trader,
        enabled: true,
    });

    let mut session = GatewaySession::new();
    let frame = WireFrame {
        sequence: MessageSeq(1),
        message: WireMessage::Logon {
            api_key: 1,
            token: [7; 32],
            heartbeat_ticks: 100,
        },
    };

    if session.receive(frame, &credentials).is_err() {
        return false;
    }

    let mut frame = WireFrame {
        sequence: MessageSeq(2),
        message: WireMessage::Heartbeat,
    };
    let mut bytes = match frame.encode() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    bytes[8] ^= 1;
    if WireFrame::decode(&bytes).is_ok() {
        return false;
    }

    frame.sequence = MessageSeq(2);
    WireFrame::decode(&frame.encode().unwrap()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certification_smoke_passes() {
        assert!(run_smoke_certification().all_passed());
    }

    #[test]
    fn deterministic_workload_replays_identically() {
        assert!(deterministic_replay_check());
    }

    #[test]
    fn ledger_integrity_is_zero_sum_per_asset() {
        assert!(ledger_integrity_check());
    }

    #[test]
    fn consensus_election_is_fenced_by_term() {
        assert!(consensus_fencing_check());
    }

    #[test]
    fn protocol_header_mutation_is_rejected() {
        assert!(protocol_integrity_check());
    }

    #[test]
    fn settlement_rejects_partial_state() {
        let cash = AssetId(0);
        let stock = AssetId(1000);
        let mut ledger = DoubleEntryLedger::new();
        ledger.seed(1, cash, 100).unwrap();
        ledger.seed(2, stock, 10).unwrap();

        let mut settlement = SettlementEngine::new(ledger);
        let result = settlement.settle_trade(
            TradeSettlement {
                trade_id: 42,
                sequence: 42,
                buyer_account: 1,
                seller_account: 2,
                instrument_asset: stock,
                cash_asset: cash,
                price: 100,
                quantity: 10,
                buyer_fee: 0,
                seller_fee: 0,
            },
            FeeSchedule::new(0, 0),
            999,
        );

        assert!(result.is_err());
        assert_eq!(settlement.ledger.balance(1, cash), 100);
        assert_eq!(settlement.ledger.balance(2, stock), 10);
    }
}
