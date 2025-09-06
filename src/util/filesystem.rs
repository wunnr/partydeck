use crate::handler::Handler;
use crate::paths::*;
use rfd::FileDialog;
use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub fn copy_dir_recursive(
    src: &PathBuf,
    dest: &PathBuf,
    symlink_instead: bool,
    overwrite_dest: bool,
) -> Result<(), Box<dyn Error>> {
    println!(
        "[partydeck] util::copy_dir_recursive - src: {}, dest: {}",
        src.display(),
        dest.display()
    );

    let walk_path = walkdir::WalkDir::new(src).min_depth(1).follow_links(false);

    for entry in walk_path {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let new_path = dest.join(rel_path);
        // println!(
        //     "entry: {}\n rel_path: {}\n new_path: {}\n",
        //     entry.path().display(),
        //     rel_path.display(),
        //     new_path.display()
        // );

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&new_path)?;
        } else if entry.file_type().is_symlink() {
            let symlink_src = std::fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(symlink_src, new_path)?;
        } else {
            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if new_path.exists() && overwrite_dest {
                std::fs::remove_file(&new_path)?;
            }
            if symlink_instead {
                std::os::unix::fs::symlink(entry.path(), new_path)?;
            } else {
                std::fs::copy(entry.path(), new_path)?;
            }
        }
    }

    Ok(())
}

pub fn get_rootpath_handler(handler: &Handler) -> Result<String, Box<dyn Error>> {
    if let Some(value) = find_saved_path(&handler.uid) {
        return value;
    }

    if let Some(appid) = &handler.steam_appid
        && let Ok(appid_number) = str::parse::<u32>(appid)
        && let Some((app, library)) = steamlocate::SteamDir::locate()?
            .find_app(appid_number)
            .ok()
            .flatten()
    {
        {
            let path = library.resolve_app_dir(&app);
            if path.exists() {
                let pathstr = path.to_string_lossy().to_string();
                add_path(&handler.uid, &pathstr)?;
                return Ok(pathstr);
            }
        }
    }

    // If we didn't get a path from the file, ask user for folder
    let path = FileDialog::new()
        .set_title(format!("Locate folder for {}", handler.uid))
        .set_directory(&*PATH_HOME)
        .pick_folder()
        .ok_or_else(|| "No folder selected")?;
    let result = path.to_string_lossy().to_string();

    // Create/update the json file
    add_path(&handler.uid, &result)?;

    Ok(result)
}

pub fn get_rootpath(uid: &str) -> Result<String, Box<dyn Error>> {
    if let Some(value) = find_saved_path(uid) {
        return value;
    }

    // If we didn't get a path from the file, ask user for folder
    let path = FileDialog::new()
        .set_title(format!("Locate folder for {uid}"))
        .set_directory(&*PATH_HOME)
        .pick_folder()
        .ok_or_else(|| "No folder selected")?;
    let result = path.to_string_lossy().to_string();

    // Create/update the json file
    add_path(uid, &result)?;

    Ok(result)
}

fn add_path(uid: &str, path: &String) -> Result<(), Box<dyn Error>> {
    println!("[partydeck] util::add_path - Updating paths.json with {uid}: {path}");
    let mut paths = if let Ok(file) = File::open(PATH_PARTY.join("paths.json")) {
        serde_json::from_reader(BufReader::new(file))
            .unwrap_or(Value::Object(serde_json::Map::new()))
    } else {
        Value::Object(serde_json::Map::new())
    };

    if let Value::Object(ref mut map) = paths {
        map.insert(uid.to_string(), Value::String(path.clone()));
        std::fs::write(
            PATH_PARTY.join("paths.json"),
            serde_json::to_string_pretty(&paths)?,
        )?;
    }
    Ok(())
}

fn find_saved_path(uid: &str) -> Option<Result<String, Box<dyn Error>>> {
    println!("[partydeck] util::find_saved_path - Reading paths.json for root path of {uid}");
    if let Ok(file) = File::open(PATH_PARTY.join("paths.json"))
        && let Ok(json) = serde_json::from_reader::<_, Value>(BufReader::new(file))
        && let Some(path) = json.get(uid)
        && let Some(path_str) = path.as_str()
    {
        {
            println!("[partydeck] util::find_saved_path - Found root path for {uid}: {path_str}");
            return Some(Ok(path_str.to_string()));
        }
    }
    None
}

pub trait SanitizePath {
    fn sanitize_path(&self) -> String;
}

impl SanitizePath for String {
    fn sanitize_path(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut sanitized = self.clone();

        // Remove potentially dangerous characters
        // Allow single quotes in paths since they are quoted when launching
        // commands. Double quotes would break the quoting though, so we still
        // strip those along with other potentially dangerous characters.
        let chars_to_sanitize = [';', '&', '|', '$', '`', '(', ')', '<', '>', '"', '\\', '/'];

        if chars_to_sanitize.iter().any(|&c| sanitized.contains(c)) {
            sanitized = sanitized
                .replace(";", "")
                .replace("&", "")
                .replace("|", "")
                .replace("$", "")
                .replace("`", "")
                .replace("(", "")
                .replace(")", "")
                .replace("<", "")
                .replace(">", "")
                .replace("\"", "")
                .replace("\\", "/") // Convert Windows backslashes to forward slashes
                .replace("//", "/"); // Remove any doubled slashes
        }

        // Prevent path traversal attacks
        while sanitized.contains("../") || sanitized.contains("./") {
            sanitized = sanitized.replace("../", "").replace("./", "");
        }

        // Remove leading slash to allow joining with other paths
        if sanitized.starts_with('/') {
            sanitized = sanitized[1..].to_string();
        }

        sanitized
    }
}
