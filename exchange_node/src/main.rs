use exchange_core::{MatchingEngine, OrderPacket};
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const FRAME_BYTES: usize = 40;
const MAGIC: u32 = 0x5345_5831;
const KIND_ORDER: u8 = 1;
const KIND_ACK: u8 = 2;

#[derive(Clone, Copy)]
struct Peer {
    node_id: u16,
    addr: SocketAddr,
}

fn frame(node_id: u16, kind: u8, packet: OrderPacket) -> [u8; FRAME_BYTES] {
    let mut out = [0u8; FRAME_BYTES];
    out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    out[4] = kind;
    out[5..7].copy_from_slice(&node_id.to_le_bytes());
    out[8..40].copy_from_slice(&packet.to_bytes());
    out
}

fn decode(buf: &[u8]) -> Option<(u16, u8, OrderPacket)> {
    if buf.len() != FRAME_BYTES || u32::from_le_bytes(buf[0..4].try_into().ok()?) != MAGIC {
        return None;
    }
    Some((
        u16::from_le_bytes(buf[5..7].try_into().ok()?),
        buf[4],
        OrderPacket::from_bytes(buf[8..40].try_into().ok()?),
    ))
}

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let node_id: u16 = args
        .next()
        .unwrap_or_else(|| "0".into())
        .parse()
        .expect("node id");
    let bind = args.next().unwrap_or_else(|| "0.0.0.0:7000".into());
    let peers_raw = args.next().unwrap_or_else(|| "0=127.0.0.1:7000".into());
    let peers: Vec<Peer> = peers_raw
        .split(',')
        .map(|entry| {
            let (id, addr) = entry.split_once('=').expect("peer must be id=addr");
            Peer {
                node_id: id.parse().expect("peer id"),
                addr: addr.parse().expect("peer addr"),
            }
        })
        .collect();
    assert!(!peers.is_empty());
    let peer_count = peers.len();

    let socket = UdpSocket::bind(&bind)?;
    socket.set_nonblocking(true)?;
    let running = Arc::new(AtomicBool::new(true));
    let accepted = Arc::new(AtomicU64::new(0));
    let forwarded = Arc::new(AtomicU64::new(0));
    let trades = Arc::new(AtomicU64::new(0));

    let r = Arc::clone(&running);
    let a = Arc::clone(&accepted);
    let f = Arc::clone(&forwarded);
    let t = Arc::clone(&trades);
    let rx = socket.try_clone()?;

    thread::spawn(move || {
        let mut engine = MatchingEngine::new();
        let mut buf = [0u8; FRAME_BYTES];
        while r.load(Ordering::Relaxed) {
            match rx.recv_from(&mut buf) {
                Ok((n, src)) => {
                    let Some((origin, kind, packet)) = decode(&buf[..n]) else {
                        continue;
                    };
                    if kind != KIND_ORDER {
                        continue;
                    }
                    let owner = packet.instrument_id as usize % peer_count;
                    let Some(owner_peer) = peers.get(owner) else {
                        continue;
                    };

                    if owner_peer.node_id != node_id {
                        let _ = rx.send_to(&buf, owner_peer.addr);
                        f.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    if let Ok(order) = engine.accept_order(&packet) {
                        engine.process_order(order.pool_index);
                        a.fetch_add(1, Ordering::Relaxed);
                        t.fetch_add(engine.trade_count as u64, Ordering::Relaxed);
                        let ack = frame(node_id, KIND_ACK, packet);
                        if let Some(origin_peer) = peers.iter().find(|p| p.node_id == origin) {
                            let _ = rx.send_to(&ack, origin_peer.addr);
                        } else {
                            let _ = rx.send_to(&ack, src);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::yield_now();
                }
                Err(_) => break,
            }
        }
    });

    eprintln!("sovereign-node id={node_id} bind={bind} peers={peer_count}");
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(5));
        eprintln!(
            "node={} accepted={} forwarded={} trades={}",
            node_id,
            accepted.load(Ordering::Relaxed),
            forwarded.load(Ordering::Relaxed),
            trades.load(Ordering::Relaxed)
        );
    }
    Ok(())
}
