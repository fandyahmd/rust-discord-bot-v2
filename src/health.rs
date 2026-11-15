//! Minimal HTTP health server.
//!
//! PaaS web services (Render, Koyeb, Fly, Railway) require the container to bind a
//! port and answer requests, otherwise they mark the service as unhealthy and sleep
//! it. This is a ~40 line std/tokio server so no web framework is pulled in.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct Health {
    pub started: Instant,
    pub ready: Arc<AtomicBool>,
}

impl Health {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark the gateway + Lavalink handshake as done.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Relaxed);
    }

    fn body(&self) -> String {
        let ready = self.ready.load(Ordering::Relaxed);
        let status = if ready { "ok" } else { "starting" };
        let code = if ready { 200 } else { 503 };
        let json = payload(self, ready);
        format!(
            "HTTP/1.1 {code} {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        )
    }
}

fn payload(h: &Health, ready: bool) -> String {
    let status = if ready { "ok" } else { "starting" };
    format!(
        "{{\"status\":\"{status}\",\"uptime_seconds\":{}}}",
        h.started.elapsed().as_secs()
    )
}

/// Serve health checks forever. `port` comes from `PORT` (default 8080).
pub async fn serve(health: Health, port: u16) {
    let addr = format!("0.0.0.0:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[health] bind {addr} failed: {e}");
            return;
        }
    };
    println!("[health] listening on {addr}");

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[health] accept failed: {e}");
                continue;
            }
        };
        let health = health.clone();
        tokio::spawn(async move {
            // Drain the request head; the path does not matter for liveness probes.
            let mut buf = [0u8; 1024];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf))
                .await;
            let _ = stream.write_all(health.body().as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}
