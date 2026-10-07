//! DNS resolution through the operating system resolver, with a timeout.

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::elapsed_ms;

pub struct Resolution {
    pub addrs: Vec<SocketAddr>,
    pub elapsed_ms: f64,
}

impl Resolution {
    /// Prefers IPv4, which is what most home networks route reliably.
    pub fn preferred(&self) -> Option<SocketAddr> {
        self.addrs.iter().find(|a| a.is_ipv4()).or(self.addrs.first()).copied()
    }
}

pub fn resolve(host: &str, port: u16, timeout: Duration) -> Result<Resolution, String> {
    // The system resolver has no timeout parameter, so it runs on its own
    // thread and we stop waiting after `timeout`.
    let (tx, rx) = mpsc::channel();
    let target = format!("{host}:{port}");
    let start = Instant::now();
    thread::spawn(move || {
        let _ = tx.send(target.to_socket_addrs().map(|addrs| addrs.collect::<Vec<_>>()));
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(addrs)) if !addrs.is_empty() => Ok(Resolution { addrs, elapsed_ms: elapsed_ms(start) }),
        Ok(Ok(_)) => Err("No addresses returned".into()),
        Ok(Err(e)) => Err(format!("Lookup failed: {e}")),
        Err(_) => Err(format!("No answer within {} s", timeout.as_secs())),
    }
}
