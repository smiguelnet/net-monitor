//! TCP connection probe (is a port accepting connections, and how fast).

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use super::elapsed_ms;

/// Returns the connection time in milliseconds.
pub fn connect(addr: SocketAddr, timeout: Duration) -> Result<f64, String> {
    let start = Instant::now();
    TcpStream::connect_timeout(&addr, timeout)
        .map(|_| elapsed_ms(start))
        .map_err(|e| e.to_string())
}

pub fn socket_addr(host: &str, port: u16) -> Option<SocketAddr> {
    (host, port).to_socket_addrs().ok()?.next()
}
