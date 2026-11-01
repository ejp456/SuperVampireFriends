mod cypher;
mod narrative;
mod net;
mod senses;
mod stats;
mod stories;
mod ui;
mod world;

use bevy::{asset::AssetMetaCheck, prelude::*, window::WindowResolution};
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{PrimaryWindow, WindowLevel};

use cypher::CypherPlugin;
use narrative::NarrativePlugin;
use net::NetPlugin;
use senses::SensesPlugin;
use stats::StatsPlugin;
use stories::StoriesPlugin;
use ui::NoirUiPlugin;
use world::WorldPlugin;

fn main() {
    #[cfg(target_arch = "wasm32")]
    let asset_path = "assets".to_string();

    #[cfg(not(target_arch = "wasm32"))]
    let asset_path = if std::path::Path::new("assets/cases/case_01.ron").exists() {
        "assets".to_string()
    } else if let Some(exe_assets) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
        .and_then(|exe| exe.parent().map(|dir| dir.join("assets")))
        .filter(|dir| dir.join("cases/case_01.ron").exists())
    {
        exe_assets.to_string_lossy().into_owned()
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
        StoriesPlugin,
        NarrativePlugin,
        CypherPlugin,
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
    fn headless_ecs_schedule_picks_story_then_character_and_switches_detectives() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins((
                StatsPlugin,
                SensesPlugin,
                StoriesPlugin,
                NarrativePlugin,
                CypherPlugin,
                NetPlugin,
                WorldPlugin,
                NoirUiPlugin,
            ));

        app.update();
        app.update();

        // 1. Story selection screen opens first on launch; Character selection waits until a story is picked
        assert!(app.world().resource::<ui::StorySelectModalOpen>().0);
        assert!(!app.world().resource::<ui::CharacterSelectModalOpen>().0);
        assert_eq!(
            app.world().resource::<world::SceneRegistry>().story_name,
            "The Velvet Alibi"
        );

        // 2. Pick Story #2 ("The Morgue Covenant") -> loads its SceneCatalog & opens Character Select
        app.world_mut()
            .send_event(stories::SelectStoryEvent::pick_and_choose_character(
                "case_02",
            ));
        app.update();

        assert!(!app.world().resource::<ui::StorySelectModalOpen>().0);
        assert!(app.world().resource::<ui::CharacterSelectModalOpen>().0);
        assert_eq!(
            app.world().resource::<world::SceneRegistry>().story_name,
            "The Morgue Covenant"
        );
        assert_eq!(
            app.world().resource::<world::ActiveScene>().story_name,
            "The Morgue Covenant"
        );
        assert_eq!(
            app.world().resource::<world::ActiveScene>().current_id,
            "morgue_autopsy"
        );
        assert_eq!(
            app.world().resource::<narrative::CaseLibrary>().title,
            "THE MORGUE COVENANT"
        );
        assert!(app
            .world()
            .resource::<narrative::NarrativeState>()
            .other_speaker
            .contains("VALE"));

        // 3. Pick Story #1 ("The Velvet Alibi") -> loads its SceneCatalog & opens Character Select
        app.world_mut()
            .send_event(stories::SelectStoryEvent::pick_and_choose_character(
                "case_01",
            ));
        app.update();

        assert!(!app.world().resource::<ui::StorySelectModalOpen>().0);
        assert!(app.world().resource::<ui::CharacterSelectModalOpen>().0);
        assert_eq!(
            app.world().resource::<world::ActiveScene>().story_name,
            "The Velvet Alibi"
        );
        assert_eq!(
            app.world().resource::<world::ActiveScene>().current_id,
            "velvet_backroom"
        );

        let nar = app.world().resource::<narrative::NarrativeState>();
        assert!(nar.other_line.contains("Richey"));
        assert!(!nar.current_choices.is_empty());

        // 4. Switch through Lucker & Ron, Pinckney, and Zombie Guinan and verify live sync
        for (det, expected_addr) in [
            (DetectiveId::LuckerAndRon, "Ron"),
            (DetectiveId::Pinckney, "Pinckney"),
            (DetectiveId::Zombie, "Guinan"),
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

        // 5. Open a scene cypher puzzle, solve it, and verify both clue discovery & raised dice roll odds
        app.world_mut()
            .resource_mut::<ui::CharacterSelectModalOpen>()
            .0 = false;
        let odds_before = app
            .world()
            .resource::<DetectiveSheet>()
            .success_chance_percent(stats::Faculty::Pulse, 10);

        app.world_mut()
            .send_event(cypher::OpenCypherEvent("cypher_vance_valise".to_string()));
        app.update();
        assert!(app.world().resource::<cypher::ActiveCypherModal>().open);
        assert_eq!(
            app.world().resource::<cypher::ActiveCypherModal>().hint_stage,
            0,
            "Margin note hint must start gated/hidden (hint_stage == 0) until player clicks Hint"
        );

        app.world_mut()
            .resource_mut::<cypher::ActiveCypherModal>()
            .typed_input = "LEFT HOLSTER".to_string();
        app.world_mut().send_event(cypher::SubmitCypherGuessEvent);
        app.update();

        let ledger = app.world().resource::<narrative::ClueLedger>();
        assert!(ledger.is_cypher_solved("cypher_vance_valise"));
        assert!(ledger.has_clue("clue_vance_cypher"));

        let sheet_after = app.world().resource::<DetectiveSheet>();
        assert_eq!(sheet_after.cypher_bonus, 2);
        let odds_after = sheet_after.success_chance_percent(stats::Faculty::Pulse, 10);
        assert!(
            odds_after > odds_before,
            "Solving cypher must raise dice roll success chance ({odds_before}% -> {odds_after}%)"
        );
    }
}
