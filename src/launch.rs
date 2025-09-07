use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::PartyConfig;
use crate::game::Game;
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::launch::Game::{ExecRef, HandlerRef};
use crate::paths::*;
use crate::util::*;

pub fn launch_game(
    game: &Game,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if let HandlerRef(h) = game {
        for instance in instances {
            create_profile(instance.profname.as_str())?;
            create_gamesave(instance.profname.as_str(), &h)?;
        }
        if h.symlink_dir {
            create_symlink_folder(&h)?;
        }
    }

    println!("\n[partydeck] Instances:");
    for instance in instances {
        println!(
            "  - Profile: {}, Monitor: {}, Resolution: {}x{}",
            instance.profname, instance.monitor, instance.width, instance.height
        );
    }

    let new_cmds = launch_cmds(game, input_devices, instances, cfg)?;
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

    let mut sleep_time = match game.win() {
        true => 6.0,
        false => 0.5,
    };
    if let HandlerRef(h) = game
        && let Some(f) = h.pause_between_starts
    {
        sleep_time = f;
    }

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

    if cfg.enable_kwin_script {
        kwin_dbus_unload_script()?;
    }

    remove_guest_profiles()?;

    Ok(())
}

pub fn launch_cmds(
    game: &Game,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<Vec<std::process::Command>, Box<dyn std::error::Error>> {
    let home = PATH_HOME.display();
    let localshare = PATH_LOCAL_SHARE.display();
    let party = PATH_PARTY.display();
    let steam = PATH_STEAM.display();

    let gamedir = match game {
        ExecRef(e) => &format!(
            "{}",
            e.path()
                .parent()
                .ok_or_else(|| "Invalid path")?
                .to_string_lossy()
        ),
        HandlerRef(h) => match h.symlink_dir {
            true => &format!("{party}/gamesyms/{}", h.uid),
            false => &get_rootpath_handler(&h)?,
        },
    };

    let win = game.win();

    let gamescope = match cfg.kbm_support {
        true => &format!("{}", BIN_GSC_KBM.to_string_lossy()),
        false => "gamescope",
    };

    let exec = match game {
        ExecRef(e) => &e.filename(),
        HandlerRef(h) => h.exec.as_str(),
    };

    if !PathBuf::from(gamedir).join(exec).exists() {
        return Err(format!("Executable not found: {gamedir}/{exec}").into());
    }

    let runtime = if let HandlerRef(h) = game {
        h.runtime.as_str()
    } else {
        ""
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
        let path_prof = &format!("{party}/profiles/{}", instance.profname.as_str());
        let path_save = match game {
            ExecRef(_) => "",
            HandlerRef(h) => &format!("{path_prof}/saves/{}", h.uid.as_str()),
        };
        let path_pfx = match cfg.proton_separate_pfxs {
            true => &format!("{party}/pfx{}", i + 1),
            false => &format!("{party}/pfx"),
        };

        let cmd = &mut cmds[i];

        cmd.current_dir(gamedir);

        cmd.env("SDL_JOYSTICK_HIDAPI", "0");
        cmd.env("ENABLE_GAMESCOPE_WSI", "0");
        cmd.env("PROTON_DISABLE_HIDRAW", "1");
        if cfg.force_sdl && !win {
            let mut path_sdl =
                "/ubuntu12_32/steam-runtime/usr/lib/x86_64-linux-gnu/libSDL2-2.0.so.0";
            if let HandlerRef(h) = game {
                if h.is32bit {
                    path_sdl = "/ubuntu12_32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0";
                }
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

            if let HandlerRef(h) = game {
                if !h.dll_overrides.is_empty() {
                    let mut overrides = String::new();
                    for dll in &h.dll_overrides {
                        overrides.push_str(&format!("{dll},"));
                    }
                    overrides.push_str("=n,b\" ");

                    cmd.env("WINEDLLOVERRIDES", &overrides);
                }
                if h.coldclient {
                    cmd.env("PROTON_DISABLE_LSTEAMCLIENT", "1");
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
            let path_appdata = format!("{path_pfx}/drive_c/users/steamuser/AppData");
            let path_documents = format!("{path_pfx}/drive_c/users/steamuser/Documents");
            cmd.args(["--bind", &format!("{path_prof}/AppData"), &path_appdata]);
            cmd.args(["--bind", &format!("{path_prof}/Documents"), &path_documents]);
        } else {
            let path_localshare = format!("{localshare}");
            let path_config = format!("{home}/.config");
            cmd.args(["--bind", &format!("{path_prof}/share"), &path_localshare]);
            cmd.args(["--bind", &format!("{path_prof}/config"), &path_config]);
            cmd.args(["--bind", &format!("{party}"), &format!("{party}")]);
            cmd.args(["--bind", &format!("{steam}"), &format!("{steam}")]);
        }
        if let HandlerRef(h) = game {
            for subdir in &h.game_unique_paths {
                let prof_subdir = format!("{path_save}/{subdir}");
                let game_subdir = format!("{gamedir}/{subdir}");
                cmd.args(["--bind", &prof_subdir, &game_subdir]);
            }

            let path_goldberg = h.path_goldberg.as_str();
            if !path_goldberg.is_empty() {
                let path_profile_gse = format!("{path_prof}/steam");
                let path_gse_linux = format!("{localshare}/GSE Saves");
                let path_gse_win =
                    format!("{path_pfx}/drive_c/users/steamuser/AppData/Roaming/GSE Saves");
                if win {
                    cmd.args(["--bind", &path_profile_gse, &path_gse_win]);
                } else {
                    cmd.args(["--bind", &path_profile_gse, &path_gse_linux]);
                }
            }
        }

        // Runtime
        if win {
            cmd.arg(format!("{}", BIN_UMU_RUN.to_string_lossy()));
        } else {
            match runtime {
                "scout" => {
                    cmd.arg(format!(
                        "{steam}/steamapps/common/SteamLinuxRuntime/scout-on-soldier-entry-point-v2"
                    ));
                    cmd.arg("--");
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

        if let HandlerRef(h) = game {
            for arg in &h.args {
                let arg = match arg.as_str() {
                    "$GAMEDIR" => &gamedir,
                    "$PROFILE" => instance.profname.as_str(),
                    "$WIDTH" => &format!("{}", instance.width),
                    "$HEIGHT" => &format!("{}", instance.height),
                    "$WIDTHXHEIGHT" => &format!("{}x{}", instance.width, instance.height),
                    _ => &arg.sanitize_path(),
                };
                cmd.arg(arg);
            }
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
