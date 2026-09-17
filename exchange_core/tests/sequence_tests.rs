use exchange_core::engine::{MatchingEngine, OrderAcceptError};
use exchange_core::instrument::spot_instrument_id;
use exchange_core::order::{ClientOrderId, OrderPacket};
use exchange_core::SequenceNumber;

fn packet(
    client_order_id: u64,
    account_id: u32,
    instrument_id: u16,
    side: u8,
    price: u32,
    quantity: u32,
    timestamp: u64,
) -> OrderPacket {
    OrderPacket {
        client_order_id,
        account_id,
        instrument_id,
        side,
        price,
        quantity,
        timestamp,
        _pad: 0,
    }
}

#[test]
fn accepted_commands_receive_monotonic_canonical_sequences() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let first = engine
        .accept_order(&packet(1, 10, instrument, 1, 10000, 10, 9999))
        .unwrap();
    let second = engine
        .accept_order(&packet(2, 11, instrument, 1, 10000, 10, 1))
        .unwrap();
    let cancel = engine
        .accept_order(&packet(1, 10, instrument, 2, 0, 0, 0))
        .unwrap();

    assert_eq!(first.sequence_number, SequenceNumber(1));
    assert_eq!(second.sequence_number, SequenceNumber(2));
    assert_eq!(cancel.sequence_number, SequenceNumber(3));
}

#[test]
fn rejected_commands_do_not_consume_sequence_numbers() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let invalid = engine.accept_order(&packet(1, 10, 392, 1, 10000, 10, 1));
    assert_eq!(invalid, Err(OrderAcceptError::InvalidInstrument(392)));

    let invalid_side = engine.accept_order(&packet(2, 10, instrument, 3, 10000, 10, 2));
    assert_eq!(invalid_side, Err(OrderAcceptError::InvalidSide(3)));

    let accepted = engine
        .accept_order(&packet(3, 10, instrument, 1, 10000, 10, 3))
        .unwrap();
    assert_eq!(accepted.sequence_number, SequenceNumber(1));
}

#[test]
fn duplicate_rejection_does_not_consume_sequence_or_exchange_id() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let first = engine
        .accept_order(&packet(7, 42, instrument, 1, 10000, 10, 100))
        .unwrap();
    engine.process_order(first.pool_index);

    let duplicate = engine.accept_order(&packet(7, 42, instrument, 0, 10000, 5, 1));
    assert!(matches!(duplicate, Err(OrderAcceptError::DuplicateClientOrderId { .. })));

    let next = engine
        .accept_order(&packet(8, 42, instrument, 1, 10000, 10, 0))
        .unwrap();
    assert_eq!(next.exchange_order_id.0, 2);
    assert_eq!(next.sequence_number, SequenceNumber(2));
}

#[test]
fn client_timestamps_never_change_fifo_priority() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(1);

    // The first order has a much later client timestamp. Canonical admission
    // sequence still makes it the first resting order at the price level.
    let first = engine
        .accept_order(&packet(100, 1, instrument, 1, 10000, 10, u64::MAX))
        .unwrap();
    engine.process_order(first.pool_index);

    let second = engine
        .accept_order(&packet(101, 2, instrument, 1, 10000, 10, 0))
        .unwrap();
    engine.process_order(second.pool_index);

    let buy = engine
        .accept_order(&packet(200, 3, instrument, 0, 10000, 15, 1))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 2);

    assert_eq!(engine.trades[0].seller, 1);
    assert_eq!(engine.trades[0].seller_sequence_number, first.sequence_number.0);
    assert_eq!(engine.trades[1].seller, 2);
    assert_eq!(engine.trades[1].seller_sequence_number, second.sequence_number.0);
}

#[test]
fn sequence_is_global_across_instruments() {
    let mut engine = MatchingEngine::new();
    let first = engine
        .accept_order(&packet(1, 1, spot_instrument_id(0), 1, 10000, 1, 0))
        .unwrap();
    let second = engine
        .accept_order(&packet(2, 2, spot_instrument_id(1), 1, 10000, 1, 0))
        .unwrap();

    assert_eq!(first.sequence_number, SequenceNumber(1));
    assert_eq!(second.sequence_number, SequenceNumber(2));
}

#[test]
fn trade_carries_canonical_sequences_for_both_orders() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(2);

    let resting = engine
        .accept_order(&packet(10, 100, instrument, 1, 10000, 5, 999))
        .unwrap();
    engine.process_order(resting.pool_index);

    let aggressor = engine
        .accept_order(&packet(11, 200, instrument, 0, 10000, 5, 1))
        .unwrap();
    engine.process_order(aggressor.pool_index);

    let trade = engine.trades[0];
    assert_eq!(trade.seller_sequence_number, resting.sequence_number.0);
    assert_eq!(trade.buyer_sequence_number, aggressor.sequence_number.0);
    assert!(trade.seller_sequence_number < trade.buyer_sequence_number);
}

#[test]
fn sequence_type_is_orderable_and_transparent() {
    assert!(SequenceNumber(10) > SequenceNumber(9));
    assert_eq!(SequenceNumber(7).get(), 7);
    assert_eq!(SequenceNumber::FIRST, SequenceNumber(1));
    assert_eq!(ClientOrderId(1).0, 1);
}
