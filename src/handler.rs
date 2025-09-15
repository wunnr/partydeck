use crate::paths::*;
use crate::util::copy_dir_recursive;

use eframe::egui::{self, ImageSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Handler {
    // Members that are determined by context
    #[serde(skip)]
    pub path_handler: PathBuf,
    #[serde(skip)]
    pub img_paths: Vec<PathBuf>,
    #[serde(skip)]
    pub uid: String,

    pub name: String,
    pub author: String,
    pub version: String,
    pub info: String,

    pub path_gameroot: String,
    pub runtime: String,
    pub is32bit: bool,
    pub exec: String,
    pub args: String,
    pub remove_paths: Vec<String>,
    pub dll_overrides: Vec<String>,
    pub pause_between_starts: Option<f64>,

    pub use_goldberg: bool,
    pub steam_appid: Option<u32>,

    pub game_unique_paths: Vec<String>,
}

impl Handler {
    pub fn new_from_uid(uid: &str) -> Self {
        let path_handler = PATH_PARTY.join("handlers").join(uid);
        Self {
            path_handler,
            img_paths: Vec::new(),
            path_gameroot: String::new(),
            uid: uid.to_string(),

            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),

            runtime: String::new(),
            is32bit: false,
            exec: String::new(),
            args: String::new(),
            remove_paths: Vec::new(),
            dll_overrides: Vec::new(),
            pause_between_starts: None,

            use_goldberg: false,
            steam_appid: None,

            game_unique_paths: Vec::new(),
        }
    }

    pub fn new_from_json2(json_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(json_path)?;
        let mut handler = serde_json::from_reader::<_, Handler>(BufReader::new(file))?;

        handler.uid = json_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        handler.path_handler = json_path
            .parent()
            .ok_or_else(|| "Invalid path")?
            .to_path_buf();
        handler.img_paths = handler.get_imgs();

        Ok(handler)
    }

    pub fn new_from_cli(exec: &str, args: &str) -> Self {
        Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),
            path_gameroot: String::new(),

            uid: "".to_string(),
            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),

            runtime: String::new(),
            is32bit: false,
            exec: exec.to_string(),
            args: args.split_whitespace().map(|s| s.to_string()).collect(),
            remove_paths: Vec::new(),
            dll_overrides: Vec::new(),
            pause_between_starts: None,

            use_goldberg: false,
            steam_appid: None,

            game_unique_paths: Vec::new(),
        }
    }

    pub fn save_to_json(&self) -> Result<(), std::io::Error> {
        if !self.path_handler.exists() {
            std::fs::create_dir_all(&self.path_handler)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(self.path_handler.join("handler.json"), json)?;
        Ok(())
    }

    pub fn icon(&self) -> ImageSource<'_> {
        if self.path_handler.join("icon.png").exists() {
            format!("file://{}/icon.png", self.path_handler.display()).into()
        } else {
            egui::include_image!("../res/executable_icon.png")
        }
    }

    pub fn display(&self) -> &str {
        if self.name.is_empty() {
            self.uid.as_str()
        } else {
            self.name.as_str()
        }
    }

    pub fn win(&self) -> bool {
        self.exec.ends_with(".exe") || self.exec.ends_with(".bat")
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

    pub fn remove_dir(&self) -> Result<(), Box<dyn Error>> {
        std::fs::remove_dir_all(self.path_handler.clone())?;

        Ok(())
    }

    pub fn locate_steamapi_path(&self) -> Option<PathBuf> {
        let dllname = match &self.win() {
            true => match &self.is32bit {
                true => "steam_api.dll",
                false => "steam_api64.dll",
            },
            false => "libsteam_api.so",
        };

        if let Ok(rootpath) = self.get_game_rootpath() {
            let walk_path = walkdir::WalkDir::new(rootpath)
                .min_depth(1)
                .follow_links(false);

            for entry in walk_path {
                if let Ok(entry) = entry
                    && entry.file_type().is_file()
                    && let Some(path_str) = entry.path().to_str()
                    && path_str.ends_with(dllname)
                {
                    return Some(entry.path().to_path_buf());
                }
            }
        }

        None
    }

    pub fn get_game_rootpath(&self) -> Result<String, Box<dyn Error>> {
        if let Some(appid) = &self.steam_appid
            && let Some((app, library)) = steamlocate::SteamDir::locate()?
                .find_app(*appid)
                .ok()
                .flatten()
        {
            {
                let path = library.resolve_app_dir(&app);
                if path.exists() {
                    let pathstr = path.to_string_lossy().to_string();
                    return Ok(pathstr);
                }
            }
        }

        if !self.path_gameroot.is_empty() && Path::new(&self.path_gameroot).exists() {
            return Ok(self.path_gameroot.clone());
        }

        Err("Game root path not found".into())
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
        if let Ok(handler) = Handler::new_from_json2(&json_path) {
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

    copy_dir_recursive(&dir_tmp, &dir_handlers.join(uid))?;
    std::fs::remove_dir_all(&dir_tmp)?;

    Ok(())
}

// pub fn create_symlink_folder(h: &Handler) -> Result<(), Box<dyn Error>> {
//     let path_root = PathBuf::from(get_rootpath_handler(&h)?);
//     let path_sym = PATH_PARTY.join(format!("gamesyms/{}", h.uid));
//     if path_sym.exists() {
//         return Ok(());
//     }
//     std::fs::create_dir_all(path_sym.to_owned())?;
//     copy_dir_recursive(&path_root, &path_sym, true, false)?;

//     // copy_instead_paths takes symlink files and replaces them with their real equivalents
//     for path in &h.copy_instead_paths {
//         let src = path_root.join(path);
//         if !src.exists() {
//             continue;
//         }
//         let dest = path_sym.join(path);
//         println!("src: {}, dest: {}", src.display(), dest.display());
//         if src.is_dir() {
//             println!("Copying directory: {}", src.display());
//             copy_dir_recursive(&src, &dest, false, true)?;
//         } else if src.is_file() {
//             println!("Copying file: {}", src.display());
//             if dest.exists() {
//                 std::fs::remove_file(&dest)?;
//             }
//             std::fs::copy(&src, &dest)?;
//         }
//     }
//     for path in h.remove_paths.iter().chain(h.game_unique_paths.iter()) {
//         let p = path_sym.join(path);
//         if !p.exists() {
//             continue;
//         }
//         if p.is_dir() {
//             std::fs::remove_dir_all(p)?;
//         } else if p.is_file() {
//             std::fs::remove_file(p)?;
//         }
//     }
//     let copypath = PathBuf::from(&h.path_handler).join("copy_to_symdir");
//     if copypath.exists() {
//         copy_dir_recursive(&copypath, &path_sym, false, true)?;
//     }

//     // Insert goldberg dll
//     if !h.path_goldberg.is_empty() {
//         let dest = path_sym.join(&h.path_goldberg);

//         let steam_settings = dest.join("steam_settings");
//         if !steam_settings.exists() {
//             std::fs::create_dir_all(steam_settings.clone())?;
//         }
//         if let Some(appid) = &h.steam_appid {
//             std::fs::write(steam_settings.join("steam_appid.txt"), appid.as_str())?;
//         }

//         // If the game uses goldberg coldclient, assume the handler owner has set up coldclient in the copy_to_symdir files
//         // And so we don't copy goldberg dlls or generate interfaces
//         if h.coldclient {
//             return Ok(());
//         }

//         let mut src = PATH_RES.clone();
//         src = match &h.win() {
//             true => src.join("goldberg/win"),
//             false => src.join("goldberg/linux"),
//         };
//         src = match &h.is32bit {
//             true => src.join("x32"),
//             false => src.join("x64"),
//         };

//         copy_dir_recursive(&src, &dest, false, true)?;

//         let path_steamdll = path_root.join(&h.path_goldberg);
//         let steamdll = match &h.win() {
//             true => match &h.is32bit {
//                 true => path_steamdll.join("steam_api.dll"),
//                 false => path_steamdll.join("steam_api64.dll"),
//             },
//             false => path_steamdll.join("libsteam_api.so"),
//         };

//         let gen_interfaces = match &h.is32bit {
//             true => PATH_RES.join("goldberg/generate_interfaces_x32"),
//             false => PATH_RES.join("goldberg/generate_interfaces_x64"),
//         };
//         let status = std::process::Command::new(gen_interfaces)
//             .arg(steamdll)
//             .current_dir(steam_settings)
//             .status()?;
//         if !status.success() {
//             return Err("Generate interfaces failed".into());
//         }
//     }

//     Ok(())
// }
