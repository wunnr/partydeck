mod app;
mod cli;
mod game;
mod handler;
mod input;
mod instance;
mod launch;
mod monitor;
mod paths;
mod util;

use crate::app::*;
use crate::cli::{parse_args, LaunchMode};
use crate::monitor::*;
use crate::paths::PATH_PARTY;
use crate::util::*;

fn main() -> eframe::Result {
    // Our sdl/multimonitor stuff essentially depends on us running through x11.
    unsafe {
        std::env::set_var("SDL_VIDEODRIVER", "x11");
    }

    let monitors = get_monitors_sdl();

    println!("[partydeck] Monitors detected:");
    for monitor in &monitors {
        println!(
            "[partydeck] {} ({}x{})",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }

    let cli_args = parse_args();

    if cli_args.kwin {
        let (w, h) = (monitors[0].width(), monitors[0].height());
        let mut cmd = std::process::Command::new("kwin_wayland");

        cmd.arg("--xwayland");
        cmd.arg("--width");
        cmd.arg(w.to_string());
        cmd.arg("--height");
        cmd.arg(h.to_string());
        cmd.arg("--exit-with-session");
        
        let args: Vec<String> = std::env::args()
            .filter(|arg| arg != "--kwin")
            .collect();
        let args_string = args
            .iter()
            .map(|arg| format!("\"{}\"", arg))
            .collect::<Vec<String>>()
            .join(" ");
        cmd.arg(args_string);

        println!("[partydeck] Launching kwin session: {:?}", cmd);

        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("[partydeck] Failed to start kwin_wayland: {}", e);
                std::process::exit(1);
            }
        }
    }

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

    let scrheight = monitors[0].height();
    let scale = match cli_args.fullscreen {
        true => scrheight as f32 / 560.0,
        false => 1.3,
    };

    let light = !matches!(cli_args.mode, LaunchMode::Gui);
    
    let win_width = match light {
        true => 900.0,
        false => 1080.0,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([win_width, 540.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(cli_args.fullscreen)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../res/icon.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    println!("[partydeck] Starting eframe app...\n");

    eframe::run_native(
        "PartyDeck",
        options,
        Box::new(move |cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(scale);
            
            Ok(match cli_args.mode {
                LaunchMode::Gui => {
                    println!("[partydeck] Starting in GUI mode");
                    Box::<PartyApp>::new(PartyApp::new(monitors.clone()))
                }
                LaunchMode::Handler(_) | LaunchMode::Executable(_, _) => {
                    println!("[partydeck] Starting in CLI mode");
                    match LightPartyApp::from_cli_args(cli_args, monitors.clone()) {
                        Ok(app) => Box::new(app),
                        Err(e) => {
                            eprintln!("[partydeck] Failed to create app: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            })
        }),
    )
}