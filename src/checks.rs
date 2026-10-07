//! Turns probe measurements into `Check` results with a `Status`.

use std::net::SocketAddr;
use std::time::Duration;

use crate::config::Config;
use crate::model::{Check, Group, Status};
use crate::probe::{dns, ping, tcp};

const PING_TIMEOUT: Duration = Duration::from_secs(1);
const TCP_TIMEOUT: Duration = Duration::from_secs(2);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
/// DNS lookups may legitimately take longer than a round trip.
const DNS_LATENCY_FACTOR: f64 = 4.0;

/// Stable check identifiers, used by the diagnosis and the web UI.
pub mod ids {
    pub const DNS: &str = "dns";
    pub const HTTPS: &str = "https";
    pub const ROUTER_PING: &str = "router_ping";
    pub const ROUTER_WEB: &str = "router_web";

    pub fn internet(index: usize) -> String {
        format!("internet_{index}")
    }

    pub fn access_point(index: usize) -> String {
        format!("ap_{index}")
    }
}

pub fn ping_check(id: impl Into<String>, group: Group, name: &str, host: &str, cfg: &Config) -> Check {
    let mut check = Check::new(id, group, name, host);
    if host.is_empty() {
        check.status = Status::Unconfigured;
        check.detail = "No IP address configured in monitor.conf".into();
        return check;
    }
    match ping::ping(host, cfg.pings_per_check, PING_TIMEOUT) {
        Err(e) => check.detail = e,
        Ok(result) => {
            let loss = result.loss_pct();
            check.loss_pct = Some(loss);
            check.latency_ms = result.avg_ms;
            check.status = if result.received == 0 {
                Status::Down
            } else if loss > 0 || result.avg_ms.is_some_and(|ms| ms > cfg.latency_warning_ms) {
                Status::Degraded
            } else {
                Status::Ok
            };
            check.detail = if result.received == 0 {
                "No reply to ping".into()
            } else {
                format!("{}/{} ping replies", result.received, result.sent)
            };
        }
    }
    check
}

pub fn tcp_check(id: impl Into<String>, group: Group, name: &str, host: &str, port: u16, cfg: &Config) -> Check {
    let mut check = Check::new(id, group, name, format!("{host}:{port}"));
    match tcp::socket_addr(host, port) {
        Some(addr) => apply_tcp(&mut check, addr, cfg),
        None => check.detail = "Invalid address".into(),
    }
    check
}

/// Resolves the test host and, if that works, checks HTTPS on the resolved address.
pub fn dns_and_https_checks(cfg: &Config) -> Vec<Check> {
    let host = cfg.dns_test_host.as_str();
    let mut dns_check = Check::new(ids::DNS, Group::Internet, "DNS resolution", host);
    let mut https_check = Check::new(ids::HTTPS, Group::Internet, "HTTPS reachability", format!("{host}:443"));

    match dns::resolve(host, 443, DNS_TIMEOUT) {
        Ok(resolution) => {
            dns_check.latency_ms = Some(resolution.elapsed_ms);
            dns_check.status = latency_status(resolution.elapsed_ms, cfg.latency_warning_ms * DNS_LATENCY_FACTOR);
            if let Some(addr) = resolution.preferred() {
                dns_check.detail = format!("Resolved to {}", addr.ip());
                apply_tcp(&mut https_check, addr, cfg);
            }
        }
        Err(e) => {
            dns_check.detail = e;
            https_check.detail = "Not tested: DNS resolution failed".into();
        }
    }
    vec![dns_check, https_check]
}

fn apply_tcp(check: &mut Check, addr: SocketAddr, cfg: &Config) {
    match tcp::connect(addr, TCP_TIMEOUT) {
        Ok(ms) => {
            check.latency_ms = Some(ms);
            check.status = latency_status(ms, cfg.latency_warning_ms);
            check.detail = format!("TCP connection accepted by {addr}");
        }
        Err(e) => {
            check.status = Status::Down;
            check.detail = format!("TCP connection failed: {e}");
        }
    }
}

fn latency_status(ms: f64, warning_ms: f64) -> Status {
    if ms > warning_ms {
        Status::Degraded
    } else {
        Status::Ok
    }
}
