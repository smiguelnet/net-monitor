//! Loading and parsing of `monitor.conf` (simple `key = value` format).

use std::path::PathBuf;
use std::str::FromStr;

pub const FILE_NAME: &str = "monitor.conf";

#[derive(Clone, Debug)]
pub struct AccessPoint {
    pub name: String,
    pub ip: String,
}

impl AccessPoint {
    /// Parses `<ip>, <display name>`. The IP may be empty (not configured yet).
    fn parse(value: &str) -> Option<AccessPoint> {
        let (ip, name) = value.split_once(',').unwrap_or((value, ""));
        let (ip, name) = (ip.trim(), name.trim());
        if ip.is_empty() && name.is_empty() {
            return None;
        }
        let name = if name.is_empty() { ip } else { name };
        Some(AccessPoint { name: name.into(), ip: ip.into() })
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub interval_secs: u64,
    pub pings_per_check: u32,
    pub latency_warning_ms: f64,
    pub open_browser: bool,
    pub isp_name: String,
    pub internet_targets: Vec<String>,
    pub dns_test_host: String,
    pub router_name: String,
    pub router_ip: String,
    /// TCP port of the router's web interface; 0 disables that check.
    pub router_web_port: u16,
    pub access_points: Vec<AccessPoint>,
    /// Where the configuration was loaded from (shown at startup).
    pub source: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            port: 8080,
            interval_secs: 10,
            pings_per_check: 4,
            latency_warning_ms: 80.0,
            open_browser: true,
            isp_name: "ISP".into(),
            internet_targets: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            dns_test_host: "google.com".into(),
            router_name: "Router".into(),
            router_ip: "192.168.0.1".into(),
            router_web_port: 80,
            access_points: Vec::new(),
            source: format!("built-in defaults ({FILE_NAME} not found)"),
        }
    }
}

impl Config {
    /// Looks for the config file in: first command-line argument, the
    /// executable's folder, then the current folder.
    pub fn load() -> Config {
        for path in candidate_paths() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let mut cfg = Config::parse(&text);
                cfg.source = path.display().to_string();
                return cfg;
            }
        }
        Config::default()
    }

    pub fn parse(text: &str) -> Config {
        let mut cfg = Config::default();
        for (key, value) in entries(text) {
            match key.as_str() {
                "port" => set(&mut cfg.port, &value),
                "interval_seconds" => set(&mut cfg.interval_secs, &value),
                "pings_per_check" => set(&mut cfg.pings_per_check, &value),
                "latency_warning_ms" => set(&mut cfg.latency_warning_ms, &value),
                "open_browser" => cfg.open_browser = parse_bool(&value),
                "isp_name" => cfg.isp_name = value,
                "internet_targets" => cfg.internet_targets = split_list(&value),
                "dns_test_host" => cfg.dns_test_host = value,
                "router_name" => cfg.router_name = value,
                "router_ip" => cfg.router_ip = value,
                "router_web_port" => set(&mut cfg.router_web_port, &value),
                "access_point" => cfg.access_points.extend(AccessPoint::parse(&value)),
                _ => eprintln!("Warning: unknown setting \"{key}\" in {FILE_NAME}"),
            }
        }
        cfg.interval_secs = cfg.interval_secs.max(2);
        cfg.pings_per_check = cfg.pings_per_check.clamp(1, 20);
        cfg
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(arg) = std::env::args().nth(1) {
        paths.push(PathBuf::from(arg));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) {
        paths.push(dir.join(FILE_NAME));
    }
    paths.push(PathBuf::from(FILE_NAME));
    paths
}

/// Yields `(key, value)` pairs, skipping blank lines and `#` comments.
/// Keys may repeat (e.g. one `access_point` line per device).
fn entries(text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
}

fn set<T: FromStr>(target: &mut T, value: &str) {
    match value.parse() {
        Ok(v) => *target = v,
        Err(_) => eprintln!("Warning: invalid value \"{value}\" in {FILE_NAME}; using the default"),
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "yes" | "y" | "true" | "1" | "on")
}

fn split_list(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings_and_repeated_access_points() {
        let cfg = Config::parse(
            "# comment\nport = 9000\ninternet_targets = 1.1.1.1, 9.9.9.9\n\
             access_point = 192.168.0.2, Deco #1 — Office\naccess_point = , Deco #2\n",
        );
        assert_eq!(cfg.port, 9000);
        assert_eq!(cfg.internet_targets, ["1.1.1.1", "9.9.9.9"]);
        assert_eq!(cfg.access_points.len(), 2);
        assert_eq!(cfg.access_points[0].name, "Deco #1 — Office");
        assert_eq!(cfg.access_points[0].ip, "192.168.0.2");
        assert_eq!(cfg.access_points[1].ip, "");
    }

    #[test]
    fn clamps_out_of_range_values() {
        let cfg = Config::parse("interval_seconds = 0\npings_per_check = 99");
        assert_eq!(cfg.interval_secs, 2);
        assert_eq!(cfg.pings_per_check, 20);
    }
}
