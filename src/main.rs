//! Network Monitor: live availability of the Internet connection (ISP),
//! the router/gateway and the Wi-Fi access points of a home network.
//!
//! Uses only the Rust standard library. The web UI is embedded in the
//! executable and served at http://localhost:<port>.

mod api;
mod checks;
mod config;
mod diagnosis;
mod http;
mod json;
mod model;
mod monitor;
mod platform;
mod probe;

use std::net::TcpListener;
use std::sync::Arc;

use config::Config;

fn main() {
    let cfg = Arc::new(Config::load());

    let listener = match TcpListener::bind(("0.0.0.0", cfg.port)) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Could not open port {}: {e}", cfg.port);
            eprintln!(
                "Is the monitor already running? Otherwise change \"port\" in {}.",
                config::FILE_NAME
            );
            platform::wait_for_enter();
            std::process::exit(1);
        }
    };

    let url = format!("http://localhost:{}", cfg.port);
    print_startup(&cfg, &url);

    let state = monitor::new_shared_state();
    monitor::spawn(cfg.clone(), state.clone());

    if cfg.open_browser {
        platform::open_browser(&url);
    }
    http::serve(listener, cfg, state);
}

fn print_startup(cfg: &Config, url: &str) {
    println!("Network Monitor started");
    println!("  Web interface : {url}");
    println!("  Configuration : {}", cfg.source);
    println!(
        "  Internet      : {} ({})",
        cfg.isp_name,
        cfg.internet_targets.join(", ")
    );
    println!("  Router        : {} ({})", cfg.router_name, cfg.router_ip);
    for ap in &cfg.access_points {
        let ip = if ap.ip.is_empty() {
            "(not configured)"
        } else {
            &ap.ip
        };
        println!("  Access point  : {} ({ip})", ap.name);
    }
    println!("  Interval      : {} s", cfg.interval_secs);
    println!("Close this window (or press Ctrl+C) to stop.\n");
}
