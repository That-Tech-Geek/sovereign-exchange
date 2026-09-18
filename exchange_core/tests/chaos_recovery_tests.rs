use exchange_core::{
    ClientOrderId, CommandJournal, NewOrder, OrderCommand, OrderSide, ReplayLedger, ReplaySnapshot,
    SnapshotMeta,
};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sovereign-exchange-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn sample_command(id: u64) -> OrderCommand {
    OrderCommand::New(NewOrder {
        client_order_id: ClientOrderId(id),
        account_id: 7,
        instrument_id: 0,
        side: OrderSide::Buy,
        price: 100,
        quantity: 3,
        client_timestamp: id,
    })
}

#[test]
fn journal_corruption_is_fail_closed() {
    let path = temp_path("journal-corruption");
    {
        let mut journal = CommandJournal::open(&path).unwrap();
        journal.append(&sample_command(1)).unwrap();
        journal.append(&sample_command(2)).unwrap();
    }

    let mut bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .open(&path)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes[16] ^= 0x80;
    let mut file = OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();

    assert!(CommandJournal::open(&path).is_err());
    let _ = fs::remove_file(path);
}

#[test]
fn journal_truncation_is_fail_closed() {
    let path = temp_path("journal-truncation");
    {
        let mut journal = CommandJournal::open(&path).unwrap();
        journal.append(&sample_command(1)).unwrap();
    }

    let metadata = fs::metadata(&path).unwrap();
    let file = OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(metadata.len() - 1).unwrap();

    assert!(CommandJournal::open(&path).is_err());
    let _ = fs::remove_file(path);
}

#[test]
fn snapshot_round_trip_and_boundary_validation() {
    let path = temp_path("snapshot");
    let ledger = ReplayLedger {
        event_count: 3,
        last_sequence: Some(3),
        accepted_orders: Default::default(),
        cancelled_orders: Default::default(),
        replaced_orders: 0,
        trades: Vec::new(),
    };

    let meta = ReplaySnapshot::write(&path, &ledger).unwrap();
    assert_eq!(
        meta,
        SnapshotMeta {
            journal_sequence: 3,
            event_count: 3
        }
    );
    let (read_meta, recovered) = ReplaySnapshot::read(&path).unwrap();
    assert_eq!(read_meta, meta);
    assert_eq!(recovered.event_count, 3);
    assert!(ReplaySnapshot::validate_journal_boundary(meta, 3).is_ok());
    assert!(ReplaySnapshot::validate_journal_boundary(meta, 2).is_err());

    let _ = fs::remove_file(path);
}
