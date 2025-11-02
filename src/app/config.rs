use crate::paths::*;

use std::error::Error;
use std::fs::File;
use std::io::BufReader;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum PadFilterType {
    All,
    NoSteamInput,
    OnlySteamInput,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PartyConfig {
    pub enable_kwin_script: bool,
    pub gamescope_fix_lowres: bool,
    pub gamescope_sdl_backend: bool,
    pub gamescope_force_grab_cursor: bool,
    pub kbm_support: bool,
    pub proton_version: String,
    pub proton_separate_pfxs: bool,
    #[serde(default)]
    pub vertical_two_player: bool,
    pub pad_filter_type: PadFilterType,
    #[serde(default)]
    pub allow_multiple_instances_on_same_device: bool,
    pub disable_mount_gamedirs: bool,
}

impl Default for PartyConfig {
    fn default() -> Self {
        PartyConfig {
            enable_kwin_script: true,
            gamescope_fix_lowres: true,
            gamescope_sdl_backend: true,
            gamescope_force_grab_cursor: false,
            kbm_support: true,
            proton_version: "".to_string(),
            proton_separate_pfxs: true,
            vertical_two_player: false,
            pad_filter_type: PadFilterType::NoSteamInput,
            allow_multiple_instances_on_same_device: false,
            disable_mount_gamedirs: false,
        }
    }
}

pub fn load_cfg() -> PartyConfig {
    let path = PATH_PARTY.join("settings.json");

    if let Ok(file) = File::open(path) {
        if let Ok(config) = serde_json::from_reader::<_, PartyConfig>(BufReader::new(file)) {
            return config;
        }
    }

    // Return default settings if file doesn't exist or has error
    return PartyConfig::default();
}

pub fn save_cfg(config: &PartyConfig) -> Result<(), Box<dyn Error>> {
    let path = PATH_PARTY.join("settings.json");
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}
