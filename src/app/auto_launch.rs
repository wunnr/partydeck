use std::thread::sleep;

use super::config::*;
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::launch::*;
use crate::monitor::Monitor;
use crate::profiles::*;
use crate::util::*;

use eframe::egui;

pub struct AutoLaunchApp {
    handler: Handler,
    monitors: Vec<Monitor>,
    input_devices: Vec<InputDevice>,
    instances: Vec<Instance>,
    options: PartyConfig,
    loading_msg: Option<String>,
    loading_since: Option<std::time::Instant>,
    task: Option<std::thread::JoinHandle<()>>,
}

impl AutoLaunchApp {
    pub fn new(monitors: Vec<Monitor>, handler: Handler) -> Self {
        let options = load_cfg();
        let input_devices = scan_input_devices(&options.pad_filter_type);

        Self {
            handler,
            monitors,
            input_devices,
            instances: Vec::new(),
            options,
            loading_msg: None,
            loading_since: None,
            task: None,
        }
    }

    fn handle_input_auto_mode(&mut self, raw_input: &egui::RawInput) {
        // Check for keyboard Enter key
        if raw_input.events.iter().any(|e| {
            matches!(e, egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            })
        }) && self.instances.len() > 0 {
            self.prepare_auto_launch();
            return;
        }

        let mut i = 0;
        while i < self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                i += 1;
                continue;
            }
            match self.input_devices[i].poll() {
                Some(PadButton::ABtn) | Some(PadButton::ZKey) | Some(PadButton::RightClick) => {
                    if self.input_devices[i].device_type() != DeviceType::Gamepad
                        && !self.options.kbm_support
                    {
                        i += 1;
                        continue;
                    }
                    if !self.options.allow_multiple_instances_on_same_device
                        && self.is_device_in_any_instance(i)
                    {
                        i += 1;
                        continue;
                    }
                    // Prevent same keyboard/mouse device in multiple instances
                    if self.input_devices[i].device_type() != DeviceType::Gamepad
                        && self.is_device_in_any_instance(i)
                    {
                        i += 1;
                        continue;
                    }

                    // Only allow up to 4 players
                    if self.instances.len() >= 4 {
                        i += 1;
                        continue;
                    }

                    // Create new instance with auto-assigned profile name
                    let profname = format!("Player{}", self.instances.len() + 1);
                    self.instances.push(Instance {
                        devices: vec![i],
                        profname,
                        profselection: 0,
                        monitor: 0,
                        width: 0,
                        height: 0,
                    });
                }
                Some(PadButton::StartBtn) => {
                    if self.instances.len() > 0 && self.is_device_in_any_instance(i) {
                        self.prepare_auto_launch();
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn is_device_in_any_instance(&self, dev: usize) -> bool {
        for instance in &self.instances {
            if instance.devices.contains(&dev) {
                return true;
            }
        }
        false
    }

    fn render_player_slot(&self, ui: &mut egui::Ui, slot: usize) {
        let instance = self.instances.get(slot);

        let frame = if instance.is_some() {
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(40, 60, 80))
                .inner_margin(16.0)
        } else {
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(20, 20, 20))
                .inner_margin(16.0)
        };

        frame.show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(format!("Player {}", slot + 1));

                if let Some(inst) = instance {
                    if let Some(&dev_idx) = inst.devices.first() {
                        let dev = &self.input_devices[dev_idx];
                        ui.label(egui::RichText::new(dev.emoji()).size(48.0));
                        ui.label(dev.fancyname());
                    }
                } else {
                    ui.label(egui::RichText::new("Press A/Z/Right-Click to Join").weak());
                }
            });
        });
    }

    pub fn prepare_auto_launch(&mut self) {
        if self.options.gamescope_sdl_backend {
            set_instance_resolutions_multimonitor(
                &mut self.instances,
                &self.monitors,
                &self.options,
            );
        } else {
            set_instance_resolutions(&mut self.instances, &self.monitors[0], &self.options);
        }

        let handler = self.handler.clone();
        let instances = self.instances.clone();
        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();
        let cfg = self.options.clone();

        self.spawn_task(
            "Launching...\n\nDon't press any buttons or move any analog sticks or mice.",
            move || {
                sleep(std::time::Duration::from_secs_f32(1.5));

                // Create profiles for filled slots
                for instance in &instances {
                    if let Err(e) = create_profile(&instance.profname) {
                        eprintln!("[partydeck] Failed to create profile: {}", e);
                        msg("Profile Error", &format!("Failed to create profile: {}", e));
                        return;
                    }
                }

                if let Err(err) = setup_profiles(&handler, &instances) {
                    println!("[partydeck] Error setting up profiles: {}", err);
                    msg("Failed setting up profiles", &format!("{err}"));
                    return;
                }
                if handler.is_saved_handler()
                    && !cfg.disable_mount_gamedirs
                    && let Err(err) = fuse_overlayfs_mount_gamedirs(&handler, &instances)
                {
                    println!("[partydeck] Error mounting game directories: {}", err);
                    msg("Failed mounting game directories", &format!("{err}"));
                    return;
                }
                if let Err(err) = launch_game(&handler, &dev_infos, &instances, &cfg) {
                    println!("[partydeck] Error launching instances: {}", err);
                    msg("Launch Error", &format!("{err}"));
                }
                if cfg.enable_kwin_script {
                    if let Err(err) = kwin_dbus_unload_script() {
                        println!("[partydeck] Error unloading KWin script: {}", err);
                    }
                }
                if let Err(err) = remove_guest_profiles() {
                    println!("[partydeck] Error removing guest profiles: {}", err);
                }
                if let Err(err) = clear_tmp() {
                    println!("[partydeck] Error removing tmp directory: {}", err);
                }
            },
        );
    }

    pub fn spawn_task<F>(&mut self, msg: &str, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.loading_msg = Some(msg.to_string());
        self.loading_since = Some(std::time::Instant::now());
        self.task = Some(std::thread::spawn(f));
    }
}

impl eframe::App for AutoLaunchApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.focused || self.task.is_some() {
            return;
        }
        self.handle_input_auto_mode(raw_input);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }

            // Render 2x2 grid
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    self.render_player_slot(ui, 0);
                    ui.add_space(8.0);
                    self.render_player_slot(ui, 2);
                });
                cols[1].vertical(|ui| {
                    self.render_player_slot(ui, 1);
                    ui.add_space(8.0);
                    self.render_player_slot(ui, 3);
                });
            });

            if !self.instances.is_empty() {
                ui.separator();
                ui.label("Press START or ENTER to begin");
            }
        });

        // Loading overlay
        if let Some(handle) = self.task.take() {
            if handle.is_finished() {
                let _ = handle.join();
                self.loading_since = None;
                self.loading_msg = None;
            } else {
                self.task = Some(handle);
            }
        }
        if let Some(start) = self.loading_since {
            if start.elapsed() > std::time::Duration::from_secs(60) {
                self.loading_msg = Some("Operation timed out".to_string());
            }
        }
        if let Some(msg) = &self.loading_msg {
            egui::Area::new("loading".into())
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 192))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::widgets::Spinner::new().size(40.0));
                                ui.add_space(8.0);
                                ui.label(msg);
                            });
                        });
                });
        }

        if ctx.input(|input| input.focused) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}