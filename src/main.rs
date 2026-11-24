// Always a windowed program: no console window opens (also when started from Explorer).
#![windows_subsystem = "windows"]
//! NapkinUML: a small UML diagram editor.
//!
//! Options:
//!   --screenshot <file.png>  draw about 120 frames, save the window as PNG and exit
//!   --new                    start with an empty project (used by Window > New Window)
//!   --diagram <n>            open the n-th example diagram first (0-10)
//!   <file.umlp>              open a saved project
//!   --ai <request> --ai-out <out.umlp> [<file.umlp>]
//!                            ask the AI agent (as set in the AI Assistant) to draw or change
//!                            the first diagram, save the result and exit without a window;
//!                            the agent's reply goes to <out.umlp>.txt

mod ai;
mod app;
mod assoc;
mod conn_create;
mod emf;
mod mindlayout;
mod fonts;
mod i18n;
mod model;
mod palette;
mod offscreen;
mod print;
mod render;
mod seq;
mod settings;
mod update;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut empty = false;
    let mut open: Option<PathBuf> = None;
    let mut start_diagram: Option<usize> = None;
    let mut ai_request: Option<String> = None;
    let mut ai_out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => screenshot = args.next().map(PathBuf::from),
            "--new" => empty = true,
            "--ai" => ai_request = args.next(),
            "--ai-out" => ai_out = args.next().map(PathBuf::from),
            "--diagram" => start_diagram = args.next().and_then(|n| n.parse().ok()),
            other if !other.starts_with("--") => open = Some(PathBuf::from(other)),
            _ => {}
        }
    }

    if let (Some(req), Some(out)) = (ai_request, ai_out) {
        let s = settings::Settings::load();
        let msg = match ai::headless(&s, &req, open.as_deref(), &out) {
            Ok(m) => m,
            Err(e) => format!("ERROR: {e}"),
        };
        let _ = std::fs::write(out.with_extension("umlp.txt"), msg);
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NapkinUML")
        .with_inner_size([1360.0, 820.0])
        .with_icon(std::sync::Arc::new(app_icon()))
        .with_min_inner_size([800.0, 500.0]);
    if screenshot.is_some() {
        // never steal focus from the person while a check runs
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, run_and_return: false, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native("NapkinUML", options, Box::new(move |cc| Ok(Box::new(app::UmlApp::new(cc, screenshot, empty, open, start_diagram)))))
}

/// The window and taskbar icon, from the pictures in icons/ (the Mac picture on macOS).
fn app_icon() -> egui::IconData {
    #[cfg(target_os = "macos")]
    let bytes: &[u8] = include_bytes!("../icons/napkin-uml-1024.png");
    #[cfg(not(target_os = "macos"))]
    let bytes: &[u8] = include_bytes!("../icons/napkin-uml-256.png");
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(img) => {
            let img = img.to_rgba8();
            let (width, height) = img.dimensions();
            egui::IconData { rgba: img.into_raw(), width, height }
        }
        Err(_) => egui::IconData::default(),
    }
}
