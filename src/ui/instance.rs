use super::*;

impl McLauncherApp {
    pub(super) fn render_instance_page(&mut self, ui: &mut egui::Ui) {
        if self.profiles.is_empty() {
            ui.label(tr(self.settings.language, "inst.none"));
            return;
        }

        let profile = self.profiles[self.selected_idx].clone();
        let avail_w = ui.available_width().max(120.0);
        let wide = avail_w >= 920.0;
        if wide {
            ui.allocate_ui_with_layout(
                Vec2::new(avail_w, 32.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.set_max_width(avail_w);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if ui_button_icon(
                        ui,
                        "inst_mr_btn",
                        Some(Icon::Compass),
                        tr(self.settings.language, "nav.discover"),
                        ButtonVariant::Primary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.switch_page(ui.ctx(), NavPage::Discover);
                    }
                    if ui_button_icon(
                        ui,
                        "inst_dir_btn",
                        Some(Icon::Folder),
                        tr(self.settings.language, "btn.open_folder"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(
                            ui.ctx(),
                            anim::pointer_or_center(ui.ctx()),
                            accent(),
                            28.0,
                        );
                        open_in_file_manager(&instance_dir(&self.paths, &profile));
                    }
                    if ui_button_icon(
                        ui,
                        "inst_export_mrpack_btn",
                        Some(Icon::Download),
                        tr(self.settings.language, "btn.export_mrpack"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(
                            ui.ctx(),
                            anim::pointer_or_center(ui.ctx()),
                            accent(),
                            30.0,
                        );
                        self.export_current_profile_mrpack(ui.ctx(), &profile);
                    }

                    let left_w = (ui.available_width() - 8.0).max(80.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(left_w, 30.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_max_width(left_w);
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                            let is_running = self.is_profile_running(&profile.id);
                            draw_profile_avatar(
                                ui.painter(),
                                av_r,
                                &profile,
                                &self.profile_icons,
                                true,
                                is_running,
                            );
                            if is_running {
                                draw_running_pulse_dot(
                                    ui,
                                    Pos2::new(av_r.right() - 5.0, av_r.bottom() - 5.0),
                                    2.8,
                                    accent(),
                                    bg_void(),
                                );
                            }
                            let title_w = (left_w - 220.0).max(50.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(title_w, 30.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(title_w);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&profile.name)
                                                .size(21.0)
                                                .strong()
                                                .color(text_primary()),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
                            badge_sized(ui, &profile.version_id, false, 30.0, None);
                            loader_badge_sized(
                                ui,
                                profile.loader.kind(),
                                &profile.loader.label(),
                                30.0,
                            );
                        },
                    );
                },
            );
        } else {
            let render_title = |ui: &mut egui::Ui, w: f32| {
                ui.allocate_ui_with_layout(
                    Vec2::new(w, 28.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(w);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&profile.name)
                                    .size(21.0)
                                    .strong()
                                    .color(text_primary()),
                            )
                            .truncate(),
                        );
                    },
                );
            };
            if avail_w < 420.0 {
                render_title(ui, avail_w);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.set_max_width(avail_w);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    badge(ui, &profile.version_id, false);
                    loader_badge(ui, profile.loader.kind(), &profile.loader.label());
                });
            } else {
                ui.horizontal(|ui| {
                    ui.set_max_width(avail_w);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    render_title(ui, (avail_w - 180.0).max(120.0));
                    badge(ui, &profile.version_id, false);
                    loader_badge(ui, profile.loader.kind(), &profile.loader.label());
                });
            }
            ui.add_space(4.0);
            ui_wrap_row(ui, 30.0, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui_button_icon(
                    ui,
                    "inst_mr_btn_narrow",
                    Some(Icon::Compass),
                    tr(self.settings.language, "nav.discover"),
                    ButtonVariant::Primary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    self.switch_page(ui.ctx(), NavPage::Discover);
                }
                if ui_button_icon(
                    ui,
                    "inst_dir_btn_narrow",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.folder"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
                    open_in_file_manager(&instance_dir(&self.paths, &profile));
                }
                if ui_button_icon(
                    ui,
                    "inst_export_mrpack_btn_narrow",
                    Some(Icon::Download),
                    tr(self.settings.language, "btn.export_mrpack"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                    self.export_current_profile_mrpack(ui.ctx(), &profile);
                }
            });
        }

        ui.add_space(6.0);

        let lang = self.settings.language;
        let (total_mods, enabled_mods) = self.profile_mod_counts(&profile);
        let worlds_cnt = self.cached_worlds(&profile).len();
        let shots_cnt = self.cached_screenshots(&profile).len();
        let tab_label_for = |tab: InstanceSubTab| -> String {
            let count_tag: Option<String> = match tab {
                InstanceSubTab::Content if profile.loader.is_modded() => {
                    if total_mods > 0 {
                        Some(format!("{enabled_mods}/{total_mods}"))
                    } else {
                        Some("0".to_string())
                    }
                }
                InstanceSubTab::Worlds if worlds_cnt > 0 => Some(format!("{worlds_cnt}")),
                InstanceSubTab::Screenshots if shots_cnt > 0 => Some(format!("{shots_cnt}")),
                _ => None,
            };
            let base = tab.translated_label(lang);
            match count_tag {
                Some(tag) => format!("{base} ({tag})"),
                None => base.to_string(),
            }
        };
        if avail_w < 680.0 {
            let prev_tab = self.instance_tab;
            egui::ComboBox::from_id_salt("inst_subtab_combo")
                .selected_text(tab_label_for(self.instance_tab))
                .width((avail_w - 16.0).clamp(110.0, 260.0))
                .truncate()
                .show_ui(ui, |ui| {
                    for tab in InstanceSubTab::ALL {
                        ui.selectable_value(&mut self.instance_tab, tab, tab_label_for(tab));
                    }
                });
            if self.instance_tab != prev_tab {
                self.mark_subtab_switch(ui.ctx());
            }
        } else {
            ui_wrap_row(ui, 30.0, |ui| {
                for tab in InstanceSubTab::ALL {
                    let is_active = self.instance_tab == tab;
                    let tab_icon = match tab {
                        InstanceSubTab::Content => Icon::Layers,
                        InstanceSubTab::Worlds => Icon::Grid,
                        InstanceSubTab::Screenshots => Icon::External,
                        InstanceSubTab::Logs => Icon::Terminal,
                        InstanceSubTab::Settings => Icon::Sliders,
                    };
                    let lbl = tab_label_for(tab);
                    if ui_pill_tab_icon(
                        ui,
                        ("inst_subtab", tab.label()),
                        Some(tab_icon),
                        &lbl,
                        is_active,
                    )
                    .clicked()
                        && !is_active
                    {
                        self.instance_tab = tab;
                        self.mark_subtab_switch(ui.ctx());
                    }
                }
            });
        }
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let subtab_t = self.subtab_switch_time;
        let active_tab = self.instance_tab;
        Self::apply_subtab_transition(ui, subtab_t, |ui| match active_tab {
            InstanceSubTab::Content => self.render_content_manager(ui, &profile),
            InstanceSubTab::Worlds => self.render_worlds_tab(ui, &profile),
            InstanceSubTab::Screenshots => self.render_screenshots_tab(ui, &profile),
            InstanceSubTab::Logs => self.render_logs_tab(ui, &profile),
            InstanceSubTab::Settings => self.render_instance_settings_tab(ui),
        });
    }

    pub(super) fn render_worlds_tab(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let inst = instance_dir(&self.paths, profile);
        let saves_dir = inst.join("saves");
        let worlds = self.cached_worlds(profile);
        let backups = self.cached_world_backups(profile);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.label(
                egui::RichText::new(format!("{} {}", worlds.len(), tr(self.settings.language, "inst.saved_worlds")))
                    .small()
                    .color(text_secondary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "worlds_open_dir",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.open_saves"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    let _ = std::fs::create_dir_all(&saves_dir);
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
                    open_in_file_manager(&saves_dir);
                }
            });
        });
        ui.add_space(6.0);
        bounded_scroll(ui, "instance_worlds_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            if worlds.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.no_worlds"))
                            .color(text_secondary()),
                    );
                });
                ui.add_space(8.0);
            }
            for w in worlds {
                let lang = self.settings.language;
                let can_play = !self.is_profile_running(&profile.id)
                    && !self.is_profile_launching(&profile.id)
                    && !self.is_busy
                    && !self.update_installing;
                let backup_enabled = !self.world_backup_busy;
                let confirming_delete =
                    self.pending_world_delete.as_deref() == Some(w.folder_name.as_str());
                let mut action_play = false;
                let mut action_backup = false;
                let mut action_ask_delete = false;
                let mut action_cancel_delete = false;
                let mut action_confirm_delete = false;

                let card_id = ui.make_persistent_id(("world_card", &profile.id, &w.folder_name));
                let card_r = animated_card(ui, card_id, false, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    let wide_world_row = card_inner_w >= 580.0;
                    if wide_world_row {
                        ui.allocate_ui_with_layout(
                            Vec2::new(card_inner_w, 34.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if confirming_delete {
                                    if ui_button_icon(
                                        ui,
                                        ("world_del_confirm", &w.folder_name),
                                        Some(Icon::Trash),
                                        tr(lang, "world.delete_confirm"),
                                        ButtonVariant::Danger,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_confirm_delete = true;
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("world_del_cancel", &w.folder_name),
                                        Some(Icon::Close),
                                        tr(lang, "btn.cancel"),
                                        ButtonVariant::Subtle,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_cancel_delete = true;
                                    }
                                } else {
                                    if ui_button_icon(
                                        ui,
                                        ("world_del", &w.folder_name),
                                        Some(Icon::Trash),
                                        tr(lang, "btn.delete"),
                                        ButtonVariant::Subtle,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_ask_delete = true;
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("world_open", &w.folder_name),
                                        Some(Icon::Folder),
                                        tr(lang, "btn.open"),
                                        ButtonVariant::Subtle,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        anim::emit_ring(
                                            ui.ctx(),
                                            anim::pointer_or_center(ui.ctx()),
                                            accent(),
                                            28.0,
                                        );
                                        open_in_file_manager(&w.path);
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("world_datapacks_btn", &w.folder_name),
                                        Some(Icon::Layers),
                                        tr(lang, "world.datapacks"),
                                        if self.expanded_world_datapacks.as_deref()
                                            == Some(w.folder_name.as_str())
                                        {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        },
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        if self.expanded_world_datapacks.as_deref()
                                            == Some(w.folder_name.as_str())
                                        {
                                            self.expanded_world_datapacks = None;
                                        } else {
                                            self.expanded_world_datapacks =
                                                Some(w.folder_name.clone());
                                        }
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("world_backup", &w.folder_name),
                                        Some(Icon::Download),
                                        tr(lang, "world.backup"),
                                        ButtonVariant::Secondary,
                                        backup_enabled,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_backup = true;
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("world_play", &w.folder_name),
                                        Some(Icon::Play),
                                        tr(lang, "world.play"),
                                        ButtonVariant::Primary,
                                        can_play,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_play = true;
                                    }
                                }
                                let left_w = (ui.available_width() - 8.0).max(40.0);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_w, 34.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.set_max_width(left_w);
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&w.display_name)
                                                    .strong()
                                                    .color(text_primary()),
                                            )
                                            .truncate(),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "saves/{} • {}",
                                                    w.folder_name,
                                                    format_size(w.size_bytes)
                                                ))
                                                .small()
                                                .color(text_muted()),
                                            )
                                            .truncate(),
                                        );
                                    },
                                );
                            },
                        );
                    } else {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&w.display_name)
                                    .strong()
                                    .color(text_primary()),
                            )
                            .truncate(),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "saves/{} • {}",
                                    w.folder_name,
                                    format_size(w.size_bytes)
                                ))
                                .small()
                                .color(text_muted()),
                            )
                            .truncate(),
                        );
                        ui.add_space(6.0);
                        ui_wrap_row(ui, 28.0, |ui| {
                            if confirming_delete {
                                if ui_button_icon(
                                    ui,
                                    ("world_del_confirm", &w.folder_name),
                                    Some(Icon::Trash),
                                    tr(lang, "world.delete_confirm"),
                                    ButtonVariant::Danger,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_confirm_delete = true;
                                }
                                if ui_button_icon(
                                    ui,
                                    ("world_del_cancel", &w.folder_name),
                                    Some(Icon::Close),
                                    tr(lang, "btn.cancel"),
                                    ButtonVariant::Subtle,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_cancel_delete = true;
                                }
                            } else {
                                if ui_button_icon(
                                    ui,
                                    ("world_play", &w.folder_name),
                                    Some(Icon::Play),
                                    tr(lang, "world.play"),
                                    ButtonVariant::Primary,
                                    can_play,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_play = true;
                                }
                                if ui_button_icon(
                                    ui,
                                    ("world_backup", &w.folder_name),
                                    Some(Icon::Download),
                                    tr(lang, "world.backup"),
                                    ButtonVariant::Secondary,
                                    backup_enabled,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_backup = true;
                                }
                                if ui_button_icon(
                                    ui,
                                    ("world_datapacks_btn", &w.folder_name),
                                    Some(Icon::Layers),
                                    tr(lang, "world.datapacks"),
                                    if self.expanded_world_datapacks.as_deref()
                                        == Some(w.folder_name.as_str())
                                    {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    },
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    if self.expanded_world_datapacks.as_deref()
                                        == Some(w.folder_name.as_str())
                                    {
                                        self.expanded_world_datapacks = None;
                                    } else {
                                        self.expanded_world_datapacks =
                                            Some(w.folder_name.clone());
                                    }
                                }
                                if ui_button_icon(
                                    ui,
                                    ("world_open", &w.folder_name),
                                    Some(Icon::Folder),
                                    tr(lang, "btn.open"),
                                    ButtonVariant::Subtle,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    anim::emit_ring(
                                        ui.ctx(),
                                        anim::pointer_or_center(ui.ctx()),
                                        accent(),
                                        28.0,
                                    );
                                    open_in_file_manager(&w.path);
                                }
                                if ui_button_icon(
                                    ui,
                                    ("world_del", &w.folder_name),
                                    Some(Icon::Trash),
                                    tr(lang, "btn.delete"),
                                    ButtonVariant::Subtle,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_ask_delete = true;
                                }
                            }
                        });
                    }
                });
                if confirming_delete {
                    let p = pulse_01(ui.input(|i| i.time), 5.0);
                    ui.painter().rect_stroke(
                        card_r.response.rect,
                        Rounding::same(10.0),
                        Stroke::new(1.3_f32, with_alpha(DANGER, (110.0 + p * 120.0) as u8)),
                    );
                    ui.ctx().request_repaint();
                }
                if action_play {
                    anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 10);
                    anim::emit_edge_pulse(ui.ctx(), accent());
                    self.start_launch_with_world(Some(w.folder_name.clone()));
                } else if action_backup {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                    self.world_backup_busy = true;
                    let inst_dir = instance_dir(&self.paths, profile);
                    let folder = w.folder_name.clone();
                    let tx = self.tx.clone();
                    self.status_text = format!("{}...", tr(lang, "world.backup"));
                    thread::spawn(move || {
                        let res = backup_world(&inst_dir, &folder);
                        let _ = tx.send(WorkerEvent::WorldBackupFinished(res));
                    });
                } else if action_ask_delete {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 28.0);
                    self.pending_world_delete = Some(w.folder_name.clone());
                } else if action_cancel_delete {
                    self.pending_world_delete = None;
                } else if action_confirm_delete {
                    anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 8);
                    let inst_dir = instance_dir(&self.paths, profile);
                    self.pending_world_delete = None;
                    match delete_world(&inst_dir, &w.folder_name) {
                        Ok(()) => {
                            self.show_toast(
                                ui.ctx(),
                                format!("{}: {}", tr(lang, "world.deleted"), w.display_name),
                            );
                        }
                        Err(e) => {
                            self.show_toast(
                                ui.ctx(),
                                format!("{}: {e}", tr(lang, "status.error")),
                            );
                        }
                    }
                }
                if self.expanded_world_datapacks.as_deref() == Some(w.folder_name.as_str()) {
                    ui.add_space(4.0);
                    let dp_list = list_world_datapacks(&inst, &w.folder_name);
                    let mut dp_toggle: Option<String> = None;
                    let mut dp_delete: Option<String> = None;
                    let mut dp_import = false;
                    let mut dp_browse = false;
                    card_frame(false, 0.0).show(ui, |ui| {
                        ui.set_min_width(card_inner_w);
                        ui.set_max_width(card_inner_w);
                        ui_wrap_row(ui, 28.0, |ui| {
                            section_header(
                                ui,
                                &format!(
                                    "{} — {} ({})",
                                    w.display_name,
                                    tr(lang, "world.datapacks"),
                                    dp_list.len()
                                ),
                            );
                            if ui_button_icon(
                                ui,
                                ("world_dp_import", &w.folder_name),
                                Some(Icon::Plus),
                                tr(lang, "world.import_datapack"),
                                ButtonVariant::Primary,
                                !self.file_picker_open,
                                26.0,
                            )
                            .clicked()
                            {
                                dp_import = true;
                            }
                            if ui_button_icon(
                                ui,
                                ("world_dp_modrinth", &w.folder_name),
                                Some(Icon::Sparkle),
                                "Modrinth",
                                ButtonVariant::Secondary,
                                true,
                                26.0,
                            )
                            .clicked()
                            {
                                dp_browse = true;
                            }
                        });
                        ui.add_space(4.0);
                        for dp in &dp_list {
                            ui.allocate_ui_with_layout(
                                Vec2::new(card_inner_w, 26.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(card_inner_w);
                                    if ui_icon_button(
                                        ui,
                                        ("world_dp_del", &w.folder_name, &dp.disk_name),
                                        Icon::Trash,
                                        tr(lang, "btn.delete"),
                                        ButtonVariant::Subtle,
                                        true,
                                        24.0,
                                    )
                                    .clicked()
                                    {
                                        dp_delete = Some(dp.disk_name.clone());
                                    }
                                    let left_w = (ui.available_width() - 6.0).max(40.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(left_w, 26.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.set_max_width(left_w);
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let sw_id = format!(
                                                "world_dp_{}_{}",
                                                w.folder_name, dp.base_name
                                            );
                                            if mod_toggle_switch(ui, &sw_id, dp.enabled).clicked() {
                                                dp_toggle = Some(dp.disk_name.clone());
                                            }
                                            ui.label(
                                                egui::RichText::new(format_size(dp.size_bytes))
                                                    .small()
                                                    .color(text_muted()),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&dp.base_name).color(
                                                        if dp.enabled {
                                                            text_primary()
                                                        } else {
                                                            text_muted()
                                                        },
                                                    ),
                                                )
                                                .truncate(),
                                            );
                                        },
                                    );
                                },
                            );
                        }
                    });
                    if dp_import {
                        self.request_file_pick(FilePickPurpose::WorldDatapackImport {
                            world_folder: w.folder_name.clone(),
                        });
                    }
                    if dp_browse {
                        self.discover_target_server_id = None;
                        self.mr_type = ModrinthProjectType::Datapack;
                        self.mr_loaded_filter_key.clear();
                        let ctx = ui.ctx().clone();
                        self.switch_page(&ctx, NavPage::Discover);
                        self.trigger_modrinth_search(false);
                    }
                    if let Some(dname) = dp_toggle {
                        if let Err(e) = toggle_world_datapack(&inst, &w.folder_name, &dname) {
                            self.show_toast(ui.ctx(), e);
                        }
                    }
                    if let Some(dname) = dp_delete {
                        if let Err(e) = delete_world_datapack(&inst, &w.folder_name, &dname) {
                            self.show_toast(ui.ctx(), e);
                        }
                    }
                }
                ui.add_space(5.0);
            }

            if !backups.is_empty() {
                let lang = self.settings.language;
                ui.add_space(8.0);
                ui_toolbar_row(ui, 28.0, |ui| {
                    section_header(
                        ui,
                        &format!("{} ({})", tr(lang, "world.backups"), backups.len()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui_button_icon(
                            ui,
                            "world_backups_open_dir",
                            Some(Icon::Folder),
                            tr(lang, "btn.open_folder"),
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .clicked()
                        {
                            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
                            open_in_file_manager(&inst.join("backups"));
                        }
                    });
                });
                ui.add_space(4.0);
                for b in backups {
                    let backup_enabled = !self.world_backup_busy;
                    let confirming_del =
                        self.pending_backup_delete.as_deref() == Some(b.file_name.as_str());
                    let mut action_restore = false;
                    let mut action_ask_del = false;
                    let mut action_cancel_del = false;
                    let mut action_confirm_del = false;

                    let b_id =
                        ui.make_persistent_id(("world_backup_card", &profile.id, &b.file_name));
                    let b_card = animated_card(ui, b_id, false, |ui| {
                        ui.set_min_width(card_inner_w);
                        ui.set_max_width(card_inner_w);
                        if card_inner_w >= 460.0 {
                            ui.allocate_ui_with_layout(
                                Vec2::new(card_inner_w, 34.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if confirming_del {
                                        if ui_button_icon(
                                            ui,
                                            ("backup_del_yes", &b.file_name),
                                            Some(Icon::Trash),
                                            tr(lang, "btn.delete"),
                                            ButtonVariant::Danger,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                        {
                                            action_confirm_del = true;
                                        }
                                        if ui_button_icon(
                                            ui,
                                            ("backup_del_no", &b.file_name),
                                            Some(Icon::Close),
                                            tr(lang, "btn.cancel"),
                                            ButtonVariant::Subtle,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                        {
                                            action_cancel_del = true;
                                        }
                                    } else {
                                        if ui_button_icon(
                                            ui,
                                            ("backup_del", &b.file_name),
                                            Some(Icon::Trash),
                                            tr(lang, "btn.delete"),
                                            ButtonVariant::Subtle,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                        {
                                            action_ask_del = true;
                                        }
                                        if ui_button_icon(
                                            ui,
                                            ("backup_restore", &b.file_name),
                                            Some(Icon::Refresh),
                                            tr(lang, "world.restore"),
                                            ButtonVariant::Secondary,
                                            backup_enabled,
                                            26.0,
                                        )
                                        .clicked()
                                        {
                                            action_restore = true;
                                        }
                                    }
                                    let left_w = (ui.available_width() - 8.0).max(40.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(left_w, 34.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_max_width(left_w);
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&b.file_name)
                                                        .strong()
                                                        .color(text_primary()),
                                                )
                                                .truncate(),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(format!(
                                                        "backups/{} • {}",
                                                        b.file_name,
                                                        format_size(b.size_bytes)
                                                    ))
                                                    .small()
                                                    .color(text_muted()),
                                                )
                                                .truncate(),
                                            );
                                        },
                                    );
                                },
                            );
                        } else {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&b.file_name)
                                        .strong()
                                        .color(text_primary()),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "backups/{} • {}",
                                        b.file_name,
                                        format_size(b.size_bytes)
                                    ))
                                    .small()
                                    .color(text_muted()),
                                )
                                .truncate(),
                            );
                            ui.add_space(6.0);
                            ui_wrap_row(ui, 26.0, |ui| {
                                if confirming_del {
                                    if ui_button_icon(
                                        ui,
                                        ("backup_del_yes", &b.file_name),
                                        Some(Icon::Trash),
                                        tr(lang, "btn.delete"),
                                        ButtonVariant::Danger,
                                        true,
                                        26.0,
                                    )
                                    .clicked()
                                    {
                                        action_confirm_del = true;
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("backup_del_no", &b.file_name),
                                        Some(Icon::Close),
                                        tr(lang, "btn.cancel"),
                                        ButtonVariant::Subtle,
                                        true,
                                        26.0,
                                    )
                                    .clicked()
                                    {
                                        action_cancel_del = true;
                                    }
                                } else {
                                    if ui_button_icon(
                                        ui,
                                        ("backup_restore", &b.file_name),
                                        Some(Icon::Refresh),
                                        tr(lang, "world.restore"),
                                        ButtonVariant::Secondary,
                                        backup_enabled,
                                        26.0,
                                    )
                                    .clicked()
                                    {
                                        action_restore = true;
                                    }
                                    if ui_button_icon(
                                        ui,
                                        ("backup_del", &b.file_name),
                                        Some(Icon::Trash),
                                        tr(lang, "btn.delete"),
                                        ButtonVariant::Subtle,
                                        true,
                                        26.0,
                                    )
                                    .clicked()
                                    {
                                        action_ask_del = true;
                                    }
                                }
                            });
                        }
                    });
                    if confirming_del {
                        let p = pulse_01(ui.input(|i| i.time), 5.0);
                        ui.painter().rect_stroke(
                            b_card.response.rect,
                            Rounding::same(10.0),
                            Stroke::new(1.3_f32, with_alpha(DANGER, (110.0 + p * 120.0) as u8)),
                        );
                        ui.ctx().request_repaint();
                    }
                    if action_restore {
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                        self.world_backup_busy = true;
                        let inst_dir = instance_dir(&self.paths, profile);
                        let fname = b.file_name.clone();
                        let tx = self.tx.clone();
                        self.status_text = format!("{}...", tr(lang, "world.restore"));
                        thread::spawn(move || {
                            let res = restore_world_backup(&inst_dir, &fname);
                            let _ = tx.send(WorkerEvent::WorldRestoreFinished(res));
                        });
                    } else if action_ask_del {
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 28.0);
                        self.pending_backup_delete = Some(b.file_name.clone());
                    } else if action_cancel_del {
                        self.pending_backup_delete = None;
                    } else if action_confirm_del {
                        anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 8);
                        let inst_dir = instance_dir(&self.paths, profile);
                        self.pending_backup_delete = None;
                        match delete_world_backup(&inst_dir, &b.file_name) {
                            Ok(()) => {
                                self.show_toast(
                                    ui.ctx(),
                                    format!(
                                        "{}: {}",
                                        tr(lang, "world.backup_deleted"),
                                        b.file_name
                                    ),
                                );
                            }
                            Err(e) => {
                                self.show_toast(
                                    ui.ctx(),
                                    format!("{}: {e}", tr(lang, "status.error")),
                                );
                            }
                        }
                    }
                    ui.add_space(4.0);
                }
            }
        });
    }

    pub(super) fn render_screenshots_tab(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let inst = instance_dir(&self.paths, profile);
        let shots_dir = inst.join("screenshots");
        let shots = self.cached_screenshots(profile);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    shots.len(),
                    tr(self.settings.language, "inst.screenshots_count")
                ))
                .small()
                .color(text_secondary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "shots_open_dir",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.open_shots"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    let _ = std::fs::create_dir_all(&shots_dir);
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
                    open_in_file_manager(&shots_dir);
                }
            });
        });
        ui.add_space(6.0);
        bounded_scroll(ui, "instance_shots_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            if shots.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.no_screenshots"))
                            .color(text_secondary()),
                    );
                });
                return;
            }
            let mut shot_to_delete: Option<String> = None;
            for shot in shots {
                let thumb_key = shot.path.to_string_lossy().to_string();
                if !self.screenshot_thumbs.contains_key(&thumb_key)
                    && self.screenshot_thumbs.len() < 120
                {
                    let loaded = decode_screenshot_thumbnail(&shot.path).ok().map(|img| {
                        ui.ctx().load_texture(
                            format!("shot_thumb:{thumb_key}"),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [img.width, img.height],
                                &img.rgba,
                            ),
                            egui::TextureOptions::LINEAR,
                        )
                    });
                    self.screenshot_thumbs.insert(thumb_key.clone(), loaded);
                }
                let thumb_tex = self
                    .screenshot_thumbs
                    .get(&thumb_key)
                    .and_then(|o| o.as_ref())
                    .cloned();

                let card_id = ui.make_persistent_id(("shot_card", &profile.id, &shot.filename));
                animated_card(ui, card_id, false, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    let wide_shot = card_inner_w >= 460.0;
                    if wide_shot {
                        ui.allocate_ui_with_layout(
                            Vec2::new(card_inner_w, 56.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(card_inner_w);
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui_icon_button(
                                    ui,
                                    ("shot_del", &shot.filename),
                                    Icon::Trash,
                                    tr(self.settings.language, "btn.delete"),
                                    ButtonVariant::Subtle,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    shot_to_delete = Some(shot.filename.clone());
                                }
                                if ui_button_icon(
                                    ui,
                                    ("shot_open", &shot.filename),
                                    Some(Icon::External),
                                    tr(self.settings.language, "btn.open_image"),
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    anim::emit_ring(
                                        ui.ctx(),
                                        anim::pointer_or_center(ui.ctx()),
                                        accent(),
                                        28.0,
                                    );
                                    open_in_file_manager(&shot.path);
                                }

                                let left_w = (ui.available_width() - 8.0).max(110.0);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_w, 56.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_max_width(left_w);
                                        ui.spacing_mut().item_spacing.x = 10.0;
                                        let thumb_size = Vec2::new(96.0, 54.0);
                                        let (t_rect, t_resp) =
                                            ui.allocate_exact_size(thumb_size, Sense::click());
                                        ui.painter().rect_filled(
                                            t_rect,
                                            Rounding::same(6.0),
                                            bg_input(),
                                        );
                                        if let Some(ref tex) = thumb_tex {
                                            ui.painter().image(
                                                tex.id(),
                                                t_rect,
                                                Rect::from_min_max(
                                                    Pos2::ZERO,
                                                    Pos2::new(1.0, 1.0),
                                                ),
                                                Color32::WHITE,
                                            );
                                        }
                                        ui.painter().rect_stroke(
                                            t_rect,
                                            Rounding::same(6.0),
                                            Stroke::new(1.0_f32, border_subtle()),
                                        );
                                        if t_resp.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if t_resp.clicked() {
                                            open_in_file_manager(&shot.path);
                                        }

                                        let info_w = (left_w - 108.0).max(40.0);
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(info_w, 50.0),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.set_max_width(info_w);
                                                ui.add_space(6.0);
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&shot.filename)
                                                            .strong()
                                                            .color(text_primary()),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(format_size(
                                                            shot.size_bytes,
                                                        ))
                                                        .small()
                                                        .color(text_muted()),
                                                    )
                                                    .truncate(),
                                                );
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    } else {
                        ui_toolbar_row(ui, 56.0, |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let thumb_size = Vec2::new(88.0, 50.0);
                            let (t_rect, t_resp) =
                                ui.allocate_exact_size(thumb_size, Sense::click());
                            ui.painter().rect_filled(t_rect, Rounding::same(6.0), bg_input());
                            if let Some(ref tex) = thumb_tex {
                                ui.painter().image(
                                    tex.id(),
                                    t_rect,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            ui.painter().rect_stroke(
                                t_rect,
                                Rounding::same(6.0),
                                Stroke::new(1.0_f32, border_subtle()),
                            );
                            if t_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if t_resp.clicked() {
                                open_in_file_manager(&shot.path);
                            }
                            let info_w = (card_inner_w - 100.0).max(40.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(info_w, 50.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_max_width(info_w);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&shot.filename)
                                                .strong()
                                                .color(text_primary()),
                                        )
                                        .truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format_size(shot.size_bytes))
                                                .small()
                                                .color(text_muted()),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
                        });
                        ui.add_space(6.0);
                        ui_wrap_row(ui, 28.0, |ui| {
                            if ui_button_icon(
                                ui,
                                ("shot_open_n", &shot.filename),
                                Some(Icon::External),
                                tr(self.settings.language, "btn.open_image"),
                                ButtonVariant::Secondary,
                                true,
                                28.0,
                            )
                            .clicked()
                            {
                                anim::emit_ring(
                                    ui.ctx(),
                                    anim::pointer_or_center(ui.ctx()),
                                    accent(),
                                    28.0,
                                );
                                open_in_file_manager(&shot.path);
                            }
                            if ui_icon_button(
                                ui,
                                ("shot_del_n", &shot.filename),
                                Icon::Trash,
                                tr(self.settings.language, "btn.delete"),
                                ButtonVariant::Subtle,
                                true,
                                28.0,
                            )
                            .clicked()
                            {
                                shot_to_delete = Some(shot.filename.clone());
                            }
                        });
                    }
                });
                ui.add_space(4.0);
            }
            if let Some(fname) = shot_to_delete {
                match delete_screenshot(&inst, &fname) {
                    Ok(()) => {
                        self.fs_cache.invalidate();
                        let key = shots_dir.join(&fname).to_string_lossy().to_string();
                        self.screenshot_thumbs.remove(&key);
                    }
                    Err(e) => self.show_toast(ui.ctx(), e),
                }
            }
        });
    }

    pub(super) fn render_content_manager(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let avail_w = ui.available_width().max(120.0);
        let inst = instance_dir(&self.paths, profile);
        let folder_name = self.content_folder.dir_name();
        let target_dir = inst.join(folder_name);
        let items = self.cached_content_items(profile, folder_name);
        let total_count = items.len();
        let enabled_count = items.iter().filter(|i| i.enabled).count();
        let disabled_count = total_count.saturating_sub(enabled_count);

        // Vanilla mods notice banner
        if !profile.loader.is_modded() && self.content_folder == ContentFolder::Mods {
            ui_alert_banner(
                ui,
                AlertKind::Info,
                "This instance is using Vanilla. Switch to Fabric, Quilt, Forge, or NeoForge in Instance Settings to load mods.",
                avail_w,
            );
            ui.add_space(6.0);
        }

        let mut go_discover = false;
        let lang = self.settings.language;

        let folder_labels: [(ContentFolder, String); 3] = ContentFolder::ALL.map(|f| {
            let count = self.folder_content_count(profile, f.dir_name());
            (f, format!("{} ({count})", f.translated_label(lang)))
        });
        let current_folder_lbl = folder_labels
            .iter()
            .find(|(f, _)| *f == self.content_folder)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let state_label = |s: ContentStateFilter| match s {
            ContentStateFilter::All => format!("{} ({total_count})", tr(lang, "lib.all")),
            ContentStateFilter::Enabled => format!("{} ({enabled_count})", tr(lang, "content.enabled")),
            ContentStateFilter::Disabled => format!("{} ({disabled_count})", tr(lang, "content.disabled")),
        };

        let mut folder_val = self.content_folder;
        let mut state_val = self.content_state_filter;
        let can_import = !self.mr_installing && !self.file_picker_open;
        let show_actions = total_count > 0 || !self.mr_installed_cache.is_empty();
        let mr_installing = self.mr_installing;
        let identifying_mods = self.identifying_mods;
        let actions_lbl = if mr_installing {
            tr(lang, "content.checking")
        } else if identifying_mods {
            tr(lang, "content.identifying")
        } else {
            tr(lang, "btn.actions")
        };

        let mut trigger_import = false;
        let mut act_check_updates = false;
        let mut act_identify = false;
        let mut act_enable_all = false;
        let mut act_disable_all = false;

        let mut render_folder_combo = |ui: &mut egui::Ui, w: f32| {
            egui::ComboBox::from_id_salt("content_folder_combo")
                .selected_text(&current_folder_lbl)
                .width(w)
                .truncate()
                .show_ui(ui, |ui| {
                    for (f, lbl) in &folder_labels {
                        ui.selectable_value(&mut folder_val, *f, lbl);
                    }
                });
        };
        let mut render_state_combo = |ui: &mut egui::Ui, w: f32| {
            egui::ComboBox::from_id_salt("content_state_combo")
                .selected_text(state_label(state_val))
                .width(w)
                .truncate()
                .show_ui(ui, |ui| {
                    for s in [
                        ContentStateFilter::All,
                        ContentStateFilter::Enabled,
                        ContentStateFilter::Disabled,
                    ] {
                        ui.selectable_value(&mut state_val, s, state_label(s));
                    }
                });
        };
        let mut render_search_box = |ui: &mut egui::Ui, w: f32| {
            ui_search_input(
                ui,
                "content_search_input",
                &mut self.content_search,
                tr(lang, "content.filter"),
                w,
            );
        };
        let mut render_add_btn = |ui: &mut egui::Ui| {
            if ui_button_icon(
                ui,
                "content_import_local_btn",
                Some(Icon::Plus),
                tr(lang, "btn.add_content"),
                ButtonVariant::Secondary,
                can_import,
                28.0,
            )
            .clicked()
            {
                trigger_import = true;
            }
        };
        let mut render_actions_combo = |ui: &mut egui::Ui, w: f32| {
            if !show_actions {
                return;
            }
            egui::ComboBox::from_id_salt("content_actions_combo")
                .selected_text(actions_lbl)
                .width(w)
                .truncate()
                .show_ui(ui, |ui| {
                    if ui
                        .add_enabled(
                            !mr_installing,
                            egui::Button::new(tr(lang, "btn.check_updates")),
                        )
                        .clicked()
                    {
                        act_check_updates = true;
                        ui.close_menu();
                    }
                    if total_count > 0
                        && ui
                            .add_enabled(
                                !identifying_mods && !mr_installing,
                                egui::Button::new(tr(lang, "btn.identify_mods")),
                            )
                            .clicked()
                    {
                        act_identify = true;
                        ui.close_menu();
                    }
                    if disabled_count > 0 && ui.button(tr(lang, "btn.enable_all")).clicked() {
                        act_enable_all = true;
                        ui.close_menu();
                    }
                    if enabled_count > 0 && ui.button(tr(lang, "btn.disable_all")).clicked() {
                        act_disable_all = true;
                        ui.close_menu();
                    }
                });
        };

        if avail_w >= 880.0 {
            ui.horizontal(|ui| {
                ui.set_max_width(avail_w);
                ui.spacing_mut().item_spacing.x = 6.0;
                render_folder_combo(ui, 148.0);
                render_state_combo(ui, 136.0);
                render_search_box(ui, (avail_w - 600.0).clamp(130.0, 260.0));
                render_add_btn(ui);
                render_actions_combo(ui, 124.0);
            });
        } else if avail_w >= 460.0 {
            ui.horizontal(|ui| {
                ui.set_max_width(avail_w);
                ui.spacing_mut().item_spacing.x = 6.0;
                render_folder_combo(ui, ((avail_w - 28.0) * 0.35).clamp(110.0, 170.0));
                render_state_combo(ui, ((avail_w - 28.0) * 0.33).clamp(110.0, 160.0));
                render_actions_combo(ui, ((avail_w - 28.0) * 0.28).clamp(96.0, 140.0));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.set_max_width(avail_w);
                ui.spacing_mut().item_spacing.x = 6.0;
                render_search_box(ui, (avail_w - 175.0).clamp(110.0, 360.0));
                render_add_btn(ui);
            });
        } else {
            if avail_w >= 260.0 {
                ui.horizontal(|ui| {
                    ui.set_max_width(avail_w);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    render_folder_combo(ui, ((avail_w - 14.0) * 0.5).max(90.0));
                    render_state_combo(ui, ((avail_w - 14.0) * 0.5).max(90.0));
                });
            } else {
                render_folder_combo(ui, (avail_w - 4.0).max(90.0));
                ui.add_space(4.0);
                render_state_combo(ui, (avail_w - 4.0).max(90.0));
            }
            ui.add_space(6.0);
            render_search_box(ui, (avail_w - 4.0).clamp(96.0, 440.0));
            ui.add_space(6.0);
            if avail_w >= 290.0 {
                ui.horizontal(|ui| {
                    ui.set_max_width(avail_w);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    render_add_btn(ui);
                    render_actions_combo(ui, (avail_w - 170.0).clamp(96.0, 140.0));
                });
            } else {
                render_add_btn(ui);
                if show_actions {
                    ui.add_space(4.0);
                    render_actions_combo(ui, (avail_w - 4.0).clamp(96.0, 200.0));
                }
            }
        }

        drop(render_search_box);
        if folder_val != self.content_folder {
            self.content_folder = folder_val;
            self.mark_subtab_switch(ui.ctx());
        }
        if state_val != self.content_state_filter {
            self.content_state_filter = state_val;
            self.mark_subtab_switch(ui.ctx());
        }
        if trigger_import {
            self.request_file_pick(FilePickPurpose::InstanceContentImport);
        }
        if act_check_updates {
            reset_download_cancel();
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
            self.mr_installing = true;
            self.progress = 0.08;
            self.status_text = tr(lang, "content.checking_updates").to_string();
            let paths = self.paths.clone();
            let prof = profile.clone();
            let tx = self.tx.clone();
            thread::spawn(move || {
                let res = update_all_installed_projects(&paths, &prof, &tx);
                let _ = tx.send(WorkerEvent::ModUpdatesFinished(res));
            });
        }
        if act_identify {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
            self.start_identify_mods();
        }
        if act_enable_all {
            match set_all_content_enabled(&target_dir, true) {
                Ok(n) => {
                    self.fs_cache.invalidate();
                    let pos = anim::pointer_or_center(ui.ctx());
                    anim::emit_burst(ui.ctx(), pos, accent(), 8);
                    anim::emit_floating_badge(ui.ctx(), pos, Some(Icon::Check), format!("+{n}"), accent());
                    self.show_toast(
                        ui.ctx(),
                        format!("{}: {n}", tr(lang, "toast.enabled_count")),
                    );
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }
        if act_disable_all {
            match set_all_content_enabled(&target_dir, false) {
                Ok(n) => {
                    self.fs_cache.invalidate();
                    let pos = anim::pointer_or_center(ui.ctx());
                    anim::emit_ring(ui.ctx(), pos, text_secondary(), 30.0);
                    anim::emit_floating_badge(ui.ctx(), pos, None, format!("-{n}"), text_secondary());
                    self.show_toast(
                        ui.ctx(),
                        format!("{}: {n}", tr(lang, "toast.disabled_count")),
                    );
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }

        let folder_name = self.content_folder.dir_name();
        let target_dir = inst.join(folder_name);

        ui.add_space(6.0);
        let q = self.content_search.trim().to_lowercase();
        let indexed_by_file: HashMap<String, &InstalledProject> = self
            .mr_installed_cache
            .values()
            .filter(|p| p.folder == folder_name)
            .map(|p| (p.filename.clone(), p))
            .collect();

        let filtered: Vec<_> = items
            .into_iter()
            .filter(|item| match self.content_state_filter {
                ContentStateFilter::All => true,
                ContentStateFilter::Enabled => item.enabled,
                ContentStateFilter::Disabled => !item.enabled,
            })
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let title_match = indexed_by_file
                    .get(&item.base_name)
                    .is_some_and(|info| info.title.to_lowercase().contains(&q));
                title_match || item.base_name.to_lowercase().contains(&q)
            })
            .collect();

        let mut toggle_disk_name: Option<String> = None;
        let mut delete_disk_name: Option<(String, String)> = None;
        let mut icons_to_request: Vec<String> = Vec::new();

        let avail_h = ui.available_height().max(120.0);
        if filtered.is_empty() {
            bounded_scroll(ui, "instance_content_scroll", |ui| {
                let card_inner_w = (ui.available_width() - 26.0).max(100.0);
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    let msg = if total_count == 0 {
                        format!("{folder_name}/ folder is currently empty.")
                    } else {
                        tr(self.settings.language, "content.no_matches").to_string()
                    };
                    let mut browse_btn = |ui: &mut egui::Ui| {
                        if ui_button_icon(
                            ui,
                            "empty_find_mr_btn",
                            Some(Icon::Compass),
                            tr(self.settings.language, "btn.browse_modrinth"),
                            ButtonVariant::Primary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            go_discover = true;
                        }
                    };
                    if card_inner_w < 380.0 {
                        ui.add(
                            egui::Label::new(egui::RichText::new(msg).color(text_secondary()))
                                .wrap(),
                        );
                        ui.add_space(8.0);
                        browse_btn(ui);
                    } else {
                        ui_wrap_row(ui, 30.0, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(msg).color(text_secondary()),
                                )
                                .wrap(),
                            );
                            browse_btn(ui);
                        });
                    }
                });
            });
        } else {
            let content_w = (avail_w - 14.0).max(100.0);
            let card_inner_w = (content_w - 28.0).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("instance_content_scroll")
                .auto_shrink([false, false])
                .max_width(avail_w)
                .max_height(avail_h)
                .show_rows(ui, 60.0, filtered.len(), |ui, row_range| {
                    ui.set_max_width(content_w);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for item in &filtered[row_range] {
                        let info = indexed_by_file.get(&item.base_name).copied();
                        let display_title = info
                            .map(|i| i.title.clone())
                            .unwrap_or_else(|| item.base_name.clone());
                        if let Some(url) = info.and_then(|i| i.icon_url.as_deref()) {
                            if !self.mr_icons.contains_key(url) {
                                icons_to_request.push(url.to_string());
                            }
                        }

                        let card_id = ui.make_persistent_id((
                            "mod_card_hover",
                            folder_name,
                            &item.base_name,
                        ));
                        animated_card(ui, card_id, false, |ui| {
                            ui.set_min_width(card_inner_w);
                            ui.set_max_width(card_inner_w);
                            let compact_row = card_inner_w < 480.0;
                            let right_pill: Option<(&str, bool)> = if !item.enabled {
                                Some((tr(self.settings.language, "content.disabled"), false))
                            } else {
                                info.map(|inf| (inf.version_number.as_str(), true))
                            };

                            ui.allocate_ui_with_layout(
                                Vec2::new(card_inner_w, 38.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(card_inner_w);
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let del_clicked = if compact_row {
                                        ui_icon_button(
                                            ui,
                                            ("mod_del", &item.disk_name),
                                            Icon::Trash,
                                            &format!("Remove {}", item.disk_name),
                                            ButtonVariant::Danger,
                                            true,
                                            28.0,
                                        )
                                        .clicked()
                                    } else {
                                        ui_button_icon(
                                            ui,
                                            ("mod_del", &item.disk_name),
                                            Some(Icon::Trash),
                                            tr(self.settings.language, "btn.remove"),
                                            ButtonVariant::Danger,
                                            true,
                                            28.0,
                                        )
                                        .on_hover_text(format!("Remove {}", item.disk_name))
                                        .clicked()
                                    };
                                    if del_clicked {
                                        delete_disk_name =
                                            Some((item.disk_name.clone(), display_title.clone()));
                                    }
                                    if ui.available_width() > 210.0 {
                                        if let Some((pill_txt, highlighted)) = right_pill {
                                            badge_sized(
                                                ui,
                                                pill_txt,
                                                highlighted,
                                                28.0,
                                                Some(108.0),
                                            )
                                            .on_hover_text(pill_txt);
                                        }
                                    }

                                    let left_w = (ui.available_width() - 4.0).max(40.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(left_w, 36.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.set_max_width(left_w);
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            if mod_toggle_switch(
                                                ui,
                                                &format!(
                                                    "{}_{}_{}",
                                                    profile.id, folder_name, item.base_name
                                                ),
                                                item.enabled,
                                            )
                                            .on_hover_text(if item.enabled {
                                                tr(self.settings.language, "content.disabled")
                                            } else {
                                                tr(self.settings.language, "content.enable")
                                            })
                                            .clicked()
                                            {
                                                toggle_disk_name = Some(item.disk_name.clone());
                                            }

                                            let icon_url_opt = info.and_then(|i| i.icon_url.as_ref());
                                            if left_w >= 120.0 {
                                                let icon_px = 30.0;
                                                let tex_opt = icon_url_opt
                                                    .and_then(|u| self.mr_icons.get(u))
                                                    .and_then(|o| o.as_ref());
                                                let is_icon_loading = icon_url_opt.is_some_and(|u| {
                                                    self.mr_icons_pending.contains(u.as_str())
                                                });
                                                let icon_size = Vec2::splat(icon_px);
                                                if let Some(tex) = tex_opt {
                                                    ui.add(
                                                        egui::Image::new(tex)
                                                            .fit_to_exact_size(icon_size)
                                                            .rounding(Rounding::same(6.5)),
                                                    );
                                                } else {
                                                    let (rect, _) = ui.allocate_exact_size(
                                                        icon_size,
                                                        Sense::hover(),
                                                    );
                                                    draw_icon_placeholder(
                                                        ui,
                                                        rect,
                                                        6.5,
                                                        &display_title,
                                                        is_icon_loading,
                                                    );
                                                }
                                            }

                                            let mid_w = ui.available_width().max(16.0);
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(mid_w, 34.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.set_max_width(mid_w);
                                                    let mut title_rt =
                                                        egui::RichText::new(&display_title)
                                                            .strong();
                                                    if !item.enabled {
                                                        title_rt = title_rt
                                                            .strikethrough()
                                                            .color(text_muted());
                                                    } else {
                                                        title_rt = title_rt.color(text_primary());
                                                    }
                                                    ui.add(egui::Label::new(title_rt).truncate());
                                                    let src_tag = if let Some(inf) = info {
                                                        inf.source.label()
                                                    } else {
                                                        tr(self.settings.language, "content.local")
                                                    };
                                                    let meta = format!(
                                                        "{src_tag} • {} • {}",
                                                        item.disk_name,
                                                        format_size(item.size_bytes)
                                                    );
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(meta)
                                                                .monospace()
                                                                .small()
                                                                .color(text_muted()),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            );
                                        },
                                    );
                                },
                            );
                        });
                    }
                });
        }
        for url in icons_to_request {
            self.request_icon(&url);
        }

        if let Some(name) = toggle_disk_name {
            let lang = self.settings.language;
            match toggle_content_file(&target_dir, &name) {
                Ok(now_on) => {
                    self.fs_cache.invalidate();
                    let pos = anim::pointer_or_center(ui.ctx());
                    let badge_col = if now_on { accent() } else { text_secondary() };
                    anim::emit_ring(ui.ctx(), pos, badge_col, 26.0);
                    anim::emit_floating_badge(
                        ui.ctx(),
                        pos,
                        if now_on { Some(Icon::Check) } else { None },
                        if now_on { "" } else { "-" },
                        badge_col,
                    );
                    let msg = if now_on {
                        format!(
                            "{}: {}",
                            tr(lang, "toast.enabled_count"),
                            name.trim_end_matches(".disabled")
                        )
                    } else {
                        format!("{}: {name}", tr(lang, "toast.disabled_count"))
                    };
                    self.show_toast(ui.ctx(), msg);
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }
        if let Some((disk_name, title)) = delete_disk_name {
            let lang = self.settings.language;
            match delete_content_file(&self.paths, profile, folder_name, &disk_name) {
                Ok(()) => {
                    self.fs_cache.invalidate();
                    self.mr_installed_cache = load_installed_index(&self.paths, profile);
                    anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 8);
                    self.show_toast(ui.ctx(), format!("{}: {title}", tr(lang, "toast.removed")));
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }
        if go_discover {
            self.mr_type = match self.content_folder {
                ContentFolder::Mods => ModrinthProjectType::Mod,
                ContentFolder::Resourcepacks => ModrinthProjectType::Resourcepack,
                ContentFolder::Shaderpacks => ModrinthProjectType::Shader,
            };
            self.switch_page(ui.ctx(), NavPage::Discover);
            self.trigger_modrinth_search(false);
        }
    }

    pub(super) fn render_logs_tab(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let lang = self.settings.language;
        let inst = instance_dir(&self.paths, profile);
        if self.inst_log_files_pid != profile.id {
            self.inst_log_files = list_instance_log_files(&inst);
            self.inst_log_files_pid = profile.id.clone();
            self.inst_log_selected = self.inst_log_files.first().map(|f| f.path.clone());
            self.inst_log_content = self
                .inst_log_selected
                .as_deref()
                .and_then(|p| read_instance_log_file(p).ok())
                .unwrap_or_default();
        }

        let mut refresh_list = false;
        let mut selected_new: Option<std::path::PathBuf> = None;
        let mut upload_clicked = false;
        let mut copy_clicked = false;
        let mut open_folder_target: Option<std::path::PathBuf> = None;

        ui_wrap_row(ui, 30.0, |ui| {
            if ui_button_icon(
                ui,
                "inst_logs_refresh_btn",
                Some(Icon::Refresh),
                tr(lang, "btn.refresh"),
                ButtonVariant::Secondary,
                true,
                28.0,
            )
            .clicked()
            {
                refresh_list = true;
            }
            if ui_button_icon(
                ui,
                "inst_logs_open_crash_btn",
                Some(Icon::Folder),
                tr(lang, "logs.open_folder"),
                ButtonVariant::Secondary,
                true,
                28.0,
            )
            .clicked()
            {
                let crash_dir = inst.join("crash-reports");
                open_folder_target = Some(if crash_dir.is_dir() {
                    crash_dir
                } else {
                    inst.join("logs")
                });
            }
            if !self.inst_log_content.is_empty() {
                if ui_button_icon(
                    ui,
                    "inst_logs_copy_btn",
                    Some(Icon::Copy),
                    tr(lang, "btn.copy_all"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    copy_clicked = true;
                }
                if ui_button_icon(
                    ui,
                    "inst_logs_mclogs_btn",
                    Some(Icon::External),
                    if self.mclogs_uploading {
                        tr(lang, "logs.uploading")
                    } else {
                        tr(lang, "logs.upload_mclogs")
                    },
                    ButtonVariant::Primary,
                    !self.mclogs_uploading,
                    28.0,
                )
                .clicked()
                {
                    upload_clicked = true;
                }
            }
            ui_search_input(
                ui,
                "inst_logs_filter",
                &mut self.inst_log_filter,
                tr(lang, "console.filter"),
                (ui.available_width() - 10.0).clamp(110.0, 240.0),
            );
        });

        if let Some(url) = &self.mclogs_last_url {
            ui.add_space(6.0);
            let mut copy_url = false;
            card_frame(false, 0.0).show(ui, |ui| {
                ui_wrap_row(ui, 26.0, |ui| {
                    ui.label(egui::RichText::new("mclo.gs:").strong().color(accent()));
                    ui.hyperlink_to(egui::RichText::new(url).monospace(), url);
                    if ui_button_icon(
                        ui,
                        "mclogs_copy_url_btn",
                        Some(Icon::Copy),
                        tr(lang, "btn.copy"),
                        ButtonVariant::Secondary,
                        true,
                        24.0,
                    )
                    .clicked()
                    {
                        copy_url = true;
                    }
                });
            });
            if copy_url {
                ui.ctx().copy_text(url.clone());
                self.show_toast(ui.ctx(), tr(lang, "toast.logs_copied").to_string());
            }
        }

        ui.add_space(6.0);
        if self.inst_log_files.is_empty() {
            card_frame(false, 0.0).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(tr(lang, "logs.empty")).color(text_secondary()),
                );
            });
        } else {
            let log_label = |entry: &InstanceLogFile| {
                let kind_prefix = match entry.kind {
                    LogFileKind::CrashReport => "CRASH",
                    LogFileKind::Log => "LOG",
                };
                format!(
                    "[{kind_prefix}] {} ({})",
                    entry.filename,
                    format_size(entry.size_bytes)
                )
            };
            let current_lbl = self
                .inst_log_files
                .iter()
                .find(|e| self.inst_log_selected.as_ref() == Some(&e.path))
                .map(log_label)
                .unwrap_or_else(|| "Select log".to_string());
            let combo_w = (ui.available_width() - 16.0).clamp(180.0, 420.0);
            egui::ComboBox::from_id_salt("inst_log_file_combo")
                .selected_text(current_lbl)
                .width(combo_w)
                .truncate()
                .show_ui(ui, |ui| {
                    for entry in &self.inst_log_files {
                        let is_sel = self.inst_log_selected.as_ref() == Some(&entry.path);
                        if ui.selectable_label(is_sel, log_label(entry)).clicked() && !is_sel {
                            selected_new = Some(entry.path.clone());
                        }
                    }
                });
            ui.add_space(6.0);

            let filter_q = self.inst_log_filter.trim().to_lowercase();
            let content = self.inst_log_content.clone();
            egui::Frame::none()
                .fill(bg_void())
                .rounding(Rounding::same(8.0))
                .stroke(Stroke::new(1.0_f32, border_subtle()))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    ui.set_min_width((ui.available_width() - 4.0).max(140.0));
                    bounded_scroll(ui, "inst_log_viewer_scroll", |ui| {
                        for line in content.lines() {
                            if !filter_q.is_empty() && !line.to_lowercase().contains(&filter_q) {
                                continue;
                            }
                            let low = line.to_lowercase();
                            let col = if low.contains("error")
                                || low.contains("exception")
                                || low.contains("fatal")
                            {
                                DANGER
                            } else if low.contains("warn") {
                                WARNING
                            } else {
                                text_secondary()
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(line).monospace().size(11.5).color(col),
                                )
                                .wrap(),
                            );
                        }
                    });
                });
        }

        if refresh_list {
            self.inst_log_files = list_instance_log_files(&inst);
            if let Some(sel) = &self.inst_log_selected {
                self.inst_log_content = read_instance_log_file(sel).unwrap_or_default();
            }
        }
        if let Some(p) = selected_new {
            self.inst_log_content = read_instance_log_file(&p).unwrap_or_else(|e| e);
            self.inst_log_selected = Some(p);
        }
        if copy_clicked {
            ui.ctx().copy_text(self.inst_log_content.clone());
            self.show_toast(ui.ctx(), tr(lang, "toast.logs_copied").to_string());
        }
        if upload_clicked {
            self.start_mclogs_upload(self.inst_log_content.clone());
        }
        if let Some(dir) = open_folder_target {
            let _ = std::fs::create_dir_all(&dir);
            open_in_file_manager(&dir);
        }
    }

    pub(super) fn render_instance_settings_tab(&mut self, ui: &mut egui::Ui) {
        if self.profiles.is_empty() {
            ui.label(tr(self.settings.language, "inst.none"));
            return;
        }
        if self.selected_idx >= self.profiles.len() {
            self.selected_idx = 0;
        }
        let profile_ver = self.profiles[self.selected_idx].version_id.clone();
        let loader_kind = self.profiles[self.selected_idx].loader.kind();
        if loader_kind.is_modded() {
            self.request_loader_versions(loader_kind, &profile_ver);
        }

        let mut changed = false;
        let filtered_versions: Vec<String> = self
            .manifest
            .as_ref()
            .map(|m| {
                m.versions
                    .iter()
                    .filter(|v| self.show_snapshots || v.version_type == VersionType::Release)
                    .map(|v| v.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let loader_list = loader_kind
            .is_modded()
            .then(|| self.loader_cache.get(&(loader_kind, profile_ver)).cloned())
            .flatten();
        let java_installs = self.java_installs.clone();
        let java_scanning = self.java_scanning;
        let mut rescan_java = false;
        let mut trigger_dup = false;
        let mut trigger_del = false;
        let mut trigger_pick_icon = false;
        let mut trigger_pick_java = false;
        let mut rename_dir = false;
        let can_delete = self.profiles.len() > 1;
        let selected_idx = self.selected_idx;

        bounded_scroll(ui, "profile_tab_scroll", |ui| {
            let profile = &mut self.profiles[selected_idx];
            let card_inner_w = (ui.available_width() - 26.0).max(100.0);
            let field_w = (card_inner_w - 4.0).clamp(80.0, 520.0);
            let wide_grid = card_inner_w >= 640.0;

            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                let dup_clicked = |ui: &mut egui::Ui| {
                    ui_button_icon(
                        ui,
                        "cfg_dup_inst",
                        Some(Icon::Duplicate),
                        tr(self.settings.language, "btn.duplicate"),
                        ButtonVariant::Subtle,
                        true,
                        24.0,
                    )
                    .clicked()
                };
                let del_clicked = |ui: &mut egui::Ui| {
                    can_delete
                        && ui_button_icon(
                            ui,
                            "cfg_del_inst",
                            Some(Icon::Trash),
                            tr(self.settings.language, "btn.delete"),
                            ButtonVariant::Danger,
                            true,
                            24.0,
                        )
                        .clicked()
                };
                if card_inner_w >= 410.0 {
                    ui_toolbar_row(ui, 24.0, |ui| {
                        section_header(ui, tr(self.settings.language, "inst.sec_loader"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if del_clicked(ui) {
                                trigger_del = true;
                            }
                            if dup_clicked(ui) {
                                trigger_dup = true;
                            }
                        });
                    });
                } else {
                    section_header(ui, tr(self.settings.language, "inst.sec_loader"));
                    ui.add_space(4.0);
                    ui_wrap_row(ui, 24.0, |ui| {
                        if dup_clicked(ui) {
                            trigger_dup = true;
                        }
                        if del_clicked(ui) {
                            trigger_del = true;
                        }
                    });
                }
                ui.add_space(6.0);

                let mut request_pick = false;
                let mut reset_icon = false;
                if wide_grid {
                    let col1_w = (card_inner_w - 190.0).clamp(160.0, 420.0);
                    egui::Grid::new("profile_main_grid")
                        .num_columns(2)
                        .spacing([16.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(tr(self.settings.language, "inst.avatar"));
                            render_instance_avatar_controls(
                                ui,
                                self.settings.language,
                                "cfg_inst_avatar_w",
                                profile,
                                &self.profile_icons,
                                &mut request_pick,
                                &mut reset_icon,
                            );
                            ui.end_row();

                            ui.label(tr(self.settings.language, "inst.name"));
                            let name_resp = ui_text_input(
                                ui,
                                "cfg_inst_name_w",
                                &mut profile.name,
                                tr(self.settings.language, "inst.name_hint"),
                                col1_w.min(360.0),
                                false,
                            );
                            if name_resp.changed() {
                                changed = true;
                            }
                            if name_resp.lost_focus() {
                                rename_dir = true;
                            }
                            ui.end_row();

                            ui.label(tr(self.settings.language, "inst.mc_version"));
                            ui.scope(|ui| {
                                ui.set_max_width(col1_w);
                                ui_wrap_row(ui, 30.0, |ui| {
                                    egui::ComboBox::from_id_salt("version_combo")
                                        .selected_text(&profile.version_id)
                                        .width(col1_w.min(160.0))
                                        .truncate()
                                        .show_ui(ui, |ui| {
                                            if filtered_versions.is_empty() {
                                                ui.label(tr(self.settings.language, "inst.loading_versions"));
                                            } else {
                                                for vid in &filtered_versions {
                                                    if ui
                                                        .selectable_value(
                                                            &mut profile.version_id,
                                                            vid.clone(),
                                                            vid,
                                                        )
                                                        .changed()
                                                    {
                                                        if let Some(slot) =
                                                            profile.loader.loader_version_mut()
                                                        {
                                                            slot.clear();
                                                        }
                                                        anim::emit_ring(
                                                            ui.ctx(),
                                                            anim::pointer_or_center(ui.ctx()),
                                                            accent(),
                                                            28.0,
                                                        );
                                                        changed = true;
                                                    }
                                                }
                                            }
                                        });
                                    ui_checkbox(
                                        ui,
                                        "cfg_snapshots_w",
                                        &mut self.show_snapshots,
                                        tr(self.settings.language, "inst.snapshots"),
                                    );
                                });
                            });
                            ui.end_row();

                            ui.label(tr(self.settings.language, "inst.mod_loader"));
                            if render_loader_kind_pills(ui, "inst_cfg_w", &mut profile.loader) {
                                anim::emit_ring(
                                    ui.ctx(),
                                    anim::pointer_or_center(ui.ctx()),
                                    accent(),
                                    30.0,
                                );
                                changed = true;
                            }
                            ui.end_row();

                            let kind = profile.loader.kind();
                            if let Some(loader_version) = profile.loader.loader_version_mut() {
                                ui.label(format!("{} Version:", kind.label()));
                                if render_loader_version_picker(
                                    ui,
                                    "loader_ver_combo",
                                    col1_w.min(220.0),
                                    kind,
                                    loader_version,
                                    loader_list.as_deref(),
                                ) {
                                    anim::emit_ring(
                                        ui.ctx(),
                                        anim::pointer_or_center(ui.ctx()),
                                        accent(),
                                        28.0,
                                    );
                                    changed = true;
                                }
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.avatar"))
                            .small()
                            .color(text_secondary()),
                    );
                    render_instance_avatar_controls(
                        ui,
                        self.settings.language,
                        "cfg_inst_avatar_n",
                        profile,
                        &self.profile_icons,
                        &mut request_pick,
                        &mut reset_icon,
                    );
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.name"))
                            .small()
                            .color(text_secondary()),
                    );
                    let name_resp = ui_text_input(
                        ui,
                        "cfg_inst_name_n",
                        &mut profile.name,
                        tr(self.settings.language, "inst.name_hint"),
                        field_w,
                        false,
                    );
                    if name_resp.changed() {
                        changed = true;
                    }
                    if name_resp.lost_focus() {
                        rename_dir = true;
                    }
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.mc_version"))
                            .small()
                            .color(text_secondary()),
                    );
                    ui_wrap_row(ui, 30.0, |ui| {
                        egui::ComboBox::from_id_salt("version_combo")
                            .selected_text(&profile.version_id)
                            .width(field_w.min(160.0))
                            .truncate()
                            .show_ui(ui, |ui| {
                                if filtered_versions.is_empty() {
                                    ui.label(tr(self.settings.language, "inst.loading_versions"));
                                } else {
                                    for vid in &filtered_versions {
                                        if ui
                                            .selectable_value(
                                                &mut profile.version_id,
                                                vid.clone(),
                                                vid,
                                            )
                                            .changed()
                                        {
                                            if let Some(slot) = profile.loader.loader_version_mut()
                                            {
                                                slot.clear();
                                            }
                                            changed = true;
                                        }
                                    }
                                }
                            });
                        ui_checkbox(
                            ui,
                            "cfg_snapshots_n",
                            &mut self.show_snapshots,
                            tr(self.settings.language, "inst.snapshots"),
                        );
                    });
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.mod_loader"))
                            .small()
                            .color(text_secondary()),
                    );
                    if render_loader_kind_pills(ui, "inst_cfg_n", &mut profile.loader) {
                        anim::emit_ring(
                            ui.ctx(),
                            anim::pointer_or_center(ui.ctx()),
                            accent(),
                            30.0,
                        );
                        changed = true;
                    }

                    let kind = profile.loader.kind();
                    if let Some(loader_version) = profile.loader.loader_version_mut() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!("{} Version:", kind.label()))
                                .small()
                                .color(text_secondary()),
                        );
                        if render_loader_version_picker(
                            ui,
                            "loader_ver_combo",
                            (field_w - 28.0).clamp(90.0, 260.0),
                            kind,
                            loader_version,
                            loader_list.as_deref(),
                        ) {
                            anim::emit_ring(
                                ui.ctx(),
                                anim::pointer_or_center(ui.ctx()),
                                accent(),
                                28.0,
                            );
                            changed = true;
                        }
                    }
                }
                if request_pick {
                    trigger_pick_icon = true;
                }
                if reset_icon {
                    profile.icon_path.clear();
                    anim::emit_ring(
                        ui.ctx(),
                        anim::pointer_or_center(ui.ctx()),
                        text_secondary(),
                        28.0,
                    );
                    changed = true;
                }
            });

            ui.add_space(12.0);

            // Memory Allocation Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                let ram_badge_text =
                    format!("-Xms{}M  -Xmx{}M", profile.min_ram_mb, profile.max_ram_mb);
                if card_inner_w >= 370.0 {
                    ui_toolbar_row(ui, 24.0, |ui| {
                        section_header(ui, tr(self.settings.language, "inst.sec_ram"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            badge(ui, &ram_badge_text, true);
                        });
                    });
                } else {
                    section_header(ui, tr(self.settings.language, "inst.sec_ram"));
                    ui.add_space(4.0);
                    badge(ui, &ram_badge_text, true);
                }
                ui.add_space(6.0);
                draw_ram_allocation_bar(
                    ui,
                    profile.min_ram_mb,
                    profile.max_ram_mb,
                    field_w,
                );
                ui.add_space(8.0);

                let stack_sliders = card_inner_w < 500.0;
                let slider_w = if stack_sliders {
                    (card_inner_w - 110.0).clamp(60.0, 340.0)
                } else {
                    (card_inner_w - 275.0).clamp(120.0, 340.0)
                };
                if stack_sliders {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.max_ram"))
                            .small()
                            .color(text_secondary()),
                    );
                    if ui_ram_slider(ui, &mut profile.max_ram_mb, 512..=16384, slider_w).changed()
                    {
                        if profile.min_ram_mb > profile.max_ram_mb {
                            profile.min_ram_mb = profile.max_ram_mb;
                        }
                        changed = true;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.min_ram"))
                            .small()
                            .color(text_secondary()),
                    );
                    let max_cap = profile.max_ram_mb.max(256);
                    if ui_ram_slider(ui, &mut profile.min_ram_mb, 256..=max_cap, slider_w).changed()
                    {
                        changed = true;
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(155.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(155.0);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(tr(
                                            self.settings.language,
                                            "inst.max_ram",
                                        ))
                                        .small()
                                        .color(text_secondary()),
                                    )
                                    .truncate(),
                                );
                            },
                        );
                        if ui_ram_slider(ui, &mut profile.max_ram_mb, 512..=16384, slider_w)
                            .changed()
                        {
                            if profile.min_ram_mb > profile.max_ram_mb {
                                profile.min_ram_mb = profile.max_ram_mb;
                            }
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(155.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(155.0);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(tr(
                                            self.settings.language,
                                            "inst.min_ram",
                                        ))
                                        .small()
                                        .color(text_secondary()),
                                    )
                                    .truncate(),
                                );
                            },
                        );
                        let max_cap = profile.max_ram_mb.max(256);
                        if ui_ram_slider(ui, &mut profile.min_ram_mb, 256..=max_cap, slider_w)
                            .changed()
                        {
                            changed = true;
                        }
                    });
                }

                ui.add_space(6.0);
                let ram_preset_lbl = format!("{} GB", profile.max_ram_mb / 1024);
                let prev_ram = profile.max_ram_mb;
                egui::ComboBox::from_id_salt("inst_ram_preset_combo")
                    .selected_text(ram_preset_lbl)
                    .width(field_w.min(140.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for preset in [2048_u32, 4096, 6144, 8192, 12288] {
                            ui.selectable_value(
                                &mut profile.max_ram_mb,
                                preset,
                                format!("{} GB ({preset} MB)", preset / 1024),
                            );
                        }
                    });
                if profile.max_ram_mb != prev_ram {
                    if profile.min_ram_mb > profile.max_ram_mb {
                        profile.min_ram_mb = profile.max_ram_mb;
                    }
                    anim::emit_ring(
                        ui.ctx(),
                        anim::pointer_or_center(ui.ctx()),
                        accent(),
                        28.0,
                    );
                    changed = true;
                }
                if profile.max_ram_mb >= 12288 {
                    ui.add_space(6.0);
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        "Allocating 12+ GB RAM can cause garbage collection pauses unless using ZGC or tuned G1GC flags.",
                        card_inner_w,
                    );
                }
            });

            ui.add_space(12.0);

            // Java & JVM Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                let render_java_action = |ui: &mut egui::Ui, rescan_flag: &mut bool| {
                    if java_scanning {
                        mono_spinner(ui, 14.0);
                    } else if ui_button_icon(
                        ui,
                        "refresh_java_btn",
                        Some(Icon::Refresh),
                        tr(self.settings.language, "inst.rescan_java"),
                        ButtonVariant::Secondary,
                        true,
                        24.0,
                    )
                    .clicked()
                    {
                        *rescan_flag = true;
                    }
                };
                if card_inner_w >= 340.0 {
                    ui_toolbar_row(ui, 26.0, |ui| {
                        section_header(ui, tr(self.settings.language, "inst.sec_java"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            render_java_action(ui, &mut rescan_java);
                        });
                    });
                } else {
                    section_header(ui, tr(self.settings.language, "inst.sec_java"));
                    ui.add_space(4.0);
                    render_java_action(ui, &mut rescan_java);
                }
                ui.add_space(6.0);

                if !java_scanning && java_installs.is_empty() && profile.java_path.trim().is_empty() {
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        "No Java runtime was detected automatically. Install OpenJDK or enter a custom java binary path below.",
                        card_inner_w,
                    );
                    ui.add_space(6.0);
                }

                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.detected_runtimes"))
                        .small()
                        .color(text_secondary()),
                );
                let cur_java_lbl = if profile.java_path.trim().is_empty() {
                    tr(self.settings.language, "inst.auto").to_string()
                } else {
                    java_installs
                        .iter()
                        .find(|j| j.path.to_string_lossy() == profile.java_path)
                        .map(|j| format!("Java {} ({})", j.major, j.path.display()))
                        .unwrap_or_else(|| profile.java_path.clone())
                };
                let prev_java = profile.java_path.clone();
                egui::ComboBox::from_id_salt("inst_java_combo")
                    .selected_text(cur_java_lbl)
                    .width(field_w.min(340.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut profile.java_path,
                            String::new(),
                            tr(self.settings.language, "inst.auto"),
                        );
                        for j in &java_installs {
                            let j_str = j.path.to_string_lossy().to_string();
                            ui.selectable_value(
                                &mut profile.java_path,
                                j_str,
                                format!("Java {}", j.major),
                            );
                        }
                    });
                if profile.java_path != prev_java {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                    changed = true;
                }
                ui.add_space(4.0);
                let java_placeholder = if cfg!(windows) {
                    r"Auto (or C:\path\to\javaw.exe)"
                } else {
                    "Auto (or /usr/bin/java)"
                };
                let java_in_w = if field_w >= 140.0 {
                    (field_w - 36.0).clamp(80.0, 440.0)
                } else {
                    field_w
                };
                ui_wrap_row(ui, 30.0, |ui| {
                    if ui_text_input(
                        ui,
                        "cfg_java_path",
                        &mut profile.java_path,
                        java_placeholder,
                        java_in_w,
                        false,
                    )
                    .changed()
                    {
                        changed = true;
                    }
                    if ui_icon_button(
                        ui,
                        "cfg_java_browse_btn",
                        Icon::Folder,
                        tr(self.settings.language, "btn.folder"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        trigger_pick_java = true;
                    }
                });

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.jvm_args"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "cfg_jvm_args",
                    &mut profile.jvm_args,
                    "-XX:+UseG1GC",
                    field_w,
                    false,
                )
                .changed()
                {
                    changed = true;
                }
                ui.add_space(4.0);
                let presets = [
                    ("G1GC", "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20"),
                    ("ZGC", "-XX:+UseZGC -XX:+ZGenerational"),
                    ("Aikar's", "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions"),
                ];
                let active_jvm_lbl = presets
                    .iter()
                    .find(|(_, f)| profile.jvm_args.trim() == *f)
                    .map(|(l, _)| *l)
                    .unwrap_or("JVM Preset");
                let prev_jvm = profile.jvm_args.clone();
                egui::ComboBox::from_id_salt("inst_jvm_preset_combo")
                    .selected_text(active_jvm_lbl)
                    .width(field_w.min(170.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for (lbl, flags) in presets {
                            ui.selectable_value(&mut profile.jvm_args, flags.to_string(), lbl);
                        }
                        ui.selectable_value(
                            &mut profile.jvm_args,
                            String::new(),
                            tr(self.settings.language, "btn.clear"),
                        );
                    });
                if profile.jvm_args != prev_jvm {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                    changed = true;
                }
            });

            ui.add_space(12.0);

            // Window, Quick Play & GPU Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                section_header(ui, tr(self.settings.language, "inst.sec_window"));
                ui.add_space(6.0);

                ui.label(egui::RichText::new(tr(self.settings.language, "inst.game_resolution")).small().color(text_secondary()));
                let cur_res = (profile.window_width, profile.window_height);
                let res_options = [
                    ("Default", None),
                    ("1280×720", Some((1280_u32, 720_u32))),
                    ("1600×900", Some((1600_u32, 900_u32))),
                    ("1920×1080", Some((1920_u32, 1080_u32))),
                    ("2560×1440", Some((2560_u32, 1440_u32))),
                ];
                let cur_res_lbl = res_options
                    .iter()
                    .find(|(_, d)| match d {
                        None => cur_res.0.is_none() || cur_res.1.is_none(),
                        Some((w, h)) => cur_res == (Some(*w), Some(*h)),
                    })
                    .map(|(l, _)| l.to_string())
                    .unwrap_or_else(|| {
                        format!(
                            "{}×{}",
                            cur_res.0.unwrap_or(854),
                            cur_res.1.unwrap_or(480)
                        )
                    });
                egui::ComboBox::from_id_salt("inst_res_preset_combo")
                    .selected_text(cur_res_lbl)
                    .width(field_w.min(170.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for (lbl, dims) in res_options {
                            let active = match dims {
                                None => cur_res.0.is_none() || cur_res.1.is_none(),
                                Some((w, h)) => cur_res == (Some(w), Some(h)),
                            };
                            if ui.selectable_label(active, lbl).clicked() {
                                match dims {
                                    None => {
                                        profile.window_width = None;
                                        profile.window_height = None;
                                    }
                                    Some((w, h)) => {
                                        profile.window_width = Some(w);
                                        profile.window_height = Some(h);
                                    }
                                }
                                anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                                changed = true;
                            }
                        }
                    });
                ui.add_space(6.0);
                if ui_checkbox(
                    ui,
                    "cfg_fullscreen_chk",
                    &mut profile.fullscreen,
                    tr(self.settings.language, "inst.fullscreen"),
                ) {
                    changed = true;
                }
                if ui_checkbox(
                    ui,
                    "cfg_discrete_gpu_chk",
                    &mut profile.prefer_discrete_gpu,
                    tr(self.settings.language, "inst.discrete_gpu"),
                ) {
                    changed = true;
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.quick_play"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "cfg_quickplay_server",
                    &mut profile.server_address,
                    "play.example.net (optional auto-join)",
                    field_w,
                    false,
                )
                .changed()
                {
                    changed = true;
                }
            });

            ui.add_space(10.0);

            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                let lang = self.settings.language;
                section_header(ui, tr(lang, "inst.hooks_title"));
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new(tr(lang, "inst.wrapper_cmd"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "cfg_wrapper_cmd",
                    &mut profile.wrapper_cmd,
                    "gamemoderun mangohud",
                    field_w,
                    true,
                )
                .changed()
                {
                    changed = true;
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "inst.pre_launch_cmd"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "cfg_pre_launch_cmd",
                    &mut profile.pre_launch_cmd,
                    "echo Starting %SUNSET_PROFILE_NAME%",
                    field_w,
                    true,
                )
                .changed()
                {
                    changed = true;
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "inst.post_exit_cmd"))
                        .small()
                        .color(text_secondary()),
                );
                if ui_text_input(
                    ui,
                    "cfg_post_exit_cmd",
                    &mut profile.post_exit_cmd,
                    "echo Exited",
                    field_w,
                    true,
                )
                .changed()
                {
                    changed = true;
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "inst.env_vars"))
                        .small()
                        .color(text_secondary()),
                );
                let env_resp = ui.add_sized(
                    [field_w, 64.0],
                    egui::TextEdit::multiline(&mut profile.env_vars)
                        .desired_width(field_w)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("MESA_GL_VERSION_OVERRIDE=4.6\nSDL_VIDEODRIVER=wayland"),
                );
                if env_resp.changed() {
                    changed = true;
                }
            });

        });

        if rescan_java {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
            self.scan_java();
        }
        if trigger_pick_icon {
            self.request_file_pick(FilePickPurpose::InstanceIcon);
        }
        if trigger_pick_java {
            self.request_file_pick(FilePickPurpose::JavaBinary);
        }
        if trigger_dup {
            anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 8);
            self.duplicate_profile(ui.ctx(), selected_idx);
        }
        if trigger_del {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 30.0);
            self.confirm_delete_idx = Some(selected_idx);
        }
        if rename_dir && !self.profiles[selected_idx].name.trim().is_empty() {
            let others: Vec<Profile> = self
                .profiles
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != selected_idx)
                .map(|(_, p)| p.clone())
                .collect();
            if rename_instance_dir_for_profile(
                &self.paths,
                &mut self.profiles[selected_idx],
                &others,
            ) {
                changed = true;
            }
        }
        if changed {
            let new_loader = self.profiles[selected_idx].loader.kind();
            if new_loader != loader_kind {
                let disabled =
                    disable_incompatible_mods_for_loader(&self.paths, &self.profiles[selected_idx]);
                if disabled > 0 {
                    self.show_toast(
                        ui.ctx(),
                        format!(
                            "Switched to {} — disabled {disabled} incompatible mod(s)",
                            new_loader.label()
                        ),
                    );
                }
            }
            let _ = save_profiles(&self.paths, &self.profiles);
            self.sync_discover_for_profile();
        }
    }
}
