use rand::prelude::*;
use std::error::Error;
use std::path::PathBuf;

use crate::util::filesystem::copy_dir_recursive;
use crate::{handler::Handler, paths::*};

// Makes a folder and sets up Goldberg Steam Emu profile for Steam games
pub fn create_profile(name: &str) -> Result<(), std::io::Error> {
    if PATH_PARTY.join(format!("profiles/{name}")).exists() {
        return Ok(());
    }

    println!("Creating profile {name}");
    let path_steam = PATH_PARTY.join(format!("profiles/{name}/steam/settings"));
    std::fs::create_dir_all(path_steam.clone())?;

    let steam_id = format!("{:017}", rand::rng().random_range(u32::MIN..u32::MAX));
    let usersettings = format!(
        "[user::general]\naccount_name={name}\naccount_steamid={steam_id}\nlanguage=english\nip_country=US"
    );
    std::fs::write(path_steam.join("configs.user.ini"), usersettings)?;

    println!("Created successfully");
    Ok(())
}

// Creates the "game save" folder for per-profile game data to go into
pub fn create_gamesave(name: &str, h: &Handler) -> Result<(), Box<dyn Error>> {
    let path_gamesave = PATH_PARTY
        .join("profiles")
        .join(name)
        .join("saves")
        .join(&h.uid);

    if path_gamesave.exists() {
        println!("{} already has save for {}, continuing...", name, h.uid);
        return Ok(());
    }
    println!("Creating game save {} for {}", h.uid, name);

    if h.win_unique_appdata {
        std::fs::create_dir_all(path_gamesave.join("_AppData/Local"))?;
        std::fs::create_dir_all(path_gamesave.join("_AppData/LocalLow"))?;
        std::fs::create_dir_all(path_gamesave.join("_AppData/Roaming"))?;
    }
    if h.win_unique_documents {
        std::fs::create_dir_all(path_gamesave.join("_Documents"))?;
    }
    if h.linux_unique_localshare {
        std::fs::create_dir_all(path_gamesave.join("_share"))?;
    }
    if h.linux_unique_config {
        std::fs::create_dir_all(path_gamesave.join("_config"))?;
    }

    for path in &h.game_unique_paths {
        if path.is_empty() {
            continue;
        }
        // If the path contains a dot, we assume it to be a file, and don't create a directory,
        // hoping that the handler uses copy_to_profilesave to get the relevant file in there.
        // Kind of a hacky solution since folders can technically have dots in their names.
        if path.contains('.') {
            continue;
        }
        println!("Creating subdirectory /{path}");
        let path = path_gamesave.join(path);
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
    }

    let copy_save_src = PathBuf::from(&h.path_handler).join("copy_to_profilesave");
    if copy_save_src.exists() {
        println!("{} handler has built-in save data, copying...", h.uid);
        copy_dir_recursive(&copy_save_src, &path_gamesave, false, true)?;
    }

    println!("Save data directories created successfully");
    Ok(())
}

// Gets a vector of all available profiles.
// include_guest true for building the profile selector dropdown, false for the profile viewer.
pub fn scan_profiles(include_guest: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(PATH_PARTY.join("profiles")) {
        for entry in entries {
            if let Ok(entry) = entry {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    if let Some(name) = entry.file_name().to_str() {
                        out.push(name.to_string());
                    }
                }
            }
        }
    }

    out.sort();

    if include_guest {
        out.insert(0, "Guest".to_string());
    }

    out
}

pub fn remove_guest_profiles() -> Result<(), Box<dyn Error>> {
    let path_profiles = PATH_PARTY.join("profiles");
    let entries = std::fs::read_dir(&path_profiles)?;
    for entry in entries.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with(".") {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub static GUEST_NAMES: [&str; 31] = [
    "Blinky", "Pinky", "Inky", "Clyde", "Beatrice", "Battler", "Miyao", "Rena", "Ellie", "Joel",
    "Leon", "Ada", "Madeline", "Theo", "Yokatta", "Wyrm", "Brodiee", "Supreme", "Conk", "Gort",
    "Lich", "Smores", "Canary", "Trico", "Yorda", "Wander", "Agro", "Jak", "Daxter", "Soap",
    "Ghost",
];
