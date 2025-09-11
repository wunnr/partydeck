use crate::game::{Executable, Game};
use crate::input::InputDevice;
use crate::instance::{set_instance_names,Instance};
use crate::util::scan_profiles;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    pub mode: LaunchMode,
    pub players: Vec<PlayerSpec>,
    pub auto_launch: bool,
    pub fullscreen: bool,
    pub kwin: bool,
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
    let mut parts: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut current_key: Option<&str> = None;
    
    for segment in spec.split(',') {
        if let Some((key, value)) = segment.split_once('=') {
            current_key = Some(key.trim());
            parts.entry(key.trim())
                .or_insert_with(Vec::new)
                .push(value.trim());
        } else if let Some(key) = current_key {
            if key == "devices" && !segment.trim().is_empty() {
                parts.entry(key)
                    .or_insert_with(Vec::new)
                    .push(segment.trim());
            }
        }
    }
    
    let profile = parts.get("profile")?.first()?.to_string();
    let devices = parts.get("devices")?
        .iter()
        .map(|d| d.to_string())
        .collect();
    let monitor = parts.get("monitor")
        .and_then(|m| m.first()?.parse().ok());
    
    Some(PlayerSpec { profile, devices, monitor })
}

pub fn build_instances_from_cli(
    players: &[PlayerSpec], 
    input_devices: &[InputDevice], 
    profiles: &[String]
) -> Result<Vec<Instance>, String> {
    let mut instances = Vec::new();
    let mut used_device_indices: Vec<usize> = Vec::new();

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
        } else {
            // Check if profile exists
            if let Some(prof_idx) = profiles
                .iter()
                .position(|p| p.eq_ignore_ascii_case(&player_spec.profile))
            {
                instance.profselection = prof_idx;
                instance.profname = profiles[prof_idx].clone();
            } else {
                // Create profile if it doesn't exist
                println!("[partydeck] Profile '{}' not found, creating new profile...",
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
            }
        }

        // Handle devices
        for device_id in &player_spec.devices {
            let idx = input_devices
                .iter()
                .enumerate()
                .find(|(idx, device)| {
                    !used_device_indices.contains(idx) && device.matches(device_id)
                })
                .map(|(idx, _)| idx);

            if let Some(idx) = idx {
                if !instance.devices.contains(&idx) {
                    instance.devices.push(idx);
                    used_device_indices.push(idx);
                }
            } else {
                println!(
                    "[partydeck] Warning: No available device matching '{}' for player {}",
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

    set_instance_names(&mut instances, profiles);

    Ok(instances)
}

pub static USAGE_TEXT: &str = r#"
Usage: partydeck [OPTIONS]

Options:
    --exec <executable>      Launch a specific executable

    --args <arguments>       Arguments for the executable (use after --exec)

    --handler <uid>          Launch a game using its handler UID

    --player <spec>          Add a player with profile and devices
                             Format: profile=<name>,devices=<dev1>,<dev2>,...
                             Optional: monitor=<index>
                             Note: Profiles will be created automatically if they don't exist

    --auto-launch            Automatically start the game without GUI interaction

    --fullscreen             Start the GUI in fullscreen mode

    --kwin                   Launch PartyDeck inside of a KWin session

    --help                   Show this help message

Examples:
    # Launch with handler and two players
    partydeck --handler "MyGameUID" \
        --player "profile=Player1,devices=/dev/input/event3,/dev/input/event5" \
        --player "profile=Player2,devices=Xbox Controller,monitor=1" \
        --auto-launch

    # Launch with handler and two players (GameMode - PartyDeck launch options)
    --handler "MyGameUID"
    --player "profile=Player1,devices=/dev/input/event3,/dev/input/event5"
    --player "profile=Player2,devices=Xbox Controller"
    --player "profile=Player3,devices=Xbox Controller"
    --auto-launch --kwin --fullscreen

Device specifications:
    - Use exact paths: /dev/input/event3
    - Use device names: "Xbox Controller", "PS Controller", "Keyboard", "Mouse"
    - Names are case-insensitive and can be partial matches

Monitor specification:
    - Add monitor=<index> to assign player to specific monitor (0-based)
"#;