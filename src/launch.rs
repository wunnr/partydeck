use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::PartyConfig;
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::paths::*;
use crate::profiles::{create_profile, create_profile_gamesave};
use crate::util::*;

pub fn launch_game(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[partydeck] Instances:");
    for instance in instances {
        if instance.profname.starts_with(".") {
            create_profile(&instance.profname)?;
        }
        if h.is_saved_handler() {
            create_profile_gamesave(&instance.profname, h)?;
        }
        println!(
            "  - Profile: {}, Monitor: {}, Resolution: {}x{}",
            instance.profname, instance.monitor, instance.width, instance.height
        );
    }

    fuse_overlayfs_mount_gamedirs(h, instances)?;

    if h.use_goldberg
        && let Some(appid) = h.steam_appid
        && let Some(steamdll_relative) = h.locate_steamapi_path()
    {
        std::fs::create_dir_all(PATH_PARTY.join("tmp/steam_settings"))?;

        std::fs::write(
            PATH_PARTY.join("tmp/steam_settings/steam_appid.txt"),
            appid.to_string(),
        )?;

        let gen_interfaces = match &h.is32bit {
            true => PATH_RES.join("goldberg/generate_interfaces_x32"),
            false => PATH_RES.join("goldberg/generate_interfaces_x64"),
        };
        let path_dll = PathBuf::from(h.get_game_rootpath()?).join(steamdll_relative);
        let status = std::process::Command::new(gen_interfaces)
            .arg(path_dll)
            .current_dir(PATH_PARTY.join("tmp/steam_settings"))
            .status()?;
        if !status.success() {
            return Err("Generate interfaces failed".into());
        }
    }

    let new_cmds = launch_cmds(h, input_devices, instances, cfg)?;
    print_launch_cmds(&new_cmds);

    if cfg.enable_kwin_script {
        let script = if instances.len() == 2 && cfg.vertical_two_player {
            "splitscreen_kwin_vertical.js"
        } else {
            "splitscreen_kwin.js"
        };

        kwin_dbus_start_script(PATH_RES.join(script))?;
    }

    let mut handles = Vec::new();

    let sleep_time = match h.pause_between_starts {
        Some(f) => f,
        None => 0.5,
    };

    let mut i = 0;
    for mut cmd in new_cmds {
        let handle = cmd.spawn()?;
        handles.push(handle);

        if i < instances.len() - 1 {
            std::thread::sleep(std::time::Duration::from_secs_f64(sleep_time));
        }
        i += 1;
    }

    for mut handle in handles {
        handle.wait()?;
    }

    Ok(())
}

pub fn launch_cmds(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<Vec<std::process::Command>, Box<dyn std::error::Error>> {
    let party = PATH_PARTY.to_string_lossy();
    let steam = PATH_STEAM.to_string_lossy();

    let win = h.win();
    let exec = h.exec.as_str();
    let runtime = h.runtime.as_str();
    let gamescope = match cfg.kbm_support {
        true => &format!("{}", BIN_GSC_KBM.to_string_lossy()),
        false => "gamescope",
    };

    if (runtime == "scout"
        && !PATH_STEAM
            .join("steamapps/common/SteamLinuxRuntime_soldier")
            .exists())
        || (runtime == "soldier"
            && !PATH_STEAM
                .join("steamapps/common/SteamLinuxRuntime_soldier")
                .exists())
    {
        return Err(format!("Steam Runtime {runtime} not found!").into());
    }

    let mut cmds: Vec<Command> = (0..instances.len())
        .map(|_| Command::new(gamescope))
        .collect();

    for (i, instance) in instances.iter().enumerate() {
        let gamedir = format!("{party}/tmp/game-{i}");

        if !PathBuf::from(&gamedir).join(exec).exists() {
            return Err(format!("Executable not found: {gamedir}/{exec}").into());
        }

        let path_prof = &format!("{party}/profiles/{}", instance.profname.as_str());
        let path_pfx = match cfg.proton_separate_pfxs {
            true => &format!("{party}/prefixes/{}", i + 1),
            false => &format!("{party}/prefixes/1"),
        };

        let cmd = &mut cmds[i];

        cmd.env("SDL_JOYSTICK_HIDAPI", "0");
        cmd.env("ENABLE_GAMESCOPE_WSI", "0");
        cmd.env("PROTON_DISABLE_HIDRAW", "1");
        if cfg.force_sdl && !win {
            let path_sdl = match h.is32bit {
                true => "/ubuntu12_32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0",
                false => "/ubuntu12_32/steam-runtime/usr/lib/x86_64-linux-gnu/libSDL2-2.0.so.0",
            };

            cmd.env("SDL_DYNAMIC_API", &format!("{steam}/{path_sdl}"));
        }
        if win {
            let protonpath = match cfg.proton_version.is_empty() {
                true => "GE-Proton",
                false => cfg.proton_version.as_str(),
            };

            cmd.env("WINEPREFIX", path_pfx);
            cmd.env("PROTON_VERB", "run");
            cmd.env("PROTONPATH", &protonpath);

            if !h.env.is_empty() {
                let env_vars: Vec<&str> = h.env.split_whitespace().collect();
                for env_var in env_vars {
                    if let Some((key, value)) = env_var.split_once('=') {
                        cmd.env(key, value);
                    }
                }
            }
        }

        // Gamescope args
        cmd.args([
            "-W",
            &instance.width.to_string(),
            "-H",
            &instance.height.to_string(),
        ]);
        if cfg.gamescope_sdl_backend {
            cmd.arg("--backend=sdl");
            cmd.arg(&format!("--display-index={}", instance.monitor));
        }
        if cfg.kbm_support {
            let mut instance_has_keyboard = false;
            let mut instance_has_mouse = false;
            let mut kbms = String::new();

            for d in &instance.devices {
                let dev_type = input_devices[*d].device_type;
                if dev_type == DeviceType::Keyboard {
                    instance_has_keyboard = true;
                } else if dev_type == DeviceType::Mouse {
                    instance_has_mouse = true;
                }
                if dev_type == DeviceType::Keyboard || dev_type == DeviceType::Mouse {
                    kbms.push_str(&format!("{},", input_devices[*d].path));
                }
            }

            if instance_has_keyboard {
                cmd.arg("--backend-disable-keyboard");
            }
            if instance_has_mouse {
                cmd.arg("--backend-disable-mouse");
            }
            if !kbms.is_empty() {
                cmd.arg(&format!("--libinput-hold-dev={}", kbms));
            }
        }
        cmd.arg("--");

        // Bwrap args
        cmd.arg("bwrap");
        cmd.arg("--die-with-parent");
        cmd.args(["--dev-bind", "/", "/"]);
        cmd.args(["--tmpfs", "/tmp"]);
        // Mask out any gamepads that aren't this player's
        for (d, dev) in input_devices.iter().enumerate() {
            if !dev.enabled
                || (!instance.devices.contains(&d) && dev.device_type == DeviceType::Gamepad)
            {
                let path = &dev.path;
                cmd.args(["--bind", "/dev/null", path]);
            }
        }

        if win {
            let path_pfx_user = format!("{path_pfx}/drive_c/users/steamuser");
            cmd.args(["--bind", &format!("{path_prof}/windata"), &path_pfx_user]);
        } else {
            let path_prof_home = format!("{path_prof}/home");
            cmd.env("HOME", &path_prof_home);
            // Steam runtime looks in HOME/.steam directory
            if PATH_HOME.join(".steam").exists() {
                cmd.args([
                    "--bind",
                    &PATH_HOME.join(".steam").to_string_lossy(),
                    &format!("{path_prof_home}/.steam"),
                ]);
            } else if PATH_HOME
                .join(".var/app/com.valvesoftware.Steam/.steam/steam")
                .exists()
            {
                cmd.args([
                    "--bind",
                    &PATH_HOME
                        .join(".var/app/com.valvesoftware.Steam/.steam/steam")
                        .to_string_lossy(),
                    &format!("{path_prof_home}/.steam"),
                ]);
            }
        }

        for subpath in &h.game_null_paths {
            let game_subpath = PathBuf::from(gamedir.clone()).join(subpath);
            if game_subpath.is_file() {
                cmd.args(["--bind", "/dev/null", &game_subpath.to_string_lossy()]);
            } else if game_subpath.is_dir() {
                cmd.args([
                    "--bind",
                    &PATH_PARTY.join("tmp/null").to_string_lossy(),
                    &game_subpath.to_string_lossy(),
                ]);
            }
        }

        if h.use_goldberg {
            if let Some(subpath) = h.locate_steamapi_path() {
                let src = match &h.win() {
                    true => match &h.is32bit {
                        true => PATH_RES.join("goldberg/steam_api.dll"),
                        false => PATH_RES.join("goldberg/steam_api64.dll"),
                    },
                    false => match &h.is32bit {
                        true => PATH_RES.join("goldberg/libsteam_api.so"),
                        false => PATH_RES.join("goldberg/libsteam_api64.so"),
                    },
                };
                let dest = PathBuf::from(&gamedir).join(subpath);
                cmd.args(["--bind", &src.to_string_lossy(), &dest.to_string_lossy()]);

                if let Some(parent) = dest.parent() {
                    cmd.args([
                        "--bind",
                        &PATH_PARTY.join("tmp/steam_settings").to_string_lossy(),
                        &parent.join("steam_settings").to_string_lossy(),
                    ]);
                }
            }
        }

        let path_profile_gse = format!("{path_prof}/steam");
        if win {
            let path_gse_win =
                format!("{path_pfx}/drive_c/users/steamuser/AppData/Roaming/GSE Saves");
            cmd.args(["--bind", &path_profile_gse, &path_gse_win]);
        } else {
            let path_gse_linux = format!("{path_prof}/home/.local/share/GSE Saves");
            cmd.args(["--bind", &path_profile_gse, &path_gse_linux]);
        }

        let path_exec = PathBuf::from(&gamedir).join(exec);
        let cwd = path_exec.parent().ok_or_else(|| "couldn't get parent")?;
        cmd.args(["--chdir", &cwd.to_string_lossy()]);

        // Runtime
        if win {
            cmd.arg(format!("{}", BIN_UMU_RUN.to_string_lossy()));
        } else {
            match runtime {
                "scout" => {
                    cmd.arg(format!("{steam}/ubuntu12_32/steam-runtime/run.sh"));
                }
                "soldier" => {
                    cmd.arg(format!(
                        "{steam}/steamapps/common/SteamLinuxRuntime_soldier/_v2-entry-point"
                    ));
                    cmd.arg("--");
                }
                _ => {}
            };
        }

        cmd.arg(format!("{gamedir}/{exec}"));

        let split_args: Vec<String> = h.args.split_whitespace().map(|s| s.to_string()).collect();
        for arg in split_args {
            let arg = match arg.as_str() {
                "$GAMEDIR" => &gamedir,
                "$PROFILE" => instance.profname.as_str(),
                "$WIDTH" => &format!("{}", instance.width),
                "$HEIGHT" => &format!("{}", instance.height),
                "$RESOLUTION" => &format!("{}x{}", instance.width, instance.height),
                _ => &arg.sanitize_path(),
            };
            cmd.arg(arg);
        }
    }

    return Ok(cmds);
}

fn print_launch_cmds(cmds: &Vec<Command>) {
    for (i, cmd) in cmds.iter().enumerate() {
        println!("[partydeck] INSTANCE {}:", i + 1);

        let cwd = cmd.get_current_dir().unwrap_or_else(|| Path::new(""));
        println!("[partydeck] CWD={}", cwd.display());

        for var in cmd.get_envs() {
            let value = var.1.ok_or_else(|| "").unwrap_or_default();
            println!(
                "[partydeck] {}={}",
                var.0.to_string_lossy(),
                value.display()
            );
        }

        println!("[partydeck] \"{}\"", cmd.get_program().display());

        print!("[partydeck] ");
        for arg in cmd.get_args() {
            let fmtarg = arg.to_string_lossy();
            if fmtarg == "--bind"
                || fmtarg == "bwrap"
                || (fmtarg.starts_with("/") && fmtarg.len() > 1)
            {
                print!("\n[partydeck] ");
            } else {
                print!(" ");
            }
            print!("\"{}\"", fmtarg);
        }

        println!("\n[partydeck] ---------------------");
    }
}

pub fn fuse_overlayfs_mount_gamedirs(
    h: &Handler,
    instances: &Vec<Instance>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = format!("{}/tmp", PATH_PARTY.to_string_lossy());
    let mut path_lowerdir = h.get_game_rootpath()?;

    if h.path_handler.join("overlay").exists() {
        path_lowerdir = format!(
            "{}:{}",
            h.path_handler.join("overlay").to_string_lossy(),
            path_lowerdir
        );
    }

    let gamename = if h.is_saved_handler() {
        h.handler_dir_name().to_string()
    } else {
        let dir = PathBuf::from(&path_lowerdir);
        dir.file_name()
            .ok_or_else(|| "No filename")?
            .to_string_lossy()
            .to_string()
    };

    let mut cmds: Vec<Command> = (0..instances.len())
        .map(|_| Command::new("fuse-overlayfs"))
        .collect();

    for i in 0..instances.len() {
        let cmd = &mut cmds[i];
        let instance = &instances[i];

        let path_game_mnt = format!("{tmp}/game-{i}");
        let path_workdir = format!("{tmp}/work-{i}");
        let path_prof = &format!(
            "{}/profiles/{}",
            PATH_PARTY.to_string_lossy(),
            instance.profname.as_str()
        );
        let path_upperdir = &format!("{path_prof}/gamesaves/{gamename}");

        std::fs::create_dir_all(&path_game_mnt)?;
        std::fs::create_dir_all(&path_workdir)?;

        cmd.args(&[
            "-o",
            &format!("lowerdir={}", path_lowerdir),
            "-o",
            &format!("upperdir={}", path_upperdir),
            "-o",
            &format!("workdir={}", path_workdir),
            &path_game_mnt,
        ]);
    }

    for cmd in &mut cmds {
        let status = cmd.status()?;
        if !status.success() {
            return Err("fuse-overlayfs mount failed.".into());
        }
    }

    Ok(())
}
