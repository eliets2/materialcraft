use std::io::{BufRead, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use serde_json::{Value, json};

use crate::budgets::json_text;
use crate::headless::RenderOptions;
use crate::security::{
    AUTH_TIMEOUT, AuthDeadline, ConnectionLimiter, LineRead, MAX_BATCH_STEPS, MAX_CONNECTIONS, MAX_REQUEST_BYTES, read_bounded_line, token_matches,
};
use crate::{AutomationError, Headless, bad};

pub const METHODS: &[&str] = &[
    "ping",
    "methods",
    "session.inspect",
    "command.list",
    "command.run",
    "command.batch",
    "package.open",
    "package.save",
    "bitmap.import",
    "outputs.export",
    "material.export",
    "render",
];

fn str_of<'a>(p: &'a Value, k: &str) -> Option<&'a str> {
    p.get(k).and_then(Value::as_str)
}

fn u32_of(p: &Value, k: &str) -> Result<Option<u32>, AutomationError> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().and_then(|v| u32::try_from(v).ok()).map(Some).ok_or_else(|| bad(format!("`{k}` must be a whole number"))),
    }
}

fn f32_of(p: &Value, k: &str) -> Result<Option<f32>, AutomationError> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().map(|v| Some(v as f32)).ok_or_else(|| bad(format!("`{k}` must be a number"))),
    }
}

fn bool_of(p: &Value, k: &str) -> Result<bool, AutomationError> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(false),
        Some(v) => v.as_bool().ok_or_else(|| bad(format!("`{k}` must be true or false"))),
    }
}

fn required<'a>(p: &'a Value, k: &str) -> Result<&'a str, AutomationError> {
    str_of(p, k).ok_or_else(|| bad(format!("`{k}` is required")))
}

impl Headless {
    /// Dispatch one request. Unknown methods and bad params are errors, never panics
    pub fn handle(&mut self, method: &str, p: &Value) -> Result<Value, AutomationError> {
        match method {
            "ping" => Ok(json!("pong")),
            "methods" => Ok(json!(METHODS)),
            "session.inspect" => Ok(self.inspect()),
            "command.list" => Ok(self.command_list(str_of(p, "filter"))),
            "command.run" => self.command_run(required(p, "id")?, p.get("params").cloned().unwrap_or(Value::Null)),
            "command.batch" => {
                let steps = p.get("steps").and_then(Value::as_array).ok_or_else(|| bad("`steps` must be an array of {id, params}"))?;
                if steps.len() > MAX_BATCH_STEPS {
                    return Err(bad(format!("at most {MAX_BATCH_STEPS} steps per batch")));
                }
                Ok(self.batch(steps))
            }
            "package.open" => self.open(required(p, "path")?),
            "package.save" => self.save(str_of(p, "path")),
            "bitmap.import" => self.import_bitmap(required(p, "path")?, str_of(p, "name")),
            "outputs.export" => self.export(p),
            "material.export" => self.export_material(p),
            "render" => {
                let o = RenderOptions {
                    view: str_of(p, "view").map(str::to_string),
                    width: u32_of(p, "width")?,
                    height: u32_of(p, "height")?,
                    yaw: f32_of(p, "yaw")?,
                    pitch: f32_of(p, "pitch")?,
                    scene: str_of(p, "scene").map(str::to_string),
                    node: u32_of(p, "node")?.map(u64::from),
                    port: u32_of(p, "port")?.map(|v| v as usize),
                    full: bool_of(p, "full")?,
                    tiling: f32_of(p, "tiling")?,
                    hq: bool_of(p, "hq")?,
                    displacement: f32_of(p, "displacement")?,
                };

                let r = self.render_png(&o)?;
                Ok(json!({ "width": r.width, "height": r.height, "png": base64::engine::general_purpose::STANDARD.encode(r.png) }))
            }
            m if m.starts_with("ui.") || m == "viewport.screenshot" => {
                Err(bad(format!("`{m}` needs the running app (materialcraft --control <port>); headless renders use `render`")))
            }
            other => Err(bad(format!("unknown method `{other}` (see `methods`)"))),
        }
    }
}

/// Answer one request line. A panic inside a command becomes an error reply (edits run on a snapshot, so the session stays consistent)
pub fn respond(h: &Mutex<Headless>, line: &str) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"id": null, "ok": false, "error": format!("bad JSON: {e}")}),
    };

    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = req.get("method").and_then(Value::as_str) else {
        return json!({"id": id, "ok": false, "error": "`method` is required"});
    };

    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.lock().unwrap_or_else(PoisonError::into_inner).handle(method, &params)))
        .unwrap_or_else(|_| Err(AutomationError::Other(format!("internal error in `{method}`"))));
    match r {
        Ok(v) => json!({"id": id, "ok": true, "result": v}),
        Err(e) => json!({"id": id, "ok": false, "error": e.to_string()}),
    }
}

/// Write one reply line; an oversized reply becomes an error with the same id
fn write_reply(w: &mut impl Write, reply: &Value) -> std::io::Result<()> {
    let text = json_text(reply).unwrap_or_else(|e| json!({"id": reply.get("id"), "ok": false, "error": e.to_string()}).to_string());
    writeln!(w, "{text}")?;
    w.flush()
}

/// Serve JSON lines from `r` to `w` until EOF (stdio mode: no authentication)
pub fn serve_lines(h: &Mutex<Headless>, mut r: impl BufRead, mut w: impl Write) -> std::io::Result<()> {
    let mut line = String::new();
    loop {
        let reply = match read_bounded_line(&mut r, &mut line) {
            Ok(LineRead::Eof) => return Ok(()),
            Ok(LineRead::TooLong) => json!({"id": null, "ok": false, "error": format!("request exceeds {MAX_REQUEST_BYTES} bytes")}),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => json!({"id": null, "ok": false, "error": "request is not UTF-8"}),
            Err(e) => return Err(e),
            Ok(LineRead::Line) if line.trim().is_empty() => continue,
            Ok(LineRead::Line) => respond(h, &line),
        };

        write_reply(&mut w, &reply)?;
    }
}

/// One TCP connection: the first request must be `auth {token}`, as on the app's channel
fn serve_connection(h: &Mutex<Headless>, stream: std::net::TcpStream, token: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(600)))?;
    let mut r = std::io::BufReader::new(stream.try_clone()?);
    let mut w = stream;
    let mut line = String::new();
    let mut authed = false;
    let mut deadline = Some(AuthDeadline::start(&w, AUTH_TIMEOUT)?);
    loop {
        match read_bounded_line(&mut r, &mut line) {
            Ok(LineRead::Eof) => return Ok(()),
            Ok(LineRead::TooLong) => return write_reply(&mut w, &json!({"id": null, "ok": false, "error": "request too large"})),
            Ok(LineRead::Line) if line.trim().is_empty() => continue,
            Ok(LineRead::Line) => {}
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                write_reply(&mut w, &json!({"id": null, "ok": false, "error": "request is not UTF-8"}))?;
                if !authed {
                    return Ok(());
                }
                continue;
            }
            Err(e) => return Err(e),
        }
        if authed {
            write_reply(&mut w, &respond(h, &line))?;
            continue;
        }
        let req: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let supplied = req.get("params").and_then(|p| p.get("token")).and_then(Value::as_str).unwrap_or_default();
        authed = req.get("method").and_then(Value::as_str) == Some("auth") && token_matches(token, supplied);
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        if !authed {
            return write_reply(&mut w, &json!({"id": id, "ok": false, "error": "authenticate first: {\"method\":\"auth\",\"params\":{\"token\":…}}"}));
        }
        drop(deadline.take());
        write_reply(&mut w, &json!({"id": id, "ok": true}))?;
    }
}

/// Listen on `127.0.0.1:port` (0 picks a port)
pub fn bind_tcp(port: u16) -> Result<TcpListener, AutomationError> {
    TcpListener::bind(("127.0.0.1", port)).map_err(|e| AutomationError::Io(format!("cannot listen on 127.0.0.1:{port}: {e}")))
}

/// Serve authenticated JSON lines on `127.0.0.1:port` (0 picks a port) with one shared session. `ready` gets the bound address
pub fn serve_tcp(port: u16, h: Arc<Mutex<Headless>>, token: String, ready: impl FnOnce(std::net::SocketAddr)) -> Result<(), AutomationError> {
    let listener = bind_tcp(port)?;
    ready(listener.local_addr().map_err(|e| AutomationError::Io(e.to_string()))?);
    serve_listener(listener, h, token)
}

/// Serve authenticated JSON lines on a bound listener (see [`bind_tcp`]) with one shared session
pub fn serve_listener(listener: TcpListener, h: Arc<Mutex<Headless>>, token: String) -> Result<(), AutomationError> {
    let limiter = ConnectionLimiter::default();
    let token = Arc::new(token);
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let Some(permit) = limiter.try_acquire(MAX_CONNECTIONS) else {
            let _ = writeln!(stream, "{}", json!({"id": null, "ok": false, "error": "too many connections"}));
            continue;
        };

        let (h, token) = (h.clone(), token.clone());
        if let Err(e) = std::thread::Builder::new().name("materialcraft-rpc-client".into()).spawn(move || {
            let _permit = permit;
            let _ = serve_connection(&h, stream, &token);
        }) {
            eprintln!("materialcraft: could not start an RPC connection: {e}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::Filesystem;

    fn session() -> Mutex<Headless> {
        Mutex::new(Headless::new(materialcraft_engine::sample_session(), Filesystem::Denied))
    }

    #[test]
    fn lines_round_trip_and_errors() {
        let h = session();
        let input = concat!(
            r#"{"id":1,"method":"command.run","params":{"id":"graph.node.add","params":{"def":"blur"}}}"#,
            "

",
            r#"{"id":2,"method":"session.inspect"}"#,
            "
",
            r#"{"id":3,"method":"nope"}"#,
            "
",
            "not json
",
            r#"{"id":4,"method":"command.run","params":{"id":"file.open","params":{"path":"/etc/passwd"}}}"#,
            "
",
            r#"{"id":5,"method":"package.open","params":{"path":"x.sbs"}}"#,
            "
",
            r#"{"id":6,"method":"command.batch","params":{"steps":[{"id":"edit.selectAll"},{"id":"nope"},{"id":"edit.selectAll"}]}}"#,
            "
",
        );
        let mut out = Vec::new();
        serve_lines(&h, input.as_bytes(), &mut out).unwrap();
        let r: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(r.len(), 7);
        assert_eq!(r[0]["ok"], true);
        assert!(r[1]["result"]["graph"]["nodes"].as_array().unwrap().iter().any(|n| n["def"] == "blur"));
        assert_eq!(r[2]["ok"], false);
        assert_eq!(r[3]["id"], Value::Null);
        assert!(r[4]["error"].as_str().unwrap().contains("disabled for automation"), "{}", r[4]);
        assert!(r[5]["error"].as_str().unwrap().contains("not granted"), "{}", r[5]);
        assert_eq!(r[6]["result"]["completed"], 1);
        assert_eq!(r[6]["result"]["failed"]["index"], 1);
    }

    /// A workspace session rooted at a fresh `inside` folder, with an `outside` sibling.
    fn workspace_session(name: &str, write: bool) -> (std::path::PathBuf, std::path::PathBuf, Mutex<Headless>) {
        let base = std::env::temp_dir().join(format!("mc-rpc-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (inside, outside) = (base.join("inside"), base.join("outside"));
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let ws = crate::AuthorizedWorkspace::new(Some(&inside), write.then_some(inside.as_path())).unwrap();
        let h = Mutex::new(Headless::new(materialcraft_engine::sample_session(), Filesystem::Workspace(ws)));
        (inside, outside, h)
    }

    fn call(h: &Mutex<Headless>, method: &str, params: Value) -> Result<Value, String> {
        h.lock().unwrap().handle(method, &params).map_err(|e| e.to_string())
    }

    #[test]
    fn material_export_writes_a_portable_material_below_the_write_root() {
        let (inside, _, h) = workspace_session("mat-ok", true);
        std::fs::create_dir(inside.join("out")).unwrap();
        let r = call(&h, "material.export", json!({"path": "out/brick", "size": 32})).unwrap();
        assert_eq!(r["path"], "out/brick.mcmat");
        let bytes = std::fs::read(inside.join("out/brick.mcmat")).unwrap();
        assert_eq!(r["bytes"], bytes.len());
        let (m, source) = materialcraft_engine::formats::material::read_material(&bytes).unwrap();
        assert_eq!(m.size, [32, 32]);
        assert!(m.maps.iter().any(|e| e.usage == "height"));
        assert!(!source.is_empty());
        assert_eq!(std::fs::read_dir(inside.join("out")).unwrap().count(), 1, "no temporary files left behind");
        // An explicit extension is kept (case-insensitively) and the file can be opened again.
        let r = call(&h, "material.export", json!({"path": "again.MCMAT", "size": 16})).unwrap();
        assert_eq!(r["path"], "again.MCMAT");
        assert!(call(&h, "package.open", json!({"path": "again.MCMAT"})).is_ok());
    }

    #[test]
    fn material_export_reports_bad_requests() {
        let (inside, _, h) = workspace_session("mat-bad", true);
        for bad in [json!({}), json!({"path": "m", "graph": 99}), json!({"path": "m", "values": {"no_such_input": 1}})] {
            assert!(call(&h, "material.export", bad.clone()).is_err(), "{bad}");
        }
        assert!(call(&h, "material.export", json!({"path": "missing/dir/m"})).is_err(), "parent must exist");
        assert_eq!(std::fs::read_dir(&inside).unwrap().count(), 0, "failed exports leave nothing behind");
    }

    #[test]
    fn material_export_needs_a_write_root() {
        let denied = session();
        let e = call(&denied, "material.export", json!({"path": "m.mcmat"})).unwrap_err();
        assert!(e.contains("not granted"), "{e}");
        let (inside, _, read_only) = workspace_session("mat-readonly", false);
        let e = call(&read_only, "material.export", json!({"path": "m.mcmat"})).unwrap_err();
        assert!(e.contains("no write root"), "{e}");
        assert_eq!(std::fs::read_dir(&inside).unwrap().count(), 0);
    }

    #[test]
    fn material_export_cannot_escape_the_write_root() {
        let (_inside, outside, h) = workspace_session("mat-escape", true);
        let abs = outside.join("abs.mcmat").display().to_string();
        for p in ["../outside/m.mcmat", "a/../../outside/m.mcmat", "/tmp/m.mcmat", "a\\b.mcmat", "C:/m.mcmat", "./m.mcmat", abs.as_str()] {
            let e = call(&h, "material.export", json!({"path": p, "size": 16}));
            assert!(e.is_err(), "accepted {p:?}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, _inside.join("link")).unwrap();
            assert!(call(&h, "material.export", json!({"path": "link/m.mcmat", "size": 16})).is_err());
        }
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0, "nothing written outside the root");
        // The generic command path stays closed too: host paths are still refused.
        let e = call(&h, "command.run", json!({"id": "file.exportMaterial", "params": {"path": abs}})).unwrap_err();
        assert!(e.contains("disabled for automation"), "{e}");
    }

    #[test]
    fn tcp_requires_the_token() {
        use std::io::{BufRead, BufReader};
        let h = Arc::new(session());
        let (tx, rx) = std::sync::mpsc::channel();
        let token = "0123456789abcdef0123".to_string();
        std::thread::spawn(move || serve_tcp(0, h, token, |a| tx.send(a).unwrap()));
        let addr = rx.recv().unwrap();
        let talk = |lines: &[&str]| {
            let s = std::net::TcpStream::connect(addr).unwrap();
            let mut w = s.try_clone().unwrap();
            let mut r = BufReader::new(s);
            lines
                .iter()
                .map(|l| {
                    writeln!(w, "{l}").unwrap();
                    let mut reply = String::new();
                    r.read_line(&mut reply).unwrap();
                    serde_json::from_str::<Value>(&reply).unwrap()
                })
                .collect::<Vec<_>>()
        };

        let denied = talk(&[r#"{"id":1,"method":"session.inspect"}"#]);
        assert_eq!(denied[0]["ok"], false);
        let ok = talk(&[r#"{"id":1,"method":"auth","params":{"token":"0123456789abcdef0123"}}"#, r#"{"id":2,"method":"ping"}"#]);
        assert_eq!(ok[0]["ok"], true);
        assert_eq!(ok[1]["result"], "pong");
        assert_eq!(ok[1]["id"], 2);
    }
}
