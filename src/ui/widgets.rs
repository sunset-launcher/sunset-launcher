use super::*;

pub(super) fn score_palette_match(query: &str, title: &str, subtitle: &str, keywords: &str) -> Option<i32> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    let title_l = title.to_lowercase();
    let sub_l = subtitle.to_lowercase();
    let kw_l = keywords.to_lowercase();
    let mut total = 0_i32;

    for token in trimmed.split_whitespace() {
        let tok = token.to_lowercase();
        if title_l.starts_with(&tok) {
            total += 120;
        } else if title_l
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(&tok))
        {
            total += 80;
        } else if title_l.contains(&tok) {
            total += 50;
        } else if sub_l.contains(&tok) || kw_l.contains(&tok) {
            total += 25;
        } else if is_subsequence(&tok, &title_l) {
            total += 15;
        } else {
            return None;
        }
    }
    Some(total)
}

pub(super) fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut it = haystack.chars();
    for ch in needle.chars() {
        if !it.any(|h| h == ch) {
            return false;
        }
    }
    true
}

/// Renders a vertical scroll area strictly clamped to `ui.available_height()` so content
/// can never expand past the bottom status bar or clip outside the viewport.
pub(super) fn bounded_scroll<R>(
    ui: &mut egui::Ui,
    id_salt: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let max_h = ui.available_height().max(0.0);
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .max_height(max_h)
        .show(ui, add_contents)
}

pub(super) fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

pub(super) fn format_downloads(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

pub(super) fn format_relative_time(iso_opt: Option<&str>, lang: Language) -> Option<String> {
    let iso = iso_opt?;
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let secs = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds();
    if secs < 0 {
        return Some(tr(lang, "time.just_now").to_string());
    }
    if secs < 60 {
        Some(tr(lang, "time.just_now").to_string())
    } else if secs < 3600 {
        Some(format!("{}{}", secs / 60, tr(lang, "time.m_ago")))
    } else if secs < 86400 {
        Some(format!("{}{}", secs / 3600, tr(lang, "time.h_ago")))
    } else {
        Some(format!("{}{}", secs / 86400, tr(lang, "time.d_ago")))
    }
}

pub(super) fn format_playtime(secs: u64, lang: Language) -> String {
    let m = tr(lang, "time.m_short");
    let h = tr(lang, "time.h_short");
    if secs == 0 {
        format!("0{m}")
    } else if secs < 60 {
        format!("<1{m}")
    } else if secs < 3600 {
        format!("{}{m}", secs / 60)
    } else {
        let hours = secs / 3600;
        let rem_mins = (secs % 3600) / 60;
        if rem_mins == 0 {
            format!("{hours}{h}")
        } else {
            format!("{hours}{h} {rem_mins}{m}")
        }
    }
}

pub(super) fn diagnose_console_logs(logs: &VecDeque<LogEntry>) -> Option<String> {
    for entry in logs.iter().rev().take(300) {
        let line = entry.text.as_str();
        if line.contains("OutOfMemoryError")
            || line.contains("Java heap space")
            || line.contains("GC overhead limit exceeded")
        {
            return Some(
                "Crash Diagnostic — Out of Memory: increase allocated RAM in Instance Settings."
                    .to_string(),
            );
        }
        if line.contains("UnsupportedClassVersionError")
            || line.contains("compiled by a more recent version of the Java Runtime")
        {
            return Some(
                "Crash Diagnostic — Java Version Mismatch: configure a newer Java runtime (e.g. Java 17 or 21) in Instance Settings."
                    .to_string(),
            );
        }
        if line.contains("ModResolutionException")
            || line.contains("Could not find required mod")
            || line.contains("Missing or unsupported mandatory dependencies")
        {
            return Some(
                "Crash Diagnostic — Missing Mod Dependency: check the Content tab and install required library mods (such as Fabric API)."
                    .to_string(),
            );
        }
        if line.contains("MixinApplyError")
            || line.contains("MixinTransformerError")
            || line.contains("critical injection failure")
        {
            return Some(
                "Crash Diagnostic — Mixin Conflict: a mod failed during bytecode patching. Try toggling off recently added or incompatible mods."
                    .to_string(),
            );
        }
    }
    None
}

pub(super) fn is_image_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico")
    )
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(super) fn build_win32_file_filter_utf16(filter_label: &str, exts: &[&str]) -> Vec<u16> {
    let patterns: Vec<String> = exts.iter().map(|e| format!("*.{e}")).collect();
    let joined = if patterns.is_empty() {
        "*.*".to_string()
    } else {
        patterns.join(";")
    };
    let clean_label = filter_label.replace('\0', "");
    format!("{clean_label} ({joined})\0{joined}\0\0")
        .encode_utf16()
        .collect()
}

pub(super) fn pick_file(title: &str, filter_label: &str, exts: &[&str]) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::io::ErrorKind;
        let patterns: Vec<String> = exts.iter().map(|e| format!("*.{e}")).collect();
        let joined = patterns.join(" ");
        let filter_arg = format!("--file-filter={filter_label} | {joined}");
        let title_arg = format!("--title={title}");
        for bin in ["zenity", "qarma"] {
            match std::process::Command::new(bin)
                .args(["--file-selection", &title_arg, &filter_arg])
                .output()
            {
                Ok(out) => {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !s.is_empty() {
                            return Some(std::path::PathBuf::from(s));
                        }
                    }
                    return None;
                }
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(_) => continue,
            }
        }
        match std::process::Command::new("yad")
            .args(["--file", &title_arg, &filter_arg])
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Some(std::path::PathBuf::from(s));
                    }
                }
                return None;
            }
            Err(_) => {}
        }
        match std::process::Command::new("kdialog")
            .args(["--title", title, "--getopenfilename", ".", &joined])
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Some(std::path::PathBuf::from(s));
                    }
                }
                return None;
            }
            Err(_) => {}
        }
        let py_script = format!(
            "import tkinter as tk; from tkinter import filedialog; r = tk.Tk(); r.withdraw(); p = filedialog.askopenfilename(title={title:?}, filetypes=[({filter_label:?}, {joined:?})]); print(p or '')"
        );
        if let Ok(out) = std::process::Command::new("python3")
            .args(["-c", &py_script])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        use std::ptr;

        #[repr(C)]
        struct OpenFileNameW {
            l_struct_size: u32,
            hwnd_owner: *mut c_void,
            h_instance: *mut c_void,
            lpstr_filter: *const u16,
            lpstr_custom_filter: *mut u16,
            n_max_cust_filter: u32,
            n_filter_index: u32,
            lpstr_file: *mut u16,
            n_max_file: u32,
            lpstr_file_title: *mut u16,
            n_max_file_title: u32,
            lpstr_initial_dir: *const u16,
            lpstr_title: *const u16,
            flags: u32,
            n_file_offset: u16,
            n_file_extension: u16,
            lpstr_def_ext: *const u16,
            l_cust_data: usize,
            lpfn_hook: *const c_void,
            lp_template_name: *const u16,
            pv_reserved: *mut c_void,
            dw_reserved: u32,
            flags_ex: u32,
        }

        #[link(name = "comdlg32")]
        extern "system" {
            fn GetOpenFileNameW(lpofn: *mut OpenFileNameW) -> i32;
        }
        #[link(name = "user32")]
        extern "system" {
            fn GetActiveWindow() -> *mut c_void;
        }

        const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
        const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
        const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
        const OFN_EXPLORER: u32 = 0x0008_0000;

        let filter_w = build_win32_file_filter_utf16(filter_label, exts);
        let title_w: Vec<u16> = title
            .replace('\0', "")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut file_buf = vec![0u16; 32768];

        let mut ofn = OpenFileNameW {
            l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
            hwnd_owner: unsafe { GetActiveWindow() },
            h_instance: ptr::null_mut(),
            lpstr_filter: filter_w.as_ptr(),
            lpstr_custom_filter: ptr::null_mut(),
            n_max_cust_filter: 0,
            n_filter_index: 1,
            lpstr_file: file_buf.as_mut_ptr(),
            n_max_file: file_buf.len() as u32,
            lpstr_file_title: ptr::null_mut(),
            n_max_file_title: 0,
            lpstr_initial_dir: ptr::null(),
            lpstr_title: title_w.as_ptr(),
            flags: OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST | OFN_EXPLORER,
            n_file_offset: 0,
            n_file_extension: 0,
            lpstr_def_ext: ptr::null(),
            l_cust_data: 0,
            lpfn_hook: ptr::null(),
            lp_template_name: ptr::null(),
            pv_reserved: ptr::null_mut(),
            dw_reserved: 0,
            flags_ex: 0,
        };

        let ok = unsafe { GetOpenFileNameW(&mut ofn) };
        if ok != 0 {
            let end = file_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(file_buf.len());
            let s = String::from_utf16_lossy(&file_buf[..end]).trim().to_string();
            if !s.is_empty() {
                return Some(std::path::PathBuf::from(s));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (title, filter_label);
        let types: Vec<String> = exts.iter().map(|e| format!("\"{e}\"")).collect();
        let script = format!("POSIX path of (choose file of type {{{}}})", types.join(", "));
        if let Ok(out) = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    None
}

pub(super) fn draw_account_avatar(
    painter: &egui::Painter,
    rect: Rect,
    account: Option<&Account>,
    fallback_name: &str,
    fallback_col: Color32,
    skin_heads: &HashMap<String, Option<egui::TextureHandle>>,
) {
    if let Some(acc) = account {
        let key = account_skin_cache_key(acc);
        if let Some(Some(tex)) = skin_heads.get(&key) {
            let rounding = Rounding::same((rect.width() * 0.2).clamp(3.0, 6.0));
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, with_alpha(fallback_col, 140)));
            return;
        }
    }
    draw_player_pixel_avatar(painter, rect, fallback_name, fallback_col);
}

pub(super) fn draw_profile_avatar(
    painter: &egui::Painter,
    rect: Rect,
    profile: &Profile,
    icons: &HashMap<String, Option<egui::TextureHandle>>,
    selected: bool,
    is_running: bool,
) {
    let key = profile.icon_path.trim();
    if let Some(Some(tex)) = (!key.is_empty()).then(|| icons.get(key)).flatten() {
        draw_instance_image_avatar(painter, rect, tex.id(), selected, is_running);
    } else {
        draw_instance_avatar(
            painter,
            rect,
            &profile.name,
            profile.loader.kind(),
            selected,
            is_running,
        );
    }
}

pub(super) fn draw_server_card_avatar(
    painter: &egui::Painter,
    rect: Rect,
    srv: &DedicatedServer,
    icon_tex: Option<&egui::TextureHandle>,
    selected: bool,
    is_running: bool,
) {
    if let Some(tex) = icon_tex {
        draw_instance_image_avatar(painter, rect, tex.id(), selected, is_running);
    } else {
        draw_server_avatar(
            painter,
            rect,
            &srv.name,
            srv.software,
            selected,
            is_running,
        );
    }
}

pub(super) fn apply_profile_icon_from_file(
    ctx: &egui::Context,
    paths: &LauncherPaths,
    icons: &mut HashMap<String, Option<egui::TextureHandle>>,
    profile: &mut Profile,
    src: &std::path::Path,
) -> Result<(), String> {
    let bytes = std::fs::read(src).map_err(|e| format!("Failed to read image: {e}"))?;
    let img = decode_icon_bytes(&bytes)?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico"))
        .unwrap_or_else(|| "png".to_string());
    let inst_dir = instance_dir(paths, profile);
    let _ = std::fs::create_dir_all(&inst_dir);
    let dest = inst_dir.join(format!("icon.{ext}"));
    let stored_path = if std::fs::write(&dest, &bytes).is_ok() {
        dest
    } else {
        src.to_path_buf()
    };
    let old_key = profile.icon_path.trim().to_string();
    if !old_key.is_empty() {
        icons.remove(&old_key);
    }
    let new_key = stored_path.to_string_lossy().to_string();
    let tex = ctx.load_texture(
        format!("profile_icon:{new_key}"),
        egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba),
        egui::TextureOptions::LINEAR,
    );
    icons.insert(new_key.clone(), Some(tex));
    profile.icon_path = new_key;
    Ok(())
}

pub(super) fn render_instance_avatar_controls(
    ui: &mut egui::Ui,
    lang: Language,
    id_salt: &str,
    profile: &Profile,
    icons: &HashMap<String, Option<egui::TextureHandle>>,
    request_pick: &mut bool,
    reset_icon: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let av_id = ui.make_persistent_id((id_salt, "avatar_click"));
        let (av_rect, av_resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
        let hov_t = ui.ctx().animate_bool_with_time(av_id.with("hov"), av_resp.hovered(), 0.12);
        let prs_t = ui.ctx().animate_bool_with_time(
            av_id.with("prs"),
            av_resp.is_pointer_button_down_on(),
            0.07,
        );
        if (hov_t > 0.01 && hov_t < 0.99) || (prs_t > 0.01 && prs_t < 0.99) {
            ui.ctx().request_repaint();
        }
        let draw_av = av_rect.shrink(prs_t * 1.0);
        draw_profile_avatar(ui.painter(), draw_av, profile, icons, true, false);
        if hov_t > 0.03 {
            ui.painter().rect_stroke(
                draw_av.expand(1.5),
                Rounding::same(9.5),
                Stroke::new(1.3_f32, with_alpha(accent(), (hov_t * 185.0) as u8)),
            );
        }
        if let Some((ct, origin)) = track_click_anim(
            ui.ctx(),
            av_id,
            draw_av,
            av_resp.clicked(),
            av_resp.interact_pointer_pos(),
            0.38,
        ) {
            paint_click_ripple(
                ui.painter(),
                draw_av,
                Rounding::same(8.0),
                origin,
                ct,
                accent(),
                true,
            );
        }
        if av_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if av_resp.clicked() {
            anim::emit_ring(ui.ctx(), draw_av.center(), accent(), 26.0);
            *request_pick = true;
        }
        if ui_button_icon(
            ui,
            (id_salt, "choose"),
            Some(Icon::Folder),
            tr(lang, "inst.choose_icon"),
            ButtonVariant::Secondary,
            true,
            28.0,
        )
        .clicked()
        {
            *request_pick = true;
        }
        if !profile.icon_path.trim().is_empty()
            && ui_button_icon(
                ui,
                (id_salt, "reset"),
                Some(Icon::Trash),
                tr(lang, "action.delete"),
                ButtonVariant::Subtle,
                true,
                28.0,
            )
            .clicked()
        {
            *reset_icon = true;
        }
    });
}

pub(super) fn clamp_summary(s: &str, max_chars: usize) -> String {
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = normalized.chars().count();
    if count <= max_chars || max_chars <= 3 {
        return normalized;
    }
    let mut out: String = normalized.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

pub(super) fn render_loader_kind_pills(ui: &mut egui::Ui, id_prefix: &str, loader: &mut ModLoader) -> bool {
    let mut changed = false;
    let current = loader.kind();
    ui_flow_row(ui, 28.0, |ui| {
        for kind in LoaderKind::ALL {
            let active = current == kind;
            if ui_loader_pill(ui, (id_prefix, kind.label()), kind, active).clicked() && !active {
                *loader = ModLoader::from_kind(kind, String::new());
                changed = true;
            }
        }
    });
    changed
}

pub(super) fn render_loader_version_picker(
    ui: &mut egui::Ui,
    combo_id: &str,
    width: f32,
    kind: LoaderKind,
    loader_version: &mut String,
    list: Option<&[LoaderVersionEntry]>,
) -> bool {
    let mut changed = false;
    ui_wrap_row(ui, 30.0, |ui| {
        let label = if loader_version.is_empty() {
            "Auto".to_string()
        } else {
            loader_version.clone()
        };
        egui::ComboBox::from_id_salt(combo_id)
            .selected_text(label)
            .width(width)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(loader_version, String::new(), "Auto")
                    .changed()
                {
                    changed = true;
                }
                if let Some(entries) = list {
                    if entries.is_empty() {
                        ui.label(format!("No {} builds for this MC version", kind.label()));
                    } else {
                        for item in entries.iter().take(40) {
                            let tag = if item.stable {
                                format!("{} (recommended)", item.version)
                            } else {
                                item.version.clone()
                            };
                            if ui
                                .selectable_value(loader_version, item.version.clone(), tag)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    }
                } else {
                    ui.label(format!("Loading {} versions...", kind.label()));
                }
            });
        if list.is_none() {
            mono_spinner(ui, 16.0);
        }
    });
    changed
}
