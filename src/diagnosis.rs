//! Interprets check results from the client outwards — router first, then
//! access points, then the Internet and DNS — so banners point at the root
//! cause instead of listing every downstream failure.

use crate::checks::ids;
use crate::config::Config;
use crate::model::{Banner, Check, Level, Node, NodeKind, Status};

pub struct Diagnosis {
    pub banners: Vec<Banner>,
    pub nodes: Vec<Node>,
}

/// Status lookup by check id. Missing checks count as `Down`.
struct Results<'a>(&'a [Check]);

impl Results<'_> {
    fn status(&self, id: &str) -> Status {
        self.0.iter().find(|c| c.id == id).map(|c| c.status).unwrap_or(Status::Down)
    }

    fn is_down(&self, id: &str) -> bool {
        self.status(id) == Status::Down
    }

    /// `Ok` if every check is OK, otherwise `Degraded`.
    fn summary(&self, ids: &[String]) -> Status {
        if ids.iter().all(|id| self.status(id) == Status::Ok) {
            Status::Ok
        } else {
            Status::Degraded
        }
    }
}

#[derive(Default)]
struct Banners(Vec<Banner>);

impl Banners {
    fn push(&mut self, level: Level, message: impl Into<String>) {
        self.0.push(Banner { level, message: message.into() });
    }

    fn has_errors(&self) -> bool {
        self.0.iter().any(|b| b.level == Level::Error)
    }
}

pub fn diagnose(cfg: &Config, checks: &[Check]) -> Diagnosis {
    let results = Results(checks);
    let mut banners = Banners::default();

    let router_down = diagnose_router(cfg, &results, &mut banners);
    diagnose_access_points(cfg, &results, &mut banners, router_down);
    let internet_down = diagnose_internet(cfg, &results, &mut banners, router_down);

    let degraded: Vec<String> = checks
        .iter()
        .filter(|c| c.status == Status::Degraded)
        .map(|c| format!("{} ({})", c.name, c.target))
        .collect();
    if !degraded.is_empty() {
        banners.push(
            Level::Warning,
            format!(
                "Degraded performance (packet loss or latency above {} ms): {}",
                cfg.latency_warning_ms,
                degraded.join(", ")
            ),
        );
    }
    if !banners.has_errors() {
        let message = if degraded.is_empty() {
            "All monitored services are operational."
        } else {
            "All monitored services are operational, with degraded performance."
        };
        banners.0.insert(0, Banner { level: Level::Ok, message: message.into() });
    }

    Diagnosis {
        banners: banners.0,
        nodes: build_nodes(cfg, &results, router_down, internet_down),
    }
}

/// Returns whether the router is unreachable.
fn diagnose_router(cfg: &Config, results: &Results, banners: &mut Banners) -> bool {
    let web_enabled = cfg.router_web_port != 0;
    let ping_down = results.is_down(ids::ROUTER_PING);
    let web_down = web_enabled && results.is_down(ids::ROUTER_WEB);
    let router_down = ping_down && (web_down || !web_enabled);

    if router_down {
        banners.push(
            Level::Error,
            format!(
                "Cannot reach the router {} ({}). Check this computer's cable or Wi-Fi connection and whether \
                 the router is powered on. The Internet failures below are a consequence.",
                cfg.router_name, cfg.router_ip
            ),
        );
    } else if ping_down {
        banners.push(Level::Warning, "The router does not answer ping, but its web interface responds.");
    } else if web_down {
        banners.push(
            Level::Warning,
            format!("The router answers ping, but its web interface (port {}) does not respond.", cfg.router_web_port),
        );
    }
    router_down
}

fn diagnose_access_points(cfg: &Config, results: &Results, banners: &mut Banners, router_down: bool) {
    if cfg.access_points.is_empty() {
        banners.push(
            Level::Info,
            "No access points configured. Add \"access_point = <ip>, <name>\" lines to monitor.conf.",
        );
        return;
    }
    let mut configured = 0;
    let mut down = 0;
    for (i, ap) in cfg.access_points.iter().enumerate() {
        match results.status(&ids::access_point(i)) {
            Status::Unconfigured => banners.push(
                Level::Info,
                format!("Access point {}: no IP address configured in monitor.conf.", ap.name),
            ),
            Status::Down => {
                configured += 1;
                down += 1;
                if !router_down {
                    banners.push(
                        Level::Error,
                        format!("Access point {} ({}) is unreachable: no reply to ping.", ap.name, ap.ip),
                    );
                }
            }
            _ => configured += 1,
        }
    }
    if configured > 1 && down == configured && !router_down {
        banners.push(
            Level::Error,
            "All access points are unreachable: check their power, the cables to the router's LAN ports \
             and that they are in Access Point mode.",
        );
    }
}

/// Returns whether every Internet target is unreachable.
fn diagnose_internet(cfg: &Config, results: &Results, banners: &mut Banners, router_down: bool) -> bool {
    let target_ids: Vec<String> = (0..cfg.internet_targets.len()).map(ids::internet).collect();
    let internet_down = !target_ids.is_empty() && target_ids.iter().all(|id| results.is_down(id));
    if router_down {
        return internet_down;
    }

    if internet_down {
        banners.push(
            Level::Error,
            format!(
                "Internet ({}) is down: the router responds, but none of {} reply. Likely an ISP / WAN problem.",
                cfg.isp_name,
                cfg.internet_targets.join(", ")
            ),
        );
        return true;
    }
    for (target, id) in cfg.internet_targets.iter().zip(&target_ids) {
        if results.is_down(id) {
            banners.push(
                Level::Warning,
                format!("{target} does not reply, but the Internet is still reachable through other targets."),
            );
        }
    }
    if results.is_down(ids::DNS) {
        banners.push(
            Level::Error,
            format!(
                "DNS failure: the Internet is reachable by IP address, but {} could not be resolved.",
                cfg.dns_test_host
            ),
        );
    } else if results.is_down(ids::HTTPS) {
        banners.push(
            Level::Error,
            format!("HTTPS unreachable: {} resolves, but port 443 does not respond.", cfg.dns_test_host),
        );
    }
    false
}

fn build_nodes(cfg: &Config, results: &Results, router_down: bool, internet_down: bool) -> Vec<Node> {
    let mut internet_ids: Vec<String> = (0..cfg.internet_targets.len()).map(ids::internet).collect();
    internet_ids.extend([ids::DNS.to_string(), ids::HTTPS.to_string()]);
    let internet_status = if router_down || internet_down || results.is_down(ids::DNS) || results.is_down(ids::HTTPS) {
        Status::Down
    } else {
        results.summary(&internet_ids)
    };

    let mut router_ids = vec![ids::ROUTER_PING.to_string()];
    if cfg.router_web_port != 0 {
        router_ids.push(ids::ROUTER_WEB.to_string());
    }
    let router_status = if router_down { Status::Down } else { results.summary(&router_ids) };

    let mut nodes = vec![
        Node {
            kind: NodeKind::Internet,
            name: format!("Internet · {}", cfg.isp_name),
            target: cfg.internet_targets.join(" · "),
            status: internet_status,
        },
        Node {
            kind: NodeKind::Router,
            name: cfg.router_name.clone(),
            target: cfg.router_ip.clone(),
            status: router_status,
        },
    ];
    nodes.extend(cfg.access_points.iter().enumerate().map(|(i, ap)| Node {
        kind: NodeKind::AccessPoint,
        name: ap.name.clone(),
        target: if ap.ip.is_empty() { "No IP configured".into() } else { ap.ip.clone() },
        status: results.status(&ids::access_point(i)),
    }));
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Group;

    fn check(id: &str, status: Status) -> Check {
        let mut c = Check::new(id, Group::Internet, id, "target");
        c.status = status;
        c
    }

    fn cfg() -> Config {
        Config::parse("access_point = 192.168.0.2, AP 1\naccess_point = 192.168.0.3, AP 2")
    }

    fn all_ok() -> Vec<Check> {
        ["internet_0", "internet_1", "dns", "https", "router_ping", "router_web", "ap_0", "ap_1"]
            .iter()
            .map(|id| check(id, Status::Ok))
            .collect()
    }

    fn set(checks: &mut [Check], id: &str, status: Status) {
        checks.iter_mut().find(|c| c.id == id).unwrap().status = status;
    }

    fn errors(d: &Diagnosis) -> Vec<&str> {
        d.banners.iter().filter(|b| b.level == Level::Error).map(|b| b.message.as_str()).collect()
    }

    #[test]
    fn healthy_network_has_single_ok_banner() {
        let d = diagnose(&cfg(), &all_ok());
        assert_eq!(d.banners.len(), 1);
        assert_eq!(d.banners[0].level, Level::Ok);
        assert!(d.nodes.iter().all(|n| n.status == Status::Ok));
    }

    #[test]
    fn router_down_hides_downstream_internet_errors() {
        let mut checks = all_ok();
        for id in ["router_ping", "router_web", "internet_0", "internet_1", "dns", "https"] {
            set(&mut checks, id, Status::Down);
        }
        let d = diagnose(&cfg(), &checks);
        let errs = errors(&d);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].starts_with("Cannot reach the router"));
    }

    #[test]
    fn internet_down_with_router_up_blames_isp() {
        let mut checks = all_ok();
        for id in ["internet_0", "internet_1", "dns", "https"] {
            set(&mut checks, id, Status::Down);
        }
        let d = diagnose(&cfg(), &checks);
        assert!(errors(&d)[0].starts_with("Internet (ISP) is down"));
        assert_eq!(d.nodes[0].status, Status::Down);
        assert_eq!(d.nodes[1].status, Status::Ok);
    }

    #[test]
    fn dns_failure_is_reported_when_ip_connectivity_works() {
        let mut checks = all_ok();
        set(&mut checks, "dns", Status::Down);
        set(&mut checks, "https", Status::Down);
        assert!(errors(&diagnose(&cfg(), &checks))[0].starts_with("DNS failure"));
    }

    #[test]
    fn access_point_down_is_reported() {
        let mut checks = all_ok();
        set(&mut checks, "ap_1", Status::Down);
        let d = diagnose(&cfg(), &checks);
        assert_eq!(errors(&d), ["Access point AP 2 (192.168.0.3) is unreachable: no reply to ping."]);
    }
}
