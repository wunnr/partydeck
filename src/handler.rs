use crate::paths::*;
use crate::util::*;

use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Handler {
    // Members that are determined by context
    pub path_handler: PathBuf,
    pub img_paths: Vec<PathBuf>,

    pub uid: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub info: String,

    pub symlink_dir: bool,
    pub win: bool,
    pub runtime: String,
    pub is32bit: bool,
    pub exec: String,
    pub args: Vec<String>,
    pub copy_instead_paths: Vec<String>,
    pub remove_paths: Vec<String>,
    pub dll_overrides: Vec<String>,
    pub pause_between_starts: Option<f64>,

    pub path_goldberg: String,
    pub steam_appid: Option<String>,
    pub coldclient: bool,

    pub game_unique_paths: Vec<String>,
}

impl Handler {
    pub fn new(json_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(json_path)?;
        let reader = BufReader::new(file);
        let json: Value = serde_json::from_reader(reader)?;

        let mut handler = Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),

            uid: json["handler.uid"].as_str().unwrap_or_default().to_string(),
            name: json["handler.name"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            info: json["handler.info"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            author: json["handler.author"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            version: json["handler.version"]
                .as_str()
                .unwrap_or_default()
                .to_string(),

            symlink_dir: json["game.symlink_dir"].as_bool().unwrap_or_default(),
            win: json["game.win"].as_bool().unwrap_or_default(),
            is32bit: json["game.32bit"].as_bool().unwrap_or_default(),
            runtime: json["game.runtime"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            exec: json["game.exec"]
                .as_str()
                .unwrap_or_default()
                .to_string()
                .sanitize_path(),
            args: json["game.args"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            copy_instead_paths: json["game.copy_instead_paths"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string().sanitize_path())
                        .collect()
                })
                .unwrap_or_default(),
            remove_paths: json["game.remove_paths"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string().sanitize_path())
                        .collect()
                })
                .unwrap_or_default(),
            dll_overrides: json["game.dll_overrides"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            pause_between_starts: json["game.pause_between_starts"].as_f64(),

            path_goldberg: json["steam.api_path"]
                .as_str()
                .unwrap_or_default()
                .to_string()
                .sanitize_path(),
            steam_appid: json["steam.appid"]
                .as_str()
                .and_then(|s| Some(s.to_string())),
            coldclient: json["steam.gb_coldclient"].as_bool().unwrap_or_default(),

            game_unique_paths: json["profiles.game_paths"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string().sanitize_path())
                        .collect()
                })
                .unwrap_or_default(),
        };

        if !handler.uid.chars().all(char::is_alphanumeric) {
            return Err("uid must be alphanumeric!".into());
        }

        handler.path_handler = json_path
            .parent()
            .ok_or_else(|| "Invalid path")?
            .to_path_buf();
        handler.img_paths = handler.get_imgs();

        Ok(handler)
    }

    pub fn display(&self) -> &str {
        if self.name.is_empty() {
            self.uid.as_str()
        } else {
            self.name.as_str()
        }
    }

    fn get_imgs(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let imgs_path = self.path_handler.join("imgs");

        let entries = match std::fs::read_dir(imgs_path) {
            Ok(entries) => entries,
            Err(_) => return out,
        };

        for entry_result in entries {
            if let Ok(entry) = entry_result
                && let Ok(file_type) = entry.file_type()
                && file_type.is_file()
                && let Some(path_str) = entry.path().to_str()
                && (path_str.ends_with(".png") || path_str.ends_with(".jpg"))
            {
                out.push(entry.path());
            }
        }

        out.sort();
        out
    }
}

pub fn scan_handlers() -> Vec<Handler> {
    let mut out: Vec<Handler> = Vec::new();
    let handlers_path = PATH_PARTY.join("handlers");

    let entries = match std::fs::read_dir(handlers_path) {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let json_path = entry.path().join("handler.json");
        if !json_path.exists() {
            continue;
        }
        if let Ok(handler) = Handler::new(&json_path) {
            out.push(handler);
        }
    }
    out.sort_by(|a, b| a.display().to_lowercase().cmp(&b.display().to_lowercase()));
    out
}

pub fn install_handler_from_file(file: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !file.exists() || !file.is_file() || file.extension().unwrap_or_default() != "pdh" {
        return Err("Handler not valid!".into());
    }

    let dir_handlers = PATH_PARTY.join("handlers");
    let dir_tmp = PATH_PARTY.join("tmp");
    if !dir_tmp.exists() {
        std::fs::create_dir_all(&dir_tmp)?;
    }

    let mut archive = zip::ZipArchive::new(File::open(&file)?)?;
    archive.extract(&dir_tmp)?;

    let handler_path = dir_tmp.join("handler.json");
    if !handler_path.exists() {
        return Err("handler.json not found in archive".into());
    }

    let handler_file = File::open(handler_path)?;
    let handler_json: Value = serde_json::from_reader(BufReader::new(handler_file))?;

    let uid = handler_json
        .get("handler.uid")
        .and_then(|v| v.as_str())
        .ok_or("No uid field found in handler.json")?;

    if !uid.chars().all(char::is_alphanumeric) {
        return Err("uid must be alphanumeric".into());
    }

    copy_dir_recursive(&dir_tmp, &dir_handlers.join(uid), false, true)?;
    std::fs::remove_dir_all(&dir_tmp)?;

    Ok(())
}

pub fn create_symlink_folder(h: &Handler) -> Result<(), Box<dyn Error>> {
    let path_root = PathBuf::from(get_rootpath_handler(&h)?);
    let path_sym = PATH_PARTY.join(format!("gamesyms/{}", h.uid));
    if path_sym.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(path_sym.to_owned())?;
    copy_dir_recursive(&path_root, &path_sym, true, false)?;

    // copy_instead_paths takes symlink files and replaces them with their real equivalents
    for path in &h.copy_instead_paths {
        let src = path_root.join(path);
        if !src.exists() {
            continue;
        }
        let dest = path_sym.join(path);
        println!("src: {}, dest: {}", src.display(), dest.display());
        if src.is_dir() {
            println!("Copying directory: {}", src.display());
            copy_dir_recursive(&src, &dest, false, true)?;
        } else if src.is_file() {
            println!("Copying file: {}", src.display());
            if dest.exists() {
                std::fs::remove_file(&dest)?;
            }
            std::fs::copy(&src, &dest)?;
        }
    }
    for path in h.remove_paths.iter().chain(h.game_unique_paths.iter()) {
        let p = path_sym.join(path);
        if !p.exists() {
            continue;
        }
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
        } else if p.is_file() {
            std::fs::remove_file(p)?;
        }
    }
    let copypath = PathBuf::from(&h.path_handler).join("copy_to_symdir");
    if copypath.exists() {
        copy_dir_recursive(&copypath, &path_sym, false, true)?;
    }

    // Insert goldberg dll
    if !h.path_goldberg.is_empty() {
        let dest = path_sym.join(&h.path_goldberg);

        let steam_settings = dest.join("steam_settings");
        if !steam_settings.exists() {
            std::fs::create_dir_all(steam_settings.clone())?;
        }
        if let Some(appid) = &h.steam_appid {
            std::fs::write(steam_settings.join("steam_appid.txt"), appid.as_str())?;
        }

        // If the game uses goldberg coldclient, assume the handler owner has set up coldclient in the copy_to_symdir files
        // And so we don't copy goldberg dlls or generate interfaces
        if h.coldclient {
            return Ok(());
        }

        let mut src = PATH_RES.clone();
        src = match &h.win {
            true => src.join("goldberg/win"),
            false => src.join("goldberg/linux"),
        };
        src = match &h.is32bit {
            true => src.join("x32"),
            false => src.join("x64"),
        };

        copy_dir_recursive(&src, &dest, false, true)?;

        let path_steamdll = path_root.join(&h.path_goldberg);
        let steamdll = match &h.win {
            true => match &h.is32bit {
                true => path_steamdll.join("steam_api.dll"),
                false => path_steamdll.join("steam_api64.dll"),
            },
            false => path_steamdll.join("libsteam_api.so"),
        };

        let gen_interfaces = match &h.is32bit {
            true => PATH_RES.join("goldberg/generate_interfaces_x32"),
            false => PATH_RES.join("goldberg/generate_interfaces_x64"),
        };
        let status = std::process::Command::new(gen_interfaces)
            .arg(steamdll)
            .current_dir(steam_settings)
            .status()?;
        if !status.success() {
            return Err("Generate interfaces failed".into());
        }
    }

    Ok(())
}
