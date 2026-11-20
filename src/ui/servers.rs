use super::*;

impl McLauncherApp {
    pub(super) fn start_provision_server(&mut self, server: DedicatedServer) {
        reset_download_cancel();
        self.server_busy = true;
        self.is_busy = true;
        self.progress = 0.08;
        self.status_text = format!(
            "Provisioning {} {} server...",
            server.software.label(),
            server.mc_version
        );
        let paths = self.paths.clone();
        let manifest = self.manifest.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = provision_server_jar(&paths, &server, manifest.as_ref(), Some(&tx));
            let _ = tx.send(WorkerEvent::ServerProvisioned(res));
        });
    }

    pub(super) fn start_server_process(&mut self, server: DedicatedServer) {
        if self.server_handle.is_some() || self.server_busy {
            return;
        }
        reset_download_cancel();
        self.server_busy = true;
        self.is_busy = true;
        self.progress = 0.1;
        self.status_text = format!("Starting {}...", server.name);
        self.server_logs.clear();
        self.server_tunnel_addr = None;
        self.server_tunnel_error = None;
        let paths = self.paths.clone();
        let manifest = self.manifest.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = start_dedicated_server(&paths, &server, manifest.as_ref(), Some(&tx));
            let _ = tx.send(WorkerEvent::ServerStarted(res));
        });
    }

    pub(super) fn render_servers_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.settings.language;
        if self.selected_server_idx >= self.servers.len() {
            self.selected_server_idx = 0;
            self.server_detail_open = false;
        }

        if !self.server_detail_open || self.servers.is_empty() {
            ui_toolbar_row(ui, 32.0, |ui| {
                ui.heading(
                    egui::RichText::new(tr(lang, "srv.title"))
                        .size(21.0)
                        .color(text_primary()),
                );
                ui.label(
                    egui::RichText::new(format!("{}", self.servers.len()))
                        .monospace()
                        .size(12.0)
                        .color(text_muted()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_button_icon(
                        ui,
                        "srv_new_btn",
                        Some(Icon::Plus),
                        tr(lang, "srv.new"),
                        ButtonVariant::Primary,
                        !self.server_busy,
                        28.0,
                    )
                    .clicked()
                    {
                        self.show_create_server_modal = true;
                    }
                });
            });
            ui.add_space(12.0);
        } else {
            let srv_name = self.servers[self.selected_server_idx].name.clone();
            ui_toolbar_row(ui, 32.0, |ui| {
                if ui_button_icon(
                    ui,
                    "srv_back_btn",
                    Some(Icon::ChevronLeft),
                    tr(lang, "srv.back"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    self.server_detail_open = false;
                    self.mark_subtab_switch(ctx);
                }
                ui.heading(
                    egui::RichText::new(&srv_name)
                        .size(20.0)
                        .color(text_primary()),
                );
            });
            ui.add_space(12.0);
        }

        if self.servers.is_empty() {
            let avail_w = ui.available_width();
            let card_w = avail_w.min(540.0);
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width((card_w - 28.0).max(160.0));
                ui.set_max_width((card_w - 28.0).max(160.0));
                ui.heading(
                    egui::RichText::new(tr(lang, "srv.empty_title"))
                        .size(16.0)
                        .color(text_primary()),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(tr(lang, "srv.empty_sub"))
                        .small()
                        .color(text_secondary()),
                );
                ui.add_space(12.0);
                if ui_button_icon(
                    ui,
                    "srv_empty_create_btn",
                    Some(Icon::Plus),
                    tr(lang, "srv.new"),
                    ButtonVariant::Primary,
                    !self.server_busy,
                    30.0,
                )
                .clicked()
                {
                    self.show_create_server_modal = true;
                }
            });
            return;
        }

        if self.selected_server_idx >= self.servers.len() {
            self.selected_server_idx = 0;
        }

        if !self.server_detail_open {
            let subtab_t = self.subtab_switch_time;
            Self::apply_subtab_transition(ui, subtab_t, |ui| {
            bounded_scroll(ui, "servers_page_scroll", |ui| {
                let avail_w = (ui.available_width() - 8.0).max(200.0);
                let gap = 12.0;
                let cols = ((avail_w + gap) / 285.0).floor().clamp(1.0, 3.0) as usize;
                let card_grid_w = ((avail_w - gap * (cols as f32 - 1.0)) / cols as f32).max(160.0);

                let mut action_start: Option<DedicatedServer> = None;
                let mut action_stop = false;
                let mut action_open: Option<usize> = None;

                let server_list: Vec<(usize, DedicatedServer)> =
                    self.servers.iter().cloned().enumerate().collect();
                for chunk in server_list.chunks(cols) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for (idx, item) in chunk {
                            let selected = *idx == self.selected_server_idx;
                            let is_running =
                                self.running_server_id.as_deref() == Some(item.id.as_str());
                            let inner_w = (card_grid_w - 22.0).max(110.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(card_grid_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    let card_id =
                                        ui.make_persistent_id(("srv_card_hover", &item.id));
                                    animated_card(ui, card_id, selected, |ui| {
                                        ui.set_min_width(inner_w);
                                        ui.set_max_width(inner_w);

                                        let hdr_resp = ui
                                            .horizontal(|ui| {
                                                let (icon_r, _) = ui.allocate_exact_size(
                                                    Vec2::splat(40.0),
                                                    Sense::hover(),
                                                );
                                                let bg = if selected {
                                                    accent_dim()
                                                } else {
                                                    bg_input()
                                                };
                                                let stroke_col = if is_running || selected {
                                                    accent()
                                                } else {
                                                    border_strong()
                                                };
                                                ui.painter().rect(
                                                    icon_r,
                                                    Rounding::same(9.0),
                                                    bg,
                                                    Stroke::new(1.1_f32, stroke_col),
                                                );
                                                draw_icon(
                                                    ui.painter(),
                                                    icon_r.shrink(10.0),
                                                    Icon::Server,
                                                    if selected || is_running {
                                                        accent_text()
                                                    } else {
                                                        text_secondary()
                                                    },
                                                );
                                                if is_running {
                                                    let dot = Pos2::new(
                                                        icon_r.right() - 5.0,
                                                        icon_r.bottom() - 5.0,
                                                    );
                                                    draw_running_pulse_dot(
                                                        ui,
                                                        dot,
                                                        2.8,
                                                        accent(),
                                                        bg_card(),
                                                    );
                                                }

                                                ui.add_space(4.0);
                                                ui.vertical(|ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&item.name)
                                                                .strong()
                                                                .size(14.5)
                                                                .color(text_primary()),
                                                        )
                                                        .truncate(),
                                                    );
                                                    ui_toolbar_row(ui, 20.0, |ui| {
                                                        ui.spacing_mut().item_spacing.x = 6.0;
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "{} {}",
                                                                item.software.label(),
                                                                item.mc_version
                                                            ))
                                                            .small()
                                                            .color(text_secondary()),
                                                        );
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                ":{}",
                                                                item.port
                                                            ))
                                                            .monospace()
                                                            .small()
                                                            .color(text_muted()),
                                                        );
                                                    });
                                                });
                                            })
                                            .response
                                            .interact(Sense::click());

                                        if hdr_resp.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if hdr_resp.clicked() {
                                            action_open = Some(*idx);
                                        }

                                        ui.add_space(8.0);
                                        ui_toolbar_row(ui, 28.0, |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            if is_running {
                                                if ui_button_icon(
                                                    ui,
                                                    ("srv_card_stop", &item.id),
                                                    Some(Icon::Stop),
                                                    tr(lang, "srv.stop"),
                                                    ButtonVariant::Secondary,
                                                    true,
                                                    26.0,
                                                )
                                                .clicked()
                                                {
                                                    action_stop = true;
                                                }
                                            } else {
                                                let can_start = !self.server_busy
                                                    && self.server_handle.is_none()
                                                    && item.eula_accepted;
                                                if ui_button_icon(
                                                    ui,
                                                    ("srv_card_start", &item.id),
                                                    Some(Icon::Play),
                                                    tr(lang, "srv.start"),
                                                    if selected {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Secondary
                                                    },
                                                    can_start,
                                                    26.0,
                                                )
                                                .clicked()
                                                {
                                                    action_open = Some(*idx);
                                                    action_start = Some(item.clone());
                                                }
                                            }
                                            if ui_button_icon(
                                                ui,
                                                ("srv_card_open", &item.id),
                                                Some(Icon::Sliders),
                                                tr(lang, "inst.settings"),
                                                ButtonVariant::Subtle,
                                                true,
                                                26.0,
                                            )
                                            .clicked()
                                            {
                                                action_open = Some(*idx);
                                            }
                                            if !is_running
                                                && ui_icon_button(
                                                    ui,
                                                    ("srv_card_del", &item.id),
                                                    Icon::Trash,
                                                    "Delete server",
                                                    ButtonVariant::Danger,
                                                    !self.server_busy,
                                                    26.0,
                                                )
                                                .clicked()
                                            {
                                                self.confirm_delete_server_idx = Some(*idx);
                                            }
                                        });
                                    });
                                },
                            );
                        }
                    });
                    ui.add_space(8.0);
                }

                if let Some(idx) = action_open {
                    self.selected_server_idx = idx;
                    self.server_detail_open = true;
                    self.mark_subtab_switch(ctx);
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
                }
                if action_stop {
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), DANGER, 34.0);
                    anim::emit_edge_pulse(ctx, DANGER);
                    if let Some(handle) = &self.server_handle {
                        if let Err(e) = stop_dedicated_server(handle) {
                            self.show_toast(ctx, e);
                        }
                    }
                }
                if let Some(srv_to_start) = action_start {
                    anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                    anim::emit_edge_pulse(ctx, accent());
                    self.start_server_process(srv_to_start);
                }
            });
            });
            return;
        }

        bounded_scroll(ui, "servers_detail_scroll", |ui| {
            let card_w = (ui.available_width() - 32.0).max(200.0);

            let srv = self.servers[self.selected_server_idx].clone();
            let is_this_running = self.running_server_id.as_deref() == Some(srv.id.as_str());
            let dir = server_dir(&self.paths, &srv);
            let has_jar = is_server_provisioned(&dir, srv.software);
            let srv_icon_tex = self.ensure_server_icon_loaded(ctx, &srv.id, &dir);

            // Server header & controls card
            card_frame(is_this_running, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                ui_wrap_row(ui, 30.0, |ui| {
                    let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                    if let Some(tex) = &srv_icon_tex {
                        ui.painter().rect_filled(ic_r, Rounding::same(6.0), bg_input());
                        ui.painter().image(
                            tex.id(),
                            ic_r.shrink(1.0),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.painter().rect_stroke(
                            ic_r,
                            Rounding::same(6.0),
                            Stroke::new(1.0_f32, border_subtle()),
                        );
                    } else {
                        ui.painter().rect_filled(ic_r, Rounding::same(6.0), bg_elevated());
                        ui.painter().rect_stroke(
                            ic_r,
                            Rounding::same(6.0),
                            Stroke::new(1.0_f32, border_subtle()),
                        );
                        crate::icons::draw_icon(
                            ui.painter(),
                            ic_r.shrink(6.0),
                            Icon::Server,
                            if is_this_running {
                                accent()
                            } else {
                                text_secondary()
                            },
                        );
                    }
                    ui.strong(
                        egui::RichText::new(&srv.name)
                            .size(16.0)
                            .color(text_primary()),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} • :{}",
                            srv.software.label(),
                            srv.mc_version,
                            srv.port
                        ))
                        .monospace()
                        .small()
                        .color(text_muted()),
                    );
                    if is_this_running {
                        let (dot_r, dot_resp) =
                            ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        draw_running_pulse_dot(ui, dot_r.center(), 3.2, accent(), bg_card());
                        dot_resp.on_hover_text(tr(lang, "lib.running"));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if !is_this_running
                            && ui_icon_button(
                                ui,
                                ("srv_del_btn", &srv.id),
                                Icon::Trash,
                                "Delete server",
                                ButtonVariant::Danger,
                                !self.server_busy,
                                28.0,
                            )
                            .clicked()
                        {
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), DANGER, 30.0);
                            self.confirm_delete_server_idx = Some(self.selected_server_idx);
                        }
                        if ui_icon_button(
                            ui,
                            ("srv_folder_btn", &srv.id),
                            Icon::Folder,
                            "Open server folder",
                            ButtonVariant::Secondary,
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                            open_in_file_manager(&dir);
                        }
                        if ui_button_icon(
                            ui,
                            ("srv_copy_addr_btn", &srv.id),
                            Some(Icon::Copy),
                            &format!("127.0.0.1:{}", srv.port),
                            ButtonVariant::Secondary,
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            ctx.copy_text(format!("127.0.0.1:{}", srv.port));
                            let pos = anim::pointer_or_center(ctx);
                            anim::emit_ring(ctx, pos, accent(), 28.0);
                            anim::emit_floating_badge(ctx, pos, Some(Icon::Check), "", accent());
                            self.show_toast(ctx, format!("Copied 127.0.0.1:{}", srv.port));
                        }
                        if is_this_running {
                            if let Some(handle) = &self.server_handle {
                                if ui_button_icon(
                                    ui,
                                    ("srv_stop_btn", &srv.id),
                                    Some(Icon::Stop),
                                    tr(lang, "srv.stop"),
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), DANGER, 34.0);
                                    anim::emit_edge_pulse(ctx, DANGER);
                                    if let Err(e) = stop_dedicated_server(handle) {
                                        self.show_toast(ctx, e);
                                    }
                                }
                            }
                        } else {
                            let can_start = !self.server_busy
                                && self.server_handle.is_none()
                                && srv.eula_accepted;
                            if ui_button_icon(
                                ui,
                                ("srv_start_btn", &srv.id),
                                Some(Icon::Play),
                                tr(lang, "srv.start"),
                                ButtonVariant::Primary,
                                can_start,
                                28.0,
                            )
                            .clicked()
                            {
                                anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                                anim::emit_edge_pulse(ctx, accent());
                                self.start_server_process(srv.clone());
                            }
                            if !has_jar
                                && ui_button_icon(
                                    ui,
                                    ("srv_dl_jar_btn", &srv.id),
                                    Some(Icon::Download),
                                    "Install server",
                                    ButtonVariant::Secondary,
                                    !self.server_busy,
                                    28.0,
                                )
                                .clicked()
                            {
                                anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
                                self.start_provision_server(srv.clone());
                            }
                        }
                    });
                });
                if is_this_running {
                    let public_addr = self.server_tunnel_addr.clone().or_else(|| {
                        self.server_handle
                            .as_ref()
                            .and_then(|h| h.upnp_mapping.as_ref())
                            .and_then(|m| m.public_ip.as_ref())
                            .map(|ip| format!("{ip}:{}", srv.port))
                    });
                    ui.add_space(8.0);
                    ui_wrap_row(ui, 26.0, |ui| {
                        if let Some(pub_addr) = public_addr {
                            ui.label(
                                egui::RichText::new("Public IP:")
                                    .small()
                                    .color(text_secondary()),
                            );
                            ui.label(
                                egui::RichText::new(&pub_addr)
                                    .monospace()
                                    .strong()
                                    .color(accent_text()),
                            );
                            if ui_button_icon(
                                ui,
                                ("srv_copy_pub_inline", &srv.id),
                                Some(Icon::Copy),
                                tr(lang, "console.copy"),
                                ButtonVariant::Secondary,
                                true,
                                24.0,
                            )
                            .clicked()
                            {
                                ctx.copy_text(pub_addr.clone());
                                let pos = anim::pointer_or_center(ctx);
                                anim::emit_ring(ctx, pos, accent(), 28.0);
                                anim::emit_floating_badge(ctx, pos, Some(Icon::Check), "", accent());
                                self.show_toast(ctx, format!("Copied {pub_addr}"));
                            }
                        } else if let Some(err) = &self.server_tunnel_error {
                            ui.label(
                                egui::RichText::new(format!("Tunnel error: {err}"))
                                    .small()
                                    .color(DANGER),
                            );
                        } else if srv.use_tunnel {
                            mono_spinner(ui, 13.0);
                            ui.label(
                                egui::RichText::new("Connecting tunnel...")
                                    .small()
                                    .color(text_muted()),
                            );
                        }
                    });
                }
                if !srv.eula_accepted {
                    ui.add_space(8.0);
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        tr(lang, "srv.eula"),
                        (card_w - 28.0).max(160.0),
                    );
                }
            });

            ui.add_space(12.0);

            ui_flow_row(ui, 30.0, |ui| {
                for (tab, ic, lbl) in [
                    (ServerSubTab::Console, Icon::Terminal, tr(lang, "nav.console")),
                    (ServerSubTab::Content, Icon::Layers, tr(lang, "srv.tab_content")),
                    (ServerSubTab::Settings, Icon::Sliders, tr(lang, "inst.settings")),
                ] {
                    let is_act = self.server_tab == tab;
                    if ui_pill_tab_icon(
                        ui,
                        ("srv_subtab", lbl),
                        Some(ic),
                        lbl,
                        is_act,
                    )
                    .clicked()
                        && !is_act
                    {
                        self.server_tab = tab;
                        self.mark_subtab_switch(ctx);
                    }
                }
            });
            ui.add_space(12.0);
            let subtab_t = self.subtab_switch_time;
            Self::apply_subtab_transition(ui, subtab_t, |ui| {

            if self.server_tab == ServerSubTab::Content {
                let supports_plugins = server_supports_plugins_live(&dir, srv.software);
                let supports_mods = srv.software.supports_mods();
                if supports_plugins && !supports_mods {
                    self.server_content_is_plugins = true;
                } else if !supports_plugins && supports_mods {
                    self.server_content_is_plugins = false;
                }

                if !supports_plugins && !supports_mods {
                    ui_alert_banner(
                        ui,
                        AlertKind::Info,
                        "Vanilla servers do not support plugins or mods. Use Paper, Purpur, or Folia for plugins, Fabric/Quilt/Forge/NeoForge for mods, or Arclight for both.",
                        card_w,
                    );
                    ui.add_space(6.0);
                }

                let folder_name = if self.server_content_is_plugins {
                    "plugins"
                } else {
                    "mods"
                };
                let content_dir = dir.join(folder_name);
                let plugins_count = list_content_items(&dir.join("plugins")).len();
                let mods_count = list_content_items(&dir.join("mods")).len();
                let items = list_content_items(&content_dir);
                let total_count = items.len();
                let enabled_count = items.iter().filter(|i| i.enabled).count();
                let disabled_count = total_count.saturating_sub(enabled_count);

                // Folder switcher (identical to Instance -> Content)
                ui_wrap_row(ui, 30.0, |ui| {
                    if supports_plugins {
                        let lbl = format!("{} ({plugins_count})", tr(lang, "mr.plugins"));
                        if ui_pill_tab_icon(
                            ui,
                            "srv_cnt_plugins",
                            Some(Icon::Server),
                            &lbl,
                            self.server_content_is_plugins,
                        )
                        .clicked()
                            && !self.server_content_is_plugins
                        {
                            self.server_content_is_plugins = true;
                            self.mark_subtab_switch(ctx);
                        }
                    }
                    if supports_mods {
                        let lbl = format!("{} ({mods_count})", tr(lang, "content.mods"));
                        if ui_pill_tab_icon(
                            ui,
                            "srv_cnt_mods",
                            Some(Icon::Layers),
                            &lbl,
                            !self.server_content_is_plugins,
                        )
                        .clicked()
                            && self.server_content_is_plugins
                        {
                            self.server_content_is_plugins = false;
                            self.mark_subtab_switch(ctx);
                        }
                    }
                    let can_browse = (self.server_content_is_plugins && supports_plugins)
                        || (!self.server_content_is_plugins && supports_mods);
                    if can_browse
                        && ui_button_icon(
                            ui,
                            "srv_browse_modrinth",
                            Some(Icon::Compass),
                            tr(lang, "btn.browse_modrinth"),
                            ButtonVariant::Primary,
                            true,
                            28.0,
                        )
                        .clicked()
                    {
                        self.discover_target_server_id = Some(srv.id.clone());
                        self.mr_type = if self.server_content_is_plugins {
                            ModrinthProjectType::Plugin
                        } else {
                            ModrinthProjectType::Mod
                        };
                        self.trigger_modrinth_search(false);
                        self.switch_page(ctx, NavPage::Discover);
                    }
                    if ui_button_icon(
                        ui,
                        "srv_cnt_open_folder",
                        Some(Icon::Folder),
                        tr(lang, "btn.open_folder"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        let _ = std::fs::create_dir_all(&content_dir);
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        open_in_file_manager(&content_dir);
                    }
                });

                ui.add_space(6.0);
                let avail_w = ui.available_width();
                ui_wrap_row(ui, 30.0, |ui| {
                    ui_search_input(
                        ui,
                        "srv_content_search_input",
                        &mut self.content_search,
                        tr(lang, "content.filter"),
                        (avail_w * 0.36).clamp(115.0, 230.0),
                    );
                    if ui_pill_tab(
                        ui,
                        "srv_state_all",
                        &format!("{} ({total_count})", tr(lang, "lib.all")),
                        self.content_state_filter == ContentStateFilter::All,
                    )
                    .clicked()
                        && self.content_state_filter != ContentStateFilter::All
                    {
                        self.content_state_filter = ContentStateFilter::All;
                        self.mark_subtab_switch(ctx);
                    }
                    if ui_pill_tab(
                        ui,
                        "srv_state_on",
                        &format!("{} ({enabled_count})", tr(lang, "content.enabled")),
                        self.content_state_filter == ContentStateFilter::Enabled,
                    )
                    .clicked()
                        && self.content_state_filter != ContentStateFilter::Enabled
                    {
                        self.content_state_filter = ContentStateFilter::Enabled;
                        self.mark_subtab_switch(ctx);
                    }
                    if ui_pill_tab(
                        ui,
                        "srv_state_off",
                        &format!("{} ({disabled_count})", tr(lang, "content.disabled")),
                        self.content_state_filter == ContentStateFilter::Disabled,
                    )
                    .clicked()
                        && self.content_state_filter != ContentStateFilter::Disabled
                    {
                        self.content_state_filter = ContentStateFilter::Disabled;
                        self.mark_subtab_switch(ctx);
                    }
                    if disabled_count > 0
                        && ui_button(
                            ui,
                            "srv_bulk_enable_btn",
                            tr(lang, "btn.enable_all"),
                            ButtonVariant::Secondary,
                            true,
                            30.0,
                        )
                        .clicked()
                    {
                        if let Ok(n) = set_all_content_enabled(&content_dir, true) {
                            let pos = anim::pointer_or_center(ctx);
                            anim::emit_burst(ctx, pos, accent(), 8);
                            anim::emit_floating_badge(ctx, pos, Some(Icon::Check), format!("+{n}"), accent());
                            self.show_toast(
                                ctx,
                                format!("{}: {n}", tr(lang, "toast.enabled_count")),
                            );
                        }
                    }
                    if enabled_count > 0
                        && ui_button(
                            ui,
                            "srv_bulk_disable_btn",
                            tr(lang, "btn.disable_all"),
                            ButtonVariant::Subtle,
                            true,
                            30.0,
                        )
                        .clicked()
                    {
                        if let Ok(n) = set_all_content_enabled(&content_dir, false) {
                            let pos = anim::pointer_or_center(ctx);
                            anim::emit_ring(ctx, pos, text_secondary(), 30.0);
                            anim::emit_floating_badge(ctx, pos, None, format!("-{n}"), text_secondary());
                            self.show_toast(
                                ctx,
                                format!("{}: {n}", tr(lang, "toast.disabled_count")),
                            );
                        }
                    }
                });

                ui.add_space(6.0);
                let query = self.content_search.trim().to_lowercase();
                let filtered: Vec<_> = items
                    .into_iter()
                    .filter(|it| {
                        let state_ok = match self.content_state_filter {
                            ContentStateFilter::All => true,
                            ContentStateFilter::Enabled => it.enabled,
                            ContentStateFilter::Disabled => !it.enabled,
                        };
                        let q_ok =
                            query.is_empty() || it.base_name.to_lowercase().contains(&query);
                        state_ok && q_ok
                    })
                    .collect();

                if filtered.is_empty() {
                    card_frame(false, 0.0).show(ui, |ui| {
                        ui.set_min_width(card_w);
                        ui.set_max_width(card_w);
                        ui.label(
                            egui::RichText::new(format!(
                                "No files in {folder_name}/ yet. Click {} above to install.",
                                tr(lang, "btn.browse_modrinth")
                            ))
                            .small()
                            .color(text_muted()),
                        );
                    });
                } else {
                    if self.server_index_backfilled.insert(srv.id.clone()) {
                        let paths = self.paths.clone();
                        let srv_bg = srv.clone();
                        thread::spawn(move || {
                            backfill_server_installed_index(&paths, &srv_bg);
                        });
                    }
                    let server_index = load_server_installed_index(&self.paths, &srv);
                    let indexed_by_file: HashMap<&str, &InstalledProject> = server_index
                        .values()
                        .filter(|p| p.folder == folder_name)
                        .map(|p| (p.filename.as_str(), p))
                        .collect();
                    let mut icons_to_request: Vec<String> = Vec::new();
                    let card_inner_w = (card_w - 24.0).max(160.0);
                    for item in filtered {
                        let info = indexed_by_file.get(item.base_name.as_str()).copied();
                        let display_title = info
                            .map(|i| i.title.clone())
                            .unwrap_or_else(|| item.base_name.clone());
                        let icon_url_opt = info.and_then(|i| i.icon_url.as_deref());
                        if let Some(url) = icon_url_opt {
                            if !self.mr_icons.contains_key(url) {
                                icons_to_request.push(url.to_string());
                            }
                        }

                        let card_id = ui.make_persistent_id((
                            "srv_mod_card_hover",
                            &srv.id,
                            folder_name,
                            &item.base_name,
                        ));
                        animated_card(ui, card_id, false, |ui| {
                            ui.set_min_width(card_inner_w);
                            ui.set_max_width(card_inner_w);
                            ui_toolbar_row(ui, 36.0, |ui| {
                                let toggle_id =
                                    format!("{}_{}_{}", srv.id, folder_name, item.base_name);
                                if mod_toggle_switch(ui, &toggle_id, item.enabled).clicked() {
                                    if let Ok(now_on) = toggle_content_file(&content_dir, &item.disk_name) {
                                        let pos = anim::pointer_or_center(ctx);
                                        let col = if now_on { accent() } else { text_secondary() };
                                        anim::emit_ring(ctx, pos, col, 26.0);
                                        anim::emit_floating_badge(
                                            ctx,
                                            pos,
                                            if now_on { Some(Icon::Check) } else { None },
                                            if now_on { "" } else { "-" },
                                            col,
                                        );
                                    }
                                }
                                let icon_size = Vec2::splat(30.0);
                                let tex_opt = icon_url_opt
                                    .and_then(|u| self.mr_icons.get(u))
                                    .and_then(|o| o.as_ref());
                                let is_icon_loading = icon_url_opt
                                    .is_some_and(|u| self.mr_icons_pending.contains(u));
                                if let Some(tex) = tex_opt {
                                    ui.add(
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(icon_size)
                                            .rounding(Rounding::same(6.5)),
                                    );
                                } else {
                                    let (rect, _) =
                                        ui.allocate_exact_size(icon_size, Sense::hover());
                                    draw_icon_placeholder(
                                        ui,
                                        rect,
                                        6.5,
                                        &display_title,
                                        is_icon_loading,
                                    );
                                }

                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(&display_title)
                                            .strong()
                                            .color(if item.enabled {
                                                text_primary()
                                            } else {
                                                text_muted()
                                            }),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} KB",
                                            (item.size_bytes / 1024).max(1)
                                        ))
                                        .small()
                                        .color(text_muted()),
                                    );
                                });

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        let compact_row = card_inner_w < 380.0;
                                        let del_clicked = if compact_row {
                                            ui_icon_button(
                                                ui,
                                                ("srv_cnt_del", &item.disk_name),
                                                Icon::Trash,
                                                tr(lang, "btn.remove"),
                                                ButtonVariant::Danger,
                                                true,
                                                28.0,
                                            )
                                            .clicked()
                                        } else {
                                            ui_button_icon(
                                                ui,
                                                ("srv_cnt_del", &item.disk_name),
                                                Some(Icon::Trash),
                                                tr(lang, "btn.remove"),
                                                ButtonVariant::Danger,
                                                true,
                                                28.0,
                                            )
                                            .clicked()
                                        };
                                        if del_clicked {
                                            match std::fs::remove_file(
                                                content_dir.join(&item.disk_name),
                                            ) {
                                                Ok(()) => {
                                                    anim::emit_burst(ctx, anim::pointer_or_center(ctx), DANGER, 8);
                                                    self.show_toast(
                                                        ctx,
                                                        format!(
                                                            "{}: {display_title}",
                                                            tr(lang, "toast.removed")
                                                        ),
                                                    );
                                                }
                                                Err(e) => {
                                                    self.show_toast(
                                                        ctx,
                                                        format!("Failed to remove {display_title}: {e}"),
                                                    );
                                                }
                                            }
                                        }
                                        if !item.enabled {
                                            badge_sized(
                                                ui,
                                                tr(lang, "content.disabled"),
                                                false,
                                                28.0,
                                                Some(112.0),
                                            );
                                        } else if let Some(inf) = info {
                                            badge_sized(
                                                ui,
                                                &inf.version_number,
                                                true,
                                                28.0,
                                                Some(112.0),
                                            )
                                            .on_hover_text(&inf.version_number);
                                        }
                                    },
                                );
                            });
                        });
                        ui.add_space(4.0);
                    }
                    for url in icons_to_request {
                        self.request_icon(&url);
                    }
                }
                return;
            }

            if self.server_tab == ServerSubTab::Settings {
                self.render_server_settings_tab(ui, ctx, card_w, &srv, &dir, is_this_running);
                return;
            }

            // Live Server Console card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                ui_toolbar_row(ui, 24.0, |ui| {
                    section_header(ui, tr(lang, "nav.console"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui_icon_button(
                            ui,
                            "srv_clear_logs",
                            Icon::Trash,
                            "Clear console",
                            ButtonVariant::Subtle,
                            !self.server_logs.is_empty(),
                            26.0,
                        )
                        .clicked()
                        {
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), text_secondary(), 26.0);
                            self.server_logs.clear();
                        }
                    });
                });
                ui.add_space(8.0);

                egui::Frame::none()
                    .fill(bg_input())
                    .stroke(Stroke::new(1.0_f32, border_subtle()))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("srv_console_scroll")
                            .max_height(260.0)
                            .min_scrolled_height(180.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_min_width((card_w - 44.0).max(120.0));
                                if self.server_logs.is_empty() {
                                    ui.label(
                                        egui::RichText::new("Server console output will appear here.")
                                            .small()
                                            .color(text_muted()),
                                    );
                                } else {
                                    for line in &self.server_logs {
                                        let col = if line.starts_with("[ERR]")
                                            || line.contains("ERROR")
                                        {
                                            DANGER
                                        } else if line.contains("WARN") {
                                            WARNING
                                        } else {
                                            text_secondary()
                                        };
                                        ui.label(
                                            egui::RichText::new(line)
                                                .monospace()
                                                .size(11.5)
                                                .color(col),
                                        );
                                    }
                                }
                            });
                    });

                ui.add_space(8.0);
                let send_w = 76.0;
                let input_w = (card_w - send_w - 38.0).max(120.0);
                ui_toolbar_row(ui, 30.0, |ui| {
                    let resp = ui_text_input(
                        ui,
                        "srv_cmd_input",
                        &mut self.server_cmd_input,
                        tr(lang, "srv.console_hint"),
                        input_w,
                        false,
                    );
                    let enter_send =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let send_clicked = ui_button(
                        ui,
                        "srv_send_cmd_btn",
                        "Send",
                        ButtonVariant::Primary,
                        is_this_running && !self.server_cmd_input.trim().is_empty(),
                        28.0,
                    )
                    .clicked();
                    if (enter_send || send_clicked) && is_this_running {
                        let cmd = self.server_cmd_input.trim().to_string();
                        if !cmd.is_empty() {
                            if let Some(handle) = &self.server_handle {
                                match send_server_command(handle, &cmd) {
                                    Ok(()) => {
                                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                                        self.server_logs.push_back(format!("> {cmd}"));
                                        self.server_cmd_input.clear();
                                    }
                                    Err(e) => self.show_toast(ctx, e),
                                }
                            }
                        }
                    }
                });
            });
            });
        });
    }

    pub(super) fn render_server_settings_tab(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        card_w: f32,
        srv: &DedicatedServer,
        dir: &std::path::Path,
        is_this_running: bool,
    ) {
        let lang = self.settings.language;
        if !dir.join("server.properties").is_file() {
            let _ = write_eula_and_properties(dir, srv);
        }
        let need_reload = self
            .server_props_cache
            .as_ref()
            .map(|(id, _)| id != &srv.id)
            .unwrap_or(true);
        if need_reload {
            self.server_props_cache =
                Some((srv.id.clone(), read_server_properties_entries(dir)));
        }
        if self
            .server_port_buf
            .as_ref()
            .map(|(id, _)| id != &srv.id)
            .unwrap_or(true)
        {
            self.server_port_buf = Some((srv.id.clone(), srv.port.to_string()));
        }

        let java_installs = self.java_installs.clone();
        let java_scanning = self.java_scanning;
        let server_busy = self.server_busy;
        let lan_ip = self
            .local_lan_ip
            .get_or_insert_with(detect_local_lan_ip)
            .clone();
        let srv_icon_tex = self.ensure_server_icon_loaded(ctx, &srv.id, dir);

        let mut cache = self.server_props_cache.take();
        let mut port_buf_state = self.server_port_buf.take();
        let entries = &mut cache
            .as_mut()
            .expect("server_props_cache initialized")
            .1;
        let port_str = &mut port_buf_state
            .as_mut()
            .expect("server_port_buf initialized")
            .1;

        let mut edited = self.servers[self.selected_server_idx].clone();
        let mut model_changed = false;
        let mut props_changed = false;
        let mut sync_model_to_props = false;
        let mut remove_prop_key: Option<String> = None;
        let mut remove_icon = false;
        let mut toast_msg: Option<String> = None;
        let mut reload_props_from_disk = false;
        let mut trigger_rescan_java = false;
        let mut trigger_reinstall = false;
        let mut trigger_delete = false;

        fn prop_get(entries: &[(String, String)], key: &str, default: &str) -> String {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| default.to_string())
        }
        fn prop_get_bool(entries: &[(String, String)], key: &str, default: bool) -> bool {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        }
        fn prop_get_u32(entries: &[(String, String)], key: &str, default: u32) -> u32 {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .and_then(|(_, v)| v.trim().parse::<u32>().ok())
                .unwrap_or(default)
        }
        fn prop_set(entries: &mut Vec<(String, String)>, key: &str, val: String) {
            if let Some(existing) = entries.iter_mut().find(|(k, _)| k == key) {
                existing.1 = val;
            } else {
                entries.push((key.to_string(), val));
            }
        }

        if is_this_running {
            ui_alert_banner(ui, AlertKind::Info, tr(lang, "srv.restart_notice"), card_w);
            ui.add_space(10.0);
        }

        let req_java = required_java_major_for_mc(&edited.mc_version);

        // Card 1: General & Appearance
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            ui_toolbar_row(ui, 26.0, |ui| {
                section_header(ui, tr(lang, "srv.sec_general"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    badge(ui, &format!("MC {}", edited.mc_version), false);
                    badge(ui, edited.software.label(), true);
                });
            });
            ui.add_space(8.0);

            // Live Minecraft Multiplayer list preview
            egui::Frame::none()
                .fill(bg_input())
                .stroke(Stroke::new(1.0_f32, border_subtle()))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    let preview_inner_w = (card_w - 22.0).max(140.0);
                    ui.set_min_width(preview_inner_w);
                    ui.set_max_width(preview_inner_w);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let icon_id = ui.make_persistent_id(("srv_preview_icon", &edited.id));
                        let (ic_rect, ic_resp) =
                            ui.allocate_exact_size(Vec2::splat(56.0), Sense::click());
                        let hov_t = ui.ctx().animate_bool_with_time(
                            icon_id.with("hov"),
                            ic_resp.hovered(),
                            0.12,
                        );
                        let prs_t = ui.ctx().animate_bool_with_time(
                            icon_id.with("prs"),
                            ic_resp.is_pointer_button_down_on(),
                            0.07,
                        );
                        if (hov_t > 0.01 && hov_t < 0.99) || (prs_t > 0.01 && prs_t < 0.99) {
                            ui.ctx().request_repaint();
                        }
                        let draw_rect = ic_rect.shrink(prs_t * 1.2);
                        if let Some(tex) = &srv_icon_tex {
                            ui.painter().rect_filled(draw_rect, Rounding::same(7.0), bg_card());
                            ui.painter().image(
                                tex.id(),
                                draw_rect.shrink(1.0),
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            ui.painter()
                                .rect_filled(draw_rect, Rounding::same(7.0), bg_elevated());
                            crate::icons::draw_icon(
                                ui.painter(),
                                draw_rect.shrink(14.0),
                                Icon::Server,
                                lerp_color(text_secondary(), accent(), hov_t),
                            );
                        }
                        let border_col = lerp_color(border_subtle(), accent(), hov_t);
                        ui.painter().rect_stroke(
                            draw_rect,
                            Rounding::same(7.0),
                            Stroke::new(1.0 + hov_t * 0.6, border_col),
                        );
                        if let Some((ct, origin)) = track_click_anim(
                            ui.ctx(),
                            icon_id,
                            draw_rect,
                            ic_resp.clicked(),
                            ic_resp.interact_pointer_pos(),
                            0.36,
                        ) {
                            paint_click_ripple(
                                ui.painter(),
                                draw_rect,
                                Rounding::same(7.0),
                                origin,
                                ct,
                                accent(),
                                true,
                            );
                        }
                        if ic_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if ic_resp.on_hover_text(tr(lang, "inst.choose_icon")).clicked() {
                            self.request_file_pick(FilePickPurpose::ServerIcon);
                        }

                        let text_w = (preview_inner_w - 72.0).max(80.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(text_w);
                            ui_toolbar_row(ui, 22.0, |ui| {
                                let title_txt = if edited.name.trim().is_empty() {
                                    "Minecraft Server"
                                } else {
                                    edited.name.as_str()
                                };
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        badge(ui, &format!("0/{}", edited.max_players), false);
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(title_txt)
                                                            .strong()
                                                            .size(14.0)
                                                            .color(text_primary()),
                                                    )
                                                    .truncate(),
                                                );
                                            },
                                        );
                                    },
                                );
                            });
                            ui.add_space(3.0);

                            let mut job = egui::text::LayoutJob::default();
                            job.wrap.max_width = text_w;
                            let segs = parse_mc_motd_segments(edited.motd.trim());
                            if segs.is_empty() {
                                job.append(
                                    "A Minecraft Server",
                                    0.0,
                                    egui::TextFormat {
                                        font_id: egui::FontId::proportional(12.5),
                                        color: text_muted(),
                                        ..Default::default()
                                    },
                                );
                            } else {
                                let light = theme::is_light_theme();
                                for (txt, rgb_opt, bold) in segs {
                                    let col = match rgb_opt {
                                        Some([255, 255, 255]) if light => text_primary(),
                                        Some([r, g, b]) => Color32::from_rgb(r, g, b),
                                        None if bold => text_primary(),
                                        None => text_secondary(),
                                    };
                                    job.append(
                                        &txt,
                                        0.0,
                                        egui::TextFormat {
                                            font_id: egui::FontId::proportional(if bold {
                                                13.0
                                            } else {
                                                12.5
                                            }),
                                            color: col,
                                            ..Default::default()
                                        },
                                    );
                                }
                            }
                            ui.add(egui::Label::new(job).wrap());
                        });
                    });
                });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(tr(lang, "inst.name"))
                    .small()
                    .color(text_secondary()),
            );
            if ui_text_input(
                ui,
                ("srv_name_edit", &edited.id),
                &mut edited.name,
                "Survival Server",
                (card_w - 16.0).clamp(160.0, 380.0),
                false,
            )
            .changed()
            {
                model_changed = true;
            }

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tr(lang, "srv.motd"))
                    .small()
                    .color(text_secondary()),
            );
            if ui_text_input(
                ui,
                ("srv_motd_edit", &edited.id),
                &mut edited.motd,
                "A Sunset Launcher Server",
                (card_w - 16.0).clamp(160.0, 520.0),
                false,
            )
            .changed()
            {
                model_changed = true;
                sync_model_to_props = true;
            }

            ui.add_space(8.0);
            let has_icon_file = dir.join("server-icon.png").is_file();
            ui_flow_row(ui, 28.0, |ui| {
                if ui_button_icon(
                    ui,
                    ("srv_pick_icon_btn", &edited.id),
                    Some(Icon::Folder),
                    tr(lang, "inst.choose_icon"),
                    ButtonVariant::Secondary,
                    !self.file_picker_open,
                    28.0,
                )
                .clicked()
                {
                    self.request_file_pick(FilePickPurpose::ServerIcon);
                }
                if has_icon_file
                    && ui_button_icon(
                        ui,
                        ("srv_del_icon_btn", &edited.id),
                        Some(Icon::Trash),
                        tr(lang, "action.delete"),
                        ButtonVariant::Subtle,
                        true,
                        28.0,
                    )
                    .clicked()
                {
                    remove_icon = true;
                }
            });
        });
        ui.add_space(12.0);

        // Card 2: Gameplay & World
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            section_header(ui, tr(lang, "srv.sec_gameplay"));
            ui.add_space(8.0);

            let cur_gm = prop_get(entries, "gamemode", "survival").to_lowercase();
            ui.label(
                egui::RichText::new(tr(lang, "srv.gamemode"))
                    .small()
                    .color(text_secondary()),
            );
            ui.add_space(4.0);
            ui_flow_row(ui, 28.0, |ui| {
                for (val, disp) in [
                    ("survival", "Survival"),
                    ("creative", "Creative"),
                    ("adventure", "Adventure"),
                    ("spectator", "Spectator"),
                ] {
                    let active = cur_gm == val;
                    if ui_pill_tab(ui, ("srv_gm_pill", &edited.id, val), disp, active).clicked()
                        && !active
                    {
                        prop_set(entries, "gamemode", val.to_string());
                        props_changed = true;
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                    }
                }
            });

            ui.add_space(8.0);
            let cur_diff = prop_get(entries, "difficulty", "easy").to_lowercase();
            ui.label(
                egui::RichText::new(tr(lang, "srv.difficulty"))
                    .small()
                    .color(text_secondary()),
            );
            ui.add_space(4.0);
            ui_flow_row(ui, 28.0, |ui| {
                for (val, disp) in [
                    ("peaceful", "Peaceful"),
                    ("easy", "Easy"),
                    ("normal", "Normal"),
                    ("hard", "Hard"),
                ] {
                    let active = cur_diff == val;
                    if ui_pill_tab(ui, ("srv_diff_pill", &edited.id, val), disp, active).clicked()
                        && !active
                    {
                        prop_set(entries, "difficulty", val.to_string());
                        props_changed = true;
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                    }
                }
            });

            ui.add_space(10.0);
            let label_w = (card_w * 0.36).clamp(96.0, 185.0);
            let slider_w = (card_w - label_w - 92.0).clamp(70.0, 320.0);

            let mut max_p = edited.max_players.max(1);
            let max_p_cap = max_p.max(200);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "srv.max_players"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                if ui_int_slider(ui, &mut max_p, 1..=max_p_cap, 1.0, "", slider_w).changed() {
                    edited.max_players = max_p.max(1);
                    model_changed = true;
                    sync_model_to_props = true;
                }
            });
            ui.add_space(4.0);
            ui_flow_row(ui, 26.0, |ui| {
                for preset in [5_u32, 10, 20, 50, 100] {
                    if ui_pill_tab(
                        ui,
                        ("srv_maxp_preset", &edited.id, preset),
                        &preset.to_string(),
                        edited.max_players == preset,
                    )
                    .clicked()
                    {
                        edited.max_players = preset;
                        model_changed = true;
                        sync_model_to_props = true;
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                    }
                }
            });

            ui.add_space(8.0);
            let mut vd = prop_get_u32(entries, "view-distance", 10).max(2);
            let vd_cap = vd.max(32);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "srv.view_distance"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                if ui_int_slider(ui, &mut vd, 2..=vd_cap, 1.0, " ch", slider_w).changed() {
                    prop_set(entries, "view-distance", vd.to_string());
                    props_changed = true;
                }
            });

            let mut sd = prop_get_u32(entries, "simulation-distance", 10).max(2);
            let sd_cap = sd.max(32);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "srv.sim_distance"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                if ui_int_slider(ui, &mut sd, 2..=sd_cap, 1.0, " ch", slider_w).changed() {
                    prop_set(entries, "simulation-distance", sd.to_string());
                    props_changed = true;
                }
            });

            let mut sp = prop_get_u32(entries, "spawn-protection", 16);
            let sp_cap = sp.max(64);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "srv.spawn_protection"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                if ui_int_slider(ui, &mut sp, 0..=sp_cap, 1.0, " blk", slider_w).changed() {
                    prop_set(entries, "spawn-protection", sp.to_string());
                    props_changed = true;
                }
            });

            ui.add_space(10.0);
            let world_cols: usize = if card_w >= 580.0 { 3 } else { 1 };
            let world_cell_w = ((card_w - 4.0 - 12.0 * (world_cols.saturating_sub(1) as f32))
                / world_cols as f32)
                .clamp(150.0, card_w);
            let mut lvl_name = prop_get(entries, "level-name", "world");
            let mut lvl_seed = prop_get(entries, "level-seed", "");
            let mut lvl_type = prop_get(entries, "level-type", "minecraft:normal");

            egui::Grid::new(("srv_world_grid", &edited.id))
                .num_columns(world_cols)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width(world_cell_w);
                        ui.label(
                            egui::RichText::new(tr(lang, "srv.level_name"))
                                .small()
                                .color(text_secondary()),
                        );
                        if ui_text_input(
                            ui,
                            ("srv_lvl_name", &edited.id),
                            &mut lvl_name,
                            "world",
                            (world_cell_w - 4.0).max(110.0),
                            false,
                        )
                        .changed()
                        {
                            prop_set(entries, "level-name", lvl_name.clone());
                            props_changed = true;
                        }
                    });
                    if world_cols == 1 {
                        ui.end_row();
                    }

                    ui.vertical(|ui| {
                        ui.set_max_width(world_cell_w);
                        ui.label(
                            egui::RichText::new(tr(lang, "srv.level_seed"))
                                .small()
                                .color(text_secondary()),
                        );
                        if ui_text_input(
                            ui,
                            ("srv_lvl_seed", &edited.id),
                            &mut lvl_seed,
                            "Random",
                            (world_cell_w - 4.0).max(110.0),
                            false,
                        )
                        .changed()
                        {
                            prop_set(entries, "level-seed", lvl_seed.clone());
                            props_changed = true;
                        }
                    });
                    if world_cols == 1 {
                        ui.end_row();
                    }

                    ui.vertical(|ui| {
                        ui.set_max_width(world_cell_w);
                        ui.label(
                            egui::RichText::new(tr(lang, "srv.level_type"))
                                .small()
                                .color(text_secondary()),
                        );
                        let type_options = [
                            ("minecraft:normal", "Normal"),
                            ("minecraft:flat", "Superflat"),
                            ("minecraft:large_biomes", "Large Biomes"),
                            ("minecraft:amplified", "Amplified"),
                            ("minecraft:single_biome_surface", "Single Biome"),
                        ];
                        let selected_lbl = type_options
                            .iter()
                            .find(|(id, _)| *id == lvl_type)
                            .map(|(_, lbl)| *lbl)
                            .unwrap_or(lvl_type.as_str())
                            .to_string();
                        let prev = lvl_type.clone();
                        egui::ComboBox::from_id_salt(("srv_lvl_type", &edited.id))
                            .selected_text(selected_lbl)
                            .width((world_cell_w - 4.0).max(120.0))
                            .show_ui(ui, |ui| {
                                for (opt_id, opt_lbl) in type_options {
                                    ui.selectable_value(
                                        &mut lvl_type,
                                        opt_id.to_string(),
                                        opt_lbl,
                                    );
                                }
                            });
                        if lvl_type != prev {
                            prop_set(entries, "level-type", lvl_type.clone());
                            props_changed = true;
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                        }
                    });
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(tr(lang, "srv.rules"))
                    .small()
                    .color(text_secondary()),
            );
            ui.add_space(6.0);

            let rules: [(&str, &str, bool); 11] = [
                ("online-mode", tr(lang, "srv.online_mode"), edited.online_mode),
                ("pvp", tr(lang, "srv.rule_pvp"), prop_get_bool(entries, "pvp", true)),
                (
                    "white-list",
                    tr(lang, "srv.rule_whitelist"),
                    prop_get_bool(entries, "white-list", false),
                ),
                (
                    "enforce-whitelist",
                    tr(lang, "srv.rule_enforce_wl"),
                    prop_get_bool(entries, "enforce-whitelist", false),
                ),
                (
                    "hardcore",
                    tr(lang, "srv.rule_hardcore"),
                    prop_get_bool(entries, "hardcore", false),
                ),
                (
                    "allow-flight",
                    tr(lang, "srv.rule_flight"),
                    prop_get_bool(entries, "allow-flight", false),
                ),
                (
                    "allow-nether",
                    tr(lang, "srv.rule_nether"),
                    prop_get_bool(entries, "allow-nether", true),
                ),
                (
                    "enable-command-block",
                    tr(lang, "srv.rule_cmd_block"),
                    prop_get_bool(entries, "enable-command-block", false),
                ),
                (
                    "spawn-monsters",
                    tr(lang, "srv.rule_monsters"),
                    prop_get_bool(entries, "spawn-monsters", true),
                ),
                (
                    "spawn-animals",
                    tr(lang, "srv.rule_animals"),
                    prop_get_bool(entries, "spawn-animals", true),
                ),
                (
                    "spawn-npcs",
                    tr(lang, "srv.rule_npcs"),
                    prop_get_bool(entries, "spawn-npcs", true),
                ),
            ];
            let rule_cols: usize = if card_w >= 540.0 { 2 } else { 1 };
            let rule_cell_w = ((card_w - 4.0 - 16.0 * (rule_cols.saturating_sub(1) as f32))
                / rule_cols as f32)
                .clamp(160.0, card_w);
            egui::Grid::new(("srv_rules_grid", &edited.id))
                .num_columns(rule_cols)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    for (idx, (key, rule_lbl, cur_val)) in rules.iter().enumerate() {
                        ui.allocate_ui_with_layout(
                            Vec2::new(rule_cell_w, 26.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(rule_cell_w);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let sw_id = format!("srv_rule_{}_{key}", edited.id);
                                        if mod_toggle_switch(ui, &sw_id, *cur_val).clicked() {
                                            let next_val = !*cur_val;
                                            if *key == "online-mode" {
                                                edited.online_mode = next_val;
                                                model_changed = true;
                                                sync_model_to_props = true;
                                            } else {
                                                prop_set(
                                                    entries,
                                                    key,
                                                    if next_val { "true" } else { "false" }
                                                        .to_string(),
                                                );
                                                props_changed = true;
                                            }
                                        }
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(*rule_lbl)
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
                        if (idx + 1) % rule_cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
        ui.add_space(12.0);

        // Card 3: Network & Access
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            let local_addr = format!("127.0.0.1:{}", edited.port);
            ui_toolbar_row(ui, 28.0, |ui| {
                section_header(ui, tr(lang, "srv.sec_network"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_button_icon(
                        ui,
                        ("srv_net_copy_local", &edited.id),
                        Some(Icon::Copy),
                        &local_addr,
                        ButtonVariant::Secondary,
                        true,
                        26.0,
                    )
                    .clicked()
                    {
                        ctx.copy_text(local_addr.clone());
                        let pos = anim::pointer_or_center(ctx);
                        anim::emit_ring(ctx, pos, accent(), 28.0);
                        anim::emit_floating_badge(ctx, pos, Some(Icon::Check), "", accent());
                        toast_msg = Some(format!("Copied {local_addr}"));
                    }
                });
            });
            ui.add_space(8.0);

            ui_flow_row(ui, 28.0, |ui| {
                ui.label(
                    egui::RichText::new(tr(lang, "srv.port"))
                        .small()
                        .color(text_secondary()),
                );
                let port_resp = ui_text_input(
                    ui,
                    ("srv_port_edit", &edited.id),
                    port_str,
                    "25565",
                    96.0,
                    false,
                );
                let parsed_port = port_str.trim().parse::<u16>().ok().filter(|p| *p > 0);
                if port_resp.changed() {
                    if let Some(p) = parsed_port {
                        if edited.port != p {
                            edited.port = p;
                            model_changed = true;
                            sync_model_to_props = true;
                        }
                    }
                }
                if parsed_port.is_none() {
                    ui.label(
                        egui::RichText::new("1–65535")
                            .monospace()
                            .small()
                            .color(DANGER),
                    );
                    if port_resp.lost_focus() {
                        *port_str = edited.port.to_string();
                    }
                }
                if edited.port != 25565
                    && ui_button(
                        ui,
                        ("srv_port_reset", &edited.id),
                        "25565",
                        ButtonVariant::Subtle,
                        true,
                        26.0,
                    )
                    .clicked()
                {
                    edited.port = 25565;
                    *port_str = "25565".to_string();
                    model_changed = true;
                    sync_model_to_props = true;
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                }
                if let Some(ip) = &lan_ip {
                    let lan_addr = format!("{ip}:{}", edited.port);
                    ui.label(
                        egui::RichText::new(tr(lang, "srv.lan_address"))
                            .small()
                            .color(text_muted()),
                    );
                    ui.label(
                        egui::RichText::new(&lan_addr)
                            .monospace()
                            .small()
                            .color(text_primary()),
                    );
                    if ui_icon_button(
                        ui,
                        ("srv_copy_lan_btn", &edited.id),
                        Icon::Copy,
                        tr(lang, "btn.copy"),
                        ButtonVariant::Subtle,
                        true,
                        26.0,
                    )
                    .clicked()
                    {
                        ctx.copy_text(lan_addr.clone());
                        let pos = anim::pointer_or_center(ctx);
                        anim::emit_ring(ctx, pos, accent(), 26.0);
                        anim::emit_floating_badge(ctx, pos, Some(Icon::Check), "", accent());
                        toast_msg = Some(format!("Copied {lan_addr}"));
                    }
                }
            });

            ui.add_space(6.0);
            if ui_checkbox(
                ui,
                ("srv_upnp_chk", &edited.id),
                &mut edited.public_upnp,
                tr(lang, "srv.public_upnp"),
            ) {
                model_changed = true;
            }
            ui.add_space(4.0);
            if ui_checkbox(
                ui,
                ("srv_tunnel_chk", &edited.id),
                &mut edited.use_tunnel,
                tr(lang, "srv.tunnel"),
            ) {
                model_changed = true;
            }
            if edited.use_tunnel {
                ui.add_space(6.0);
                ui_alert_banner(
                    ui,
                    AlertKind::Warning,
                    tr(lang, "srv.tunnel_notice"),
                    (card_w - 8.0).max(160.0),
                );
            }
            ui.add_space(4.0);
            if ui_checkbox(
                ui,
                ("srv_eula_chk", &edited.id),
                &mut edited.eula_accepted,
                tr(lang, "srv.eula"),
            ) {
                model_changed = true;
            }
        });
        ui.add_space(12.0);

        // Card 4: Memory Allocation
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            ui_toolbar_row(ui, 24.0, |ui| {
                section_header(ui, tr(lang, "inst.sec_ram"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    badge(
                        ui,
                        &format!("-Xms{}M  -Xmx{}M", edited.min_ram_mb, edited.max_ram_mb),
                        true,
                    );
                });
            });
            ui.add_space(6.0);
            draw_ram_allocation_bar(
                ui,
                edited.min_ram_mb,
                edited.max_ram_mb,
                (card_w - 16.0).clamp(120.0, 520.0),
            );
            ui.add_space(8.0);

            let label_w = (card_w * 0.36).clamp(96.0, 185.0);
            let slider_w = (card_w - label_w - 92.0).clamp(70.0, 320.0);
            let max_ram_cap = edited.max_ram_mb.max(32768);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "inst.max_ram"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                if ui_ram_slider(ui, &mut edited.max_ram_mb, 512..=max_ram_cap, slider_w).changed()
                {
                    if edited.min_ram_mb > edited.max_ram_mb {
                        edited.min_ram_mb = edited.max_ram_mb;
                    }
                    model_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(label_w, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tr(lang, "inst.min_ram"))
                                    .small()
                                    .color(text_secondary()),
                            )
                            .truncate(),
                        );
                    },
                );
                let min_cap = edited.max_ram_mb.max(256);
                if ui_ram_slider(ui, &mut edited.min_ram_mb, 256..=min_cap, slider_w).changed() {
                    model_changed = true;
                }
            });

            ui.add_space(6.0);
            ui_flow_row(ui, 28.0, |ui| {
                for preset in [1024_u32, 2048, 4096, 6144, 8192, 12288, 16384] {
                    if ui_pill_tab(
                        ui,
                        ("srv_ram_preset", &edited.id, preset),
                        &format!("{} GB", preset / 1024),
                        edited.max_ram_mb == preset,
                    )
                    .clicked()
                    {
                        edited.max_ram_mb = preset;
                        edited.min_ram_mb = (preset / 2).clamp(512, 4096).min(preset);
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        model_changed = true;
                    }
                }
            });
            if edited.max_ram_mb >= 12288 {
                ui.add_space(6.0);
                ui_alert_banner(
                    ui,
                    AlertKind::Warning,
                    "Allocating 12+ GB RAM works best with Aikar's G1GC flags or Generational ZGC below.",
                    (card_w - 8.0).max(160.0),
                );
            }
        });
        ui.add_space(12.0);

        // Card 5: Java Runtime & JVM Arguments
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            ui_toolbar_row(ui, 26.0, |ui| {
                section_header(ui, tr(lang, "inst.sec_java"));
                badge(ui, &format!("Java {req_java}+"), false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if java_scanning {
                        mono_spinner(ui, 14.0);
                    } else if ui_button_icon(
                        ui,
                        ("srv_rescan_java_btn", &edited.id),
                        Some(Icon::Refresh),
                        tr(lang, "inst.rescan_java"),
                        ButtonVariant::Secondary,
                        true,
                        24.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        trigger_rescan_java = true;
                    }
                });
            });
            ui.add_space(6.0);

            ui.label(
                egui::RichText::new(tr(lang, "inst.detected_runtimes"))
                    .small()
                    .color(text_secondary()),
            );
            ui.add_space(4.0);
            ui_flow_row(ui, 28.0, |ui| {
                let is_auto = edited.java_path.trim().is_empty();
                if ui_pill_tab(
                    ui,
                    ("srv_java_auto", &edited.id),
                    tr(lang, "inst.auto"),
                    is_auto,
                )
                .clicked()
                {
                    edited.java_path.clear();
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                    model_changed = true;
                }
                for j in &java_installs {
                    let j_str = j.path.to_string_lossy().to_string();
                    let active = edited.java_path == j_str;
                    if ui_pill_tab(
                        ui,
                        ("srv_java_pill", &edited.id, j_str.as_str()),
                        &format!("Java {}", j.major),
                        active,
                    )
                    .on_hover_text(&j_str)
                    .clicked()
                    {
                        edited.java_path = j_str;
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                        model_changed = true;
                    }
                }
            });

            ui.add_space(6.0);
            let java_placeholder = if cfg!(windows) {
                r"Auto (or C:\path\to\java.exe)"
            } else {
                "Auto (or /usr/bin/java)"
            };
            ui_wrap_row(ui, 30.0, |ui| {
                if ui_text_input(
                    ui,
                    ("srv_java_path", &edited.id),
                    &mut edited.java_path,
                    java_placeholder,
                    (card_w - 56.0).clamp(150.0, 420.0),
                    false,
                )
                .changed()
                {
                    model_changed = true;
                }
                if ui_icon_button(
                    ui,
                    ("srv_java_browse_btn", &edited.id),
                    Icon::Folder,
                    tr(lang, "btn.folder"),
                    ButtonVariant::Secondary,
                    !self.file_picker_open,
                    28.0,
                )
                .clicked()
                {
                    self.request_file_pick(FilePickPurpose::ServerJavaBinary);
                }
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tr(lang, "inst.jvm_args"))
                    .small()
                    .color(text_secondary()),
            );
            if ui_text_input(
                ui,
                ("srv_jvm_args", &edited.id),
                &mut edited.jvm_args,
                "-XX:+UseG1GC ...",
                (card_w - 16.0).clamp(160.0, 460.0),
                false,
            )
            .changed()
            {
                model_changed = true;
            }

            ui.add_space(4.0);
            ui_flow_row(ui, 26.0, |ui| {
                let presets = [
                    (
                        "Aikar's Flags",
                        "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1",
                    ),
                    (
                        "G1GC",
                        "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=100",
                    ),
                    ("ZGC", "-XX:+UseZGC -XX:+ZGenerational"),
                ];
                for (lbl, flags) in presets {
                    if ui_button(
                        ui,
                        ("srv_jvm_preset", &edited.id, lbl),
                        lbl,
                        ButtonVariant::Subtle,
                        true,
                        26.0,
                    )
                    .on_hover_text(flags)
                    .clicked()
                    {
                        edited.jvm_args = flags.to_string();
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                        model_changed = true;
                    }
                }
                if !edited.jvm_args.trim().is_empty()
                    && ui_button(
                        ui,
                        ("srv_jvm_clear", &edited.id),
                        tr(lang, "btn.clear"),
                        ButtonVariant::Subtle,
                        true,
                        26.0,
                    )
                    .clicked()
                {
                    edited.jvm_args.clear();
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), text_secondary(), 24.0);
                    model_changed = true;
                }
            });
        });
        ui.add_space(12.0);

        // Card 6: Advanced server.properties Editor
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            let total_props = entries.len();
            let props_expanded =
                self.server_props_expanded || !self.server_props_filter.trim().is_empty();
            ui_toolbar_row(ui, 28.0, |ui| {
                section_header(ui, "server.properties");
                badge(ui, &total_props.to_string(), false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let toggle_lbl = tr(
                        lang,
                        if props_expanded {
                            "srv.hide_all_props"
                        } else {
                            "srv.show_all_props"
                        },
                    );
                    if ui_button(
                        ui,
                        ("srv_props_toggle_btn", &edited.id),
                        toggle_lbl,
                        ButtonVariant::Subtle,
                        true,
                        26.0,
                    )
                    .clicked()
                    {
                        self.server_props_expanded = !self.server_props_expanded;
                        if !self.server_props_expanded {
                            self.server_props_filter.clear();
                        }
                    }
                    if ui_button_icon(
                        ui,
                        ("srv_props_reload", &edited.id),
                        Some(Icon::Refresh),
                        tr(lang, "srv.reload"),
                        ButtonVariant::Secondary,
                        true,
                        26.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        reload_props_from_disk = true;
                    }
                    if ui_button_icon(
                        ui,
                        ("srv_props_open_file", &edited.id),
                        Some(Icon::External),
                        tr(lang, "srv.open_props"),
                        ButtonVariant::Secondary,
                        true,
                        26.0,
                    )
                    .clicked()
                    {
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 26.0);
                        open_in_file_manager(&dir.join("server.properties"));
                    }
                });
            });

            ui.add_space(6.0);
            ui_search_input(
                ui,
                "srv_props_search",
                &mut self.server_props_filter,
                tr(lang, "content.filter"),
                (card_w - 16.0).clamp(140.0, 320.0),
            );

            if props_expanded {
                ui.add_space(8.0);
                let q = self.server_props_filter.trim().to_lowercase();
                let val_w = (card_w * 0.40).clamp(110.0, 230.0);

                for (k, v) in entries.iter_mut() {
                    if !q.is_empty()
                        && !k.to_lowercase().contains(&q)
                        && !v.to_lowercase().contains(&q)
                    {
                        continue;
                    }
                    let mut row_changed = false;
                    let is_core = matches!(
                        k.as_str(),
                        "server-port" | "motd" | "max-players" | "online-mode"
                    );
                    ui_toolbar_row(ui, 28.0, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if !is_core
                                && ui_icon_button(
                                    ui,
                                    ("srv_prop_del", &edited.id, k.as_str()),
                                    Icon::Trash,
                                    tr(lang, "btn.remove"),
                                    ButtonVariant::Subtle,
                                    true,
                                    24.0,
                                )
                                .clicked()
                            {
                                remove_prop_key = Some(k.clone());
                            }
                            if v == "true" || v == "false" {
                                let on = v == "true";
                                let tid = format!("srv_prop_sw_{}_{}", edited.id, k);
                                if mod_toggle_switch(ui, &tid, on).clicked() {
                                    *v = if on { "false" } else { "true" }.to_string();
                                    row_changed = true;
                                }
                            } else if k == "difficulty" {
                                let mut cur = v.clone();
                                egui::ComboBox::from_id_salt(("srv_prop_diff", &edited.id))
                                    .selected_text(&cur)
                                    .width(val_w)
                                    .show_ui(ui, |ui| {
                                        for opt in ["peaceful", "easy", "normal", "hard"] {
                                            ui.selectable_value(&mut cur, opt.to_string(), opt);
                                        }
                                    });
                                if cur != *v {
                                    *v = cur;
                                    row_changed = true;
                                }
                            } else if k == "gamemode" {
                                let mut cur = v.clone();
                                egui::ComboBox::from_id_salt(("srv_prop_gm", &edited.id))
                                    .selected_text(&cur)
                                    .width(val_w)
                                    .show_ui(ui, |ui| {
                                        for opt in [
                                            "survival",
                                            "creative",
                                            "adventure",
                                            "spectator",
                                        ] {
                                            ui.selectable_value(&mut cur, opt.to_string(), opt);
                                        }
                                    });
                                if cur != *v {
                                    *v = cur;
                                    row_changed = true;
                                }
                            } else if k == "level-type" {
                                let mut cur = v.clone();
                                egui::ComboBox::from_id_salt(("srv_prop_lt", &edited.id))
                                    .selected_text(&cur)
                                    .width(val_w)
                                    .show_ui(ui, |ui| {
                                        for opt in [
                                            "minecraft:normal",
                                            "minecraft:flat",
                                            "minecraft:large_biomes",
                                            "minecraft:amplified",
                                            "minecraft:single_biome_surface",
                                        ] {
                                            ui.selectable_value(&mut cur, opt.to_string(), opt);
                                        }
                                    });
                                if cur != *v {
                                    *v = cur;
                                    row_changed = true;
                                }
                            } else if ui_text_input(
                                ui,
                                ("srv_prop_val", &edited.id, k.as_str()),
                                v,
                                "",
                                val_w,
                                false,
                            )
                            .changed()
                            {
                                row_changed = true;
                            }

                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(k.as_str())
                                            .monospace()
                                            .small()
                                            .color(text_primary()),
                                    )
                                    .truncate(),
                                );
                            });
                        });
                    });

                    if row_changed {
                        props_changed = true;
                        match k.as_str() {
                            "server-port" => {
                                if let Ok(p) = v.trim().parse::<u16>() {
                                    if p > 0 {
                                        edited.port = p;
                                        *port_str = p.to_string();
                                        model_changed = true;
                                    }
                                }
                            }
                            "motd" => {
                                edited.motd = v.clone();
                                model_changed = true;
                            }
                            "max-players" => {
                                if let Ok(mp) = v.trim().parse::<u32>() {
                                    edited.max_players = mp.clamp(1, 10000);
                                    model_changed = true;
                                }
                            }
                            "online-mode" => {
                                edited.online_mode = v == "true";
                                model_changed = true;
                            }
                            _ => {}
                        }
                    }
                }

                ui.add_space(8.0);
                ui_flow_row(ui, 28.0, |ui| {
                    let key_w = (card_w * 0.34).clamp(100.0, 180.0);
                    let new_val_w = (card_w * 0.34).clamp(100.0, 180.0);
                    ui_text_input(
                        ui,
                        "srv_prop_new_k",
                        &mut self.server_props_new_key,
                        "key",
                        key_w,
                        false,
                    );
                    ui_text_input(
                        ui,
                        "srv_prop_new_v",
                        &mut self.server_props_new_val,
                        "value",
                        new_val_w,
                        false,
                    );
                    let can_add = !self.server_props_new_key.trim().is_empty();
                    if ui_button_icon(
                        ui,
                        "srv_prop_add_btn",
                        Some(Icon::Plus),
                        tr(lang, "btn.add"),
                        ButtonVariant::Secondary,
                        can_add,
                        28.0,
                    )
                    .clicked()
                    {
                        let nk = self.server_props_new_key.trim().to_string();
                        let nv = self.server_props_new_val.trim().to_string();
                        prop_set(entries, &nk, nv);
                        self.server_props_new_key.clear();
                        self.server_props_new_val.clear();
                        anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                        props_changed = true;
                    }
                });
            }
        });
        ui.add_space(12.0);

        // Card 7: Maintenance
        card_frame(false, 0.0).show(ui, |ui| {
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);
            section_header(ui, tr(lang, "srv.sec_maintenance"));
            ui.add_space(8.0);
            ui_flow_row(ui, 28.0, |ui| {
                if ui_button_icon(
                    ui,
                    ("srv_maint_folder", &edited.id),
                    Some(Icon::Folder),
                    tr(lang, "btn.open_folder"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 28.0);
                    open_in_file_manager(dir);
                }
                if ui_button_icon(
                    ui,
                    ("srv_maint_reinstall", &edited.id),
                    Some(Icon::Download),
                    tr(lang, "srv.reinstall_core"),
                    ButtonVariant::Secondary,
                    !is_this_running && !server_busy,
                    28.0,
                )
                .clicked()
                {
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
                    trigger_reinstall = true;
                }
                if ui_button_icon(
                    ui,
                    ("srv_maint_delete", &edited.id),
                    Some(Icon::Trash),
                    tr(lang, "btn.delete"),
                    ButtonVariant::Danger,
                    !is_this_running && !server_busy,
                    28.0,
                )
                .clicked()
                {
                    anim::emit_ring(ctx, anim::pointer_or_center(ctx), DANGER, 30.0);
                    trigger_delete = true;
                }
            });
        });

        if sync_model_to_props {
            sync_server_properties_from_model(entries, &edited);
            props_changed = true;
        }
        if let Some(del_k) = remove_prop_key {
            entries.retain(|(ek, _)| ek != &del_k);
            let _ = delete_server_property_entry(dir, &del_k);
            anim::emit_ring(ctx, anim::pointer_or_center(ctx), text_secondary(), 24.0);
        }
        if props_changed {
            let _ = write_server_properties_entries(dir, entries);
        }

        if reload_props_from_disk {
            let fresh = read_server_properties_entries(dir);
            let mut reloaded_model_dirty = false;
            if let Some(p) = prop_get_u32(&fresh, "server-port", edited.port as u32)
                .try_into()
                .ok()
                .filter(|p: &u16| *p > 0)
            {
                if edited.port != p {
                    edited.port = p;
                    reloaded_model_dirty = true;
                }
                *port_str = p.to_string();
            }
            let fresh_motd = prop_get(&fresh, "motd", &edited.motd);
            if edited.motd != fresh_motd {
                edited.motd = fresh_motd;
                reloaded_model_dirty = true;
            }
            let fresh_max_p = prop_get_u32(&fresh, "max-players", edited.max_players).max(1);
            if edited.max_players != fresh_max_p {
                edited.max_players = fresh_max_p;
                reloaded_model_dirty = true;
            }
            let fresh_online = prop_get_bool(&fresh, "online-mode", edited.online_mode);
            if edited.online_mode != fresh_online {
                edited.online_mode = fresh_online;
                reloaded_model_dirty = true;
            }
            if reloaded_model_dirty {
                self.servers[self.selected_server_idx] = edited.clone();
                let _ = save_servers(&self.paths, &self.servers);
            }
            cache = Some((srv.id.clone(), fresh));
        }

        self.server_props_cache = cache;
        self.server_port_buf = port_buf_state;

        if model_changed {
            self.servers[self.selected_server_idx] = edited.clone();
            let _ = save_servers(&self.paths, &self.servers);
            let _ = write_eula_and_properties(dir, &edited);
        }

        if remove_icon {
            let _ = std::fs::remove_file(dir.join("server-icon.png"));
            self.server_icons.remove(&edited.id);
            anim::emit_ring(ctx, anim::pointer_or_center(ctx), text_secondary(), 26.0);
            self.show_toast(ctx, "Removed server-icon.png");
        }
        if let Some(msg) = toast_msg {
            self.show_toast(ctx, msg);
        }
        if trigger_rescan_java {
            self.scan_java();
        }
        if trigger_reinstall {
            self.start_provision_server(edited);
        }
        if trigger_delete {
            self.confirm_delete_server_idx = Some(self.selected_server_idx);
        }
    }

    pub(super) fn render_create_server_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("create_srv_modal_anim"),
            self.show_create_server_modal,
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_create_server_modal && modal_t <= 0.01 {
            return;
        }
        let lang = self.settings.language;
        let screen = ctx.screen_rect();
        if self.show_create_server_modal && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_create_server_modal = false;
            return;
        }
        egui::Area::new(egui::Id::new("srv_modal_backdrop"))
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
                    self.show_create_server_modal = false;
                }
            });

        if !self.show_create_server_modal {
            return;
        }

        let mut open = self.show_create_server_modal;
        let max_modal_w = (screen.width() - 32.0).clamp(280.0, 480.0);
        let offset_y = (1.0 - ease_out_cubic(modal_t)) * 18.0;
        let win_resp = egui::Window::new(tr(lang, "srv.new"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_modal_w)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, offset_y))
            .show(ctx, |ui| {
                let field_w = (ui.available_width() - 108.0).clamp(140.0, 290.0);
                egui::Grid::new("create_server_grid")
                    .num_columns(2)
                    .spacing([12.0, 12.0])
                    .show(ui, |ui| {
                        ui.label(tr(lang, "modal.name"));
                        ui_text_input(
                            ui,
                            "new_srv_name",
                            &mut self.new_srv_name,
                            "Survival Server",
                            field_w,
                            false,
                        );
                        ui.end_row();

                        ui.label("Software");
                        ui.vertical(|ui| {
                            ui.set_max_width(field_w.max(320.0));
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                for sw in ServerSoftware::ALL {
                                    let active = self.new_srv_software == sw;
                                    if ui_pill_tab(
                                        ui,
                                        ("new_srv_sw", sw.label()),
                                        sw.label(),
                                        active,
                                    )
                                    .clicked()
                                    {
                                        self.new_srv_software = sw;
                                    }
                                }
                            });
                        });
                        ui.end_row();

                        ui.label(tr(lang, "modal.version"));
                        ui_text_input(
                            ui,
                            "new_srv_ver",
                            &mut self.new_srv_version,
                            "1.21.4",
                            130.0,
                            false,
                        );
                        ui.end_row();

                        ui.label("Port");
                        ui_text_input(
                            ui,
                            "new_srv_port",
                            &mut self.new_srv_port,
                            "25565",
                            100.0,
                            false,
                        );
                        ui.end_row();

                        ui.label("RAM (MB)");
                        ui_ram_slider(ui, &mut self.new_srv_max_ram, 1024..=16384, field_w);
                        ui.end_row();

                        ui.label("MOTD");
                        ui_text_input(
                            ui,
                            "new_srv_motd",
                            &mut self.new_srv_motd,
                            "A Sunset Launcher Server",
                            field_w,
                            false,
                        );
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui_checkbox(
                    ui,
                    "new_srv_online_chk",
                    &mut self.new_srv_online_mode,
                    tr(lang, "srv.online_mode"),
                );
                ui.add_space(4.0);
                ui_checkbox(
                    ui,
                    "new_srv_upnp_chk",
                    &mut self.new_srv_upnp,
                    tr(lang, "srv.public_upnp"),
                );
                ui.add_space(4.0);
                ui_checkbox(
                    ui,
                    "new_srv_tunnel_chk",
                    &mut self.new_srv_tunnel,
                    tr(lang, "srv.tunnel"),
                );
                if self.new_srv_tunnel {
                    ui.add_space(4.0);
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        tr(lang, "srv.tunnel_notice"),
                        (ui.available_width() - 8.0).max(180.0),
                    );
                }
                ui.add_space(4.0);
                ui_checkbox(
                    ui,
                    "new_srv_eula_chk",
                    &mut self.new_srv_eula,
                    tr(lang, "srv.eula"),
                );

                ui.add_space(12.0);
                let name_valid = !self.new_srv_name.trim().is_empty()
                    && !self.new_srv_version.trim().is_empty();
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if ui_button(
                            ui,
                            "create_srv_confirm_btn",
                            tr(lang, "btn.create"),
                            ButtonVariant::Primary,
                            name_valid && !self.server_busy,
                            28.0,
                        )
                        .clicked()
                        {
                            let port = self.new_srv_port.trim().parse::<u16>().unwrap_or(25565);
                            let mut srv = DedicatedServer::new(
                                self.new_srv_name.trim().to_string(),
                                self.new_srv_software,
                                self.new_srv_version.trim().to_string(),
                                port,
                                self.new_srv_max_ram,
                                self.new_srv_online_mode,
                                self.new_srv_motd.trim().to_string(),
                                self.new_srv_eula,
                            );
                            srv.public_upnp = self.new_srv_upnp;
                            srv.use_tunnel = self.new_srv_tunnel;
                            srv.dir_name =
                                unique_server_dir_name(&self.paths, &srv.name, &self.servers);
                            self.servers.push(srv.clone());
                            self.selected_server_idx = self.servers.len() - 1;
                            let _ = save_servers(&self.paths, &self.servers);
                            self.show_create_server_modal = false;
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                            self.start_provision_server(srv);
                        }
                        if ui_button(
                            ui,
                            "create_srv_cancel_btn",
                            tr(lang, "btn.cancel"),
                            ButtonVariant::Secondary,
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            self.show_create_server_modal = false;
                        }
                    });
                });
            });
        if let Some(ir) = win_resp {
            if modal_t < 0.99 {
                let win_r = ir.response.rect;
                ctx.layer_painter(ir.response.layer_id).rect_stroke(
                    win_r,
                    Rounding::same(12.0),
                    Stroke::new(1.5_f32, with_alpha(accent(), ((1.0 - modal_t) * 165.0) as u8)),
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
            self.show_create_server_modal = false;
        }
    }

    pub(super) fn render_delete_server_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("del_srv_modal_anim"),
            self.confirm_delete_server_idx.is_some(),
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if self.confirm_delete_server_idx.is_none() && modal_t <= 0.01 {
            return;
        }
        if self.confirm_delete_server_idx.is_some()
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.confirm_delete_server_idx = None;
            return;
        }
        let lang = self.settings.language;
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("del_srv_modal_backdrop"))
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
                    self.confirm_delete_server_idx = None;
                }
            });

        let Some(idx) = self.confirm_delete_server_idx else {
            return;
        };
        let Some(target) = self.servers.get(idx).cloned() else {
            self.confirm_delete_server_idx = None;
            return;
        };

        let mut open = true;
        let max_w = (screen.width() - 32.0).clamp(240.0, 400.0);
        let offset_y = (1.0 - ease_out_cubic(modal_t)) * 16.0;
        let win_resp = egui::Window::new(tr(lang, "modal.delete_title"))
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
                        tr(lang, "modal.delete_confirm"),
                        target.name
                    ))
                    .strong()
                    .color(text_primary()),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {}",
                        target.mc_version,
                        target.software.label()
                    ))
                    .small()
                    .color(text_secondary()),
                );
                ui.add_space(12.0);
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui_button_icon(
                        ui,
                        "del_srv_confirm_btn",
                        Some(Icon::Trash),
                        tr(lang, "btn.delete"),
                        ButtonVariant::Danger,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        let removed = self.servers.remove(idx);
                        let dir = server_dir(&self.paths, &removed);
                        let _ = std::fs::remove_dir_all(dir);
                        let _ = save_servers(&self.paths, &self.servers);
                        if self.selected_server_idx >= self.servers.len() {
                            self.selected_server_idx = self.servers.len().saturating_sub(1);
                        }
                        self.server_detail_open = false;
                        self.confirm_delete_server_idx = None;
                        self.show_toast(ctx, format!("Removed '{}'", removed.name));
                        anim::emit_burst(ctx, anim::pointer_or_center(ctx), DANGER, 8);
                        anim::emit_edge_pulse(ctx, DANGER);
                    }
                    if ui_button(
                        ui,
                        "del_srv_cancel_btn",
                        tr(lang, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.confirm_delete_server_idx = None;
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
            self.confirm_delete_server_idx = None;
        }
    }
}
