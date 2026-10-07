//! Data types shared by the checks, the diagnosis and the API.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    Degraded,
    Down,
    Unconfigured,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Degraded => "degraded",
            Status::Down => "down",
            Status::Unconfigured => "unconfigured",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Degraded => "DEGRADED",
            Status::Down => "DOWN",
            Status::Unconfigured => "NOT CONFIGURED",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Internet,
    LocalNetwork,
    AccessPoints,
}

impl Group {
    pub fn as_str(self) -> &'static str {
        match self {
            Group::Internet => "internet",
            Group::LocalNetwork => "local",
            Group::AccessPoints => "access_points",
        }
    }
}

/// Result of a single check (one card in the UI).
#[derive(Clone, Debug)]
pub struct Check {
    pub id: String,
    pub group: Group,
    pub name: String,
    pub target: String,
    pub status: Status,
    pub latency_ms: Option<f64>,
    pub loss_pct: Option<u32>,
    pub detail: String,
}

impl Check {
    /// Creates a check that is `Down` until a probe proves otherwise.
    pub fn new(id: impl Into<String>, group: Group, name: impl Into<String>, target: impl Into<String>) -> Check {
        Check {
            id: id.into(),
            group,
            name: name.into(),
            target: target.into(),
            status: Status::Down,
            latency_ms: None,
            loss_pct: None,
            detail: String::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Error,
    Warning,
    Info,
    Ok,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Info => "info",
            Level::Ok => "ok",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub level: Level,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Internet,
    Router,
    AccessPoint,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Internet => "internet",
            NodeKind::Router => "router",
            NodeKind::AccessPoint => "ap",
        }
    }
}

/// One element of the topology strip (Internet ⇄ Router ⇄ Access points).
#[derive(Clone, Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub name: String,
    pub target: String,
    pub status: Status,
}

/// Outcome of one full monitoring cycle.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub checked_at_ms: u128,
    pub duration_ms: u128,
    pub checks: Vec<Check>,
    pub banners: Vec<Banner>,
    pub nodes: Vec<Node>,
}

pub fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
