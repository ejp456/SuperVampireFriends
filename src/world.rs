use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

use bevy::{
    prelude::*,
    render::camera::Viewport,
    window::PrimaryWindow,
};

use crate::{
    narrative::{ClueLedger, EnterNodeEvent},
    net::NetSession,
    senses::{BloodEcho, BloodSight, HeartbeatPulseRing, MundaneAmbiance, SuspectHeartbeat},
    ui::{LedgerModalOpen, MultiplayerModalOpen},
};

/// Fraction of window width dedicated to the 2D noir crime scene viewport (left side).
pub const SCENE_WIDTH_FRACTION: f32 = 0.60;

#[derive(Component)]
pub struct SceneCamera;

#[derive(Component)]
pub struct DetectivePlayer {
    pub speed: f32,
    pub walk_phase: f32,
    pub is_moving: bool,
    pub waypoints: Vec<Vec2>,
    /// `(hotspot_pos, trigger_radius, node_id)` triggered automatically upon arrival.
    pub pending_interaction: Option<(Vec2, f32, String)>,
}

#[derive(Component)]
pub struct CharacterVisualSprite;

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
    pub radius: f32,
    pub requires_blood_sight_unless_clue: Option<String>,
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
        app.init_resource::<NearbyHotspot>()
            .add_systems(Startup, (setup_scene_camera, spawn_velvet_lounge_scene))
            .add_systems(
                Update,
                (
                    sync_camera_viewport,
                    handle_point_and_click_input,
                    move_detective_player,
                    sync_remote_peer_entities,
                    camera_follow_player,
                    animate_rain_and_neon,
                    update_hotspot_prompts,
                )
                    .chain(),
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
        Transform::from_xyz(0.0, 60.0, 999.0),
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

fn spawn_velvet_lounge_scene(mut commands: Commands, asset_server: Option<Res<AssetServer>>) {
    let load_tex = |path: &'static str| -> Option<Handle<Image>> {
        asset_server.as_ref().map(|s| s.load(path))
    };

    // --- 1. BACKROOM FLOOR & ALLEY COBBLESTONES ---
    commands.spawn((
        Sprite {
            color: Color::srgb(0.14, 0.09, 0.08),
            custom_size: Some(Vec2::new(620.0, 360.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 105.0, 0.0),
        MundaneAmbiance {
            normal_color: Color::srgb(0.14, 0.09, 0.08),
            blood_sight_color: Color::srgb(0.06, 0.06, 0.09),
        },
    ));

    commands.spawn((
        Sprite {
            color: Color::srgba(0.85, 0.55, 0.24, 0.09),
            custom_size: Some(Vec2::new(420.0, 250.0)),
            ..default()
        },
        Transform::from_xyz(10.0, 115.0, 0.5),
        MundaneAmbiance {
            normal_color: Color::srgba(0.85, 0.55, 0.24, 0.09),
            blood_sight_color: Color::srgba(0.45, 0.05, 0.12, 0.07),
        },
    ));

    commands.spawn((
        Sprite {
            color: Color::srgb(0.07, 0.08, 0.11),
            custom_size: Some(Vec2::new(620.0, 240.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -195.0, 0.0),
        MundaneAmbiance {
            normal_color: Color::srgb(0.07, 0.08, 0.11),
            blood_sight_color: Color::srgb(0.04, 0.04, 0.06),
        },
    ));

    commands.spawn((
        Sprite {
            color: Color::srgba(0.85, 0.14, 0.26, 0.20),
            custom_size: Some(Vec2::new(150.0, 48.0)),
            ..default()
        },
        Transform::from_xyz(140.0, -190.0, 0.6),
        NeonFlicker { base_alpha: 0.20 },
    ));

    // --- 2. WALLS & COLLIDERS ---
    let wall_color = Color::srgb(0.22, 0.18, 0.20);
    let walls = [
        (Vec2::new(0.0, 290.0), Vec2::new(640.0, 20.0)),
        (Vec2::new(0.0, -315.0), Vec2::new(640.0, 20.0)),
        (Vec2::new(-315.0, -12.0), Vec2::new(20.0, 620.0)),
        (Vec2::new(315.0, -12.0), Vec2::new(20.0, 620.0)),
        // Partition wall left of doorway (open gap at x: -48..48)
        (Vec2::new(-182.0, -75.0), Vec2::new(268.0, 18.0)),
        // Partition wall right of doorway
        (Vec2::new(182.0, -75.0), Vec2::new(268.0, 18.0)),
    ];
    for (pos, size) in walls {
        commands.spawn((
            Sprite {
                color: wall_color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 3.0),
            SolidCollider {
                half_extents: size * 0.5,
            },
        ));
    }

    // --- 3. FURNITURE & SET PIECES ---
    commands.spawn((
        Sprite {
            color: Color::srgb(0.46, 0.08, 0.14),
            custom_size: Some(Vec2::new(220.0, 22.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 268.0, 3.5),
    ));

    commands.spawn((
        Sprite {
            color: Color::srgb(0.28, 0.11, 0.13),
            custom_size: Some(Vec2::new(170.0, 115.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 105.0, 1.0),
        MundaneAmbiance {
            normal_color: Color::srgb(0.28, 0.11, 0.13),
            blood_sight_color: Color::srgb(0.11, 0.09, 0.13),
        },
    ));

    // Julian Vance's Corpse
    commands.spawn((
        Sprite {
            color: Color::srgba(0.38, 0.04, 0.06, 0.85),
            custom_size: Some(Vec2::new(32.0, 22.0)),
            ..default()
        },
        Transform::from_xyz(-4.0, 102.0, 1.8),
    ));
    let corpse_sprite = if let Some(tex) = load_tex("sprites/vance_corpse.png") {
        Sprite {
            image: tex,
            custom_size: Some(Vec2::splat(82.0)),
            ..default()
        }
    } else {
        Sprite {
            color: Color::srgb(0.76, 0.73, 0.68),
            custom_size: Some(Vec2::new(44.0, 20.0)),
            ..default()
        }
    };
    commands.spawn((
        corpse_sprite,
        Transform::from_xyz(0.0, 105.0, 2.0),
        InteractableHotspot {
            label: "Julian Vance's Corpse".to_string(),
            node_id: "inspect_corpse".to_string(),
            radius: 72.0,
            requires_blood_sight_unless_clue: None,
        },
    ));

    // Overturned Poker Table & Ledger
    commands.spawn((
        Sprite {
            color: Color::srgb(0.32, 0.20, 0.13),
            custom_size: Some(Vec2::new(76.0, 52.0)),
            ..default()
        },
        Transform::from_xyz(-185.0, 130.0, 2.5),
        SolidCollider {
            half_extents: Vec2::new(38.0, 26.0),
        },
        InteractableHotspot {
            label: "Overturned Table & Ledger".to_string(),
            node_id: "inspect_table".to_string(),
            radius: 82.0,
            requires_blood_sight_unless_clue: None,
        },
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.90, 0.84, 0.68),
            custom_size: Some(Vec2::new(12.0, 16.0)),
            ..default()
        },
        Transform::from_xyz(-140.0, 118.0, 2.6),
    ));

    // Lounge Bar Counter
    commands.spawn((
        Sprite {
            color: Color::srgb(0.27, 0.16, 0.11),
            custom_size: Some(Vec2::new(42.0, 175.0)),
            ..default()
        },
        Transform::from_xyz(245.0, 145.0, 2.5),
        SolidCollider {
            half_extents: Vec2::new(21.0, 87.5),
        },
    ));

    // Alley Dumpster
    commands.spawn((
        Sprite {
            color: Color::srgb(0.18, 0.23, 0.22),
            custom_size: Some(Vec2::new(84.0, 44.0)),
            ..default()
        },
        Transform::from_xyz(-185.0, -210.0, 2.5),
        SolidCollider {
            half_extents: Vec2::new(42.0, 22.0),
        },
        InteractableHotspot {
            label: "Alley Dumpster".to_string(),
            node_id: "inspect_dumpster".to_string(),
            radius: 84.0,
            requires_blood_sight_unless_clue: None,
        },
    ));

    // --- 4. HIDDEN BLOOD SIGHT ECHOES ---
    let trail_points = [
        (Vec2::new(-3.0, 246.0), Vec2::new(34.0, 18.0), 0.72),
        (Vec2::new(4.0, 224.0), Vec2::new(14.0, 22.0), 0.55),
        (Vec2::new(-2.0, 202.0), Vec2::new(12.0, 20.0), 0.50),
        (Vec2::new(3.0, 180.0), Vec2::new(13.0, 20.0), 0.48),
        (Vec2::new(-1.0, 158.0), Vec2::new(11.0, 18.0), 0.45),
        (Vec2::new(2.0, 136.0), Vec2::new(15.0, 16.0), 0.52),
    ];
    for (i, (pos, sz, alpha)) in trail_points.into_iter().enumerate() {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.95, 0.08, 0.20, alpha),
                custom_size: Some(sz),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 1.9),
            Visibility::Hidden,
            BloodEcho {
                base_alpha: alpha,
                phase_offset: i as f32 * 0.45,
            },
        ));
    }

    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.06, 0.18, 0.75),
            custom_size: Some(Vec2::new(56.0, 24.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 250.0, 2.2),
        Visibility::Hidden,
        BloodEcho {
            base_alpha: 0.65,
            phase_offset: 0.0,
        },
        InteractableHotspot {
            label: "Blood Pool [Curtain]".to_string(),
            node_id: "inspect_curtain_echo".to_string(),
            radius: 76.0,
            requires_blood_sight_unless_clue: Some("clue_curtain_kill_site".to_string()),
        },
    ));

    commands.spawn((
        Sprite {
            color: Color::srgba(0.98, 0.10, 0.22, 0.70),
            custom_size: Some(Vec2::new(24.0, 16.0)),
            ..default()
        },
        Transform::from_xyz(-152.0, -196.0, 2.8),
        Visibility::Hidden,
        BloodEcho {
            base_alpha: 0.60,
            phase_offset: 1.3,
        },
    ));

    // --- 5. LIVING SUSPECTS ---
    commands.spawn((
        Sprite {
            color: Color::srgba(0.92, 0.16, 0.28, 0.5),
            custom_size: Some(Vec2::new(54.0, 54.0)),
            ..default()
        },
        Transform::from_xyz(185.0, 155.0, 2.1),
        Visibility::Hidden,
        HeartbeatPulseRing {
            owner_npc_id: "clara".to_string(),
            base_size: 54.0,
        },
    ));
    let clara_sprite = if let Some(tex) = load_tex("sprites/clara.png") {
        Sprite {
            image: tex,
            custom_size: Some(Vec2::splat(68.0)),
            ..default()
        }
    } else {
        Sprite {
            color: Color::srgb(0.18, 0.52, 0.44),
            custom_size: Some(Vec2::new(26.0, 26.0)),
            ..default()
        }
    };
    commands
        .spawn((
            clara_sprite,
            Transform::from_xyz(185.0, 155.0, 3.2),
            SolidCollider {
                half_extents: Vec2::new(15.0, 15.0),
            },
            SuspectHeartbeat::new("clara", "Clara Moreau", 76.0),
            InteractableHotspot {
                label: "Clara Moreau".to_string(),
                node_id: "talk_clara".to_string(),
                radius: 78.0,
                requires_blood_sight_unless_clue: None,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite {
                    color: Color::srgba(0.02, 0.02, 0.04, 0.50),
                    custom_size: Some(Vec2::new(30.0, 10.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, -28.0, -0.2),
            ));
            parent.spawn((
                Text2d::new("Clara"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.88, 0.78)),
                Transform::from_xyz(0.0, 38.0, 1.0),
            ));
        });

    commands.spawn((
        Sprite {
            color: Color::srgba(0.92, 0.16, 0.28, 0.5),
            custom_size: Some(Vec2::new(56.0, 56.0)),
            ..default()
        },
        Transform::from_xyz(86.0, -28.0, 2.1),
        Visibility::Hidden,
        HeartbeatPulseRing {
            owner_npc_id: "miller".to_string(),
            base_size: 56.0,
        },
    ));
    let miller_sprite = if let Some(tex) = load_tex("sprites/miller.png") {
        Sprite {
            image: tex,
            custom_size: Some(Vec2::splat(70.0)),
            ..default()
        }
    } else {
        Sprite {
            color: Color::srgb(0.24, 0.34, 0.52),
            custom_size: Some(Vec2::new(28.0, 28.0)),
            ..default()
        }
    };
    commands
        .spawn((
            miller_sprite,
            Transform::from_xyz(86.0, -28.0, 3.2),
            SolidCollider {
                half_extents: Vec2::new(16.0, 16.0),
            },
            SuspectHeartbeat::new("miller", "Sgt. Frank Miller", 72.0),
            InteractableHotspot {
                label: "Sgt. Frank Miller".to_string(),
                node_id: "talk_miller".to_string(),
                radius: 80.0,
                requires_blood_sight_unless_clue: None,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite {
                    color: Color::srgba(0.02, 0.02, 0.04, 0.50),
                    custom_size: Some(Vec2::new(34.0, 11.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, -29.0, -0.2),
            ));
            parent.spawn((
                Text2d::new("Sgt. Miller"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.84, 0.94)),
                Transform::from_xyz(0.0, 40.0, 1.0),
            ));
        });

    // --- 6. PLAYER DETECTIVE (HARLAN VALE) ---
    let vale_visual = if let Some(tex) = load_tex("sprites/vale.png") {
        Sprite {
            image: tex,
            custom_size: Some(Vec2::splat(68.0)),
            ..default()
        }
    } else {
        Sprite {
            color: Color::srgb(0.22, 0.20, 0.25),
            custom_size: Some(Vec2::new(26.0, 26.0)),
            ..default()
        }
    };
    commands
        .spawn((
            Transform::from_xyz(-35.0, 20.0, 4.0),
            Visibility::Visible,
            DetectivePlayer {
                speed: 205.0,
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
                vale_visual,
                Transform::from_xyz(0.0, 0.0, 0.1),
                CharacterVisualSprite,
            ));
        });

    // --- 7. POINT-AND-CLICK DESTINATION MARKER & PROMPTS ---
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

    // --- 8. PROCEDURAL RAIN STREAKS ---
    for i in 0..55 {
        let rx = -300.0 + ((i * 37) % 600) as f32;
        let ry = -305.0 + ((i * 53) % 580) as f32;
        commands.spawn((
            Sprite {
                color: Color::srgba(0.68, 0.78, 0.92, 0.22),
                custom_size: Some(Vec2::new(1.5, 16.0)),
                ..default()
            },
            Transform::from_xyz(rx, ry, 5.0).with_rotation(Quat::from_rotation_z(-0.22)),
            RainDrop {
                velocity: Vec2::new(-65.0, -310.0),
                min_y: -310.0,
                reset_y: 285.0,
            },
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_point_and_click_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ledger_modal: Res<LedgerModalOpen>,
    mp_modal: Res<MultiplayerModalOpen>,
    blood_sight: Res<BloodSight>,
    ledger: Res<ClueLedger>,
    cam_q: Query<&Transform, With<SceneCamera>>,
    colliders: Query<(&Transform, &SolidCollider), Without<DetectivePlayer>>,
    hotspots: Query<(&Transform, &InteractableHotspot)>,
    mut player_q: Query<(&Transform, &mut DetectivePlayer)>,
    mut marker_q: Query<
        (&mut Transform, &mut Visibility),
        (
            With<ClickTargetMarker>,
            Without<SceneCamera>,
            Without<DetectivePlayer>,
            Without<SolidCollider>,
            Without<InteractableHotspot>,
        ),
    >,
    mut enter_writer: EventWriter<EnterNodeEvent>,
) {
    if ledger_modal.0 || mp_modal.open {
        return;
    }
    let clicked = mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right);
    if !clicked {
        return;
    }

    let (Ok(window), Ok(cam_tf), Ok((player_tf, mut player))) =
        (windows.get_single(), cam_q.get_single(), player_q.get_single_mut())
    else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let scene_width = window.width() * SCENE_WIDTH_FRACTION;
    // Ignore clicks on top/bottom HUD strips or right dialogue pane.
    if cursor.x > scene_width || cursor.y < 36.0 || cursor.y > window.height() - 34.0 {
        return;
    }

    let world_click = Vec2::new(
        (cam_tf.translation.x + (cursor.x - scene_width * 0.5)).clamp(-292.0, 292.0),
        (cam_tf.translation.y + (window.height() * 0.5 - cursor.y)).clamp(-292.0, 268.0),
    );
    let start_pos = player_tf.translation.truncate();

    // Check if the player clicked on an interactable clue or suspect!
    let mut clicked_hotspot: Option<(Vec2, &InteractableHotspot)> = None;
    for (htf, hs) in &hotspots {
        if let Some(clue_id) = &hs.requires_blood_sight_unless_clue {
            if !blood_sight.active && !ledger.has_clue(clue_id) {
                continue;
            }
        }
        let h_pos = htf.translation.truncate();
        if world_click.distance(h_pos) <= 44.0 {
            clicked_hotspot = Some((h_pos, hs));
            break;
        }
    }

    let obstacle_boxes: Vec<(Vec2, Vec2)> = colliders
        .iter()
        .map(|(t, c)| (t.translation.truncate(), c.half_extents))
        .collect();

    if let Some((h_pos, hs)) = clicked_hotspot {
        // Already close enough? Interact immediately!
        if start_pos.distance(h_pos) <= hs.radius {
            player.waypoints.clear();
            player.pending_interaction = None;
            if let Ok((_, mut vis)) = marker_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            enter_writer.send(EnterNodeEvent::fresh(hs.node_id.clone()));
            return;
        }

        // Otherwise pathfind to a walkable approach spot right beside the hotspot!
        let approach = find_nearest_walkable(h_pos, start_pos, &obstacle_boxes);
        player.waypoints = compute_astar_path(start_pos, approach, &obstacle_boxes);
        player.pending_interaction = Some((h_pos, hs.radius, hs.node_id.clone()));

        if let Ok((mut mtf, mut mvis)) = marker_q.get_single_mut() {
            mtf.translation.x = h_pos.x;
            mtf.translation.y = h_pos.y - 24.0;
            *mvis = Visibility::Visible;
        }
    } else {
        // Walk to clicked floor point!
        let goal = find_nearest_walkable(world_click, start_pos, &obstacle_boxes);
        player.waypoints = compute_astar_path(start_pos, goal, &obstacle_boxes);
        player.pending_interaction = None;

        if let Ok((mut mtf, mut mvis)) = marker_q.get_single_mut() {
            mtf.translation.x = goal.x;
            mtf.translation.y = goal.y - 22.0;
            *mvis = Visibility::Visible;
        }
    }
}

#[allow(clippy::type_complexity)]
fn move_detective_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mp_modal: Res<MultiplayerModalOpen>,
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
        (With<CharacterVisualSprite>, Without<ClickTargetMarker>),
    >,
    mut marker_q: Query<
        &mut Visibility,
        (
            With<ClickTargetMarker>,
            Without<DetectivePlayer>,
            Without<CharacterVisualSprite>,
        ),
    >,
    mut enter_writer: EventWriter<EnterNodeEvent>,
) {
    let Ok((mut player, mut transform)) = player_q.get_single_mut() else {
        return;
    };

    // Keyboard movement remains available when not typing an invite code
    let mut key_dir = Vec2::ZERO;
    if !mp_modal.open {
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

        let cand_x = Vec2::new(transform.translation.x + move_vec.x, transform.translation.y);
        if !overlaps_any(cand_x, player_half, &colliders) {
            transform.translation.x = cand_x.x;
        }

        let cand_y = Vec2::new(transform.translation.x, transform.translation.y + move_vec.y);
        if !overlaps_any(cand_y, player_half, &colliders) {
            transform.translation.y = cand_y.y;
        }
    } else {
        player.is_moving = false;
        if let Ok((mut vtf, _)) = visual_q.get_single_mut() {
            vtf.translation.y *= 0.8;
        }
    }

    // Check if we arrived at a clicked clue/suspect!
    if let Some((h_pos, radius, node_id)) = player.pending_interaction.clone() {
        let cur = transform.translation.truncate();
        if cur.distance(h_pos) <= radius + 6.0 || player.waypoints.is_empty() {
            player.waypoints.clear();
            player.pending_interaction = None;
            if let Ok(mut mvis) = marker_q.get_single_mut() {
                *mvis = Visibility::Hidden;
            }
            if cur.distance(h_pos) <= radius + 24.0 {
                enter_writer.send(EnterNodeEvent::fresh(node_id));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_remote_peer_entities(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<NetSession>,
    asset_server: Option<Res<AssetServer>>,
    mut actors_q: Query<(Entity, &mut RemotePeerActor, &mut Transform)>,
    mut visuals_q: Query<(&RemotePeerVisual, &mut Transform, &mut Sprite), Without<RemotePeerActor>>,
    mut auras_q: Query<(&RemotePeerSightAura, &mut Visibility), Without<RemotePeerActor>>,
) {
    let mut existing_ids = HashMap::new();
    for (entity, actor, _) in &actors_q {
        existing_ids.insert(actor.peer_id, entity);
    }

    // Despawn peers who left
    for (pid, entity) in &existing_ids {
        if !session.peers.contains_key(pid) {
            commands.entity(*entity).despawn_recursive();
        }
    }

    // Spawn newly joined detective partners!
    for (&pid, snap) in &session.peers {
        if !existing_ids.contains_key(&pid) {
            let tint = match pid % 3 {
                0 => Color::srgb(0.95, 0.78, 0.58), // Amber partner coat tint
                1 => Color::srgb(0.75, 0.90, 0.95), // Cool slate-blue coat tint
                _ => Color::srgb(0.96, 0.68, 0.72), // Crimson noir coat tint
            };
            let partner_sprite = if let Some(srv) = &asset_server {
                Sprite {
                    image: srv.load("sprites/vale.png"),
                    color: tint,
                    custom_size: Some(Vec2::splat(68.0)),
                    flip_x: snap.flip_x,
                    ..default()
                }
            } else {
                Sprite {
                    color: tint,
                    custom_size: Some(Vec2::new(26.0, 26.0)),
                    ..default()
                }
            };

            commands
                .spawn((
                    Transform::from_xyz(snap.x, snap.y, 3.9),
                    Visibility::Visible,
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
                            color: Color::srgba(0.95, 0.14, 0.26, 0.32),
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
                        RemotePeerVisual { peer_id: pid },
                    ));
                    parent.spawn((
                        Text2d::new(format!("Partner #{pid}")),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(tint),
                        Transform::from_xyz(0.0, 40.0, 0.5),
                    ));
                });
        }
    }

    // Smoothly interpolate active partners!
    let dt = time.delta_secs();
    let lerp_t = (dt * 12.0).min(1.0);
    for (_, mut actor, mut tf) in &mut actors_q {
        if let Some(snap) = session.peers.get(&actor.peer_id) {
            tf.translation.x += (snap.x - tf.translation.x) * lerp_t;
            tf.translation.y += (snap.y - tf.translation.y) * lerp_t;
            if snap.moving {
                actor.walk_phase += dt * 13.0;
            } else {
                actor.walk_phase = 0.0;
            }
        }
    }

    for (vis_tag, mut vtf, mut sprite) in &mut visuals_q {
        if let Some(snap) = session.peers.get(&vis_tag.peer_id) {
            sprite.flip_x = snap.flip_x;
            if snap.moving {
                vtf.translation.y = (time.elapsed_secs() * 13.0).sin().abs() * 3.0;
            } else {
                vtf.translation.y *= 0.8;
            }
        }
    }

    for (aura, mut vis) in &mut auras_q {
        if let Some(snap) = session.peers.get(&aura.peer_id) {
            *vis = if snap.blood_sight {
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

// --- COMPACT 2D GRID A* PATHFINDING FOR POINT-AND-CLICK ---
const GRID_STEP: f32 = 16.0;
const GRID_MIN_X: i32 = -18;
const GRID_MAX_X: i32 = 18;
const GRID_MIN_Y: i32 = -18;
const GRID_MAX_Y: i32 = 17;

fn cell_to_world(cx: i32, cy: i32) -> Vec2 {
    Vec2::new(cx as f32 * GRID_STEP, cy as f32 * GRID_STEP)
}

fn world_to_cell(p: Vec2) -> (i32, i32) {
    (
        (p.x / GRID_STEP).round() as i32,
        (p.y / GRID_STEP).round() as i32,
    )
}

fn point_blocked(pos: Vec2, pad: f32, obstacles: &[(Vec2, Vec2)]) -> bool {
    if pos.x < -296.0 || pos.x > 296.0 || pos.y < -296.0 || pos.y > 272.0 {
        return true;
    }
    for (center, half) in obstacles {
        if (pos.x - center.x).abs() < (half.x + pad) && (pos.y - center.y).abs() < (half.y + pad) {
            return true;
        }
    }
    false
}

fn find_nearest_walkable(desired: Vec2, from: Vec2, obstacles: &[(Vec2, Vec2)]) -> Vec2 {
    if !point_blocked(desired, 14.0, obstacles) {
        return desired;
    }
    // Spiral outward toward `from` first so clicking a table/NPC stands on the player's side!
    let dir_to_player = (from - desired).normalize_or_zero();
    for dist in [36.0, 48.0, 60.0, 72.0] {
        let preferred = desired + dir_to_player * dist;
        if !point_blocked(preferred, 14.0, obstacles) {
            return preferred;
        }
        for angle_idx in 0..12 {
            let ang = (angle_idx as f32) * std::f32::consts::TAU / 12.0;
            let cand = desired + Vec2::from_angle(ang) * dist;
            if !point_blocked(cand, 14.0, obstacles) {
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

pub fn compute_astar_path(start: Vec2, goal: Vec2, obstacles: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    // Direct line-of-sight shortcut when nothing blocks the straight walk!
    if has_clear_line(start, goal, obstacles) {
        return vec![goal];
    }

    let start_c = (
        world_to_cell(start).0.clamp(GRID_MIN_X, GRID_MAX_X),
        world_to_cell(start).1.clamp(GRID_MIN_Y, GRID_MAX_Y),
    );
    let goal_c = (
        world_to_cell(goal).0.clamp(GRID_MIN_X, GRID_MAX_X),
        world_to_cell(goal).1.clamp(GRID_MIN_Y, GRID_MAX_Y),
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
            if !(GRID_MIN_X..=GRID_MAX_X).contains(&nx) || !(GRID_MIN_Y..=GRID_MAX_Y).contains(&ny) {
                continue;
            }
            let wpos = cell_to_world(nx, ny);
            if (nx, ny) != goal_c && point_blocked(wpos, 13.5, obstacles) {
                continue;
            }
            // Prevent diagonal corner-cutting across wall edges
            if dx != 0 && dy != 0 {
                let side_a = cell_to_world(current.cell.0 + dx, current.cell.1);
                let side_b = cell_to_world(current.cell.0, current.cell.1 + dy);
                if point_blocked(side_a, 13.5, obstacles) || point_blocked(side_b, 13.5, obstacles) {
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

    // String-pulling smoothing pass so movement looks natural rather than stair-stepped!
    let mut smoothed = Vec::new();
    let mut anchor = start;
    let mut idx = 0;
    while idx < raw_pts.len() {
        let mut furthest = idx;
        for probe in (idx + 1)..raw_pts.len() {
            if has_clear_line(anchor, raw_pts[probe], obstacles) {
                furthest = probe;
            }
        }
        anchor = raw_pts[furthest];
        smoothed.push(anchor);
        idx = furthest + 1;
    }
    smoothed
}

fn has_clear_line(a: Vec2, b: Vec2, obstacles: &[(Vec2, Vec2)]) -> bool {
    let dist = a.distance(b);
    let steps = ((dist / 10.0).ceil() as usize).max(1);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        if point_blocked(a.lerp(b, t), 13.0, obstacles) {
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
    player_q: Query<&Transform, (With<DetectivePlayer>, Without<SceneCamera>)>,
    mut cam_q: Query<&mut Transform, With<SceneCamera>>,
) {
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let Ok(mut cam_tf) = cam_q.get_single_mut() else {
        return;
    };

    let target_x = p_tf.translation.x.clamp(-95.0, 95.0);
    let target_y = p_tf.translation.y.clamp(-110.0, 95.0);
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
        if tf.translation.x < -305.0 {
            tf.translation.x += 610.0;
        }
    }

    let t = time.elapsed_secs();
    for (neon, mut sprite) in &mut neon_q {
        let flicker = if (t * 7.3).sin() > 0.92 { 0.06 } else { neon.base_alpha };
        sprite.color = sprite.color.with_alpha(flicker);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_hotspot_prompts(
    keys: Res<ButtonInput<KeyCode>>,
    mp_modal: Res<MultiplayerModalOpen>,
    player_q: Query<&Transform, With<DetectivePlayer>>,
    blood_sight: Res<BloodSight>,
    ledger: Res<ClueLedger>,
    hotspots: Query<(&Transform, &InteractableHotspot)>,
    mut nearby: ResMut<NearbyHotspot>,
    mut highlight_q: Query<
        (&mut Transform, &mut Visibility),
        (
            With<HotspotHighlightRing>,
            Without<DetectivePlayer>,
            Without<WorldPromptLabel>,
            Without<InteractableHotspot>,
        ),
    >,
    mut label_q: Query<
        (&mut Transform, &mut Visibility, &mut Text2d),
        (
            With<WorldPromptLabel>,
            Without<DetectivePlayer>,
            Without<HotspotHighlightRing>,
            Without<InteractableHotspot>,
        ),
    >,
    mut enter_writer: EventWriter<EnterNodeEvent>,
) {
    let Ok(player_tf) = player_q.get_single() else {
        return;
    };
    let p_pos = player_tf.translation.truncate();

    let mut best: Option<(f32, Vec2, &InteractableHotspot)> = None;
    for (htf, hs) in &hotspots {
        if let Some(clue_id) = &hs.requires_blood_sight_unless_clue {
            if !blood_sight.active && !ledger.has_clue(clue_id) {
                continue;
            }
        }
        let h_pos = htf.translation.truncate();
        let dist = p_pos.distance(h_pos);
        if dist <= hs.radius && (best.is_none() || dist < best.unwrap().0) {
            best = Some((dist, h_pos, hs));
        }
    }

    if let Some((_, pos, hs)) = best {
        nearby.label = Some(hs.label.clone());
        nearby.node_id = Some(hs.node_id.clone());

        if let Ok((mut ring_tf, mut ring_vis)) = highlight_q.get_single_mut() {
            ring_tf.translation.x = pos.x;
            ring_tf.translation.y = pos.y;
            *ring_vis = Visibility::Visible;
        }
        if let Ok((mut lbl_tf, mut lbl_vis, mut txt)) = label_q.get_single_mut() {
            lbl_tf.translation.x = pos.x;
            lbl_tf.translation.y = pos.y + 44.0;
            txt.0 = format!("Click / [E] {}", hs.label);
            *lbl_vis = Visibility::Visible;
        }

        if !mp_modal.open && (keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Space)) {
            enter_writer.send(EnterNodeEvent::fresh(hs.node_id.clone()));
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
        // Start behind west poker table, click south-west alley dumpster!
        let start = Vec2::new(-185.0, 60.0);
        let goal = Vec2::new(-185.0, -180.0);
        let path = compute_astar_path(start, goal, &obstacles);
        assert!(
            path.len() >= 2,
            "Path must route around partition wall through center doorway"
        );
        assert_eq!(*path.last().unwrap(), goal);
    }
}
