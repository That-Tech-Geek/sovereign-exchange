use exchange_core::{
    CancelOrder, ClientOrderId, ExchangeEvent, MatchingEngine, NewOrder, OrderCommand,
    OrderSide, ReplaceOrder, SequenceNumber,
};

fn new_order(id: u64, account: u32, price: u32) -> OrderCommand {
    OrderCommand::New(NewOrder {
        client_order_id: ClientOrderId(id),
        account_id: account,
        instrument_id: 0,
        side: OrderSide::Buy,
        price,
        quantity: 10,
        client_timestamp: u64::MAX,
    })
}

#[test]
fn typed_new_command_gets_sequence_and_accept_event() {
    let mut engine = MatchingEngine::new();
    let accepted = engine.accept_command(&new_order(1, 10, 1000)).unwrap();
    assert_eq!(accepted.sequence_number, SequenceNumber::FIRST);
    engine.process_command(accepted.pool_index);
    assert!(engine.events().iter().any(|event| matches!(
        event,
        Some(ExchangeEvent::OrderAccepted { client_order_id, sequence_number, .. })
            if *client_order_id == ClientOrderId(1) && *sequence_number == SequenceNumber::FIRST
    )));
}

#[test]
fn cancel_is_typed_and_emits_cancelled_event() {
    let mut engine = MatchingEngine::new();
    let accepted = engine.accept_command(&new_order(7, 10, 1000)).unwrap();
    engine.process_command(accepted.pool_index);

    let cancel = OrderCommand::Cancel(CancelOrder {
        account_id: 10,
        instrument_id: 0,
        client_order_id: ClientOrderId(7),
    });
    let accepted_cancel = engine.accept_command(&cancel).unwrap();
    assert_eq!(accepted_cancel.exchange_order_id.0, 0);
    engine.process_command(accepted_cancel.pool_index);

    assert!(engine.events().iter().any(|event| matches!(
        event,
        Some(ExchangeEvent::OrderCancelled { client_order_id, .. })
            if *client_order_id == ClientOrderId(7)
    )));
}

#[test]
fn unknown_cancel_is_rejected_without_consuming_sequence() {
    let mut engine = MatchingEngine::new();
    let cancel = OrderCommand::Cancel(CancelOrder {
        account_id: 10,
        instrument_id: 0,
        client_order_id: ClientOrderId(999),
    });
    assert!(engine.accept_command(&cancel).is_err());

    let accepted = engine.accept_command(&new_order(1, 10, 1000)).unwrap();
    assert_eq!(accepted.sequence_number, SequenceNumber::FIRST);
}

#[test]
fn replace_gets_new_exchange_identity_and_new_sequence() {
    let mut engine = MatchingEngine::new();
    let first = engine.accept_command(&new_order(1, 10, 1000)).unwrap();
    engine.process_command(first.pool_index);

    let replace = OrderCommand::Replace(ReplaceOrder {
        account_id: 10,
        instrument_id: 0,
        target_client_order_id: ClientOrderId(1),
        new_client_order_id: ClientOrderId(2),
        side: OrderSide::Buy,
        price: 999,
        quantity: 20,
        client_timestamp: 1,
    });
    let replaced = engine.accept_command(&replace).unwrap();
    assert!(replaced.exchange_order_id.0 > first.exchange_order_id.0);
    assert!(replaced.sequence_number.0 > first.sequence_number.0);
    engine.process_command(replaced.pool_index);

    assert!(engine.book(0).unwrap().contains_order(10, ClientOrderId(2)));
    assert!(!engine.book(0).unwrap().contains_order(10, ClientOrderId(1)));
    assert!(engine.events().iter().any(|event| matches!(
        event,
        Some(ExchangeEvent::OrderReplaced { old_client_order_id, new_client_order_id, .. })
            if *old_client_order_id == ClientOrderId(1) && *new_client_order_id == ClientOrderId(2)
    )));
}

#[test]
fn replace_rejects_missing_target_without_consuming_sequence() {
    let mut engine = MatchingEngine::new();
    let replace = OrderCommand::Replace(ReplaceOrder {
        account_id: 10,
        instrument_id: 0,
        target_client_order_id: ClientOrderId(1),
        new_client_order_id: ClientOrderId(2),
        side: OrderSide::Buy,
        price: 999,
        quantity: 20,
        client_timestamp: 1,
    });
    assert!(engine.accept_command(&replace).is_err());
    let accepted = engine.accept_command(&new_order(1, 10, 1000)).unwrap();
    assert_eq!(accepted.sequence_number, SequenceNumber::FIRST);
}
