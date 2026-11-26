use super::*;

impl McLauncherApp {
    pub(super) fn render_accounts_page(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        ui_toolbar_row(ui, CTL_HEIGHT, |ui| {
            page_heading(ui, tr(lang, "nav.accounts"));
            ui.label(
                egui::RichText::new(format!("{}", self.account_store.accounts.len()))
                    .monospace()
                    .size(12.0)
                    .color(text_muted()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "acc_refresh_skins",
                    Some(Icon::Refresh),
                    tr(lang, "acc.refresh_skins"),
                    ButtonVariant::Secondary,
                    self.skin_pending.is_empty(),
                    28.0,
                )
                .clicked()
                {
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
                    self.refresh_all_skins();
                }
            });
        });
        ui.add_space(8.0);

        bounded_scroll(ui, "accounts_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(130.0);
            let inner_w = card_inner_w.clamp(120.0, 460.0);

            if let Some(err) = &self.auth_error {
                ui_alert_banner(
                    ui,
                    AlertKind::Error,
                    &format!("Error: {err}"),
                    card_inner_w + 22.0,
                );
                ui.add_space(6.0);
            }
            if let Some(msg) = &self.auth_status {
                ui_alert_banner(ui, AlertKind::Success, msg, card_inner_w + 22.0);
                ui.add_space(6.0);
            }

            if let Some(active) = self.active_account().cloned() {
                let key = account_skin_cache_key(&active);
                let body_tex = self.skin_bodies.get(&key).and_then(|o| o.as_ref()).cloned();
                let acc_col = match active.kind {
                    AccountKind::Microsoft { .. } => accent(),
                    AccountKind::ElyBy { .. } => INFO,
                    AccountKind::Offline => text_secondary(),
                };
                let preview_id = ui.make_persistent_id(("acc_skin_preview", &active.id));
                animated_card(ui, preview_id, true, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    let slot_size = if body_tex.is_some() {
                        Vec2::new(40.0, 80.0)
                    } else {
                        Vec2::splat(44.0)
                    };
                    let is_loading = self.skin_pending.contains(&key);
                    let stack_h = if is_loading { 56.0 } else { 38.0 };
                    let top_pad = ((slot_size.y - stack_h) * 0.5).max(0.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        let (body_r, _) = ui.allocate_exact_size(slot_size, Sense::hover());
                        if let Some(ref tex) = body_tex {
                            ui.painter().rect_filled(
                                body_r.expand(3.0),
                                Rounding::same(8.0),
                                bg_input(),
                            );
                            ui.painter().image(
                                tex.id(),
                                body_r,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            draw_account_avatar(
                                ui.painter(),
                                body_r,
                                Some(&active),
                                &active.username,
                                acc_col,
                                &self.skin_heads,
                            );
                        }
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            if top_pad > 0.0 {
                                ui.add_space(top_pad);
                            }
                            section_header(ui, tr(lang, "acc.skin_preview"));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().interact_size.y = 20.0;
                                ui.strong(
                                    egui::RichText::new(&active.username)
                                        .size(13.5)
                                        .color(text_primary()),
                                );
                                badge_sized(ui, active.kind_label(), true, 20.0, None);
                            });
                            if is_loading {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().interact_size.y = 16.0;
                                    mono_spinner(ui, 13.0);
                                    ui.label(
                                        egui::RichText::new(tr(lang, "top.loading"))
                                            .small()
                                            .color(text_muted()),
                                    );
                                });
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }

            let active_id = self.account_store.active_id.clone();
            let mut select_id: Option<String> = None;
            let mut delete_id: Option<String> = None;
            let mut refresh_acc: Option<Account> = None;

            for acc in &self.account_store.accounts {
                let is_active = active_id.as_deref() == Some(acc.id.as_str());
                let acc_col = match acc.kind {
                    AccountKind::Microsoft { .. } => accent(),
                    AccountKind::ElyBy { .. } => INFO,
                    AccountKind::Offline => text_secondary(),
                };
                let uuid_short = match &acc.kind {
                    AccountKind::Microsoft { uuid, .. } | AccountKind::ElyBy { uuid, .. } => {
                        Some(uuid.get(..8).unwrap_or(uuid.as_str()).to_string())
                    }
                    AccountKind::Offline => None,
                };

                let acc_card_id = ui.make_persistent_id(("acc_card_hover", &acc.id));
                animated_card(ui, acc_card_id, is_active, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    if card_inner_w >= 430.0 {
                        ui_toolbar_row(ui, 34.0, |ui| {
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                            draw_account_avatar(
                                ui.painter(),
                                av_r,
                                Some(acc),
                                &acc.username,
                                acc_col,
                                &self.skin_heads,
                            );
                            ui.strong(
                                egui::RichText::new(&acc.username)
                                    .size(13.5)
                                    .color(text_primary()),
                            );
                            ui.label(
                                egui::RichText::new(acc.kind_label())
                                    .small()
                                    .color(text_muted()),
                            );
                            if is_active {
                                badge(ui, tr(self.settings.language, "acc.active"), true);
                            }
                            if let Some(u) = &uuid_short {
                                if card_inner_w >= 480.0 {
                                    ui.label(
                                        egui::RichText::new(format!("{u}..."))
                                            .monospace()
                                            .small()
                                            .color(text_muted()),
                                    );
                                }
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if self.account_store.accounts.len() > 1
                                        && ui_icon_button(
                                            ui,
                                            ("acc_del", &acc.id),
                                            Icon::Trash,
                                            tr(self.settings.language, "acc.delete_tooltip"),
                                            ButtonVariant::Danger,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                    {
                                        delete_id = Some(acc.id.clone());
                                    }
                                    if !matches!(acc.kind, AccountKind::Offline)
                                        && ui_icon_button(
                                            ui,
                                            ("acc_ref", &acc.id),
                                            Icon::Refresh,
                                            tr(self.settings.language, "acc.refresh_session"),
                                            ButtonVariant::Subtle,
                                            !self.acc_refresh_busy && !self.settings.offline_mode,
                                            26.0,
                                        )
                                        .clicked()
                                    {
                                        refresh_acc = Some(acc.clone());
                                    }
                                    if !is_active
                                        && ui_button(
                                            ui,
                                            ("acc_sel", &acc.id),
                                            tr(self.settings.language, "btn.select"),
                                            ButtonVariant::Secondary,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                    {
                                        select_id = Some(acc.id.clone());
                                    }
                                },
                            );
                        });
                    } else {
                        ui_wrap_row(ui, 28.0, |ui| {
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                            draw_account_avatar(
                                ui.painter(),
                                av_r,
                                Some(acc),
                                &acc.username,
                                acc_col,
                                &self.skin_heads,
                            );
                            ui.strong(&acc.username);
                            ui.label(
                                egui::RichText::new(acc.kind_label())
                                    .small()
                                    .color(text_muted()),
                            );
                            if is_active {
                                badge(ui, tr(self.settings.language, "acc.active"), true);
                            }
                            if !is_active
                                && ui_button(
                                    ui,
                                    ("acc_sel", &acc.id),
                                    tr(self.settings.language, "btn.select"),
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                            {
                                select_id = Some(acc.id.clone());
                            }
                            if !matches!(acc.kind, AccountKind::Offline)
                                && ui_icon_button(
                                    ui,
                                    ("acc_ref_n", &acc.id),
                                    Icon::Refresh,
                                    tr(self.settings.language, "acc.refresh_session"),
                                    ButtonVariant::Subtle,
                                    !self.acc_refresh_busy && !self.settings.offline_mode,
                                    28.0,
                                )
                                .clicked()
                            {
                                refresh_acc = Some(acc.clone());
                            }
                            if self.account_store.accounts.len() > 1
                                && ui_icon_button(
                                    ui,
                                    ("acc_del", &acc.id),
                                    Icon::Trash,
                                    tr(self.settings.language, "acc.delete_tooltip"),
                                    ButtonVariant::Danger,
                                    true,
                                    28.0,
                                )
                                .clicked()
                            {
                                delete_id = Some(acc.id.clone());
                            }
                        });
                    }
                });
                ui.add_space(4.0);
            }

            if let Some(mut acc) = refresh_acc {
                self.acc_refresh_busy = true;
                self.auth_error = None;
                let ms_client_id = self.ms_client_id.clone();
                let tx = self.tx.clone();
                thread::spawn(move || {
                    let res = ensure_valid_account_with_mode(&mut acc, &ms_client_id, false)
                        .map(|()| acc);
                    let _ = tx.send(WorkerEvent::AccountManualRefreshed(res));
                });
            }
            if let Some(id) = select_id {
                self.account_store.active_id = Some(id);
                let _ = save_accounts(&self.paths, &self.account_store);
                anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 8);
            }
            if let Some(id) = delete_id {
                delete_account_secret(&id);
                self.account_store.accounts.retain(|a| a.id != id);
                if self.account_store.active_id.as_deref() == Some(id.as_str()) {
                    self.account_store.active_id =
                        self.account_store.accounts.first().map(|a| a.id.clone());
                }
                let _ = save_accounts(&self.paths, &self.account_store);
                anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 8);
            }

            ui.add_space(12.0);

            // Unified Add Account card with provider ComboBox
            let prov_id = egui::Id::new("acc_add_provider_idx");
            let mut prov: u8 = ui.ctx().data_mut(|d| d.get_temp::<u8>(prov_id).unwrap_or(0));
            let prov_label = |p: u8, lang: Language| -> &'static str {
                match p {
                    0 => tr(lang, "acc.ms_title"),
                    1 => "Ely.by",
                    _ => tr(lang, "acc.offline_title"),
                }
            };

            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 26.0, |ui| {
                    let prev_prov = prov;
                    egui::ComboBox::from_id_salt("acc_provider_combo")
                        .selected_text(prov_label(prov, self.settings.language))
                        .width(172.0)
                        .truncate()
                        .show_ui(ui, |ui| {
                            for opt in [0_u8, 1, 2] {
                                ui.selectable_value(
                                    &mut prov,
                                    opt,
                                    prov_label(opt, self.settings.language),
                                );
                            }
                        });
                    if prov != prev_prov {
                        ui.ctx().data_mut(|d| d.insert_temp(prov_id, prov));
                        self.mark_subtab_switch(ui.ctx());
                    }

                    if prov == 0 {
                        ui.label(
                            egui::RichText::new("OAuth Device Code")
                                .small()
                                .color(text_muted()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let toggle_lbl = if self.ms_show_custom_id {
                                tr(self.settings.language, "acc.hide")
                            } else {
                                tr(self.settings.language, "acc.custom_id")
                            };
                            if ui_button(
                                ui,
                                "ms_toggle_cid",
                                toggle_lbl,
                                ButtonVariant::Subtle,
                                true,
                                22.0,
                            )
                            .clicked()
                            {
                                self.ms_show_custom_id = !self.ms_show_custom_id;
                            }
                        });
                    } else if prov == 1 {
                        ui.label(
                            egui::RichText::new("authlib-injector")
                                .small()
                                .color(text_muted()),
                        );
                    }
                });
                ui.add_space(6.0);

                if prov == 0 {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "acc.ms_desc"))
                            .small()
                            .color(text_secondary()),
                    );
                    if self.ms_show_custom_id {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(tr(self.settings.language, "acc.client_id"))
                                .small()
                                .color(text_secondary()),
                        );
                        ui_text_input(
                            ui,
                            "ms_cid_input",
                            &mut self.ms_client_id,
                            DEFAULT_MS_CLIENT_ID,
                            inner_w,
                            false,
                        );
                    }
                    ui.add_space(6.0);

                    if let Some(dc) = self.ms_device_code.clone() {
                        ui.label(&dc.message);
                        ui.add_space(6.0);
                        ui_wrap_row(ui, 26.0, |ui| {
                            ui.label(tr(self.settings.language, "acc.code"));
                            solid_accent_badge(ui, &dc.user_code);
                            if ui_button_icon(
                                ui,
                                "ms_copy_code_btn",
                                Some(Icon::Copy),
                                tr(self.settings.language, "btn.copy"),
                                ButtonVariant::Secondary,
                                true,
                                26.0,
                            )
                            .clicked()
                            {
                                ui.ctx().copy_text(dc.user_code.clone());
                                let pos = anim::pointer_or_center(ui.ctx());
                                anim::emit_ring(ui.ctx(), pos, accent(), 26.0);
                                self.show_toast(
                                    ui.ctx(),
                                    tr(self.settings.language, "acc.code_copied"),
                                );
                            }
                            if ui_button_icon(
                                ui,
                                "ms_open_browser_btn",
                                Some(Icon::External),
                                tr(self.settings.language, "acc.open_browser"),
                                ButtonVariant::Primary,
                                true,
                                26.0,
                            )
                            .clicked()
                            {
                                anim::emit_ring(
                                    ui.ctx(),
                                    anim::pointer_or_center(ui.ctx()),
                                    accent(),
                                    26.0,
                                );
                                ui.ctx()
                                    .open_url(egui::OpenUrl::new_tab(&dc.verification_uri));
                            }
                            if ui_button_icon(
                                ui,
                                "ms_cancel_login_btn",
                                Some(Icon::Close),
                                tr(self.settings.language, "acc.cancel_login"),
                                ButtonVariant::Danger,
                                true,
                                26.0,
                            )
                            .clicked()
                            {
                                if let Some(tok) = self.ms_login_cancel.take() {
                                    tok.store(true, Ordering::SeqCst);
                                }
                                self.ms_busy = false;
                                self.ms_device_code = None;
                                self.auth_status = None;
                            }
                            mono_spinner(ui, 15.0);
                        });
                    } else {
                        ui_toolbar_row(ui, 26.0, |ui| {
                            if ui_button_icon(
                                ui,
                                "ms_login_btn",
                                Some(Icon::User),
                                tr(self.settings.language, "acc.ms_signin"),
                                ButtonVariant::Primary,
                                !self.ms_busy,
                                26.0,
                            )
                            .clicked()
                            {
                                anim::emit_ring(
                                    ui.ctx(),
                                    anim::pointer_or_center(ui.ctx()),
                                    accent(),
                                    26.0,
                                );
                                let cancel_tok = Arc::new(AtomicBool::new(false));
                                self.ms_login_cancel = Some(Arc::clone(&cancel_tok));
                                self.ms_busy = true;
                                self.auth_error = None;
                                self.auth_status =
                                    Some("Requesting Microsoft device code...".to_string());
                                start_microsoft_device_login(
                                    self.ms_client_id.clone(),
                                    cancel_tok,
                                    self.tx.clone(),
                                );
                            }
                            if self.ms_busy {
                                if ui_button_icon(
                                    ui,
                                    "ms_cancel_login_init_btn",
                                    Some(Icon::Close),
                                    tr(self.settings.language, "acc.cancel_login"),
                                    ButtonVariant::Danger,
                                    true,
                                    26.0,
                                )
                                .clicked()
                                {
                                    if let Some(tok) = self.ms_login_cancel.take() {
                                        tok.store(true, Ordering::SeqCst);
                                    }
                                    self.ms_busy = false;
                                    self.auth_status = None;
                                }
                                mono_spinner(ui, 15.0);
                            }
                        });
                    }
                } else if prov == 1 {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "acc.ely_desc"))
                            .small()
                            .color(text_secondary()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "acc.login_label"))
                            .small()
                            .color(text_secondary()),
                    );
                    ui_toolbar_row(ui, 26.0, |ui| {
                        ui_text_input(
                            ui,
                            "ely_login_in",
                            &mut self.ely_login,
                            tr(self.settings.language, "acc.login_hint"),
                            inner_w,
                            false,
                        );
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "acc.password"))
                            .small()
                            .color(text_secondary()),
                    );
                    ui_toolbar_row(ui, 26.0, |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui_text_input(
                            ui,
                            "ely_pass_in",
                            &mut self.ely_password,
                            tr(self.settings.language, "acc.pass_hint"),
                            (inner_w - 60.0).max(80.0),
                            !self.ely_show_password,
                        );
                        if ui_button(
                            ui,
                            "ely_show_pw",
                            if self.ely_show_password {
                                tr(self.settings.language, "acc.hide")
                            } else {
                                tr(self.settings.language, "acc.show")
                            },
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .clicked()
                        {
                            self.ely_show_password = !self.ely_show_password;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "acc.2fa_label"))
                            .small()
                            .color(text_secondary()),
                    );
                    ui_text_input(
                        ui,
                        "ely_totp_in",
                        &mut self.ely_totp,
                        tr(self.settings.language, "acc.2fa_hint"),
                        inner_w,
                        false,
                    );
                    ui.add_space(6.0);
                    ui_toolbar_row(ui, 26.0, |ui| {
                        let ely_valid = !self.ely_busy
                            && !self.ely_login.trim().is_empty()
                            && !self.ely_password.is_empty();
                        if ui_button_icon(
                            ui,
                            "ely_login_btn",
                            Some(Icon::User),
                            tr(self.settings.language, "acc.ely_signin"),
                            ButtonVariant::Primary,
                            ely_valid,
                            26.0,
                        )
                        .clicked()
                        {
                            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), INFO, 26.0);
                            self.ely_busy = true;
                            self.auth_error = None;
                            self.auth_status = Some("Signing in to Ely.by...".to_string());
                            let login = self.ely_login.clone();
                            let password = self.ely_password.clone();
                            let totp = self.ely_totp.clone();
                            let tx = self.tx.clone();
                            thread::spawn(move || {
                                let res = login_ely_by(&login, &password, &totp);
                                let _ = tx.send(WorkerEvent::ElyAuthFinished(res));
                            });
                        }
                        if self.ely_busy {
                            mono_spinner(ui, 15.0);
                        }
                    });
                } else {
                    ui_wrap_row(ui, 26.0, |ui| {
                        ui.label(tr(self.settings.language, "acc.username"));
                        let edit_resp = ui_text_input(
                            ui,
                            "offline_user_in",
                            &mut self.offline_input,
                            "Steve",
                            (card_inner_w - 150.0).clamp(110.0, 260.0),
                            false,
                        );
                        let enter = edit_resp.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let valid = !self.offline_input.trim().is_empty();
                        if (ui_button_icon(
                            ui,
                            "offline_add_btn",
                            Some(Icon::Plus),
                            tr(self.settings.language, "btn.add"),
                            ButtonVariant::Primary,
                            valid,
                            26.0,
                        )
                        .clicked()
                            || enter)
                            && valid
                        {
                            let name = self.offline_input.trim().to_string();
                            if let Some(existing) = self.account_store.accounts.iter().find(|a| {
                                matches!(a.kind, AccountKind::Offline)
                                    && a.username.eq_ignore_ascii_case(&name)
                            }) {
                                self.account_store.active_id = Some(existing.id.clone());
                            } else {
                                let acc = Account::offline(name.clone());
                                self.account_store.active_id = Some(acc.id.clone());
                                self.account_store.accounts.push(acc);
                            }
                            let _ = save_accounts(&self.paths, &self.account_store);
                            self.offline_input.clear();
                            self.auth_error = None;
                            self.auth_status = Some(format!(
                                "{}: {name}",
                                tr(self.settings.language, "acc.offline_added")
                            ));
                            anim::emit_burst(
                                ui.ctx(),
                                anim::pointer_or_center(ui.ctx()),
                                accent(),
                                8,
                            );
                        }
                    });
                }
            });
        });
    }

    pub(super) fn render_discover_page(&mut self, ui: &mut egui::Ui) {
        let target_server = self
            .discover_target_server_id
            .as_ref()
            .and_then(|id| self.servers.iter().find(|s| &s.id == id))
            .cloned();
        let cur_profile = self.profiles.get(self.selected_idx).cloned();
        let cur_loader = cur_profile.as_ref().map(|p| p.loader.kind());
        let is_vanilla_mods = target_server.is_none()
            && cur_loader == Some(LoaderKind::Vanilla)
            && self.mr_type == ModrinthProjectType::Mod;
        let target_label = if let Some(srv) = &target_server {
            format!(
                "Server: {} ({} {})",
                srv.name,
                srv.software.label(),
                srv.mc_version
            )
        } else {
            cur_profile
                .as_ref()
                .map(|p| {
                    if is_vanilla_mods {
                        format!(
                            "Vanilla {} ({})",
                            p.version_id,
                            tr(self.settings.language, "mr.no_loader")
                        )
                    } else {
                        format!(
                            "{} {} • {}",
                            tr(self.settings.language, "mr.only_for"),
                            p.version_id,
                            p.loader.short_name()
                        )
                    }
                })
                .unwrap_or_else(|| tr(self.settings.language, "mr.no_instance").to_string())
        };

        let avail_w = ui.available_width();
        if avail_w >= 460.0 {
            ui_toolbar_row(ui, CTL_HEIGHT, |ui| {
                page_heading(ui, self.discover_source.label());
                if avail_w >= 540.0 {
                    let max_badge_w = (avail_w - 320.0).max(80.0);
                    badge_sized(ui, &target_label, !is_vanilla_mods, 26.0, Some(max_badge_w));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if target_server.is_some() {
                        if ui_button_icon(
                            ui,
                            "mr_back_srv_btn",
                            Some(Icon::Server),
                            tr(self.settings.language, "nav.servers"),
                            ButtonVariant::Secondary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            self.switch_page(ui.ctx(), NavPage::Servers);
                        }
                    } else if ui_button_icon(
                        ui,
                        "mr_inst_content_btn",
                        Some(Icon::Layers),
                        tr(self.settings.language, "btn.instance_content"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.instance_tab = InstanceSubTab::Content;
                        self.switch_page(ui.ctx(), NavPage::Instance);
                    }
                });
            });
        } else {
            ui_wrap_row(ui, CTL_HEIGHT, |ui| {
                page_heading(ui, self.discover_source.label());
                if target_server.is_some() {
                    if ui_button_icon(
                        ui,
                        "mr_back_srv_btn_n",
                        Some(Icon::Server),
                        tr(self.settings.language, "nav.servers"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        self.switch_page(ui.ctx(), NavPage::Servers);
                    }
                } else if ui_button_icon(
                    ui,
                    "mr_inst_content_btn_n",
                    Some(Icon::Layers),
                    tr(self.settings.language, "btn.instance_content"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    self.instance_tab = InstanceSubTab::Content;
                    self.switch_page(ui.ctx(), NavPage::Instance);
                }
            });
        }

        ui.add_space(6.0);
        ui_wrap_row(ui, 26.0, |ui| {
            if !self.servers.is_empty() {
                let lang = self.settings.language;
                let inst_lbl = cur_profile
                    .as_ref()
                    .map(|p| format!("{}: {}", tr(lang, "nav.instance"), p.name))
                    .unwrap_or_else(|| tr(lang, "nav.instance").to_string());
                let selected_target_lbl = if let Some(srv) = &target_server {
                    format!("{}: {}", tr(lang, "nav.servers"), srv.name)
                } else {
                    inst_lbl.clone()
                };
                let mut chosen_srv_id = self.discover_target_server_id.clone();
                egui::ComboBox::from_id_salt("mr_target_combo")
                    .selected_text(selected_target_lbl)
                    .width(164.0)
                    .truncate()
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen_srv_id, None, inst_lbl);
                        for s in &self.servers {
                            ui.selectable_value(
                                &mut chosen_srv_id,
                                Some(s.id.clone()),
                                format!("{}: {}", tr(lang, "nav.servers"), s.name),
                            );
                        }
                    });
                if chosen_srv_id != self.discover_target_server_id {
                    if let Some(ref sid) = chosen_srv_id {
                        if let Some(chosen_srv) = self.servers.iter().find(|s| &s.id == sid) {
                            let srv_dir = server_dir(&self.paths, chosen_srv);
                            if !chosen_srv.software.supports_mods()
                                && server_supports_plugins_live(&srv_dir, chosen_srv.software)
                            {
                                self.mr_type = ModrinthProjectType::Plugin;
                            } else if chosen_srv.software.supports_mods()
                                && (self.mr_type != ModrinthProjectType::Plugin
                                    || !server_supports_plugins_live(
                                        &srv_dir,
                                        chosen_srv.software,
                                    ))
                            {
                                self.mr_type = ModrinthProjectType::Mod;
                            }
                        }
                    } else if self.mr_type == ModrinthProjectType::Plugin {
                        self.mr_type = ModrinthProjectType::Mod;
                    }
                    self.discover_target_server_id = chosen_srv_id;
                    self.mark_subtab_switch(ui.ctx());
                    self.trigger_modrinth_search(false);
                }
            }
            let prev_src = self.discover_source;
            egui::ComboBox::from_id_salt("discover_src_combo")
                .selected_text(self.discover_source.label())
                .width(122.0)
                .show_ui(ui, |ui| {
                    for src in [ProjectSource::Modrinth, ProjectSource::CurseForge] {
                        ui.selectable_value(&mut self.discover_source, src, src.label());
                    }
                });
            if self.discover_source != prev_src {
                self.mr_results.clear();
                self.mr_total_hits = 0;
                self.mr_next_offset = 0;
                self.mr_has_more = false;
                self.mr_error = None;
                self.mark_subtab_switch(ui.ctx());
                self.trigger_modrinth_search(false);
            }
            let (allow_srv_mods, allow_srv_plugins) = if let Some(srv) = &target_server {
                let srv_dir = server_dir(&self.paths, srv);
                (
                    srv.software.supports_mods(),
                    server_supports_plugins_live(&srv_dir, srv.software),
                )
            } else {
                (true, false)
            };
            let lang = self.settings.language;
            let pt_label = |pt: ModrinthProjectType| match pt {
                ModrinthProjectType::Mod => tr(lang, "content.mods"),
                ModrinthProjectType::Plugin => tr(lang, "mr.plugins"),
                ModrinthProjectType::Modpack => tr(lang, "mr.modpacks"),
                ModrinthProjectType::Resourcepack => tr(lang, "content.resourcepacks"),
                ModrinthProjectType::Shader => tr(lang, "content.shaderpacks"),
                ModrinthProjectType::Datapack => tr(lang, "mr.datapacks"),
            };
            let prev_pt = self.mr_type;
            egui::ComboBox::from_id_salt("mr_type_combo")
                .selected_text(pt_label(self.mr_type))
                .width(142.0)
                .show_ui(ui, |ui| {
                    for pt in [
                        ModrinthProjectType::Mod,
                        ModrinthProjectType::Plugin,
                        ModrinthProjectType::Modpack,
                        ModrinthProjectType::Resourcepack,
                        ModrinthProjectType::Shader,
                        ModrinthProjectType::Datapack,
                    ] {
                        if target_server.is_some() {
                            match pt {
                                ModrinthProjectType::Mod if !allow_srv_mods => continue,
                                ModrinthProjectType::Plugin if !allow_srv_plugins => continue,
                                ModrinthProjectType::Modpack
                                | ModrinthProjectType::Resourcepack
                                | ModrinthProjectType::Shader
                                | ModrinthProjectType::Datapack => continue,
                                _ => {}
                            }
                        } else if pt == ModrinthProjectType::Plugin {
                            continue;
                        }
                        ui.selectable_value(&mut self.mr_type, pt, pt_label(pt));
                    }
                });
            if self.mr_type != prev_pt {
                self.mark_subtab_switch(ui.ctx());
                self.trigger_modrinth_search(false);
            }
            let sort_label = |s: ModrinthSort| match s {
                ModrinthSort::Relevance => tr(lang, "mr.sort_relevance"),
                ModrinthSort::Downloads => tr(lang, "mr.sort_downloads"),
                ModrinthSort::Newest => tr(lang, "mr.sort_newest"),
                ModrinthSort::Updated => tr(lang, "mr.sort_updated"),
            };
            egui::ComboBox::from_id_salt("mr_sort_combo")
                .selected_text(sort_label(self.mr_sort))
                .width(132.0)
                .show_ui(ui, |ui| {
                    for s in [
                        ModrinthSort::Relevance,
                        ModrinthSort::Downloads,
                        ModrinthSort::Newest,
                        ModrinthSort::Updated,
                    ] {
                        if ui
                            .selectable_value(&mut self.mr_sort, s, sort_label(s))
                            .changed()
                        {
                            self.mark_subtab_switch(ui.ctx());
                            self.trigger_modrinth_search(false);
                        }
                    }
                });
        });

        ui.add_space(5.0);
        let search_w = if ui.available_width() < 340.0 {
            (ui.available_width() - 16.0).max(110.0)
        } else {
            (ui.available_width() - 154.0).clamp(110.0, 520.0)
        };
        ui_wrap_row(ui, 30.0, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let search_placeholder = match self.discover_source {
                ProjectSource::Modrinth => tr(self.settings.language, "mr.search_placeholder"),
                ProjectSource::CurseForge => tr(self.settings.language, "cf.search_placeholder"),
            };
            let enter_or_cleared = ui_search_input(
                ui,
                "mr_search_input",
                &mut self.mr_query,
                search_placeholder,
                search_w,
            );
            if (ui_button_icon(
                ui,
                "mr_search_btn",
                Some(Icon::Search),
                tr(self.settings.language, "btn.search"),
                ButtonVariant::Primary,
                !self.mr_searching,
                30.0,
            )
            .clicked()
                || enter_or_cleared)
                && !self.mr_searching
            {
                anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
                self.mark_subtab_switch(ui.ctx());
                self.trigger_modrinth_search(false);
            }
            if self.mr_searching {
                mono_spinner(ui, 16.0);
            }
        });

        if let Some(err) = &self.mr_error {
            ui.add_space(6.0);
            ui_alert_banner(
                ui,
                AlertKind::Error,
                &format!("Error: {err}"),
                ui.available_width(),
            );
        }
        if let Some(msg) = &self.mr_status {
            ui.add_space(6.0);
            ui_alert_banner(ui, AlertKind::Success, msg, ui.available_width());
        }

        ui.add_space(6.0);
        let mut install_req: Option<(String, String, Option<String>)> = None;
        let mut open_detail: Option<ModrinthSearchHit> = None;
        let mut load_more = false;
        let mr_type = self.mr_type;
        let mr_installing = self.mr_installing;
        let mr_installing_id = self.mr_installing_id.clone();
        let subtab_t = self.subtab_switch_time;
        let mut open_inst_settings_req = false;

        let is_server_target = target_server.is_some();
        if let Some(ref srv) = target_server {
            self.sync_server_installed_index(srv);
        }
        Self::apply_subtab_transition(ui, subtab_t, |ui| {
            bounded_scroll(ui, "modrinth_scroll", |ui| {
                let usable_w = (ui.available_width() - 12.0).max(160.0);
                let card_inner_w = (usable_w - 22.0).max(140.0);

                if self.mr_results.is_empty() && self.mr_searching {
                    for _ in 0..4 {
                        draw_skeleton_card(ui, card_inner_w);
                        ui.add_space(6.0);
                    }
                    return;
                } else if self.mr_results.is_empty() && !self.mr_searching {
                    let mut open_inst_settings = false;
                    card_frame(false, 0.0).show(ui, |ui| {
                        ui.set_min_width(card_inner_w);
                        ui.set_max_width(card_inner_w);
                        if is_vanilla_mods {
                            ui_wrap_row(ui, 30.0, |ui| {
                                ui.label(
                                    egui::RichText::new(tr(
                                        self.settings.language,
                                        "mr.vanilla_warning",
                                    ))
                                    .color(text_muted()),
                                );
                                if ui_button_icon(
                                    ui,
                                    "mr_open_inst_settings",
                                    Some(Icon::Sliders),
                                    tr(self.settings.language, "inst.settings_btn"),
                                    ButtonVariant::Primary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    open_inst_settings = true;
                                }
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(tr(self.settings.language, "mr.no_results"))
                                    .color(text_muted()),
                            );
                        }
                    });
                    if open_inst_settings {
                        open_inst_settings_req = true;
                    }
                    return;
                }

                let results = self.mr_results.clone();
                let active_loader_slug = cur_loader.and_then(|l| l.modrinth_slug());
                let wide_card = card_inner_w >= 540.0;
                for hit in &results {
                    let is_installed = if is_server_target {
                        self.server_installed_cache.contains_key(&hit.project_id)
                    } else {
                        self.mr_installed_cache.contains_key(&hit.project_id)
                    };
                    let is_this_installing = mr_installing
                        && mr_installing_id.as_deref() == Some(hit.project_id.as_str());
                    let btn_lbl = if is_this_installing {
                        tr(self.settings.language, "mr.installing")
                    } else if mr_type == ModrinthProjectType::Modpack {
                        tr(self.settings.language, "modal.create_instance")
                    } else if is_installed {
                        tr(self.settings.language, "mr.update")
                    } else {
                        tr(self.settings.language, "mr.install")
                    };
                    let btn_variant = if is_installed {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Primary
                    };
                    let btn_icon = if is_installed {
                        Icon::Refresh
                    } else {
                        Icon::Download
                    };

                    let card_id = ui.make_persistent_id(("mr_card_hover", &hit.project_id));
                    animated_card(ui, card_id, is_installed, |ui| {
                        ui.set_min_width(card_inner_w);
                        ui.set_max_width(card_inner_w);

                        let tex_opt = hit
                            .icon_url
                            .as_ref()
                            .and_then(|u| self.mr_icons.get(u))
                            .and_then(|o| o.as_ref());
                        let is_icon_loading = hit
                            .icon_url
                            .as_ref()
                            .is_some_and(|u| self.mr_icons_pending.contains(u.as_str()));

                        if wide_card {
                            let icon_px = 52.0;
                            let action_w = 206.0;
                            let btn_w = 132.0;
                            let gap_x = 12.0;
                            let info_w =
                                (card_inner_w - icon_px - action_w - gap_x * 2.0).max(120.0);
                            let max_desc_chars =
                                ((info_w / 6.3) * 1.85).clamp(48.0, 145.0) as usize;
                            let desc = clamp_summary(&hit.description, max_desc_chars);

                            ui.allocate_ui_with_layout(
                                Vec2::new(card_inner_w, 72.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_size(Vec2::new(card_inner_w, 72.0));
                                    ui.set_max_size(Vec2::new(card_inner_w, 72.0));
                                    ui.spacing_mut().item_spacing.x = gap_x;

                                    let icon_size = Vec2::splat(icon_px);
                                    if let Some(tex) = tex_opt {
                                        ui.add(
                                            egui::Image::new(tex)
                                                .fit_to_exact_size(icon_size)
                                                .rounding(Rounding::same(10.0)),
                                        );
                                    } else {
                                        let (rect, _) =
                                            ui.allocate_exact_size(icon_size, Sense::hover());
                                        draw_icon_placeholder(
                                            ui,
                                            rect,
                                            10.0,
                                            &hit.title,
                                            is_icon_loading,
                                        );
                                    }

                                    ui.allocate_ui_with_layout(
                                        Vec2::new(info_w, 72.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_size(Vec2::new(info_w, 72.0));
                                            ui.set_max_size(Vec2::new(info_w, 72.0));
                                            ui.spacing_mut().item_spacing.y = 2.0;

                                            // Line 1: Title + Installed/CF badge
                                            ui_toolbar_row(ui, 20.0, |ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
                                                let badge_w = if is_installed { 76.0 } else { 0.0 }
                                                    + if hit.source == ProjectSource::CurseForge {
                                                        38.0
                                                    } else {
                                                        0.0
                                                    };
                                                ui.allocate_ui(
                                                    Vec2::new((info_w - badge_w).max(60.0), 20.0),
                                                    |ui| {
                                                        let title_id = ui.make_persistent_id((
                                                            "mr_title_w",
                                                            &hit.project_id,
                                                        ));
                                                        let prev_hov = ui
                                                            .ctx()
                                                            .data(|d| d.get_temp::<bool>(title_id))
                                                            .unwrap_or(false);
                                                        let title_hov =
                                                            ui.ctx().animate_bool_with_time(
                                                                title_id.with("h"),
                                                                prev_hov,
                                                                0.11,
                                                            );
                                                        if title_hov > 0.01 && title_hov < 0.99 {
                                                            ui.ctx().request_repaint();
                                                        }
                                                        let title_col = lerp_color(
                                                            text_primary(),
                                                            accent_text(),
                                                            title_hov,
                                                        );
                                                        let title_resp = ui
                                                            .add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(&hit.title)
                                                                        .strong()
                                                                        .size(13.5)
                                                                        .color(title_col),
                                                                )
                                                                .truncate()
                                                                .sense(Sense::click()),
                                                            )
                                                            .on_hover_cursor(
                                                                egui::CursorIcon::PointingHand,
                                                            )
                                                            .on_hover_text(tr(
                                                                self.settings.language,
                                                                "mr.click_versions",
                                                            ));
                                                        ui.ctx().data_mut(|d| {
                                                            d.insert_temp(
                                                                title_id,
                                                                title_resp.hovered(),
                                                            )
                                                        });
                                                        if title_resp.clicked() {
                                                            open_detail = Some(hit.clone());
                                                        }
                                                    },
                                                );
                                                if hit.source == ProjectSource::CurseForge {
                                                    badge(ui, "CF", false);
                                                }
                                                if is_installed {
                                                    badge(
                                                        ui,
                                                        tr(self.settings.language, "mr.installed"),
                                                        true,
                                                    );
                                                }
                                            });

                                            // Line 2: Uniform 28px summary slot so all rows have equal height
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(info_w, 28.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.set_min_size(Vec2::new(info_w, 28.0));
                                                    ui.set_max_size(Vec2::new(info_w, 28.0));
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&desc)
                                                                .small()
                                                                .color(text_secondary()),
                                                        )
                                                        .wrap(),
                                                    );
                                                },
                                            );

                                            // Line 3: Single-line stats + category chips
                                            ui_toolbar_row(ui, 20.0, |ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
                                                let display_cats: Vec<&str> = hit
                                                    .categories
                                                    .iter()
                                                    .map(String::as_str)
                                                    .filter(|c| match *c {
                                                        "fabric" | "quilt" | "forge"
                                                        | "neoforge" => {
                                                            active_loader_slug == Some(*c)
                                                        }
                                                        _ => true,
                                                    })
                                                    .collect();
                                                let cats_count = if info_w >= 380.0 {
                                                    display_cats.len().min(3)
                                                } else if info_w >= 300.0 {
                                                    display_cats.len().min(2)
                                                } else {
                                                    0
                                                };
                                                let cats_reserve = if cats_count > 0 {
                                                    cats_count as f32 * 76.0
                                                } else {
                                                    0.0
                                                };
                                                let meta_w =
                                                    (info_w - cats_reserve - 18.0).max(80.0);
                                                let (dl_r, _) = ui.allocate_exact_size(
                                                    Vec2::splat(11.0),
                                                    Sense::hover(),
                                                );
                                                draw_icon(
                                                    ui.painter(),
                                                    dl_r,
                                                    Icon::Download,
                                                    text_muted(),
                                                );
                                                ui.allocate_ui(Vec2::new(meta_w, 20.0), |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(format!(
                                                                "{} • {} {}",
                                                                format_downloads(hit.downloads),
                                                                tr(self.settings.language, "mr.by"),
                                                                hit.author
                                                            ))
                                                            .small()
                                                            .color(text_muted()),
                                                        )
                                                        .truncate(),
                                                    );
                                                });
                                                let cats_text: String = display_cats
                                                    .iter()
                                                    .take(cats_count)
                                                    .copied()
                                                    .collect::<Vec<_>>()
                                                    .join(" • ");
                                                if !cats_text.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(cats_text)
                                                            .small()
                                                            .color(text_muted()),
                                                    );
                                                }
                                            });
                                        },
                                    );

                                    ui.allocate_ui_with_layout(
                                        Vec2::new(action_w, 72.0),
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            if ui_icon_button(
                                                ui,
                                                ("mr_ext_w", &hit.project_id),
                                                Icon::External,
                                                hit.source.label(),
                                                ButtonVariant::Subtle,
                                                true,
                                                30.0,
                                            )
                                            .clicked()
                                            {
                                                let url =
                                                    hit.website_url.clone().unwrap_or_else(|| {
                                                        format!(
                                                            "https://modrinth.com/{}/{}",
                                                            mr_type.api_str(),
                                                            hit.project_id
                                                        )
                                                    });
                                                anim::emit_ring(
                                                    ui.ctx(),
                                                    anim::pointer_or_center(ui.ctx()),
                                                    accent(),
                                                    28.0,
                                                );
                                                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                                            }
                                            if ui_icon_button(
                                                ui,
                                                ("mr_vers_w", &hit.project_id),
                                                Icon::List,
                                                tr(self.settings.language, "mr.browse_versions"),
                                                ButtonVariant::Subtle,
                                                true,
                                                30.0,
                                            )
                                            .clicked()
                                            {
                                                open_detail = Some(hit.clone());
                                            }
                                            if is_this_installing {
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(btn_w, 30.0),
                                                    egui::Layout::centered_and_justified(
                                                        egui::Direction::LeftToRight,
                                                    ),
                                                    |ui| mono_spinner(ui, 18.0),
                                                );
                                            } else if ui_button_sized(
                                                ui,
                                                ("mr_inst_w", &hit.project_id),
                                                Some(btn_icon),
                                                btn_lbl,
                                                btn_variant,
                                                !mr_installing,
                                                Vec2::new(btn_w, 30.0),
                                            )
                                            .clicked()
                                            {
                                                install_req = Some((
                                                    hit.project_id.clone(),
                                                    hit.title.clone(),
                                                    hit.icon_url.clone(),
                                                ));
                                            }
                                        },
                                    );
                                },
                            );
                        } else {
                            let icon_px = 42.0;
                            let info_w = (card_inner_w - icon_px - 10.0).max(80.0);
                            ui_toolbar_row(ui, 44.0, |ui| {
                                let icon_size = Vec2::splat(icon_px);
                                if let Some(tex) = tex_opt {
                                    ui.add(
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(icon_size)
                                            .rounding(Rounding::same(9.0)),
                                    );
                                } else {
                                    let (rect, _) =
                                        ui.allocate_exact_size(icon_size, Sense::hover());
                                    draw_icon_placeholder(
                                        ui,
                                        rect,
                                        9.0,
                                        &hit.title,
                                        is_icon_loading,
                                    );
                                }
                                ui.allocate_ui_with_layout(
                                    Vec2::new(info_w, 42.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui_toolbar_row(ui, 20.0, |ui| {
                                            let badge_w = if is_installed { 72.0 } else { 0.0 }
                                                + if hit.source == ProjectSource::CurseForge {
                                                    36.0
                                                } else {
                                                    0.0
                                                };
                                            ui.allocate_ui(
                                                Vec2::new((info_w - badge_w).max(40.0), 20.0),
                                                |ui| {
                                                    let title_id = ui.make_persistent_id((
                                                        "mr_title_n",
                                                        &hit.project_id,
                                                    ));
                                                    let prev_hov = ui
                                                        .ctx()
                                                        .data(|d| d.get_temp::<bool>(title_id))
                                                        .unwrap_or(false);
                                                    let title_hov =
                                                        ui.ctx().animate_bool_with_time(
                                                            title_id.with("h"),
                                                            prev_hov,
                                                            0.11,
                                                        );
                                                    if title_hov > 0.01 && title_hov < 0.99 {
                                                        ui.ctx().request_repaint();
                                                    }
                                                    let title_col = lerp_color(
                                                        text_primary(),
                                                        accent_text(),
                                                        title_hov,
                                                    );
                                                    let title_resp = ui
                                                        .add(
                                                            egui::Label::new(
                                                                egui::RichText::new(&hit.title)
                                                                    .strong()
                                                                    .color(title_col),
                                                            )
                                                            .truncate()
                                                            .sense(Sense::click()),
                                                        )
                                                        .on_hover_cursor(
                                                            egui::CursorIcon::PointingHand,
                                                        )
                                                        .on_hover_text(tr(
                                                            self.settings.language,
                                                            "mr.click_versions",
                                                        ));
                                                    ui.ctx().data_mut(|d| {
                                                        d.insert_temp(
                                                            title_id,
                                                            title_resp.hovered(),
                                                        )
                                                    });
                                                    if title_resp.clicked() {
                                                        open_detail = Some(hit.clone());
                                                    }
                                                },
                                            );
                                            if hit.source == ProjectSource::CurseForge {
                                                badge(ui, "CF", false);
                                            }
                                            if is_installed {
                                                badge(
                                                    ui,
                                                    tr(self.settings.language, "mr.installed"),
                                                    true,
                                                );
                                            }
                                        });
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "{} • {}",
                                                    format_downloads(hit.downloads),
                                                    hit.author
                                                ))
                                                .small()
                                                .color(text_muted()),
                                            )
                                            .truncate(),
                                        );
                                    },
                                );
                            });
                            ui.add_space(5.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(clamp_summary(&hit.description, 90))
                                        .small()
                                        .color(text_secondary()),
                                )
                                .wrap(),
                            );
                            ui.add_space(6.0);
                            ui_toolbar_row(ui, 30.0, |ui| {
                                if is_this_installing {
                                    mono_spinner(ui, 16.0);
                                } else if ui_button_sized(
                                    ui,
                                    ("mr_inst_n", &hit.project_id),
                                    Some(btn_icon),
                                    btn_lbl,
                                    btn_variant,
                                    !mr_installing,
                                    Vec2::new((card_inner_w - 44.0).max(60.0), 30.0),
                                )
                                .clicked()
                                {
                                    install_req = Some((
                                        hit.project_id.clone(),
                                        hit.title.clone(),
                                        hit.icon_url.clone(),
                                    ));
                                }
                                if ui_icon_button(
                                    ui,
                                    ("mr_vers_n", &hit.project_id),
                                    Icon::List,
                                    tr(self.settings.language, "mr.browse_versions"),
                                    ButtonVariant::Subtle,
                                    true,
                                    30.0,
                                )
                                .clicked()
                                {
                                    open_detail = Some(hit.clone());
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                }

                if !self.mr_results.is_empty() && self.mr_has_more {
                    ui.add_space(4.0);
                    ui_wrap_row(ui, 30.0, |ui| {
                        if ui_button_icon(
                            ui,
                            "mr_load_more_btn",
                            Some(Icon::Plus),
                            tr(self.settings.language, "btn.load_more"),
                            ButtonVariant::Secondary,
                            !self.mr_searching,
                            30.0,
                        )
                        .clicked()
                        {
                            load_more = true;
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                self.mr_results.len(),
                                tr(self.settings.language, "mr.compatible"),
                            ))
                            .small()
                            .color(text_muted()),
                        );
                    });
                }
            });
        });

        if open_inst_settings_req {
            self.instance_tab = InstanceSubTab::Settings;
            self.switch_page(ui.ctx(), NavPage::Instance);
        }

        if let Some(hit) = open_detail {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
            self.open_project_details(&hit);
        }

        if load_more {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
            self.trigger_modrinth_search(true);
        }

        if let Some((project_id, title, icon_url)) = install_req {
            let cancel_tok = Arc::new(AtomicBool::new(false));
            self.mr_cancel_token = Some(Arc::clone(&cancel_tok));
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 34.0);
            if let Some(srv) = target_server.or_else(|| {
                if mr_type == ModrinthProjectType::Plugin {
                    self.servers.get(self.selected_server_idx).cloned()
                } else {
                    None
                }
            }) {
                self.mr_installing = true;
                self.mr_installing_id = Some(project_id.clone());
                self.mr_error = None;
                self.mr_status = Some(format!("Installing {title} -> {}...", srv.name));
                self.progress = 0.05;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                let cf_key = self.settings.curseforge_api_key.clone();
                thread::spawn(move || {
                    let res = with_download_cancel_token(cancel_tok, || {
                        if let Some(cf_mod_id) = parse_cf_project_id(&project_id) {
                            install_curseforge_server_project(
                                &cf_key,
                                &paths,
                                &srv,
                                cf_mod_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                None,
                                &tx,
                            )
                        } else {
                            install_server_project(
                                &paths,
                                &srv,
                                &project_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                &tx,
                            )
                        }
                    });
                    let _ = tx.send(WorkerEvent::ServerProjectInstalled(res));
                });
            } else if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                self.mr_installing = true;
                self.mr_installing_id = Some(project_id.clone());
                self.mr_error = None;
                self.mr_status = Some(format!("Installing {title}..."));
                self.progress = 0.05;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                let cf_key = self.settings.curseforge_api_key.clone();
                thread::spawn(move || {
                    let res = with_download_cancel_token(cancel_tok, || {
                        if let Some(cf_mod_id) = parse_cf_project_id(&project_id) {
                            install_curseforge_project(
                                &cf_key,
                                &paths,
                                &profile,
                                cf_mod_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                None,
                                &tx,
                            )
                        } else {
                            install_project(
                                &paths,
                                &profile,
                                &project_id,
                                mr_type,
                                &title,
                                icon_url.as_deref(),
                                &tx,
                            )
                        }
                    });
                    let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
                });
            }
        }
    }
}
