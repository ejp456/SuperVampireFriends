use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::senses::{BloodSight, SetSuspectPulseEvent};
use crate::stats::{DetectiveSheet, Faculty};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClueDef {
    pub id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassiveVoice {
    pub faculty: Option<Faculty>,
    pub min_stat: i32,
    #[serde(default)]
    pub requires_blood_sight: bool,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseTrigger {
    pub npc_id: String,
    pub target_bpm: f32,
    pub is_lie_spike: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCheckSpec {
    pub id: String,
    pub faculty: Faculty,
    pub dc: i32,
    pub success_node: String,
    pub failure_node: String,
    #[serde(default)]
    pub hunger_cost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub label: String,
    #[serde(default)]
    pub next_node: String,
    #[serde(default)]
    pub requires_clue: Option<String>,
    #[serde(default)]
    pub hide_if_clue: Option<String>,
    #[serde(default)]
    pub requires_blood_sight: bool,
    #[serde(default)]
    pub skill_check: Option<SkillCheckSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueNode {
    pub id: String,
    pub speaker: String,
    #[serde(default)]
    pub active_suspect_id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub grants_clues: Vec<String>,
    #[serde(default)]
    pub pulse_trigger: Option<PulseTrigger>,
    #[serde(default)]
    pub hunger_delta: f32,
    #[serde(default)]
    pub passive_voices: Vec<PassiveVoice>,
    #[serde(default)]
    pub choices: Vec<DialogueChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseData {
    pub title: String,
    pub subtitle: String,
    pub opening_node: String,
    pub clues: Vec<ClueDef>,
    pub nodes: Vec<DialogueNode>,
}

#[derive(Resource, Debug, Clone)]
pub struct CaseLibrary {
    pub title: String,
    pub opening_node: String,
    pub clues: HashMap<String, ClueDef>,
    pub clue_order: Vec<String>,
    pub nodes: HashMap<String, DialogueNode>,
}

impl CaseLibrary {
    pub fn from_case_data(data: CaseData) -> Self {
        let mut clues = HashMap::new();
        let mut clue_order = Vec::new();
        for c in data.clues {
            clue_order.push(c.id.clone());
            clues.insert(c.id.clone(), c);
        }
        let mut nodes = HashMap::new();
        for n in data.nodes {
            nodes.insert(n.id.clone(), n);
        }
        Self {
            title: data.title,
            opening_node: data.opening_node,
            clues,
            clue_order,
            nodes,
        }
    }
}

#[derive(Resource, Debug, Default, Clone)]
pub struct ClueLedger {
    pub discovered: Vec<String>,
    pub discovered_set: HashSet<String>,
    pub passed_checks: HashSet<String>,
    pub failed_checks: HashSet<String>,
}

impl ClueLedger {
    pub fn add_clue(&mut self, clue_id: &str) -> bool {
        if self.discovered_set.insert(clue_id.to_string()) {
            self.discovered.push(clue_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn has_clue(&self, clue_id: &str) -> bool {
        self.discovered_set.contains(clue_id)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedChoice {
    pub raw: DialogueChoice,
    pub locked_check_failed: bool,
}

/// Focused one-exchange-at-a-time dialogue state:
/// Only `my_line` (what you just said) + `other_speaker`/`other_line` (their direct reply).
#[derive(Resource, Debug, Default)]
pub struct NarrativeState {
    pub current_node_id: String,
    pub active_suspect_id: Option<String>,
    /// What Harlan Vale (you) just said in this exchange, if anything.
    pub my_line: Option<String>,
    /// Name of the character (or object) currently responding.
    pub other_speaker: String,
    /// Their single current response.
    pub other_line: String,
    /// Optional compact 1-line instinct whisper (at most one per turn).
    pub whisper_line: Option<(String, Color)>,
    /// Optional compact 1-line roll/clue badge.
    pub outcome_badge: Option<(String, bool)>,
    pub current_choices: Vec<ResolvedChoice>,
    pub generation: u64,
}

#[derive(Event, Debug, Clone)]
pub struct EnterNodeEvent {
    pub node_id: String,
    pub preserve_player_line: bool,
}

impl EnterNodeEvent {
    pub fn fresh(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            preserve_player_line: false,
        }
    }
}

#[derive(Event, Debug, Clone, Copy)]
pub struct SelectChoiceEvent(pub usize);

const EMBEDDED_CASE_01: &str = include_str!("../assets/cases/case_01.ron");

pub struct NarrativePlugin;

impl Plugin for NarrativePlugin {
    fn build(&self, app: &mut App) {
        let case_data = load_case_01_data();
        let library = CaseLibrary::from_case_data(case_data);
        let opening = library.opening_node.clone();

        app.insert_resource(library)
            .init_resource::<ClueLedger>()
            .init_resource::<NarrativeState>()
            .add_event::<EnterNodeEvent>()
            .add_event::<SelectChoiceEvent>()
            .add_systems(Startup, move |mut writer: EventWriter<EnterNodeEvent>| {
                writer.send(EnterNodeEvent::fresh(opening.clone()));
            })
            .add_systems(
                Update,
                (
                    handle_choice_hotkeys,
                    process_select_choice_events,
                    process_enter_node_events,
                    refresh_choices_on_sense_change,
                )
                    .chain(),
            );
    }
}

pub fn load_case_01_data() -> CaseData {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let disk_paths = [
            "assets/cases/case_01.ron",
            "C:/Users/etaip/.gemini/antigravity/scratch/vampire_noir_rpg/assets/cases/case_01.ron",
        ];
        for p in disk_paths {
            if let Ok(raw) = std::fs::read_to_string(p) {
                if let Ok(parsed) = ron::from_str::<CaseData>(&raw) {
                    return parsed;
                }
            }
        }
    }
    ron::from_str::<CaseData>(EMBEDDED_CASE_01).expect("Embedded case_01.ron must parse cleanly")
}

fn handle_choice_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mp_modal: Option<Res<crate::ui::MultiplayerModalOpen>>,
    state: Res<NarrativeState>,
    mut writer: EventWriter<SelectChoiceEvent>,
) {
    if mp_modal.map(|m| m.open).unwrap_or(false) {
        return;
    }
    let key_map = [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
    ];

    for (code, idx) in key_map {
        if keys.just_pressed(code) && idx < state.current_choices.len() {
            writer.send(SelectChoiceEvent(idx));
            break;
        }
    }
}

fn process_select_choice_events(
    mut events: EventReader<SelectChoiceEvent>,
    mut state: ResMut<NarrativeState>,
    mut sheet: ResMut<DetectiveSheet>,
    mut ledger: ResMut<ClueLedger>,
    mut node_writer: EventWriter<EnterNodeEvent>,
) {
    for SelectChoiceEvent(idx) in events.read().copied() {
        let Some(resolved) = state.current_choices.get(idx).cloned() else {
            continue;
        };
        if resolved.locked_check_failed {
            continue;
        }

        // Record ONLY the player's latest spoken line for the 1-on-1 exchange view.
        state.my_line = Some(resolved.raw.label.clone());
        state.outcome_badge = None;

        if let Some(check) = &resolved.raw.skill_check {
            if check.hunger_cost > 0.0 {
                sheet.add_hunger(check.hunger_cost);
            }
            let roll = sheet.roll_check(check.faculty, check.dc);
            let badge = format!(
                "{} {} vs DC {} ({})",
                check.faculty.name(),
                roll.total,
                roll.dc,
                if roll.succeeded { "Passed" } else { "Failed" }
            );
            state.outcome_badge = Some((badge, roll.succeeded));

            let target = if roll.succeeded {
                ledger.passed_checks.insert(check.id.clone());
                check.success_node.clone()
            } else {
                ledger.failed_checks.insert(check.id.clone());
                check.failure_node.clone()
            };
            node_writer.send(EnterNodeEvent {
                node_id: target,
                preserve_player_line: true,
            });
        } else if !resolved.raw.next_node.is_empty() {
            node_writer.send(EnterNodeEvent {
                node_id: resolved.raw.next_node.clone(),
                preserve_player_line: true,
            });
        }

        state.generation += 1;
    }
}

fn process_enter_node_events(
    mut events: EventReader<EnterNodeEvent>,
    library: Res<CaseLibrary>,
    blood_sight: Res<BloodSight>,
    mut sheet: ResMut<DetectiveSheet>,
    mut ledger: ResMut<ClueLedger>,
    mut state: ResMut<NarrativeState>,
    mut pulse_writer: EventWriter<SetSuspectPulseEvent>,
) {
    for ev in events.read() {
        if ev.node_id == "LEAVE_CONVERSATION" {
            state.current_node_id.clear();
            state.active_suspect_id = None;
            state.my_line = None;
            state.other_speaker.clear();
            state.other_line.clear();
            state.whisper_line = None;
            state.outcome_badge = None;
            state.current_choices.clear();
            state.generation += 1;
            continue;
        }

        let Some(node) = library.nodes.get(&ev.node_id) else {
            continue;
        };

        if !ev.preserve_player_line {
            state.my_line = None;
            state.outcome_badge = None;
        }

        state.current_node_id = node.id.clone();
        state.active_suspect_id = node.active_suspect_id.clone();
        state.other_speaker = node.speaker.clone();
        state.other_line = node.text.clone();
        state.whisper_line = None;

        if node.hunger_delta != 0.0 {
            sheet.add_hunger(node.hunger_delta);
        }

        if let Some(pt) = &node.pulse_trigger {
            pulse_writer.send(SetSuspectPulseEvent {
                npc_id: pt.npc_id.clone(),
                target_bpm: pt.target_bpm,
                is_lie_spike: pt.is_lie_spike,
            });
        }

        // Grant clues and fold notification neatly into the compact single-line badge.
        for clue_id in &node.grants_clues {
            if ledger.add_clue(clue_id) {
                if let Some(def) = library.clues.get(clue_id) {
                    let clue_tag = format!("+Clue: {}", def.title);
                    state.outcome_badge = Some(match state.outcome_badge.take() {
                        Some((prev, ok)) => (format!("{prev}  •  {clue_tag}"), ok),
                        None => (clue_tag, true),
                    });
                }
            }
        }

        // Pick AT MOST ONE short instinct whisper so the screen never crowds.
        for pv in &node.passive_voices {
            if pv.requires_blood_sight && !blood_sight.active {
                continue;
            }
            match pv.faculty {
                Some(fac) if sheet.effective_stat(fac) >= pv.min_stat => {
                    state.whisper_line = Some((format!("[{}] {}", fac.name(), pv.text), fac.color()));
                    break;
                }
                None if (sheet.hunger_pips() as i32) >= pv.min_stat => {
                    state.whisper_line = Some((
                        format!("[THIRST] {}", pv.text),
                        Color::srgb(0.95, 0.26, 0.34),
                    ));
                    break;
                }
                _ => {}
            }
        }

        state.current_choices = resolve_node_choices(node, &ledger, blood_sight.active);
        state.generation += 1;
    }
}

fn refresh_choices_on_sense_change(
    blood_sight: Res<BloodSight>,
    library: Res<CaseLibrary>,
    ledger: Res<ClueLedger>,
    mut state: ResMut<NarrativeState>,
) {
    if !blood_sight.is_changed() || state.current_node_id.is_empty() {
        return;
    }
    if let Some(node) = library.nodes.get(&state.current_node_id) {
        state.current_choices = resolve_node_choices(node, &ledger, blood_sight.active);
        state.generation += 1;
    }
}

pub fn resolve_node_choices(
    node: &DialogueNode,
    ledger: &ClueLedger,
    blood_sight_active: bool,
) -> Vec<ResolvedChoice> {
    let mut out = Vec::new();
    for ch in &node.choices {
        if ch.requires_blood_sight && !blood_sight_active {
            continue;
        }
        if let Some(req) = &ch.requires_clue {
            if !ledger.has_clue(req) {
                continue;
            }
        }
        if let Some(hide) = &ch.hide_if_clue {
            if ledger.has_clue(hide) {
                continue;
            }
        }

        let locked_check_failed = ch
            .skill_check
            .as_ref()
            .map(|sc| ledger.failed_checks.contains(&sc.id))
            .unwrap_or(false);

        if let Some(sc) = &ch.skill_check {
            if ledger.passed_checks.contains(&sc.id) {
                continue;
            }
        }

        out.push(ResolvedChoice {
            raw: ch.clone(),
            locked_check_failed,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_case_01_parses_and_all_links_are_valid() {
        let data = ron::from_str::<CaseData>(EMBEDDED_CASE_01)
            .expect("case_01.ron failed to deserialize");
        let lib = CaseLibrary::from_case_data(data);

        assert!(lib.nodes.contains_key(&lib.opening_node));

        for (id, node) in &lib.nodes {
            for clue_id in &node.grants_clues {
                assert!(
                    lib.clues.contains_key(clue_id),
                    "Node '{id}' grants unknown clue '{clue_id}'"
                );
            }
            for choice in &node.choices {
                if let Some(sc) = &choice.skill_check {
                    assert!(
                        lib.nodes.contains_key(&sc.success_node),
                        "Node '{id}' check '{}' missing success_node '{}'",
                        sc.id,
                        sc.success_node
                    );
                    assert!(
                        lib.nodes.contains_key(&sc.failure_node),
                        "Node '{id}' check '{}' missing failure_node '{}'",
                        sc.id,
                        sc.failure_node
                    );
                } else {
                    assert!(
                        choice.next_node == "LEAVE_CONVERSATION"
                            || lib.nodes.contains_key(&choice.next_node),
                        "Node '{id}' choice '{}' points to missing node '{}'",
                        choice.label,
                        choice.next_node
                    );
                }
            }
        }
    }
}
