use super::*;

impl McLauncherApp {
    pub(super) fn render_library_page(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let avail_w = ui.available_width();
        let wide_hdr = avail_w >= 620.0;
        let search_w = (avail_w - 340.0).clamp(110.0, 240.0);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.heading(egui::RichText::new(tr(lang, "lib.title")).size(21.0).color(text_primary()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui_button_icon(
                    ui,
                    "new_instance_btn",
                    Some(Icon::Plus),
                    tr(lang, "btn.create"),
                    ButtonVariant::Primary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    self.show_new_modal = true;
                }
                if wide_hdr {
                    ui_search_input(
                        ui,
                        "lib_search_w",
                        &mut self.library_search,
                        tr(lang, "lib.search"),
                        search_w,
                    );
                }
            });
        });

        ui.add_space(6.0);
        if !wide_hdr {
            ui_toolbar_row(ui, 30.0, |ui| {
                ui_search_input(
                    ui,
                    "lib_search_n",
                    &mut self.library_search,
                    tr(lang, "lib.search"),
                    (avail_w - 10.0).max(120.0),
                );
            });
            ui.add_space(6.0);
        }

        // Filter, Sort & View Mode bar
        {
            let total_cnt = self.profiles.len();
            let modded_cnt = self.profiles.iter().filter(|p| p.loader.is_modded()).count();
            let vanilla_cnt = total_cnt.saturating_sub(modded_cnt);
            let filter_label = |f: LibraryFilter| match f {
                LibraryFilter::All => format!("{} ({total_cnt})", tr(lang, "lib.all")),
                LibraryFilter::Modded => format!("{} ({modded_cnt})", tr(lang, "lib.modded")),
                LibraryFilter::Vanilla => format!("{} ({vanilla_cnt})", tr(lang, "lib.vanilla")),
            };
            ui_wrap_row(ui, 30.0, |ui| {
                let prev_filter = self.library_filter;
                egui::ComboBox::from_id_salt("lib_filter_combo")
                    .selected_text(filter_label(self.library_filter))
                    .width(136.0)
                    .show_ui(ui, |ui| {
                        for f in [
                            LibraryFilter::All,
                            LibraryFilter::Modded,
                            LibraryFilter::Vanilla,
                        ] {
                            ui.selectable_value(&mut self.library_filter, f, filter_label(f));
                        }
                    });
                if self.library_filter != prev_filter {
                    self.mark_subtab_switch(ui.ctx());
                }
                let prev_sort = self.library_sort;
                egui::ComboBox::from_id_salt("lib_sort_combo")
                    .selected_text(match self.library_sort {
                        LibrarySort::Recent => tr(lang, "lib.sort_recent"),
                        LibrarySort::Name => tr(lang, "lib.sort_name"),
                        LibrarySort::Playtime => tr(lang, "lib.sort_playtime"),
                    })
                    .width(136.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Recent,
                            tr(lang, "lib.sort_recent"),
                        );
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Name,
                            tr(lang, "lib.sort_name"),
                        );
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Playtime,
                            tr(lang, "lib.sort_playtime"),
                        );
                    });
                if self.library_sort != prev_sort {
                    self.mark_subtab_switch(ui.ctx());
                }
                if ui_icon_button(
                    ui,
                    "lib_view_grid",
                    Icon::Grid,
                    tr(self.settings.language, "lib.grid_view"),
                    if self.library_view == LibraryViewMode::Grid {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Subtle
                    },
                    true,
                    28.0,
                )
                .clicked()
                    && self.library_view != LibraryViewMode::Grid
                {
                    self.library_view = LibraryViewMode::Grid;
                    self.mark_subtab_switch(ui.ctx());
                }
                if ui_icon_button(
                    ui,
                    "lib_view_list",
                    Icon::List,
                    tr(self.settings.language, "lib.list_view"),
                    if self.library_view == LibraryViewMode::List {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Subtle
                    },
                    true,
                    28.0,
                )
                .clicked()
                    && self.library_view != LibraryViewMode::List
                {
                    self.library_view = LibraryViewMode::List;
                    self.mark_subtab_switch(ui.ctx());
                }
            });
            ui.add_space(8.0);
        }

        let query = self.library_search.trim().to_lowercase();
        let mut action_select: Option<usize> = None;
        let mut action_play: Option<usize> = None;
        let mut action_stop: Option<String> = None;
        let mut action_open: Option<usize> = None;
        let mut action_delete: Option<usize> = None;
        let can_delete = self.profiles.len() > 1;
        let subtab_t = self.subtab_switch_time;
        let mut reset_lib_filters = false;

        Self::apply_subtab_transition(ui, subtab_t, |ui| {
        bounded_scroll(ui, "library_grid_scroll", |ui| {
            let usable_w = (ui.available_width() - 12.0).max(160.0);

            let cols = if usable_w >= 980.0 {
                3
            } else if usable_w >= 560.0 {
                2
            } else {
                1
            };
            let gap = 10.0;
            let card_w = ((usable_w - gap * (cols as f32 - 1.0)) / cols as f32)
                .floor()
                .max(150.0);

            let mut visible: Vec<(usize, Profile)> = self
                .profiles
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    query.is_empty()
                        || p.name.to_lowercase().contains(&query)
                        || p.version_id.to_lowercase().contains(&query)
                })
                .filter(|(_, p)| match self.library_filter {
                    LibraryFilter::All => true,
                    LibraryFilter::Modded => p.loader.is_modded(),
                    LibraryFilter::Vanilla => !p.loader.is_modded(),
                })
                .map(|(i, p)| (i, p.clone()))
                .collect();

            match self.library_sort {
                LibrarySort::Recent => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| b.last_played.cmp(&a.last_played))
                    });
                }
                LibrarySort::Name => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                }
                LibrarySort::Playtime => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| b.playtime_secs.cmp(&a.playtime_secs))
                    });
                }
            }

            if visible.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    let inner_w = (usable_w - 22.0).max(110.0);
                    ui.set_min_width(inner_w);
                    ui.set_max_width(inner_w);
                    ui_wrap_row(ui, 30.0, |ui| {
                        ui.label(
                            egui::RichText::new(tr(lang, "lib.no_match"))
                                .color(text_secondary()),
                        );
                        if ui_button(
                            ui,
                            "lib_clear_filters",
                            tr(self.settings.language, "btn.reset_filters"),
                            ButtonVariant::Secondary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            reset_lib_filters = true;
                        }
                    });
                });
                return;
            }

            if self.library_view == LibraryViewMode::List {
                let list_inner_w = (usable_w - 22.0).max(150.0);
                for (idx, p) in &visible {
                    let selected = *idx == self.selected_idx;
                    let is_running = self.is_profile_running(&p.id);
                    let is_launching = self.is_profile_launching(&p.id);
                    let (total_mods, enabled_mods) = self.profile_mod_counts(p);

                    let row_id = ui.make_persistent_id(("lib_list_hover", &p.id));
                    animated_card(ui, row_id, selected, |ui| {
                        ui.set_min_width(list_inner_w);
                        ui.set_max_width(list_inner_w);
                        let wide_list_row = list_inner_w >= 500.0;
                        if wide_list_row {
                            ui.allocate_ui_with_layout(
                                Vec2::new(list_inner_w, 38.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    if can_delete
                                        && ui_icon_button(
                                            ui,
                                            ("list_del", &p.id),
                                            Icon::Trash,
                                            tr(self.settings.language, "lib.delete"),
                                            ButtonVariant::Danger,
                                            true,
                                            28.0,
                                        )
                                        .clicked()
                                    {
                                        action_delete = Some(*idx);
                                    }
                                    if ui_icon_button(
                                        ui,
                                        ("list_cfg", &p.id),
                                        Icon::Sliders,
                                        tr(self.settings.language, "lib.settings"),
                                        ButtonVariant::Secondary,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_open = Some(*idx);
                                    }
                                    if is_running {
                                        if ui_button_icon(
                                            ui,
                                            ("list_stop", &p.id),
                                            Some(Icon::Stop),
                                            tr(lang, "top.stop"),
                                            ButtonVariant::Danger,
                                            true,
                                            28.0,
                                        )
                                        .clicked()
                                        {
                                            action_stop = Some(p.id.clone());
                                        }
                                    } else if ui_button_icon(
                                        ui,
                                        ("list_play", &p.id),
                                        Some(Icon::Play),
                                        tr(lang, "btn.play"),
                                        if selected {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        },
                                        !is_launching && !self.is_busy && !self.update_installing,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_play = Some(*idx);
                                    }

                                    let left_w = (ui.available_width() - 6.0).max(60.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(left_w, 38.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.set_max_width(left_w);
                                            ui.spacing_mut().item_spacing.x = 8.0;
                                            let (av_r, _) = ui.allocate_exact_size(
                                                Vec2::splat(34.0),
                                                Sense::hover(),
                                            );
                                            draw_profile_avatar(
                                                ui.painter(),
                                                av_r,
                                                p,
                                                &self.profile_icons,
                                                selected,
                                                is_running,
                                            );
                                            if is_running {
                                                draw_running_pulse_dot(
                                                    ui,
                                                    Pos2::new(
                                                        av_r.right() - 5.5,
                                                        av_r.bottom() - 5.5,
                                                    ),
                                                    3.1,
                                                    accent(),
                                                    bg_card(),
                                                );
                                            }
                                            let mid_w = (left_w - 44.0).max(40.0);
                                            let info_resp = ui
                                                .allocate_ui_with_layout(
                                                    Vec2::new(mid_w, 36.0),
                                                    egui::Layout::top_down(egui::Align::Min),
                                                    |ui| {
                                                        ui.set_max_width(mid_w);
                                                        ui_toolbar_row(ui, 18.0, |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(&p.name)
                                                                        .strong()
                                                                        .size(14.0)
                                                                        .color(text_primary()),
                                                                )
                                                                .truncate(),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(&p.version_id)
                                                                    .monospace()
                                                                    .small()
                                                                    .color(text_muted()),
                                                            );
                                                            loader_badge(
                                                                ui,
                                                                p.loader.kind(),
                                                                p.loader.short_name(),
                                                            );
                                                        });
                                                        let mut meta = format!(
                                                            "{enabled_mods}/{total_mods} {}",
                                                            tr(lang, "lib.mods_count")
                                                        );
                                                        if p.playtime_secs > 0 {
                                                            meta.push_str(&format!(
                                                                " • {} {}",
                                                                format_playtime(
                                                                    p.playtime_secs,
                                                                    lang
                                                                ),
                                                                tr(lang, "lib.played")
                                                            ));
                                                        }
                                                        if let Some(rel) = format_relative_time(
                                                            p.last_played.as_deref(),
                                                            lang,
                                                        ) {
                                                            meta.push_str(&format!(" • {rel}"));
                                                        }
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(meta)
                                                                    .small()
                                                                    .color(text_muted()),
                                                            )
                                                            .truncate(),
                                                        );
                                                    },
                                                )
                                                .response
                                                .interact(Sense::click());
                                            if info_resp.hovered() {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                            }
                                            if info_resp.clicked() {
                                                action_select = Some(*idx);
                                            }
                                        },
                                    );
                                },
                            );
                        } else {
                            let mid_w = (list_inner_w - 46.0).max(60.0);
                            ui_toolbar_row(ui, 38.0, |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let (av_r, _) =
                                    ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                draw_profile_avatar(
                                    ui.painter(),
                                    av_r,
                                    p,
                                    &self.profile_icons,
                                    selected,
                                    is_running,
                                );
                                if is_running {
                                    draw_running_pulse_dot(
                                        ui,
                                        Pos2::new(av_r.right() - 5.5, av_r.bottom() - 5.5),
                                        3.1,
                                        accent(),
                                        bg_card(),
                                    );
                                }
                                let info_resp = ui
                                    .allocate_ui_with_layout(
                                        Vec2::new(mid_w, 36.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_max_width(mid_w);
                                            ui_toolbar_row(ui, 18.0, |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&p.name)
                                                            .strong()
                                                            .size(14.0)
                                                            .color(text_primary()),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.label(
                                                    egui::RichText::new(&p.version_id)
                                                        .monospace()
                                                        .small()
                                                        .color(text_muted()),
                                                );
                                            });
                                            let mut meta = format!(
                                                "{enabled_mods}/{total_mods} {}",
                                                tr(lang, "lib.mods_count")
                                            );
                                            if p.playtime_secs > 0 {
                                                meta.push_str(&format!(
                                                    " • {} {}",
                                                    format_playtime(p.playtime_secs, lang),
                                                    tr(lang, "lib.played")
                                                ));
                                            }
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(meta)
                                                        .small()
                                                        .color(text_muted()),
                                                )
                                                .truncate(),
                                            );
                                        },
                                    )
                                    .response
                                    .interact(Sense::click());
                                if info_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if info_resp.clicked() {
                                    action_select = Some(*idx);
                                }
                            });
                            ui.add_space(6.0);
                            ui_wrap_row(ui, 28.0, |ui| {
                                if is_running {
                                    if ui_button_icon(
                                        ui,
                                        ("list_stop", &p.id),
                                        Some(Icon::Stop),
                                        tr(lang, "top.stop"),
                                        ButtonVariant::Danger,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                    {
                                        action_stop = Some(p.id.clone());
                                    }
                                } else if ui_button_icon(
                                    ui,
                                    ("list_play", &p.id),
                                    Some(Icon::Play),
                                    tr(lang, "btn.play"),
                                    if selected {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    },
                                    !is_launching && !self.is_busy && !self.update_installing,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_play = Some(*idx);
                                }
                                if ui_icon_button(
                                    ui,
                                    ("list_cfg", &p.id),
                                    Icon::Sliders,
                                    tr(self.settings.language, "lib.settings"),
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_open = Some(*idx);
                                }
                                if can_delete
                                    && ui_icon_button(
                                        ui,
                                        ("list_del", &p.id),
                                        Icon::Trash,
                                        tr(self.settings.language, "lib.delete"),
                                        ButtonVariant::Danger,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                {
                                    action_delete = Some(*idx);
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                }
            } else {
                for chunk in visible.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (idx, p) in chunk {
                        let selected = *idx == self.selected_idx;
                        let is_running = self.is_profile_running(&p.id);
                        let is_launching = self.is_profile_launching(&p.id);
                        let (total_mods, enabled_mods) = self.profile_mod_counts(p);

                        let inner_w = (card_w - 22.0).max(110.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(card_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let card_id = ui.make_persistent_id(("lib_card_hover", &p.id));
                                animated_card(ui, card_id, selected, |ui| {
                                    ui.set_min_width(inner_w);
                                    ui.set_max_width(inner_w);

                                    let hdr_resp = ui
                                        .horizontal(|ui| {
                                            let (icon_r, _) = ui.allocate_exact_size(
                                                Vec2::splat(44.0),
                                                Sense::hover(),
                                            );
                                            draw_profile_avatar(
                                                ui.painter(),
                                                icon_r,
                                                p,
                                                &self.profile_icons,
                                                selected,
                                                is_running,
                                            );
                                            if is_running {
                                                draw_running_pulse_dot(
                                                    ui,
                                                    Pos2::new(icon_r.right() - 5.5, icon_r.bottom() - 5.5),
                                                    3.1,
                                                    accent(),
                                                    bg_card(),
                                                );
                                            }

                                            let meta_w = (inner_w - 56.0).max(60.0);
                                            ui.vertical(|ui| {
                                                ui.set_max_width(meta_w);
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&p.name)
                                                            .strong()
                                                            .size(14.5)
                                                            .color(text_primary()),
                                                    )
                                                    .truncate(),
                                                );
                                                ui_toolbar_row(ui, 20.0, |ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;
                                                    ui.label(
                                                        egui::RichText::new(&p.version_id)
                                                            .monospace()
                                                            .small()
                                                            .color(text_muted()),
                                                    );
                                                    loader_badge(
                                                        ui,
                                                        p.loader.kind(),
                                                        p.loader.short_name(),
                                                    );
                                                });
                                                let mut sub = if is_running {
                                                    tr(lang, "lib.running").to_string()
                                                } else if total_mods == 0 {
                                                    format!("0 {}", tr(lang, "lib.mods_count"))
                                                } else if enabled_mods == total_mods {
                                                    format!("{total_mods} {}", tr(lang, "lib.mods_count"))
                                                } else {
                                                    format!("{enabled_mods}/{total_mods} {}", tr(lang, "lib.mods_count"))
                                                };
                                                if !is_running {
                                                    if p.playtime_secs > 0 {
                                                        sub.push_str(&format!(
                                                            " • {}",
                                                            format_playtime(p.playtime_secs, lang)
                                                        ));
                                                    }
                                                    if let Some(rel) =
                                                        format_relative_time(
                                                            p.last_played.as_deref(),
                                                            lang,
                                                        )
                                                    {
                                                        sub.push_str(&format!(" • {rel}"));
                                                    }
                                                }
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(sub)
                                                            .small()
                                                            .color(if is_running {
                                                                accent()
                                                            } else {
                                                                text_muted()
                                                            }),
                                                    )
                                                    .truncate(),
                                                );
                                            });
                                        })
                                        .response
                                        .interact(Sense::click());
                                    if hdr_resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if hdr_resp.clicked() {
                                        action_select = Some(*idx);
                                    }

                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.add_space(4.0);

                                    ui_toolbar_row(ui, 28.0, |ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        if is_running {
                                            let stop_lbl = if inner_w < 230.0 {
                                                ""
                                            } else {
                                                tr(lang, "top.stop")
                                            };
                                            if ui_button_icon(
                                                ui,
                                                ("card_stop", &p.id),
                                                Some(Icon::Stop),
                                                stop_lbl,
                                                ButtonVariant::Danger,
                                                true,
                                                28.0,
                                            )
                                            .clicked()
                                            {
                                                action_stop = Some(p.id.clone());
                                            }
                                        } else {
                                            let play_variant = if selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            };
                                            let play_lbl = if inner_w < 230.0 {
                                                ""
                                            } else {
                                                tr(lang, "btn.play")
                                            };
                                            if ui_button_icon(
                                                ui,
                                                ("card_play", &p.id),
                                                Some(Icon::Play),
                                                play_lbl,
                                                play_variant,
                                                !is_launching
                                                    && !self.is_busy
                                                    && !self.update_installing,
                                                28.0,
                                            )
                                            .clicked()
                                            {
                                                action_play = Some(*idx);
                                            }
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                if can_delete
                                                    && ui_icon_button(
                                                        ui,
                                                        ("card_del", &p.id),
                                                        Icon::Trash,
                                                        tr(self.settings.language, "lib.delete"),
                                                        ButtonVariant::Danger,
                                                        true,
                                                        28.0,
                                                    )
                                                    .clicked()
                                                {
                                                    action_delete = Some(*idx);
                                                }
                                                if ui_icon_button(
                                                    ui,
                                                    ("card_cfg", &p.id),
                                                    Icon::Sliders,
                                                    tr(self.settings.language, "lib.settings"),
                                                    ButtonVariant::Secondary,
                                                    true,
                                                    28.0,
                                                )
                                                .clicked()
                                                {
                                                    action_open = Some(*idx);
                                                }
                                            },
                                        );
                                    });
                                });
                            },
                        );
                    }
                });
                ui.add_space(gap);
                }
            }
        });
        });

        if reset_lib_filters {
            self.library_search.clear();
            self.library_filter = LibraryFilter::All;
            self.mark_subtab_switch(ui.ctx());
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 28.0);
        }

        if let Some(idx) = action_select {
            self.select_profile(idx);
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 34.0);
        }
        if let Some(idx) = action_open {
            self.select_profile(idx);
            self.instance_tab = InstanceSubTab::Settings;
            self.switch_page(ui.ctx(), NavPage::Instance);
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 30.0);
        }
        if let Some(idx) = action_play {
            self.select_profile(idx);
            anim::emit_burst(ui.ctx(), anim::pointer_or_center(ui.ctx()), accent(), 10);
            anim::emit_edge_pulse(ui.ctx(), accent());
            self.start_launch();
        }
        if let Some(pid) = action_stop {
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 32.0);
            self.stop_profile(&pid);
        }
        if let Some(idx) = action_delete {
            self.confirm_delete_idx = Some(idx);
            anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 30.0);
        }
    }

    pub(super) fn export_current_profile_mrpack(&mut self, ctx: &egui::Context, profile: &Profile) {
        if let Some(idx) = self.profiles.iter().position(|p| p.id == profile.id) {
            self.export_mrpack_profile_idx = Some(idx);
            self.show_export_mrpack_modal = true;
            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 30.0);
        }
    }
}
