use bevy::prelude::*;

use crate::{
    narrative::{CaseData, CaseLibrary, ClueLedger, EnterNodeEvent, NarrativeState},
    net::NetSession,
    senses::BloodSight,
    stats::DetectiveSheet,
    ui::{CharacterSelectModalOpen, LedgerModalOpen, StorySelectModalOpen},
    world::{
        ActiveScene, ClickTargetMarker, DetectivePlayer, HotspotHighlightRing, NearbyHotspot,
        SceneCamera, SceneCatalog, SceneRegistry, SceneScoped, WorldPromptLabel,
    },
};

pub const DEFAULT_STORY_ID: &str = "case_01";

const EMBEDDED_CASE_01: &str = include_str!("../assets/cases/case_01.ron");
const EMBEDDED_CASE_01_SCENES: &str = include_str!("../assets/scenes/case_01_scenes.ron");

const EMBEDDED_CASE_02: &str = include_str!("../assets/cases/case_02.ron");
const EMBEDDED_CASE_02_SCENES: &str = include_str!("../assets/scenes/case_02_scenes.ron");

const EMBEDDED_CASE_03: &str = include_str!("../assets/cases/case_03.ron");
const EMBEDDED_CASE_03_SCENES: &str = include_str!("../assets/scenes/case_03_scenes.ron");

#[derive(Debug, Clone, Copy)]
pub struct StoryEntry {
    pub id: &'static str,
    pub setting_line: &'static str,
    pub synopsis: &'static str,
    pub accent_rgb: (f32, f32, f32),
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub case_rel_path: &'static str,
    pub embedded_case_ron: &'static str,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub scenes_rel_path: &'static str,
    pub embedded_scenes_ron: &'static str,
}

pub const STORIES: &[StoryEntry] = &[
    StoryEntry {
        id: "case_01",
        setting_line: "2:14 AM  •  Velvet Lounge & Harbor Street",
        synopsis: "Financier Julian Vance lies shot through his silk vest on a suspiciously spotless parlor rug while Sgt. Miller pushes for a quick stickup sign-off.",
        accent_rgb: (0.90, 0.28, 0.36),
        case_rel_path: "assets/cases/case_01.ron",
        embedded_case_ron: EMBEDDED_CASE_01,
        scenes_rel_path: "assets/scenes/case_01_scenes.ron",
        embedded_scenes_ron: EMBEDDED_CASE_01_SCENES,
    },
    StoryEntry {
        id: "case_02",
        setting_line: "3:45 AM  •  City Morgue, 13th Precinct & Pier 9",
        synopsis: "Dockmaster Arthur Harlow is logged as a Pier 9 drowning—yet his veins were surgically siphoned dry forty minutes before Harbor Patrol ever pulled him in.",
        accent_rgb: (0.38, 0.84, 0.80),
        case_rel_path: "assets/cases/case_02.ron",
        embedded_case_ron: EMBEDDED_CASE_02,
        scenes_rel_path: "assets/scenes/case_02_scenes.ron",
        embedded_scenes_ron: EMBEDDED_CASE_02_SCENES,
    },
    StoryEntry {
        id: "case_03",
        setting_line: "1:45 AM  •  Boudoir 4, Apothecary Salon & Red-Lantern Alley",
        synopsis: "Superior Court Judge Harlan Cross drops dead with silver-burnt ash on his lips after drinking from an indentured blood-courtesan who is still very much alive.",
        accent_rgb: (0.82, 0.44, 0.90),
        case_rel_path: "assets/cases/case_03.ron",
        embedded_case_ron: EMBEDDED_CASE_03,
        scenes_rel_path: "assets/scenes/case_03_scenes.ron",
        embedded_scenes_ron: EMBEDDED_CASE_03_SCENES,
    },
];

impl StoryEntry {
    pub fn load_case_data(&self) -> CaseData {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let exe_fallback = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
                .and_then(|exe| exe.parent().map(|dir| dir.join(self.case_rel_path)));
            let manifest_fallback = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), self.case_rel_path);
            let mut candidates = vec![std::path::PathBuf::from(self.case_rel_path)];
            if let Some(p) = exe_fallback {
                candidates.push(p);
            }
            candidates.push(std::path::PathBuf::from(manifest_fallback));
            for path in candidates {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = ron::from_str::<CaseData>(&raw) {
                        return parsed;
                    }
                }
            }
        }
        ron::from_str::<CaseData>(self.embedded_case_ron)
            .unwrap_or_else(|e| panic!("Failed to parse embedded CaseData for {}: {e}", self.id))
    }

    pub fn load_scene_catalog(&self) -> SceneCatalog {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let exe_fallback = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
                .and_then(|exe| exe.parent().map(|dir| dir.join(self.scenes_rel_path)));
            let manifest_fallback =
                format!("{}/{}", env!("CARGO_MANIFEST_DIR"), self.scenes_rel_path);
            let mut candidates = vec![std::path::PathBuf::from(self.scenes_rel_path)];
            if let Some(p) = exe_fallback {
                candidates.push(p);
            }
            candidates.push(std::path::PathBuf::from(manifest_fallback));
            for path in candidates {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = ron::from_str::<SceneCatalog>(&raw) {
                        return parsed;
                    }
                }
            }
        }
        ron::from_str::<SceneCatalog>(self.embedded_scenes_ron).unwrap_or_else(|e| {
            panic!("Failed to parse embedded SceneCatalog for {}: {e}", self.id)
        })
    }
}

pub fn find_story_entry(story_id: &str) -> &'static StoryEntry {
    STORIES
        .iter()
        .find(|s| s.id == story_id)
        .unwrap_or(&STORIES[0])
}

#[derive(Debug, Clone)]
pub struct StorySummary {
    pub id: String,
    pub story_name: String,
    pub setting_line: String,
    pub synopsis: String,
    pub scene_titles: Vec<String>,
    pub clue_count: usize,
    pub accent_rgb: (f32, f32, f32),
}

#[derive(Resource, Debug, Clone)]
pub struct StoryCatalog {
    pub stories: Vec<StorySummary>,
}

impl Default for StoryCatalog {
    fn default() -> Self {
        let stories = STORIES
            .iter()
            .map(|entry| {
                let catalog = entry.load_scene_catalog();
                let case = entry.load_case_data();
                let scene_titles = catalog.scenes.iter().map(|s| s.title.clone()).collect();
                StorySummary {
                    id: entry.id.to_string(),
                    story_name: catalog.story_name,
                    setting_line: entry.setting_line.to_string(),
                    synopsis: entry.synopsis.to_string(),
                    scene_titles,
                    clue_count: case.clues.len(),
                    accent_rgb: entry.accent_rgb,
                }
            })
            .collect();
        Self { stories }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ActiveStory {
    pub story_id: String,
    pub story_name: String,
}

impl Default for ActiveStory {
    fn default() -> Self {
        let entry = find_story_entry(DEFAULT_STORY_ID);
        let catalog = entry.load_scene_catalog();
        Self {
            story_id: entry.id.to_string(),
            story_name: catalog.story_name,
        }
    }
}

/// Event fired when a story is picked on the Story Selection screen or synced from a Co-op Host.
/// Loads that story's `SceneCatalog` and `CaseData`, respawns the scene world, and optionally
/// transitions directly to the Character Selection screen.
#[derive(Event, Debug, Clone)]
pub struct SelectStoryEvent {
    pub story_id: String,
    pub open_character_select: bool,
    pub initial_clues: Vec<String>,
}

impl SelectStoryEvent {
    pub fn pick_and_choose_character(story_id: impl Into<String>) -> Self {
        Self {
            story_id: story_id.into(),
            open_character_select: true,
            initial_clues: Vec::new(),
        }
    }

    pub fn sync_from_host(story_id: impl Into<String>, initial_clues: Vec<String>) -> Self {
        Self {
            story_id: story_id.into(),
            open_character_select: false,
            initial_clues,
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorySystems;

pub struct StoriesPlugin;

impl Plugin for StoriesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StoryCatalog>()
            .init_resource::<ActiveStory>()
            .add_event::<SelectStoryEvent>()
            .add_systems(Update, apply_select_story_events.in_set(StorySystems));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_select_story_events(
    mut commands: Commands,
    mut events: EventReader<SelectStoryEvent>,
    asset_server: Option<Res<AssetServer>>,
    (mut active_story, mut scene_registry, mut active_scene, mut nearby): (
        ResMut<ActiveStory>,
        ResMut<SceneRegistry>,
        ResMut<ActiveScene>,
        ResMut<NearbyHotspot>,
    ),
    (mut case_library, mut clue_ledger, mut nar_state): (
        ResMut<CaseLibrary>,
        ResMut<ClueLedger>,
        ResMut<NarrativeState>,
    ),
    (mut sheet, mut blood_sight, mut session): (
        ResMut<DetectiveSheet>,
        ResMut<BloodSight>,
        ResMut<NetSession>,
    ),
    (mut story_modal, mut char_modal, mut ledger_modal): (
        ResMut<StorySelectModalOpen>,
        ResMut<CharacterSelectModalOpen>,
        ResMut<LedgerModalOpen>,
    ),
    scoped_q: Query<Entity, With<SceneScoped>>,
    mut player_q: Query<
        (&mut DetectivePlayer, &mut Transform),
        (
            Without<SceneCamera>,
            Without<ClickTargetMarker>,
            Without<HotspotHighlightRing>,
            Without<WorldPromptLabel>,
        ),
    >,
    mut cam_q: Query<
        &mut Transform,
        (
            With<SceneCamera>,
            Without<DetectivePlayer>,
            Without<ClickTargetMarker>,
            Without<HotspotHighlightRing>,
            Without<WorldPromptLabel>,
        ),
    >,
    mut marker_vis: ParamSet<(
        Query<&mut Visibility, With<ClickTargetMarker>>,
        Query<&mut Visibility, With<HotspotHighlightRing>>,
        Query<&mut Visibility, With<WorldPromptLabel>>,
    )>,
    mut enter_writer: EventWriter<EnterNodeEvent>,
) {
    let Some(ev) = events.read().last().cloned() else {
        return;
    };

    let entry = find_story_entry(&ev.story_id);
    let catalog = entry.load_scene_catalog();
    let new_registry = SceneRegistry::from_catalog(catalog);
    let case_data = entry.load_case_data();
    let new_library = CaseLibrary::from_case_data(case_data);

    active_story.story_id = entry.id.to_string();
    active_story.story_name = new_registry.story_name.clone();

    // 1. Replace SceneRegistry and respawn the newly selected SceneCatalog's initial scene
    for entity in &scoped_q {
        commands.entity(entity).despawn_recursive();
    }

    let spawn_pos = new_registry.initial_spawn;
    active_scene.story_name = new_registry.story_name.clone();
    active_scene.saved_pulses.clear();

    if let Some(initial) = new_registry.scenes.get(&new_registry.initial_scene) {
        active_scene.current_id = initial.id.clone();
        active_scene.current_title = initial.title.clone();
        active_scene.environment = initial.environment;
        active_scene.walk_bounds = Vec2::new(initial.walk_bounds.0, initial.walk_bounds.1);
        active_scene.camera_pan_bounds =
            Vec2::new(initial.camera_pan_bounds.0, initial.camera_pan_bounds.1);
        initial.spawn_entities(
            &mut commands,
            asset_server.as_deref(),
            &active_scene.saved_pulses,
        );
    }

    *scene_registry = new_registry;

    if let Ok((mut player, mut p_tf)) = player_q.get_single_mut() {
        p_tf.translation.x = spawn_pos.x;
        p_tf.translation.y = spawn_pos.y;
        player.is_moving = false;
        player.waypoints.clear();
        player.pending_interaction = None;
    }

    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        cam_tf.translation.x = spawn_pos
            .x
            .clamp(-active_scene.camera_pan_bounds.x, active_scene.camera_pan_bounds.x);
        cam_tf.translation.y = spawn_pos
            .y
            .clamp(-active_scene.camera_pan_bounds.y, active_scene.camera_pan_bounds.y);
    }

    nearby.label = None;
    nearby.node_id = None;
    if let Ok(mut vis) = marker_vis.p0().get_single_mut() {
        *vis = Visibility::Hidden;
    }
    if let Ok(mut vis) = marker_vis.p1().get_single_mut() {
        *vis = Visibility::Hidden;
    }
    if let Ok(mut vis) = marker_vis.p2().get_single_mut() {
        *vis = Visibility::Hidden;
    }

    // 2. Replace CaseLibrary, reset ClueLedger (plus any host-synced clues), and open arrival node
    let opening_node = new_library.opening_node.clone();
    *case_library = new_library;

    *clue_ledger = ClueLedger::default();
    for cid in &ev.initial_clues {
        clue_ledger.add_clue(cid);
    }
    session.known_clues_sent = clue_ledger.discovered.len();

    blood_sight.active = false;
    sheet.hunger = 1.0;

    let next_gen = nar_state.generation + 1;
    *nar_state = NarrativeState {
        generation: next_gen,
        ..default()
    };
    enter_writer.send(EnterNodeEvent::fresh(opening_node));

    // 3. Advance from Story Selection to Character Selection (or straight into hosted Co-op)
    story_modal.0 = false;
    ledger_modal.0 = false;
    if ev.open_character_select {
        char_modal.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_story_pair_valid(story_label: &str, scene_cat: &SceneCatalog, case_data: CaseData) {
        assert!(
            !scene_cat.story_name.trim().is_empty(),
            "SceneCatalog for '{story_label}' must have a non-empty story_name"
        );
        assert!(
            scene_cat
                .scenes
                .iter()
                .any(|s| s.id == scene_cat.initial_scene),
            "initial_scene '{}' missing in '{story_label}'",
            scene_cat.initial_scene
        );

        let lib = CaseLibrary::from_case_data(case_data);
        assert!(
            lib.nodes.contains_key(&lib.opening_node),
            "opening_node '{}' missing in '{story_label}'",
            lib.opening_node
        );

        for sc in &scene_cat.scenes {
            assert!(
                !sc.doorways.is_empty(),
                "Scene '{}' in '{story_label}' must have at least one doorway",
                sc.id
            );
            for dw in &sc.doorways {
                assert!(
                    scene_cat.scenes.iter().any(|s| s.id == dw.target_scene_id),
                    "Doorway '{}' in '{story_label}' points to missing scene '{}'",
                    dw.id,
                    dw.target_scene_id
                );
            }
            for ch in &sc.characters {
                assert!(
                    lib.nodes.contains_key(&ch.dialogue_node),
                    "Character '{}' in story '{story_label}' references missing node '{}'",
                    ch.id,
                    ch.dialogue_node
                );
            }
            for cl in &sc.clues {
                assert!(
                    lib.nodes.contains_key(&cl.node_id),
                    "Clue '{}' in story '{story_label}' references missing node '{}'",
                    cl.id,
                    cl.node_id
                );
            }
        }

        for (nid, node) in &lib.nodes {
            for gid in &node.grants_clues {
                assert!(
                    lib.clues.contains_key(gid),
                    "Node '{nid}' in story '{story_label}' grants unknown clue '{gid}'"
                );
            }
            for choice in &node.choices {
                if let Some(req) = &choice.requires_clue {
                    assert!(
                        lib.clues.contains_key(req),
                        "Choice in node '{nid}' (story '{story_label}') requires unknown clue '{req}'"
                    );
                }
                if let Some(sc) = &choice.skill_check {
                    assert!(
                        lib.nodes.contains_key(&sc.success_node),
                        "SkillCheck '{}' in story '{story_label}' missing success_node '{}'",
                        sc.id,
                        sc.success_node
                    );
                    assert!(
                        lib.nodes.contains_key(&sc.failure_node),
                        "SkillCheck '{}' in story '{story_label}' missing failure_node '{}'",
                        sc.id,
                        sc.failure_node
                    );
                } else if choice.next_node != "LEAVE_CONVERSATION" && !choice.next_node.is_empty() {
                    assert!(
                        lib.nodes.contains_key(&choice.next_node),
                        "Choice '{}' in node '{nid}' (story '{story_label}') points to missing node '{}'",
                        choice.label,
                        choice.next_node
                    );
                }
            }
        }
    }

    #[test]
    fn all_registered_stories_have_story_names_and_valid_cases() {
        let catalog = StoryCatalog::default();
        assert!(
            catalog.stories.len() >= 2,
            "Expected multiple playable stories registered"
        );

        for entry in STORIES {
            let scene_cat = entry.load_scene_catalog();
            let case_data = entry.load_case_data();
            assert_story_pair_valid(entry.id, &scene_cat, case_data);
        }
    }

    #[test]
    fn create_noir_story_skill_templates_parse_and_link_cleanly() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let skill_dir = root.join(".agents/skills/create-noir-story");
        assert!(
            skill_dir.join("SKILL.md").is_file(),
            "Expected SKILL.md inside .agents/skills/create-noir-story/"
        );

        let raw_scenes = std::fs::read_to_string(skill_dir.join("resources/scenes_template.ron"))
            .expect("Failed to read scenes_template.ron")
            .replace("__STORY_NAME__", "The Clocktower Requiem");
        let raw_case = std::fs::read_to_string(skill_dir.join("resources/case_template.ron"))
            .expect("Failed to read case_template.ron")
            .replace("__STORY_UPPER__", "THE CLOCKTOWER REQUIEM")
            .replace("__STORY_NAME__", "The Clocktower Requiem");

        let scene_cat: SceneCatalog =
            ron::from_str(&raw_scenes).expect("scenes_template.ron must deserialize cleanly");
        let case_data: CaseData =
            ron::from_str(&raw_case).expect("case_template.ron must deserialize cleanly");

        assert_story_pair_valid("template_story", &scene_cat, case_data);
    }
}
