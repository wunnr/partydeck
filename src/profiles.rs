use rand::prelude::*;
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::{handler::Handler, paths::*, util::copy_dir_recursive};

// Makes a folder and sets up Goldberg Steam Emu profile for Steam games
pub fn create_profile(name: &str) -> Result<(), std::io::Error> {
    if PATH_PARTY.join(format!("profiles/{name}")).exists() {
        return Ok(());
    }

    println!("[partydeck] Creating profile {name}");
    let path_profile = PATH_PARTY.join(format!("profiles/{name}"));
    let path_steam = path_profile.join("steam/settings");

    std::fs::create_dir_all(path_profile.join("work"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/Local"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/LocalLow"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/Roaming"))?;
    std::fs::create_dir_all(path_profile.join("windata/Documents"))?;
    std::fs::create_dir_all(path_profile.join("home/.local/share"))?;
    std::fs::create_dir_all(path_profile.join("home/.config"))?;
    std::fs::create_dir_all(path_steam.clone())?;

    let steam_id = format!("{:017}", rand::rng().random_range(u32::MIN..u32::MAX));
    let usersettings = format!(
        "[user::general]\naccount_name={name}\naccount_steamid={steam_id}\nlanguage=english\nip_country=US"
    );
    std::fs::write(path_steam.join("configs.user.ini"), usersettings)?;

    println!("[partydeck] Profile created successfully");
    Ok(())
}

// Creates the "game save" folder for per-profile game data to go into
pub fn create_profile_gamesave(name: &str, h: &Handler) -> Result<(), Box<dyn Error>> {
    let uid = h.handler_dir_name();
    let path_prof = PATH_PARTY.join("profiles").join(name);
    let path_gamesave = path_prof.join("gamesaves").join(&uid);
    let path_home = path_prof.join("home");
    let path_windata = path_prof.join("windata");

    if path_gamesave.exists() {
        return Ok(());
    }
    println!("[partydeck] Creating game save {} for {}", uid, name);

    for path in &h.game_save_paths {
        if path.is_empty() {
            continue;
        }
        let Ok(rootpath) = h.get_game_rootpath() else {
            break;
        };
        let game_subpath = PathBuf::from(rootpath).join(path);
        if game_subpath.is_file() {
            if let Some(parent) = PathBuf::from(path).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(path_gamesave.join(parent))?;
            }
            std::fs::copy(&game_subpath, path_gamesave.join(path))?;
        } else if game_subpath.is_dir() {
            std::fs::create_dir_all(path_gamesave.join(path))?;
        }
    }

    let profile_copy_gamesave = PathBuf::from(&h.path_handler).join("profile_copy_gamesave");
    if profile_copy_gamesave.exists() {
        copy_dir_recursive(&profile_copy_gamesave, &path_gamesave)?;
    }

    let profile_copy_home = PathBuf::from(&h.path_handler).join("profile_copy_home");
    if profile_copy_home.exists() {
        copy_dir_recursive(&profile_copy_home, &path_home)?;
    }

    let profile_copy_windata = PathBuf::from(&h.path_handler).join("profile_copy_windata");
    if profile_copy_windata.exists() {
        copy_dir_recursive(&profile_copy_windata, &path_windata)?;
    }

    println!("[partydeck] Profile save data created successfully");
    Ok(())
}

// Gets a vector of all available profiles.
// include_guest true for building the profile selector dropdown, false for the profile viewer.
pub fn scan_profiles(include_guest: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(PATH_PARTY.join("profiles")) {
        for entry in entries {
            if let Ok(entry) = entry
                && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && let Some(name) = entry.file_name().to_str()
            {
                out.push(name.to_string());
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
            // When bwrap uses a work folder it locks permissions, so we need to unlock them before removing the directory
            let path = entry.path().join("work").join("work");
            if path.exists() {
                let mut perms = std::fs::metadata(&path)?.permissions();
                perms.set_mode(0o777);
                std::fs::set_permissions(&path, perms)?;
            }

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
