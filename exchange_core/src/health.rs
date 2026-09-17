//! Lightweight, non-blocking HTTP health check server.
//!
//! Listens on `constants::HTTP_PORT` (port 8080 by default) on an isolated worker thread.
//! Exposes engine liveness, ring buffer depth, and real-time sampled latency telemetry
//! without contending or blocking the Core 0 matching hot path.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::metrics::PerformanceMetrics;
use crate::ring::RingBuffer;

/// Shared telemetry state exposed via the health check endpoint.
#[derive(Clone)]
pub struct HealthContext {
    pub is_running: Arc<AtomicBool>,
    pub metrics: Arc<PerformanceMetrics>,
    pub ring: RingBuffer,
    pub start_time: Instant,
    pub port: u16,
}

impl HealthContext {
    pub fn new(
        is_running: Arc<AtomicBool>,
        metrics: Arc<PerformanceMetrics>,
        ring: RingBuffer,
        port: u16,
    ) -> Self {
        Self {
            is_running,
            metrics,
            ring,
            start_time: Instant::now(),
            port,
        }
    }
}

/// Generates the status JSON payload for the health check.
pub fn generate_health_json(context: &HealthContext) -> String {
    let running = context.is_running.load(Ordering::Relaxed);
    let orders = context.metrics.orders_processed.load(Ordering::Relaxed);
    let trades = context.metrics.trades_generated.load(Ordering::Relaxed);
    let avg_latency = context.metrics.average_latency_micros();
    let min_latency = context.metrics.min_latency_nanos();
    let max_latency = context.metrics.max_latency_nanos();
    let ring_depth = context.ring.len();
    let uptime_secs = context.start_time.elapsed().as_secs();
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let status_str = if running { "HEALTHY" } else { "STOPPED" };

    format!(
        r#"{{"status":"{}","engine_running":{},"ring_buffer_depth":{},"orders_processed":{},"trades_generated":{},"avg_latency_micros":{:.4},"min_latency_nanos":{},"max_latency_nanos":{},"uptime_seconds":{},"timestamp_unix_ms":{},"http_port":{}}}"#,
        status_str,
        running,
        ring_depth,
        orders,
        trades,
        avg_latency,
        min_latency,
        max_latency,
        uptime_secs,
        timestamp_ms,
        context.port
    )
}

/// Handles a single incoming HTTP request on the health socket.
fn handle_client(mut stream: TcpStream, context: &HealthContext) {
    let mut buffer = [0u8; 1024];
    if stream.read(&mut buffer).is_err() {
        return;
    }

    let request = String::from_utf8_lossy(&buffer);

    // Support CORS preflight
    if request.starts_with("OPTIONS") {
        let response = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, OPTIONS\r\n\
            Access-Control-Allow-Headers: Content-Type\r\n\
            Connection: close\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        return;
    }

    let json_body = generate_health_json(context);
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: application/json; charset=utf-8\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, OPTIONS\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\r\n\
        {}",
        json_body.len(),
        json_body
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Spawns the HTTP health server in a dedicated background thread.
/// Returns the thread JoinHandle.
pub fn start_health_server(context: HealthContext, port: u16) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("health-http".to_string())
        .spawn(move || {
            let bind_addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&bind_addr) {
                Ok(l) => {
                    eprintln!(
                        "✅ Health check HTTP server listening on http://0.0.0.0:{}",
                        port
                    );
                    l
                }
                Err(e) => {
                    let alt_port = if port == 8080 { 8081 } else { port + 1 };
                    eprintln!(
                        "⚠️ Port {} unavailable ({}). Attempting fallback port {}",
                        port, e, alt_port
                    );
                    match TcpListener::bind(format!("0.0.0.0:{}", alt_port)) {
                        Ok(l) => {
                            eprintln!(
                                "✅ Health check HTTP server listening on http://0.0.0.0:{}",
                                alt_port
                            );
                            l
                        }
                        Err(e2) => {
                            eprintln!(
                                "❌ Failed to bind fallback health HTTP port {}: {}",
                                alt_port, e2
                            );
                            return;
                        }
                    }
                }
            };

            // Set read timeout so thread can accept without hanging indefinitely
            let _ = listener.set_nonblocking(false);

            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                        let _ = s.set_write_timeout(Some(Duration::from_millis(500)));
                        handle_client(s, &context);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        eprintln!("[Health] Error accepting connection: {}", e);
                    }
                }

                // Check if engine stopped
                if !context.is_running.load(Ordering::Relaxed) {
                    break;
                }
            }
        })
        .expect("Failed to spawn health check server thread")
}
