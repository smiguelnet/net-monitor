//! JSON representation of the monitor state for `GET /api/status`.

use crate::config::Config;
use crate::json::{self, Object};
use crate::model::{now_ms, Banner, Check, Node};
use crate::monitor::MonitorState;

pub fn status_json(state: &MonitorState, cfg: &Config) -> String {
    let base = Object::new()
        .bool("ready", state.snapshot.is_some())
        .num("server_time_ms", now_ms())
        .num("interval_s", cfg.interval_secs)
        .num("latency_warning_ms", cfg.latency_warning_ms)
        .bool("checking", state.checking)
        .num("cycle_started_ms", state.cycle_started_ms)
        .str("isp_name", &cfg.isp_name)
        .str("router_name", &cfg.router_name);

    let Some(snapshot) = &state.snapshot else {
        return base.build();
    };
    base.num("checked_at_ms", snapshot.checked_at_ms)
        .num("duration_ms", snapshot.duration_ms)
        .raw("banners", &json::array(snapshot.banners.iter().map(banner_json)))
        .raw("nodes", &json::array(snapshot.nodes.iter().map(node_json)))
        .raw("checks", &json::array(snapshot.checks.iter().map(check_json)))
        .build()
}

fn banner_json(b: &Banner) -> String {
    Object::new().str("level", b.level.as_str()).str("message", &b.message).build()
}

fn node_json(n: &Node) -> String {
    Object::new()
        .str("kind", n.kind.as_str())
        .str("name", &n.name)
        .str("target", &n.target)
        .str("status", n.status.as_str())
        .build()
}

fn check_json(c: &Check) -> String {
    Object::new()
        .str("id", &c.id)
        .str("group", c.group.as_str())
        .str("name", &c.name)
        .str("target", &c.target)
        .str("status", c.status.as_str())
        .opt_f64("latency_ms", c.latency_ms)
        .opt_num("loss_pct", c.loss_pct)
        .str("detail", &c.detail)
        .build()
}
