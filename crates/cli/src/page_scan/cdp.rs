//! A fast lane for page operations: `Runtime.evaluate` over the DevTools
//! endpoint agent-browser hands out (`get cdp-url`). Every agent-browser
//! command costs a fixed ~170 ms round trip through its daemon, and a scan
//! asks the page a few hundred questions; over the endpoint each one takes a
//! millisecond or two. agent-browser still owns the browser: it opens,
//! waits, logs errors, takes screenshots, and closes.

use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

pub struct PageChannel {
    socket: WebSocket<TcpStream>,
    session_id: String,
    next_id: u64,
}

impl PageChannel {
    /// Attach to the page at `page_url` through the browser endpoint `ws_url`.
    pub fn attach(ws_url: &str, page_url: &str) -> Result<PageChannel, String> {
        let rest = ws_url.strip_prefix("ws://").ok_or("not a local DevTools endpoint")?;
        let host = rest.split('/').next().unwrap_or("");
        let stream = TcpStream::connect(host).map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(60))).map_err(|e| e.to_string())?;
        let (socket, _) = tungstenite::client(ws_url, stream).map_err(|e| e.to_string())?;
        let mut channel = PageChannel { socket, session_id: String::new(), next_id: 0 };
        let targets = channel.send("Target.getTargets", json!({}), None)?;
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
        let attached = channel.send("Target.attachToTarget", json!({ "targetId": target, "flatten": true }), None)?;
        channel.session_id = attached.get("sessionId").and_then(Value::as_str).ok_or("attach returned no session")?.to_string();
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

    fn send(&mut self, method: &str, params: Value, session: Option<&str>) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            message["sessionId"] = json!(s);
        }
        self.socket.send(Message::text(message.to_string())).map_err(|e| e.to_string())?;
        loop {
            let text = match self.socket.read().map_err(|e| e.to_string())? {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                Message::Close(_) => return Err("the DevTools endpoint closed".into()),
                _ => continue,
            };
            let Ok(reply) = serde_json::from_str::<Value>(&text) else { continue };
            // Events and replies to other sessions' commands pass by.
            if reply.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = reply.get("error") {
                return Err(error.get("message").and_then(Value::as_str).unwrap_or("DevTools error").to_string());
            }
            return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}
