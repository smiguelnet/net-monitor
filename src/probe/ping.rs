//! ICMP ping using the operating system's `ping` command (no admin rights needed).

use std::time::Duration;

use crate::platform;

#[derive(Debug, Clone, PartialEq)]
pub struct PingResult {
    pub sent: u32,
    pub received: u32,
    pub avg_ms: Option<f64>,
}

impl PingResult {
    pub fn loss_pct(&self) -> u32 {
        if self.sent == 0 {
            return 100;
        }
        100 * (self.sent - self.received) / self.sent
    }
}

pub fn ping(host: &str, count: u32, timeout: Duration) -> Result<PingResult, String> {
    let mut cmd = platform::hidden_command("ping");
    if cfg!(windows) {
        cmd.arg("-n").arg(count.to_string());
        cmd.arg("-w").arg(timeout.as_millis().to_string());
    } else if cfg!(target_os = "macos") {
        cmd.arg("-c").arg(count.to_string());
        cmd.arg("-W").arg(timeout.as_millis().to_string());
    } else {
        cmd.arg("-c").arg(count.to_string());
        cmd.arg("-W").arg(timeout.as_secs().max(1).to_string());
    }
    cmd.arg(host);

    let output = cmd.output().map_err(|e| format!("Could not run ping: {e}"))?;
    Ok(parse_output(&String::from_utf8_lossy(&output.stdout), count))
}

/// Counts replies by the "TTL=" marker, which appears in every successful
/// reply on Windows (any language), Linux and macOS, but not in
/// "destination unreachable" or timeout lines.
fn parse_output(text: &str, sent: u32) -> PingResult {
    let text = text.to_lowercase();
    let replies: Vec<&str> = text.lines().filter(|line| line.contains("ttl=")).collect();
    let times: Vec<f64> = replies.iter().filter_map(|line| parse_latency(line)).collect();
    let avg_ms = (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64);
    PingResult {
        sent,
        received: (replies.len() as u32).min(sent),
        avg_ms,
    }
}

/// Extracts the round-trip time from fragments such as "time=12ms",
/// "time<1ms", "time=4ms" or "time=10.2 ms".
fn parse_latency(line: &str) -> Option<f64> {
    let bytes = line.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'=' && b != b'<' {
            continue;
        }
        let start = i + 1;
        let end = start
            + bytes[start..]
                .iter()
                .take_while(|c| c.is_ascii_digit() || **c == b'.' || **c == b',')
                .count();
        if end > start && line[end..].trim_start().starts_with("ms") {
            if let Ok(ms) = line[start..end].replace(',', ".").parse() {
                return Some(ms);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_output_with_packet_loss_and_submillisecond_latency() {
        let out = "Pinging 192.168.0.1 with 32 bytes of data:\r\n\
                   Reply from 192.168.0.1: bytes=32 time=4ms TTL=64\r\n\
                   Reply from 192.168.0.1: bytes=32 time=6ms TTL=64\r\n\
                   Request timed out.\r\n\
                   Reply from 192.168.0.1: bytes=32 time<1ms TTL=64\r\n";
        let r = parse_output(out, 4);
        assert_eq!(r.received, 3);
        assert_eq!(r.loss_pct(), 25);
        assert!((r.avg_ms.unwrap() - 11.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn parses_windows_english_output() {
        let r = parse_output("Reply from 1.1.1.1: bytes=32 time=12ms TTL=57\r\n", 1);
        assert_eq!(r, PingResult { sent: 1, received: 1, avg_ms: Some(12.0) });
    }

    #[test]
    fn parses_linux_output() {
        let r = parse_output("64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=10.2 ms\n", 1);
        assert_eq!(r.avg_ms, Some(10.2));
    }

    #[test]
    fn unreachable_is_not_a_reply() {
        let r = parse_output("Reply from 192.168.0.88: Destination host unreachable.\r\n", 1);
        assert_eq!(r.received, 0);
        assert_eq!(r.loss_pct(), 100);
    }
}
