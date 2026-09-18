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
    engine.process_order(first.pool_index);
    engine.process_order(second.pool_index);

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