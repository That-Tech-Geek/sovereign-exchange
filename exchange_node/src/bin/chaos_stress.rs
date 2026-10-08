use sovereign_exchange_node::{Frame, MessageKind, PeerIngress, ProtocolError, FRAME_BYTES};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pct(&mut self) -> u8 { (self.next() % 100) as u8 }
}

fn frame(sender: u16, epoch: u64, sequence: u64) -> Frame {
    Frame {
        kind: MessageKind::Order,
        sender,
        epoch,
        sequence,
        instrument: sequence as u16 % 392,
        payload: [0; 24],
    }
}

fn run(name: &str, count: u64, loss: u8, dup: u8, reorder: bool, partition: bool) -> (u64, u64, u64, u64) {
    let mut rng = XorShift(0x5EED_2026);
    let mut ingress = PeerIngress::default();
    let mut wire = Vec::with_capacity(count as usize);
    let mut dropped = 0;
    let mut duplicated = 0;
    for seq in 1..=count {
        if partition && seq > count / 3 && seq <= count * 2 / 3 {
            dropped += 1;
            continue;
        }
        let f = frame(1, 1, seq);
        if rng.pct() < loss {
            dropped += 1;
            continue;
        }
        wire.push(f);
        if rng.pct() < dup {
            wire.push(f);
            duplicated += 1;
        }
    }
    if reorder {
        for i in (1..wire.len()).step_by(7) {
            wire.swap(i - 1, i);
        }
    }

    let mut accepted = 0;
    let mut gaps = 0;
    for f in wire {
        match ingress.observe(f) {
            Ok(()) => accepted += 1,
            Err(ProtocolError::SequenceGap) => gaps += 1,
            Err(ProtocolError::Duplicate) => {}
            Err(e) => panic!("unexpected protocol error: {e:?}"),
        }
    }
    println!("{name}: generated={count} accepted={accepted} dropped={dropped} duplicated={duplicated} gaps={gaps}");
    (count, accepted, dropped, gaps)
}

fn main() {
    let loads = [10_000u64, 100_000, 1_000_000, 5_000_000];
    let mut report = BTreeMap::new();

    for count in loads {
        report.insert(format!("clean_{count}"), run("clean", count, 0, 0, false, false));
        report.insert(format!("loss1_{count}"), run("loss1", count, 1, 0, false, false));
        report.insert(format!("loss5_{count}"), run("loss5", count, 5, 0, false, false));
        report.insert(format!("dup2_{count}"), run("dup2", count, 0, 2, false, false));
        report.insert(format!("reorder_{count}"), run("reorder", count, 0, 0, true, false));
        report.insert(format!("partition_{count}"), run("partition", count, 0, 0, false, true));
    }

    for (name, (generated, accepted, dropped, gaps)) in &report {
        if name.starts_with("clean_") && *accepted != *generated {
            panic!("clean run lost state: {name}");
        }
        if name.starts_with("dup2_") && *accepted != *generated {
            panic!("duplicate run changed canonical count: {name}");
        }
        if name.starts_with("loss") && *gaps == 0 {
            panic!("loss run failed to detect missing sequence: {name}");
        }
        if name.starts_with("reorder_") && *gaps == 0 {
            panic!("reorder run failed to detect sequence disorder: {name}");
        }
        if name.starts_with("partition_") && *gaps == 0 {
            panic!("partition run failed to fence the gap: {name}");
        }
        let _ = dropped;
    }

    let mut stale = PeerIngress::default();
    assert!(stale.observe(frame(2, 7, 1)).is_ok());
    assert_eq!(stale.observe(frame(2, 6, 2)), Err(ProtocolError::Duplicate));
    assert!(FRAME_BYTES <= 64);
    println!("CHAOS_RESULT=PASS");
}
