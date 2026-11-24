use super::*;

impl McLauncherApp {
    pub(super) fn render_settings_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.settings.language;
        ui.heading(
            egui::RichText::new(tr(lang, "settings.title"))
                .size(21.0)
                .color(text_primary()),
        );
        ui.label(
            egui::RichText::new(tr(lang, "settings.subtitle"))
                .small()
                .color(text_secondary()),
        );
        ui.add_space(12.0);

        bounded_scroll(ui, "launcher_settings_scroll", |ui| {
            let card_w = (ui.available_width() - 32.0).max(160.0);

            // 1. Language
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.lang.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.lang.desc"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                let prev_lang = self.settings.language;
                egui::ComboBox::from_id_salt("settings_lang_combo")
                    .selected_text(format!(
                        "{} ({})",
                        self.settings.language.label(),
                        self.settings.language.code()
                    ))
                    .width((card_w - 28.0).clamp(160.0, 260.0))
                    .show_ui(ui, |ui| {
                        for option in Language::ALL {
                            ui.selectable_value(
                                &mut self.settings.language,
                                option,
                                format!("{} ({})", option.label(), option.code()),
                            );
                        }
                    });
                if self.settings.language != prev_lang {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    self.sync_discord_presence();
                    anim::trigger_screen_veil(ctx);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 32.0);
                }
            });
            ui.add_space(12.0);

            // 2. Color Palette
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.theme.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.theme.preset"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    let prev_preset = self.settings.theme_preset;
                    egui::ComboBox::from_id_salt("settings_theme_combo")
                        .selected_text(self.settings.theme_preset.label())
                        .width((card_w - 160.0).clamp(150.0, 240.0))
                        .show_ui(ui, |ui| {
                            for preset in ThemePreset::ALL {
                                ui.selectable_value(
                                    &mut self.settings.theme_preset,
                                    preset,
                                    preset.label(),
                                );
                            }
                        });
                    if self.settings.theme_preset != prev_preset {
                        let preset = self.settings.theme_preset;
                        if preset != ThemePreset::Custom {
                            self.settings.custom_palette = preset.palette();
                        }
                        let target = self.settings.effective_palette();
                        transition_theme_palette(ctx, &target);
                        let _ = save_launcher_settings(&self.paths, &self.settings);
                        let target_accent =
                            Color32::from_rgb(target.accent[0], target.accent[1], target.accent[2]);
                        anim::emit_edge_pulse(ctx, target_accent);
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), target_accent, 34.0);
                    }
                    if ui_button_icon(
                        ui,
                        "theme_reset_btn",
                        Some(Icon::Refresh),
                        tr(lang, "settings.theme.reset"),
                        ButtonVariant::Subtle,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        self.settings.theme_preset = ThemePreset::Sunset;
                        self.settings.custom_palette = ThemePreset::Sunset.palette();
                        let target = self.settings.effective_palette();
                        transition_theme_palette(ctx, &target);
                        let _ = save_launcher_settings(&self.paths, &self.settings);
                        let target_accent =
                            Color32::from_rgb(target.accent[0], target.accent[1], target.accent[2]);
                        anim::emit_edge_pulse(ctx, target_accent);
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), target_accent, 34.0);
                    }
                });
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(tr(lang, "settings.theme.custom"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                let mut pal = self.settings.effective_palette();
                let mut pal_changed = false;
                let cols: usize = if card_w >= 720.0 {
                    3
                } else if card_w >= 460.0 {
                    2
                } else {
                    1
                };
                let slots: [(&str, &mut [u8; 3]); 12] = [
                    ("settings.theme.c_accent", &mut pal.accent),
                    ("settings.theme.c_bg_void", &mut pal.bg_void),
                    ("settings.theme.c_bg_sidebar", &mut pal.bg_sidebar),
                    ("settings.theme.c_bg_elevated", &mut pal.bg_elevated),
                    ("settings.theme.c_bg_card", &mut pal.bg_card),
                    ("settings.theme.c_bg_card_hover", &mut pal.bg_card_hover),
                    ("settings.theme.c_bg_input", &mut pal.bg_input),
                    ("settings.theme.c_border_subtle", &mut pal.border_subtle),
                    ("settings.theme.c_border_strong", &mut pal.border_strong),
                    ("settings.theme.c_text_primary", &mut pal.text_primary),
                    ("settings.theme.c_text_secondary", &mut pal.text_secondary),
                    ("settings.theme.c_text_muted", &mut pal.text_muted),
                ];
                let cell_w = ((card_w - 16.0 - 12.0 * (cols.saturating_sub(1) as f32))
                    / cols as f32)
                    .clamp(130.0, (card_w - 16.0).max(130.0));
                egui::Grid::new("full_palette_grid")
                    .num_columns(cols)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for (idx, (key, color_ref)) in slots.into_iter().enumerate() {
                            ui.allocate_ui_with_layout(
                                Vec2::new(cell_w, 26.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(cell_w);
                                    if ui.color_edit_button_srgb(color_ref).changed() {
                                        pal_changed = true;
                                    }
                                    ui.add_space(4.0);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "#{:02X}{:02X}{:02X}",
                                                    color_ref[0], color_ref[1], color_ref[2]
                                                ))
                                                .monospace()
                                                .size(10.5)
                                                .color(text_muted()),
                                            );
                                            ui.with_layout(
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(tr(lang, key))
                                                                .small()
                                                                .color(text_primary()),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            );
                                        },
                                    );
                                },
                            );
                            if (idx + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
                if pal_changed {
                    self.settings.theme_preset = ThemePreset::Custom;
                    self.settings.custom_palette = pal;
                    self.settings.custom_accent = pal.accent;
                    set_theme_palette(&self.settings.effective_palette());
                    apply_modrinth_theme(ctx);
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                }
            });
            ui.add_space(12.0);

            // 2b. UI Font
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.font.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.font.desc"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                let current_font_name = if self.settings.ui_font_path.trim().is_empty() {
                    tr(lang, "settings.font.default").to_string()
                } else {
                    self.system_fonts
                        .iter()
                        .find(|f| f.path == self.settings.ui_font_path)
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| {
                            std::path::Path::new(&self.settings.ui_font_path)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Custom")
                                .to_string()
                        })
                };
                ui_flow_row(ui, 30.0, |ui| {
                    egui::ComboBox::from_id_salt("settings_ui_font_combo")
                        .selected_text(&current_font_name)
                        .width((card_w - 140.0).clamp(140.0, 240.0))
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.settings.ui_font_path,
                                    String::new(),
                                    tr(lang, "settings.font.default"),
                                )
                                .changed()
                            {
                                apply_ui_font(ctx, &self.settings.ui_font_path);
                                let _ = save_launcher_settings(&self.paths, &self.settings);
                                anim::trigger_screen_veil(ctx);
                                anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
                            }
                            for f in &self.system_fonts {
                                if ui
                                    .selectable_value(
                                        &mut self.settings.ui_font_path,
                                        f.path.clone(),
                                        &f.name,
                                    )
                                    .changed()
                                {
                                    apply_ui_font(ctx, &self.settings.ui_font_path);
                                    let _ = save_launcher_settings(&self.paths, &self.settings);
                                    anim::trigger_screen_veil(ctx);
                                    anim::emit_ring(
                                        ctx,
                                        anim::pointer_or_center(ctx),
                                        accent(),
                                        30.0,
                                    );
                                }
                            }
                        });
                    if ui_button_icon(
                        ui,
                        "rescan_fonts_btn",
                        Some(Icon::Refresh),
                        tr(lang, "btn.rescan"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        self.system_fonts = discover_system_fonts();
                    }
                    if !self.settings.ui_font_path.trim().is_empty()
                        && ui_button(
                            ui,
                            "reset_font_btn",
                            tr(lang, "btn.clear"),
                            ButtonVariant::Subtle,
                            true,
                            28.0,
                        )
                        .clicked()
                    {
                        self.settings.ui_font_path.clear();
                        apply_ui_font(ctx, "");
                        let _ = save_launcher_settings(&self.paths, &self.settings);
                        anim::trigger_screen_veil(ctx);
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                    }
                });
                ui.add_space(6.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    if ui_text_input(
                        ui,
                        "custom_font_path_in",
                        &mut self.settings.ui_font_path,
                        "/usr/share/fonts/.../font.ttf",
                        (card_w - 72.0).clamp(160.0, 390.0),
                        false,
                    )
                    .changed()
                    {
                        let p = self.settings.ui_font_path.trim();
                        if p.is_empty() || std::path::Path::new(p).is_file() {
                            apply_ui_font(ctx, p);
                            let _ = save_launcher_settings(&self.paths, &self.settings);
                            anim::trigger_screen_veil(ctx);
                        }
                    }
                    if ui_icon_button(
                        ui,
                        "browse_custom_font_btn",
                        Icon::Folder,
                        tr(lang, "btn.folder"),
                        ButtonVariant::Secondary,
                        !self.file_picker_open,
                        28.0,
                    )
                    .clicked()
                    {
                        self.request_file_pick(FilePickPurpose::CustomUiFont);
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(tr(lang, "settings.font.preview")).color(text_primary()),
                );
            });
            ui.add_space(12.0);

            // 3. Discord Rich Presence
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.discord.title"));
                if ui_checkbox(
                    ui,
                    "discord_rpc_toggle",
                    &mut self.settings.discord_rpc_enabled,
                    tr(lang, "settings.discord.enable"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    self.sync_discord_presence();
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                }
                ui.add_space(4.0);
                let kind = self.current_discord_kind();
                let (details, state_text) = format_presence(self.settings.language, &kind);
                let preview_text = if state_text.trim().is_empty() {
                    details
                } else {
                    format!("{details} • {state_text}")
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(tr(lang, "settings.discord.preview"))
                            .small()
                            .color(text_muted()),
                    );
                    badge(ui, &preview_text, self.settings.discord_rpc_enabled);
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "settings.discord.app_id"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "discord_client_id_in",
                    &mut self.settings.discord_client_id,
                    "1571137394923667579 (default)",
                    (card_w - 28.0).clamp(180.0, 360.0),
                    false,
                )
                .changed()
                {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    self.sync_discord_presence();
                }
                ui.label(
                    egui::RichText::new(tr(lang, "settings.discord.app_id_hint"))
                        .small()
                        .color(text_muted()),
                );
            });
            ui.add_space(12.0);

            // 4. Java Runtimes Manager
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.java.title"));
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        tr(lang, "settings.java.desc"),
                        self.paths.root.join("java").display()
                    ))
                    .small()
                    .color(text_secondary()),
                );
                ui.add_space(8.0);
                ui_flow_row(ui, 28.0, |ui| {
                    for major in [8u32, 16, 17, 21, 25] {
                        let busy_this = self.java_downloading_major == Some(major);
                        let label = if busy_this {
                            format!("Java {major}...")
                        } else {
                            format!("Java {major}")
                        };
                        if ui_button_icon(
                            ui,
                            ("dl_java_btn", major),
                            Some(Icon::Download),
                            &label,
                            ButtonVariant::Secondary,
                            self.java_downloading_major.is_none(),
                            28.0,
                        )
                        .on_hover_text(format!("{} Java {major}", tr(lang, "btn.download")))
                        .clicked()
                        {
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
                            self.download_java_version(major);
                        }
                    }
                    if ui_button_icon(
                        ui,
                        "settings_java_dir",
                        Some(Icon::Folder),
                        tr(lang, "btn.open_folder"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        open_in_file_manager(&self.paths.root.join("java"));
                    }
                });
                ui.add_space(8.0);

                let mut to_remove: Option<std::path::PathBuf> = None;
                for (idx, j) in self.java_installs.iter().enumerate() {
                    let managed = is_managed_java_path(&self.paths, &j.path);
                    ui.horizontal(|ui| {
                        ui.set_max_width(card_w);
                        badge(ui, &format!("Java {}", j.major), managed);
                        badge(
                            ui,
                            if managed {
                                tr(lang, "settings.java.managed")
                            } else {
                                tr(lang, "settings.java.system")
                            },
                            false,
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if managed
                                && ui_button(
                                    ui,
                                    ("rm_java_btn", idx),
                                    tr(lang, "btn.remove"),
                                    ButtonVariant::Danger,
                                    true,
                                    26.0,
                                )
                                .clicked()
                            {
                                to_remove = Some(j.path.clone());
                            }
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(j.path.to_string_lossy().as_ref())
                                                .small()
                                                .color(text_muted()),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
                        });
                    });
                    ui.add_space(3.0);
                }
                if let Some(path) = to_remove {
                    match remove_managed_java(&self.paths, &path) {
                        Ok(()) => {
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), DANGER, 8);
                            self.show_toast(ctx, "Removed managed Java runtime");
                            self.scan_java();
                        }
                        Err(e) => {
                            self.show_toast(ctx, format!("Failed to remove Java: {e}"));
                        }
                    }
                }
            });

            ui.add_space(12.0);

            // 5. Launch Behavior & Updates
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.behavior.title"));
                if ui_checkbox(
                    ui,
                    "settings_offline_mode",
                    &mut self.settings.offline_mode,
                    tr(lang, "settings.offline_mode"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 24.0);
                }
                ui.add_space(4.0);
                if ui_checkbox(
                    ui,
                    "settings_hide_on_launch",
                    &mut self.settings.hide_on_launch,
                    tr(lang, "settings.hide_on_launch"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 24.0);
                }
                ui.add_space(4.0);
                if ui_checkbox(
                    ui,
                    "settings_open_console_crash",
                    &mut self.settings.open_console_on_crash,
                    tr(lang, "settings.open_console_crash"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 24.0);
                }
                ui.add_space(4.0);
                if ui_checkbox(
                    ui,
                    "settings_check_updates_startup",
                    &mut self.settings.check_updates_on_startup,
                    tr(lang, "settings.check_updates_startup"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 24.0);
                }
                ui.add_space(4.0);
                if ui_checkbox(
                    ui,
                    "settings_auto_update",
                    &mut self.settings.auto_update,
                    tr(lang, "settings.auto_update"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 24.0);
                }
                ui.add_space(12.0);
                ui_flow_row(ui, 30.0, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: v{}",
                            tr(lang, "settings.updates.current"),
                            env!("CARGO_PKG_VERSION")
                        ))
                        .monospace()
                        .small()
                        .color(text_muted()),
                    );
                    if ui_button_icon(
                        ui,
                        "settings_check_updates_btn",
                        Some(Icon::Refresh),
                        tr(lang, "settings.updates.check"),
                        ButtonVariant::Secondary,
                        !self.update_checking && !self.update_installing,
                        28.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        self.check_launcher_updates();
                    }
                    if let Some(upd) = self.update_available.clone() {
                        let has_asset = upd.download_url.is_some();
                        if has_asset
                            && ui_button_icon(
                                ui,
                                "settings_install_update_btn",
                                Some(Icon::Download),
                                tr(lang, "settings.updates.install"),
                                ButtonVariant::Primary,
                                !self.update_installing
                                    && !self.is_busy
                                    && !self.any_game_running()
                                    && self.launching_profiles.is_empty(),
                                28.0,
                            )
                            .clicked()
                        {
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                            self.start_launcher_update();
                        }
                        if ui_button_icon(
                            ui,
                            "settings_open_release_btn",
                            Some(Icon::External),
                            tr(lang, "settings.updates.open"),
                            if has_asset {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            },
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                            ctx.open_url(egui::OpenUrl::new_tab(&upd.html_url));
                        }
                    }
                    if self.update_installing {
                        mono_spinner(ui, 16.0);
                    }
                });
                if let Some(upd) = self.update_available.clone() {
                    let ver = upd.version.trim_start_matches(['v', 'V']);
                    ui.add_space(6.0);
                    ui_alert_banner(
                        ui,
                        AlertKind::Success,
                        &format!("{}: v{ver}", tr(lang, "settings.updates.available")),
                        (card_w - 28.0).max(160.0),
                    );
                    let notes = upd.notes.trim();
                    if !notes.is_empty() {
                        ui.add_space(4.0);
                        let preview: String = notes.chars().take(240).collect();
                        ui.label(egui::RichText::new(preview).small().color(text_muted()));
                    }
                } else if self.update_checked_up_to_date {
                    ui.add_space(6.0);
                    badge(ui, tr(lang, "settings.updates.up_to_date"), true);
                } else if let Some(err) = self.update_error.clone() {
                    ui.add_space(6.0);
                    ui_alert_banner(ui, AlertKind::Warning, &err, (card_w - 28.0).max(160.0));
                }
            });

            ui.add_space(12.0);
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.defaults.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.defaults.desc"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                let mut def_changed = false;
                let field_w = (card_w - 28.0).clamp(140.0, 520.0);
                let stack_sliders = card_w < 460.0;
                let slider_w = if stack_sliders {
                    (card_w - 104.0).clamp(90.0, 320.0)
                } else {
                    (card_w - 260.0).clamp(120.0, 340.0)
                };

                ui.label(
                    egui::RichText::new(format!(
                        "{}: {} MB — {}: {} MB",
                        tr(lang, "inst.min_ram"),
                        self.settings.default_min_ram_mb,
                        tr(lang, "inst.max_ram"),
                        self.settings.default_max_ram_mb
                    ))
                    .small()
                    .color(text_secondary()),
                );
                ui.add_space(4.0);
                if stack_sliders {
                    ui.label(egui::RichText::new(tr(lang, "inst.min_ram")).small());
                    if ui_ram_slider(
                        ui,
                        &mut self.settings.default_min_ram_mb,
                        256..=16384,
                        slider_w,
                    )
                    .changed()
                    {
                        if self.settings.default_max_ram_mb < self.settings.default_min_ram_mb {
                            self.settings.default_max_ram_mb = self.settings.default_min_ram_mb;
                        }
                        def_changed = true;
                    }
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(tr(lang, "inst.max_ram")).small());
                    if ui_ram_slider(
                        ui,
                        &mut self.settings.default_max_ram_mb,
                        512..=32768,
                        slider_w,
                    )
                    .changed()
                    {
                        if self.settings.default_min_ram_mb > self.settings.default_max_ram_mb {
                            self.settings.default_min_ram_mb = self.settings.default_max_ram_mb;
                        }
                        def_changed = true;
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(Vec2::new(150.0, 22.0), |ui| {
                            ui.label(egui::RichText::new(tr(lang, "inst.min_ram")).small());
                        });
                        if ui_ram_slider(
                            ui,
                            &mut self.settings.default_min_ram_mb,
                            256..=16384,
                            slider_w,
                        )
                        .changed()
                        {
                            if self.settings.default_max_ram_mb < self.settings.default_min_ram_mb {
                                self.settings.default_max_ram_mb = self.settings.default_min_ram_mb;
                            }
                            def_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.allocate_ui(Vec2::new(150.0, 22.0), |ui| {
                            ui.label(egui::RichText::new(tr(lang, "inst.max_ram")).small());
                        });
                        if ui_ram_slider(
                            ui,
                            &mut self.settings.default_max_ram_mb,
                            512..=32768,
                            slider_w,
                        )
                        .changed()
                        {
                            if self.settings.default_min_ram_mb > self.settings.default_max_ram_mb {
                                self.settings.default_min_ram_mb = self.settings.default_max_ram_mb;
                            }
                            def_changed = true;
                        }
                    });
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "settings.defaults.java_path"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "settings_default_java_path",
                    &mut self.settings.default_java_path,
                    "Auto",
                    field_w,
                    true,
                )
                .changed()
                {
                    def_changed = true;
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "inst.jvm_args"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "settings_default_jvm_args",
                    &mut self.settings.default_jvm_args,
                    "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions",
                    field_w,
                    true,
                )
                .changed()
                {
                    def_changed = true;
                }
                ui.add_space(6.0);
                let mut w_str = self
                    .settings
                    .default_window_width
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let mut h_str = self
                    .settings
                    .default_window_height
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                ui_wrap_row(ui, 26.0, |ui| {
                    ui.label(egui::RichText::new(tr(lang, "inst.window_size")).small());
                    let r_w = ui.add_sized(
                        [76.0, 24.0],
                        egui::TextEdit::singleline(&mut w_str).hint_text("854"),
                    );
                    ui.label("×");
                    let r_h = ui.add_sized(
                        [76.0, 24.0],
                        egui::TextEdit::singleline(&mut h_str).hint_text("480"),
                    );
                    if r_w.changed() {
                        self.settings.default_window_width = w_str.trim().parse::<u32>().ok();
                        def_changed = true;
                    }
                    if r_h.changed() {
                        self.settings.default_window_height = h_str.trim().parse::<u32>().ok();
                        def_changed = true;
                    }
                });
                ui.add_space(4.0);
                if ui_checkbox(
                    ui,
                    "settings_default_fullscreen",
                    &mut self.settings.default_fullscreen,
                    tr(lang, "inst.fullscreen"),
                ) {
                    def_changed = true;
                }
                if ui_checkbox(
                    ui,
                    "settings_default_dgpu",
                    &mut self.settings.default_prefer_discrete_gpu,
                    tr(lang, "inst.discrete_gpu"),
                ) {
                    def_changed = true;
                }
                if def_changed {
                    self.new_ram_mb = self.settings.default_max_ram_mb.clamp(512, 32768);
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                }
            });

            ui.add_space(12.0);
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.curseforge.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.curseforge.desc"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
                if ui_text_input(
                    ui,
                    "settings_curseforge_api_key",
                    &mut self.settings.curseforge_api_key,
                    tr(lang, "settings.curseforge.hint"),
                    (card_w - 28.0).clamp(180.0, 460.0),
                    true,
                )
                .changed()
                {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                }
            });
        });
    }

    pub(super) fn render_console_page(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let active_pid = self.console_profile_id.clone();
        let is_running = match &active_pid {
            Some(pid) => self.running_games.contains_key(pid),
            None => self.any_game_running(),
        };
        let logs_snapshot: VecDeque<LogEntry> =
            self.current_console_logs().iter().cloned().collect();
        let logs_empty = logs_snapshot.is_empty();
        let logs_len = logs_snapshot.len();
        if self.log_cache.diag_revision != self.log_revision {
            self.log_cache.diag_summary = diagnose_console_logs(&logs_snapshot);
            self.log_cache.warn_err_count = logs_snapshot
                .iter()
                .filter(|e| e.level != LogLevel::Info)
                .count();
            self.log_cache.diag_revision = self.log_revision;
        }
        let diag_summary = self.log_cache.diag_summary.clone();
        let warn_err_count = self.log_cache.warn_err_count;

        let mut do_clear = false;
        let mut do_copy_all = false;
        let mut do_upload_mclogs = false;
        let mut do_stop_pid: Option<String> = None;
        let mut switch_console_pid: Option<Option<String>> = None;

        let avail_w = ui.available_width();
        let has_crash_btn = self.last_crash_code.is_some() || diag_summary.is_some();
        let wide_hdr = avail_w >= if has_crash_btn { 680.0 } else { 560.0 };
        if wide_hdr {
            ui_toolbar_row(ui, 32.0, |ui| {
                ui.heading(
                    egui::RichText::new(tr(lang, "nav.console"))
                        .size(21.0)
                        .color(text_primary()),
                );
                if is_running {
                    let (dot_r, dot_resp) =
                        ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    draw_running_pulse_dot(ui, dot_r.center(), 3.2, accent(), bg_void());
                    dot_resp.on_hover_text(tr(lang, "console.live"));
                }
                ui.label(
                    egui::RichText::new(format!("{logs_len} {}", tr(lang, "console.lines")))
                        .monospace()
                        .size(11.5)
                        .color(text_muted()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if let Some(pid) = &active_pid {
                        if self.running_games.contains_key(pid)
                            && ui_button_icon(
                                ui,
                                "console_stop_inst_btn",
                                Some(Icon::Stop),
                                tr(lang, "top.stop"),
                                ButtonVariant::Danger,
                                true,
                                28.0,
                            )
                            .clicked()
                        {
                            do_stop_pid = Some(pid.clone());
                        }
                    }
                    if !logs_empty
                        && ui_button_icon(
                            ui,
                            "console_mclogs_btn",
                            Some(Icon::External),
                            if self.mclogs_uploading {
                                tr(lang, "logs.uploading")
                            } else {
                                "mclo.gs"
                            },
                            ButtonVariant::Secondary,
                            !self.mclogs_uploading,
                            28.0,
                        )
                        .clicked()
                    {
                        do_upload_mclogs = true;
                    }
                    if ui_button_icon(
                        ui,
                        "console_clear_btn",
                        Some(Icon::Trash),
                        tr(lang, "btn.clear"),
                        ButtonVariant::Subtle,
                        !logs_empty,
                        28.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(
                            ui.ctx(),
                            anim::pointer_or_center(ui.ctx()),
                            text_secondary(),
                            28.0,
                        );
                        do_clear = true;
                    }
                    if ui_button_icon(
                        ui,
                        "console_copy_btn",
                        Some(Icon::Copy),
                        tr(lang, "btn.copy_all"),
                        ButtonVariant::Secondary,
                        !logs_empty,
                        28.0,
                    )
                    .clicked()
                    {
                        do_copy_all = true;
                    }
                    if (self.last_crash_code.is_some() || diag_summary.is_some())
                        && ui_button_icon(
                            ui,
                            "console_copy_crash_btn",
                            Some(Icon::Alert),
                            tr(lang, "console.copy_crash"),
                            ButtonVariant::Danger,
                            !logs_empty,
                            28.0,
                        )
                        .clicked()
                    {
                        let mut parts = Vec::new();
                        parts.push(format!("Sunset Launcher v{}", env!("CARGO_PKG_VERSION")));
                        let crash_prof = active_pid
                            .as_deref()
                            .and_then(|pid| self.profiles.iter().find(|p| p.id == pid))
                            .or_else(|| self.profiles.get(self.selected_idx));
                        if let Some(p) = crash_prof {
                            parts.push(format!(
                                "Instance: {} (MC {}, {:?})",
                                p.name, p.version_id, p.loader
                            ));
                        }
                        if let Some(code) = self.last_crash_code {
                            parts.push(format!("Exit code: {code}"));
                        }
                        if let Some(diag) = &diag_summary {
                            parts.push(format!("Diagnosis: {diag}"));
                        }
                        let tail_start = logs_snapshot.len().saturating_sub(150);
                        parts.push("--- Tail Logs ---".to_string());
                        for l in logs_snapshot.iter().skip(tail_start) {
                            parts.push(format!("[{}] {}", l.timestamp, l.text));
                        }
                        ui.ctx().copy_text(parts.join("\n"));
                        let pos = anim::pointer_or_center(ui.ctx());
                        anim::emit_ring(ui.ctx(), pos, DANGER, 30.0);
                        anim::emit_floating_badge(ui.ctx(), pos, Some(Icon::Check), "", DANGER);
                        self.show_toast(ui.ctx(), tr(lang, "console.copy_crash"));
                    }
                });
            });
        } else {
            ui_wrap_row(ui, 28.0, |ui| {
                ui.heading(
                    egui::RichText::new(tr(lang, "nav.console"))
                        .size(21.0)
                        .color(text_primary()),
                );
                if is_running {
                    let (dot_r, dot_resp) =
                        ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    draw_running_pulse_dot(ui, dot_r.center(), 3.2, accent(), bg_void());
                    dot_resp.on_hover_text(tr(lang, "console.live"));
                }
                ui.label(
                    egui::RichText::new(format!("{logs_len} {}", tr(lang, "console.lines")))
                        .monospace()
                        .size(11.5)
                        .color(text_muted()),
                );
            });
            ui.add_space(4.0);
            ui_wrap_row(ui, 28.0, |ui| {
                if let Some(pid) = &active_pid {
                    if self.running_games.contains_key(pid)
                        && ui_button_icon(
                            ui,
                            "console_stop_inst_btn_n",
                            Some(Icon::Stop),
                            tr(lang, "top.stop"),
                            ButtonVariant::Danger,
                            true,
                            28.0,
                        )
                        .clicked()
                    {
                        do_stop_pid = Some(pid.clone());
                    }
                }
                if !logs_empty
                    && ui_button_icon(
                        ui,
                        "console_copy_btn_n",
                        Some(Icon::Copy),
                        tr(lang, "btn.copy_all"),
                        ButtonVariant::Secondary,
                        !logs_empty,
                        28.0,
                    )
                    .clicked()
                {
                    do_copy_all = true;
                }
                if !logs_empty
                    && ui_button_icon(
                        ui,
                        "console_mclogs_btn_n",
                        Some(Icon::External),
                        if self.mclogs_uploading {
                            tr(lang, "logs.uploading")
                        } else {
                            "mclo.gs"
                        },
                        ButtonVariant::Secondary,
                        !self.mclogs_uploading,
                        28.0,
                    )
                    .clicked()
                {
                    do_upload_mclogs = true;
                }
                if ui_button_icon(
                    ui,
                    "console_clear_btn_n",
                    Some(Icon::Trash),
                    tr(lang, "btn.clear"),
                    ButtonVariant::Subtle,
                    !logs_empty,
                    28.0,
                )
                .clicked()
                {
                    anim::emit_ring(
                        ui.ctx(),
                        anim::pointer_or_center(ui.ctx()),
                        text_secondary(),
                        28.0,
                    );
                    do_clear = true;
                }
                if (self.last_crash_code.is_some() || diag_summary.is_some())
                    && ui_button_icon(
                        ui,
                        "console_copy_crash_btn_n",
                        Some(Icon::Alert),
                        tr(lang, "console.copy_crash"),
                        ButtonVariant::Danger,
                        !logs_empty,
                        28.0,
                    )
                    .clicked()
                {
                    let mut parts = Vec::new();
                    parts.push(format!("Sunset Launcher v{}", env!("CARGO_PKG_VERSION")));
                    let crash_prof = active_pid
                        .as_deref()
                        .and_then(|pid| self.profiles.iter().find(|p| p.id == pid))
                        .or_else(|| self.profiles.get(self.selected_idx));
                    if let Some(p) = crash_prof {
                        parts.push(format!(
                            "Instance: {} (MC {}, {:?})",
                            p.name, p.version_id, p.loader
                        ));
                    }
                    if let Some(code) = self.last_crash_code {
                        parts.push(format!("Exit code: {code}"));
                    }
                    if let Some(diag) = &diag_summary {
                        parts.push(format!("Diagnosis: {diag}"));
                    }
                    let tail_start = logs_snapshot.len().saturating_sub(150);
                    parts.push("--- Tail Logs ---".to_string());
                    for l in logs_snapshot.iter().skip(tail_start) {
                        parts.push(format!("[{}] {}", l.timestamp, l.text));
                    }
                    ui.ctx().copy_text(parts.join("\n"));
                    let pos = anim::pointer_or_center(ui.ctx());
                    anim::emit_ring(ui.ctx(), pos, DANGER, 30.0);
                    anim::emit_floating_badge(ui.ctx(), pos, Some(Icon::Check), "", DANGER);
                    self.show_toast(ui.ctx(), tr(lang, "console.copy_crash"));
                }
            });
        }

        ui.add_space(6.0);
        let total_logs = logs_len;
        let console_search_w = if avail_w < 460.0 {
            (avail_w - 24.0).max(110.0)
        } else {
            (avail_w - 360.0).clamp(110.0, 260.0)
        };
        ui_wrap_row(ui, 30.0, |ui| {
            if !self.instance_logs.is_empty() || !self.running_games.is_empty() {
                let mut seen_pids: Vec<(String, String)> = Vec::new();
                for p in &self.profiles {
                    if self.running_games.contains_key(&p.id)
                        || self.instance_logs.contains_key(&p.id)
                    {
                        let lbl = if self.running_games.contains_key(&p.id) {
                            format!("● {}", p.name)
                        } else {
                            p.name.clone()
                        };
                        seen_pids.push((p.id.clone(), lbl));
                    }
                }
                let src_text = active_pid
                    .as_deref()
                    .and_then(|pid| {
                        seen_pids
                            .iter()
                            .find(|(id, _)| id == pid)
                            .map(|(_, l)| l.clone())
                    })
                    .unwrap_or_else(|| tr(lang, "console.src_launcher").to_string());
                egui::ComboBox::from_id_salt("console_src_combo")
                    .selected_text(src_text)
                    .width(148.0)
                    .truncate()
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                active_pid.is_none(),
                                tr(lang, "console.src_launcher"),
                            )
                            .clicked()
                            && active_pid.is_some()
                        {
                            switch_console_pid = Some(None);
                        }
                        for (pid, lbl) in seen_pids {
                            let sel = active_pid.as_deref() == Some(pid.as_str());
                            if ui.selectable_label(sel, lbl).clicked() && !sel {
                                switch_console_pid = Some(Some(pid));
                            }
                        }
                    });
            }

            let level_lbl = if self.log_errors_only {
                format!("{} ({warn_err_count})", tr(lang, "console.warn_err"))
            } else {
                format!("{} ({total_logs})", tr(lang, "lib.all"))
            };
            let prev_err_only = self.log_errors_only;
            egui::ComboBox::from_id_salt("console_level_combo")
                .selected_text(level_lbl)
                .width(136.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.log_errors_only,
                        false,
                        format!("{} ({total_logs})", tr(lang, "lib.all")),
                    );
                    ui.selectable_value(
                        &mut self.log_errors_only,
                        true,
                        format!("{} ({warn_err_count})", tr(lang, "console.warn_err")),
                    );
                });
            if self.log_errors_only != prev_err_only {
                self.mark_subtab_switch(ui.ctx());
            }

            ui_search_input(
                ui,
                "console_search",
                &mut self.log_search,
                tr(lang, "console.filter"),
                console_search_w,
            );
            ui_checkbox(
                ui,
                "console_autoscroll",
                &mut self.log_auto_scroll,
                tr(lang, "console.autoscroll"),
            );
        });

        ui.add_space(8.0);
        if let Some(code) = self.last_crash_code {
            let banner = match &diag_summary {
                Some(d) => format!("{}: {code} — {d}", tr(lang, "console.crash_banner")),
                None => format!("{}: {code}", tr(lang, "console.crash_banner")),
            };
            ui_alert_banner(ui, AlertKind::Error, &banner, ui.available_width());
            ui.add_space(6.0);
        } else if let Some(d) = &diag_summary {
            ui_alert_banner(ui, AlertKind::Warning, d, ui.available_width());
            ui.add_space(6.0);
        }
        let q = self.log_search.trim().to_lowercase();
        let scroll_h = ui.available_height().max(80.0);
        let mut copied_line: Option<String> = None;

        let frame_outer_w = ui.available_width().max(140.0);
        let frame_inner_w = (frame_outer_w - 22.0).max(110.0);
        egui::Frame::none()
            .fill(bg_input())
            .stroke(Stroke::new(1.0_f32, border_subtle()))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_min_height((scroll_h - 22.0).max(60.0));
                ui.set_min_width(frame_inner_w);
                ui.set_max_width(frame_inner_w);

                if logs_snapshot.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(28.0);
                        let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        draw_icon(ui.painter(), ic_r, Icon::Terminal, text_muted());
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(tr(lang, "console.empty")).color(text_muted()),
                        );
                    });
                    return;
                }

                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("console_log_scroll")
                    .auto_shrink([false, false])
                    .max_height((scroll_h - 24.0).max(60.0));
                if self.log_auto_scroll {
                    scroll = scroll.stick_to_bottom(true);
                }
                scroll.show_viewport(ui, |ui, viewport| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let row_w = (frame_inner_w - 14.0).max(96.0);
                    ui.set_max_width(row_w);
                    let wrap_w = ui.fonts(|f| {
                        let ts_w = f
                            .layout_no_wrap(
                                "00:00:00".to_string(),
                                egui::FontId::monospace(11.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x;
                        let tag_w =
                            f.layout_no_wrap(
                                "WARN".to_string(),
                                egui::FontId::monospace(10.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x + 10.0;
                        (row_w - 12.0 - ts_w - tag_w - 14.0).max(60.0)
                    });

                    if self.log_cache.revision != self.log_revision
                        || self.log_cache.errors_only != self.log_errors_only
                        || self.log_cache.query != q
                        || (self.log_cache.wrap_width - wrap_w).abs() > 1.0
                    {
                        self.log_cache.indices.clear();
                        self.log_cache.y_offsets.clear();
                        self.log_cache.y_offsets.push(0.0);
                        let mut y = 0.0_f32;
                        ui.fonts(|f| {
                            let font_id = egui::FontId::monospace(11.5);
                            for (idx, entry) in logs_snapshot.iter().enumerate() {
                                if self.log_errors_only && entry.level == LogLevel::Info {
                                    continue;
                                }
                                if !q.is_empty() && !entry.text.to_lowercase().contains(&q) {
                                    continue;
                                }
                                let text_h = f
                                    .layout(
                                        entry.text.clone(),
                                        font_id.clone(),
                                        Color32::WHITE,
                                        wrap_w,
                                    )
                                    .size()
                                    .y;
                                let row_h = (text_h + 5.0).max(20.0);
                                y += row_h + CONSOLE_ROW_GAP;
                                self.log_cache.indices.push(idx);
                                self.log_cache.y_offsets.push(y);
                            }
                        });
                        self.log_cache.revision = self.log_revision;
                        self.log_cache.errors_only = self.log_errors_only;
                        self.log_cache.query = q.clone();
                        self.log_cache.wrap_width = wrap_w;
                    }

                    let total_h = self.log_cache.y_offsets.last().copied().unwrap_or(0.0);
                    let (first, last) = visible_console_range(
                        &self.log_cache.y_offsets,
                        viewport.min.y,
                        viewport.max.y,
                    );
                    let top_h = self.log_cache.y_offsets.get(first).copied().unwrap_or(0.0);
                    let bottom_h = (total_h
                        - self
                            .log_cache
                            .y_offsets
                            .get(last)
                            .copied()
                            .unwrap_or(total_h))
                    .max(0.0);

                    if top_h > 0.0 {
                        ui.allocate_space(Vec2::new(row_w, top_h));
                    }

                    for vis_idx in first..last {
                        let entry = &logs_snapshot[self.log_cache.indices[vis_idx]];
                        let (tag, tag_col, tag_bg, msg_col, row_tint) = match entry.level {
                            LogLevel::Error => (
                                "ERR",
                                Color32::from_rgb(255, 146, 150),
                                Color32::from_rgb(68, 28, 32),
                                Color32::from_rgb(248, 138, 142),
                                Color32::from_rgba_unmultiplied(229, 72, 77, 18),
                            ),
                            LogLevel::Warn => (
                                "WARN",
                                WARNING,
                                Color32::from_rgb(56, 42, 20),
                                Color32::from_rgb(250, 198, 102),
                                Color32::from_rgba_unmultiplied(245, 166, 35, 14),
                            ),
                            LogLevel::Info => (
                                "INFO",
                                text_muted(),
                                Color32::from_rgb(27, 31, 39),
                                text_secondary(),
                                if vis_idx % 2 == 1 {
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 4)
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ),
                        };
                        let row_id =
                            ui.make_persistent_id(("console_row", self.log_cache.indices[vis_idx]));
                        let prev_hov = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(row_id))
                            .unwrap_or(false);
                        let row_hov =
                            ui.ctx()
                                .animate_bool_with_time(row_id.with("h"), prev_hov, 0.10);
                        if row_hov > 0.01 && row_hov < 0.99 {
                            ui.ctx().request_repaint();
                        }
                        let fill_col = lerp_color(row_tint, bg_card_hover(), row_hov * 0.45);
                        let row_resp = egui::Frame::none()
                            .fill(fill_col)
                            .rounding(Rounding::same(5.0))
                            .inner_margin(Margin::symmetric(6.0, 2.5))
                            .show(ui, |ui| {
                                ui.set_min_width((row_w - 12.0).max(80.0));
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 7.0;
                                    ui.label(
                                        egui::RichText::new(&entry.timestamp)
                                            .monospace()
                                            .size(11.0)
                                            .color(text_muted()),
                                    );
                                    egui::Frame::none()
                                        .fill(tag_bg)
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(Margin::symmetric(5.0, 0.5))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(tag)
                                                    .monospace()
                                                    .strong()
                                                    .size(10.0)
                                                    .color(tag_col),
                                            );
                                        });
                                    ui.set_max_width(wrap_w);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.text)
                                                .monospace()
                                                .size(11.5)
                                                .color(msg_col),
                                        )
                                        .wrap(),
                                    );
                                });
                            })
                            .response
                            .interact(Sense::click())
                            .on_hover_text("Click to copy line");
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(row_id, row_resp.hovered()));
                        if let Some((ct, origin)) = track_click_anim(
                            ui.ctx(),
                            row_id,
                            row_resp.rect,
                            row_resp.clicked(),
                            row_resp.interact_pointer_pos(),
                            0.36,
                        ) {
                            paint_click_ripple(
                                ui.painter(),
                                row_resp.rect,
                                Rounding::same(5.0),
                                origin,
                                ct,
                                accent(),
                                false,
                            );
                        }
                        if row_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if row_resp.clicked() {
                            let pos = anim::pointer_or_center(ui.ctx());
                            anim::emit_ring(ui.ctx(), pos, accent(), 26.0);
                            anim::emit_floating_badge(
                                ui.ctx(),
                                pos,
                                Some(Icon::Check),
                                "",
                                accent(),
                            );
                            copied_line = Some(format!("[{}] {}", entry.timestamp, entry.text));
                        }
                        ui.add_space(CONSOLE_ROW_GAP);
                    }

                    if bottom_h > 0.0 {
                        ui.allocate_space(Vec2::new(row_w, bottom_h));
                    }
                });
            });
        if let Some(line) = copied_line {
            ui.ctx().copy_text(line);
            let msg = tr(self.settings.language, "toast.logs_copied").to_string();
            self.show_toast(ui.ctx(), msg);
        }
        if let Some(new_pid) = switch_console_pid {
            self.console_profile_id = new_pid;
            self.log_revision = self.log_revision.wrapping_add(1);
        }
        if do_clear {
            self.clear_logs();
        }
        if do_copy_all {
            let all_text: String = logs_snapshot
                .iter()
                .map(|l| format!("[{}] {}", l.timestamp, l.text))
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(all_text);
            let pos = anim::pointer_or_center(ui.ctx());
            anim::emit_ring(ui.ctx(), pos, accent(), 28.0);
            anim::emit_floating_badge(ui.ctx(), pos, Some(Icon::Check), "", accent());
            let msg = tr(self.settings.language, "toast.logs_copied").to_string();
            self.show_toast(ui.ctx(), msg);
        }
        if do_upload_mclogs {
            let all_text: String = logs_snapshot
                .iter()
                .map(|l| format!("[{}] {}", l.timestamp, l.text))
                .collect::<Vec<_>>()
                .join("\n");
            self.start_mclogs_upload(all_text);
        }
        if let Some(pid) = do_stop_pid {
            self.stop_profile(&pid);
        }
    }
}
