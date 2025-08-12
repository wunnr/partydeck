use std::path::PathBuf;

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

    let cmd = launch_cmd(game, input_devices, instances, cfg)?;
    println!("\nCOMMAND:\n{}\n", cmd);

    if cfg.enable_kwin_script {
        let script = if instances.len() == 2 && cfg.vertical_two_player {
            "splitscreen_kwin_vertical.js"
        } else {
            "splitscreen_kwin.js"
        };

        kwin_dbus_start_script(PATH_RES.join(script))?;
    }

    std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .status()?;

    if cfg.enable_kwin_script {
        kwin_dbus_unload_script()?;
    }

    remove_guest_profiles()?;

    Ok(())
}

pub fn launch_cmd(
    game: &Game,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<String, Box<dyn std::error::Error>> {
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

    let win = match game {
        ExecRef(e) => e.path().extension().unwrap_or_default() == "exe",
        HandlerRef(h) => h.win,
    };

    let mut cmd = String::new();
    // Command: "gamescope [settings] -- bwrap [binds] [runtime] [exec] [args] & ..."
    cmd.push_str("export ");
    cmd.push_str("SDL_JOYSTICK_HIDAPI=0 ");
    cmd.push_str("ENABLE_GAMESCOPE_WSI=0 ");
    cmd.push_str("PROTON_DISABLE_HIDRAW=1 ");

    if cfg.force_sdl && !win {
        let mut path_sdl = "/ubuntu12_32/steam-runtime/usr/lib/x86_64-linux-gnu/libSDL2-2.0.so.0";
        if let HandlerRef(h) = game {
            if h.is32bit {
                path_sdl = "/ubuntu12_32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0";
            }
        };
        cmd.push_str(&format!("SDL_DYNAMIC_API=\"{steam}/{path_sdl}\" "));
    }
    if win {
        let protonpath = match cfg.proton_version.is_empty() {
            true => "GE-Proton",
            false => cfg.proton_version.as_str(),
        };
        cmd.push_str(&format!("PROTON_VERB=run "));
        cmd.push_str(&format!("PROTONPATH={protonpath} "));

        if let HandlerRef(h) = game {
            if !h.dll_overrides.is_empty() {
                cmd.push_str("WINEDLLOVERRIDES=\"");
                for dll in &h.dll_overrides {
                    cmd.push_str(&format!("{dll},"));
                }
                cmd.push_str("=n,b\" ");
            }
            if h.coldclient {
                cmd.push_str("PROTON_DISABLE_LSTEAMCLIENT=1 ");
            }
        }
    }
    cmd.push_str("; ");

    let runtime = match win {
        // UMU CHANGE
        true => &format!("{}", BIN_UMU_RUN.to_string_lossy()),
        false => {
            if let HandlerRef(h) = game {
                match h.runtime.as_str() {
                    "scout" => &format!("\"{steam}/ubuntu12_32/steam-runtime/run.sh\""),
                    "soldier" => &format!(
                        "\"{steam}/steamapps/common/SteamLinuxRuntime_soldier/_v2-entry-point\""
                    ),
                    _ => "",
                }
            } else {
                ""
            }
        }
    };

    let exec = match game {
        ExecRef(e) => &e.filename(),
        HandlerRef(h) => h.exec.as_str(),
    };

    if !PathBuf::from(gamedir).join(exec).exists() {
        return Err(format!("Executable not found: {gamedir}/{exec}").into());
    }

    if let HandlerRef(h) = game {
        if h.runtime == "scout" && !PATH_STEAM.join("ubuntu12_32/steam-runtime/run.sh").exists() {
            return Err("Steam Scout Runtime not found".into());
        } else if h.runtime == "soldier"
            && !PATH_STEAM
                .join("steamapps/common/SteamLinuxRuntime_soldier")
                .exists()
        {
            return Err("Steam Soldier Runtime not found".into());
        }
    }

    cmd.push_str(&format!("cd \"{gamedir}\"; "));

    for (i, instance) in instances.iter().enumerate() {
        let path_prof = &format!("{party}/profiles/{}", instance.profname.as_str());
        let path_save = match game {
            ExecRef(_) => "",
            HandlerRef(h) => &format!("{path_prof}/saves/{}", h.uid.as_str()),
        };

        let pfx = match cfg.proton_separate_pfxs {
            true => &format!("{party}/pfx{}", i + 1),
            false => &format!("{party}/pfx"),
        };
        if win {
            cmd.push_str(&format!("WINEPREFIX={pfx} "));
        }

        let (gsc_width, gsc_height) = (instance.width, instance.height);

        let gsc_sdl = match cfg.gamescope_sdl_backend {
            true => "--backend=sdl",
            false => "",
        };

        let gamescope = match cfg.kbm_support {
            true => &format!("{}", BIN_GSC_KBM.to_string_lossy()),
            false => "gamescope",
        };

        cmd.push_str(&format!(
            "{gamescope} -W {gsc_width} -H {gsc_height} {gsc_sdl} "
        ));

        if cfg.kbm_support {
            let mut instance_has_keyboard = false;
            let mut instance_has_mouse = false;
            let mut kbms = String::new();

            for d in &instance.devices {
                if input_devices[*d].device_type == DeviceType::Keyboard {
                    instance_has_keyboard = true;
                } else if input_devices[*d].device_type == DeviceType::Mouse {
                    instance_has_mouse = true;
                }
                if input_devices[*d].device_type == DeviceType::Keyboard
                    || input_devices[*d].device_type == DeviceType::Mouse
                {
                    kbms.push_str(&format!("{},", input_devices[*d].path));
                }
            }

            if instance_has_keyboard {
                cmd.push_str("--backend-disable-keyboard ");
            }
            if instance_has_mouse {
                cmd.push_str("--backend-disable-mouse ");
            }
            if !kbms.is_empty() {
                cmd.push_str(&format!("--libinput-hold-dev {} ", kbms));
            }
        }

        cmd.push_str(&format!("-- "));

        cmd.push_str(&format!(
            "bwrap --die-with-parent --dev-bind / / --tmpfs /tmp "
        ));

        // Bind player profile directories to the game's directories
        let mut binds = String::new();

        // Mask out any gamepads that aren't this player's
        for (d, dev) in input_devices.iter().enumerate() {
            if !dev.enabled
                || (!instance.devices.contains(&d) && dev.device_type == DeviceType::Gamepad)
            {
                let path = &dev.path;
                binds.push_str(&format!("--bind /dev/null {path} "));
            }
        }

        if let HandlerRef(h) = game {
            let path_goldberg = h.path_goldberg.as_str();
            if !path_goldberg.is_empty() {
                binds.push_str(&format!(
                    "--bind \"{path_prof}/steam\" \"{gamedir}/{path_goldberg}/goldbergsave\" "
                ));
            }
            if h.win {
                let path_windata = format!("{pfx}/drive_c/users/steamuser/");
                if h.win_unique_appdata {
                    binds.push_str(&format!(
                        "--bind \"{path_save}/_AppData\" \"{path_windata}/AppData\" "
                    ));
                }
                if h.win_unique_documents {
                    binds.push_str(&format!(
                        "--bind \"{path_save}/_Documents\" \"{path_windata}/Documents\" "
                    ));
                }
            } else {
                if h.linux_unique_localshare {
                    binds.push_str(&format!("--bind \"{path_save}/_share\" \"{localshare}\" "));
                }
                if h.linux_unique_config {
                    binds.push_str(&format!(
                        "--bind \"{path_save}/_config\" \"{home}/.config\" "
                    ));
                }
            }
            for subdir in &h.game_unique_paths {
                binds.push_str(&format!(
                    "--bind \"{path_save}/{subdir}\" \"{gamedir}/{subdir}\" "
                ));
            }
        }

        let args = match game {
            HandlerRef(h) => h
                .args
                .iter()
                .map(|arg| match arg.as_str() {
                    "$GAMEDIR" => format!(" \"{gamedir}\""),
                    "$PROFILE" => format!(" \"{}\"", instance.profname.as_str()),
                    "$WIDTH" => format!(" {gsc_width}"),
                    "$HEIGHT" => format!(" {gsc_height}"),
                    "$WIDTHXHEIGHT" => format!(" \"{gsc_width}x{gsc_height}\""),
                    _ => format!(" {arg}"),
                })
                .collect::<String>(),
            ExecRef(e) => e.args.clone().sanitize_path(),
        };

        cmd.push_str(&format!("{binds} {runtime} \"{gamedir}/{exec}\" {args} "));

        if i < instances.len() - 1 {
            // Proton games need a ~5 second buffer in-between launches
            // TODO: investigate why this is
            match win {
                true => cmd.push_str("& sleep 6; "),
                false => cmd.push_str("& sleep 0.01; "),
            }
        }
    }

    Ok(cmd)
}
