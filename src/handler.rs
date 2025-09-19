use crate::paths::*;
use crate::util::{copy_dir_recursive, zip_dir};

use dialog::DialogBox;
use eframe::egui::{self, ImageSource};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
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
    pub env: String,

    pub pause_between_starts: Option<f64>,

    pub use_goldberg: bool,
    pub steam_appid: Option<u32>,

    pub game_null_paths: Vec<String>,
    pub game_save_paths: Vec<String>,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),
            path_gameroot: String::new(),
            uid: String::new(),

            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),

            runtime: String::new(),
            is32bit: false,
            exec: String::new(),
            args: String::new(),
            env: String::new(),
            pause_between_starts: None,

            use_goldberg: false,
            steam_appid: None,

            game_null_paths: Vec::new(),
            game_save_paths: Vec::new(),
        }
    }
}

impl Handler {
    pub fn from_json(json_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
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

    pub fn from_cli(exec: &str, args: &str) -> Self {
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
            env: String::new(),
            args: args.split_whitespace().map(|s| s.to_string()).collect(),
            pause_between_starts: None,

            use_goldberg: false,
            steam_appid: None,

            game_null_paths: Vec::new(),
            game_save_paths: Vec::new(),
        }
    }

    pub fn save_to_json(&mut self) -> Result<(), Box<dyn Error>> {
        // If handler has no path, assume we're saving a newly created handler
        if self.path_handler.as_os_str().is_empty() {
            if let Some(uid) =
                dialog::Input::new("Enter unique ID for new handler (must be alphanumeric):")
                    .title("New Handler")
                    .show()
                    .expect("Could not display dialog box")
            {
                if uid.is_empty() {
                    return Err("ID cannot be empty".into());
                } else if !uid.chars().all(char::is_alphanumeric) {
                    return Err("ID must be alphanumeric".into());
                } else if PATH_PARTY.join("handlers").join(&uid).exists() {
                    return Err(format!("Handler with ID '{}' already exists", uid).into());
                } else {
                    self.path_handler = PATH_PARTY.join("handlers").join(&uid);
                }
            } else {
                return Err("Handler not saved".into());
            }
        }

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

    pub fn export_pd2(&self) -> Result<(), Box<dyn Error>> {
        let mut file = FileDialog::new()
            .set_title("Save file to:")
            .set_directory(&*PATH_HOME)
            .add_filter("PartyDeck Handler Package", &["pd2"])
            .save_file()
            .ok_or_else(|| "File not specified")?;

        if file.extension().is_none() || file.extension() != Some("pd2".as_ref()) {
            file.set_extension("pd2");
        }

        let tmpdir = PATH_PARTY.join("tmp");
        std::fs::create_dir_all(&tmpdir)?;

        copy_dir_recursive(&self.path_handler, &tmpdir)?;

        // Clear the rootpath before exporting so that users downloading it can set their own
        let mut handlerclone = self.clone();
        handlerclone.path_gameroot = String::new();
        // Overwrite the handler.json file with handlerclone
        let json = serde_json::to_string_pretty(&mut handlerclone)?;
        std::fs::write(tmpdir.join("handler.json"), json)?;

        zip_dir(&tmpdir, &file)?;
        std::fs::remove_dir_all(&tmpdir)?;

        Ok(())
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
        if let Ok(entry) = entry_result
            && let Ok(file_type) = entry.file_type()
            && file_type.is_dir()
        {
            let json_path = entry.path().join("handler.json");
            if json_path.exists()
                && let Ok(handler) = Handler::from_json(&json_path)
            {
                out.push(handler);
            }
        }
    }
    out.sort_by(|a, b| a.display().to_lowercase().cmp(&b.display().to_lowercase()));
    out
}

pub fn import_pd2() -> Result<(), Box<dyn Error>> {
    let Some(file) = FileDialog::new()
        .set_title("Select File")
        .set_directory(&*PATH_HOME)
        .add_filter("PartyDeck Handler Package", &["pd2"])
        .pick_file()
    else {
        return Ok(());
    };

    if !file.exists() || !file.is_file() || file.extension().unwrap_or_default() != "pd2" {
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

    if let Some(uid) =
        dialog::Input::new("Enter unique ID to save handler to (must be alphanumeric):")
            .title("New Handler")
            .show()
            .expect("Could not display dialog box")
    {
        if uid.is_empty() {
            return Err("ID cannot be empty".into());
        } else if !uid.chars().all(char::is_alphanumeric) {
            return Err("ID must be alphanumeric".into());
        } else if PATH_PARTY.join("handlers").join(&uid).exists() {
            return Err(format!("Handler with ID '{}' already exists", uid).into());
        } else {
            copy_dir_recursive(&dir_tmp, &dir_handlers.join(uid))?;
            std::fs::remove_dir_all(&dir_tmp)?;

            Ok(())
        }
    } else {
        Err("Handler not saved".into())
    }
}
