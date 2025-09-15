use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

pub static PATH_RES: LazyLock<PathBuf> = LazyLock::new(|| {
    let localinstall = PathBuf::from("/usr/share/partydeck");
    if localinstall.exists() {
        return localinstall;
    }
    env::current_exe().unwrap().parent().unwrap().join("res")
});

pub static PATH_HOME: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var("HOME").unwrap()));

pub static PATH_LOCAL_SHARE: LazyLock<PathBuf> = LazyLock::new(|| PATH_HOME.join(".local/share"));

pub static PATH_PARTY: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(xdg_data_home) = env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join("partydeck");
    }
    PATH_LOCAL_SHARE.join("partydeck")
});

pub static PATH_STEAM: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(steamdir) = steamlocate::SteamDir::locate() {
        let steam_path = steamdir.path().to_path_buf();
        if steam_path.exists() {
            return steam_path;
        }
    }

    // Backup
    if let Ok(steam_path) = env::var("STEAM_BASE_FOLDER") {
        return PathBuf::from(steam_path);
    } else if PATH_LOCAL_SHARE.join("Steam").exists() {
        PATH_LOCAL_SHARE.join("Steam")
    } else if PATH_HOME
        .join(".var/app/com.valvesoftware.Steam/.steam/steam")
        .exists()
    {
        PATH_HOME.join(".var/app/com.valvesoftware.Steam/.steam/steam")
    } else {
        PATH_HOME.join(".steam/steam")
    }
});

pub static BIN_UMU_RUN: LazyLock<PathBuf> = LazyLock::new(|| {
    let bin_candidates = [PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")];

    for candidate in &bin_candidates {
        let bin = candidate.join("umu-run");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("umu-run")
});

pub static BIN_GSC_KBM: LazyLock<PathBuf> = LazyLock::new(|| {
    let bin_candidates = [PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")];

    for candidate in &bin_candidates {
        let bin = candidate.join("gamescope-kbm");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("gamescope-kbm")
});
