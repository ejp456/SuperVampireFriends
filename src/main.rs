mod narrative;
mod net;
mod senses;
mod stats;
mod ui;
mod world;

use bevy::{asset::AssetMetaCheck, prelude::*, window::WindowResolution};
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{PrimaryWindow, WindowLevel};

use narrative::NarrativePlugin;
use net::NetPlugin;
use senses::SensesPlugin;
use stats::StatsPlugin;
use ui::NoirUiPlugin;
use world::WorldPlugin;

fn main() {
    #[cfg(target_arch = "wasm32")]
    let asset_path = "assets".to_string();

    #[cfg(not(target_arch = "wasm32"))]
    let asset_path = if std::path::Path::new("assets/cases/case_01.ron").exists() {
        "assets".to_string()
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string()
    };

    let primary_window = Window {
        title: "Crimson Ledger — Supernatural Noir Detective RPG".to_string(),
        resolution: WindowResolution::new(1360.0, 820.0),
        resizable: true,
        #[cfg(target_arch = "wasm32")]
        canvas: Some("#bevy-canvas".to_string()),
        #[cfg(target_arch = "wasm32")]
        fit_canvas_to_parent: true,
        #[cfg(not(target_arch = "wasm32"))]
        visible: false,
        focused: true,
        ..default()
    };

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.07)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_path,
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(primary_window),
                    ..default()
                }),
        );

    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, force_show_window_after_startup);

    app.add_plugins((
        StatsPlugin,
        SensesPlugin,
        NarrativePlugin,
        NetPlugin,
        WorldPlugin,
        NoirUiPlugin,
    ))
    .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn force_show_window_after_startup(
    mut frame: Local<u8>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if *frame > 4 {
        return;
    }
    *frame += 1;
    if *frame == 2 {
        if let Ok(mut win) = windows.get_single_mut() {
            win.visible = true;
            win.focused = true;
            win.window_level = WindowLevel::AlwaysOnTop;
        }
    } else if *frame == 4 {
        if let Ok(mut win) = windows.get_single_mut() {
            win.window_level = WindowLevel::Normal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use stats::{DetectiveId, DetectiveSheet};

    #[test]
    fn headless_ecs_schedule_runs_and_switches_all_four_detectives() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins((
                StatsPlugin,
                SensesPlugin,
                NarrativePlugin,
                NetPlugin,
                WorldPlugin,
                NoirUiPlugin,
            ));

        app.update();
        app.update();

        // Character selection modal opens on startup
        assert!(app.world().resource::<ui::CharacterSelectModalOpen>().0);

        let nar = app.world().resource::<narrative::NarrativeState>();
        assert!(nar.other_line.contains("Richey"));
        assert!(!nar.current_choices.is_empty());

        // Switch through Lucker & Ron, Pinckney, and Zombie Graves and verify live sync
        for (det, expected_addr) in [
            (DetectiveId::LuckerAndRon, "Ron"),
            (DetectiveId::Pinckney, "Pinckney"),
            (DetectiveId::Zombie, "Graves"),
            (DetectiveId::Richey, "Richey"),
        ] {
            *app.world_mut().resource_mut::<DetectiveSheet>() = DetectiveSheet::from_character(det);
            app.update();

            let nar = app.world().resource::<narrative::NarrativeState>();
            assert!(
                nar.other_line.contains(expected_addr),
                "Expected '{}' in opening line '{}'",
                expected_addr,
                nar.other_line
            );
        }
    }
}
