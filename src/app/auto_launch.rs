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
    waiting_for_device: Option<usize>,
    profiles: Vec<String>,
}

impl AutoLaunchApp {
    pub fn new(monitors: Vec<Monitor>, handler: Handler) -> Self {
        let options = load_cfg();
        let input_devices = scan_input_devices(&options.pad_filter_type);
        let profiles = scan_profiles(false);

        Self {
            handler,
            monitors,
            input_devices,
            instances: Vec::new(),
            options,
            loading_msg: None,
            loading_since: None,
            task: None,
            waiting_for_device: None,
            profiles,
        }
    }

    fn handle_input(&mut self, raw_input: &egui::RawInput) {
        if Self::has_key_event(raw_input, egui::Key::Enter) && self.instances.len() > 0 {
            self.prepare_auto_launch();
            return;
        }

        if self.profiles.len() > 1 {
            for (dev_idx, dev) in self.input_devices.iter().enumerate() {
                if dev.device_type() == DeviceType::Keyboard && dev.enabled() {
                    if let Some((player_idx, _)) = self.find_device_in_instance(dev_idx) {
                        if Self::has_key_event(raw_input, egui::Key::ArrowLeft) {
                            self.cycle_profile(player_idx, -1);
                            break;
                        } else if Self::has_key_event(raw_input, egui::Key::ArrowRight) {
                            self.cycle_profile(player_idx, 1);
                            break;
                        }
                    }
                }
            }
        }

        for i in 0..self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                continue;
            }

            match self.input_devices[i].poll() {
                Some(PadButton::ABtn) | Some(PadButton::ZKey) | Some(PadButton::RightClick) => {
                    if !self.can_add_device_to_instance(i) {
                        continue;
                    }

                    if let Some(instance_idx) = self.waiting_for_device {
                        self.instances[instance_idx].devices.push(i);
                        self.waiting_for_device = None;
                        continue;
                    }

                    let (profname, profselection) = if self.profiles.is_empty() {
                        let name = format!("Player{}", self.instances.len() + 1);
                        let _ = create_profile(&name);
                        (name, 0)
                    } else {
                        let profile_idx = self.find_next_unassigned_profile().unwrap_or(0);
                        (self.profiles[profile_idx].clone(), profile_idx)
                    };

                    self.instances.push(Instance {
                        devices: vec![i],
                        profname,
                        profselection,
                        monitor: 0,
                        width: 0,
                        height: 0,
                    });
                }
                Some(PadButton::Left) | Some(PadButton::Right) => {
                    let direction = if matches!(self.input_devices[i].poll(), Some(PadButton::Left)) { -1 } else { 1 };
                    if let Some((player_idx, _)) = self.find_device_in_instance(i) {
                        if self.profiles.len() > 1 {
                            self.cycle_profile(player_idx, direction);
                        }
                    }
                }
                Some(PadButton::StartBtn) => {
                    if self.instances.len() > 0 && is_device_in_any_instance(&self.instances, i) {
                        self.prepare_auto_launch();
                    }
                }
                _ => {}
            }
        }
    }

    fn has_key_event(raw_input: &egui::RawInput, key: egui::Key) -> bool {
        raw_input.events.iter().any(|e| {
            matches!(e, egui::Event::Key {
                key: k,
                pressed: true,
                ..
            } if *k == key)
        })
    }

    fn can_add_device_to_instance(&self, dev_idx: usize) -> bool {
        let device = &self.input_devices[dev_idx];

        if !device.enabled() {
            return false;
        }

        if device.device_type() != DeviceType::Gamepad && !self.options.kbm_support {
            return false;
        }

        if is_device_in_any_instance(&self.instances, dev_idx) {
            if self.waiting_for_device.is_none() {
                return false;
            }
            if !self.options.allow_multiple_instances_on_same_device {
                return false;
            }
            if device.device_type() != DeviceType::Gamepad {
                return false;
            }
        }

        if self.instances.len() >= 4 {
            return false;
        }

        true
    }

    fn find_next_unassigned_profile(&self) -> Option<usize> {
        if self.profiles.is_empty() {
            return None;
        }
        
        let assigned_indices: std::collections::HashSet<usize> = self.instances
            .iter()
            .map(|instance| instance.profselection)
            .collect();
        
        for (idx, _) in self.profiles.iter().enumerate() {
            if !assigned_indices.contains(&idx) {
                return Some(idx);
            }
        }
        
        Some(0)
    }

    fn find_device_in_instance(&self, dev: usize) -> Option<(usize, usize)> {
        for (player_idx, instance) in self.instances.iter().enumerate() {
            for (device_idx, &device) in instance.devices.iter().enumerate() {
                if device == dev {
                    return Some((player_idx, device_idx));
                }
            }
        }
        None
    }

    fn cycle_profile(&mut self, player_idx: usize, direction: i32) {
        if self.profiles.is_empty() {
            return;
        }

        let instance = &mut self.instances[player_idx];
        let current = instance.profselection;
        let len = self.profiles.len();

        let new_idx = if direction > 0 {
            (current + 1) % len
        } else {
            current.checked_sub(1).unwrap_or(len - 1)
        };

        instance.profselection = new_idx;
        instance.profname = self.profiles[new_idx].clone();
    }

    fn render_instruction_box(&self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(10, 10, 15))
            .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 255, 255)))
            .corner_radius(16.0)
            .inner_margin(egui::Margin::symmetric(32, 28))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("PRESS INPUT TO ADD PLAYER")
                            .size(26.0)
                            .strong()
                            .color(egui::Color32::from_rgb(255, 255, 255)),
                    );
                    
                    ui.add_space(20.0);
                    
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - (56.0 * 3.0 + 24.0 * 2.0)) / 2.0);
                        ui.label(egui::RichText::new("🎮").size(56.0));
                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("🖮").size(56.0));
                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("🖱").size(56.0));
                    });
                });
            });
    }

    fn render_player_boxes(&mut self, ui: &mut egui::Ui) {
        if self.instances.is_empty() {
            return;
        }

        let mut profile_actions: Vec<(usize, i32)> = Vec::new();
        let mut waiting_actions: Vec<Option<usize>> = Vec::new();

        ui.horizontal_centered(|ui| {
            for (idx, instance) in self.instances.iter().enumerate() {
                if idx > 0 {
                    ui.add_space(20.0);
                }

                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(15, 15, 20))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::WHITE))
                    .corner_radius(16.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        ui.set_width(200.0);
                        ui.set_max_height(250.0);
                        
                        ui.vertical_centered(|ui| {
                            // Profile name display
                            ui.label(
                                egui::RichText::new(&instance.profname)
                                    .size(22.0)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );

                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(6.0);

                            for chunk in instance.devices.chunks(2) {
                                Self::render_device_row(
                                    ui,
                                    &self.input_devices,
                                    chunk,
                                );
                                if chunk.len() == 2 {
                                    ui.add_space(8.0);
                                }
                            }

                            ui.add_space(ui.available_height() - 28.0);

                            let buttons_width = 40.0 + 8.0 + 28.0 + 8.0 + 40.0;
                            let buttons_padding = ((ui.available_width() - buttons_width) / 2.0).max(0.0);
                            
                            ui.horizontal(|ui| {
                                ui.add_space(buttons_padding);
                                
                                // Left arrow
                                let left_btn = egui::Button::new(
                                    egui::RichText::new("◀").size(18.0)
                                )
                                .fill(egui::Color32::from_rgb(30, 30, 40))
                                .stroke(egui::Stroke::new(2.0, egui::Color32::WHITE))
                                .min_size(egui::Vec2::new(32.0, 32.0));

                                if ui.add(left_btn).clicked() {
                                    profile_actions.push((idx, -1));
                                }

                                ui.add_space(8.0);

                                // Add device button
                                let is_waiting = self.waiting_for_device == Some(idx);
                                let button_color = if is_waiting {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_rgb(30, 30, 40)
                                };
                                let button_stroke = if is_waiting {
                                    egui::Stroke::new(3.0, egui::Color32::WHITE)
                                } else {
                                    egui::Stroke::new(2.0, egui::Color32::WHITE)
                                };

                                let button = egui::Button::new(
                                    egui::RichText::new(if is_waiting { "⏳" } else { "+" })
                                        .size(if is_waiting { 16.0 } else { 20.0 })
                                        .color(if is_waiting { egui::Color32::BLACK } else { egui::Color32::WHITE })
                                )
                                .fill(button_color)
                                .stroke(button_stroke)
                                .min_size(egui::Vec2::new(28.0, 28.0));

                                if ui.add(button).clicked() {
                                    waiting_actions.push(if is_waiting { None } else { Some(idx) });
                                }

                                ui.add_space(8.0);

                                // Right arrow
                                let right_btn = egui::Button::new(
                                    egui::RichText::new("▶").size(18.0)
                                )
                                .fill(egui::Color32::from_rgb(30, 30, 40))
                                .stroke(egui::Stroke::new(2.0, egui::Color32::WHITE))
                                .min_size(egui::Vec2::new(32.0, 32.0));

                                if ui.add(right_btn).clicked() {
                                    profile_actions.push((idx, 1));
                                }
                            });
                        });
                    });
            }
        });

        for (player_idx, direction) in profile_actions {
            self.cycle_profile(player_idx, direction);
        }

        for action in waiting_actions {
            self.waiting_for_device = action;
        }
    }

    fn render_device_row(
        ui: &mut egui::Ui,
        input_devices: &[InputDevice],
        devices: &[usize],
    ) {
        let device_width = 70.0;
        let content_width = (device_width * devices.len() as f32)
            + (8.0 * (devices.len().saturating_sub(1)) as f32);
        let row_padding = ((ui.available_width() - content_width) / 2.0).max(0.0);
        
        ui.horizontal(|ui| {
            ui.add_space(row_padding);
            
            for (i, &dev_idx) in devices.iter().enumerate() {
                let dev = &input_devices[dev_idx];
                
                let name = if dev.device_type() == DeviceType::Keyboard
                    || dev.device_type() == DeviceType::Mouse {
                    dev.fancyname().split_whitespace().next()
                        .unwrap_or(dev.fancyname())
                } else {
                    dev.fancyname()
                };
                
                let (rect, _response) = ui.allocate_exact_size(
                    egui::Vec2::new(device_width, 60.0),
                    egui::Sense::hover()
                );
                
                let painter = ui.painter();
                
                let emoji_pos = egui::pos2(
                    rect.center().x - 41.0 / 2.0,
                    rect.min.y + 8.0
                );
                
                let text_galley = ui.fonts(|fonts| {
                    fonts.layout_no_wrap(
                        name.to_string(),
                        egui::FontId::new(12.0, egui::FontFamily::Proportional),
                        egui::Color32::from_rgb(200, 200, 200)
                    )
                });
                
                let text_pos = egui::pos2(
                    rect.center().x - text_galley.size().x / 2.0,
                    emoji_pos.y + 41.0 + 15.0
                );
                
                painter.text(
                    emoji_pos,
                    egui::Align2::LEFT_TOP,
                    dev.emoji(),
                    egui::FontId::new(41.0, egui::FontFamily::Proportional),
                    egui::Color32::WHITE
                );
                
                painter.galley(text_pos, text_galley, egui::Color32::WHITE);
                
                if i < devices.len() - 1 {
                    ui.add_space(8.0);
                }
            }
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
        let cfg = self.options.clone();
        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();

        self.spawn_task(
            "Launching...\n\nDon't press any buttons or move any analog sticks or mice.",
            move || {
                sleep(std::time::Duration::from_secs_f32(1.5));

                if let Err(err) = launch_common(&handler, &dev_infos, &instances, &cfg) {
                    println!("[partydeck] Error launching instances: {}", err);
                    msg("Launch Error", &format!("{err}"));
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
        self.handle_input(raw_input);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ctx, |_ui| {});

        let screen_rect = ctx.screen_rect();
        let upper_y = screen_rect.height() * 0.05;
        
        egui::Area::new("instruction_box".into())
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, upper_y))
            .interactable(false)
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                self.render_instruction_box(ui);
            });

        let lower_y = screen_rect.height() * 0.5;
        
        egui::Area::new("player_boxes".into())
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, lower_y))
            .interactable(false)
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                self.render_player_boxes(ui);
            });

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