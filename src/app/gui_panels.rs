use super::app::{MenuPage, PartyApp};
use crate::game::{Game::*, *};
use crate::input::*;
use crate::paths::*;
use crate::util::*;

use eframe::egui::RichText;
use eframe::egui::{self, Ui};

macro_rules! cur_game {
    ($self:expr) => {
        &$self.games[$self.selected_game]
    };
}

impl PartyApp {
    pub fn display_panel_top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(egui::include_image!("../../res/BTN_EAST.png")).max_height(12.0),
            );
            ui.selectable_value(&mut self.cur_page, MenuPage::Home, "Home");
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
                    "Open Source Licenses",
                    "https://github.com/wunnr/partydeck/tree/main?tab=License-2-ov-file",
                );
            });
        });
    }

    pub fn display_panel_left(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Games");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕").clicked() {
                    if let Err(err) = add_game() {
                        println!("[partydeck] Couldn't add game: {err}");
                        msg("Error", &format!("Couldn't add game: {err}"));
                    }
                    let dir_tmp = PATH_PARTY.join("tmp");
                    if dir_tmp.exists() {
                        std::fs::remove_dir_all(&dir_tmp).unwrap();
                    }
                    self.games = crate::game::scan_all_games();
                }
                if ui.button("🔄").clicked() {
                    self.games = crate::game::scan_all_games();
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
                        match cur_game!(self){
                            Game::ExecRef(e) =>
                                self.infotext = format!("{}", e.path().display()),
                            Game::HandlerRef(h) =>
                                self.infotext = h.info.to_owned(),
                        }
                    }
                    MenuPage::Profiles =>
                        self.infotext = "Create profiles to persistently store game save data, settings, and stats.".to_string(),
                    _ => {}
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(&self.infotext);
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
        let mut refresh_games = false;

        for (i, game) in self.games.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(game.icon())
                        .max_width(16.0)
                        .corner_radius(2),
                );
                let btn = ui.selectable_value(&mut self.selected_game, i, game.name());
                if btn.has_focus() {
                    btn.scroll_to_me(None);
                }
                if btn.clicked() {
                    self.cur_page = MenuPage::Game;
                };

                let popup_id = ui.make_persistent_id(format!("gamectx{}", i));
                egui::old_popup::popup_below_widget(
                    ui,
                    popup_id,
                    &btn,
                    egui::PopupCloseBehavior::CloseOnClick,
                    |ui| {
                        if ui.button("Remove").clicked() {
                            if yesno(
                                "Remove game?",
                                &format!("Are you sure you want to remove {}?", game.name()),
                            ) {
                                if let Err(err) = remove_game(&self.games[i]) {
                                    println!("[partydeck] Failed to remove game: {}", err);
                                    msg("Error", &format!("Failed to remove game: {}", err));
                                }
                            }
                            refresh_games = true;
                        }
                        if let HandlerRef(h) = game {
                            if ui.button("Open Handler Folder").clicked() {
                                if let Err(_) = std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(format!("xdg-open {}", h.path_handler.display()))
                                    .status()
                                {
                                    msg("Error", "Couldn't open handler folder!");
                                }
                            }
                        }
                    },
                );

                if btn.secondary_clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                }
            });
        }
        // Hacky workaround to avoid borrowing conflicts from inside the loop
        if refresh_games {
            self.games = scan_all_games();
        }
    }
}
