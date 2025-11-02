use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::PartyConfig;
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::paths::*;
use crate::profiles::{create_profile, create_profile_gamesave};
use crate::util::*;

pub fn setup_profiles(
    h: &Handler,
    instances: &Vec<Instance>,
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
            "[partydeck] - Profile: {}, Monitor: {}, Resolution: {}x{}",
            instance.profname, instance.monitor, instance.width, instance.height
        );
    }

    Ok(())
}

pub fn launch_game(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_cmds = launch_cmds(h, input_devices, instances, cfg)?;
    print_launch_cmds(&new_cmds);

    if h.use_goldberg
        && let Some(appid) = h.steam_appid
    {
        let tmp_dir = PATH_PARTY.join("tmp");
        if !tmp_dir.exists() {
            std::fs::create_dir_all(tmp_dir.clone())?;
        }
        std::fs::write(tmp_dir.join("steam_appid.txt"), appid.to_string())?;
    }

    if cfg.enable_kwin_script {
        let script = match cfg.vertical_two_player {
            true => "splitscreen_kwin_vertical.js",
            false => "splitscreen_kwin.js",
        };

        kwin_dbus_start_script(PATH_RES.join(script))?;
    }

    let sleep_time = match h.pause_between_starts {
        Some(f) => f,
        None => 0.5,
    };

    let mut handles = Vec::new();

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
    let win = h.win();
    let exec = Path::new(&h.exec);
    let runtime = h.runtime.as_str();
    let gamescope = match cfg.kbm_support {
        true => BIN_GSC_KBM.as_path(),
        false => Path::new("gamescope"),
    };

    if (runtime == "scout" && !PATH_STEAM.join("bin32/steam-runtime/run.sh").exists())
        || (runtime == "soldier"
            && !PATH_STEAM
                .join("steam/steamapps/common/SteamLinuxRuntime_soldier")
                .exists())
    {
        return Err(format!("Steam Runtime {runtime} not found!").into());
    }

    let mut cmds: Vec<Command> = (0..instances.len())
        .map(|_| Command::new(gamescope))
        .collect();

    for (i, instance) in instances.iter().enumerate() {
        let gamedir = if h.is_saved_handler() && !cfg.disable_mount_gamedirs {
            PATH_PARTY.join("tmp").join(format!("game-{}", i))
        } else {
            PathBuf::from(h.get_game_rootpath()?)
        };

        if !gamedir.join(exec).exists() {
            return Err(format!("Executable not found: {}", gamedir.join(exec).display()).into());
        }

        let path_exec = gamedir.join(exec);
        let cwd = path_exec.parent().ok_or_else(|| "couldn't get parent")?;

        let path_prof = PATH_PARTY.join("profiles").join(&instance.profname);
        let path_pfx = PATH_PARTY
            .join("prefixes")
            .join(match cfg.proton_separate_pfxs {
                true => (i + 1).to_string(),
                false => "1".to_string(),
            });

        let cmd = &mut cmds[i];

        cmd.current_dir(cwd);

        cmd.env("SDL_JOYSTICK_HIDAPI", "0");
        cmd.env("ENABLE_GAMESCOPE_WSI", "0");
        cmd.env("PROTON_DISABLE_HIDRAW", "1");
        if h.sdl2_override != SDL2Override::No {
            let path_sdl = match h.sdl2_override {
                SDL2Override::Srt => {
                    PATH_STEAM.join("bin32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0")
                }
                SDL2Override::Sys => PathBuf::from("/usr/lib/libSDL2.so"),
                _ => PathBuf::new(),
            };
            cmd.env("SDL_DYNAMIC_API", path_sdl);
        }
        if win {
            let protonpath = match cfg.proton_version.is_empty() {
                true => "GE-Proton",
                false => &cfg.proton_version,
            };

            cmd.env("WINEPREFIX", &path_pfx);
            cmd.env("PROTON_VERB", "run");
            cmd.env("PROTONPATH", protonpath);
        }
        if !h.env.is_empty() {
            for env_var in h.env.split_whitespace() {
                if let Some((key, value)) = env_var.split_once('=') {
                    cmd.env(key, value);
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
        if cfg.gamescope_force_grab_cursor {
            cmd.arg("--force-grab-cursor");
        }
        if cfg.gamescope_sdl_backend {
            cmd.arg("--backend=sdl");
            cmd.arg(format!("--display-index={}", instance.monitor));
        }
        if cfg.kbm_support {
            let mut instance_has_keyboard = false;
            let mut instance_has_mouse = false;
            let mut kbms = String::new();

            for &d in &instance.devices {
                let dev = &input_devices[d];
                if dev.device_type == DeviceType::Keyboard {
                    instance_has_keyboard = true;
                } else if dev.device_type == DeviceType::Mouse {
                    instance_has_mouse = true;
                }
                if dev.device_type == DeviceType::Keyboard || dev.device_type == DeviceType::Mouse {
                    kbms.push_str(&format!("{},", &dev.path));
                }
            }

            if instance_has_keyboard {
                cmd.arg("--backend-disable-keyboard");
            }
            if instance_has_mouse {
                cmd.arg("--backend-disable-mouse");
            }
            if !kbms.is_empty() {
                cmd.arg(format!("--libinput-hold-dev={}", kbms));
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
                cmd.args(["--bind", "/dev/null", &dev.path]);
            }
        }

        if win {
            let path_pfx_user = path_pfx.join("drive_c/users/steamuser");
            cmd.arg("--bind")
                .args([&path_prof.join("windata"), &path_pfx_user]);
        } else {
            let path_prof_home = path_prof.join("home");
            cmd.env("HOME", &path_prof_home);
            // Also bind the Steam directory as the Steam runtimes look for HOME/.steam
            cmd.args([
                "--bind",
                &PATH_STEAM.to_string_lossy(),
                &path_prof_home.join(".steam").to_string_lossy(),
            ]);
        }

        for subpath in &h.game_null_paths {
            let game_subpath = gamedir.join(subpath);
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
            cmd.env("GseAppPath", PATH_RES.join("goldberg"));
            cmd.env("GseSavePath", path_prof.join("steam"));
            cmd.env("SteamAppUser", instance.profname.clone());
            cmd.env("SteamUser", instance.profname.clone());
            cmd.env("SteamClientLaunch", "1");
            cmd.env("SteamEnv", "1");
            if let Some(appid) = h.steam_appid {
                cmd.env("SteamAppId", &appid.to_string());
                cmd.env("SteamGameId", &appid.to_string());
                cmd.arg("--bind").args([
                    PATH_PARTY.join("tmp/steam_appid.txt"),
                    cwd.join("steam_appid.txt"),
                ]);
            }

            cmd.arg("--bind").args([
                PATH_RES.join("goldberg/linux32"),
                std::fs::read_link(PATH_STEAM.join("sdk32"))?,
            ]);

            cmd.arg("--bind").args([
                PATH_RES.join("goldberg/linux64"),
                std::fs::read_link(PATH_STEAM.join("sdk64"))?,
            ]);

            if win {
                cmd.arg("--bind").args([
                    PATH_RES.join("goldberg/win"),
                    path_pfx.join("drive_c/Program Files (x86)/Steam"),
                ]);
            }
        }

        // Runtime
        if win {
            cmd.arg(&*BIN_UMU_RUN);
        } else {
            match runtime {
                "scout" => {
                    cmd.arg(PATH_STEAM.join("bin32/steam-runtime/run.sh"));
                }
                "soldier" => {
                    cmd.arg(
                        PATH_STEAM.join(
                            "steam/steamapps/common/SteamLinuxRuntime_soldier/_v2-entry-point",
                        ),
                    );
                    cmd.arg("--");
                }
                _ => {}
            };
        }

        cmd.arg(&path_exec);

        for arg in h.args.split_whitespace() {
            let processed_arg = match arg {
                "$PROFILE" => &instance.profname,
                "$WIDTH" => &instance.width.to_string(),
                "$HEIGHT" => &instance.height.to_string(),
                "$RESOLUTION" => &format!("{}x{}", instance.width, instance.height),
                "$INSTANCECOUNT" => &instances.len().to_string(),
                "$INSTANCENUM" => &i.to_string(),
                "$GAMEDIR" => &gamedir.os_fmt(win),
                "$HANDLERDIR" => &h.path_handler.os_fmt(win),
                _ => &String::from(arg).sanitize_path(),
            };
            cmd.arg(processed_arg);
        }
    }

    Ok(cmds)
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
    let tmp_dir = PATH_PARTY.join("tmp");
    let mut path_lowerdir = h.get_game_rootpath()?;

    let overlay_path = h.path_handler.join("overlay");
    if overlay_path.exists() {
        path_lowerdir = format!("{}:{}", overlay_path.display(), path_lowerdir);
    }

    let gamename = h.handler_dir_name().to_string();

    let mut cmds: Vec<Command> = (0..instances.len())
        .map(|_| Command::new("fuse-overlayfs"))
        .collect();

    for (i, instance) in instances.iter().enumerate() {
        let cmd = &mut cmds[i];

        let path_game_mnt = tmp_dir.join(format!("game-{}", i));
        let path_workdir = tmp_dir.join(format!("work-{}", i));
        let path_prof = PATH_PARTY.join("profiles").join(&instance.profname);
        let path_upperdir = path_prof.join("gamesaves").join(&gamename);

        std::fs::create_dir_all(&path_game_mnt)?;
        std::fs::create_dir_all(&path_workdir)?;

        cmd.arg("-o");
        cmd.arg(format!("lowerdir={}", path_lowerdir));
        cmd.arg("-o");
        cmd.arg(format!("upperdir={}", path_upperdir.display()));
        cmd.arg("-o");
        cmd.arg(format!("workdir={}", path_workdir.display()));
        cmd.arg(&path_game_mnt);
    }

    for cmd in &mut cmds {
        let status = cmd
            .status()
            .map_err(|_| "Fuse-overlayfs executable not found; Please install fuse-overlayfs through your distro's package manager. If you already have it installed (or are on SteamOS, where it should be pre-installed), open up an issue on the GitHub.")?;
        if !status.success() {
            return Err("fuse-overlayfs mount failed.".into());
        }
    }

    Ok(())
}
