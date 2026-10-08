use exchange_core::{MatchingEngine, OrderPacket};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const NODES: usize = 3;
const ORDERS: usize = 300_000;
const FRAME: usize = 40;
const MAGIC: u32 = 0x5345_5831;

fn packet(id: u64, account: u32, instrument: u16, side: u8, price: u32) -> OrderPacket {
    OrderPacket {
        client_order_id: id,
        account_id: account,
        instrument_id: instrument,
        side,
        price,
        quantity: 1,
        timestamp: id,
        _pad: 0,
    }
}

fn encode(node: u16, p: OrderPacket) -> [u8; FRAME] {
    let mut out = [0u8; FRAME];
    out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    out[4] = 1;
    out[5..7].copy_from_slice(&node.to_le_bytes());
    out[8..40].copy_from_slice(&p.to_bytes());
    out
}

fn run_node(
    id: usize,
    socket: UdpSocket,
    done: Arc<AtomicBool>,
    accepted: Arc<AtomicU64>,
    trades: Arc<AtomicU64>,
) {
    thread::spawn(move || {
        let mut engine = MatchingEngine::new();
        let mut buf = [0u8; FRAME];
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap();
        while !done.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buf) {
                Ok((n, _))
                    if n == FRAME
                        && u32::from_le_bytes(buf[0..4].try_into().unwrap()) == MAGIC =>
                {
                    let p = OrderPacket::from_bytes((&buf[8..40]).try_into().unwrap());
                    if p.instrument_id as usize % NODES != id {
                        continue;
                    }
                    if let Ok(a) = engine.accept_order(&p) {
                        engine.process_order(a.pool_index);
                        accepted.fetch_add(1, Ordering::Relaxed);
                        trades.fetch_add(engine.trade_count as u64, Ordering::Relaxed);
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => panic!("node {id} UDP receive failed: {e}"),
                _ => {}
            }
        }
        assert_ne!(engine.state_fingerprint(), 0);
    });
}

fn main() {
    let sockets: Vec<UdpSocket> = (0..NODES)
        .map(|_| UdpSocket::bind("127.0.0.1:0").unwrap())
        .collect();
    let addrs: Vec<_> = sockets.iter().map(|s| s.local_addr().unwrap()).collect();
    let done = Arc::new(AtomicBool::new(false));
    let accepted = Arc::new(AtomicU64::new(0));
    let trades = Arc::new(AtomicU64::new(0));

    for (id, socket) in sockets.iter().enumerate() {
        run_node(
            id,
            socket.try_clone().unwrap(),
            Arc::clone(&done),
            Arc::clone(&accepted),
            Arc::clone(&trades),
        );
    }

    let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
    let start = Instant::now();
    for i in 0..ORDERS {
        let instrument = (i % 6) as u16;
        let side = if i % 2 == 0 { 1 } else { 0 };
        let owner = instrument as usize % NODES;
        tx.send_to(
            &encode(
                owner as u16,
                packet(
                    i as u64 + 1,
                    (i % 100_000) as u32 + 1,
                    instrument,
                    side,
                    10_000 + (i % 3) as u32,
                ),
            ),
            addrs[owner],
        )
        .unwrap();
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    while accepted.load(Ordering::Relaxed) < ORDERS as u64 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    done.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(50));

    let elapsed = start.elapsed().as_secs_f64();
    let got = accepted.load(Ordering::Relaxed);
    println!("peer_mesh_orders_sent={ORDERS}");
    println!("peer_mesh_orders_accepted={got}");
    println!("peer_mesh_trades={}", trades.load(Ordering::Relaxed));
    println!("peer_mesh_elapsed_s={elapsed:.6}");
    println!("peer_mesh_orders_per_sec={:.0}", got as f64 / elapsed);
    assert_eq!(got, ORDERS, "UDP mesh lost orders before deadline");
}
