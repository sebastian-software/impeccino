//! A fast lane for page operations over agent-browser's local DevTools endpoint.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::handshake::client::ClientHandshake;
use tungstenite::handshake::HandshakeError;
use tungstenite::{Message, WebSocket};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const ATTACH_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const INTERRUPT_POLL: Duration = Duration::from_millis(50);

pub struct PageChannel {
    socket: WebSocket<TcpStream>,
    session_id: String,
    next_id: u64,
}

impl PageChannel {
    /// Attach to the page at `page_url` through a loopback-only DevTools URL.
    pub fn attach(ws_url: &str, page_url: &str) -> Result<PageChannel, String> {
        let deadline = Instant::now() + ATTACH_TIMEOUT;
        let socket = connect_endpoint(ws_url, deadline)?;
        let mut channel = PageChannel {
            socket,
            session_id: String::new(),
            next_id: 0,
        };
        let targets = channel.send_until("Target.getTargets", json!({}), None, deadline)?;
        let pages: Vec<&Value> = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|t| t.get("type").and_then(Value::as_str) == Some("page"))
            .collect();
        let target = pages
            .iter()
            .find(|t| t.get("url").and_then(Value::as_str) == Some(page_url))
            .or_else(|| (pages.len() == 1).then(|| &pages[0]))
            .and_then(|t| t.get("targetId").and_then(Value::as_str))
            .ok_or("the page is not among the browser's targets")?
            .to_string();
        let attached = channel.send_until(
            "Target.attachToTarget",
            json!({ "targetId": target, "flatten": true }),
            None,
            deadline,
        )?;
        channel.session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or("attach returned no session")?
            .to_string();
        Ok(channel)
    }

    /// Evaluate `script` in the page, await it, and return its value.
    pub fn eval(&mut self, script: &str) -> Result<Value, String> {
        let session = self.session_id.clone();
        let params = json!({ "expression": script, "awaitPromise": true, "returnByValue": true });
        let out = self.send("Runtime.evaluate", params, Some(&session))?;
        if let Some(details) = out.get("exceptionDetails") {
            let text = details
                .pointer("/exception/description")
                .or_else(|| details.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("evaluation failed");
            return Err(text.to_string());
        }
        Ok(out.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }

    fn send(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, String> {
        self.send_until(method, params, session, Instant::now() + REQUEST_TIMEOUT)
    }

    fn send_until(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
        deadline: Instant,
    ) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            message["sessionId"] = json!(s);
        }
        let text = message.to_string();
        if Instant::now() >= deadline {
            return Err(format!("DevTools {method} timed out"));
        }
        match self.socket.send(Message::text(text)) {
            Ok(()) => {}
            Err(tungstenite::Error::Io(error)) if is_would_block(&error) => {
                self.flush_until(method, deadline)?
            }
            Err(error) => return Err(format!("DevTools {method}: {error}")),
        }

        loop {
            if super::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(format!("DevTools {method}: interrupted"));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("DevTools {method} timed out"));
            }
            let text = match self.socket.read() {
                Ok(Message::Text(t)) => t.to_string(),
                Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
                Ok(Message::Close(_)) => return Err("the DevTools endpoint closed".into()),
                Ok(_) => continue,
                Err(tungstenite::Error::Io(error)) if is_would_block(&error) => {
                    thread::sleep(INTERRUPT_POLL.min(remaining));
                    continue;
                }
                Err(error) => return Err(format!("DevTools {method}: {error}")),
            };
            let Ok(reply) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            // Events and replies to other sessions' commands pass by, but do
            // not reset the deadline for this request.
            if reply.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = reply.get("error") {
                return Err(error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("DevTools error")
                    .to_string());
            }
            return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn flush_until(&mut self, method: &str, deadline: Instant) -> Result<(), String> {
        loop {
            if super::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(format!("DevTools {method}: interrupted"));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("DevTools {method} timed out"));
            }
            match self.socket.flush() {
                Ok(()) => return Ok(()),
                Err(tungstenite::Error::Io(error)) if is_would_block(&error) => {
                    thread::sleep(INTERRUPT_POLL.min(remaining));
                }
                Err(error) => return Err(format!("DevTools {method}: {error}")),
            }
        }
    }
}

fn is_would_block(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

fn parse_local_endpoint(ws_url: &str) -> Result<Vec<SocketAddr>, String> {
    let rest = ws_url
        .strip_prefix("ws://")
        .ok_or("not a local DevTools endpoint")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("not a local DevTools endpoint".into());
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, port) = bracketed
            .split_once(']')
            .ok_or("not a local DevTools endpoint")?;
        let port = port
            .strip_prefix(':')
            .ok_or("not a local DevTools endpoint")?;
        (host, port)
    } else {
        authority
            .split_once(':')
            .ok_or("not a local DevTools endpoint")?
    };
    let port = port
        .parse::<u16>()
        .map_err(|_| "not a local DevTools endpoint")?;
    if port == 0 {
        return Err("not a local DevTools endpoint".into());
    }
    let ips = if host.eq_ignore_ascii_case("localhost") {
        vec![
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ]
    } else {
        vec![host
            .parse::<IpAddr>()
            .map_err(|_| "not a local DevTools endpoint")?]
    };
    if ips.iter().any(|ip| !ip.is_loopback()) {
        return Err("not a local DevTools endpoint".into());
    }
    Ok(ips
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .collect())
}

fn connect_endpoint(ws_url: &str, deadline: Instant) -> Result<WebSocket<TcpStream>, String> {
    let addresses = parse_local_endpoint(ws_url)?;
    let mut last_error = None;
    for address in addresses {
        if super::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("DevTools connection interrupted".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("DevTools connection timed out".into());
        }
        let stream = match TcpStream::connect_timeout(&address, remaining.min(CONNECT_TIMEOUT)) {
            Ok(stream) => stream,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        let handshake_deadline = (Instant::now() + HANDSHAKE_TIMEOUT).min(deadline);
        stream.set_nonblocking(true).map_err(|e| e.to_string())?;
        let request = ws_url
            .into_client_request()
            .map_err(|e| format!("DevTools handshake: {e}"))?;
        let mut handshake = ClientHandshake::start(stream, request, None)
            .map_err(|e| format!("DevTools handshake: {e}"))?;
        loop {
            match handshake.handshake() {
                Ok((socket, _)) => {
                    if Instant::now() >= handshake_deadline {
                        return Err("DevTools handshake timed out".into());
                    }
                    return Ok(socket);
                }
                Err(HandshakeError::Interrupted(next)) => {
                    if super::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
                        return Err("DevTools handshake interrupted".into());
                    }
                    let remaining = handshake_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err("DevTools handshake timed out".into());
                    }
                    handshake = next;
                    thread::sleep(INTERRUPT_POLL.min(remaining));
                }
                Err(HandshakeError::Failure(error)) => {
                    return Err(format!("DevTools handshake: {error}"))
                }
            }
        }
    }
    Err(format!(
        "DevTools connect: {}",
        last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "timed out".into())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn accept_with_timeout(listener: &TcpListener, timeout: Duration) -> Option<TcpStream> {
        listener.set_nonblocking(true).ok()?;
        let deadline = Instant::now() + timeout;
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(false).is_err() {
                        return None;
                    }
                    return Some(stream);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return None,
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn reply_to_request(socket: &mut WebSocket<TcpStream>, request: &Value) -> bool {
        let Some(id) = request.get("id").and_then(Value::as_u64) else {
            return false;
        };
        let result = match request.get("method").and_then(Value::as_str) {
            Some("Target.getTargets") => {
                json!({ "targetInfos": [{ "type": "page", "url": "http://localhost/", "targetId": "target" }] })
            }
            Some("Target.attachToTarget") => json!({ "sessionId": "attached" }),
            Some("Runtime.evaluate") => {
                json!({ "result": { "type": "string", "value": "local reply" } })
            }
            _ => return false,
        };
        socket
            .send(Message::text(
                json!({ "id": id, "result": result }).to_string(),
            ))
            .is_ok()
    }

    #[test]
    fn accepts_only_numeric_loopback_and_localhost_endpoints() {
        assert_eq!(
            parse_local_endpoint("ws://127.0.0.1:9222/devtools/browser/id").unwrap(),
            vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9222)]
        );
        assert_eq!(
            parse_local_endpoint("ws://localhost:9222/devtools/browser/id").unwrap(),
            vec![
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9222),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9222),
            ]
        );
        assert_eq!(
            parse_local_endpoint("ws://[::1]:9222/devtools/browser/id").unwrap(),
            vec![SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9222)]
        );
        for endpoint in [
            "ws://example.com:9222/devtools/browser/id",
            "ws://192.0.2.1:9222/devtools/browser/id",
            "ws://user@localhost:9222/devtools/browser/id",
            "wss://localhost:9222/devtools/browser/id",
            "ws://[::]:9222/devtools/browser/id",
        ] {
            assert!(
                parse_local_endpoint(endpoint).is_err(),
                "accepted {endpoint}"
            );
        }
    }

    #[test]
    fn a_loopback_ipv6_endpoint_can_connect() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return;
        };
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let Some(stream) = accept_with_timeout(&listener, Duration::from_secs(2)) else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let _socket = tungstenite::accept(stream);
        });
        let url = format!("ws://[::1]:{}/devtools/browser/test", address.port());
        let socket = connect_endpoint(&url, Instant::now() + Duration::from_secs(2)).unwrap();
        drop(socket);
        server.join().unwrap();
    }

    #[test]
    fn handshake_deadline_is_not_reset_by_a_trickling_response() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let Some(mut stream) = accept_with_timeout(&listener, Duration::from_secs(2)) else {
                return;
            };
            if stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .is_err()
                || stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .is_err()
            {
                return;
            }
            let mut request = Vec::new();
            let mut buffer = [0; 512];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = match stream.read(&mut buffer) {
                    Ok(count) => count,
                    Err(_) => return,
                };
                if count == 0 {
                    return;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.len() >= 8192 {
                    return;
                }
            }
            let response = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n";
            for byte in response {
                if stream.write_all(&[*byte]).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let url = format!("ws://127.0.0.1:{}/devtools/browser/test", address.port());
        let start = Instant::now();
        let error = match connect_endpoint(&url, start + Duration::from_millis(100)) {
            Ok(_) => panic!("the endpoint unexpectedly completed its handshake"),
            Err(error) => error,
        };
        assert!(
            error.contains("handshake") || error.contains("timed out"),
            "{error}"
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        server.join().unwrap();
    }

    #[test]
    fn unrelated_events_do_not_restart_a_devtools_request_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let Some(stream) = accept_with_timeout(&listener, Duration::from_secs(2)) else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let Ok(mut socket) = tungstenite::accept(stream) else {
                return;
            };
            let until = Instant::now() + Duration::from_millis(350);
            if socket
                .get_mut()
                .set_write_timeout(Some(Duration::from_millis(50)))
                .is_err()
            {
                return;
            }
            while Instant::now() < until {
                if socket
                    .send(Message::text(
                        r#"{"method":"Page.loadEventFired","params":{}}"#,
                    ))
                    .is_err()
                {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        let url = format!("ws://127.0.0.1:{}/devtools/browser/test", address.port());
        let socket = connect_endpoint(&url, Instant::now() + Duration::from_secs(2)).unwrap();
        let mut channel = PageChannel {
            socket,
            session_id: String::new(),
            next_id: 0,
        };
        let start = Instant::now();
        let result = channel.send_until(
            "never.replies",
            json!({}),
            None,
            start + Duration::from_millis(120),
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(channel);
        server.join().unwrap();
    }

    #[test]
    fn a_trickling_partial_frame_cannot_extend_a_devtools_request_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let Some(stream) = accept_with_timeout(&listener, Duration::from_secs(2)) else {
                return 0;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let Ok(mut socket) = tungstenite::accept(stream) else {
                return 0;
            };
            if socket.read().is_err() {
                return 0;
            }
            let mut stream = socket.into_inner();
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
            // A server text frame with a 100-byte payload, deliberately
            // delivered one byte at a time. Per-read timeouts alone would
            // keep this frame alive for about a second.
            if stream.write_all(&[0x81, 100]).is_err() {
                return 0;
            }
            let mut sent = 0;
            for _ in 0..100 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                sent += 1;
                thread::sleep(Duration::from_millis(10));
                let mut probe = [0; 1];
                match stream.peek(&mut probe) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) if is_would_block(&error) => {}
                    Err(_) => break,
                }
            }
            sent
        });
        let endpoint = format!("ws://127.0.0.1:{}/devtools/browser/test", address.port());
        let socket = connect_endpoint(&endpoint, Instant::now() + Duration::from_secs(2)).unwrap();
        let mut channel = PageChannel {
            socket,
            session_id: String::new(),
            next_id: 0,
        };
        let start = Instant::now();
        let result = channel.send_until(
            "wait.for.frame",
            json!({}),
            None,
            start + Duration::from_millis(120),
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(channel);
        assert!(
            server.join().unwrap() < 100,
            "server completed the slow frame"
        );
    }

    #[test]
    fn a_backpressured_websocket_write_obeys_the_request_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let request_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_request_started = std::sync::Arc::clone(&request_started);
        let server = thread::spawn(move || {
            let Some(stream) = accept_with_timeout(&listener, Duration::from_secs(2)) else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let Ok(socket) = tungstenite::accept(stream) else {
                return;
            };
            let stream = socket.get_ref();
            let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
            let until = Instant::now() + Duration::from_secs(2);
            while Instant::now() < until {
                let mut probe = [0; 1];
                match stream.peek(&mut probe) {
                    Ok(count) if count > 0 => {
                        server_request_started.store(true, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                    Ok(0) => return,
                    Ok(_) => {}
                    Err(error) if is_would_block(&error) => {}
                    Err(_) => return,
                }
                thread::sleep(Duration::from_millis(2));
            }
            thread::sleep(Duration::from_millis(1_200));
            drop(socket);
        });
        let endpoint = format!("ws://127.0.0.1:{}/devtools/browser/test", address.port());
        let socket = connect_endpoint(&endpoint, Instant::now() + Duration::from_secs(2)).unwrap();
        let mut channel = PageChannel {
            socket,
            session_id: String::new(),
            next_id: 0,
        };
        let large_params = json!({ "payload": "x".repeat(8 * 1024 * 1024) });
        let start = Instant::now();
        let result = channel.send_until(
            "write.until.blocked",
            large_params,
            None,
            start + Duration::from_secs(1),
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_millis(1_150));
        assert!(
            request_started.load(std::sync::atomic::Ordering::SeqCst),
            "client timed out before any request bytes reached the server"
        );
        drop(channel);
        server.join().unwrap();
    }

    #[test]
    fn attaches_and_evaluates_over_a_bounded_local_tcp_server() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let stream = accept_with_timeout(&listener, Duration::from_secs(2))
                .ok_or("no client connection")?;
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let mut socket =
                tungstenite::accept(stream).map_err(|error| format!("handshake: {error}"))?;
            for _ in 0..3 {
                let message = socket
                    .read()
                    .map_err(|error| format!("read request: {error}"))?;
                let text = match message {
                    Message::Text(text) => text.to_string(),
                    Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                    message => return Err(format!("unexpected client message: {message:?}")),
                };
                let request = serde_json::from_str::<Value>(&text)
                    .map_err(|error| format!("parse request {text:?}: {error}"))?;
                if !reply_to_request(&mut socket, &request) {
                    return Err(format!("unknown or unreplyable request: {request}"));
                }
            }
            Ok::<(), String>(())
        });
        let endpoint = format!("ws://127.0.0.1:{}/devtools/browser/test", address.port());
        let mut channel = match PageChannel::attach(&endpoint, "http://localhost/") {
            Ok(channel) => channel,
            Err(error) => panic!("{error}; local server: {:?}", server.join().unwrap()),
        };
        assert_eq!(channel.eval("1 + 1").unwrap(), json!("local reply"));
        drop(channel);
        server.join().unwrap().unwrap();
    }
}
