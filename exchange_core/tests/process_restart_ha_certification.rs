use exchange_core::{
    ClientOrderId, CommandJournal, MatchingEngine, NewOrder, OrderCommand, OrderSide,
};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const PREFIX_LEN: u64 = 100;
const TRIALS: usize = 10;

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

fn child_path(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

fn wait_for(path: &Path) {
    for _ in 0..2000 {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("timed out waiting for {}", path.display());
}

fn run_writer(journal: &Path, ready: &Path) {
    let mut journal = CommandJournal::open(journal).unwrap();
    for id in 1..=PREFIX_LEN {
        journal.append(&command(id)).unwrap();
    }
    File::create(ready).unwrap().sync_all().unwrap();
    thread::sleep(Duration::from_secs(30));
}

fn run_recovery(journal: &Path, recovered: &Path) {
    let started = Instant::now();
    let mut journal = CommandJournal::open(journal).unwrap();
    let mut engine = MatchingEngine::new();
    let count = engine.recover_from_command_journal(&mut journal).unwrap();
    assert_eq!(count, PREFIX_LEN as usize);
    assert_eq!(engine.next_sequence_number_for_test(), PREFIX_LEN + 1);

    let next = command(PREFIX_LEN + 1);
    let accepted = engine.accept_durable(&next, &mut journal).unwrap();
    engine.process_command(accepted.pool_index);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(recovered).unwrap();
    writeln!(file, "recovered_commands={count}").unwrap();
    writeln!(file, "first_post_recovery_order=true").unwrap();
    writeln!(file, "recovery_and_first_accept_ms={elapsed_ms:.6}").unwrap();
    file.sync_all().unwrap();
}

fn spawn_writer(exe: &Path, journal: &Path, ready: &Path) -> Child {
    Command::new(exe)
        .env("SE_HA_CHILD", "writer")
        .env("SE_HA_JOURNAL", journal)
        .env("SE_HA_READY", ready)
        .spawn()
        .unwrap()
}

fn spawn_recovery(exe: &Path, journal: &Path, recovered: &Path) -> Child {
    Command::new(exe)
        .env("SE_HA_CHILD", "recovery")
        .env("SE_HA_JOURNAL", journal)
        .env("SE_HA_RECOVERED", recovered)
        .spawn()
        .unwrap()
}

#[test]
fn process_restart_recovery_rpo_rto() {
    if let Ok(role) = std::env::var("SE_HA_CHILD") {
        let journal = PathBuf::from(std::env::var("SE_HA_JOURNAL").unwrap());
        match role.as_str() {
            "writer" => run_writer(&journal, &PathBuf::from(std::env::var("SE_HA_READY").unwrap())),
            "recovery" => run_recovery(&journal, &PathBuf::from(std::env::var("SE_HA_RECOVERED").unwrap())),
            _ => panic!("unknown child role: {role}"),
        }
        return;
    }

    let root = std::env::temp_dir().join(format!(
        "sovereign-exchange-process-ha-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut rto_ms = Vec::with_capacity(TRIALS);

    for trial in 0..TRIALS {
        let journal = child_path(&root, &format!("journal-{trial}.bin"));
        let ready = child_path(&root, &format!("ready-{trial}"));
        let recovered = child_path(&root, &format!("recovered-{trial}"));

        let mut writer = spawn_writer(&exe, &journal, &ready);
        wait_for(&ready);
        assert!(journal.exists());
        writer.kill().unwrap();
        let _ = writer.wait();

        let restart_at = Instant::now();
        let mut recovery = spawn_recovery(&exe, &journal, &recovered);
        wait_for(&recovered);
        let status = recovery.wait().unwrap();
        assert!(status.success());
        let elapsed_ms = restart_at.elapsed().as_secs_f64() * 1000.0;
        rto_ms.push(elapsed_ms);

        let report = fs::read_to_string(&recovered).unwrap();
        assert!(report.contains("recovered_commands=100"));
        assert!(report.contains("first_post_recovery_order=true"));
    }

    rto_ms.sort_by(f64::total_cmp);
    let percentile = |p: f64| rto_ms[((rto_ms.len() - 1) as f64 * p).round() as usize];
    let min = *rto_ms.first().unwrap();
    let max = *rto_ms.last().unwrap();
    println!("PROCESS_HA_CERTIFICATION");
    println!("trials={TRIALS}");
    println!("durable_prefix_entries={PREFIX_LEN}");
    println!("rpo_entries=0");
    println!("process_restart_recovery_rto_ms_p50={:.3}", percentile(0.50));
    println!("process_restart_recovery_rto_ms_p95={:.3}", percentile(0.95));
    println!("process_restart_recovery_rto_ms_p99={:.3}", percentile(0.99));
    println!("process_restart_recovery_rto_ms_min={min:.3}");
    println!("process_restart_recovery_rto_ms_max={max:.3}");
    println!("durable_replay_verified=true");
    println!("first_post_recovery_order=true");
    println!("VERDICT=PASS");

    let _ = fs::remove_dir_all(root);
}
