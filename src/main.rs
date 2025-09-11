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

use crate::app::{PartyApp, LightPartyApp, load_cfg};
use crate::cli::{parse_args, list_all_devices, list_all_handlers, LaunchMode};
use crate::instance::build_instance_from_specs;
use crate::game::{Game, find_game_by_handler_uid, Executable};
use crate::input::{scan_input_devices};
use crate::monitor::get_monitors_sdl;
use crate::paths::PATH_PARTY;
use crate::util::{scan_profiles, remove_guest_profiles};
use std::path::PathBuf;

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
                    create_cli_app(cli_args, monitors.clone())
                }
            })
        }),
    )
}

fn create_cli_app(
    cli_args: cli::CliArgs, 
    monitors: Vec<crate::monitor::Monitor>
) -> Box<dyn eframe::App> {
    let game = match &cli_args.mode {
        LaunchMode::Handler(uid) => {
            match find_game_by_handler_uid(uid) {
                Some(g) => g,
                None => {
                    eprintln!("[partydeck] Error: Handler with UID '{}' not found", uid);
                    eprintln!("\nAvailable handlers:");
                    list_all_handlers();
                    std::process::exit(1);
                }
            }
        }
        LaunchMode::Executable(exec, args) => {
            Game::ExecRef(Executable::new(PathBuf::from(exec), args.clone()))
        }
        LaunchMode::Gui => {
            eprintln!("[partydeck] Error: GUI mode does not specify a game");
            std::process::exit(1);
        }
    };
    
    let cfg = load_cfg();
    let input_devices = scan_input_devices(&cfg.pad_filter_type);
    
    // List devices if no players specified and not auto launching
    if cli_args.players.is_empty() && !cli_args.auto_launch {
        println!("\nNo players specified. Available devices:");
        list_all_devices(&input_devices);
        println!("\nUse --player to specify players or see --help for usage");
    }
    
    // Build instances if players were specified
    if !cli_args.players.is_empty() {
        create_cli_app_with_players(game, cli_args, monitors, input_devices)
    } else {
        // No players specified - start in manual configuration mode
        let (exec, args) = match game {
            Game::ExecRef(ref e) => (e.path().to_string_lossy().to_string(), e.args.clone()),
            Game::HandlerRef(ref h) => (h.uid.clone(), String::new()),
        };
        Box::<LightPartyApp>::new(LightPartyApp::new(exec, args, monitors))
    }
}

fn create_cli_app_with_players(
    game: Game,
    cli_args: cli::CliArgs,
    monitors: Vec<crate::monitor::Monitor>,
    input_devices: Vec<crate::input::InputDevice>,
) -> Box<dyn eframe::App> {
    let profiles = scan_profiles(true);
    
    match build_instance_from_specs(&cli_args.players, &input_devices, &profiles) {
        Ok(instances) => {
            println!("[partydeck] Created {} instances from CLI", instances.len());
            for (i, instance) in instances.iter().enumerate() {
                println!(
                    "  Instance {}: Profile '{}', {} devices, monitor {}",
                    i + 1,
                    instance.profname,
                    instance.devices.len(),
                    instance.monitor
                );
            }
            
            Box::<LightPartyApp>::new(LightPartyApp::new_with_instances(
                game,
                instances,
                monitors,
                cli_args.auto_launch,
            ))
        }
        Err(e) => {
            eprintln!("[partydeck] Error building instances: {}", e);
            eprintln!("\nAvailable devices:");
            list_all_devices(&input_devices);
            eprintln!("\nCheck your device specifications or use --help for usage");
            std::process::exit(1);
        }
    }
}