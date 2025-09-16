use super::app::{MenuPage, PartyApp};
use crate::handler::scan_handlers;
use crate::input::*;
use crate::profiles::scan_profiles;
use crate::util::*;

use eframe::egui::Popup;
use eframe::egui::RichText;
use eframe::egui::{self, Ui};

macro_rules! cur_handler {
    ($self:expr) => {
        &$self.handlers[$self.selected_handler]
    };
}

impl PartyApp {
    pub fn display_panel_top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(egui::include_image!("../../res/BTN_EAST.png")).max_height(12.0),
            );

            let hometext = match self.is_lite() {
                true => "Play",
                false => "Home",
            };
            let homepage = match self.is_lite() {
                true => MenuPage::Instances,
                false => MenuPage::Home,
            };
            ui.selectable_value(&mut self.cur_page, homepage, hometext);
            ui.add(
                egui::Image::new(egui::include_image!("../../res/BTN_NORTH.png")).max_height(12.0),
            );
            ui.selectable_value(&mut self.cur_page, MenuPage::Settings, "Settings");
            ui.add(
                egui::Image::new(egui::include_image!("../../res/BTN_WEST.png")).max_height(12.0),
            );
            if ui
                .selectable_value(&mut self.cur_page, MenuPage::Profiles, "Profiles")
                .clicked()
            {
                self.profiles = scan_profiles(false);
                self.cur_page = MenuPage::Profiles;
            }

            if ui.button("🎮 Rescan").clicked() {
                self.instances.clear();
                self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("❌ Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let version_label = match self.needs_update {
                    true => format!("v{} (Update Available)", env!("CARGO_PKG_VERSION")),
                    false => format!("v{}", env!("CARGO_PKG_VERSION")),
                };
                ui.hyperlink_to(version_label, "https://github.com/wunnr/partydeck/releases");
                ui.add(egui::Separator::default().vertical());
                ui.hyperlink_to(
                    "Licenses",
                    "https://github.com/wunnr/partydeck/tree/main?tab=License-2-ov-file",
                );
                ui.add(egui::Separator::default().vertical());
                ui.hyperlink_to(
                    "Handlers",
                    "https://drive.proton.me/urls/D9HBKM18YR#zG8XC8yVy9WL",
                );
                ui.add(egui::Separator::default().vertical());
                ui.hyperlink_to("Donate", "https://ko-fi.com/wunner");
                ui.add(egui::Separator::default().vertical());
                ui.hyperlink_to("GitHub", "https://github.com/wunnr/partydeck");
            });
        });
    }

    pub fn display_panel_left(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Games");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕").clicked() {
                    if let Err(e) = self.add_handler() {
                        msg("Error adding handler", &e);
                    }
                }
                if ui.button("🔄").clicked() {
                    self.handlers = scan_handlers();
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.panel_left_game_list(ui);
        });
    }

    pub fn display_panel_bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("info_panel")
            .exact_height(100.0)
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                match self.cur_page {
                    MenuPage::Game => {
                        self.infotext = cur_handler!(self).info.to_owned();
                    }
                    MenuPage::Profiles => {
                        self.infotext = "Create profiles to persistently store game save data, settings, and stats.".to_string();
                    }
                    _ => {}
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.cur_page == MenuPage::EditHandler && let Some(handler) = &mut self.handler_edit {
                        ui.add(egui::TextEdit::multiline(&mut handler.info).hint_text("Put game info/instructions here"));
                    } else {
                        ui.label(&self.infotext);
                    }
                });
            });
    }

    pub fn display_panel_right(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(6.0);

        ui.heading("Devices");
        ui.separator();

        for pad in self.input_devices.iter() {
            let mut dev_text = RichText::new(format!(
                "{} {} ({})",
                pad.emoji(),
                pad.fancyname(),
                pad.path()
            ))
            .small();

            if !pad.enabled() {
                dev_text = dev_text.weak();
            } else if pad.has_button_held() {
                dev_text = dev_text.strong();
            }

            ui.label(dev_text);
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.link("Devices not being detected?").on_hover_ui(|ui| {
                ui.style_mut().interaction.selectable_labels = true;
                ui.label("Try adding your user to the `input` group.");
                ui.label("In a terminal, enter the following command:");
                ui.horizontal(|ui| {
                    ui.code("sudo usermod -aG input $USER");
                    if ui.button("📎").clicked() {
                        ctx.copy_text("sudo usermod -aG input $USER".to_string());
                    }
                });
            });
        });
    }

    pub fn panel_left_game_list(&mut self, ui: &mut Ui) {
        for i in 0..self.handlers.len() {
            // Skip if index is out of bounds to catch for removing/rescanning handlers
            if i >= self.handlers.len() {
                continue;
            }

            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(self.handlers[i].icon())
                        .max_width(16.0)
                        .corner_radius(2),
                );

                let btn =
                    ui.selectable_value(&mut self.selected_handler, i, self.handlers[i].display());
                if btn.has_focus() {
                    btn.scroll_to_me(None);
                }
                if btn.clicked() {
                    self.cur_page = MenuPage::Game;
                };

                Popup::context_menu(&btn).show(|ui| self.handler_ctx_menu(ui, i));
            });
        }
    }

    pub fn handler_ctx_menu(&mut self, ui: &mut Ui, i: usize) {
        if ui.button("Edit").clicked() {
            self.handler_edit = Some(self.handlers[i].clone());
            self.cur_page = MenuPage::EditHandler;
        }

        if ui.button("Open Folder").clicked() {
            if let Err(_) = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "xdg-open {}",
                    self.handlers[i].path_handler.display()
                ))
                .status()
            {
                msg("Error", "Couldn't open handler folder!");
            }
        }

        if ui.button("Remove").clicked() {
            if yesno(
                "Remove handler?",
                &format!(
                    "Are you sure you want to remove {}?",
                    self.handlers[i].display()
                ),
            ) {
                if let Err(err) = self.handlers[i].remove_dir() {
                    println!("[partydeck] Failed to remove handler: {}", err);
                    msg("Error", &format!("Failed to remove handler: {}", err));
                }
                self.handlers = scan_handlers();
            }
        }
    }
}
