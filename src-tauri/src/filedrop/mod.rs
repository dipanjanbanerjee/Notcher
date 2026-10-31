//! Notcher Drop: send files between a phone and this PC over the local network.
//!
//! Starting a session opens a small web server on a random port, guarded by a random
//! key. The island shows a QR code with the address. The phone's camera opens it in the
//! browser, where you can send files to the PC (saved to `Downloads\Notcher Drop`) and
//! download files the PC shares. Nothing leaves the local network, and the server stops
//! on its own after a while without activity.

mod http;
mod names;

use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, UdpSocket},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tiny_http::{Method, Request, Server};

/// Full state for the Drop tab, sent whenever anything changes.
pub const STATE_EVENT: &str = "drop://state";
/// A file finished arriving from the phone.
pub const RECEIVED_EVENT: &str = "drop://received";

/// The server stops after this long without a request or an upload in progress.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Upload progress is sent to the UI at most this often per file.
const PROGRESS_EVERY: Duration = Duration::from_millis(150);
/// Received files kept in the list (older ones stay on disk, just not listed).
const MAX_RECEIVED: usize = 30;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DropState {
    pub active: bool,
    /// Address the phone opens, including the key.
    pub url: Option<String>,
    /// QR code of `url` as an SVG `data:` URL.
    pub qr: Option<String>,
    /// Where received files are saved.
    pub folder: String,
    /// Newest first.
    pub received: Vec<Received>,
    pub shared: Vec<Shared>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Receiving,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Received {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub done: u64,
    pub status: Status,
    #[serde(skip)]
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shared {
    pub id: u64,
    pub name: String,
    pub size: u64,
    #[serde(skip)]
    path: PathBuf,
}

struct Session {
    server: Arc<Server>,
    key: String,
    last_activity: Instant,
    /// Changes with every session so old watchdog threads know to quit.
    generation: u64,
}

#[derive(Default)]
struct Inner {
    state: DropState,
    session: Option<Session>,
    next_id: u64,
    generation: u64,
}

impl Inner {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

static INNER: LazyLock<Mutex<Inner>> = LazyLock::new(Mutex::default);

fn with<T>(f: impl FnOnce(&mut Inner) -> T) -> T {
    let mut inner = INNER.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
}

fn snapshot() -> DropState {
    with(|i| i.state.clone())
}

fn publish<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(STATE_EVENT, snapshot());
}

fn touch() {
    with(|i| {
        if let Some(s) = i.session.as_mut() {
            s.last_activity = Instant::now();
        }
    });
}

fn folder<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let base = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Notcher Drop")
}

/// The address other devices on the network can reach this PC at. Connecting a UDP
/// socket sends nothing; it only asks the system which interface it would route
/// through.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

fn random_key() -> Result<String, String> {
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn qr_data_url(text: &str) -> Option<String> {
    use base64::Engine;
    use qrcode::{render::svg, EcLevel, QrCode};
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).ok()?;
    let svg = code
        .render::<svg::Color>()
        .quiet_zone(false)
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Some(format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    ))
}

fn start_session<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if with(|i| i.session.is_some()) {
        return Ok(());
    }
    let ip = lan_ip().ok_or("Connect to Wi-Fi or a network first.")?;
    let dir = folder(app);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let server = Arc::new(Server::http("0.0.0.0:0").map_err(|e| e.to_string())?);
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or("The server has no port")?;
    let key = random_key()?;
    let url = format!("http://{ip}:{port}/?k={key}");
    let generation = with(|i| {
        i.generation += 1;
        let generation = i.generation;
        i.session = Some(Session {
            server: server.clone(),
            key,
            last_activity: Instant::now(),
            generation,
        });
        i.state.active = true;
        i.state.qr = qr_data_url(&url);
        i.state.url = Some(url);
        i.state.folder = dir.display().to_string();
        i.state.error = None;
        generation
    });

    let handle = app.clone();
    thread::Builder::new()
        .name("notcher-drop".into())
        .spawn(move || {
            for request in server.incoming_requests() {
                let app = handle.clone();
                let _ = thread::Builder::new()
                    .name("notcher-drop-request".into())
                    .spawn(move || handle_request(&app, request));
            }
        })
        .map_err(|e| e.to_string())?;

    let handle = app.clone();
    let _ = thread::Builder::new()
        .name("notcher-drop-idle".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            let idle = with(|i| {
                let busy = i
                    .state
                    .received
                    .iter()
                    .any(|r| r.status == Status::Receiving);
                match &i.session {
                    Some(s) if s.generation == generation => {
                        Some(!busy && s.last_activity.elapsed() > IDLE_TIMEOUT)
                    }
                    _ => None,
                }
            });
            match idle {
                Some(true) => {
                    log::info!("Notcher Drop stopped after being idle");
                    stop_session(&handle);
                    return;
                }
                Some(false) => {}
                None => return,
            }
        });
    Ok(())
}

fn stop_session<R: Runtime>(app: &AppHandle<R>) {
    let session = with(|i| {
        i.state.active = false;
        i.state.url = None;
        i.state.qr = None;
        i.session.take()
    });
    if let Some(s) = session {
        s.server.unblock();
    }
    publish(app);
}

// ---------------------------------------------------------------- requests

fn handle_request<R: Runtime>(app: &AppHandle<R>, request: Request) {
    let target = http::Target::parse(request.url());
    let key_ok = with(|i| {
        i.session
            .as_ref()
            .is_some_and(|s| http::same_secret(&target.query("k").unwrap_or_default(), &s.key))
    });
    if !key_ok {
        let _ = request.respond(http::text(
            403,
            "This Notcher Drop link has expired. Scan the code on your PC again.",
        ));
        return;
    }
    touch();
    match (request.method(), target.path.as_str()) {
        (Method::Get, "/") => {
            let _ = request.respond(http::html(PHONE_PAGE));
        }
        (Method::Get, "/api/files") => {
            let body = with(|i| {
                serde_json::json!({
                    "pc": computer_name(),
                    "shared": i.state.shared,
                })
            });
            let _ = request.respond(http::json(&body));
        }
        (Method::Get, path) if path.starts_with("/api/file/") => {
            let id = path["/api/file/".len()..].parse::<u64>().ok();
            send_shared(request, id);
        }
        (Method::Post, "/api/upload") => {
            let name = target.query("name").unwrap_or_default();
            receive(app, request, &name);
        }
        _ => {
            let _ = request.respond(http::text(404, "Not found"));
        }
    }
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "this PC".into())
}

fn send_shared(request: Request, id: Option<u64>) {
    let file = id.and_then(|id| with(|i| i.state.shared.iter().find(|s| s.id == id).cloned()));
    let Some(shared) = file else {
        let _ = request.respond(http::text(404, "That file is no longer shared."));
        return;
    };
    match File::open(&shared.path) {
        Ok(f) => {
            let _ = request.respond(http::download(f, &shared.name));
        }
        Err(_) => {
            let _ = request.respond(http::text(410, "That file was moved or deleted on the PC."));
        }
    }
}

fn receive<R: Runtime>(app: &AppHandle<R>, mut request: Request, raw_name: &str) {
    let dir = folder(app);
    let name = names::sanitize(raw_name);
    let size = request.body_length().unwrap_or(0) as u64;
    let opened = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|_| create_unique(&dir, &name));
    let (path, mut file) = match opened {
        Ok(v) => v,
        Err(err) => {
            log::warn!("Notcher Drop could not save {name}: {err}");
            let _ = request.respond(http::text(500, "The PC could not save this file."));
            return;
        }
    };
    let saved_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(name);
    let id = with(|i| {
        let id = i.id();
        i.state.received.insert(
            0,
            Received {
                id,
                name: saved_name.clone(),
                size,
                done: 0,
                status: Status::Receiving,
                path: path.clone(),
            },
        );
        i.state.received.truncate(MAX_RECEIVED);
        id
    });
    publish(app);

    let result = copy_with_progress(app, request.as_reader(), &mut file, id);
    drop(file);
    let status = match result {
        Ok(_) => Status::Done,
        Err(ref err) => {
            log::warn!("Notcher Drop upload of {saved_name} failed: {err}");
            let _ = std::fs::remove_file(&path);
            Status::Failed
        }
    };
    let total = with(|i| {
        let entry = i.state.received.iter_mut().find(|r| r.id == id)?;
        entry.status = status;
        if let Ok(n) = result {
            entry.done = n;
            entry.size = n;
        }
        Some(entry.size)
    });
    publish(app);
    touch();
    if status == Status::Done {
        let _ = app.emit(
            RECEIVED_EVENT,
            serde_json::json!({ "name": saved_name, "size": total.unwrap_or(size) }),
        );
        let _ = request.respond(http::json(
            &serde_json::json!({ "ok": true, "name": saved_name }),
        ));
    } else {
        let _ = request.respond(http::text(500, "The upload was interrupted."));
    }
}

/// Creates a new file, adding " (2)", " (3)"… so nothing already there is replaced.
fn create_unique(dir: &Path, name: &str) -> Result<(PathBuf, File), String> {
    for n in 1..10_000 {
        let path = dir.join(names::numbered(name, n));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("too many files with this name".into())
}

fn copy_with_progress<R: Runtime>(
    app: &AppHandle<R>,
    reader: &mut dyn Read,
    file: &mut File,
    id: u64,
) -> std::io::Result<u64> {
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    let mut last = Instant::now();
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        total += n as u64;
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            with(|i| {
                if let Some(s) = i.session.as_mut() {
                    s.last_activity = last;
                }
                if let Some(r) = i.state.received.iter_mut().find(|r| r.id == id) {
                    r.done = total;
                }
            });
            publish(app);
        }
    }
    file.flush()?;
    Ok(total)
}

const PHONE_PAGE: &str = include_str!("phone.html");

// ---------------------------------------------------------------- commands

/// Starts a session (if needed) and returns the state with its QR code.
#[tauri::command]
pub fn drop_start<R: Runtime>(app: AppHandle<R>) -> DropState {
    if let Err(err) = start_session(&app) {
        log::warn!("Notcher Drop could not start: {err}");
        with(|i| i.state.error = Some(err));
    }
    publish(&app);
    snapshot()
}

#[tauri::command]
pub fn drop_stop<R: Runtime>(app: AppHandle<R>) {
    stop_session(&app);
}

#[tauri::command]
pub fn drop_state<R: Runtime>(app: AppHandle<R>) -> DropState {
    with(|i| {
        if i.state.folder.is_empty() {
            i.state.folder = folder(&app).display().to_string();
        }
    });
    snapshot()
}

/// Shares files with the phone, starting a session if none is running.
#[tauri::command]
pub fn drop_share<R: Runtime>(app: AppHandle<R>, paths: Vec<String>) -> DropState {
    with(|i| {
        for p in paths {
            let path = PathBuf::from(p);
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() || i.state.shared.iter().any(|s| s.path == path) {
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            let id = i.id();
            i.state.shared.push(Shared {
                id,
                name,
                size: meta.len(),
                path,
            });
        }
    });
    drop_start(app)
}

#[tauri::command]
pub fn drop_unshare<R: Runtime>(app: AppHandle<R>, id: u64) {
    with(|i| i.state.shared.retain(|s| s.id != id));
    publish(&app);
}

/// Forgets finished and failed transfers (the files stay on disk).
#[tauri::command]
pub fn drop_clear_received<R: Runtime>(app: AppHandle<R>) {
    with(|i| i.state.received.retain(|r| r.status == Status::Receiving));
    publish(&app);
}

/// Opens a received file with its default app, or the Drop folder when `id` is empty.
#[tauri::command]
pub fn drop_open<R: Runtime>(app: AppHandle<R>, id: Option<u64>, reveal: bool) {
    let path = match id {
        Some(id) => with(|i| {
            i.state
                .received
                .iter()
                .find(|r| r.id == id && r.status == Status::Done)
                .map(|r| r.path.clone())
        }),
        None => {
            let dir = folder(&app);
            let _ = std::fs::create_dir_all(&dir);
            Some(dir)
        }
    };
    if let Some(path) = path {
        open_path(&path, reveal);
    }
}

#[cfg(windows)]
fn open_path(path: &Path, reveal: bool) {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("explorer.exe");
    if reveal {
        // explorer wants "/select," and the path as one argument, unquoted.
        cmd.raw_arg(format!("/select,\"{}\"", path.display()));
    } else {
        cmd.arg(path);
    }
    if let Err(err) = cmd.spawn() {
        log::warn!("could not open {}: {err}", path.display());
    }
}

#[cfg(not(windows))]
fn open_path(path: &Path, reveal: bool) {
    let target = if reveal {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let _ = std::process::Command::new("xdg-open").arg(target).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sends a raw HTTP request to the running server and returns (status, body).
    fn try_request(port: u16, head: &str, body: &[u8]) -> std::io::Result<(u16, Vec<u8>)> {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port))?;
        s.write_all(head.as_bytes())?;
        s.write_all(body)?;
        let mut out = Vec::new();
        s.read_to_end(&mut out)?;
        let status = String::from_utf8_lossy(out.get(9..12).unwrap_or_default())
            .parse()
            .unwrap_or(0);
        let split = out
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map_or(out.len(), |i| i + 4);
        Ok((status, out[split..].to_vec()))
    }

    fn request(port: u16, head: &str, body: &[u8]) -> (u16, Vec<u8>) {
        try_request(port, head, body).unwrap()
    }

    #[test]
    fn phone_can_upload_and_download_and_the_key_is_required() {
        if lan_ip().is_none() {
            eprintln!("no network here, skipping");
            return;
        }
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        start_session(&handle).unwrap();
        let state = snapshot();
        let url = state.url.clone().unwrap();
        let port: u16 = url
            .rsplit(':')
            .next()
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let key = url.split("k=").nth(1).unwrap().to_string();
        let close = "Connection: close\r\n";

        // Without the key nothing is served.
        let (status, _) = request(
            port,
            &format!("GET / HTTP/1.1\r\nHost: x\r\n{close}\r\n"),
            b"",
        );
        assert_eq!(status, 403);
        let (status, page) = request(
            port,
            &format!("GET /?k={key} HTTP/1.1\r\nHost: x\r\n{close}\r\n"),
            b"",
        );
        assert_eq!(status, 200);
        assert!(String::from_utf8_lossy(&page).contains("Notcher Drop"));

        // Phone → PC: a name with folders is saved safely inside the Drop folder.
        let body = b"hello from the phone";
        let head = format!(
            "POST /api/upload?k={key}&name=..%2F..%2Fnote%201.txt HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n{close}\r\n",
            body.len()
        );
        let (status, _) = request(port, &head, body);
        assert_eq!(status, 200);
        let got = snapshot()
            .received
            .into_iter()
            .find(|r| r.name.starts_with("note 1"))
            .unwrap();
        assert_eq!(got.status, Status::Done);
        assert_eq!(got.path.parent().unwrap(), folder(&handle));
        assert_eq!(std::fs::read(&got.path).unwrap(), body);
        let _ = std::fs::remove_file(&got.path);

        // PC → phone.
        let shared = std::env::temp_dir().join(format!("notcher-share-{}.txt", std::process::id()));
        std::fs::write(&shared, b"from the pc").unwrap();
        drop_share(handle.clone(), vec![shared.display().to_string()]);
        let id = snapshot().shared[0].id;
        let (status, list) = request(
            port,
            &format!("GET /api/files?k={key} HTTP/1.1\r\nHost: x\r\n{close}\r\n"),
            b"",
        );
        assert_eq!(status, 200);
        assert!(String::from_utf8_lossy(&list).contains("notcher-share-"));
        let (status, file) = request(
            port,
            &format!("GET /api/file/{id}?k={key} HTTP/1.1\r\nHost: x\r\n{close}\r\n"),
            b"",
        );
        assert_eq!(status, 200);
        assert_eq!(file, b"from the pc");
        let _ = std::fs::remove_file(&shared);

        // After stopping, the old link no longer works.
        stop_session(&handle);
        assert!(!snapshot().active);
        let after = try_request(
            port,
            &format!("GET /?k={key} HTTP/1.1\r\nHost: x\r\n{close}\r\n"),
            b"",
        );
        assert!(!matches!(after, Ok((200, _))));
    }

    #[test]
    fn keys_are_random_and_url_safe() {
        let a = random_key().unwrap();
        let b = random_key().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn qr_is_an_svg_data_url() {
        let qr = qr_data_url("http://192.168.1.5:50000/?k=abc").unwrap();
        assert!(qr.starts_with("data:image/svg+xml;base64,"));
    }

    #[test]
    fn unique_names_never_replace_existing_files() {
        let dir = std::env::temp_dir().join(format!("notcher-drop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, _) = create_unique(&dir, "photo.jpg").unwrap();
        let (b, _) = create_unique(&dir, "photo.jpg").unwrap();
        assert_eq!(a.file_name().unwrap(), "photo.jpg");
        assert_eq!(b.file_name().unwrap(), "photo (2).jpg");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
