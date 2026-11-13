#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod icons;
mod launcher;
mod models;
mod modrinth;
mod mojang;
mod storage;
mod theme;
mod ui;

use eframe::egui;
use storage::{get_launcher_paths, load_accounts, load_profiles};
use ui::McLauncherApp;

fn main() -> eframe::Result<()> {
    let paths = get_launcher_paths().expect("Failed to initialize launcher directories");
    let profiles = load_profiles(&paths);
    let accounts = load_accounts(&paths);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1060.0, 680.0])
            .with_min_inner_size([440.0, 380.0])
            .with_icon(std::sync::Arc::new(icons::build_window_icon()))
            .with_title("Svarka Launcher"),
        ..Default::default()
    };

    eframe::run_native(
        "Svarka Launcher",
        options,
        Box::new(move |cc| {
            theme::apply_modrinth_theme(&cc.egui_ctx);
            Ok(Box::new(McLauncherApp::new(paths, profiles, accounts)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::launcher::offline_uuid;
    use super::models::{OsRule, Rule};
    use super::mojang::{current_os_name, maven_to_path, rules_allow};

    #[test]
    fn test_maven_to_path_basic() {
        assert_eq!(
            maven_to_path("com.mojang:brigadier:1.2.9"),
            Some("com/mojang/brigadier/1.2.9/brigadier-1.2.9.jar".to_string())
        );
    }

    #[test]
    fn test_maven_to_path_classifier_and_ext() {
        assert_eq!(
            maven_to_path("org.lwjgl:lwjgl:3.3.3:natives-linux"),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar".to_string())
        );
        assert_eq!(
            maven_to_path("net.fabricmc:intermediary:1.21.4:v2@jar"),
            Some("net/fabricmc/intermediary/1.21.4/intermediary-1.21.4-v2.jar".to_string())
        );
    }

    #[test]
    fn test_offline_uuid_deterministic() {
        let u1 = offline_uuid("Steve");
        let u2 = offline_uuid("Steve");
        assert_eq!(u1, u2);
        assert_eq!(u1.len(), 32);
        assert_ne!(u1, offline_uuid("Alex"));
    }

    #[test]
    fn test_rules_allow_current_os() {
        let rules = vec![Rule {
            action: "allow".to_string(),
            os: Some(OsRule {
                name: Some(current_os_name().to_string()),
                arch: None,
            }),
            features: None,
        }];
        assert!(rules_allow(Some(&rules)));

        let deny_rules = vec![Rule {
            action: "disallow".to_string(),
            os: Some(OsRule {
                name: Some(current_os_name().to_string()),
                arch: None,
            }),
            features: None,
        }];
        assert!(!rules_allow(Some(&deny_rules)));
    }
}

