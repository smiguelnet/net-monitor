# Network Monitor

Live availability dashboard for a home network:

```text
Internet (ISP)  ⇄  Router / gateway  ⇄  Access point 1
                                     ⇄  Access point 2 …
```

- Web UI, no login, no history: it shows the **current** state only.
- Red banners whenever something is unavailable, with the most likely root cause.
- A single executable with no dependencies (Rust standard library only).

## Quick start (Windows)

1. Edit `monitor.conf`. At minimum, set `router_ip` and one `access_point` line per device.
2. Double-click `monitor.exe`. A console window opens, and your browser opens
   <http://localhost:8080>.
3. The first time it runs, Windows Firewall may ask for permission. Allow it on
   **private networks** if you want to open the dashboard from another device
   (`http://<this-pc-ip>:8080`).
4. To stop the monitor, close the console window.

Keep `monitor.conf` in the same folder as `monitor.exe`. You can also pass a
path to the config file as the first argument. Restart the monitor after you
change the config.

## Configuration (`monitor.conf`)

| Key | Meaning | Default |
|---|---|---|
| `port` | Web UI port | `8080` |
| `open_browser` | Open the browser on start (`yes`/`no`) | `yes` |
| `interval_seconds` | Time between check cycles | `10` |
| `pings_per_check` | Pings sent per host each cycle | `4` |
| `latency_warning_ms` | Average latency above this marks a check as DEGRADED | `80` |
| `isp_name` | Label for the Internet provider | `ISP` |
| `internet_targets` | Comma-separated hosts pinged to test the Internet | `1.1.1.1, 8.8.8.8` |
| `dns_test_host` | Name resolved to test DNS; also used for the HTTPS check | `google.com` |
| `router_name`, `router_ip` | Your router / gateway | `Router`, `192.168.0.1` |
| `router_web_port` | Router web-interface port (`0` disables this check) | `80` |
| `access_point` | `<ip>, <name>`; repeat this line for each access point | — |

To find an access point's IP, look at the router's client list, or run
`arp -a` on Windows and match the device's MAC address. If your router
supports it, reserve a fixed IP (DHCP reservation) for each access point.

## What is checked

| Group | Check | Method |
|---|---|---|
| Internet | Each `internet_targets` host | ping |
| Internet | DNS resolution | system resolver |
| Internet | HTTPS reachability | TCP connection to port 443 |
| Local network | Router | ping, plus a TCP connection to the web interface |
| Access points | Each `access_point` | ping |

Each check gets one of these statuses: **Operational**, **Degraded** (packet loss
or high latency), **Down**, or **Not configured**.

The diagnosis works outwards from this computer, so the banners point at the
root cause:

- If the router is down, the problem is the local network or this PC, and
  Internet errors are suppressed because they are only a consequence.
- If the router is up but every Internet target is down, it is an ISP / WAN
  problem.
- If the Internet is reachable by IP but DNS fails, it is a DNS problem.
- If an access point is down, it is powered off, unplugged or misconfigured.

**Limitations**

- Results include this computer's own link. Run the monitor on a machine
  **wired** to the router; otherwise a weak Wi-Fi link on the host looks like a
  network-wide problem.
- Ping cannot show whether a mesh node uses Ethernet or Wi-Fi backhaul. Check
  that in the vendor's app.

## Project layout

```text
src/
  main.rs        startup: load config, start the monitor loop and the web server
  config.rs      monitor.conf parsing
  model.rs       shared types (Status, Check, Banner, Node, Snapshot)
  probe/         low-level measurements: ping.rs, tcp.rs, dns.rs
  checks.rs      probe results → Check with a Status
  diagnosis.rs   root-cause analysis → banners and topology nodes
  monitor.rs     parallel check cycle + shared state
  api.rs         /api/status JSON
  json.rs        minimal JSON writer
  http.rs        tiny HTTP server (serves web/ and the API)
  platform.rs    OS helpers (hidden console, open browser)
web/
  index.html, style.css, app.js   dashboard UI, embedded into the exe at build time
```

## Building

```bash
cargo test                                         # unit tests
cargo build --release                              # native build

# Windows .exe cross-compiled from Linux/WSL
rustup target add x86_64-pc-windows-gnu            # once
# also install the MinGW-w64 linker (Arch: mingw-w64-gcc, Debian/Ubuntu: mingw-w64)
cargo build --release --target x86_64-pc-windows-gnu
```

Tip: if the project sits in a cloud-synced folder (Google Drive, OneDrive…),
set `CARGO_TARGET_DIR` to a folder outside it so build artifacts are not synced.
