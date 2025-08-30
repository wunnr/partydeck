mod app;
mod game;
mod handler;
mod input;
mod instance;
mod launch;
mod monitor;
mod paths;
mod util;

use crate::app::*;
use crate::monitor::*;
use crate::paths::PATH_PARTY;
use crate::util::*;

use eframe::UserEvent;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();

    if std::env::args().any(|arg| arg == "--help") {
        println!("{}", USAGE_TEXT);
        std::process::exit(0);
    }

    if std::env::args().any(|arg| arg == "--kwin") {
        let args: Vec<String> = std::env::args().filter(|arg| arg != "--kwin").collect();

        let (w, h) = get_screen_resolution();
        let mut cmd = std::process::Command::new("kwin_wayland");

        cmd.arg("--xwayland");
        cmd.arg("--width");
        cmd.arg(w.to_string());
        cmd.arg("--height");
        cmd.arg(h.to_string());
        cmd.arg("--exit-with-session");
        let args_string = args
            .iter()
            .map(|arg| format!("\"{}\"", arg))
            .collect::<Vec<String>>()
            .join(" ");
        cmd.arg(args_string);

        println!("[PARTYDECK] Launching kwin session: {:?}", cmd);

        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("Failed to start kwin_wayland: {}", e);
                std::process::exit(1);
            }
        }
    }

    let mut exec = String::new();
    let mut execargs = String::new();
    if let Some(exec_index) = args.iter().position(|arg| arg == "--exec") {
        if let Some(next_arg) = args.get(exec_index + 1) {
            exec = next_arg.clone();
        } else {
            eprintln!("{}", USAGE_TEXT);
            std::process::exit(1);
        }
    }
    if let Some(execargs_index) = args.iter().position(|arg| arg == "--args") {
        if let Some(next_arg) = args.get(execargs_index + 1) {
            execargs = next_arg.clone();
        } else {
            eprintln!("{}", USAGE_TEXT);
            std::process::exit(1);
        }
    }

    let fullscreen = std::env::args().any(|arg| arg == "--fullscreen");

    std::fs::create_dir_all(PATH_PARTY.join("gamesyms"))
        .expect("Failed to create gamesyms directory");
    std::fs::create_dir_all(PATH_PARTY.join("handlers"))
        .expect("Failed to create handlers directory");
    std::fs::create_dir_all(PATH_PARTY.join("profiles"))
        .expect("Failed to create profiles directory");

    remove_guest_profiles().unwrap();

    if PATH_PARTY.join("tmp").exists() {
        std::fs::remove_dir_all(PATH_PARTY.join("tmp")).unwrap();
    }

    let (_, scrheight) = get_screen_resolution();

    let scale = match fullscreen {
        true => scrheight as f32 / 560.0,
        false => 1.3,
    };

    let light = !exec.is_empty();

    let win_width = match light {
        true => 900.0,
        false => 1080.0,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([win_width, 540.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(fullscreen)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../res/icon.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    let mut eventloop = EventLoop::<UserEvent>::with_user_event().build()?;
    eventloop.set_control_flow(ControlFlow::Poll);

    let monitors = get_monitors_sdl();

    println!("Monitors detected: {}", monitors.len());
    for monitor in &monitors {
        println!(
            "Monitor {}: {}x{}",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }

    println!("\n[PARTYDECK] starting...\n");

    let mut party_winit_app = eframe::create_native(
        "PartyDeck",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(scale);
            Ok(match light {
                true => {
                    Box::<LightPartyApp>::new(LightPartyApp::new(exec, execargs, monitors.clone()))
                }
                false => Box::<PartyApp>::new(PartyApp::new(monitors.clone())),
            })
        }),
        &eventloop,
    );

    eventloop.run_app_on_demand(&mut party_winit_app)?;

    Ok(())
}

static USAGE_TEXT: &str = r#"
{}
Usage: partydeck [OPTIONS]

Options:
    --exec <executable>   Execute the specified executable in splitscreen. If this isn't specified, PartyDeck will launch in the regular GUI mode.
    --args [args]         Specify arguments for the executable to be launched with. Must be quoted if containing spaces.
    --fullscreen          Start the GUI in fullscreen mode
    --kwin                Launch PartyDeck inside of a KWin session
"#;
