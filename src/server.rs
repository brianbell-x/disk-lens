use crate::actions;
use crate::cache;
use crate::orphans::{self, OrphanReport};
use crate::scanner::{self, DriveInfo, ScanTree};
use crate::watcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::time::Duration;

const INDEX_HTML: &[u8] = include_bytes!("../assets/index.html");
const TOKENS_CSS: &[u8] = include_bytes!("../assets/tokens.css");
const STYLES_CSS: &[u8] = include_bytes!("../assets/styles.css");
const APP_JS: &[u8] = include_bytes!("../assets/app.js");
const FONT_GEIST: &[u8] = include_bytes!("../assets/Geist-Variable.woff2");
const FONT_GEIST_MONO: &[u8] = include_bytes!("../assets/GeistMono-Variable.woff2");
const FAVICON_ICO: &[u8] = include_bytes!("../assets/app.ico");

const NOT_FOUND_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Not found · Disk Lens</title>
  <link rel="stylesheet" href="/tokens.css">
  <link rel="stylesheet" href="/styles.css">
</head>
<body>
  <main class="app-shell">
    <section class="tree-card">
      <div class="empty-state">
        <p>Page not found.</p>
        <p style="margin-top: 12px"><a href="/" style="color: var(--text-primary)">Return to Disk Lens</a></p>
      </div>
    </section>
  </main>
</body>
</html>"#;

pub struct AppState {
    pub active_drive: RwLock<String>,
    pub tree: RwLock<Option<Arc<ScanTree>>>,
    pub orphans: RwLock<Option<Arc<OrphanReport>>>,
    pub dirty: AtomicBool,
    pub scanning: AtomicBool,
    pub cancel_scan: Mutex<Option<Arc<AtomicBool>>>,
    pub watcher_stop: Mutex<Option<Arc<AtomicBool>>>,
    pub live_bytes: AtomicU64,
    pub live_files: AtomicU64,
    pub live_dirs: AtomicU64,
    pub subscribers: Mutex<Vec<mpsc::Sender<String>>>,
}

impl AppState {
    pub fn new(initial_drive: String) -> Arc<Self> {
        Arc::new(Self {
            active_drive: RwLock::new(initial_drive),
            tree: RwLock::new(None),
            orphans: RwLock::new(None),
            dirty: AtomicBool::new(false),
            scanning: AtomicBool::new(false),
            cancel_scan: Mutex::new(None),
            watcher_stop: Mutex::new(None),
            live_bytes: AtomicU64::new(0),
            live_files: AtomicU64::new(0),
            live_dirs: AtomicU64::new(0),
            subscribers: Mutex::new(Vec::new()),
        })
    }

    pub fn recompute_orphans(&self) {
        let tree_opt = self.tree.read().unwrap().clone();
        let report = tree_opt.map(|t| Arc::new(orphans::detect_orphans(&t)));
        *self.orphans.write().unwrap() = report;
    }

    pub fn flush_if_dirty(&self) {
        if self.dirty.swap(false, Ordering::AcqRel) {
            if let Some(tree) = self.tree.read().unwrap().clone() {
                let _ = cache::save_snapshot(&tree);
            }
        }
    }

    fn dirty_clear_after_save(&self, drive_root: &str, arc_tree: &Arc<ScanTree>) {
        let cur = self.active_drive.read().unwrap().clone();
        if cur.eq_ignore_ascii_case(drive_root) {
            {
                let mut w = self.tree.write().unwrap();
                *w = Some(Arc::clone(arc_tree));
                self.dirty.store(false, Ordering::Release);
            }
            self.recompute_orphans();
        }
    }

    pub fn broadcast(&self, event_name: &str, json_payload: &str) {
        let msg = format!("event: {event_name}\ndata: {json_payload}\n\n");
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| tx.send(msg.clone()).is_ok());
    }

    pub fn ensure_watcher(self: &Arc<Self>, drive_root: &str) {
        let mut guard = self.watcher_stop.lock().unwrap();
        if let Some(old) = guard.take() {
            old.store(true, Ordering::Relaxed);
        }
        let stop = Arc::new(AtomicBool::new(false));
        *guard = Some(Arc::clone(&stop));
        drop(guard);

        let state_weak = Arc::downgrade(self);
        let watched_drive = drive_root.to_string();
        watcher::spawn_drive_watcher(drive_root.to_string(), stop, move |changed_batch| {
            let Some(st) = state_weak.upgrade() else {
                return;
            };
            if st.scanning.load(Ordering::Relaxed) {
                return;
            }
            let current_drive = st.active_drive.read().unwrap().clone();
            if !current_drive.eq_ignore_ascii_case(&watched_drive) {
                return;
            }

            let changed = {
                let mut w = st.tree.write().unwrap();
                let Some(arc_ref) = w.as_mut() else {
                    return;
                };
                Arc::make_mut(arc_ref).refresh_changed_paths(&changed_batch)
            };

            if changed {
                st.dirty.store(true, Ordering::Release);
                st.recompute_orphans();
                st.broadcast("updated", r#"{"reason":"watcher"}"#);
            }
        });
    }

    pub fn start_scan(self: &Arc<Self>, drive_root: String) {
        {
            let mut cancel_guard = self.cancel_scan.lock().unwrap();
            if let Some(prev) = cancel_guard.take() {
                prev.store(true, Ordering::Relaxed);
            }
            let token = Arc::new(AtomicBool::new(false));
            *cancel_guard = Some(Arc::clone(&token));

            self.scanning.store(true, Ordering::Release);
            self.live_bytes.store(0, Ordering::Relaxed);
            self.live_files.store(0, Ordering::Relaxed);
            self.live_dirs.store(0, Ordering::Relaxed);

            let st_prog = Arc::clone(self);
            let tok_prog = Arc::clone(&token);
            std::thread::spawn(move || {
                while !tok_prog.load(Ordering::Relaxed) && st_prog.scanning.load(Ordering::Acquire)
                {
                    std::thread::sleep(Duration::from_millis(250));
                    if tok_prog.load(Ordering::Relaxed) || !st_prog.scanning.load(Ordering::Acquire)
                    {
                        break;
                    }
                    let b = st_prog.live_bytes.load(Ordering::Relaxed);
                    let d = st_prog.live_dirs.load(Ordering::Relaxed);
                    let f = st_prog.live_files.load(Ordering::Relaxed);
                    let payload = format!(
                        r#"{{"scanning":true,"scanned_bytes":{b},"scanned_dirs":{d},"scanned_files":{f}}}"#
                    );
                    st_prog.broadcast("progress", &payload);
                }
            });

            let st_scan = Arc::clone(self);
            std::thread::spawn(move || {
                let result = scanner::scan_root(
                    &drive_root,
                    Some(&st_scan.live_bytes),
                    Some(&st_scan.live_files),
                    Some(&st_scan.live_dirs),
                    Some(&token),
                );
                if token.load(Ordering::Relaxed) {
                    return;
                }
                let arc_tree = Arc::new(result);
                let _ = cache::save_snapshot(&arc_tree);
                st_scan.dirty_clear_after_save(&drive_root, &arc_tree);
                st_scan.scanning.store(false, Ordering::Release);
                st_scan.broadcast("updated", r#"{"reason":"scan_complete"}"#);
            });
        }
    }

    pub fn initialize_startup(self: &Arc<Self>) {
        let st = Arc::clone(self);
        std::thread::spawn(move || {
            let drive = st.active_drive.read().unwrap().clone();
            st.ensure_watcher(&drive);

            if let Some((cached_tree, saved_at)) = cache::load_snapshot(&drive) {
                {
                    let mut w = st.tree.write().unwrap();
                    *w = Some(Arc::new(cached_tree));
                }
                st.recompute_orphans();
                st.broadcast("updated", r#"{"reason":"cache_loaded"}"#);

                // Refresh silently in background if snapshot is older than 15 minutes
                let age = cache::now_unix_secs().saturating_sub(saved_at);
                if age > 900 {
                    st.start_scan(drive);
                }
            } else {
                st.start_scan(drive);
            }
        });
    }
}

#[derive(Serialize)]
struct StateResponse {
    active_drive: String,
    drives: Vec<DriveInfo>,
    ready: bool,
    scanning: bool,
    scanned_bytes: u64,
    scanned_dirs: u64,
    scanned_files: u64,
    total_alloc_bytes: u64,
    elapsed_ms: u128,
    orphan_count: usize,
    orphan_total_bytes: u64,
}

#[derive(Serialize, Clone)]
struct UiRow {
    key: String,
    name: String,
    path: String,
    vscode_path: String,
    size: u64,
    kind: &'static str,
    expandable: bool,
    select_in_explorer: bool,
    access_denied: bool,
    can_delete: bool,
    leftover_reason: Option<String>,
    location_hint: Option<String>,
}

#[derive(Serialize)]
struct ChildrenResponse {
    rows: Vec<UiRow>,
    has_more: bool,
    remaining_count: usize,
    remaining_size: u64,
    next_offset: usize,
}

#[derive(Deserialize)]
struct OpenExplorerReq {
    path: String,
    #[serde(default)]
    select_file: bool,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct OpenVscodeReq {
    path: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct DeleteReq {
    path: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct RescanReq {
    drive: Option<String>,
    #[serde(default)]
    force: bool,
}

pub fn start_http_server(state: Arc<AppState>, bind_port: u16) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", bind_port))
        .map_err(|e| format!("Failed binding local server: {e}"))?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| format!("Failed reading local addr: {e}"))?
        .port();

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let st = Arc::clone(&state);
            std::thread::spawn(move || {
                let _ = handle_connection(stream, st);
            });
        }
    });

    Ok(actual_port)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_query(q: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(percent_decode(k), percent_decode(v));
        } else {
            map.insert(percent_decode(pair), String::new());
        }
    }
    map
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    cache_ctrl: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {cache_ctrl}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn write_json<T: Serialize>(stream: &mut TcpStream, status: &str, val: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(val).unwrap_or_else(|_| b"{}".to_vec());
    write_response(
        stream,
        status,
        "application/json; charset=utf-8",
        "no-store",
        &body,
    )
}

fn build_file_rows(node: &scanner::DirNode, dir_path: &str) -> Vec<UiRow> {
    let base = dir_path.trim_end_matches('\\');
    let mut rows = Vec::with_capacity(node.top_files.len() + 1);
    let mut shown_sum = 0u64;

    for f in &node.top_files {
        shown_sum = shown_sum.saturating_add(f.size);
        let full_file = format!("{base}\\{}", f.name);
        let can_delete = actions::is_deletable_path(&full_file);
        rows.push(UiRow {
            key: format!("file:{full_file}"),
            name: f.name.to_string(),
            path: full_file,
            vscode_path: dir_path.to_string(),
            size: f.size,
            kind: "file",
            expandable: false,
            select_in_explorer: true,
            access_denied: false,
            can_delete,
            leftover_reason: None,
            location_hint: None,
        });
    }

    let shown_count = node.top_files.len() as u32;
    if node.direct_file_count > shown_count && node.direct_size > shown_sum {
        let rem_count = node.direct_file_count - shown_count;
        let rem_bytes = node.direct_size - shown_sum;
        let label = if rem_count == 1 {
            "1 smaller file".to_string()
        } else {
            format!("{rem_count} smaller files")
        };
        rows.push(UiRow {
            key: format!("summary:{dir_path}"),
            name: label,
            path: dir_path.to_string(),
            vscode_path: dir_path.to_string(),
            size: rem_bytes,
            kind: "summary",
            expandable: false,
            select_in_explorer: false,
            access_denied: false,
            can_delete: false,
            leftover_reason: None,
            location_hint: None,
        });
    }

    rows
}

fn paginate_rows(all_rows: Vec<UiRow>, offset: usize, limit: usize) -> ChildrenResponse {
    let start = offset.min(all_rows.len());
    let end = start.saturating_add(limit).min(all_rows.len());
    let has_more = end < all_rows.len();
    let remaining_count = all_rows.len().saturating_sub(end);
    let remaining_size: u64 = all_rows[end..].iter().map(|r| r.size).sum();
    let rows = all_rows.into_iter().skip(start).take(end - start).collect();

    ChildrenResponse {
        rows,
        has_more,
        remaining_count,
        remaining_size,
        next_offset: end,
    }
}

fn handle_connection(mut stream: TcpStream, state: Arc<AppState>) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let cloned = stream.try_clone()?;
    let mut reader = BufReader::new(cloned);

    let mut req_line = String::new();
    if reader.read_line(&mut req_line)? == 0 {
        return Ok(());
    }
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let raw_target = parts.next().unwrap_or("/");

    let mut content_len = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((h, v)) = line.split_once(':') {
            if h.trim().eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse::<usize>().unwrap_or(0).min(1_048_576);
            }
        }
    }

    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        reader.read_exact(&mut body)?;
    }

    let (route, query_str) = match raw_target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (raw_target, ""),
    };

    match (method, route) {
        ("GET", "/") => write_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            "no-cache",
            INDEX_HTML,
        ),
        ("GET", "/tokens.css") => write_response(
            &mut stream,
            "200 OK",
            "text/css; charset=utf-8",
            "no-cache",
            TOKENS_CSS,
        ),
        ("GET", "/styles.css") => write_response(
            &mut stream,
            "200 OK",
            "text/css; charset=utf-8",
            "no-cache",
            STYLES_CSS,
        ),
        ("GET", "/app.js") => write_response(
            &mut stream,
            "200 OK",
            "application/javascript; charset=utf-8",
            "no-cache",
            APP_JS,
        ),
        ("GET", "/fonts/Geist-Variable.woff2") => write_response(
            &mut stream,
            "200 OK",
            "font/woff2",
            "public, max-age=31536000, immutable",
            FONT_GEIST,
        ),
        ("GET", "/fonts/GeistMono-Variable.woff2") => write_response(
            &mut stream,
            "200 OK",
            "font/woff2",
            "public, max-age=31536000, immutable",
            FONT_GEIST_MONO,
        ),
        ("GET", "/favicon.ico") => write_response(
            &mut stream,
            "200 OK",
            "image/x-icon",
            "public, max-age=86400",
            FAVICON_ICO,
        ),
        ("GET", "/api/state") => {
            let active_drive = state.active_drive.read().unwrap().clone();
            let drives = scanner::list_drives();
            let tree_opt = state.tree.read().unwrap().clone();
            let orphans_opt = state.orphans.read().unwrap().clone();
            let scanning = state.scanning.load(Ordering::Acquire);
            let resp = StateResponse {
                active_drive,
                drives,
                ready: tree_opt.is_some(),
                scanning,
                scanned_bytes: state.live_bytes.load(Ordering::Relaxed),
                scanned_dirs: state.live_dirs.load(Ordering::Relaxed),
                scanned_files: state.live_files.load(Ordering::Relaxed),
                total_alloc_bytes: tree_opt
                    .as_ref()
                    .map_or(0, |t| t.stats.total_alloc_bytes),
                elapsed_ms: tree_opt.as_ref().map_or(0, |t| t.stats.elapsed_ms),
                orphan_count: orphans_opt.as_ref().map_or(0, |o| o.sorted_list.len()),
                orphan_total_bytes: orphans_opt.as_ref().map_or(0, |o| o.total_bytes),
            };
            write_json(&mut stream, "200 OK", &resp)
        }
        ("GET", "/api/children") => {
            let params = parse_query(query_str);
            let offset = params
                .get("offset")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let limit = params
                .get("limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(200)
                .clamp(1, 1000);

            let tree_opt = state.tree.read().unwrap().clone();
            let Some(tree) = tree_opt else {
                return write_json(
                    &mut stream,
                    "200 OK",
                    &paginate_rows(Vec::new(), 0, limit),
                );
            };
            let orphans_opt = state.orphans.read().unwrap().clone();

            let default_key = format!("dir:{}", tree.root_path);
            let key = params.get("key").unwrap_or(&default_key);

            if key == "orphans" {
                let mut rows = Vec::new();
                if let Some(ref rep) = orphans_opt {
                    rows.reserve(rep.sorted_list.len());
                    for m in &rep.sorted_list {
                        let Some(c) = tree.nodes.get(m.node_id as usize) else {
                            continue;
                        };
                        if c.total_size == 0 {
                            continue;
                        }
                        let expandable = !c.children.is_empty() || !c.top_files.is_empty();
                        let can_delete = !c.access_denied && actions::is_deletable_path(&m.path);
                        rows.push(UiRow {
                            key: format!("dir:{}", m.path),
                            name: m.name.clone(),
                            path: m.path.clone(),
                            vscode_path: m.path.clone(),
                            size: c.total_size,
                            kind: "folder",
                            expandable,
                            select_in_explorer: false,
                            access_denied: c.access_denied,
                            can_delete,
                            leftover_reason: Some(m.reason.clone()),
                            location_hint: Some(m.location_group.to_string()),
                        });
                    }
                }
                return write_json(&mut stream, "200 OK", &paginate_rows(rows, offset, limit));
            }

            if let Some(dir_path) = key.strip_prefix("files:") {
                let Some(nid) = tree.find_node_by_path(dir_path) else {
                    return write_json(
                        &mut stream,
                        "404 Not Found",
                        &serde_json::json!({"error": "Folder not found"}),
                    );
                };
                let canonical = tree.node_path(nid).unwrap_or_else(|| dir_path.to_string());
                let rows = build_file_rows(&tree.nodes[nid as usize], &canonical);
                return write_json(&mut stream, "200 OK", &paginate_rows(rows, offset, limit));
            }

            let target_path = key.strip_prefix("dir:").unwrap_or(key);
            let Some(nid) = tree.find_node_by_path(target_path) else {
                return write_json(
                    &mut stream,
                    "404 Not Found",
                    &serde_json::json!({"error": "Folder not found"}),
                );
            };

            let node = &tree.nodes[nid as usize];
            let canonical_parent = tree
                .node_path(nid)
                .unwrap_or_else(|| target_path.to_string());

            if node.children.is_empty() {
                let rows = build_file_rows(node, &canonical_parent);
                return write_json(&mut stream, "200 OK", &paginate_rows(rows, offset, limit));
            }

            let parent_base = canonical_parent.trim_end_matches('\\');
            let mut rows = Vec::with_capacity(node.children.len() + 1);

            for &cid in &node.children {
                let c = &tree.nodes[cid as usize];
                let full = format!("{parent_base}\\{}", c.name);
                let expandable = !c.children.is_empty() || !c.top_files.is_empty();
                let leftover_reason = orphans_opt.as_ref().and_then(|rep| {
                    rep.by_path_lower
                        .get(&full.to_ascii_lowercase())
                        .map(|m| m.reason.clone())
                });
                let can_delete = !c.access_denied && actions::is_deletable_path(&full);
                rows.push(UiRow {
                    key: format!("dir:{full}"),
                    name: c.name.to_string(),
                    path: full.clone(),
                    vscode_path: full,
                    size: c.total_size,
                    kind: "folder",
                    expandable,
                    select_in_explorer: false,
                    access_denied: c.access_denied,
                    can_delete,
                    leftover_reason,
                    location_hint: None,
                });
            }

            if node.direct_size > 0 {
                let label = if node.direct_file_count == 1 {
                    "1 file in folder".to_string()
                } else {
                    format!("{} files in folder", node.direct_file_count)
                };
                rows.push(UiRow {
                    key: format!("files:{canonical_parent}"),
                    name: label,
                    path: canonical_parent.clone(),
                    vscode_path: canonical_parent.clone(),
                    size: node.direct_size,
                    kind: "files_group",
                    expandable: !node.top_files.is_empty(),
                    select_in_explorer: false,
                    access_denied: false,
                    can_delete: false,
                    leftover_reason: None,
                    location_hint: None,
                });
                rows.sort_unstable_by(|a, b| b.size.cmp(&a.size));
            }

            write_json(&mut stream, "200 OK", &paginate_rows(rows, offset, limit))
        }
        ("POST", "/api/open-explorer") => {
            let Ok(req) = serde_json::from_slice::<OpenExplorerReq>(&body) else {
                return write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": "Invalid request payload."}),
                );
            };
            if req.dry_run {
                let exists = std::path::Path::new(&req.path).exists();
                return write_json(
                    &mut stream,
                    "200 OK",
                    &serde_json::json!({"ok": exists, "path": req.path}),
                );
            }
            match actions::open_in_explorer(&req.path, req.select_file) {
                Ok(()) => write_json(&mut stream, "200 OK", &serde_json::json!({"ok": true})),
                Err(msg) => write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": msg}),
                ),
            }
        }
        ("POST", "/api/open-vscode") => {
            let Ok(req) = serde_json::from_slice::<OpenVscodeReq>(&body) else {
                return write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": "Invalid request payload."}),
                );
            };
            if req.dry_run {
                let exists = std::path::Path::new(&req.path).exists();
                return write_json(
                    &mut stream,
                    "200 OK",
                    &serde_json::json!({"ok": exists, "path": req.path}),
                );
            }
            match actions::open_in_vscode(&req.path) {
                Ok(()) => write_json(&mut stream, "200 OK", &serde_json::json!({"ok": true})),
                Err(msg) => write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": msg}),
                ),
            }
        }
        ("POST", "/api/delete") => {
            let Ok(req) = serde_json::from_slice::<DeleteReq>(&body) else {
                return write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": "Invalid request payload."}),
                );
            };
            if req.dry_run {
                let ok =
                    actions::is_deletable_path(&req.path) && std::path::Path::new(&req.path).exists();
                return write_json(
                    &mut stream,
                    "200 OK",
                    &serde_json::json!({"ok": ok, "path": req.path}),
                );
            }
            match actions::move_to_recycle_bin(&req.path) {
                Ok(()) => {
                    {
                        let mut w = state.tree.write().unwrap();
                        if let Some(arc_ref) = w.as_mut() {
                            let root_trim = arc_ref.root_path.trim_end_matches('\\');
                            let norm = req.path.replace('/', "\\");
                            let rel = if norm.len() >= root_trim.len()
                                && norm[..root_trim.len()].eq_ignore_ascii_case(root_trim)
                            {
                                norm[root_trim.len()..].trim_start_matches('\\').to_string()
                            } else {
                                norm
                            };
                            Arc::make_mut(arc_ref).refresh_changed_paths(&[rel]);
                        }
                    }
                    state.dirty.store(true, Ordering::Release);
                    state.recompute_orphans();
                    state.broadcast("updated", r#"{"reason":"deleted"}"#);
                    write_json(&mut stream, "200 OK", &serde_json::json!({"ok": true}))
                }
                Err(msg) => write_json(
                    &mut stream,
                    "400 Bad Request",
                    &serde_json::json!({"error": msg}),
                ),
            }
        }
        ("POST", "/api/rescan") => {
            let req = serde_json::from_slice::<RescanReq>(&body).unwrap_or(RescanReq {
                drive: None,
                force: true,
            });
            let target_drive = req
                .drive
                .unwrap_or_else(|| state.active_drive.read().unwrap().clone());

            let switched = {
                let mut cur = state.active_drive.write().unwrap();
                let diff = !cur.eq_ignore_ascii_case(&target_drive);
                if diff {
                    *cur = target_drive.clone();
                }
                diff
            };

            if switched {
                state.ensure_watcher(&target_drive);
                if let Some((cached, saved_at)) = cache::load_snapshot(&target_drive) {
                    *state.tree.write().unwrap() = Some(Arc::new(cached));
                    state.recompute_orphans();
                    state.broadcast("updated", r#"{"reason":"drive_switched"}"#);
                    let age = cache::now_unix_secs().saturating_sub(saved_at);
                    if req.force || age > 900 {
                        state.start_scan(target_drive);
                    }
                    return write_json(&mut stream, "200 OK", &serde_json::json!({"ok": true}));
                } else {
                    *state.tree.write().unwrap() = None;
                    *state.orphans.write().unwrap() = None;
                }
            }

            state.start_scan(target_drive);
            write_json(&mut stream, "200 OK", &serde_json::json!({"ok": true}))
        }
        ("GET", "/api/events") => {
            let _ = stream.set_read_timeout(None);
            let _ = stream.set_nodelay(true);
            stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n: connected\n\n",
            )?;
            stream.flush()?;

            let (tx, rx) = mpsc::channel::<String>();
            state.subscribers.lock().unwrap().push(tx);

            loop {
                match rx.recv_timeout(Duration::from_secs(15)) {
                    Ok(msg) => {
                        if stream.write_all(msg.as_bytes()).is_err() || stream.flush().is_err() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if stream.write_all(b": keepalive\n\n").is_err() || stream.flush().is_err()
                        {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            Ok(())
        }
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            "no-store",
            NOT_FOUND_HTML.as_bytes(),
        ),
    }
}
