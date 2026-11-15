#![windows_subsystem = "windows"]

mod actions;
mod cache;
mod orphans;
mod scanner;
mod server;
mod watcher;

use std::time::Instant;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::{Icon, Theme, WindowBuilder};
use wry::{WebContext, WebViewBuilder};

const ICON_RGBA_32: &[u8] = include_bytes!("../assets/icon_32x32.rgba");

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
}
const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;

fn default_drive() -> String {
    let drives = scanner::list_drives();
    if drives.iter().any(|d| d.root.eq_ignore_ascii_case("C:\\")) {
        "C:\\".to_string()
    } else {
        drives
            .into_iter()
            .next()
            .map(|d| d.root)
            .unwrap_or_else(|| "C:\\".to_string())
    }
}

fn run_warm_cache() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let drive = default_drive();
    println!("Scanning {drive} to warm binary snapshot cache...");
    let tree = scanner::scan_root(&drive, None, None, None, None);
    println!(
        "Scan completed in {} ms ({} dirs, {} files, {:.2} GB)",
        tree.stats.elapsed_ms,
        tree.stats.total_dirs,
        tree.stats.total_files,
        tree.stats.total_alloc_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    let t_save = Instant::now();
    cache::save_snapshot(&tree).expect("Failed saving snapshot");
    let save_ms = t_save.elapsed().as_millis();

    let t_load = Instant::now();
    let (loaded, _) = cache::load_snapshot(&drive).expect("Failed reloading snapshot");
    let load_ms = t_load.elapsed().as_millis();
    println!(
        "Snapshot saved in {save_ms} ms and verified reload of {} dirs in {load_ms} ms!",
        loaded.nodes.len()
    );

    let t_orph = Instant::now();
    let rep = orphans::detect_orphans(&loaded);
    let orph_ms = t_orph.elapsed().as_millis();
    println!(
        "Detected {} suspected leftover folders ({:.2} GB) in {orph_ms} ms:",
        rep.sorted_list.len(),
        rep.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    for (i, item) in rep.sorted_list.iter().enumerate() {
        println!(
            "  {:>2}. {:>9.2} MB  [{}]  {}  ->  {}",
            i + 1,
            item.size as f64 / (1024.0 * 1024.0),
            item.location_group,
            item.path,
            item.reason
        );
    }
}

fn run_headless_server(port: u16) {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let state = server::AppState::new(default_drive());
    state.initialize_startup();
    let actual = server::start_http_server(state, port).expect("Failed starting HTTP server");
    println!("Disk Lens server listening on http://127.0.0.1:{actual}");
    loop {
        std::thread::park();
    }
}

fn run_check_orphans() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let drive = default_drive();
    let (loaded, _) = cache::load_snapshot(&drive).expect("No cached snapshot yet; run --warm-cache first");
    let t_orph = Instant::now();
    let rep = orphans::detect_orphans(&loaded);
    let orph_ms = t_orph.elapsed().as_millis();
    println!(
        "Detected {} suspected leftover folders ({:.2} GB) in {orph_ms} ms:",
        rep.sorted_list.len(),
        rep.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    for (i, item) in rep.sorted_list.iter().enumerate() {
        println!(
            "  {:>2}. {:>9.2} MB  [{}]  {}  ->  {}",
            i + 1,
            item.size as f64 / (1024.0 * 1024.0),
            item.location_group,
            item.path,
            item.reason
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--warm-cache") {
        run_warm_cache();
        return;
    }
    if args.iter().any(|a| a == "--check-orphans") {
        run_check_orphans();
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--serve") {
        let port = args
            .get(pos + 1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(47821);
        run_headless_server(port);
        return;
    }

    let state = server::AppState::new(default_drive());
    let port = server::start_http_server(std::sync::Arc::clone(&state), 0)
        .expect("Could not bind local server");
    state.initialize_startup();

    let event_loop = EventLoop::new();
    let icon = Icon::from_rgba(ICON_RGBA_32.to_vec(), 32, 32).ok();

    let window = WindowBuilder::new()
        .with_title("Disk Lens")
        .with_inner_size(LogicalSize::new(780.0, 680.0))
        .with_min_inner_size(LogicalSize::new(420.0, 380.0))
        .with_theme(Some(Theme::Dark))
        .with_window_icon(icon)
        .build(&event_loop)
        .expect("Failed creating Disk Lens window");

    let mut web_context = WebContext::new(Some(cache::data_dir().join("webview")));
    let url = format!("http://127.0.0.1:{port}/");

    let _webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_url(&url)
        .with_background_color((24, 24, 24, 255))
        .build(&window)
        .expect("Failed initializing WebView2");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            state.flush_if_dirty();
            *control_flow = ControlFlow::Exit;
        }
    });
}
