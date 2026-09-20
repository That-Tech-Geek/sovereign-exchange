use exchange_core::{ClientOrderId, CommandJournal, MatchingEngine, NewOrder, OrderCommand, OrderSide};
use std::fs;
use std::time::Instant;

const SIZES: &[u64] = &[100, 1_000, 5_000, 10_000];
const RTO_BUDGET_MS: f64 = 1_000.0;

fn command(id: u64) -> OrderCommand {
    OrderCommand::New(NewOrder {
        client_order_id: ClientOrderId(id),
        account_id: 7,
        instrument_id: 0,
        side: OrderSide::Buy,
        price: 100,
        quantity: 1,
        client_timestamp: id,
    })
}

fn rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            if let Some(line) = status.lines().find(|line| line.starts_with("VmRSS:")) {
                return line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
    }
    0
}

#[test]
fn oracle_always_free_recovery_capacity() {
    let root = std::env::temp_dir().join(format!(
        "sovereign-exchange-oracle-recovery-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();

    println!("ORACLE_ALWAYS_FREE_RECOVERY");
    println!("target_rto_budget_ms={RTO_BUDGET_MS:.0}");

    for &size in SIZES {
        let path = root.join(format!("journal-{size}.bin"));
        let mut journal = CommandJournal::open(&path).unwrap();
        for id in 1..=size {
            journal.append(&command(id)).unwrap();
        }
        drop(journal);

        let bytes = fs::metadata(&path).unwrap().len();
        let replay_start = Instant::now();
        let mut journal = CommandJournal::open(&path).unwrap();
        let mut engine = MatchingEngine::new();
        let recovered = engine
            .recover_from_command_journal(&mut journal)
            .unwrap();
        let replay_ms = replay_start.elapsed().as_secs_f64() * 1000.0;

        assert_eq!(recovered, size as usize);
        println!(
            "size={size} journal_bytes={bytes} bytes_per_command={:.2} replay_ms={replay_ms:.3} replay_orders_s={:.0} rss_kb={}",
            bytes as f64 / size as f64,
            size as f64 / (replay_ms / 1000.0).max(f64::MIN_POSITIVE),
            rss_kb()
        );

        assert!(
            replay_ms < RTO_BUDGET_MS,
            "replaying {size} commands exceeded the 1s recovery budget: {replay_ms:.3} ms"
        );
    }

    println!("VERDICT=PASS");
    let _ = fs::remove_dir_all(root);
}
