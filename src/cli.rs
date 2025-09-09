use crate::game::{find_game_by_handler_uid, Executable, Game};
use crate::input::{find_device_index, InputDevice};
use crate::instance::Instance;
use crate::util::{scan_profiles, GUEST_NAMES};
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    pub mode: LaunchMode,
    pub players: Vec<PlayerSpec>,
    pub auto_launch: bool,
    pub fullscreen: bool,
    pub kwin: bool,
    pub create_profiles: bool,
}

#[derive(Debug, Clone)]
pub enum LaunchMode {
    Gui,
    Handler(String),
    Executable(String, String),
}

impl Default for LaunchMode {
    fn default() -> Self {
        LaunchMode::Gui
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSpec {
    pub profile: String,
    pub devices: Vec<String>,
    pub monitor: Option<usize>,
}

pub fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli_args = CliArgs::default();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--help" => {
                println!("{}", USAGE_TEXT);
                std::process::exit(0);
            }
            "--handler" => {
                if let Some(uid) = args.get(i + 1) {
                    cli_args.mode = LaunchMode::Handler(uid.clone());
                    i += 2;
                } else {
                    eprintln!("Error: --handler requires a handler UID");
                    std::process::exit(1);
                }
            }
            "--exec" => {
                if let Some(exec) = args.get(i + 1) {
                    let exec_args = if args.get(i + 2).map_or(false, |a| a == "--args") {
                        i += 2;
                        args.get(i + 1).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    cli_args.mode = LaunchMode::Executable(exec.clone(), exec_args);
                    i += 2;
                } else {
                    eprintln!("Error: --exec requires an executable path");
                    std::process::exit(1);
                }
            }
            "--args" => {
                // Handle args that follow --exec (already processed above)
                if !matches!(cli_args.mode, LaunchMode::Executable(_, _)) {
                    eprintln!("Error: --args must follow --exec");
                    std::process::exit(1);
                }
                i += 2;
            }
            "--player" => {
                if let Some(spec) = args.get(i + 1) {
                    match parse_player_spec(spec) {
                        Some(player) => cli_args.players.push(player),
                        None => {
                            eprintln!("Error: Invalid player specification: {}", spec);
                            eprintln!("Format: profile=<name>,devices=<dev1>,<dev2>,...");
                            std::process::exit(1);
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("Error: --player requires a specification");
                    std::process::exit(1);
                }
            }
            "--auto-launch" => {
                cli_args.auto_launch = true;
                i += 1;
            }
            "--fullscreen" => {
                cli_args.fullscreen = true;
                i += 1;
            }
            "--kwin" => {
                cli_args.kwin = true;
                i += 1;
            }
            "--create-profiles" => {
                cli_args.create_profiles = true;
                i += 1;
            }
            _ => {
                if args[i].starts_with("--") {
                    eprintln!("Unknown argument: {}", args[i]);
                    eprintln!("Use --help for usage information");
                    std::process::exit(1);
                }
                i += 1;
            }
        }
    }

    cli_args
}

fn parse_player_spec(spec: &str) -> Option<PlayerSpec> {
    let mut profile = String::new();
    let mut devices = Vec::new();
    let mut monitor = None;
    let mut in_devices = false;

    for part in spec.split(',') {
        if let Some((key, value)) = part.split_once('=') {
            match key.trim() {
                "profile" => {
                    profile = value.trim().to_string();
                    in_devices = false;
                }
                "devices" => {
                    devices.push(value.trim().to_string());
                    in_devices = true;
                }
                "monitor" => {
                    monitor = value.trim().parse::<usize>().ok();
                    in_devices = false;
                }
                _ => {}
            }
        } else if in_devices || part.starts_with("/dev/") {
            devices.push(part.trim().to_string());
        }
    }

    if !profile.is_empty() && !devices.is_empty() {
        Some(PlayerSpec { profile, devices, monitor })
    } else {
        None
    }
}

pub fn build_instances_from_cli(
    players: &[PlayerSpec],
    input_devices: &[InputDevice],
    profiles: &[String],
    create_profiles: bool,
) -> Result<Vec<Instance>, String> {
    let mut instances = Vec::new();
    let mut used_guest_names = Vec::new();

    for (i, player_spec) in players.iter().enumerate() {
        let mut instance = Instance {
            devices: Vec::new(),
            profname: String::new(),
            profselection: 0,
            monitor: player_spec.monitor.unwrap_or(0),
            width: 0,
            height: 0,
        };

        // Handle profile
        if player_spec.profile.eq_ignore_ascii_case("guest") {
            instance.profselection = 0;
            let available_names: Vec<&str> = GUEST_NAMES
                .iter()
                .filter(|&&name| !used_guest_names.contains(&name))
                .copied()
                .collect();
            
            if !available_names.is_empty() {
                let chosen = available_names[fastrand::usize(..available_names.len())];
                instance.profname = format!(".{}", chosen);
                used_guest_names.push(chosen);
            } else {
                instance.profname = format!(".Guest{}", i + 1);
            }
        } else if let Some(prof_idx) = profiles
            .iter()
            .position(|p| p.eq_ignore_ascii_case(&player_spec.profile))
        {
            instance.profselection = prof_idx;
            instance.profname = player_spec.profile.clone();
        } else if create_profiles {
            println!(
                "[partydeck] Creating new profile '{}'",
                player_spec.profile
            );
            if let Err(e) = crate::util::create_profile(&player_spec.profile) {
                return Err(format!(
                    "Failed to create profile '{}': {}",
                    player_spec.profile, e
                ));
            }
            // Rescan profiles and find the index
            let updated_profiles = scan_profiles(true);
            if let Some(prof_idx) = updated_profiles
                .iter()
                .position(|p| p.eq_ignore_ascii_case(&player_spec.profile))
            {
                instance.profselection = prof_idx;
                instance.profname = player_spec.profile.clone();
            } else {
                return Err(format!(
                    "Failed to find profile '{}' after creation",
                    player_spec.profile
                ));
            }
        } else {
            return Err(format!(
                "Profile '{}' not found. Use --create-profiles to create new profiles.",
                player_spec.profile
            ));
        }

        // Handle devices
        for device_id in &player_spec.devices {
            if let Some(idx) = find_device_index(input_devices, device_id) {
                if !instance.devices.contains(&idx) {
                    instance.devices.push(idx);
                }
            } else {
                println!(
                    "[partydeck] Warning: Device '{}' not found for player {}",
                    device_id,
                    i + 1
                );
            }
        }

        if instance.devices.is_empty() {
            return Err(format!(
                "No valid devices found for player with profile '{}'",
                player_spec.profile
            ));
        }

        instances.push(instance);
    }

    if instances.is_empty() {
        return Err("No instances created from CLI specifications".to_string());
    }

    Ok(instances)
}

pub fn resolve_game_from_cli(mode: &LaunchMode) -> Result<Game, String> {
    match mode {
        LaunchMode::Gui => Err("GUI mode does not specify a game".to_string()),
        LaunchMode::Handler(uid) => find_game_by_handler_uid(uid)
            .ok_or_else(|| format!("Handler with UID '{}' not found", uid)),
        LaunchMode::Executable(exec, args) => Ok(Game::ExecRef(Executable::new(
            PathBuf::from(exec),
            args.clone(),
        ))),
    }
}

pub static USAGE_TEXT: &str = r#"
Usage: partydeck [OPTIONS]

Options:
    --handler <uid>          Launch a game using its handler UID

    --exec <executable>      Launch a specific executable

    --args <arguments>       Arguments for the executable (use after --exec)

    --player <spec>          Add a player with profile and devices
                             Format: profile=<name>,devices=<dev1>,<dev2>,...
                             Optional: monitor=<index>

    --auto-launch            Automatically start the game without GUI interaction

    --create-profiles        Allow creation of new profiles if they don't exist

    --fullscreen             Start the GUI in fullscreen mode

    --kwin                   Launch PartyDeck inside of a KWin session

    --help                   Show this help message

Examples:
    # Launch with handler and two players
    partydeck --handler "MyGameUID" \
        --player "profile=Player1,devices=/dev/input/event3,/dev/input/event5" \
        --player "profile=Player2,devices=Xbox Controller,monitor=1" \
        --create-profiles --auto-launch

    # GUI mode with fullscreen
    partydeck --fullscreen

Device specifications:
    - Use exact paths: /dev/input/event3
    - Use device names: "Xbox Controller", "PS Controller", "Keyboard", "Mouse"
    - Names are case-insensitive and can be partial matches

Monitor specification:
    - Add monitor=<index> to assign player to specific monitor (0-based)
"#;