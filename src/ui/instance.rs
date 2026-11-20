use super::*;

impl McLauncherApp {
    pub(super) fn render_instance_page(&mut self, ui: &mut egui::Ui) {
        if self.profiles.is_empty() {
            ui.label(tr(self.settings.language, "inst.none"));
            return;
        }

        let profile = self.profiles[self.selected_idx].clone();
        let avail_w = ui.available_width();
        let wide = avail_w >= 820.0;
        if wide {
            let left_w = (avail_w - 420.0).max(140.0);
            ui_toolbar_row(ui, 30.0, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(left_w, 30.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(left_w);
                        let (av_r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                        let is_running = self.running_profile_id.as_deref() == Some(profile.id.as_str())
                            && self.game_handle.is_some();
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
                        let title_w = (left_w - 220.0).max(60.0);
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
                        loader_badge_sized(ui, profile.loader.kind(), &profile.loader.label(), 30.0);
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
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
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                        self.export_current_profile_mrpack(ui.ctx(), &profile);
                    }
                });
            });
        } else {
            ui_wrap_row(ui, 28.0, |ui| {
                ui.heading(egui::RichText::new(&profile.name).size(21.0).color(text_primary()));
                badge(ui, &profile.version_id, false);
                loader_badge(ui, profile.loader.kind(), &profile.loader.label());
            });
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

        let (total_mods, enabled_mods) = self.profile_mod_counts(&profile);
        let worlds_cnt = self.cached_worlds(&profile).len();
        let shots_cnt = self.cached_screenshots(&profile).len();
        ui_flow_row(ui, 30.0, |ui| {
            for tab in InstanceSubTab::ALL {
                let is_active = self.instance_tab == tab;
                let tab_icon = match tab {
                    InstanceSubTab::Content => Icon::Layers,
                    InstanceSubTab::Worlds => Icon::Grid,
                    InstanceSubTab::Screenshots => Icon::External,
                    InstanceSubTab::Settings => Icon::Sliders,
                };
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
                let base = tab.translated_label(self.settings.language);
                let lbl = match count_tag {
                    Some(tag) => format!("{base} ({tag})"),
                    None => base.to_string(),
                };
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
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let subtab_t = self.subtab_switch_time;
        let active_tab = self.instance_tab;
        Self::apply_subtab_transition(ui, subtab_t, |ui| match active_tab {
            InstanceSubTab::Content => self.render_content_manager(ui, &profile),
            InstanceSubTab::Worlds => self.render_worlds_tab(ui, &profile),
            InstanceSubTab::Screenshots => self.render_screenshots_tab(ui, &profile),
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
                let can_play = !self.is_busy && self.game_handle.is_none();
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
                    ui_toolbar_row(ui, 32.0, |ui| {
                        ui.vertical(|ui| {
                            ui.strong(egui::RichText::new(&w.display_name).color(text_primary()));
                            ui.label(
                                egui::RichText::new(format!(
                                    "saves/{} • {}",
                                    w.folder_name,
                                    format_size(w.size_bytes)
                                ))
                                .small()
                                .color(text_muted()),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
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
                                        self.expanded_world_datapacks = Some(w.folder_name.clone());
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
                        });
                    });
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
                        ui_toolbar_row(ui, 28.0, |ui| {
                            section_header(
                                ui,
                                &format!(
                                    "{} — {} ({})",
                                    w.display_name,
                                    tr(lang, "world.datapacks"),
                                    dp_list.len()
                                ),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
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
                                },
                            );
                        });
                        ui.add_space(4.0);
                        for dp in &dp_list {
                            ui_toolbar_row(ui, 26.0, |ui| {
                                let sw_id = format!("world_dp_{}_{}", w.folder_name, dp.base_name);
                                if mod_toggle_switch(ui, &sw_id, dp.enabled).clicked() {
                                    dp_toggle = Some(dp.disk_name.clone());
                                }
                                ui.label(
                                    egui::RichText::new(&dp.base_name).color(if dp.enabled {
                                        text_primary()
                                    } else {
                                        text_muted()
                                    }),
                                );
                                ui.label(
                                    egui::RichText::new(format_size(dp.size_bytes))
                                        .small()
                                        .color(text_muted()),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
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
                                    },
                                );
                            });
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
                        ui_toolbar_row(ui, 30.0, |ui| {
                            ui.vertical(|ui| {
                                ui.strong(egui::RichText::new(&b.file_name).color(text_primary()));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "backups/{} • {}",
                                        b.file_name,
                                        format_size(b.size_bytes)
                                    ))
                                    .small()
                                    .color(text_muted()),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                            });
                        });
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
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let thumb_size = Vec2::new(96.0, 54.0);
                        let (t_rect, t_resp) = ui.allocate_exact_size(thumb_size, Sense::click());
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

                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            ui.strong(egui::RichText::new(&shot.filename).color(text_primary()));
                            ui.label(
                                egui::RichText::new(format_size(shot.size_bytes))
                                    .small()
                                    .color(text_muted()),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                        });
                    });
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
                ui.available_width(),
            );
            ui.add_space(6.0);
        }

        let mut go_discover = false;

        // Folder switcher
        ui_flow_row(ui, 30.0, |ui| {
            for f in ContentFolder::ALL {
                let count = self.folder_content_count(profile, f.dir_name());
                let lbl = format!("{} ({count})", f.translated_label(self.settings.language));
                let ic = match f {
                    ContentFolder::Mods => Icon::Layers,
                    ContentFolder::Resourcepacks => Icon::Grid,
                    ContentFolder::Shaderpacks => Icon::Sparkle,
                };
                if ui_pill_tab_icon(
                    ui,
                    ("folder_tab", f.dir_name()),
                    Some(ic),
                    &lbl,
                    self.content_folder == f,
                )
                .clicked()
                    && self.content_folder != f
                {
                    self.content_folder = f;
                    self.mark_subtab_switch(ui.ctx());
                }
            }
            if ui_button_icon(
                ui,
                "content_import_local_btn",
                Some(Icon::Plus),
                tr(self.settings.language, "btn.add_content"),
                ButtonVariant::Secondary,
                !self.mr_installing && !self.file_picker_open,
                28.0,
            )
            .clicked()
            {
                self.request_file_pick(FilePickPurpose::InstanceContentImport);
            }
            if (total_count > 0 || !self.mr_installed_cache.is_empty())
                && ui_button_icon(
                    ui,
                    "content_check_updates_btn",
                    Some(Icon::Refresh),
                    if self.mr_installing {
                        tr(self.settings.language, "content.checking")
                    } else {
                        tr(self.settings.language, "btn.check_updates")
                    },
                    ButtonVariant::Secondary,
                    !self.mr_installing,
                    28.0,
                )
                .clicked()
            {
                reset_download_cancel();
                anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                self.mr_installing = true;
                self.progress = 0.08;
                self.status_text = tr(self.settings.language, "content.checking_updates").to_string();
                let paths = self.paths.clone();
                let prof = profile.clone();
                let tx = self.tx.clone();
                thread::spawn(move || {
                    let res = update_all_installed_projects(&paths, &prof, &tx);
                    let _ = tx.send(WorkerEvent::ModUpdatesFinished(res));
                });
            }
        });

        ui.add_space(6.0);
        let avail_w = ui.available_width();
        ui_wrap_row(ui, 30.0, |ui| {
            ui_search_input(
                ui,
                "content_search_input",
                &mut self.content_search,
                tr(self.settings.language, "content.filter"),
                (avail_w * 0.36).clamp(115.0, 230.0),
            );
            if ui_pill_tab(
                ui,
                "state_all",
                &format!("{} ({total_count})", tr(self.settings.language, "lib.all")),
                self.content_state_filter == ContentStateFilter::All,
            )
            .clicked()
                && self.content_state_filter != ContentStateFilter::All
            {
                self.content_state_filter = ContentStateFilter::All;
                self.mark_subtab_switch(ui.ctx());
            }
            if ui_pill_tab(
                ui,
                "state_on",
                &format!("{} ({enabled_count})", tr(self.settings.language, "content.enabled")),
                self.content_state_filter == ContentStateFilter::Enabled,
            )
            .clicked()
                && self.content_state_filter != ContentStateFilter::Enabled
            {
                self.content_state_filter = ContentStateFilter::Enabled;
                self.mark_subtab_switch(ui.ctx());
            }
            if ui_pill_tab(
                ui,
                "state_off",
                &format!("{} ({disabled_count})", tr(self.settings.language, "content.disabled")),
                self.content_state_filter == ContentStateFilter::Disabled,
            )
            .clicked()
                && self.content_state_filter != ContentStateFilter::Disabled
            {
                self.content_state_filter = ContentStateFilter::Disabled;
                self.mark_subtab_switch(ui.ctx());
            }
            if disabled_count > 0
                && ui_button(
                    ui,
                    "bulk_enable_btn",
                    tr(self.settings.language, "btn.enable_all"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
            {
                match set_all_content_enabled(&target_dir, true) {
                    Ok(n) => {
                        self.fs_cache.invalidate();
                        let pos = anim::pointer_or_center(ui.ctx());
                        anim::emit_burst(ui.ctx(), pos, accent(), 8);
                        anim::emit_floating_badge(ui.ctx(), pos, Some(Icon::Check), format!("+{n}"), accent());
                        self.show_toast(
                            ui.ctx(),
                            format!("{}: {n}", tr(self.settings.language, "toast.enabled_count")),
                        );
                    }
                    Err(e) => self.show_toast(ui.ctx(), e),
                }
            }
            if enabled_count > 0
                && ui_button(
                    ui,
                    "bulk_disable_btn",
                    tr(self.settings.language, "btn.disable_all"),
                    ButtonVariant::Subtle,
                    true,
                    30.0,
                )
                .clicked()
            {
                match set_all_content_enabled(&target_dir, false) {
                    Ok(n) => {
                        self.fs_cache.invalidate();
                        let pos = anim::pointer_or_center(ui.ctx());
                        anim::emit_ring(ui.ctx(), pos, text_secondary(), 30.0);
                        anim::emit_floating_badge(ui.ctx(), pos, None, format!("-{n}"), text_secondary());
                        self.show_toast(
                            ui.ctx(),
                            format!("{}: {n}", tr(self.settings.language, "toast.disabled_count")),
                        );
                    }
                    Err(e) => self.show_toast(ui.ctx(), e),
                }
            }
        });

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
                let card_outer_w = (ui.available_width() - 10.0).max(150.0);
                let card_inner_w = (card_outer_w - 24.0).max(110.0);
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui_wrap_row(ui, 30.0, |ui| {
                        let msg = if total_count == 0 {
                            format!("{folder_name}/ folder is currently empty.")
                        } else {
                            tr(self.settings.language, "content.no_matches").to_string()
                        };
                        ui.label(egui::RichText::new(msg).color(text_secondary()));
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
                    });
                });
            });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("instance_content_scroll")
                .auto_shrink([false, false])
                .max_height(avail_h)
                .show_rows(ui, 60.0, filtered.len(), |ui, row_range| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let card_outer_w = (ui.available_width() - 10.0).max(150.0);
                    let card_inner_w = (card_outer_w - 24.0).max(110.0);
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
                            let compact_row = card_inner_w < 380.0;
                            let right_pill: Option<(&str, bool)> = if !item.enabled {
                                Some((tr(self.settings.language, "content.disabled"), false))
                            } else {
                                info.map(|inf| (inf.version_number.as_str(), true))
                            };
                            let show_right_pill = !compact_row && right_pill.is_some();
                            let del_reserve = if compact_row {
                                36.0
                            } else if show_right_pill {
                                214.0
                            } else {
                                96.0
                            };
                            let switch_w = 38.0;
                            let icon_px = 30.0;
                            let mid_w =
                                (card_inner_w - switch_w - icon_px - del_reserve - 26.0).max(60.0);

                            ui_toolbar_row(ui, 36.0, |ui| {
                                if mod_toggle_switch(
                                    ui,
                                    &format!("{}_{}_{}", profile.id, folder_name, item.base_name),
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
                                let tex_opt = icon_url_opt
                                    .and_then(|u| self.mr_icons.get(u))
                                    .and_then(|o| o.as_ref());
                                let is_icon_loading = icon_url_opt
                                    .is_some_and(|u| self.mr_icons_pending.contains(u.as_str()));
                                let icon_size = Vec2::splat(icon_px);
                                if let Some(tex) = tex_opt {
                                    ui.add(
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(icon_size)
                                            .rounding(Rounding::same(6.5)),
                                    );
                                } else {
                                    let (rect, _) = ui.allocate_exact_size(icon_size, Sense::hover());
                                    draw_icon_placeholder(
                                        ui,
                                        rect,
                                        6.5,
                                        &display_title,
                                        is_icon_loading,
                                    );
                                }

                                ui.allocate_ui_with_layout(
                                    Vec2::new(mid_w, 34.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.set_min_width(mid_w);
                                        ui.set_max_width(mid_w);
                                        let mut title_rt =
                                            egui::RichText::new(&display_title).strong();
                                        if !item.enabled {
                                            title_rt = title_rt.strikethrough().color(text_muted());
                                        } else {
                                            title_rt = title_rt.color(text_primary());
                                        }
                                        ui.add(egui::Label::new(title_rt).truncate());
                                        let src_tag = if info.is_some() {
                                            "Modrinth"
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

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
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
                                        if show_right_pill {
                                            if let Some((pill_txt, highlighted)) = right_pill {
                                                badge_sized(
                                                    ui,
                                                    pill_txt,
                                                    highlighted,
                                                    28.0,
                                                    Some(112.0),
                                                )
                                                .on_hover_text(pill_txt);
                                            }
                                        }
                                    },
                                );
                            });
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
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            let field_w = (card_inner_w - 20.0).min(360.0);
            let wide_grid = card_inner_w >= 480.0;

            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 24.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_loader"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if can_delete
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
                        {
                            trigger_del = true;
                        }
                        if ui_button_icon(
                            ui,
                            "cfg_dup_inst",
                            Some(Icon::Duplicate),
                            tr(self.settings.language, "btn.duplicate"),
                            ButtonVariant::Subtle,
                            true,
                            24.0,
                        )
                        .clicked()
                        {
                            trigger_dup = true;
                        }
                    });
                });
                ui.add_space(6.0);

                let mut request_pick = false;
                let mut reset_icon = false;
                if wide_grid {
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
                                (field_w - 90.0).max(160.0),
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
                            ui_wrap_row(ui, 30.0, |ui| {
                                egui::ComboBox::from_id_salt("version_combo")
                                    .selected_text(&profile.version_id)
                                    .width(160.0)
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
                                    210.0,
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
                            .width(140.0)
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
                            (field_w - 28.0).max(120.0),
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
                ui_toolbar_row(ui, 24.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_ram"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge(
                            ui,
                            &format!("-Xms{}M  -Xmx{}M", profile.min_ram_mb, profile.max_ram_mb),
                            true,
                        );
                    });
                });
                ui.add_space(6.0);
                draw_ram_allocation_bar(
                    ui,
                    profile.min_ram_mb,
                    profile.max_ram_mb,
                    (card_inner_w - 16.0).clamp(120.0, 520.0),
                );
                ui.add_space(8.0);

                let slider_w = (field_w - 90.0).max(100.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(self.settings.language, "inst.max_ram")).small().color(text_secondary()));
                    if ui_ram_slider(ui, &mut profile.max_ram_mb, 512..=16384, slider_w).changed()
                    {
                        if profile.min_ram_mb > profile.max_ram_mb {
                            profile.min_ram_mb = profile.max_ram_mb;
                        }
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(self.settings.language, "inst.min_ram")).small().color(text_secondary()));
                    let max_cap = profile.max_ram_mb.max(256);
                    if ui_ram_slider(ui, &mut profile.min_ram_mb, 256..=max_cap, slider_w).changed()
                    {
                        changed = true;
                    }
                });

                ui.add_space(4.0);
                ui_flow_row(ui, 28.0, |ui| {
                    for preset in [2048_u32, 4096, 6144, 8192, 12288] {
                        if ui_pill_tab(
                            ui,
                            ("ram_preset", preset),
                            &format!("{} GB", preset / 1024),
                            profile.max_ram_mb == preset,
                        )
                        .clicked()
                        {
                            profile.max_ram_mb = preset;
                            if profile.min_ram_mb > preset {
                                profile.min_ram_mb = preset;
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
                });
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
                ui_toolbar_row(ui, 26.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_java"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                            rescan_java = true;
                        }
                    });
                });
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
                ui_flow_row(ui, 28.0, |ui| {
                    let is_auto = profile.java_path.trim().is_empty();
                    if ui_pill_tab(ui, "java_auto_pill", tr(self.settings.language, "inst.auto"), is_auto).clicked() {
                        profile.java_path.clear();
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                        changed = true;
                    }
                    for j in &java_installs {
                        let j_str = j.path.to_string_lossy().to_string();
                        let active = profile.java_path == j_str;
                        if ui_pill_tab(
                            ui,
                            ("java_pill", j_str.as_str()),
                            &format!("Java {}", j.major),
                            active,
                        )
                        .on_hover_text(&j_str)
                        .clicked()
                        {
                            profile.java_path = j_str;
                            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                            changed = true;
                        }
                    }
                });
                ui.add_space(4.0);
                let java_placeholder = if cfg!(windows) {
                    r"Auto (or C:\path\to\javaw.exe)"
                } else {
                    "Auto (or /usr/bin/java)"
                };
                ui_wrap_row(ui, 30.0, |ui| {
                    if ui_text_input(
                        ui,
                        "cfg_java_path",
                        &mut profile.java_path,
                        java_placeholder,
                        (field_w - 44.0).max(140.0),
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
                ui_flow_row(ui, 26.0, |ui| {
                    let presets = [
                        ("G1GC", "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20"),
                        ("ZGC", "-XX:+UseZGC -XX:+ZGenerational"),
                        ("Aikar's", "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions"),
                    ];
                    for (lbl, flags) in presets {
                        if ui_button(
                            ui,
                            ("jvm_preset", lbl),
                            lbl,
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .on_hover_text(flags)
                        .clicked()
                        {
                            profile.jvm_args = flags.to_string();
                            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 26.0);
                            changed = true;
                        }
                    }
                    if !profile.jvm_args.is_empty()
                        && ui_button(
                            ui,
                            "jvm_preset_clear",
                            tr(self.settings.language, "btn.clear"),
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .clicked()
                    {
                        profile.jvm_args.clear();
                        anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), text_secondary(), 24.0);
                        changed = true;
                    }
                });
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
                ui_flow_row(ui, 28.0, |ui| {
                    for (lbl, dims) in [
                        ("Default", None),
                        ("1280×720", Some((1280_u32, 720_u32))),
                        ("1600×900", Some((1600_u32, 900_u32))),
                        ("1920×1080", Some((1920_u32, 1080_u32))),
                        ("2560×1440", Some((2560_u32, 1440_u32))),
                    ] {
                        let active = match dims {
                            None => cur_res.0.is_none() || cur_res.1.is_none(),
                            Some((w, h)) => cur_res == (Some(w), Some(h)),
                        };
                        if ui_pill_tab(ui, ("res_preset", lbl), lbl, active).clicked() {
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
