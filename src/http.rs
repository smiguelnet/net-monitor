//! Tiny HTTP/1.1 server: serves the embedded web UI and the status API.

use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::api;
use crate::config::Config;
use crate::monitor::{self, SharedState};

// The UI lives in `web/` for easy editing and is compiled into the executable.
const INDEX_HTML: &str = include_str!("../web/index.html");
const STYLE_CSS: &str = include_str!("../web/style.css");
const APP_JS: &str = include_str!("../web/app.js");

const MAX_REQUEST_BYTES: usize = 16 * 1024;

struct Response {
    status: &'static str,
    content_type: &'static str,
    body: Cow<'static, str>,
}

impl Response {
    fn ok(content_type: &'static str, body: impl Into<Cow<'static, str>>) -> Response {
        Response { status: "200 OK", content_type, body: body.into() }
    }

    fn not_found() -> Response {
        Response { status: "404 Not Found", content_type: "text/plain; charset=utf-8", body: "Not found".into() }
    }
}

pub fn serve(listener: TcpListener, cfg: Arc<Config>, state: SharedState) {
    for stream in listener.incoming().flatten() {
        let cfg = Arc::clone(&cfg);
        let state = Arc::clone(&state);
        thread::spawn(move || handle(stream, &cfg, &state));
    }
}

fn handle(mut stream: TcpStream, cfg: &Config, state: &SharedState) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some((method, path)) = read_request_line(&mut stream) else { return };
    let response = route(&method, &path, cfg, state);
    write_response(&mut stream, &response);
}

fn route(method: &str, path: &str, cfg: &Config, state: &SharedState) -> Response {
    if method != "GET" {
        return Response::not_found();
    }
    match path {
        "/" | "/index.html" => Response::ok("text/html; charset=utf-8", INDEX_HTML),
        "/style.css" => Response::ok("text/css; charset=utf-8", STYLE_CSS),
        "/app.js" => Response::ok("text/javascript; charset=utf-8", APP_JS),
        "/api/status" => {
            let json = api::status_json(&monitor::lock(state), cfg);
            Response::ok("application/json; charset=utf-8", json)
        }
        _ => Response::not_found(),
    }
}

/// Reads the request headers and returns `(method, path)` without the query string.
fn read_request_line(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut request = Vec::new();
    let mut buf = [0u8; 4096];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < MAX_REQUEST_BYTES {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => request.extend_from_slice(&buf[..n]),
        }
    }
    let text = String::from_utf8_lossy(&request);
    let mut parts = text.lines().next()?.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.split('?').next()?.to_string();
    Some((method, path))
}

fn write_response(stream: &mut TcpStream, response: &Response) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
}
