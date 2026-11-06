//! MaterialCraft desktop app
//! Usage: `materialcraft [package.sbs] [--control <port>] [--empty] [--version]`
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod control_server;
mod start_failure;

use materialcraft_engine::Session;
use materialcraft_ui_egui::MaterialCraftApp;

const ICON_PNG: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.materialcraft.png");

fn version() -> String {
    let sha = env!("MATERIALCRAFT_BUILD_SHA");
    let short = sha.get(..7).unwrap_or(sha);
    let date = env!("MATERIALCRAFT_BUILD_DATE");
    match (short.is_empty(), date.is_empty()) {
        (true, true) => format!("MaterialCraft {}", materialcraft_engine::VERSION),
        (true, false) => format!("MaterialCraft {} ({date})", materialcraft_engine::VERSION),
        _ => format!("MaterialCraft {} ({short} {date})", materialcraft_engine::VERSION),
    }
}

struct Args {
    file: Option<String>,
    control: Option<u16>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args { file: None, control: None };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--version" | "-V" => {
                println!("{}", version());
                return Ok(None);
            }
            "--help" | "-h" => {
                println!("{}\n\nUsage: materialcraft [package.sbs] [--control <port>] [--empty]", version());
                return Ok(None);
            }
            "--control" => {
                let p = it.next().ok_or("--control needs a port")?;
                args.control = Some(p.parse().map_err(|_| format!("invalid port `{p}`"))?);
            }
            // Kept for scripts written before empty startup became the default
            "--empty" => {}
            f if !f.starts_with('-') => args.file = Some(f.to_string()),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    Ok(Some(args))
}

fn initial_session(args: &Args) -> Session {
    let mut s = materialcraft_engine::new_session();
    if let Some(f) = &args.file {
        match materialcraft_engine::file_cmds::open_file(&mut s, f).map_err(|e| e.to_string()) {
            Ok(()) => {}
            Err(e) => eprintln!("materialcraft: could not open {f}: {e}"),
        }
    }
    s
}

fn icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn main() {
    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => return,
        Err(e) => {
            eprintln!("materialcraft: {e}");
            std::process::exit(2);
        }
    };

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("materialcraft: internal error: {info}");
        prev(info);
    }));
    let session = initial_session(&args);
    let mut viewport = egui::ViewportBuilder::default().with_title("MaterialCraft").with_inner_size([1600.0, 960.0]).with_min_inner_size([960.0, 600.0]);
    if let Some(i) = icon() {
        viewport = viewport.with_icon(i);
    }
    // Scripted runs (agents, tests) set MATERIALCRAFT_NO_PREFS so they neither read nor overwrite the user's saved layout and settings: state goes to a throwaway file
    let persistence_path =
        std::env::var_os("MATERIALCRAFT_NO_PREFS").map(|_| std::env::temp_dir().join(format!("materialcraft-noprefs-{}.ron", std::process::id())));
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Wgpu, persistence_path, ..Default::default() };
    let control = args.control;
    let r = eframe::run_native(
        "MaterialCraft",
        options,
        Box::new(move |cc| {
            let mut app = MaterialCraftApp::new(cc, session);
            if let Some(port) = control {
                app.control = Some(control_server::start(port, cc.egui_ctx.clone()));
            }
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = r {
        eprintln!("materialcraft: {e}");
        // Say why instead of exiting silently (`start_failure`).
        start_failure::report(&e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_without_a_file_has_one_empty_graph() {
        let s = initial_session(&Args { file: None, control: None });
        assert_eq!(s.pkg.name, "Untitled package");
        assert_eq!(s.pkg.graphs.len(), 1);
        let g = s.graph().unwrap();
        assert_eq!(g.name, "New_graph");
        assert!(g.nodes.is_empty());
        assert!(g.connections.is_empty());
        assert!(s.path.is_none());
        assert!(!s.modified);
        assert!(s.history().is_empty());
    }

    #[test]
    fn startup_opens_the_requested_package() {
        let path = std::env::temp_dir().join(format!("mc-startup-{}.sbs", std::process::id()));
        let original = materialcraft_engine::sample_session();
        std::fs::write(&path, materialcraft_engine::file_cmds::save_bytes(&original).unwrap()).unwrap();
        let s = initial_session(&Args { file: Some(path.to_string_lossy().into_owned()), control: None });
        assert_eq!(s.pkg.graphs.len(), original.pkg.graphs.len());
        assert_eq!(s.pkg.graphs[0].name, original.pkg.graphs[0].name);
        assert_eq!(materialcraft_engine::file_cmds::save_bytes(&s).unwrap(), materialcraft_engine::file_cmds::save_bytes(&original).unwrap());
        assert_eq!(s.path.as_deref(), path.to_str());
        assert!(!s.modified);
        std::fs::remove_file(path).unwrap();
    }
}
