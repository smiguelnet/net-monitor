//! The monitoring loop: runs every check in parallel once per interval and
//! publishes the latest `Snapshot` to the shared state read by the web API.

use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::checks::{self, ids};
use crate::config::Config;
use crate::diagnosis::diagnose;
use crate::model::{now_ms, Check, Group, Snapshot, Status};
use crate::platform::utc_clock;

#[derive(Default)]
pub struct MonitorState {
    pub snapshot: Option<Snapshot>,
    /// True while a cycle is running (the UI shows a progress animation).
    pub checking: bool,
    pub cycle_started_ms: u128,
}

pub type SharedState = Arc<Mutex<MonitorState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(MonitorState::default()))
}

/// Locks the state, recovering it if another thread panicked while holding it.
pub fn lock(state: &SharedState) -> MutexGuard<'_, MonitorState> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn spawn(cfg: Arc<Config>, state: SharedState) {
    thread::spawn(move || {
        let mut last_summary = String::new();
        loop {
            {
                let mut s = lock(&state);
                s.checking = true;
                s.cycle_started_ms = now_ms();
            }
            let snapshot = run_cycle(&cfg);
            log_if_changed(&snapshot, &mut last_summary);
            {
                let mut s = lock(&state);
                s.snapshot = Some(snapshot);
                s.checking = false;
            }
            thread::sleep(Duration::from_secs(cfg.interval_secs));
        }
    });
}

type Task = Box<dyn FnOnce(&Config) -> Vec<Check> + Send>;

fn build_tasks(cfg: &Config) -> Vec<Task> {
    let mut tasks: Vec<Task> = Vec::new();
    for (i, target) in cfg.internet_targets.iter().cloned().enumerate() {
        tasks.push(Box::new(move |c: &Config| {
            vec![checks::ping_check(ids::internet(i), Group::Internet, "Internet ping", &target, c)]
        }));
    }
    tasks.push(Box::new(checks::dns_and_https_checks));
    tasks.push(Box::new(|c: &Config| {
        vec![checks::ping_check(ids::ROUTER_PING, Group::LocalNetwork, "Router ping", &c.router_ip, c)]
    }));
    if cfg.router_web_port != 0 {
        tasks.push(Box::new(|c: &Config| {
            let name = "Router web interface";
            vec![checks::tcp_check(ids::ROUTER_WEB, Group::LocalNetwork, name, &c.router_ip, c.router_web_port, c)]
        }));
    }
    for (i, ap) in cfg.access_points.iter().cloned().enumerate() {
        tasks.push(Box::new(move |c: &Config| {
            vec![checks::ping_check(ids::access_point(i), Group::AccessPoints, &ap.name, &ap.ip, c)]
        }));
    }
    tasks
}

pub fn run_cycle(cfg: &Arc<Config>) -> Snapshot {
    let start = Instant::now();
    let handles: Vec<_> = build_tasks(cfg)
        .into_iter()
        .map(|task| {
            let cfg = Arc::clone(cfg);
            thread::spawn(move || task(&cfg))
        })
        .collect();
    let checks: Vec<Check> = handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect();

    let diagnosis = diagnose(cfg, &checks);
    Snapshot {
        checked_at_ms: now_ms(),
        duration_ms: start.elapsed().as_millis(),
        checks,
        banners: diagnosis.banners,
        nodes: diagnosis.nodes,
    }
}

/// Prints the topology summary, plus the reason for every unhealthy check,
/// whenever the overall picture changes.
fn log_if_changed(snapshot: &Snapshot, last_summary: &mut String) {
    let summary = snapshot
        .nodes
        .iter()
        .map(|n| format!("{}: {}", n.name, n.status.label()))
        .collect::<Vec<_>>()
        .join(" | ");
    if summary == *last_summary {
        return;
    }
    println!("[{} UTC] {summary}", utc_clock(snapshot.checked_at_ms));
    for c in snapshot.checks.iter().filter(|c| matches!(c.status, Status::Degraded | Status::Down)) {
        let latency = c.latency_ms.map(|ms| format!(", {ms:.0} ms")).unwrap_or_default();
        let loss = c.loss_pct.map(|p| format!(", {p}% loss")).unwrap_or_default();
        println!("    - {} ({}): {} — {}{latency}{loss}", c.name, c.target, c.status.label(), c.detail);
    }
    *last_summary = summary;
}
