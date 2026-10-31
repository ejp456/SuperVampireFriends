use bevy::prelude::*;

use crate::stats::{DetectiveId, DetectiveSheet};

/// Global toggle for the active detective's supernatural sense (`Tab` key):
/// - Det. Richey (Vampire): Vampire Senses (Blood Sight & Heartbeats)
/// - Det. Lucker & Ron (Ghost/Host): Phantom Sight (Death Echoes & Guilt Auras)
/// - Det. Pinckney (Werewolf): Lupine Scent (Spoor Plumes & Fear-Sweat)
/// - Det. Graves (Zombie): Mortis Sight (Necrotic Psychometry & Nerve Vitals)
#[derive(Resource, Debug, Default)]
pub struct BloodSight {
    pub active: bool,
    pub pulse_phase: f32,
}

/// Marks forensic traces (dragged blood smears, ectoplasmic residue, scent plumes, necrotic traces)
/// that are invisible to mundane eyes and appear while the detective's Supernatural Sense is active.
#[derive(Component, Debug)]
pub struct BloodEcho {
    pub base_alpha: f32,
    pub phase_offset: f32,
}

/// Marks mundane room lighting overlays that shift into supernatural vision when `[Tab]` is active.
#[derive(Component, Debug)]
pub struct MundaneAmbiance {
    pub normal_color: Color,
    pub blood_sight_color: Color,
}

/// Attached to living NPCs to simulate their real-time physiological / spiritual rhythm.
#[derive(Component, Debug, Clone)]
pub struct SuspectHeartbeat {
    pub npc_id: String,
    pub display_name: String,
    pub resting_bpm: f32,
    pub current_bpm: f32,
    pub target_bpm: f32,
    pub is_lie_spike: bool,
    pub beat_progress: f32,
    pub last_beat_flash: f32,
}

impl SuspectHeartbeat {
    pub fn new(npc_id: impl Into<String>, display_name: impl Into<String>, resting_bpm: f32) -> Self {
        Self {
            npc_id: npc_id.into(),
            display_name: display_name.into(),
            resting_bpm,
            current_bpm: resting_bpm,
            target_bpm: resting_bpm,
            is_lie_spike: false,
            beat_progress: 0.0,
            last_beat_flash: 0.0,
        }
    }
}

/// Visual concentric pulse/aura ring rendered around living NPCs during Supernatural Sense.
#[derive(Component, Debug)]
pub struct HeartbeatPulseRing {
    pub owner_npc_id: String,
    pub base_size: f32,
}

/// Event fired by narrative nodes when a line of questioning alters an NPC's pulse/scent/aura.
#[derive(Event, Debug, Clone)]
pub struct SetSuspectPulseEvent {
    pub npc_id: String,
    pub target_bpm: f32,
    pub is_lie_spike: bool,
}

pub struct SensesPlugin;

impl Plugin for SensesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BloodSight>()
            .add_event::<SetSuspectPulseEvent>()
            .add_systems(
                Update,
                (
                    toggle_blood_sight_input,
                    tick_blood_sight_and_hunger,
                    apply_suspect_pulse_events,
                    tick_suspect_heartbeats,
                    update_blood_echo_visuals,
                    update_mundane_ambiance_visuals,
                    animate_heartbeat_rings,
                ),
            );
    }
}

fn toggle_blood_sight_input(
    keys: Res<ButtonInput<KeyCode>>,
    mp_modal: Option<Res<crate::ui::MultiplayerModalOpen>>,
    mut blood_sight: ResMut<BloodSight>,
) {
    if mp_modal.map(|m| m.open).unwrap_or(false) {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        blood_sight.active = !blood_sight.active;
    }
}

fn tick_blood_sight_and_hunger(
    time: Res<Time>,
    mut blood_sight: ResMut<BloodSight>,
    mut sheet: ResMut<DetectiveSheet>,
) {
    let dt = time.delta_secs();
    blood_sight.pulse_phase += dt * 3.2;

    if blood_sight.active {
        // Channeling supernatural perception builds strain (+1 pip per ~10s).
        sheet.add_hunger(dt * 0.10);
    } else if sheet.hunger > 1.0 {
        // Resting supernatural sight lets strain cool toward baseline 1.0.
        let cooled = (sheet.hunger - dt * 0.025).max(1.0);
        sheet.hunger = cooled;
    }
}

fn apply_suspect_pulse_events(
    mut events: EventReader<SetSuspectPulseEvent>,
    mut query: Query<&mut SuspectHeartbeat>,
) {
    for ev in events.read() {
        for mut hb in &mut query {
            if hb.npc_id == ev.npc_id {
                hb.target_bpm = ev.target_bpm;
                hb.is_lie_spike = ev.is_lie_spike;
            }
        }
    }
}

fn tick_suspect_heartbeats(time: Res<Time>, mut query: Query<&mut SuspectHeartbeat>) {
    let dt = time.delta_secs();
    for mut hb in &mut query {
        let diff = hb.target_bpm - hb.current_bpm;
        hb.current_bpm += diff * (dt * 3.5).min(1.0);

        let beats_per_sec = (hb.current_bpm / 60.0).max(0.4);
        hb.beat_progress += dt * beats_per_sec;
        if hb.beat_progress >= 1.0 {
            hb.beat_progress -= 1.0;
            hb.last_beat_flash = 1.0;
        } else {
            hb.last_beat_flash = (hb.last_beat_flash - dt * 4.2).max(0.0);
        }
    }
}

pub fn echo_rgb_for_character(character: DetectiveId) -> (f32, f32, f32) {
    match character {
        DetectiveId::Richey => (0.95, 0.08, 0.20),       // Crimson Blood Sight
        DetectiveId::LuckerAndRon => (0.28, 0.88, 1.00), // Spectral Ectoplasm Cyan
        DetectiveId::Pinckney => (1.00, 0.68, 0.14),     // Lupine Scent Amber
        DetectiveId::Zombie => (0.74, 0.38, 0.98),       // Necrotic Psychometry Violet
    }
}

fn update_blood_echo_visuals(
    blood_sight: Res<BloodSight>,
    sheet: Res<DetectiveSheet>,
    mut query: Query<(&BloodEcho, &mut Visibility, &mut Sprite)>,
) {
    let (r, g, b) = echo_rgb_for_character(sheet.character);
    for (echo, mut vis, mut sprite) in &mut query {
        if blood_sight.active {
            *vis = Visibility::Visible;
            let wave = ((blood_sight.pulse_phase + echo.phase_offset).sin() * 0.5 + 0.5) * 0.35;
            let alpha = (echo.base_alpha + wave).clamp(0.15, 0.95);
            sprite.color = Color::srgba(r, g, b, alpha);
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn update_mundane_ambiance_visuals(
    blood_sight: Res<BloodSight>,
    sheet: Res<DetectiveSheet>,
    mut clear_color: ResMut<ClearColor>,
    mut query: Query<(&MundaneAmbiance, &mut Sprite)>,
) {
    clear_color.0 = if blood_sight.active {
        match sheet.character {
            DetectiveId::Richey => Color::srgb(0.04, 0.015, 0.03),
            DetectiveId::LuckerAndRon => Color::srgb(0.015, 0.035, 0.05),
            DetectiveId::Pinckney => Color::srgb(0.04, 0.03, 0.015),
            DetectiveId::Zombie => Color::srgb(0.03, 0.02, 0.048),
        }
    } else {
        Color::srgb(0.05, 0.05, 0.07)
    };

    for (amb, mut sprite) in &mut query {
        if !blood_sight.active {
            sprite.color = amb.normal_color;
            continue;
        }
        let s = amb.blood_sight_color.to_srgba();
        sprite.color = match sheet.character {
            DetectiveId::Richey => amb.blood_sight_color,
            DetectiveId::LuckerAndRon => {
                Color::srgba(s.blue * 0.65, s.red.max(s.blue) * 0.95, (s.red + 0.08).min(1.0), s.alpha)
            }
            DetectiveId::Pinckney => {
                Color::srgba((s.red + 0.06).min(1.0), (s.red * 0.68).min(1.0), s.blue * 0.55, s.alpha)
            }
            DetectiveId::Zombie => {
                Color::srgba((s.red * 0.85).min(1.0), s.green * 0.9, (s.red + 0.08).min(1.0), s.alpha)
            }
        };
    }
}

fn animate_heartbeat_rings(
    blood_sight: Res<BloodSight>,
    sheet: Res<DetectiveSheet>,
    heartbeats: Query<&SuspectHeartbeat>,
    mut rings: Query<(&HeartbeatPulseRing, &mut Visibility, &mut Transform, &mut Sprite)>,
) {
    let (cr, cg, cb) = echo_rgb_for_character(sheet.character);
    for (ring, mut vis, mut transform, mut sprite) in &mut rings {
        if !blood_sight.active {
            *vis = Visibility::Hidden;
            continue;
        }

        if let Some(hb) = heartbeats.iter().find(|h| h.npc_id == ring.owner_npc_id) {
            *vis = Visibility::Visible;
            sprite.custom_size = Some(Vec2::splat(ring.base_size));
            let scale = 0.65 + hb.beat_progress * 1.45;
            transform.scale = Vec3::splat(scale);
            let alpha = (1.0 - hb.beat_progress).powi(2) * 0.78;
            sprite.color = if hb.is_lie_spike || hb.current_bpm >= 120.0 {
                // Bright heightened spike in the detective's signature supernatural spectrum
                Color::srgba((cr + 0.15).min(1.0), (cg + 0.08).min(1.0), (cb + 0.08).min(1.0), alpha)
            } else {
                Color::srgba(cr * 0.85, cg * 0.85, cb * 0.85, alpha * 0.72)
            };
        }
    }
}
