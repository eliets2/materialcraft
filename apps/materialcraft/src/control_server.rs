//! Loopback JSON-lines control server (`--control <port>`): one request per line,
//! `{"id", "method", "params"}`, one reply per line. The first request must be
//! `{"method":"auth","params":{"token":"…"}}` with the token from `MATERIALCRAFT_CONTROL_TOKEN`
//! or, when that is unset, the token written (owner-only) to
//! `<temp>/materialcraft-control-<port>.token`. Token handling, request framing and connection
//! limits are shared with `materialcraft-cli serve` (`materialcraft_automation::security`).

use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use materialcraft_automation::security::{
    AUTH_TIMEOUT, AuthDeadline, ConnectionLimiter, LineRead, MAX_CONNECTIONS, read_bounded_line, server_token, token_matches,
};
use materialcraft_ui_egui::ControlRequest;
use serde_json::{Value, json};

#[cfg(test)]
#[path = "control_server.test.rs"]
mod exact_token_tests;

fn auth_ok(token: &str, method: &str, params: &Value) -> bool {
    method == "auth" && params.get("token").and_then(Value::as_str).is_some_and(|t| token_matches(token, t))
}

/// How long an authenticated connection may stay idle.
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);

/// Listen on `127.0.0.1:port` and return the requests for the UI thread to answer
/// ([`ControlRequest::reply`]). Bind or token failures are printed and leave the channel empty.
pub fn start(port: u16, ctx: egui::Context) -> Receiver<ControlRequest> {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("materialcraft: control server failed to bind 127.0.0.1:{port}: {e}");
            return channel().1;
        }
    };
    start_listener(listener, ctx)
}

fn start_listener(listener: TcpListener, ctx: egui::Context) -> Receiver<ControlRequest> {
    let (tx, rx) = channel::<ControlRequest>();
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("materialcraft: could not determine the control port: {e}");
            return rx;
        }
    };
    // After binding, so an app that cannot listen never replaces another server's token.
    let token = match server_token(addr.port()) {
        Ok((token, Some(path))) => {
            eprintln!("materialcraft: control token written to {}", path.display());
            token
        }
        Ok((token, None)) => token,
        Err(e) => {
            eprintln!("materialcraft: control server disabled: {e}");
            return rx;
        }
    };
    eprintln!("materialcraft: control server listening on {addr}");
    if let Err(e) = std::thread::Builder::new().name("materialcraft-control".into()).spawn(move || accept(listener, &token, &tx, &ctx)) {
        eprintln!("materialcraft: could not start the control server: {e}");
    }
    rx
}

/// Serve connections from `listener` (at most [`MAX_CONNECTIONS`] at once).
fn accept(listener: TcpListener, token: &str, tx: &Sender<ControlRequest>, ctx: &egui::Context) {
    let limiter = ConnectionLimiter::default();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let Some(permit) = limiter.try_acquire(MAX_CONNECTIONS) else {
            let _ = writeln!(stream, "{}", json!({"id": null, "ok": false, "error": "too many connections"}));
            continue;
        };
        let (tx, ctx, token) = (tx.clone(), ctx.clone(), token.to_string());
        if let Err(e) = std::thread::Builder::new().name("materialcraft-control-client".into()).spawn(move || {
            let _permit = permit;
            serve(stream, &token, &tx, &ctx);
        }) {
            log::warn!("could not start a control connection: {e}");
        }
    }
}

/// Send `reply` with the request's `id`; false when the client is gone.
fn reply(out: &mut TcpStream, id: Value, mut reply: Value) -> bool {
    if let Some(o) = reply.as_object_mut() {
        o.insert("id".into(), id);
    }
    writeln!(out, "{reply}").is_ok() && out.flush().is_ok()
}

/// One connection: `auth` first (within [`AUTH_TIMEOUT`], one attempt), then requests
/// forwarded to the UI thread one at a time until the client disconnects or idles out.
fn serve(stream: TcpStream, token: &str, tx: &Sender<ControlRequest>, ctx: &egui::Context) {
    let Ok(mut deadline) = AuthDeadline::start(&stream, AUTH_TIMEOUT).map(Some) else { return };
    let _ = stream.set_read_timeout(Some(IDLE_TIMEOUT));
    let Ok(read) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read);
    let mut out = stream;
    let mut line = String::new();
    loop {
        match read_bounded_line(&mut reader, &mut line) {
            Ok(LineRead::Line) => {}
            Ok(LineRead::TooLong) => {
                reply(&mut out, Value::Null, json!({"ok": false, "error": "request too large"}));
                return;
            }
            Ok(LineRead::Eof) => return,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                let sent = reply(&mut out, Value::Null, json!({"ok": false, "error": "request is not UTF-8"}));
                if !sent || deadline.is_some() {
                    return;
                }
                continue;
            }
            Err(_) => return,
        }
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let sent = reply(&mut out, Value::Null, json!({"ok": false, "error": format!("bad JSON: {e}")}));
                // Before `auth` a malformed line is the one refused attempt, as on `serve --port`.
                if !sent || deadline.is_some() {
                    return;
                }
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        if deadline.is_some() {
            if !auth_ok(token, &method, &params) {
                // One attempt per connection, as on `materialcraft-cli serve`.
                reply(&mut out, id, json!({"ok": false, "error": "authenticate first: {\"method\":\"auth\",\"params\":{\"token\":…}}"}));
                return;
            }
            drop(deadline.take());
            if !reply(&mut out, id, json!({"ok": true})) {
                return;
            }
            continue;
        }
        let (req, rrx) = ControlRequest::new(method, params);
        if tx.send(req).is_err() {
            return;
        }
        ctx.request_repaint();
        let answer = rrx.recv_timeout(Duration::from_secs(120)).unwrap_or_else(|_| json!({"ok": false, "error": "timeout"}));
        if !reply(&mut out, id, answer) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn ephemeral_listener_uses_its_actual_port_for_the_token() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        assert_ne!(addr.port(), 0);
        let _requests = start_listener(listener, egui::Context::default());
        let token = materialcraft_automation::security::client_token(addr.port(), None).unwrap();
        let (mut w, mut r) = connect(addr);
        let auth = json!({"method": "auth", "params": {"token": token}}).to_string();
        assert_eq!(call(&mut w, &mut r, &auth).unwrap()["ok"], true);
    }

    /// A server on a free port whose app side answers every request with its method name.
    fn server() -> std::net::SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = channel::<ControlRequest>();
        std::thread::spawn(move || accept(listener, TOKEN, &tx, &egui::Context::default()));
        std::thread::spawn(move || {
            for r in rx {
                let _ = r.reply.send(json!({"ok": true, "result": r.method}));
            }
        });
        addr
    }

    fn connect(addr: std::net::SocketAddr) -> (TcpStream, BufReader<TcpStream>) {
        let s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let r = BufReader::new(s.try_clone().unwrap());
        (s, r)
    }

    fn call(w: &mut TcpStream, r: &mut BufReader<TcpStream>, line: &str) -> Option<Value> {
        writeln!(w, "{line}").ok()?;
        let mut reply = String::new();
        (r.read_line(&mut reply).ok()? > 0).then(|| serde_json::from_str(&reply).unwrap())
    }

    #[test]
    fn requests_need_the_token() {
        let addr = server();
        // Anything before `auth` is refused and the connection closed.
        let (mut w, mut r) = connect(addr);
        let denied = call(&mut w, &mut r, r#"{"id":1,"method":"command.run","params":{"id":"file.open"}}"#).unwrap();
        assert_eq!(denied["ok"], false);
        assert_eq!(denied["id"], 1);
        assert!(call(&mut w, &mut r, r#"{"id":2,"method":"ping"}"#).is_none(), "closed after a refused request");
        // A wrong token is refused and gets no second try.
        let (mut w, mut r) = connect(addr);
        let wrong = format!(r#"{{"id":1,"method":"auth","params":{{"token":"{}x"}}}}"#, &TOKEN[1..]);
        assert_eq!(call(&mut w, &mut r, &wrong).unwrap()["ok"], false);
        let right = format!(r#"{{"id":2,"method":"auth","params":{{"token":"{TOKEN}"}}}}"#);
        assert!(call(&mut w, &mut r, &right).is_none());
        // So is a malformed first line: no further lines before `auth`.
        let (mut w, mut r) = connect(addr);
        assert_eq!(call(&mut w, &mut r, "not json").unwrap()["ok"], false);
        assert!(call(&mut w, &mut r, &right).is_none(), "closed after malformed JSON before auth");
        // The right token unlocks the connection.
        let (mut w, mut r) = connect(addr);
        let ok = call(&mut w, &mut r, &right).unwrap();
        assert_eq!((ok["ok"].clone(), ok["id"].clone()), (json!(true), json!(2)));
        let pong = call(&mut w, &mut r, r#"{"id":3,"method":"ping"}"#).unwrap();
        assert_eq!((pong["result"].clone(), pong["id"].clone()), (json!("ping"), json!(3)));
        assert_eq!(call(&mut w, &mut r, "not json").unwrap()["ok"], false);
        assert_eq!(call(&mut w, &mut r, r#"{"id":4,"method":"ping"}"#).unwrap()["id"], 4, "bad JSON keeps an authenticated connection");
    }

    #[test]
    fn non_utf8_line_before_auth_gets_refusal_reply() {
        let addr = server();
        let (mut w, mut r) = connect(addr);
        w.write_all(b"\xff\n").unwrap();
        w.flush().unwrap();
        let mut reply = String::new();
        let n = r.read_line(&mut reply).unwrap();
        assert!(n > 0, "a non-UTF8 first line gets a refusal reply, not a silent close");
        assert_eq!(serde_json::from_str::<Value>(&reply).unwrap()["ok"], false);
    }

    #[test]
    fn connections_are_limited_and_idle_ones_time_out() {
        let addr = server();
        let idle: Vec<_> = (0..MAX_CONNECTIONS).map(|_| connect(addr)).collect();
        // Let the server take every idle connection before the one over the limit.
        std::thread::sleep(Duration::from_millis(300));
        // Read without writing: the server closes at once, and unread input would reset the
        // connection before its reply arrives.
        let (_, mut r) = connect(addr);
        let mut full = String::new();
        r.read_line(&mut full).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&full).unwrap()["error"], "too many connections");
        // Unauthenticated connections are closed after AUTH_TIMEOUT, which frees their slots.
        for (_, mut r) in idle {
            let mut rest = String::new();
            assert_eq!(r.read_line(&mut rest).unwrap(), 0);
        }
        // A slot frees once its thread has ended, which can trail the client's EOF a little.
        let right = format!(r#"{{"id":1,"method":"auth","params":{{"token":"{TOKEN}"}}}}"#);
        let authed = (0..50).any(|_| {
            let (mut w, mut r) = connect(addr);
            let ok = call(&mut w, &mut r, &right).is_some_and(|v| v["ok"] == true);
            if !ok {
                std::thread::sleep(Duration::from_millis(100));
            }
            ok
        });
        assert!(authed, "slots are freed after the auth deadline");
    }
}
