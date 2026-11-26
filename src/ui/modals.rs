use super::*;

impl McLauncherApp {
    pub(super) fn render_new_profile_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("new_profile_modal_anim"),
            self.show_new_modal,
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_new_modal && modal_t <= 0.01 {
            return;
        }

        let screen = ctx.screen_rect();
        if self.show_new_modal && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_new_modal = false;
            return;
        }
        egui::Area::new(egui::Id::new("modal_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 170.0) as u8),
                );
                if resp.clicked() {
                    self.show_new_modal = false;
                }
            });

        if !self.show_new_modal {
            return;
        }

        let mut open = self.show_new_modal;
        let max_modal_w = (screen.width() - 32.0).clamp(260.0, 480.0);
        let max_modal_h = (screen.height() - 32.0).max(240.0);
        let modal_offset_y = (1.0 - ease_out_cubic(modal_t)) * 18.0;
        let win_resp = egui::Window::new(tr(self.settings.language, "modal.create_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_modal_w)
            .max_height(max_modal_h)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, modal_offset_y))
            .show(ctx, |ui| {
                let inner_w = (max_modal_w - 28.0).max(200.0);
                let field_w = (inner_w - 96.0).clamp(120.0, 300.0);
                let mut enter_create = false;
                egui::ScrollArea::vertical()
                    .id_salt("new_profile_modal_scroll")
                    .auto_shrink([false, true])
                    .max_height((max_modal_h - 90.0).max(120.0))
                    .show(ui, |ui| {
                        egui::Grid::new("new_profile_grid")
                            .num_columns(2)
                            .spacing([12.0, 10.0])
                            .show(ui, |ui| {
                                ui.label(tr(self.settings.language, "modal.name"));
                                let r = ui_text_input(
                                    ui,
                                    "modal_inst_name",
                                    &mut self.new_name,
                                    "My Modded World",
                                    field_w,
                                    false,
                                );
                                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    enter_create = true;
                                }
                                ui.end_row();

                                ui.label(tr(self.settings.language, "modal.version"));
                                ui_wrap_row(ui, 30.0, |ui| {
                                    egui::ComboBox::from_id_salt("new_ver_combo")
                                        .selected_text(&self.new_version)
                                        .width((field_w - 108.0).clamp(100.0, 180.0))
                                        .show_ui(ui, |ui| {
                                            if let Some(m) = &self.manifest {
                                                for v in m
                                                    .versions
                                                    .iter()
                                                    .filter(|v| {
                                                        self.new_show_snapshots
                                                            || v.version_type
                                                                == VersionType::Release
                                                    })
                                                    .take(80)
                                                {
                                                    ui.selectable_value(
                                                        &mut self.new_version,
                                                        v.id.clone(),
                                                        &v.id,
                                                    );
                                                }
                                            }
                                        });
                                    ui_checkbox(
                                        ui,
                                        "modal_snapshots_chk",
                                        &mut self.new_show_snapshots,
                                        tr(self.settings.language, "inst.snapshots"),
                                    );
                                });
                                ui.end_row();

                                ui.label(tr(self.settings.language, "modal.loader"));
                                egui::ComboBox::from_id_salt("modal_loader_combo")
                                    .selected_text(self.new_loader.label())
                                    .width(field_w.clamp(140.0, 220.0))
                                    .show_ui(ui, |ui| {
                                        for kind in LoaderKind::ALL {
                                            ui.selectable_value(
                                                &mut self.new_loader,
                                                kind,
                                                kind.label(),
                                            );
                                        }
                                    });
                                ui.end_row();

                                ui.label(tr(self.settings.language, "modal.memory"));
                                ui.vertical(|ui| {
                                    ui.set_max_width(field_w.max(180.0));
                                    ui.spacing_mut().item_spacing.y = 6.0;
                                    egui::ComboBox::from_id_salt("modal_ram_preset_combo")
                                        .selected_text(format!("{} GB", self.new_ram_mb / 1024))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for preset in [2048_u32, 4096, 6144, 8192, 12288] {
                                                ui.selectable_value(
                                                    &mut self.new_ram_mb,
                                                    preset,
                                                    format!("{} GB ({preset} MB)", preset / 1024),
                                                );
                                            }
                                        });
                                    ui_ram_slider(
                                        ui,
                                        &mut self.new_ram_mb,
                                        512..=16384,
                                        (field_w - 86.0).clamp(100.0, 220.0),
                                    );
                                });
                                ui.end_row();
                            });
                    });

                ui.add_space(12.0);
                let trimmed_inst_name = self.new_name.trim();
                let inst_name_taken = !trimmed_inst_name.is_empty()
                    && self
                        .profiles
                        .iter()
                        .any(|p| p.name.trim().to_lowercase() == trimmed_inst_name.to_lowercase());
                if inst_name_taken {
                    ui_alert_banner(
                        ui,
                        AlertKind::Error,
                        tr(self.settings.language, "err.duplicate_instance_name"),
                        (ui.available_width() - 8.0).max(160.0),
                    );
                    ui.add_space(6.0);
                }
                let name_valid = !trimmed_inst_name.is_empty() && !inst_name_taken;
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if (ui_button_icon(
                        ui,
                        "modal_create_btn",
                        Some(Icon::Plus),
                        tr(self.settings.language, "modal.create_instance"),
                        ButtonVariant::Primary,
                        name_valid,
                        30.0,
                    )
                    .clicked()
                        || enter_create)
                        && name_valid
                    {
                        let loader = ModLoader::from_kind(self.new_loader, String::new());
                        let mut profile = Profile::new_with_defaults(
                            self.new_name.trim().to_string(),
                            self.new_version.clone(),
                            loader,
                            self.new_username.trim().to_string(),
                            &self.settings,
                        );
                        profile.max_ram_mb = self.new_ram_mb;
                        profile.min_ram_mb = profile.min_ram_mb.min(profile.max_ram_mb);
                        profile.dir_name =
                            unique_instance_dir_name(&self.paths, &profile.name, &self.profiles);
                        if profile.loader.is_modded() {
                            let _ = ensure_instance_mods_dir(&self.paths, &profile);
                        }
                        self.profiles.push(profile);
                        let new_idx = self.profiles.len() - 1;
                        let _ = save_profiles(&self.paths, &self.profiles);
                        self.select_profile(new_idx);
                        self.show_new_modal = false;
                        self.show_toast(ctx, tr(self.settings.language, "inst.created"));
                        anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                        anim::emit_edge_pulse(ctx, accent());
                    }
                    if ui_button_icon(
                        ui,
                        "modal_import_mrpack_btn",
                        Some(Icon::Download),
                        tr(self.settings.language, "btn.import_modpack"),
                        ButtonVariant::Secondary,
                        !self.mr_installing && !self.file_picker_open,
                        30.0,
                    )
                    .clicked()
                    {
                        self.request_file_pick(FilePickPurpose::ModpackImport);
                    }
                    if ui_button(
                        ui,
                        "modal_cancel_btn",
                        tr(self.settings.language, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.show_new_modal = false;
                    }
                });
            });
        if let Some(ir) = win_resp {
            ctx.move_to_top(ir.response.layer_id);
            if modal_t < 0.99 {
                let win_r = ir.response.rect;
                let glow_a = ((1.0 - modal_t) * 165.0) as u8;
                ctx.layer_painter(ir.response.layer_id).rect_stroke(
                    win_r,
                    Rounding::same(12.0),
                    Stroke::new(1.5_f32, with_alpha(accent(), glow_a)),
                );
                paint_fade_veil(
                    &ctx.layer_painter(ir.response.layer_id),
                    win_r,
                    bg_void(),
                    modal_t,
                    110.0,
                );
            }
        }
        if !open {
            self.show_new_modal = false;
        }
    }

    pub(super) fn render_delete_confirm_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("del_modal_anim"),
            self.confirm_delete_idx.is_some(),
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if self.confirm_delete_idx.is_none() && modal_t <= 0.01 {
            return;
        }
        if self.confirm_delete_idx.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_delete_idx = None;
            return;
        }

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("del_modal_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 165.0) as u8),
                );
                if resp.clicked() {
                    self.confirm_delete_idx = None;
                }
            });

        let Some(del_idx) = self.confirm_delete_idx else {
            return;
        };
        let Some(target) = self.profiles.get(del_idx).cloned() else {
            self.confirm_delete_idx = None;
            return;
        };

        let mut open = true;
        let max_w = (screen.width() - 32.0).clamp(240.0, 400.0);
        let offset_y = (1.0 - ease_out_cubic(modal_t)) * 16.0;
        let win_resp = egui::Window::new(tr(self.settings.language, "modal.delete_title"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_w)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, offset_y))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        tr(self.settings.language, "modal.delete_confirm"),
                        target.name
                    ))
                    .strong()
                    .color(text_primary()),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {}",
                        target.version_id,
                        target.loader.label()
                    ))
                    .small()
                    .color(text_secondary()),
                );
                let target_active =
                    self.is_profile_running(&target.id) || self.is_profile_launching(&target.id);
                if target_active {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(tr(
                            self.settings.language,
                            "inst.delete_running_blocked",
                        ))
                        .small()
                        .color(WARNING),
                    );
                }
                ui.add_space(12.0);
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui_button_icon(
                        ui,
                        "confirm_del_yes",
                        Some(Icon::Trash),
                        tr(self.settings.language, "btn.delete"),
                        ButtonVariant::Danger,
                        !target_active,
                        30.0,
                    )
                    .clicked()
                    {
                        if del_idx < self.profiles.len() && self.profiles.len() > 1 {
                            let removed = self.profiles.remove(del_idx);
                            let _ = remove_instance_dir(&self.paths, &removed);
                            self.instance_logs.remove(&removed.id);
                            self.fs_cache.invalidate();
                            if del_idx < self.selected_idx {
                                self.selected_idx -= 1;
                            } else if self.selected_idx >= self.profiles.len() {
                                self.selected_idx = self.profiles.len().saturating_sub(1);
                            }
                            let _ = save_profiles(&self.paths, &self.profiles);
                            self.persist_selected_profile();
                            self.sync_installed_index();
                            self.sync_discover_for_profile();
                            let msg = format!(
                                "{}: '{}'",
                                tr(self.settings.language, "toast.removed"),
                                removed.name
                            );
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), DANGER, 8);
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                        self.confirm_delete_idx = None;
                    }
                    if ui_button(
                        ui,
                        "confirm_del_cancel",
                        tr(self.settings.language, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.confirm_delete_idx = None;
                    }
                });
            });
        if let Some(ir) = win_resp {
            if modal_t < 0.99 {
                let win_r = ir.response.rect;
                ctx.layer_painter(ir.response.layer_id).rect_stroke(
                    win_r,
                    Rounding::same(12.0),
                    Stroke::new(1.5_f32, with_alpha(DANGER, ((1.0 - modal_t) * 185.0) as u8)),
                );
                paint_fade_veil(
                    &ctx.layer_painter(ir.response.layer_id),
                    win_r,
                    bg_void(),
                    modal_t,
                    110.0,
                );
            }
        }
        if !open {
            self.confirm_delete_idx = None;
        }
    }

    pub(super) fn render_toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let active = match &self.toast {
            Some(t) if now - t.created_at < 3.8 => Some((t.text.clone(), now - t.created_at)),
            _ => {
                self.toast = None;
                None
            }
        };

        if let Some((text, age)) = active {
            let enter = (age / 0.22).clamp(0.0, 1.0) as f32;
            let exit = ((3.8 - age) / 0.30).clamp(0.0, 1.0) as f32;
            let enter_eased = ease_out_cubic(enter);
            let exit_eased = ease_out_cubic(exit);
            let alpha = enter_eased.min(exit_eased);
            let slide_y = (1.0 - enter_eased) * 18.0 + (1.0 - exit_eased) * 10.0;
            let rem_frac = (1.0 - (age / 3.8)).clamp(0.0, 1.0) as f32;
            let is_err =
                text.to_lowercase().contains("error") || text.to_lowercase().contains("failed");
            let accent_col = if is_err { DANGER } else { accent() };
            let bg_col = if is_err {
                Color32::from_rgb(46, 24, 28)
            } else {
                bg_card_active()
            };
            let max_w = (ctx.screen_rect().width() - 36.0).clamp(160.0, 420.0);
            ctx.request_repaint();

            egui::Area::new(egui::Id::new("toast_overlay"))
                .order(egui::Order::Tooltip)
                .anchor(
                    egui::Align2::RIGHT_BOTTOM,
                    Vec2::new(-16.0, -44.0 + slide_y),
                )
                .show(ctx, |ui| {
                    ui.set_max_width(max_w);
                    let a = (alpha * 255.0) as u8;
                    let f_resp = egui::Frame::none()
                        .fill(with_alpha(bg_col, a))
                        .stroke(Stroke::new(1.2_f32, with_alpha(accent_col, a)))
                        .rounding(Rounding::same(9.5))
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let (ic_r, _) =
                                    ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                                let ic_scale = 0.72 + 0.28 * ease_out_back(enter).min(1.14);
                                draw_icon_animated(
                                    ui.painter(),
                                    ic_r,
                                    if is_err { Icon::Alert } else { Icon::Check },
                                    with_alpha(accent_col, a),
                                    self::anim::IconAnim {
                                        offset: Vec2::ZERO,
                                        scale: ic_scale,
                                        rotation: 0.0,
                                    },
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(text)
                                            .color(with_alpha(text_primary(), a)),
                                    )
                                    .wrap(),
                                );
                            });
                        });
                    let rect = f_resp.response.rect;
                    if enter < 0.99 {
                        let expand = ease_out_cubic(enter) * 5.0;
                        let glow_a = ((1.0 - enter) * 140.0) as u8;
                        ui.painter().rect_stroke(
                            rect.expand(expand),
                            Rounding::same(11.0),
                            Stroke::new(1.3_f32, with_alpha(accent_col, glow_a)),
                        );
                    }
                    let bar_w = ((rect.width() - 12.0) * rem_frac).max(0.0);
                    if bar_w > 1.0 {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(rect.left() + 6.0, rect.bottom() - 2.5),
                                Vec2::new(bar_w, 2.0),
                            ),
                            Rounding::same(1.0),
                            with_alpha(accent_col, (alpha * 190.0) as u8),
                        );
                    }
                });
        }
    }

    pub(super) fn open_project_details(&mut self, hit: &ModrinthSearchHit) {
        self.mr_detail_hit = Some(hit.clone());
        self.mr_detail_loading = true;
        self.mr_detail_versions = None;

        let target_srv = self
            .discover_target_server_id
            .as_ref()
            .and_then(|id| self.servers.iter().find(|s| &s.id == id))
            .or_else(|| {
                if self.mr_type == ModrinthProjectType::Plugin {
                    self.servers.get(self.selected_server_idx)
                } else {
                    None
                }
            });
        let (game_version, loader) = if let Some(srv) = target_srv {
            let l = if self.mr_type == ModrinthProjectType::Mod {
                ModLoader::from_kind(
                    srv.software
                        .modrinth_mod_loader()
                        .unwrap_or(LoaderKind::Vanilla),
                    String::new(),
                )
            } else {
                ModLoader::Vanilla
            };
            (srv.mc_version.clone(), l)
        } else {
            self.profiles
                .get(self.selected_idx)
                .map(|p| (p.version_id.clone(), p.loader.clone()))
                .unwrap_or_else(|| ("1.21.4".to_string(), ModLoader::Vanilla))
        };
        let project_id = hit.project_id.clone();
        let mr_type = self.mr_type;
        let cf_key = self.settings.curseforge_api_key.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = if let Some(cf_mod_id) = parse_cf_project_id(&project_id) {
                fetch_curseforge_versions_for_profile(
                    &cf_key,
                    cf_mod_id,
                    mr_type,
                    &game_version,
                    &loader,
                )
            } else {
                fetch_project_versions_for_profile(&project_id, mr_type, &game_version, &loader)
            };
            let _ = tx.send(WorkerEvent::ProjectVersionsLoaded { project_id, result });
        });
    }

    pub(super) fn handle_file_drop(&mut self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let drop_t =
            ctx.animate_bool_with_time(egui::Id::new("file_drop_overlay_anim"), hovering, 0.14);
        if drop_t > 0.01 && drop_t < 0.99 {
            ctx.request_repaint();
        }
        if hovering || drop_t > 0.02 {
            let screen = ctx.screen_rect();
            let target_hint = self
                .profiles
                .get(self.selected_idx)
                .map(|p| format!("Into \"{}\" ({})", p.name, self.content_folder.label()))
                .unwrap_or_else(|| "No active instance selected".to_string());
            let eased = ease_out_cubic(drop_t);
            let a = (eased * 255.0) as u8;
            ctx.request_repaint();
            egui::Area::new(egui::Id::new("file_drop_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(screen.min)
                .show(ctx, |ui| {
                    let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        Rounding::ZERO,
                        Color32::from_rgba_unmultiplied(8, 12, 18, (eased * 200.0) as u8),
                    );
                    let box_w = (screen.width() - 48.0).clamp(260.0, 440.0);
                    let scale = 0.92 + 0.08 * eased;
                    let box_rect = Rect::from_center_size(
                        rect.center() + Vec2::new(0.0, (1.0 - eased) * 14.0),
                        Vec2::new(box_w * scale, 128.0 * scale),
                    );
                    let pulse = pulse_01(ctx.input(|i| i.time), 4.0);
                    ui.painter().rect_stroke(
                        box_rect.expand(pulse * 3.5),
                        Rounding::same(14.0),
                        Stroke::new(
                            1.4_f32,
                            with_alpha(accent(), ((1.0 - pulse) * eased * 130.0) as u8),
                        ),
                    );
                    ui.painter().rect(
                        box_rect,
                        Rounding::same(12.0),
                        with_alpha(bg_card(), a),
                        Stroke::new(1.5_f32, with_alpha(accent(), a)),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(box_rect.shrink(16.0))
                            .layout(egui::Layout::top_down(egui::Align::Center)),
                    );
                    let (ic_r, _) = child.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                    let bob = (ctx.input(|i| i.time) as f32 * 5.0).sin() * 2.0;
                    draw_icon_animated(
                        child.painter(),
                        ic_r,
                        Icon::Download,
                        with_alpha(accent(), a),
                        self::anim::IconAnim {
                            offset: Vec2::new(0.0, bob),
                            scale: 1.0 + pulse * 0.05,
                            rotation: 0.0,
                        },
                    );
                    child.add_space(6.0);
                    child.label(
                        egui::RichText::new(tr(self.settings.language, "content.drop_hint"))
                            .strong()
                            .size(15.0)
                            .color(with_alpha(text_primary(), a)),
                    );
                    child.label(
                        egui::RichText::new(target_hint)
                            .small()
                            .color(with_alpha(text_secondary(), a)),
                    );
                });
        }

        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        anim::emit_burst(ctx, ctx.screen_rect().center(), accent(), 12);
        anim::emit_edge_pulse(ctx, accent());
        let Some(profile) = self.profiles.get(self.selected_idx).cloned() else {
            self.show_toast(ctx, "Create or select an instance before importing files");
            return;
        };
        let (images, other_files): (Vec<_>, Vec<_>) =
            dropped.into_iter().partition(|p| is_image_file(p));
        if let Some(img_path) = images.into_iter().next() {
            if let Some(target) = self.profiles.get_mut(self.selected_idx) {
                match apply_profile_icon_from_file(
                    ctx,
                    &self.paths,
                    &mut self.profile_icons,
                    target,
                    &img_path,
                ) {
                    Ok(()) => {
                        let _ = save_profiles(&self.paths, &self.profiles);
                        let msg = tr(self.settings.language, "inst.icon_updated").to_string();
                        self.show_toast(ctx, msg);
                    }
                    Err(err) => self.show_toast(ctx, &err),
                }
            }
        }
        if other_files.is_empty() {
            return;
        }
        let active_folder = self.content_folder.dir_name().to_string();
        let paths = self.paths.clone();
        let cf_key = self.settings.curseforge_api_key.clone();
        let tx = self.tx.clone();
        let cancel_tok = Arc::new(AtomicBool::new(false));
        self.mr_cancel_token = Some(Arc::clone(&cancel_tok));
        self.mr_installing = true;
        self.progress = 0.08;
        let total = other_files.len();
        self.status_text = format!("Importing {total} file(s)...");
        thread::spawn(move || {
            let mut imported = 0usize;
            let mut errors = Vec::new();
            with_download_cancel_token(cancel_tok, || {
                for (idx, path) in other_files.into_iter().enumerate() {
                    if crate::mojang::is_download_cancelled() {
                        errors.push(CANCELLED_ERR_MSG.to_string());
                        break;
                    }
                    let fname = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());
                    let _ = tx.send(WorkerEvent::Progress {
                        stage: format!("Importing ({}/{total}): {fname}...", idx + 1),
                        fraction: (idx as f32 + 0.1) / total.max(1) as f32,
                    });
                    match import_local_file(&paths, &profile, &path, &active_folder, &cf_key, &tx) {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(format!("{fname}: {e}")),
                    }
                }
            });
            let _ = tx.send(WorkerEvent::BatchImportFinished { imported, errors });
        });
    }

    pub(super) fn render_project_details_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("mr_detail_modal_anim"),
            self.mr_detail_hit.is_some(),
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if self.mr_detail_hit.is_none() && modal_t <= 0.01 {
            return;
        }
        if self.mr_detail_hit.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.mr_detail_hit = None;
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("mr_detail_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 175.0) as u8),
                );
                if resp.clicked() {
                    self.mr_detail_hit = None;
                }
            });
        let Some(hit) = self.mr_detail_hit.clone() else {
            return;
        };

        let max_w = (screen.width() - 40.0).clamp(320.0, 680.0);
        let max_h = (screen.height() - 56.0).clamp(280.0, 560.0);
        let mr_type = self.mr_type;
        let mr_installing = self.mr_installing;
        let target_label = self
            .profiles
            .get(self.selected_idx)
            .map(|p| format!("{} • {}", p.version_id, p.loader.kind().label()))
            .unwrap_or_else(|| "Default".to_string());

        let mut open = true;
        let mut install_version_req: Option<ModrinthVersion> = None;
        let offset_y = (1.0 - ease_out_cubic(modal_t)) * 18.0;

        let win_resp = egui::Window::new(&hit.title)
            .id(egui::Id::new("mr_project_details_modal"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(max_w, max_h))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, offset_y))
            .show(ctx, |ui| {
                let tex_opt = hit
                    .icon_url
                    .as_ref()
                    .and_then(|u| self.mr_icons.get(u))
                    .and_then(|o| o.as_ref());

                ui_toolbar_row(ui, 52.0, |ui| {
                    let icon_size = Vec2::splat(48.0);
                    if let Some(tex) = tex_opt {
                        ui.add(
                            egui::Image::new(tex)
                                .fit_to_exact_size(icon_size)
                                .rounding(Rounding::same(10.0)),
                        );
                    } else {
                        let (r, _) = ui.allocate_exact_size(icon_size, Sense::hover());
                        draw_icon_placeholder(ui, r, 10.0, &hit.title, false);
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&hit.title)
                                .strong()
                                .size(15.5)
                                .color(text_primary()),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {} • {} {}",
                                tr(self.settings.language, "mr.by"),
                                hit.author,
                                format_downloads(hit.downloads),
                                tr(self.settings.language, "mr.downloads")
                            ))
                            .small()
                            .color(text_muted()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ext_label = hit.source.label();
                        if ui_button_icon(
                            ui,
                            "mr_detail_ext_btn",
                            Some(Icon::External),
                            ext_label,
                            ButtonVariant::Subtle,
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            let url = hit.website_url.clone().unwrap_or_else(|| {
                                format!(
                                    "https://modrinth.com/{}/{}",
                                    mr_type.api_str(),
                                    hit.project_id
                                )
                            });
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                    });
                });

                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&hit.description)
                            .small()
                            .color(text_secondary()),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui_toolbar_row(ui, 26.0, |ui| {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "mr.compat_versions"))
                            .strong()
                            .small()
                            .color(text_secondary()),
                    );
                    badge(ui, &target_label, true);
                });
                ui.add_space(6.0);

                let list_h = (ui.available_height() - 8.0).max(120.0);
                if self.mr_detail_loading {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), list_h),
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.horizontal(|ui| {
                                mono_spinner(ui, 20.0);
                                ui.label(
                                    egui::RichText::new(tr(
                                        self.settings.language,
                                        "mr.loading_versions",
                                    ))
                                    .color(text_secondary()),
                                );
                            });
                        },
                    );
                } else if let Some(Err(err)) = &self.mr_detail_versions {
                    ui_alert_banner(ui, AlertKind::Error, err, ui.available_width());
                } else if let Some(Ok(versions)) = &self.mr_detail_versions {
                    if versions.is_empty() {
                        ui_alert_banner(
                            ui,
                            AlertKind::Info,
                            tr(self.settings.language, "mr.no_compat_versions"),
                            ui.available_width(),
                        );
                    } else {
                        let scroll_w = ui.available_width();
                        let inner_w = (scroll_w - 38.0).max(160.0);
                        egui::ScrollArea::vertical()
                            .id_salt("mr_detail_versions_scroll")
                            .max_height(list_h)
                            .show(ui, |ui| {
                                ui.set_max_width((scroll_w - 14.0).max(160.0));
                                for ver in versions.iter().take(45) {
                                    let ver_card_id =
                                        ui.make_persistent_id(("mr_ver_row", &ver.id));
                                    animated_card(ui, ver_card_id, false, |ui| {
                                        ui.set_min_width(inner_w);
                                        ui.set_max_width(inner_w);
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(inner_w, 34.0),
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let ch_accent = ver.version_type == "release";
                                                if ui_button_sized(
                                                    ui,
                                                    ("mr_ver_inst", &ver.id),
                                                    Some(Icon::Download),
                                                    if mr_type == ModrinthProjectType::Modpack {
                                                        tr(
                                                            self.settings.language,
                                                            "modal.create_instance",
                                                        )
                                                    } else {
                                                        tr(self.settings.language, "mr.install")
                                                    },
                                                    if ch_accent {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Secondary
                                                    },
                                                    !mr_installing,
                                                    Vec2::new(88.0, 28.0),
                                                )
                                                .clicked()
                                                {
                                                    install_version_req = Some(ver.clone());
                                                }

                                                let title_str = if ver.name.trim().is_empty() {
                                                    &ver.version_number
                                                } else {
                                                    &ver.name
                                                };
                                                let ch_lbl = if ver.version_type.is_empty() {
                                                    "release"
                                                } else {
                                                    ver.version_type.as_str()
                                                };
                                                let file_lbl = ver
                                                    .files
                                                    .first()
                                                    .map(|f| f.filename.as_str())
                                                    .unwrap_or_default();
                                                let date_lbl = format_relative_time(
                                                    Some(&ver.date_published),
                                                    self.settings.language,
                                                )
                                                .unwrap_or_default();

                                                let left_w = (ui.available_width() - 8.0).max(40.0);
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(left_w, 34.0),
                                                    egui::Layout::top_down(egui::Align::Min),
                                                    |ui| {
                                                        ui.set_max_width(left_w);
                                                        ui_toolbar_row(ui, 18.0, |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(title_str)
                                                                        .strong()
                                                                        .color(text_primary()),
                                                                )
                                                                .truncate(),
                                                            );
                                                            badge(ui, ch_lbl, ch_accent);
                                                        });
                                                        let meta = [
                                                            ver.version_number.as_str(),
                                                            date_lbl.as_str(),
                                                            file_lbl,
                                                        ]
                                                        .into_iter()
                                                        .filter(|s| !s.is_empty())
                                                        .collect::<Vec<_>>()
                                                        .join(" • ");
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(meta)
                                                                    .small()
                                                                    .color(text_muted()),
                                                            )
                                                            .truncate(),
                                                        );
                                                    },
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                    }
                }
            });

        if let Some(ir) = win_resp {
            if modal_t < 0.99 {
                let win_r = ir.response.rect;
                ctx.layer_painter(ir.response.layer_id).rect_stroke(
                    win_r,
                    Rounding::same(12.0),
                    Stroke::new(
                        1.5_f32,
                        with_alpha(accent(), ((1.0 - modal_t) * 165.0) as u8),
                    ),
                );
                paint_fade_veil(
                    &ctx.layer_painter(ir.response.layer_id),
                    win_r,
                    bg_void(),
                    modal_t,
                    110.0,
                );
            }
        }
        if !open {
            self.mr_detail_hit = None;
        }
        if let Some(version) = install_version_req {
            let cancel_tok = Arc::new(AtomicBool::new(false));
            self.mr_cancel_token = Some(Arc::clone(&cancel_tok));
            let target_srv = self
                .discover_target_server_id
                .as_ref()
                .and_then(|id| self.servers.iter().find(|s| &s.id == id))
                .cloned()
                .or_else(|| {
                    if self.mr_type == ModrinthProjectType::Plugin {
                        self.servers.get(self.selected_server_idx).cloned()
                    } else {
                        None
                    }
                });

            if let Some(srv) = target_srv {
                self.mr_installing = true;
                self.mr_installing_id = Some(hit.project_id.clone());
                self.mr_error = None;
                self.mr_status = Some(format!(
                    "{} {} ({}) -> {}...",
                    tr(self.settings.language, "mr.installing"),
                    hit.title,
                    version.version_number,
                    srv.name
                ));
                self.progress = 0.05;
                self.mr_detail_hit = None;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                let pid = hit.project_id.clone();
                let title = hit.title.clone();
                let icon_url = hit.icon_url.clone();
                let cf_key = self.settings.curseforge_api_key.clone();
                thread::spawn(move || {
                    let res = with_download_cancel_token(cancel_tok, || {
                        if let Some(cf_mod_id) = parse_cf_project_id(&pid) {
                            install_curseforge_server_project(
                                &cf_key,
                                &paths,
                                &srv,
                                cf_mod_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                Some(&version),
                                &tx,
                            )
                        } else {
                            install_server_specific_version(
                                &paths,
                                &srv,
                                &pid,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                &version,
                                &tx,
                            )
                        }
                    });
                    let _ = tx.send(WorkerEvent::ServerProjectInstalled(res));
                });
            } else if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                self.mr_installing = true;
                self.mr_installing_id = Some(hit.project_id.clone());
                self.mr_error = None;
                self.mr_status = Some(format!(
                    "{} {} ({})...",
                    tr(self.settings.language, "mr.installing"),
                    hit.title,
                    version.version_number
                ));
                self.progress = 0.05;
                self.mr_detail_hit = None;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                let pid = hit.project_id.clone();
                let title = hit.title.clone();
                let icon_url = hit.icon_url.clone();
                let cf_key = self.settings.curseforge_api_key.clone();
                thread::spawn(move || {
                    let res = with_download_cancel_token(cancel_tok, || {
                        if let Some(cf_mod_id) = parse_cf_project_id(&pid) {
                            install_curseforge_project(
                                &cf_key,
                                &paths,
                                &profile,
                                cf_mod_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                Some(&version),
                                &tx,
                            )
                        } else {
                            install_specific_version(
                                &paths,
                                &profile,
                                &pid,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                &version,
                                &tx,
                            )
                        }
                    });
                    let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
                });
            }
        }
    }

    pub(super) fn render_command_palette_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("cmd_palette_anim"),
            self.show_cmd_palette,
            0.13,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_cmd_palette && modal_t <= 0.01 {
            return;
        }

        #[derive(Clone, Copy)]
        enum PaletteAction {
            Page(NavPage),
            DiscoverType(ModrinthProjectType),
            ContentFolder(ContentFolder),
            InstanceTab(InstanceSubTab),
            LaunchSelected,
            NewInstance,
            CheckUpdates,
            OpenInstanceFolder,
            CopyLogs,
            ClearLogs,
            SelectInstance(usize),
            SelectAndLaunch(usize),
        }

        struct PaletteItem {
            category: PaletteCategory,
            icon: Icon,
            title: String,
            subtitle: String,
            keywords: &'static str,
            shortcut: &'static str,
            action: PaletteAction,
        }

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("cmd_palette_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 170.0) as u8),
                );
                if resp.clicked() {
                    self.show_cmd_palette = false;
                }
            });
        if !self.show_cmd_palette {
            return;
        }

        let active_prof_summary = self
            .profiles
            .get(self.selected_idx)
            .map(|p| format!("{} ({})", p.name, p.version_id))
            .unwrap_or_else(|| tr(self.settings.language, "mr.no_instance").to_string());

        let lang = self.settings.language;
        let mut items: Vec<PaletteItem> = vec![
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Grid,
                title: tr(lang, "nav.library").to_string(),
                subtitle: tr(lang, "lib.title").to_string(),
                keywords: "library home instances profiles библиотека главная инстансы профили",
                shortcut: "Ctrl+1",
                action: PaletteAction::Page(NavPage::Library),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Compass,
                title: format!("{} — {}", tr(lang, "nav.discover"), tr(lang, "content.mods")),
                subtitle: tr(lang, "mr.search_placeholder").to_string(),
                keywords: "discover search modrinth mods каталог поиск моды модрич",
                shortcut: "Ctrl+2",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Mod),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Layers,
                title: format!("{} — {}", tr(lang, "nav.discover"), tr(lang, "content.resourcepacks")),
                subtitle: tr(lang, "mr.search_placeholder").to_string(),
                keywords: "discover resourcepacks textures ресурспаки текстуры",
                shortcut: "",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Resourcepack),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Star,
                title: format!("{} — {}", tr(lang, "nav.discover"), tr(lang, "content.shaderpacks")),
                subtitle: tr(lang, "mr.search_placeholder").to_string(),
                keywords: "discover shaders shaderpacks bsl iris шейдеры",
                shortcut: "",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Shader),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Download,
                title: format!("{} — {}", tr(lang, "nav.discover"), tr(lang, "mr.modpacks")),
                subtitle: tr(lang, "mr.search_placeholder").to_string(),
                keywords: "discover modpacks mrpack сборки модпаки",
                shortcut: "",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Modpack),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Layers,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "content.mods")),
                subtitle: tr(lang, "lib.manage_mods").to_string(),
                keywords: "instance content mods jar моды инстанс список модов",
                shortcut: "Ctrl+3",
                action: PaletteAction::ContentFolder(ContentFolder::Mods),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Grid,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "content.resourcepacks")),
                subtitle: tr(lang, "lib.manage_content").to_string(),
                keywords: "instance content resourcepacks textures ресурспаки текстуры",
                shortcut: "",
                action: PaletteAction::ContentFolder(ContentFolder::Resourcepacks),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Star,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "content.shaderpacks")),
                subtitle: tr(lang, "lib.manage_content").to_string(),
                keywords: "instance content shaderpacks shaders шейдеры",
                shortcut: "",
                action: PaletteAction::ContentFolder(ContentFolder::Shaderpacks),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Folder,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "inst.tab_worlds")),
                subtitle: tr(lang, "btn.open_saves").to_string(),
                keywords: "worlds saves singleplayer миры сохранения",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Worlds),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Grid,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "inst.tab_shots")),
                subtitle: tr(lang, "btn.open_shots").to_string(),
                keywords: "screenshots photos gallery скриншоты фото",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Screenshots),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Sliders,
                title: format!("{} — {}", tr(lang, "nav.instance"), tr(lang, "inst.settings")),
                subtitle: tr(lang, "lib.settings").to_string(),
                keywords: "settings version loader fabric forge quilt neoforge vanilla ram java jvm gpu window server настройки опра память джава версии лоадер",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Settings),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Server,
                title: tr(lang, "nav.servers").to_string(),
                subtitle: tr(lang, "srv.title").to_string(),
                keywords: "server dedicated paper purpur fabric vanilla host port серверы дедикейтед хост",
                shortcut: "Ctrl+4",
                action: PaletteAction::Page(NavPage::Servers),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Terminal,
                title: tr(lang, "nav.console").to_string(),
                subtitle: tr(lang, "bar.logs").to_string(),
                keywords: "console logs crash terminal диагностика консоль логи краш",
                shortcut: "Ctrl+4",
                action: PaletteAction::Page(NavPage::Console),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::User,
                title: tr(lang, "nav.accounts").to_string(),
                subtitle: tr(lang, "rpc.accounts").to_string(),
                keywords: "accounts profile microsoft ely offline login аккаунты вход ник",
                shortcut: "Ctrl+5",
                action: PaletteAction::Page(NavPage::Accounts),
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Play,
                title: tr(lang, "top.play").to_string(),
                subtitle: active_prof_summary,
                keywords: "play launch start run game играть запуск старт",
                shortcut: "",
                action: PaletteAction::LaunchSelected,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Plus,
                title: tr(lang, "modal.create_title").to_string(),
                subtitle: tr(lang, "btn.new_instance").to_string(),
                keywords: "new create profile instance создать новый профиль инстанс",
                shortcut: "Ctrl+N",
                action: PaletteAction::NewInstance,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Refresh,
                title: tr(lang, "btn.check_updates").to_string(),
                subtitle: tr(lang, "content.checking_updates").to_string(),
                keywords: "update check mods refresh обновить моды апдейт",
                shortcut: "",
                action: PaletteAction::CheckUpdates,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Folder,
                title: tr(lang, "lib.open_folder").to_string(),
                subtitle: tr(lang, "btn.open_folder").to_string(),
                keywords: "folder directory open files папка директория файлы",
                shortcut: "",
                action: PaletteAction::OpenInstanceFolder,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Copy,
                title: format!("{} — {}", tr(lang, "nav.console"), tr(lang, "btn.copy")),
                subtitle: tr(lang, "bar.logs").to_string(),
                keywords: "copy clipboard logs console скопировать логи буфер",
                shortcut: "",
                action: PaletteAction::CopyLogs,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Trash,
                title: format!("{} — {}", tr(lang, "nav.console"), tr(lang, "btn.clear")),
                subtitle: tr(lang, "bar.logs").to_string(),
                keywords: "clear reset logs console очистить логи консоль",
                shortcut: "",
                action: PaletteAction::ClearLogs,
            },
        ];

        for (idx, p) in self.profiles.iter().enumerate() {
            items.push(PaletteItem {
                category: PaletteCategory::Instances,
                icon: Icon::Layers,
                title: format!("{}: {}", tr(lang, "palette.open_instance"), p.name),
                subtitle: format!("{} • {}", p.version_id, p.loader.kind().label()),
                keywords: "instance profile switch открыть переключить профиль",
                shortcut: "",
                action: PaletteAction::SelectInstance(idx),
            });
            items.push(PaletteItem {
                category: PaletteCategory::Instances,
                icon: Icon::Play,
                title: format!("{}: {}", tr(lang, "top.play"), p.name),
                subtitle: format!(
                    "{}: {} ({})",
                    tr(lang, "palette.switch_launch"),
                    p.version_id,
                    p.loader.kind().label()
                ),
                keywords: "play launch instance profile играть запустить",
                shortcut: "",
                action: PaletteAction::SelectAndLaunch(idx),
            });
        }

        let (key_down, key_up, key_enter, key_tab, key_esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Tab),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if key_esc {
            self.show_cmd_palette = false;
            return;
        }
        if key_tab {
            let next_idx = (PaletteCategory::ALL
                .iter()
                .position(|&c| c == self.cmd_palette_category)
                .unwrap_or(0)
                + 1)
                % PaletteCategory::ALL.len();
            self.cmd_palette_category = PaletteCategory::ALL[next_idx];
            self.cmd_palette_selected = 0;
        }

        let mut scored: Vec<(i32, usize, PaletteItem)> = items
            .into_iter()
            .enumerate()
            .filter(|(_, it)| {
                self.cmd_palette_category == PaletteCategory::All
                    || it.category == self.cmd_palette_category
            })
            .filter_map(|(order_idx, it)| {
                score_palette_match(
                    &self.cmd_palette_query,
                    &it.title,
                    &it.subtitle,
                    it.keywords,
                )
                .map(|sc| (sc, order_idx, it))
            })
            .collect();
        if !self.cmd_palette_query.trim().is_empty() {
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        }

        let total_matches = scored.len();
        if total_matches == 0 {
            self.cmd_palette_selected = 0;
        } else {
            if self.cmd_palette_selected >= total_matches {
                self.cmd_palette_selected = total_matches - 1;
            }
            if key_down {
                self.cmd_palette_selected = (self.cmd_palette_selected + 1) % total_matches;
            } else if key_up {
                self.cmd_palette_selected = if self.cmd_palette_selected == 0 {
                    total_matches - 1
                } else {
                    self.cmd_palette_selected - 1
                };
            }
        }
        let scroll_to_selection = key_down || key_up;

        let max_w = (screen.width() - 36.0).clamp(320.0, 580.0);
        let max_h = (screen.height() - 100.0).clamp(280.0, 440.0);
        let mut chosen: Option<PaletteAction> = None;
        let palette_y = 32.0 + ease_out_cubic(modal_t) * 26.0;

        let win_resp = egui::Window::new(tr(self.settings.language, "palette.title"))
            .id(egui::Id::new("cmd_palette_modal"))
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(max_w, max_h))
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, palette_y))
            .show(ctx, |ui| {
                // Top styled search input bar
                let bar_w = ui.available_width();
                let mut clear_clicked = false;
                let mut refocus_input =
                    self.cmd_palette_just_opened || key_tab || key_up || key_down;
                egui::Frame::none()
                    .fill(bg_input())
                    .stroke(Stroke::new(1.2_f32, with_alpha(accent(), 185)))
                    .rounding(Rounding::same(9.0))
                    .inner_margin(Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_min_width((bar_w - 22.0).max(120.0));
                        ui_toolbar_row(ui, 26.0, |ui| {
                            let (ic_r, _) =
                                ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                            draw_icon(ui.painter(), ic_r, Icon::Search, accent());

                            let right_reserve = if self.cmd_palette_query.is_empty() {
                                46.0
                            } else {
                                74.0
                            };
                            let edit_w = (ui.available_width() - right_reserve).max(90.0);
                            let edit_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.cmd_palette_query)
                                    .id_salt("cmd_palette_search_edit")
                                    .frame(false)
                                    .hint_text(tr(self.settings.language, "palette.hint"))
                                    .desired_width(edit_w),
                            );
                            if edit_resp.changed() {
                                self.cmd_palette_selected = 0;
                            }
                            if refocus_input {
                                edit_resp.request_focus();
                                self.cmd_palette_just_opened = false;
                                refocus_input = false;
                            }
                            let _ = refocus_input;

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    badge(ui, "Esc", false);
                                    if !self.cmd_palette_query.is_empty()
                                        && ui_icon_button(
                                            ui,
                                            "cmd_palette_clear_btn",
                                            Icon::Close,
                                            tr(self.settings.language, "palette.clear"),
                                            ButtonVariant::Subtle,
                                            true,
                                            24.0,
                                        )
                                        .clicked()
                                    {
                                        clear_clicked = true;
                                    }
                                },
                            );
                        });
                    });
                if clear_clicked {
                    self.cmd_palette_query.clear();
                    self.cmd_palette_selected = 0;
                    self.cmd_palette_just_opened = true;
                }

                ui.add_space(6.0);
                // Category filter dropdown
                ui_toolbar_row(ui, 26.0, |ui| {
                    let prev_cat = self.cmd_palette_category;
                    egui::ComboBox::from_id_salt("cmd_cat_combo")
                        .selected_text(
                            self.cmd_palette_category
                                .translated_label(self.settings.language),
                        )
                        .width(148.0)
                        .truncate()
                        .show_ui(ui, |ui| {
                            for cat in PaletteCategory::ALL {
                                ui.selectable_value(
                                    &mut self.cmd_palette_category,
                                    cat,
                                    cat.translated_label(self.settings.language),
                                );
                            }
                        });
                    if self.cmd_palette_category != prev_cat {
                        self.cmd_palette_selected = 0;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{total_matches} {}",
                                tr(self.settings.language, "palette.found")
                            ))
                            .small()
                            .color(text_muted()),
                        );
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if key_enter {
                    if let Some((_, _, it)) = scored.get(self.cmd_palette_selected) {
                        chosen = Some(it.action);
                    }
                }

                let list_h = (ui.available_height() - 28.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt("cmd_palette_scroll")
                    .max_height(list_h)
                    .show(ui, |ui| {
                        if scored.is_empty() {
                            ui.add_space(16.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(tr(
                                        self.settings.language,
                                        "palette.empty",
                                    ))
                                    .color(text_secondary()),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(tr(
                                        self.settings.language,
                                        "palette.empty_sub",
                                    ))
                                    .small()
                                    .color(text_muted()),
                                );
                            });
                            return;
                        }

                        for (idx, (_, _, it)) in scored.into_iter().enumerate() {
                            let is_sel = idx == self.cmd_palette_selected;
                            let row_id = ui.make_persistent_id(("cmd_row", idx));
                            let card_r = animated_card(ui, row_id, is_sel, |ui| {
                                ui.set_min_width((ui.available_width() - 8.0).max(180.0));
                                ui_toolbar_row(ui, 34.0, |ui| {
                                    let (ic_box, _) =
                                        ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                    ui.painter().rect(
                                        ic_box,
                                        Rounding::same(7.0),
                                        if is_sel {
                                            with_alpha(accent(), 34)
                                        } else {
                                            bg_input()
                                        },
                                        Stroke::new(
                                            1.0_f32,
                                            if is_sel {
                                                with_alpha(accent(), 130)
                                            } else {
                                                border_subtle()
                                            },
                                        ),
                                    );
                                    draw_icon(
                                        ui.painter(),
                                        ic_box.shrink(6.0),
                                        it.icon,
                                        if is_sel { accent() } else { text_secondary() },
                                    );

                                    let shortcut_reserve =
                                        if it.shortcut.is_empty() { 70.0 } else { 130.0 };
                                    let text_w =
                                        (ui.available_width() - shortcut_reserve).max(100.0);
                                    ui.allocate_ui(Vec2::new(text_w, 34.0), |ui| {
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 0.0;
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&it.title)
                                                        .strong()
                                                        .color(text_primary()),
                                                )
                                                .truncate(),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&it.subtitle)
                                                        .small()
                                                        .color(text_muted()),
                                                )
                                                .truncate(),
                                            );
                                        });
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if !it.shortcut.is_empty() {
                                                badge(ui, it.shortcut, is_sel);
                                            }
                                            ui.label(
                                                egui::RichText::new(
                                                    it.category
                                                        .translated_badge(self.settings.language),
                                                )
                                                .small()
                                                .color(text_muted()),
                                            );
                                        },
                                    );
                                });
                            });
                            let row_resp = card_r.response.interact(Sense::click());
                            if row_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                self.cmd_palette_selected = idx;
                            }
                            if is_sel && scroll_to_selection {
                                row_resp.scroll_to_me(Some(egui::Align::Center));
                            }
                            if row_resp.clicked() {
                                anim::emit_ring(ctx, row_resp.rect.center(), accent(), 34.0);
                                chosen = Some(it.action);
                            }
                            ui.add_space(3.0);
                        }
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui_toolbar_row(ui, 20.0, |ui| {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "palette.footer_hint"))
                            .small()
                            .color(text_muted()),
                    );
                });
            });

        if let Some(ir) = win_resp {
            if modal_t < 0.99 {
                let win_r = ir.response.rect;
                ctx.layer_painter(ir.response.layer_id).rect_stroke(
                    win_r,
                    Rounding::same(12.0),
                    Stroke::new(
                        1.5_f32,
                        with_alpha(accent(), ((1.0 - modal_t) * 165.0) as u8),
                    ),
                );
                paint_fade_veil(
                    &ctx.layer_painter(ir.response.layer_id),
                    win_r,
                    bg_void(),
                    modal_t,
                    110.0,
                );
            }
        }

        if let Some(act) = chosen {
            self.show_cmd_palette = false;
            match act {
                PaletteAction::Page(page) => self.switch_page(ctx, page),
                PaletteAction::DiscoverType(mr_type) => {
                    if self.mr_type != mr_type {
                        self.mr_type = mr_type;
                        self.mr_query.clear();
                        self.mr_results.clear();
                        self.trigger_modrinth_search(false);
                    }
                    self.switch_page(ctx, NavPage::Discover);
                }
                PaletteAction::ContentFolder(folder) => {
                    self.content_folder = folder;
                    self.instance_tab = InstanceSubTab::Content;
                    self.switch_page(ctx, NavPage::Instance);
                }
                PaletteAction::InstanceTab(tab) => {
                    self.instance_tab = tab;
                    self.switch_page(ctx, NavPage::Instance);
                }
                PaletteAction::LaunchSelected => self.start_launch(),
                PaletteAction::NewInstance => self.show_new_modal = true,
                PaletteAction::CheckUpdates => {
                    if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                        let cancel_tok = Arc::new(AtomicBool::new(false));
                        self.mr_cancel_token = Some(Arc::clone(&cancel_tok));
                        self.mr_installing = true;
                        self.progress = 0.08;
                        self.status_text =
                            tr(self.settings.language, "content.checking_updates").to_string();
                        let paths = self.paths.clone();
                        let cf_key = self.settings.curseforge_api_key.clone();
                        let tx = self.tx.clone();
                        thread::spawn(move || {
                            let res = with_download_cancel_token(cancel_tok, || {
                                update_all_installed_projects(&paths, &profile, &cf_key, &tx)
                            });
                            let _ = tx.send(WorkerEvent::ModUpdatesFinished(res));
                        });
                    }
                }
                PaletteAction::OpenInstanceFolder => {
                    if let Some(profile) = self.profiles.get(self.selected_idx) {
                        open_in_file_manager(&instance_dir(&self.paths, profile));
                    }
                }
                PaletteAction::CopyLogs => {
                    let text: String = self
                        .current_console_logs()
                        .iter()
                        .map(|e| format!("[{}] {}", e.timestamp, e.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ctx.copy_text(text);
                    let msg = tr(self.settings.language, "toast.logs_copied").to_string();
                    self.show_toast(ctx, msg);
                }
                PaletteAction::ClearLogs => {
                    self.clear_logs();
                    let msg = tr(self.settings.language, "toast.logs_cleared").to_string();
                    self.show_toast(ctx, msg);
                }
                PaletteAction::SelectInstance(idx) => {
                    if idx < self.profiles.len() {
                        self.select_profile(idx);
                        self.switch_page(ctx, NavPage::Instance);
                    }
                }
                PaletteAction::SelectAndLaunch(idx) => {
                    if idx < self.profiles.len() {
                        self.select_profile(idx);
                        self.start_launch();
                    }
                }
            }
        }
    }

    pub(super) fn render_export_mrpack_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("export_mrpack_modal_anim"),
            self.show_export_mrpack_modal,
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_export_mrpack_modal && modal_t <= 0.01 {
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("export_mrpack_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 170.0) as u8),
                );
                if resp.clicked() {
                    self.show_export_mrpack_modal = false;
                }
            });
        if !self.show_export_mrpack_modal {
            return;
        }

        let lang = self.settings.language;
        let idx = self.export_mrpack_profile_idx.unwrap_or(self.selected_idx);
        let prof_name = self
            .profiles
            .get(idx)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Instance".to_string());
        let max_modal_w = (screen.width() - 32.0).clamp(260.0, 440.0);
        let max_modal_h = (screen.height() - 32.0).max(240.0);
        let field_w = (max_modal_w - 36.0).max(160.0);
        let offset_y = (1.0 - ease_out_cubic(modal_t)) * 18.0;

        let mut open = true;
        let mut do_export = false;
        let mut do_cancel = false;

        let win_resp = egui::Window::new(format!("{}: {prof_name}", tr(lang, "export.title")))
            .id(egui::Id::new("export_mrpack_modal_win"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_modal_w)
            .max_height(max_modal_h)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, offset_y))
            .show(ctx, |ui| {
                ui.set_min_width(field_w);
                egui::ScrollArea::vertical()
                    .id_salt("export_mrpack_modal_scroll")
                    .auto_shrink([false, true])
                    .max_height((max_modal_h - 90.0).max(120.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(tr(lang, "export.desc"))
                                .small()
                                .color(text_secondary()),
                        );
                        ui.add_space(6.0);
                        ui_checkbox(
                            ui,
                            "exp_opt_config",
                            &mut self.export_mrpack_opts.include_config,
                            tr(lang, "export.inc_config"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_options",
                            &mut self.export_mrpack_opts.include_options_txt,
                            tr(lang, "export.inc_options"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_unmatched",
                            &mut self.export_mrpack_opts.include_unmatched_mods,
                            tr(lang, "export.inc_unmatched"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_rpacks",
                            &mut self.export_mrpack_opts.include_resourcepacks,
                            tr(lang, "content.resourcepacks"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_shaders",
                            &mut self.export_mrpack_opts.include_shaderpacks,
                            tr(lang, "content.shaderpacks"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_saves",
                            &mut self.export_mrpack_opts.include_saves,
                            tr(lang, "export.inc_saves"),
                        );
                        ui_checkbox(
                            ui,
                            "exp_opt_shots",
                            &mut self.export_mrpack_opts.include_screenshots,
                            tr(lang, "export.inc_screenshots"),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(tr(lang, "export.extra_excludes"))
                                .small()
                                .color(text_secondary()),
                        );
                        ui_text_input(
                            ui,
                            "exp_extra_excludes",
                            &mut self.export_mrpack_opts.extra_excludes,
                            "*.bak, config/secret.toml, journey-map/",
                            field_w,
                            true,
                        );
                    });
                ui.add_space(10.0);
                ui_toolbar_row(ui, 30.0, |ui| {
                    if ui_button_icon(
                        ui,
                        "exp_confirm_btn",
                        Some(Icon::Download),
                        tr(lang, "btn.export_mrpack"),
                        ButtonVariant::Primary,
                        !self.is_busy,
                        30.0,
                    )
                    .clicked()
                    {
                        do_export = true;
                    }
                    if ui_button(
                        ui,
                        "exp_cancel_btn",
                        tr(lang, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        do_cancel = true;
                    }
                });
            });

        if let Some(ir) = win_resp {
            ctx.move_to_top(ir.response.layer_id);
        }
        if !open || do_cancel {
            self.show_export_mrpack_modal = false;
        }
        if do_export {
            self.show_export_mrpack_modal = false;
            let opts = self.export_mrpack_opts.clone();
            self.start_export_mrpack_with_options(idx, opts);
        }
    }
}
