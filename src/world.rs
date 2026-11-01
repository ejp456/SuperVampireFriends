use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use bevy::{prelude::*, render::camera::Viewport, window::PrimaryWindow};
use serde::{Deserialize, Serialize};

use crate::{
    cypher::{ActiveCypherModal, OpenCypherEvent},
    narrative::{ClueLedger, EnterNodeEvent, NarrativeState},
    net::NetSession,
    senses::{BloodEcho, BloodSight, HeartbeatPulseRing, MundaneAmbiance, SuspectHeartbeat},
    stats::{DetectiveId, DetectiveSheet},
    ui::{CharacterSelectModalOpen, LedgerModalOpen, MultiplayerModalOpen, StorySelectModalOpen},
};

const EMBEDDED_CASE_01_SCENES: &str = include_str!("../assets/scenes/case_01_scenes.ron");

/// Fraction of window width dedicated to the 2D noir crime scene viewport (left side).
pub const SCENE_WIDTH_FRACTION: f32 = 0.60;

// ============================================================================
// SCENE CLASSES & DATA DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SceneEnvironment {
    #[default]
    Room,
    Alley,
    Street,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCatalog {
    pub story_name: String,
    pub initial_scene: String,
    pub initial_spawn: (f32, f32),
    pub scenes: Vec<SceneClass>,
}

/// Core Scene Class describing how a room, alley, or street is rendered
/// and how the detective can move, investigate clues, talk to characters,
/// and traverse doorways into linked scenes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneClass {
    pub id: String,
    pub title: String,
    pub environment: SceneEnvironment,
    pub walk_bounds: (f32, f32),
    pub camera_pan_bounds: (f32, f32),
    pub rain_drops: usize,
    pub surfaces: Vec<SceneSurface>,
    pub walls: Vec<SceneWall>,
    pub props: Vec<SceneProp>,
    pub characters: Vec<SceneCharacter>,
    pub clues: Vec<SceneClue>,
    #[serde(default)]
    pub cyphers: Vec<SceneCypher>,
    pub blood_echoes: Vec<SceneBloodEcho>,
    pub doorways: Vec<SceneDoorwaySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSurface {
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub z: f32,
    pub normal_rgba: (f32, f32, f32, f32),
    pub blood_sight_rgba: (f32, f32, f32, f32),
    pub neon_flicker: Option<f32>,
    #[serde(default)]
    pub sprite_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneWall {
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub solid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneProp {
    pub label: Option<String>,
    #[serde(default)]
    pub sprite_path: Option<String>,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    #[serde(default)]
    pub collider_half: Option<(f32, f32)>,
    #[serde(default)]
    pub rotation_deg: f32,
    pub z: f32,
    pub color_rgba: (f32, f32, f32, f32),
    pub solid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCharacter {
    pub id: String,
    pub display_name: String,
    pub nameplate: Option<String>,
    pub sprite_path: String,
    pub fallback_rgb: (f32, f32, f32),
    pub pos: (f32, f32),
    pub sprite_size: (f32, f32),
    pub collider_half: Option<(f32, f32)>,
    pub resting_bpm: Option<f32>,
    pub dialogue_node: String,
    pub interact_radius: f32,
    pub nameplate_rgb: (f32, f32, f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneClue {
    pub id: String,
    pub label: String,
    pub node_id: String,
    #[serde(default)]
    pub sprite_path: Option<String>,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub z: f32,
    pub color_rgba: (f32, f32, f32, f32),
    pub collider_half: Option<(f32, f32)>,
    pub interact_radius: f32,
    pub requires_blood_sight_unless_clue: Option<String>,
    pub blood_echo_alpha: Option<f32>,
}

/// Interactive Cypher Puzzle placed directly in a 2D noir crime scene.
/// Solving the referenced `CypherDef` awards a strong clue and raises dice roll odds!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCypher {
    pub id: String,
    pub label: String,
    pub cypher_id: String,
    #[serde(default)]
    pub sprite_path: Option<String>,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub z: f32,
    pub color_rgba: (f32, f32, f32, f32),
    #[serde(default)]
    pub collider_half: Option<(f32, f32)>,
    pub interact_radius: f32,
    #[serde(default)]
    pub requires_blood_sight_unless_clue: Option<String>,
    #[serde(default)]
    pub blood_echo_alpha: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneBloodEcho {
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub alpha: f32,
    pub phase_offset: f32,
    #[serde(default)]
    pub sprite_path: Option<String>,
    #[serde(default)]
    pub rotation_deg: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDoorwaySpec {
    pub id: String,
    pub label: String,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub trigger_radius: f32,
    pub target_scene_id: String,
    pub target_spawn: (f32, f32),
    pub glow_rgb: (f32, f32, f32),
}

impl SceneClass {
    /// Returns all static solid obstacle `(center, half_extents)` boxes in this scene.
    #[allow(dead_code)]
    pub fn static_obstacle_boxes(&self) -> Vec<(Vec2, Vec2)> {
        let mut boxes = Vec::new();
        for w in &self.walls {
            if w.solid {
                boxes.push((
                    Vec2::new(w.pos.0, w.pos.1),
                    Vec2::new(w.size.0 * 0.5, w.size.1 * 0.5),
                ));
            }
        }
        for p in &self.props {
            if p.solid {
                let half = p
                    .collider_half
                    .map(|(hx, hy)| Vec2::new(hx, hy))
                    .unwrap_or_else(|| Vec2::new(p.size.0 * 0.5, p.size.1 * 0.5));
                boxes.push((Vec2::new(p.pos.0, p.pos.1), half));
            }
        }
        for ch in &self.characters {
            if let Some((hx, hy)) = ch.collider_half {
                boxes.push((Vec2::new(ch.pos.0, ch.pos.1), Vec2::new(hx, hy)));
            }
        }
        for cl in &self.clues {
            if let Some((hx, hy)) = cl.collider_half {
                boxes.push((Vec2::new(cl.pos.0, cl.pos.1), Vec2::new(hx, hy)));
            }
        }
        for cy in &self.cyphers {
            if let Some((hx, hy)) = cy.collider_half {
                boxes.push((Vec2::new(cy.pos.0, cy.pos.1), Vec2::new(hx, hy)));
            }
        }
        boxes
    }

    /// Instantiates all rendered surfaces, architecture, props, characters, clues,
    /// Blood Sight echoes, linked doorways, and weather effects for this scene.
    pub fn spawn_entities(
        &self,
        commands: &mut Commands,
        asset_server: Option<&AssetServer>,
        saved_pulses: &HashMap<String, (f32, bool)>,
    ) {
        let load_tex = |path: &str| -> Option<Handle<Image>> {
            asset_server.map(|s| s.load(path.to_owned()))
        };

        // 1. Surfaces (floors, streets, sidewalks, rugs, light pools, neon puddles)
        for surf in &self.surfaces {
            let normal = Color::srgba(
                surf.normal_rgba.0,
                surf.normal_rgba.1,
                surf.normal_rgba.2,
                surf.normal_rgba.3,
            );
            let sight = Color::srgba(
                surf.blood_sight_rgba.0,
                surf.blood_sight_rgba.1,
                surf.blood_sight_rgba.2,
                surf.blood_sight_rgba.3,
            );
            let surf_sprite = if let Some(tex) = surf.sprite_path.as_deref().and_then(&load_tex) {
                Sprite {
                    image: tex,
                    color: normal,
                    custom_size: Some(Vec2::new(surf.size.0, surf.size.1)),
                    ..default()
                }
            } else {
                Sprite {
                    color: normal,
                    custom_size: Some(Vec2::new(surf.size.0, surf.size.1)),
                    ..default()
                }
            };
            let mut ent = commands.spawn((
                SceneScoped,
                surf_sprite,
                Transform::from_xyz(surf.pos.0, surf.pos.1, surf.z),
                MundaneAmbiance {
                    normal_color: normal,
                    blood_sight_color: sight,
                },
            ));
            if let Some(alpha) = surf.neon_flicker {
                ent.insert(NeonFlicker { base_alpha: alpha });
            }
        }

        // 2. Walls, Curbs & Perimeter Barriers
        for wall in &self.walls {
            let sz = Vec2::new(wall.size.0, wall.size.1);
            let mut ent = commands.spawn((
                SceneScoped,
                Sprite {
                    color: Color::srgb(wall.color_rgb.0, wall.color_rgb.1, wall.color_rgb.2),
                    custom_size: Some(sz),
                    ..default()
                },
                Transform::from_xyz(wall.pos.0, wall.pos.1, 3.0),
            ));
            if wall.solid {
                ent.insert(SolidCollider {
                    half_extents: sz * 0.5,
                });
            }
        }

        // 3. Props & Set Pieces
        for prop in &self.props {
            let sz = Vec2::new(prop.size.0, prop.size.1);
            let prop_sprite = if let Some(tex) = prop.sprite_path.as_deref().and_then(&load_tex) {
                Sprite {
                    image: tex,
                    color: Color::srgba(1.0, 1.0, 1.0, prop.color_rgba.3),
                    custom_size: Some(sz),
                    ..default()
                }
            } else {
                Sprite {
                    color: Color::srgba(
                        prop.color_rgba.0,
                        prop.color_rgba.1,
                        prop.color_rgba.2,
                        prop.color_rgba.3,
                    ),
                    custom_size: Some(sz),
                    ..default()
                }
            };
            let mut tf = Transform::from_xyz(prop.pos.0, prop.pos.1, prop.z);
            if prop.rotation_deg.abs() > 0.01 {
                tf.rotation = Quat::from_rotation_z(prop.rotation_deg.to_radians());
            }
            let mut ent = commands.spawn((SceneScoped, prop_sprite, tf));
            if prop.solid {
                let half_extents = prop
                    .collider_half
                    .map(|(hx, hy)| Vec2::new(hx, hy))
                    .unwrap_or(sz * 0.5);
                ent.insert(SolidCollider { half_extents });
            }
        }

        // 4. Characters (Living Suspects with Heartbeat Rings & Corpses)
        for ch in &self.characters {
            if ch.resting_bpm.is_some() {
                let ring_sz = ch.sprite_size.0.max(54.0) * 0.80;
                commands.spawn((
                    SceneScoped,
                    Sprite {
                        color: Color::srgba(0.92, 0.16, 0.28, 0.5),
                        custom_size: Some(Vec2::splat(ring_sz)),
                        ..default()
                    },
                    Transform::from_xyz(ch.pos.0, ch.pos.1, 2.1),
                    Visibility::Hidden,
                    HeartbeatPulseRing {
                        owner_npc_id: ch.id.clone(),
                        base_size: ring_sz,
                    },
                ));
            }

            let char_sprite = if let Some(tex) = load_tex(&ch.sprite_path) {
                Sprite {
                    image: tex,
                    custom_size: Some(Vec2::new(ch.sprite_size.0, ch.sprite_size.1)),
                    ..default()
                }
            } else {
                Sprite {
                    color: Color::srgb(ch.fallback_rgb.0, ch.fallback_rgb.1, ch.fallback_rgb.2),
                    custom_size: Some(Vec2::new(28.0, 28.0)),
                    ..default()
                }
            };

            let z = if ch.resting_bpm.is_some() { 3.2 } else { 2.0 };
            let mut ent = commands.spawn((
                SceneScoped,
                char_sprite,
                Transform::from_xyz(ch.pos.0, ch.pos.1, z),
                InteractableHotspot {
                    label: ch.display_name.clone(),
                    node_id: ch.dialogue_node.clone(),
                    cypher_id: None,
                    radius: ch.interact_radius,
                    requires_blood_sight_unless_clue: None,
                },
            ));

            if let Some((hx, hy)) = ch.collider_half {
                ent.insert(SolidCollider {
                    half_extents: Vec2::new(hx, hy),
                });
            }

            if let Some(resting) = ch.resting_bpm {
                let mut hb = SuspectHeartbeat::new(&ch.id, &ch.display_name, resting);
                if let Some(&(saved_bpm, saved_spike)) = saved_pulses.get(&ch.id) {
                    hb.current_bpm = saved_bpm;
                    hb.target_bpm = saved_bpm;
                    hb.is_lie_spike = saved_spike;
                }
                ent.insert(hb);
            }

            if let Some(plate) = &ch.nameplate {
                let plate_txt = plate.clone();
                let plate_col = Color::srgb(
                    ch.nameplate_rgb.0,
                    ch.nameplate_rgb.1,
                    ch.nameplate_rgb.2,
                );
                let top_offset = ch.sprite_size.1 * 0.5 + 5.0;
                ent.with_children(|parent| {
                    parent.spawn((
                        Sprite {
                            color: Color::srgba(0.02, 0.02, 0.04, 0.50),
                            custom_size: Some(Vec2::new(32.0, 10.0)),
                            ..default()
                        },
                        Transform::from_xyz(0.0, -28.0, -0.2),
                    ));
                    parent.spawn((
                        Text2d::new(plate_txt),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(plate_col),
                        Transform::from_xyz(0.0, top_offset, 1.0),
                    ));
                });
            }
        }

        // 5. Interactive Forensic Clue Objects
        for cl in &self.clues {
            let clue_sprite = if let Some(tex) = cl.sprite_path.as_deref().and_then(&load_tex) {
                Sprite {
                    image: tex,
                    color: Color::srgba(1.0, 1.0, 1.0, cl.color_rgba.3),
                    custom_size: Some(Vec2::new(cl.size.0, cl.size.1)),
                    ..default()
                }
            } else {
                Sprite {
                    color: Color::srgba(
                        cl.color_rgba.0,
                        cl.color_rgba.1,
                        cl.color_rgba.2,
                        cl.color_rgba.3,
                    ),
                    custom_size: Some(Vec2::new(cl.size.0, cl.size.1)),
                    ..default()
                }
            };
            let mut ent = commands.spawn((
                SceneScoped,
                clue_sprite,
                Transform::from_xyz(cl.pos.0, cl.pos.1, cl.z),
                InteractableHotspot {
                    label: cl.label.clone(),
                    node_id: cl.node_id.clone(),
                    cypher_id: None,
                    radius: cl.interact_radius,
                    requires_blood_sight_unless_clue: cl.requires_blood_sight_unless_clue.clone(),
                },
            ));

            if let Some((hx, hy)) = cl.collider_half {
                ent.insert(SolidCollider {
                    half_extents: Vec2::new(hx, hy),
                });
            }

            if let Some(alpha) = cl.blood_echo_alpha {
                ent.insert((
                    Visibility::Hidden,
                    BloodEcho {
                        base_alpha: alpha,
                        phase_offset: 0.0,
                    },
                ));
            }
        }

        // 5b. Interactive Scene Cypher Puzzles
        for cy in &self.cyphers {
            let cypher_sprite = if let Some(tex) = cy.sprite_path.as_deref().and_then(&load_tex) {
                Sprite {
                    image: tex,
                    color: Color::srgba(1.0, 1.0, 1.0, cy.color_rgba.3),
                    custom_size: Some(Vec2::new(cy.size.0, cy.size.1)),
                    ..default()
                }
            } else {
                Sprite {
                    color: Color::srgba(
                        cy.color_rgba.0,
                        cy.color_rgba.1,
                        cy.color_rgba.2,
                        cy.color_rgba.3,
                    ),
                    custom_size: Some(Vec2::new(cy.size.0, cy.size.1)),
                    ..default()
                }
            };
            let top_offset = cy.size.1 * 0.5 + 10.0;
            let glow_size = Vec2::new(cy.size.0 + 14.0, cy.size.1 + 14.0);
            let cy_id_badge = cy.cypher_id.clone();
            let cy_id_glow = cy.cypher_id.clone();

            let mut ent = commands.spawn((
                SceneScoped,
                cypher_sprite,
                Transform::from_xyz(cy.pos.0, cy.pos.1, cy.z),
                InteractableHotspot {
                    label: cy.label.clone(),
                    node_id: String::new(),
                    cypher_id: Some(cy.cypher_id.clone()),
                    radius: cy.interact_radius,
                    requires_blood_sight_unless_clue: cy.requires_blood_sight_unless_clue.clone(),
                },
            ));

            ent.with_children(|parent| {
                parent.spawn((
                    Sprite {
                        color: Color::srgba(0.96, 0.78, 0.28, 0.22),
                        custom_size: Some(glow_size),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -0.15),
                    SceneCypherGlowPlate {
                        cypher_id: cy_id_glow,
                    },
                ));
                parent.spawn((
                    Text2d::new("[CYPHER]"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.84, 0.38)),
                    Transform::from_xyz(0.0, top_offset, 1.2),
                    SceneCypherBadgeText {
                        cypher_id: cy_id_badge,
                    },
                ));
            });

            if let Some((hx, hy)) = cy.collider_half {
                ent.insert(SolidCollider {
                    half_extents: Vec2::new(hx, hy),
                });
            }

            if let Some(alpha) = cy.blood_echo_alpha {
                ent.insert((
                    Visibility::Hidden,
                    BloodEcho {
                        base_alpha: alpha,
                        phase_offset: 0.0,
                    },
                ));
            }
        }

        // 6. Supernatural Blood Sight Trails & Residue
        for echo in &self.blood_echoes {
            let echo_sprite = if let Some(tex) = echo.sprite_path.as_deref().and_then(&load_tex) {
                Sprite {
                    image: tex,
                    color: Color::srgba(0.95, 0.08, 0.20, echo.alpha),
                    custom_size: Some(Vec2::new(echo.size.0, echo.size.1)),
                    ..default()
                }
            } else {
                Sprite {
                    color: Color::srgba(0.95, 0.08, 0.20, echo.alpha),
                    custom_size: Some(Vec2::new(echo.size.0, echo.size.1)),
                    ..default()
                }
            };
            let mut tf = Transform::from_xyz(echo.pos.0, echo.pos.1, 1.9);
            if echo.rotation_deg.abs() > 0.01 {
                tf.rotation = Quat::from_rotation_z(echo.rotation_deg.to_radians());
            }
            commands.spawn((
                SceneScoped,
                echo_sprite,
                tf,
                Visibility::Hidden,
                BloodEcho {
                    base_alpha: echo.alpha,
                    phase_offset: echo.phase_offset,
                },
            ));
        }

        // 7. Linked Scene Doorways / Street Transitions
        for dw in &self.doorways {
            let accent = Color::srgb(dw.glow_rgb.0, dw.glow_rgb.1, dw.glow_rgb.2);
            let glow_bg = Color::srgba(dw.glow_rgb.0, dw.glow_rgb.1, dw.glow_rgb.2, 0.28);
            let short_dest = dw
                .label
                .split("->")
                .nth(1)
                .map(|s| format!("Exit: {}", s.trim()))
                .unwrap_or_else(|| dw.label.clone());

            // Label offset nudged inward toward center `(0, 0)` so it stays clearly legible inside walls
            let inward_x = if dw.pos.0.abs() > 140.0 {
                -dw.pos.0.signum() * 54.0
            } else {
                0.0
            };
            let inward_y = if dw.pos.1.abs() > 120.0 {
                -dw.pos.1.signum() * 24.0
            } else {
                26.0
            };

            commands
                .spawn((
                    SceneScoped,
                    Sprite {
                        color: glow_bg,
                        custom_size: Some(Vec2::new(dw.size.0, dw.size.1)),
                        ..default()
                    },
                    Transform::from_xyz(dw.pos.0, dw.pos.1, 1.7),
                    SceneDoorway {
                        id: dw.id.clone(),
                        label: dw.label.clone(),
                        trigger_radius: dw.trigger_radius,
                        target_scene_id: dw.target_scene_id.clone(),
                        target_spawn: Vec2::new(dw.target_spawn.0, dw.target_spawn.1),
                    },
                ))
                .with_children(|parent| {
                    // Crisp inner threshold strip
                    parent.spawn((
                        Sprite {
                            color: accent.with_alpha(0.70),
                            custom_size: Some(Vec2::new(
                                (dw.size.0 - 8.0).max(6.0),
                                (dw.size.1 - 8.0).max(6.0),
                            )),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, 0.1),
                    ));
                    parent.spawn((
                        Text2d::new(short_dest),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(accent),
                        Transform::from_xyz(inward_x, inward_y, 2.0),
                    ));
                });
        }

        // 8. Exterior Weather (Rainfall Streaks)
        let (half_w, half_h) = self.walk_bounds;
        for i in 0..self.rain_drops {
            let span_w = (half_w * 2.0).max(200.0);
            let span_h = (half_h * 2.0).max(200.0);
            let rx = -half_w + ((i * 41) as f32 % span_w);
            let ry = -half_h + ((i * 59) as f32 % span_h);
            commands.spawn((
                SceneScoped,
                Sprite {
                    color: Color::srgba(0.68, 0.78, 0.92, 0.24),
                    custom_size: Some(Vec2::new(1.5, 16.0)),
                    ..default()
                },
                Transform::from_xyz(rx, ry, 5.0).with_rotation(Quat::from_rotation_z(-0.22)),
                RainDrop {
                    velocity: Vec2::new(-68.0, -320.0),
                    min_y: -half_h - 15.0,
                    reset_y: half_h + 15.0,
                    wrap_half_w: half_w + 15.0,
                },
            ));
        }
    }
}

// ============================================================================
// RESOURCES, EVENTS & ECS COMPONENTS
// ============================================================================

#[derive(Resource, Debug, Clone)]
pub struct SceneRegistry {
    pub story_name: String,
    pub initial_scene: String,
    pub initial_spawn: Vec2,
    pub scenes: HashMap<String, SceneClass>,
}

impl SceneRegistry {
    pub fn load_from_disk_or_embedded() -> Self {
        let raw_ron = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::fs::read_to_string("assets/scenes/case_01_scenes.ron")
                    .unwrap_or_else(|_| EMBEDDED_CASE_01_SCENES.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            {
                EMBEDDED_CASE_01_SCENES.to_string()
            }
        };

        let catalog = ron::from_str::<SceneCatalog>(&raw_ron)
            .expect("Failed to parse assets/scenes/case_01_scenes.ron");
        Self::from_catalog(catalog)
    }

    pub fn from_catalog(catalog: SceneCatalog) -> Self {
        let mut scenes = HashMap::new();
        for sc in catalog.scenes {
            scenes.insert(sc.id.clone(), sc);
        }
        Self {
            story_name: catalog.story_name,
            initial_scene: catalog.initial_scene,
            initial_spawn: Vec2::new(catalog.initial_spawn.0, catalog.initial_spawn.1),
            scenes,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ActiveScene {
    pub story_name: String,
    pub current_id: String,
    pub current_title: String,
    pub environment: SceneEnvironment,
    pub walk_bounds: Vec2,
    pub camera_pan_bounds: Vec2,
    pub saved_pulses: HashMap<String, (f32, bool)>,
}

impl Default for ActiveScene {
    fn default() -> Self {
        Self {
            story_name: String::new(),
            current_id: String::new(),
            current_title: String::new(),
            environment: SceneEnvironment::Room,
            walk_bounds: Vec2::new(292.0, 218.0),
            camera_pan_bounds: Vec2::new(95.0, 75.0),
            saved_pulses: HashMap::new(),
        }
    }
}

#[derive(Event, Debug, Clone)]
pub struct TransitionSceneEvent {
    pub target_scene_id: String,
    pub spawn_pos: Vec2,
}

/// Marks all entities belonging to the currently active `SceneClass` so they
/// despawn cleanly when stepping through a doorway into another scene.
#[derive(Component)]
pub struct SceneScoped;

#[derive(Component)]
pub struct SceneCamera;

#[derive(Debug, Clone)]
pub enum PendingInteraction {
    Hotspot {
        pos: Vec2,
        radius: f32,
        node_id: String,
        cypher_id: Option<String>,
    },
    Doorway {
        pos: Vec2,
        radius: f32,
        target_scene_id: String,
        target_spawn: Vec2,
    },
}

#[derive(Component)]
pub struct DetectivePlayer {
    pub speed: f32,
    pub walk_phase: f32,
    pub is_moving: bool,
    pub waypoints: Vec<Vec2>,
    pub pending_interaction: Option<PendingInteraction>,
}

#[derive(Component)]
pub struct CharacterVisualSprite;

#[derive(Component)]
pub struct LocalPlayerSenseAura;

#[derive(Component)]
pub struct ClickTargetMarker;

#[derive(Component)]
pub struct RemotePeerActor {
    pub peer_id: u32,
    pub walk_phase: f32,
}

#[derive(Component)]
pub struct RemotePeerVisual {
    pub peer_id: u32,
    pub character: DetectiveId,
}

#[derive(Component)]
pub struct RemotePeerNameplate {
    pub peer_id: u32,
}

#[derive(Component)]
pub struct RemotePeerSightAura {
    pub peer_id: u32,
}

#[derive(Component)]
pub struct SolidCollider {
    pub half_extents: Vec2,
}

#[derive(Component, Clone)]
pub struct InteractableHotspot {
    pub label: String,
    pub node_id: String,
    pub cypher_id: Option<String>,
    pub radius: f32,
    pub requires_blood_sight_unless_clue: Option<String>,
}

#[derive(Component)]
pub struct SceneCypherGlowPlate {
    pub cypher_id: String,
}

#[derive(Component)]
pub struct SceneCypherBadgeText {
    pub cypher_id: String,
}

#[derive(Component, Clone)]
pub struct SceneDoorway {
    #[allow(dead_code)]
    pub id: String,
    pub label: String,
    pub trigger_radius: f32,
    pub target_scene_id: String,
    pub target_spawn: Vec2,
}

#[derive(Component)]
pub struct HotspotHighlightRing;

#[derive(Component)]
pub struct WorldPromptLabel;

#[derive(Component)]
pub struct RainDrop {
    pub velocity: Vec2,
    pub min_y: f32,
    pub reset_y: f32,
    pub wrap_half_w: f32,
}

#[derive(Component)]
pub struct NeonFlicker {
    pub base_alpha: f32,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct NearbyHotspot {
    pub label: Option<String>,
    pub node_id: Option<String>,
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        let registry = SceneRegistry::load_from_disk_or_embedded();
        app.insert_resource(registry)
            .init_resource::<ActiveScene>()
            .init_resource::<NearbyHotspot>()
            .add_event::<TransitionSceneEvent>()
            .add_systems(Startup, (setup_scene_camera, spawn_initial_world))
            .add_systems(
                Update,
                (
                    sync_camera_viewport,
                    apply_scene_transitions,
                    sync_local_player_character,
                    handle_point_and_click_input,
                    move_detective_player,
                    sync_remote_peer_entities,
                    camera_follow_player,
                    animate_rain_and_neon,
                    sync_scene_cypher_badges,
                    update_hotspot_prompts,
                )
                    .chain()
                    .after(crate::stories::StorySystems),
            );
    }
}

fn setup_scene_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 999.0),
        SceneCamera,
    ));
}

fn sync_camera_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<SceneCamera>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let phys_w = window.physical_width();
    let phys_h = window.physical_height();
    if phys_w == 0 || phys_h == 0 {
        return;
    }
    let scene_w = ((phys_w as f32) * SCENE_WIDTH_FRACTION).round() as u32;
    for mut cam in &mut cameras {
        cam.viewport = Some(Viewport {
            physical_position: UVec2::ZERO,
            physical_size: UVec2::new(scene_w.max(1), phys_h.max(1)),
            ..default()
        });
    }
}

fn spawn_initial_world(
    mut commands: Commands,
    registry: Res<SceneRegistry>,
    sheet: Res<DetectiveSheet>,
    mut active_scene: ResMut<ActiveScene>,
    asset_server: Option<Res<AssetServer>>,
) {
    let srv = asset_server.as_deref();

    active_scene.story_name = registry.story_name.clone();
    if let Some(initial) = registry.scenes.get(&registry.initial_scene) {
        active_scene.current_id = initial.id.clone();
        active_scene.current_title = initial.title.clone();
        active_scene.environment = initial.environment;
        active_scene.walk_bounds = Vec2::new(initial.walk_bounds.0, initial.walk_bounds.1);
        active_scene.camera_pan_bounds =
            Vec2::new(initial.camera_pan_bounds.0, initial.camera_pan_bounds.1);
        initial.spawn_entities(&mut commands, srv, &active_scene.saved_pulses);
    }

    // Persistent Player Detective — dynamically reflects the selected DetectiveId!
    let player_visual = if let Some(s) = srv {
        Sprite {
            image: s.load(sheet.character.sprite_path()),
            custom_size: Some(Vec2::splat(68.0)),
            ..default()
        }
    } else {
        Sprite {
            color: sheet.character.accent_color(),
            custom_size: Some(Vec2::new(26.0, 26.0)),
            ..default()
        }
    };

    let spawn_pos = registry.initial_spawn;
    commands
        .spawn((
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 4.0),
            Visibility::Visible,
            DetectivePlayer {
                speed: sheet.character.walk_speed(),
                walk_phase: 0.0,
                is_moving: false,
                waypoints: Vec::new(),
                pending_interaction: None,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite {
                    color: Color::srgba(0.02, 0.02, 0.04, 0.55),
                    custom_size: Some(Vec2::new(32.0, 10.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, -28.0, -0.2),
            ));
            parent.spawn((
                Sprite {
                    color: sheet.character.accent_color().with_alpha(0.32),
                    custom_size: Some(Vec2::splat(54.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, -0.1),
                Visibility::Hidden,
                LocalPlayerSenseAura,
            ));
            parent.spawn((
                player_visual,
                Transform::from_xyz(0.0, 0.0, 0.1),
                CharacterVisualSprite,
            ));
        });

    // Persistent Point-and-Click Destination Marker & Hover Prompts
    commands.spawn((
        Sprite {
            color: Color::srgba(0.94, 0.78, 0.42, 0.55),
            custom_size: Some(Vec2::new(22.0, 9.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.4),
        Visibility::Hidden,
        ClickTargetMarker,
    ));

    commands.spawn((
        Sprite {
            color: Color::srgba(0.95, 0.78, 0.38, 0.25),
            custom_size: Some(Vec2::new(56.0, 56.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.6),
        Visibility::Hidden,
        HotspotHighlightRing,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 13.5,
            ..default()
        },
        TextColor(Color::srgb(0.98, 0.88, 0.56)),
        Transform::from_xyz(0.0, 0.0, 6.0),
        Visibility::Hidden,
        WorldPromptLabel,
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_scene_transitions(
    mut commands: Commands,
    mut events: EventReader<TransitionSceneEvent>,
    registry: Res<SceneRegistry>,
    mut active_scene: ResMut<ActiveScene>,
    nar_state: Res<NarrativeState>,
    mut cypher_modal: Option<ResMut<ActiveCypherModal>>,
    asset_server: Option<Res<AssetServer>>,
    scoped_q: Query<Entity, With<SceneScoped>>,
    heartbeats_q: Query<&SuspectHeartbeat>,
    mut player_q: Query<(&mut DetectivePlayer, &mut Transform), Without<SceneCamera>>,
    mut cam_q: Query<&mut Transform, (With<SceneCamera>, Without<DetectivePlayer>)>,
    mut marker_q: Query<&mut Visibility, With<ClickTargetMarker>>,
    mut enter_writer: EventWriter<EnterNodeEvent>,
) {
    let Some(ev) = events.read().last().cloned() else {
        return;
    };
    let Some(next_scene) = registry.scenes.get(&ev.target_scene_id) else {
        return;
    };

    if let Some(ref mut cm) = cypher_modal {
        cm.close();
    }

    // Preserve any suspect heartbeat states (e.g. lie spikes) before despawning old room
    for hb in &heartbeats_q {
        active_scene
            .saved_pulses
            .insert(hb.npc_id.clone(), (hb.target_bpm, hb.is_lie_spike));
    }

    for entity in &scoped_q {
        commands.entity(entity).despawn_recursive();
    }

    active_scene.current_id = next_scene.id.clone();
    active_scene.current_title = next_scene.title.clone();
    active_scene.environment = next_scene.environment;
    active_scene.walk_bounds = Vec2::new(next_scene.walk_bounds.0, next_scene.walk_bounds.1);
    active_scene.camera_pan_bounds =
        Vec2::new(next_scene.camera_pan_bounds.0, next_scene.camera_pan_bounds.1);

    next_scene.spawn_entities(&mut commands, asset_server.as_deref(), &active_scene.saved_pulses);

    if let Ok((mut player, mut p_tf)) = player_q.get_single_mut() {
        p_tf.translation.x = ev.spawn_pos.x;
        p_tf.translation.y = ev.spawn_pos.y;
        player.is_moving = false;
        player.waypoints.clear();
        player.pending_interaction = None;
    }

    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        cam_tf.translation.x = ev
            .spawn_pos
            .x
            .clamp(-active_scene.camera_pan_bounds.x, active_scene.camera_pan_bounds.x);
        cam_tf.translation.y = ev
            .spawn_pos
            .y
            .clamp(-active_scene.camera_pan_bounds.y, active_scene.camera_pan_bounds.y);
    }

    if let Ok(mut mvis) = marker_q.get_single_mut() {
        *mvis = Visibility::Hidden;
    }

    // Close previous room's active conversation card cleanly when stepping into a new scene
    if !nar_state.current_node_id.is_empty() {
        enter_writer.send(EnterNodeEvent::fresh("LEAVE_CONVERSATION"));
    }
}

#[allow(clippy::type_complexity)]
fn sync_local_player_character(
    sheet: Res<DetectiveSheet>,
    blood_sight: Res<BloodSight>,
    asset_server: Option<Res<AssetServer>>,
    mut player_q: Query<&mut DetectivePlayer>,
    mut visual_q: Query<&mut Sprite, (With<CharacterVisualSprite>, Without<LocalPlayerSenseAura>)>,
    mut aura_q: Query<(&mut Visibility, &mut Sprite), (With<LocalPlayerSenseAura>, Without<CharacterVisualSprite>)>,
    mut last_char: Local<Option<DetectiveId>>,
) {
    if *last_char != Some(sheet.character) {
        *last_char = Some(sheet.character);
        if let Ok(mut player) = player_q.get_single_mut() {
            player.speed = sheet.character.walk_speed();
        }
        if let Ok(mut vsprite) = visual_q.get_single_mut() {
            if let Some(srv) = &asset_server {
                vsprite.image = srv.load(sheet.character.sprite_path());
                vsprite.color = Color::WHITE;
                vsprite.custom_size = Some(Vec2::splat(68.0));
            } else {
                vsprite.color = sheet.character.accent_color();
            }
        }
    }

    if let Ok((mut avis, mut asprite)) = aura_q.get_single_mut() {
        if blood_sight.active {
            *avis = Visibility::Visible;
            let pulse = (blood_sight.pulse_phase.sin() * 0.5 + 0.5) * 0.16;
            asprite.color = sheet.character.accent_color().with_alpha(0.22 + pulse);
        } else {
            *avis = Visibility::Hidden;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_point_and_click_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    active_scene: Res<ActiveScene>,
    (ledger_modal, mp_modal, char_modal, story_modal, cypher_modal): (
        Res<LedgerModalOpen>,
        Res<MultiplayerModalOpen>,
        Res<CharacterSelectModalOpen>,
        Res<StorySelectModalOpen>,
        Option<Res<ActiveCypherModal>>,
    ),
    blood_sight: Res<BloodSight>,
    ledger: Res<ClueLedger>,
    cam_q: Query<&Transform, (With<SceneCamera>, Without<ClickTargetMarker>)>,
    colliders: Query<
        (&Transform, &SolidCollider),
        (Without<DetectivePlayer>, Without<ClickTargetMarker>),
    >,
    hotspots: Query<(&Transform, &InteractableHotspot), Without<ClickTargetMarker>>,
    doorways: Query<(&Transform, &SceneDoorway), Without<ClickTargetMarker>>,
    mut player_q: Query<(&Transform, &mut DetectivePlayer), Without<ClickTargetMarker>>,
    mut marker_q: Query<(&mut Transform, &mut Visibility), With<ClickTargetMarker>>,
    (mut enter_writer, mut cypher_writer, mut scene_writer): (
        EventWriter<EnterNodeEvent>,
        EventWriter<OpenCypherEvent>,
        EventWriter<TransitionSceneEvent>,
    ),
) {
    if ledger_modal.0
        || mp_modal.open
        || char_modal.0
        || story_modal.0
        || cypher_modal.as_deref().is_some_and(|m| m.open)
    {
        return;
    }
    let clicked = mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right);
    if !clicked {
        return;
    }

    let (Ok(window), Ok(cam_tf), Ok((player_tf, mut player))) = (
        windows.get_single(),
        cam_q.get_single(),
        player_q.get_single_mut(),
    ) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let scene_width = window.width() * SCENE_WIDTH_FRACTION;
    if cursor.x > scene_width || cursor.y < 36.0 || cursor.y > window.height() - 34.0 {
        return;
    }

    let wb = active_scene.walk_bounds;
    let world_click = Vec2::new(
        (cam_tf.translation.x + (cursor.x - scene_width * 0.5)).clamp(-wb.x, wb.x),
        (cam_tf.translation.y + (window.height() * 0.5 - cursor.y)).clamp(-wb.y, wb.y),
    );
    let start_pos = player_tf.translation.truncate();

    let obstacle_boxes: Vec<(Vec2, Vec2)> = colliders
        .iter()
        .map(|(t, c)| (t.translation.truncate(), c.half_extents))
        .collect();

    // 1. Check if player clicked on a Scene Doorway!
    for (dtf, door) in &doorways {
        let d_pos = dtf.translation.truncate();
        if world_click.distance(d_pos) <= 48.0 {
            if start_pos.distance(d_pos) <= door.trigger_radius {
                player.waypoints.clear();
                player.pending_interaction = None;
                if let Ok((_, mut vis)) = marker_q.get_single_mut() {
                    *vis = Visibility::Hidden;
                }
                scene_writer.send(TransitionSceneEvent {
                    target_scene_id: door.target_scene_id.clone(),
                    spawn_pos: door.target_spawn,
                });
                return;
            }

            let approach = find_nearest_walkable_in_bounds(d_pos, start_pos, wb, &obstacle_boxes);
            player.waypoints =
                compute_astar_path_in_bounds(start_pos, approach, wb, &obstacle_boxes);
            player.pending_interaction = Some(PendingInteraction::Doorway {
                pos: d_pos,
                radius: door.trigger_radius,
                target_scene_id: door.target_scene_id.clone(),
                target_spawn: door.target_spawn,
            });
            if let Ok((mut mtf, mut mvis)) = marker_q.get_single_mut() {
                mtf.translation.x = d_pos.x;
                mtf.translation.y = d_pos.y;
                *mvis = Visibility::Visible;
            }
            return;
        }
    }

    // 2. Check if player clicked on an interactable Clue or Character!
    let mut clicked_hotspot: Option<(Vec2, &InteractableHotspot)> = None;
    for (htf, hs) in &hotspots {
        if let Some(clue_id) = &hs.requires_blood_sight_unless_clue {
            if !blood_sight.active && !ledger.has_clue(clue_id) {
                continue;
            }
        }
        let h_pos = htf.translation.truncate();
        if world_click.distance(h_pos) <= 46.0 {
            clicked_hotspot = Some((h_pos, hs));
            break;
        }
    }

    if let Some((h_pos, hs)) = clicked_hotspot {
        if start_pos.distance(h_pos) <= hs.radius {
            player.waypoints.clear();
            player.pending_interaction = None;
            if let Ok((_, mut vis)) = marker_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            if let Some(cid) = &hs.cypher_id {
                cypher_writer.send(OpenCypherEvent(cid.clone()));
            } else {
                enter_writer.send(EnterNodeEvent::fresh(hs.node_id.clone()));
            }
            return;
        }

        let approach = find_nearest_walkable_in_bounds(h_pos, start_pos, wb, &obstacle_boxes);
        player.waypoints = compute_astar_path_in_bounds(start_pos, approach, wb, &obstacle_boxes);
        player.pending_interaction = Some(PendingInteraction::Hotspot {
            pos: h_pos,
            radius: hs.radius,
            node_id: hs.node_id.clone(),
            cypher_id: hs.cypher_id.clone(),
        });

        if let Ok((mut mtf, mut mvis)) = marker_q.get_single_mut() {
            mtf.translation.x = h_pos.x;
            mtf.translation.y = h_pos.y - 24.0;
            *mvis = Visibility::Visible;
        }
    } else {
        // 3. Walk to clicked floor/street point!
        let goal = find_nearest_walkable_in_bounds(world_click, start_pos, wb, &obstacle_boxes);
        player.waypoints = compute_astar_path_in_bounds(start_pos, goal, wb, &obstacle_boxes);
        player.pending_interaction = None;

        if let Ok((mut mtf, mut mvis)) = marker_q.get_single_mut() {
            mtf.translation.x = goal.x;
            mtf.translation.y = goal.y - 22.0;
            *mvis = Visibility::Visible;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn move_detective_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    active_scene: Res<ActiveScene>,
    (mp_modal, char_modal, story_modal, cypher_modal): (
        Res<MultiplayerModalOpen>,
        Res<CharacterSelectModalOpen>,
        Res<StorySelectModalOpen>,
        Option<Res<ActiveCypherModal>>,
    ),
    colliders: Query<
        (&Transform, &SolidCollider),
        (
            Without<DetectivePlayer>,
            Without<CharacterVisualSprite>,
            Without<ClickTargetMarker>,
        ),
    >,
    mut player_q: Query<
        (&mut DetectivePlayer, &mut Transform),
        (Without<CharacterVisualSprite>, Without<ClickTargetMarker>),
    >,
    mut visual_q: Query<
        (&mut Transform, &mut Sprite),
        (
            With<CharacterVisualSprite>,
            Without<DetectivePlayer>,
            Without<ClickTargetMarker>,
        ),
    >,
    mut marker_q: Query<
        &mut Visibility,
        (
            With<ClickTargetMarker>,
            Without<DetectivePlayer>,
            Without<CharacterVisualSprite>,
        ),
    >,
    (mut enter_writer, mut cypher_writer, mut scene_writer): (
        EventWriter<EnterNodeEvent>,
        EventWriter<OpenCypherEvent>,
        EventWriter<TransitionSceneEvent>,
    ),
) {
    let Ok((mut player, mut transform)) = player_q.get_single_mut() else {
        return;
    };

    let mut key_dir = Vec2::ZERO;
    if !mp_modal.open
        && !char_modal.0
        && !story_modal.0
        && !cypher_modal.as_deref().is_some_and(|m| m.open)
    {
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            key_dir.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            key_dir.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            key_dir.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            key_dir.x += 1.0;
        }
    }

    if key_dir != Vec2::ZERO {
        player.waypoints.clear();
        player.pending_interaction = None;
        if let Ok(mut mvis) = marker_q.get_single_mut() {
            *mvis = Visibility::Hidden;
        }
    }

    let dt = time.delta_secs();
    let player_half = Vec2::splat(13.0);
    let wb = active_scene.walk_bounds;
    let mut move_vec = Vec2::ZERO;

    if key_dir != Vec2::ZERO {
        move_vec = key_dir.normalize() * player.speed * dt;
    } else if let Some(&next_wp) = player.waypoints.first() {
        let cur = transform.translation.truncate();
        let to_wp = next_wp - cur;
        let dist = to_wp.length();
        let step = player.speed * dt;
        if dist <= step.max(4.0) {
            transform.translation.x = next_wp.x;
            transform.translation.y = next_wp.y;
            player.waypoints.remove(0);
            if player.waypoints.is_empty() {
                if let Ok(mut mvis) = marker_q.get_single_mut() {
                    *mvis = Visibility::Hidden;
                }
            }
        } else {
            move_vec = (to_wp / dist) * step;
        }
    }

    if move_vec != Vec2::ZERO {
        player.is_moving = true;
        player.walk_phase += dt * 13.5;

        if let Ok((mut vtf, mut vsprite)) = visual_q.get_single_mut() {
            if move_vec.x < -0.05 {
                vsprite.flip_x = true;
            } else if move_vec.x > 0.05 {
                vsprite.flip_x = false;
            }
            vtf.translation.y = player.walk_phase.sin().abs() * 3.0;
        }

        let cand_x = Vec2::new(
            (transform.translation.x + move_vec.x).clamp(-wb.x, wb.x),
            transform.translation.y,
        );
        if !overlaps_any(cand_x, player_half, &colliders) {
            transform.translation.x = cand_x.x;
        }

        let cand_y = Vec2::new(
            transform.translation.x,
            (transform.translation.y + move_vec.y).clamp(-wb.y, wb.y),
        );
        if !overlaps_any(cand_y, player_half, &colliders) {
            transform.translation.y = cand_y.y;
        }
    } else {
        player.is_moving = false;
        if let Ok((mut vtf, _)) = visual_q.get_single_mut() {
            vtf.translation.y *= 0.8;
        }
    }

    // Trigger pending clue/character inspection or doorway transition upon arrival!
    if let Some(pending) = player.pending_interaction.clone() {
        let cur = transform.translation.truncate();
        match pending {
            PendingInteraction::Hotspot {
                pos,
                radius,
                node_id,
                cypher_id,
            } => {
                if cur.distance(pos) <= radius + 6.0 || player.waypoints.is_empty() {
                    player.waypoints.clear();
                    player.pending_interaction = None;
                    if let Ok(mut mvis) = marker_q.get_single_mut() {
                        *mvis = Visibility::Hidden;
                    }
                    if cur.distance(pos) <= radius + 24.0 {
                        if let Some(cid) = cypher_id {
                            cypher_writer.send(OpenCypherEvent(cid));
                        } else {
                            enter_writer.send(EnterNodeEvent::fresh(node_id));
                        }
                    }
                }
            }
            PendingInteraction::Doorway {
                pos,
                radius,
                target_scene_id,
                target_spawn,
            } => {
                if cur.distance(pos) <= radius + 8.0 || player.waypoints.is_empty() {
                    player.waypoints.clear();
                    player.pending_interaction = None;
                    if let Ok(mut mvis) = marker_q.get_single_mut() {
                        *mvis = Visibility::Hidden;
                    }
                    if cur.distance(pos) <= radius + 28.0 {
                        scene_writer.send(TransitionSceneEvent {
                            target_scene_id,
                            spawn_pos: target_spawn,
                        });
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_remote_peer_entities(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<NetSession>,
    active_scene: Res<ActiveScene>,
    asset_server: Option<Res<AssetServer>>,
    mut actors_q: Query<(Entity, &mut RemotePeerActor, &mut Transform, &mut Visibility)>,
    mut visuals_q: Query<
        (&mut RemotePeerVisual, &mut Transform, &mut Sprite),
        (Without<RemotePeerActor>, Without<RemotePeerSightAura>),
    >,
    mut plates_q: Query<(&RemotePeerNameplate, &mut Text2d, &mut TextColor)>,
    mut auras_q: Query<
        (&RemotePeerSightAura, &mut Visibility, &mut Sprite),
        (Without<RemotePeerActor>, Without<RemotePeerVisual>),
    >,
) {
    let mut existing_ids = HashMap::new();
    for (entity, actor, _, _) in &actors_q {
        existing_ids.insert(actor.peer_id, entity);
    }

    for (pid, entity) in &existing_ids {
        if !session.peers.contains_key(pid) {
            commands.entity(*entity).despawn_recursive();
        }
    }

    for (&pid, snap) in &session.peers {
        if !existing_ids.contains_key(&pid) {
            let accent = snap.character.accent_color();
            let partner_sprite = if let Some(srv) = &asset_server {
                Sprite {
                    image: srv.load(snap.character.sprite_path()),
                    custom_size: Some(Vec2::splat(68.0)),
                    flip_x: snap.flip_x,
                    ..default()
                }
            } else {
                Sprite {
                    color: accent,
                    custom_size: Some(Vec2::new(26.0, 26.0)),
                    ..default()
                }
            };

            let in_same_scene =
                snap.scene_id.is_empty() || snap.scene_id == active_scene.current_id;

            commands
                .spawn((
                    Transform::from_xyz(snap.x, snap.y, 3.9),
                    if in_same_scene {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    RemotePeerActor {
                        peer_id: pid,
                        walk_phase: 0.0,
                    },
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite {
                            color: Color::srgba(0.02, 0.02, 0.04, 0.50),
                            custom_size: Some(Vec2::new(32.0, 10.0)),
                            ..default()
                        },
                        Transform::from_xyz(0.0, -28.0, -0.2),
                    ));
                    parent.spawn((
                        Sprite {
                            color: accent.with_alpha(0.32),
                            custom_size: Some(Vec2::splat(52.0)),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, -0.1),
                        Visibility::Hidden,
                        RemotePeerSightAura { peer_id: pid },
                    ));
                    parent.spawn((
                        partner_sprite,
                        Transform::from_xyz(0.0, 0.0, 0.1),
                        RemotePeerVisual {
                            peer_id: pid,
                            character: snap.character,
                        },
                    ));
                    parent.spawn((
                        Text2d::new(format!("{} (#{pid})", snap.character.display_name())),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(accent),
                        Transform::from_xyz(0.0, 40.0, 0.5),
                        RemotePeerNameplate { peer_id: pid },
                    ));
                });
        }
    }

    let dt = time.delta_secs();
    let lerp_t = (dt * 12.0).min(1.0);
    for (_, mut actor, mut tf, mut vis) in &mut actors_q {
        if let Some(snap) = session.peers.get(&actor.peer_id) {
            let in_same_scene =
                snap.scene_id.is_empty() || snap.scene_id == active_scene.current_id;
            *vis = if in_same_scene {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            tf.translation.x += (snap.x - tf.translation.x) * lerp_t;
            tf.translation.y += (snap.y - tf.translation.y) * lerp_t;
            if snap.moving {
                actor.walk_phase += dt * 13.0;
            } else {
                actor.walk_phase = 0.0;
            }
        }
    }

    for (mut vis_tag, mut vtf, mut sprite) in &mut visuals_q {
        if let Some(snap) = session.peers.get(&vis_tag.peer_id) {
            if vis_tag.character != snap.character {
                vis_tag.character = snap.character;
                if let Some(srv) = &asset_server {
                    sprite.image = srv.load(snap.character.sprite_path());
                    sprite.color = Color::WHITE;
                } else {
                    sprite.color = snap.character.accent_color();
                }
            }
            sprite.flip_x = snap.flip_x;
            if snap.moving {
                vtf.translation.y = (time.elapsed_secs() * 13.0).sin().abs() * 3.0;
            } else {
                vtf.translation.y *= 0.8;
            }
        }
    }

    for (plate, mut txt, mut col) in &mut plates_q {
        if let Some(snap) = session.peers.get(&plate.peer_id) {
            txt.0 = format!("{} (#{})", snap.character.display_name(), plate.peer_id);
            col.0 = snap.character.accent_color();
        }
    }

    for (aura, mut vis, mut asprite) in &mut auras_q {
        if let Some(snap) = session.peers.get(&aura.peer_id) {
            let in_same_scene =
                snap.scene_id.is_empty() || snap.scene_id == active_scene.current_id;
            asprite.color = snap.character.accent_color().with_alpha(0.32);
            *vis = if in_same_scene && snap.blood_sight {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn overlaps_any(
    pos: Vec2,
    half: Vec2,
    colliders: &Query<
        (&Transform, &SolidCollider),
        (
            Without<DetectivePlayer>,
            Without<CharacterVisualSprite>,
            Without<ClickTargetMarker>,
        ),
    >,
) -> bool {
    for (ct, col) in colliders {
        let cpos = ct.translation.truncate();
        if (pos.x - cpos.x).abs() < (half.x + col.half_extents.x)
            && (pos.y - cpos.y).abs() < (half.y + col.half_extents.y)
        {
            return true;
        }
    }
    false
}

// ============================================================================
// SCENE-AWARE 2D GRID A* PATHFINDING
// ============================================================================

const GRID_STEP: f32 = 16.0;

fn cell_to_world(cx: i32, cy: i32) -> Vec2 {
    Vec2::new(cx as f32 * GRID_STEP, cy as f32 * GRID_STEP)
}

fn world_to_cell(p: Vec2) -> (i32, i32) {
    (
        (p.x / GRID_STEP).round() as i32,
        (p.y / GRID_STEP).round() as i32,
    )
}

fn point_blocked_in_bounds(pos: Vec2, pad: f32, bounds: Vec2, obstacles: &[(Vec2, Vec2)]) -> bool {
    if pos.x < -bounds.x || pos.x > bounds.x || pos.y < -bounds.y || pos.y > bounds.y {
        return true;
    }
    for (center, half) in obstacles {
        if (pos.x - center.x).abs() < (half.x + pad) && (pos.y - center.y).abs() < (half.y + pad) {
            return true;
        }
    }
    false
}

fn find_nearest_walkable_in_bounds(
    desired: Vec2,
    from: Vec2,
    bounds: Vec2,
    obstacles: &[(Vec2, Vec2)],
) -> Vec2 {
    let clamped = Vec2::new(
        desired.x.clamp(-bounds.x + 2.0, bounds.x - 2.0),
        desired.y.clamp(-bounds.y + 2.0, bounds.y - 2.0),
    );
    if !point_blocked_in_bounds(clamped, 14.0, bounds, obstacles) {
        return clamped;
    }
    let dir_to_player = (from - clamped).normalize_or_zero();
    for dist in [32.0, 46.0, 60.0, 74.0] {
        let preferred = clamped + dir_to_player * dist;
        if !point_blocked_in_bounds(preferred, 14.0, bounds, obstacles) {
            return preferred;
        }
        for angle_idx in 0..12 {
            let ang = (angle_idx as f32) * std::f32::consts::TAU / 12.0;
            let cand = clamped + Vec2::from_angle(ang) * dist;
            if !point_blocked_in_bounds(cand, 14.0, bounds, obstacles) {
                return cand;
            }
        }
    }
    from
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    f_cost: i32,
    g_cost: i32,
    cell: (i32, i32),
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost)
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[allow(dead_code)]
pub fn compute_astar_path(start: Vec2, goal: Vec2, obstacles: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    compute_astar_path_in_bounds(start, goal, Vec2::new(296.0, 296.0), obstacles)
}

pub fn compute_astar_path_in_bounds(
    start: Vec2,
    goal: Vec2,
    bounds: Vec2,
    obstacles: &[(Vec2, Vec2)],
) -> Vec<Vec2> {
    if has_clear_line_in_bounds(start, goal, bounds, obstacles) {
        return vec![goal];
    }

    let max_cx = (bounds.x / GRID_STEP).floor() as i32;
    let max_cy = (bounds.y / GRID_STEP).floor() as i32;

    let start_c = (
        world_to_cell(start).0.clamp(-max_cx, max_cx),
        world_to_cell(start).1.clamp(-max_cy, max_cy),
    );
    let goal_c = (
        world_to_cell(goal).0.clamp(-max_cx, max_cx),
        world_to_cell(goal).1.clamp(-max_cy, max_cy),
    );

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_scores: HashMap<(i32, i32), i32> = HashMap::new();

    g_scores.insert(start_c, 0);
    open.push(AStarNode {
        f_cost: octile_h(start_c, goal_c),
        g_cost: 0,
        cell: start_c,
    });

    let dirs = [
        (-1, 0, 10),
        (1, 0, 10),
        (0, -1, 10),
        (0, 1, 10),
        (-1, -1, 14),
        (-1, 1, 14),
        (1, -1, 14),
        (1, 1, 14),
    ];

    let mut reached = None;
    let mut best_fallback = (octile_h(start_c, goal_c), start_c);

    while let Some(current) = open.pop() {
        if current.cell == goal_c {
            reached = Some(goal_c);
            break;
        }
        let h = octile_h(current.cell, goal_c);
        if h < best_fallback.0 {
            best_fallback = (h, current.cell);
        }

        let cur_g = *g_scores.get(&current.cell).unwrap_or(&i32::MAX);
        if current.g_cost > cur_g {
            continue;
        }

        for (dx, dy, step_cost) in dirs {
            let nx = current.cell.0 + dx;
            let ny = current.cell.1 + dy;
            if !(-max_cx..=max_cx).contains(&nx) || !(-max_cy..=max_cy).contains(&ny) {
                continue;
            }
            let wpos = cell_to_world(nx, ny);
            if (nx, ny) != goal_c && point_blocked_in_bounds(wpos, 13.5, bounds, obstacles) {
                continue;
            }
            if dx != 0 && dy != 0 {
                let side_a = cell_to_world(current.cell.0 + dx, current.cell.1);
                let side_b = cell_to_world(current.cell.0, current.cell.1 + dy);
                if point_blocked_in_bounds(side_a, 13.5, bounds, obstacles)
                    || point_blocked_in_bounds(side_b, 13.5, bounds, obstacles)
                {
                    continue;
                }
            }

            let tentative_g = cur_g + step_cost;
            if tentative_g < *g_scores.get(&(nx, ny)).unwrap_or(&i32::MAX) {
                came_from.insert((nx, ny), current.cell);
                g_scores.insert((nx, ny), tentative_g);
                open.push(AStarNode {
                    f_cost: tentative_g + octile_h((nx, ny), goal_c),
                    g_cost: tentative_g,
                    cell: (nx, ny),
                });
            }
        }
    }

    let end_cell = reached.unwrap_or(best_fallback.1);
    let mut cells = vec![end_cell];
    let mut curr = end_cell;
    while let Some(&prev) = came_from.get(&curr) {
        if prev == start_c {
            break;
        }
        cells.push(prev);
        curr = prev;
    }
    cells.reverse();

    let mut raw_pts: Vec<Vec2> = cells.into_iter().map(|(x, y)| cell_to_world(x, y)).collect();
    if reached.is_some() {
        if let Some(last) = raw_pts.last_mut() {
            *last = goal;
        } else {
            raw_pts.push(goal);
        }
    }

    let mut smoothed = Vec::new();
    let mut anchor = start;
    let mut idx = 0;
    while idx < raw_pts.len() {
        let mut furthest = idx;
        for probe in (idx + 1)..raw_pts.len() {
            if has_clear_line_in_bounds(anchor, raw_pts[probe], bounds, obstacles) {
                furthest = probe;
            }
        }
        anchor = raw_pts[furthest];
        smoothed.push(anchor);
        idx = furthest + 1;
    }
    smoothed
}

fn has_clear_line_in_bounds(
    a: Vec2,
    b: Vec2,
    bounds: Vec2,
    obstacles: &[(Vec2, Vec2)],
) -> bool {
    let dist = a.distance(b);
    let steps = ((dist / 10.0).ceil() as usize).max(1);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        if point_blocked_in_bounds(a.lerp(b, t), 13.0, bounds, obstacles) {
            return false;
        }
    }
    true
}

fn octile_h(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    10 * (dx + dy) - 6 * dx.min(dy)
}

fn camera_follow_player(
    time: Res<Time>,
    active_scene: Res<ActiveScene>,
    player_q: Query<&Transform, (With<DetectivePlayer>, Without<SceneCamera>)>,
    mut cam_q: Query<&mut Transform, (With<SceneCamera>, Without<DetectivePlayer>)>,
) {
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let Ok(mut cam_tf) = cam_q.get_single_mut() else {
        return;
    };

    let pan = active_scene.camera_pan_bounds;
    let target_x = p_tf.translation.x.clamp(-pan.x, pan.x);
    let target_y = p_tf.translation.y.clamp(-pan.y, pan.y);
    let smooth = (time.delta_secs() * 5.0).min(1.0);
    cam_tf.translation.x += (target_x - cam_tf.translation.x) * smooth;
    cam_tf.translation.y += (target_y - cam_tf.translation.y) * smooth;
}

fn animate_rain_and_neon(
    time: Res<Time>,
    mut rain_q: Query<(&RainDrop, &mut Transform)>,
    mut neon_q: Query<(&NeonFlicker, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (drop, mut tf) in &mut rain_q {
        tf.translation.x += drop.velocity.x * dt;
        tf.translation.y += drop.velocity.y * dt;
        if tf.translation.y < drop.min_y {
            tf.translation.y = drop.reset_y;
        }
        if tf.translation.x < -drop.wrap_half_w {
            tf.translation.x += drop.wrap_half_w * 2.0;
        }
    }

    let t = time.elapsed_secs();
    for (neon, mut sprite) in &mut neon_q {
        let flicker = if (t * 7.3).sin() > 0.92 {
            0.06
        } else {
            neon.base_alpha
        };
        sprite.color = sprite.color.with_alpha(flicker);
    }
}

fn sync_scene_cypher_badges(
    time: Res<Time>,
    ledger: Res<ClueLedger>,
    mut glow_q: Query<(&SceneCypherGlowPlate, &mut Sprite)>,
    mut badge_q: Query<(&SceneCypherBadgeText, &mut Text2d, &mut TextColor)>,
) {
    let pulse = (time.elapsed_secs() * 2.8).sin().abs() * 0.12;
    for (plate, mut sprite) in &mut glow_q {
        if ledger.is_cypher_solved(&plate.cypher_id) {
            sprite.color = Color::srgba(0.28, 0.86, 0.52, 0.20);
        } else {
            sprite.color = Color::srgba(0.96, 0.78, 0.28, 0.18 + pulse);
        }
    }
    for (badge, mut txt, mut col) in &mut badge_q {
        if ledger.is_cypher_solved(&badge.cypher_id) {
            if txt.0 != "[✓ DECODED]" {
                txt.0 = "[✓ DECODED]".to_string();
            }
            col.0 = Color::srgb(0.46, 0.92, 0.62);
        } else {
            if txt.0 != "[CYPHER]" {
                txt.0 = "[CYPHER]".to_string();
            }
            col.0 = Color::srgb(0.98, 0.84, 0.38);
        }
    }
}

#[derive(Clone)]
enum NearbyCandidate {
    Hotspot {
        label: String,
        node_id: String,
        cypher_id: Option<String>,
    },
    Doorway {
        label: String,
        target_scene_id: String,
        target_spawn: Vec2,
    },
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_hotspot_prompts(
    keys: Res<ButtonInput<KeyCode>>,
    (mp_modal, char_modal, story_modal, cypher_modal): (
        Res<MultiplayerModalOpen>,
        Res<CharacterSelectModalOpen>,
        Res<StorySelectModalOpen>,
        Option<Res<ActiveCypherModal>>,
    ),
    player_q: Query<
        &Transform,
        (
            With<DetectivePlayer>,
            Without<HotspotHighlightRing>,
            Without<WorldPromptLabel>,
        ),
    >,
    blood_sight: Res<BloodSight>,
    ledger: Res<ClueLedger>,
    hotspots: Query<
        (&Transform, &InteractableHotspot),
        (Without<HotspotHighlightRing>, Without<WorldPromptLabel>),
    >,
    doorways: Query<
        (&Transform, &SceneDoorway),
        (Without<HotspotHighlightRing>, Without<WorldPromptLabel>),
    >,
    mut nearby: ResMut<NearbyHotspot>,
    mut highlight_q: Query<
        (&mut Transform, &mut Visibility),
        (With<HotspotHighlightRing>, Without<WorldPromptLabel>),
    >,
    mut label_q: Query<
        (&mut Transform, &mut Visibility, &mut Text2d),
        (With<WorldPromptLabel>, Without<HotspotHighlightRing>),
    >,
    (mut enter_writer, mut cypher_writer, mut scene_writer): (
        EventWriter<EnterNodeEvent>,
        EventWriter<OpenCypherEvent>,
        EventWriter<TransitionSceneEvent>,
    ),
) {
    let Ok(player_tf) = player_q.get_single() else {
        return;
    };
    let p_pos = player_tf.translation.truncate();

    let mut best: Option<(f32, Vec2, NearbyCandidate)> = None;

    for (htf, hs) in &hotspots {
        if let Some(clue_id) = &hs.requires_blood_sight_unless_clue {
            if !blood_sight.active && !ledger.has_clue(clue_id) {
                continue;
            }
        }
        let h_pos = htf.translation.truncate();
        let dist = p_pos.distance(h_pos);
        if dist <= hs.radius && (best.is_none() || dist < best.as_ref().unwrap().0) {
            best = Some((
                dist,
                h_pos,
                NearbyCandidate::Hotspot {
                    label: hs.label.clone(),
                    node_id: hs.node_id.clone(),
                    cypher_id: hs.cypher_id.clone(),
                },
            ));
        }
    }

    for (dtf, dw) in &doorways {
        let d_pos = dtf.translation.truncate();
        let dist = p_pos.distance(d_pos);
        if dist <= dw.trigger_radius && (best.is_none() || dist < best.as_ref().unwrap().0) {
            best = Some((
                dist,
                d_pos,
                NearbyCandidate::Doorway {
                    label: dw.label.clone(),
                    target_scene_id: dw.target_scene_id.clone(),
                    target_spawn: dw.target_spawn,
                },
            ));
        }
    }

    if let Some((_, pos, cand)) = best {
        let display_label = match &cand {
            NearbyCandidate::Hotspot {
                label,
                node_id,
                cypher_id,
            } => {
                nearby.label = Some(label.clone());
                nearby.node_id = if cypher_id.is_some() {
                    None
                } else {
                    Some(node_id.clone())
                };
                label.clone()
            }
            NearbyCandidate::Doorway { label, .. } => {
                nearby.label = Some(label.clone());
                nearby.node_id = None;
                label.clone()
            }
        };

        if let Ok((mut ring_tf, mut ring_vis)) = highlight_q.get_single_mut() {
            ring_tf.translation.x = pos.x;
            ring_tf.translation.y = pos.y;
            *ring_vis = Visibility::Visible;
        }
        if let Ok((mut lbl_tf, mut lbl_vis, mut txt)) = label_q.get_single_mut() {
            lbl_tf.translation.x = pos.x.clamp(-220.0, 220.0);
            lbl_tf.translation.y = (pos.y + 44.0).clamp(-190.0, 195.0);
            txt.0 = format!("Click / [E] {display_label}");
            *lbl_vis = Visibility::Visible;
        }

        if !mp_modal.open
            && !char_modal.0
            && !story_modal.0
            && !cypher_modal.as_deref().is_some_and(|m| m.open)
            && (keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Space))
        {
            match cand {
                NearbyCandidate::Hotspot {
                    node_id,
                    cypher_id,
                    ..
                } => {
                    if let Some(cid) = cypher_id {
                        cypher_writer.send(OpenCypherEvent(cid));
                    } else {
                        enter_writer.send(EnterNodeEvent::fresh(node_id));
                    }
                }
                NearbyCandidate::Doorway {
                    target_scene_id,
                    target_spawn,
                    ..
                } => {
                    scene_writer.send(TransitionSceneEvent {
                        target_scene_id,
                        spawn_pos: target_spawn,
                    });
                }
            }
        }
    } else {
        nearby.label = None;
        nearby.node_id = None;
        if let Ok((_, mut ring_vis)) = highlight_q.get_single_mut() {
            *ring_vis = Visibility::Hidden;
        }
        if let Ok((_, mut lbl_vis, _)) = label_q.get_single_mut() {
            *lbl_vis = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astar_navigates_around_partition_wall_through_doorway() {
        let obstacles = vec![
            (Vec2::new(-182.0, -75.0), Vec2::new(134.0, 9.0)),
            (Vec2::new(182.0, -75.0), Vec2::new(134.0, 9.0)),
        ];
        let start = Vec2::new(-185.0, 60.0);
        let goal = Vec2::new(-185.0, -180.0);
        let path = compute_astar_path(start, goal, &obstacles);
        assert!(
            path.len() >= 2,
            "Path must route around partition wall through center doorway"
        );
        assert_eq!(*path.last().unwrap(), goal);
    }

    #[test]
    fn scene_catalog_parses_and_all_doorways_link_to_valid_walkable_spawns() {
        for story in crate::stories::STORIES {
            let catalog = story.load_scene_catalog();
            assert!(
                !catalog.story_name.trim().is_empty(),
                "SceneCatalog for '{}' must have a non-empty story_name",
                story.id
            );
            let registry = SceneRegistry::from_catalog(catalog);

            let initial = registry
                .scenes
                .get(&registry.initial_scene)
                .expect("initial_scene must exist in SceneRegistry");
            assert!(
                !point_blocked_in_bounds(
                    registry.initial_spawn,
                    13.0,
                    Vec2::new(initial.walk_bounds.0, initial.walk_bounds.1),
                    &initial.static_obstacle_boxes()
                ),
                "initial_spawn must not overlap any collider in initial scene of '{}'",
                story.id
            );

            assert!(
                registry.scenes.len() >= 3,
                "Expected at least 3 linked scenes in '{}'",
                story.id
            );

            for (scene_id, scene) in &registry.scenes {
                assert!(
                    !scene.doorways.is_empty(),
                    "Scene '{scene_id}' in '{}' should have at least one doorway",
                    story.id
                );
                for dw in &scene.doorways {
                    let target = registry.scenes.get(&dw.target_scene_id).unwrap_or_else(|| {
                        panic!(
                            "Doorway '{}' in '{}' (story '{}') points to missing scene '{}'",
                            dw.id, scene_id, story.id, dw.target_scene_id
                        )
                    });
                    let spawn = Vec2::new(dw.target_spawn.0, dw.target_spawn.1);
                    let target_bounds = Vec2::new(target.walk_bounds.0, target.walk_bounds.1);
                    let target_obs = target.static_obstacle_boxes();
                    assert!(
                        !point_blocked_in_bounds(spawn, 13.0, target_bounds, &target_obs),
                        "Doorway '{}' target_spawn {:?} is blocked inside '{}' (story '{}')",
                        dw.id,
                        spawn,
                        target.id,
                        story.id
                    );
                }
            }
        }
    }

    #[test]
    fn transition_scene_event_swaps_active_scene_and_moves_player() {
        use bevy::input::InputPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins((
                crate::stats::StatsPlugin,
                crate::senses::SensesPlugin,
                crate::stories::StoriesPlugin,
                crate::narrative::NarrativePlugin,
                crate::cypher::CypherPlugin,
                crate::net::NetPlugin,
                WorldPlugin,
                crate::ui::NoirUiPlugin,
            ));

        app.update();
        assert_eq!(
            app.world().resource::<ActiveScene>().current_id,
            "velvet_backroom"
        );

        app.world_mut().send_event(TransitionSceneEvent {
            target_scene_id: "harbor_street".to_string(),
            spawn_pos: Vec2::new(218.0, 58.0),
        });
        app.update();

        let active = app.world().resource::<ActiveScene>();
        assert_eq!(active.current_id, "harbor_street");
        assert_eq!(active.environment, SceneEnvironment::Street);
    }

    #[test]
    fn all_scene_sprite_paths_and_prop_kit_assets_exist_on_disk() {
        let assets_root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));
        let mut referenced = Vec::new();
        for story in crate::stories::STORIES {
            let catalog = story.load_scene_catalog();
            for scene in &catalog.scenes {
                for s in &scene.surfaces {
                    if let Some(p) = &s.sprite_path {
                        referenced.push(p.clone());
                    }
                }
                for p in &scene.props {
                    if let Some(sp) = &p.sprite_path {
                        referenced.push(sp.clone());
                    }
                }
                for c in &scene.characters {
                    referenced.push(c.sprite_path.clone());
                }
                for cl in &scene.clues {
                    if let Some(sp) = &cl.sprite_path {
                        referenced.push(sp.clone());
                    }
                }
                for cy in &scene.cyphers {
                    if let Some(sp) = &cy.sprite_path {
                        referenced.push(sp.clone());
                    }
                }
                for e in &scene.blood_echoes {
                    if let Some(sp) = &e.sprite_path {
                        referenced.push(sp.clone());
                    }
                }
            }
        }

        assert!(
            referenced.len() >= 20,
            "Expected all existing scene props, clues, rugs, and blood echoes to have sprites"
        );
        for rel in &referenced {
            let full = assets_root.join(rel);
            assert!(
                full.is_file(),
                "Referenced scene asset '{}' does not exist at {}",
                rel,
                full.display()
            );
        }

        let scene_setting_kit = [
            "props/blood_pool.png",
            "props/blood_drag_smear.png",
            "props/blood_drops.png",
            "props/blood_bootprint.png",
            "props/echo_pool.png",
            "props/echo_smear.png",
            "props/echo_drops.png",
            "props/echo_bootprint.png",
            "props/alley_dumpster.png",
            "props/overturned_poker_table.png",
            "props/mahogany_bar.png",
            "props/velvet_curtain.png",
            "props/crimson_rug.png",
            "props/scattered_papers.png",
            "props/wooden_crates.png",
            "props/rusted_drainpipe.png",
            "props/police_cruiser.png",
            "props/streetlamp.png",
            "props/detective_desk.png",
            "props/filing_cabinet.png",
            "props/vintage_jukebox.png",
            "props/club_table_chairs.png",
            "props/neon_lounge_sign.png",
            "props/phone_booth.png",
            "props/metal_trash_cans.png",
            "props/harbor_bollard_barrels.png",
            "props/fire_hydrant.png",
            "props/morgue_autopsy_table.png",
            "props/occult_candelabra.png",
            "props/crime_chalk_markers.png",
            "props/velvet_divan.png",
            "props/absinthe_tray.png",
            "props/judicial_briefcase.png",
        ];

        for rel in scene_setting_kit {
            let full = assets_root.join(rel);
            let img = image::open(&full)
                .unwrap_or_else(|e| panic!("Failed to decode {}: {e}", full.display()));
            assert!(img.width() >= 32 && img.height() >= 32);
        }
    }
}
