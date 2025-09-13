use crate::util::{scan_profiles, create_profile, GUEST_NAMES};
use crate::cli::PlayerSpec;
use crate::input::InputDevice;
use crate::monitor::Monitor;
use crate::app::PartyConfig;

#[derive(Clone)]
pub struct Instance {
    pub devices: Vec<usize>,
    pub profname: String,
    pub profselection: usize,
    pub monitor: usize,
    pub width: u32,
    pub height: u32,
}

pub fn set_instance_resolutions(
    instances: &mut Vec<Instance>,
    primary_monitor: &Monitor,
    cfg: &PartyConfig,
) {
    let (basewidth, baseheight) = (primary_monitor.width(), primary_monitor.height());
    let playercount = instances.len();

    for instance in instances {
        let (mut w, mut h) = match playercount {
            1 => (basewidth, baseheight),
            2 => {
                if cfg.vertical_two_player {
                    (basewidth / 2, baseheight)
                } else {
                    (basewidth, baseheight / 2)
                }
            }
            _ => (basewidth / 2, baseheight / 2),
        };
        if h < 600 && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = 600;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

pub fn set_instance_resolutions_multimonitor(
    instances: &mut Vec<Instance>,
    monitors: &Vec<Monitor>,
    cfg: &PartyConfig,
) {
    let mut mon_playercounts: Vec<usize> = vec![0; monitors.len()];
    for instance in instances.iter() {
        let mon = instance.monitor;
        mon_playercounts[mon] += 1;
    }

    for instance in instances.iter_mut() {
        let playercount = mon_playercounts[instance.monitor];
        let (basewidth, baseheight) = (
            monitors[instance.monitor].width(),
            monitors[instance.monitor].height(),
        );

        let (mut w, mut h) = match playercount {
            1 => (basewidth, baseheight),
            2 => {
                if cfg.vertical_two_player {
                    (basewidth / 2, baseheight)
                } else {
                    (basewidth, baseheight / 2)
                }
            }
            _ => (basewidth / 2, baseheight / 2),
        };
        if h < 600 && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = 600;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

pub fn set_instance_names(instances: &mut Vec<Instance>, profiles: &[String]) {
    let mut guests = GUEST_NAMES.to_vec();

    for instance in instances {
        if instance.profselection == 0 {
            let i = fastrand::usize(..guests.len());
            instance.profname = format!(".{}", guests[i]);
            guests.swap_remove(i);
        } else {
            instance.profname = profiles[instance.profselection].to_owned();
        }
    }
}

pub fn build_instance_from_specs(
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
                if let Err(e) = create_profile(&player_spec.profile) {
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