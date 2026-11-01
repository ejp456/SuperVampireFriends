use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    narrative::{
        format_for_character, resolve_node_choices, CaseLibrary, ClueLedger, DialogueChoice,
        NarrativeState, ResolvedChoice,
    },
    senses::BloodSight,
    stats::DetectiveSheet,
};

fn default_roll_bonus() -> i32 {
    2
}

/// Supported classic noir cypher families.
/// Authors write human-readable `solution` plaintext in RON, and the engine
/// encrypts it automatically (unless `ciphertext_override` is provided).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CypherKind {
    /// Shifts each letter forward in the alphabet by `shift` (1..=25).
    Caesar { shift: u8 },
    /// 1940s telegraph/ledger number code (`A=1, B=2 ... Z=26`, `-` between letters, `/` between words).
    A1Z26,
    /// Alchemical mirror alphabet (`A↔Z, B↔Y, C↔X ...`).
    Atbash,
}

impl CypherKind {
    pub fn badge_label(&self) -> &'static str {
        match self {
            CypherKind::Caesar { .. } => "CAESAR SHIFT CYPHER",
            CypherKind::A1Z26 => "A1Z26 NUMBER CYPHER",
            CypherKind::Atbash => "ATBASH MIRROR CYPHER",
        }
    }

    /// Encrypts `plaintext` according to this cypher's rule.
    pub fn encode(&self, plaintext: &str) -> String {
        match self {
            CypherKind::Caesar { shift } => {
                let s = (*shift % 26) as u8;
                plaintext
                    .chars()
                    .map(|ch| {
                        if ch.is_ascii_alphabetic() {
                            let idx = ch.to_ascii_uppercase() as u8 - b'A';
                            (b'A' + (idx + s) % 26) as char
                        } else {
                            ch.to_ascii_uppercase()
                        }
                    })
                    .collect()
            }
            CypherKind::A1Z26 => {
                let words: Vec<String> = plaintext
                    .split_whitespace()
                    .filter_map(|word| {
                        let tokens: Vec<String> = word
                            .chars()
                            .filter_map(|ch| {
                                if ch.is_ascii_alphabetic() {
                                    let n = (ch.to_ascii_uppercase() as u8 - b'A') + 1;
                                    Some(n.to_string())
                                } else if ch.is_ascii_digit() {
                                    Some(format!("#{ch}"))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if tokens.is_empty() {
                            None
                        } else {
                            Some(tokens.join("-"))
                        }
                    })
                    .collect();
                words.join("  /  ")
            }
            CypherKind::Atbash => plaintext
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphabetic() {
                        let idx = ch.to_ascii_uppercase() as u8 - b'A';
                        (b'Z' - idx) as char
                    } else {
                        ch.to_ascii_uppercase()
                    }
                })
                .collect(),
        }
    }
}

/// Data-driven cypher puzzle definition stored in `CaseData.cyphers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypherDef {
    pub id: String,
    pub title: String,
    pub kind: CypherKind,
    /// Decoded plaintext solution (e.g. `"LEFT HOLSTER"`).
    pub solution: String,
    /// Optional manual ciphertext override; when `None`, `kind.encode(&solution)` is used.
    #[serde(default)]
    pub ciphertext_override: Option<String>,
    /// Scene context describing where/how the encrypted note was found.
    pub flavor: String,
    /// Key hint found in the margin or surroundings.
    pub hint: String,
    /// Narrative payoff shown once the cypher is solved.
    pub solved_text: String,
    /// `ClueDef.id` granted upon cracking the cypher.
    pub grants_clue: String,
    /// Case-wide modifier bonus added to all 2d6 skill check rolls once solved.
    #[serde(default = "default_roll_bonus")]
    pub roll_bonus: i32,
}

impl CypherDef {
    /// Convenience constructor for creating simple cypher puzzles in code or tests.
    #[allow(dead_code)]
    pub fn simple(
        id: impl Into<String>,
        title: impl Into<String>,
        kind: CypherKind,
        solution: impl Into<String>,
        hint: impl Into<String>,
        grants_clue: impl Into<String>,
        roll_bonus: i32,
    ) -> Self {
        let title_s = title.into();
        let sol_s = solution.into();
        Self {
            id: id.into(),
            title: title_s.clone(),
            kind,
            solution: sol_s.clone(),
            ciphertext_override: None,
            flavor: format!("An encrypted slip titled '{title_s}' awaits decoding."),
            hint: hint.into(),
            solved_text: format!("Decoded '{sol_s}' — your forensic instincts sharpen!"),
            grants_clue: grants_clue.into(),
            roll_bonus,
        }
    }

    pub fn ciphertext(&self) -> String {
        self.ciphertext_override
            .clone()
            .unwrap_or_else(|| self.kind.encode(&self.solution))
    }

    pub fn is_correct_guess(&self, raw_guess: &str) -> bool {
        let target = normalize_cypher_answer(&self.solution);
        !target.is_empty() && normalize_cypher_answer(raw_guess) == target
    }

    /// Shows blank word lengths (e.g. `"_ _ _ _    _ _ _ _ _ _ _  (4, 7 letters)"`) to guide input.
    pub fn word_pattern_hint(&self) -> String {
        let words: Vec<usize> = self
            .solution
            .split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_ascii_alphanumeric()).count())
            .filter(|&len| len > 0)
            .collect();
        if words.is_empty() {
            return String::new();
        }
        let blanks: Vec<String> = words
            .iter()
            .map(|&n| vec!["_"; n].join(" "))
            .collect();
        let counts: Vec<String> = words.iter().map(|n| n.to_string()).collect();
        format!("{}   ({} letters)", blanks.join("    "), counts.join(" + "))
    }

    pub fn detective_tip(&self) -> String {
        let clean: Vec<char> = self
            .solution
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        match (clean.first(), clean.last()) {
            (Some(&first), Some(&last)) if clean.len() >= 3 => {
                format!("Forensic Tip: Plaintext starts with '{first}' and ends with '{last}'.")
            }
            (Some(&first), _) => format!("Forensic Tip: Plaintext starts with '{first}'."),
            _ => String::new(),
        }
    }
}

pub fn normalize_cypher_answer(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Formats Caesar wheel key table for a chosen `dial_shift` (0..26),
/// where Cipher `(A + dial_shift)` decodes back to Plain `A`.
pub fn format_caesar_wheel_lines(dial_shift: u8) -> (String, String) {
    let s = dial_shift % 26;
    let mut cipher_row = String::from("CIPHER: ");
    let mut plain_row = String::from("PLAIN : ");
    for i in 0..26u8 {
        if i > 0 {
            cipher_row.push(' ');
            plain_row.push(' ');
        }
        let c_ch = (b'A' + i) as char;
        let p_ch = (b'A' + (26 + i - s) % 26) as char;
        cipher_row.push(c_ch);
        plain_row.push(p_ch);
    }
    (cipher_row, plain_row)
}

#[derive(Resource, Debug, Default, Clone)]
pub struct ActiveCypherModal {
    pub open: bool,
    pub cypher_id: String,
    pub typed_input: String,
    pub dial_shift: u8,
    pub show_extra_tip: bool,
    pub feedback: Option<(String, bool)>,
    pub revision: u64,
}

impl ActiveCypherModal {
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.revision += 1;
        }
    }
}

#[derive(Event, Debug, Clone)]
pub struct OpenCypherEvent(pub String);

#[derive(Event, Debug, Clone, Copy)]
pub struct SubmitCypherGuessEvent;

pub struct CypherPlugin;

impl Plugin for CypherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveCypherModal>()
            .add_event::<OpenCypherEvent>()
            .add_event::<SubmitCypherGuessEvent>()
            .add_systems(
                Update,
                (process_open_cypher_events, process_submit_cypher_guess)
                    .chain()
                    .after(crate::stories::StorySystems),
            );
    }
}

fn process_open_cypher_events(
    mut events: EventReader<OpenCypherEvent>,
    library: Res<CaseLibrary>,
    ledger: Res<ClueLedger>,
    mut modal: ResMut<ActiveCypherModal>,
    mut ledger_modal: Option<ResMut<crate::ui::LedgerModalOpen>>,
    mut mp_modal: Option<ResMut<crate::ui::MultiplayerModalOpen>>,
    mut char_modal: Option<ResMut<crate::ui::CharacterSelectModalOpen>>,
    mut story_modal: Option<ResMut<crate::ui::StorySelectModalOpen>>,
) {
    let Some(OpenCypherEvent(cid)) = events.read().last().cloned() else {
        return;
    };
    let Some(def) = library.cyphers.get(&cid) else {
        return;
    };

    if let Some(ref mut lm) = ledger_modal {
        lm.0 = false;
    }
    if let Some(ref mut mm) = mp_modal {
        mm.open = false;
    }
    if let Some(ref mut cm) = char_modal {
        cm.0 = false;
    }
    if let Some(ref mut sm) = story_modal {
        sm.0 = false;
    }

    modal.open = true;
    modal.cypher_id = cid.clone();
    modal.show_extra_tip = false;

    if ledger.is_cypher_solved(&cid) {
        modal.typed_input = def.solution.to_ascii_uppercase();
        modal.dial_shift = match def.kind {
            CypherKind::Caesar { shift } => shift % 26,
            _ => 0,
        };
        let clue_title = library
            .clues
            .get(&def.grants_clue)
            .map(|c| c.title.as_str())
            .unwrap_or(&def.grants_clue);
        modal.feedback = Some((
            format!(
                "✓ ALREADY CRACKED — +Clue: {}  •  +{} Dice Roll Bonus Active!",
                clue_title, def.roll_bonus
            ),
            true,
        ));
    } else {
        modal.typed_input.clear();
        modal.dial_shift = 0;
        modal.feedback = None;
    }
    modal.revision += 1;
}

fn process_submit_cypher_guess(
    mut events: EventReader<SubmitCypherGuessEvent>,
    library: Res<CaseLibrary>,
    blood_sight: Res<BloodSight>,
    mut modal: ResMut<ActiveCypherModal>,
    mut ledger: ResMut<ClueLedger>,
    mut sheet: ResMut<DetectiveSheet>,
    mut nar_state: ResMut<NarrativeState>,
) {
    let mut submitted = false;
    for _ in events.read() {
        submitted = true;
    }
    if !submitted || !modal.open {
        return;
    }

    let Some(def) = library.cyphers.get(&modal.cypher_id) else {
        return;
    };

    if ledger.is_cypher_solved(&def.id) {
        modal.close();
        return;
    }

    if modal.typed_input.trim().is_empty() {
        modal.feedback = Some((
            "Enter the decoded message first, then press [Enter] or click [Crack Cypher].".to_string(),
            false,
        ));
        modal.revision += 1;
        return;
    }

    if def.is_correct_guess(&modal.typed_input) {
        modal.typed_input = def.solution.to_ascii_uppercase();
        apply_cypher_solved(
            &def.id,
            &library,
            blood_sight.active,
            &mut ledger,
            &mut sheet,
            &mut nar_state,
        );
        let clue_title = library
            .clues
            .get(&def.grants_clue)
            .map(|c| c.title.as_str())
            .unwrap_or(&def.grants_clue);
        modal.feedback = Some((
            format!(
                "✓ CYPHER CRACKED! +Clue: {}  •  +{} Bonus to All 2d6 Skill Checks!",
                clue_title, def.roll_bonus
            ),
            true,
        ));
        modal.revision += 1;
    } else {
        modal.feedback = Some((
            format!(
                "✗ '{}' doesn't fit the cypher — check the key alignment and try again.",
                modal.typed_input.trim()
            ),
            false,
        ));
        modal.revision += 1;
    }
}

/// Applies rewards when a cypher is solved locally or synced from a Co-op partner.
/// Returns `true` if this cypher was newly solved.
pub fn apply_cypher_solved(
    cypher_id: &str,
    library: &CaseLibrary,
    blood_sight_active: bool,
    ledger: &mut ClueLedger,
    sheet: &mut DetectiveSheet,
    nar_state: &mut NarrativeState,
) -> bool {
    let Some(def) = library.cyphers.get(cypher_id) else {
        return false;
    };
    if !ledger.mark_cypher_solved(cypher_id) {
        return false;
    }

    ledger.add_clue(&def.grants_clue);
    sheet.cypher_bonus += def.roll_bonus.max(0);
    // Fresh forensic insight unlocks any previously failed skill checks for a retry!
    ledger.failed_checks.clear();

    let clue_title = library
        .clues
        .get(&def.grants_clue)
        .map(|c| c.title.as_str())
        .unwrap_or(&def.grants_clue);
    let badge = format!(
        "✓ Cypher Cracked! +Clue: {}  •  +{} All Dice Rolls",
        clue_title, def.roll_bonus
    );

    if !nar_state.current_node_id.is_empty() {
        if let Some(node) = library.nodes.get(&nar_state.current_node_id) {
            nar_state.current_choices =
                resolve_node_choices(node, ledger, blood_sight_active, sheet.character);
            nar_state.outcome_badge = Some((badge, true));
        }
    } else {
        nar_state.current_node_id = format!("CYPHER_SOLVED_{}", def.id);
        nar_state.active_suspect_id = None;
        nar_state.my_line = Some(format!("[Decoded Cypher] \"{}\"", def.solution.to_ascii_uppercase()));
        nar_state.other_speaker = format!("{} — CRACKED", def.title.to_uppercase());
        nar_state.other_line = format_for_character(&def.solved_text, sheet.character);
        nar_state.whisper_line = None;
        nar_state.outcome_badge = Some((badge, true));
        let leave_choice = DialogueChoice {
            label: "Pocket the decoded note and continue investigating.".to_string(),
            next_node: "LEAVE_CONVERSATION".to_string(),
            requires_clue: None,
            hide_if_clue: None,
            requires_character: None,
            requires_blood_sight: false,
            open_cypher: None,
            skill_check: None,
        };
        nar_state.current_choices = vec![ResolvedChoice {
            formatted_label: leave_choice.label.clone(),
            raw: leave_choice,
            locked_check_failed: false,
        }];
    }

    nar_state.generation += 1;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caesar_a1z26_and_atbash_encode_and_verify_cleanly() {
        let caesar = CypherDef::simple(
            "c1",
            "Vance Slip",
            CypherKind::Caesar { shift: 3 },
            "LEFT HOLSTER",
            "Shift +3",
            "clue_1",
            2,
        );
        assert_eq!(caesar.ciphertext(), "OHIW KROVWHU");
        assert!(caesar.is_correct_guess("left holster"));
        assert!(caesar.is_correct_guess("LEFT-HOLSTER"));
        assert!(!caesar.is_correct_guess("RIGHT HOLSTER"));

        let num_code = CypherDef::simple(
            "c2",
            "Telegraph Slip",
            CypherKind::A1Z26,
            "GATE NINE",
            "A=1..Z=26",
            "clue_2",
            2,
        );
        assert_eq!(num_code.ciphertext(), "7-1-20-5  /  14-9-14-5");
        assert!(num_code.is_correct_guess("Gate Nine"));

        let mirror = CypherDef::simple(
            "c3",
            "Apothecary Blotter",
            CypherKind::Atbash,
            "SILVER VIAL",
            "A<->Z",
            "clue_3",
            2,
        );
        assert_eq!(mirror.ciphertext(), "HROEVI ERZO");
        assert!(mirror.is_correct_guess("silver vial"));
    }

    #[test]
    fn caesar_wheel_table_aligns_cipher_to_plain_for_shift() {
        let (cipher_row, plain_row) = format_caesar_wheel_lines(3);
        assert!(cipher_row.starts_with("CIPHER: A B C D"));
        // With shift=3, Cipher 'D' (4th letter) maps to Plain 'A'!
        assert!(plain_row.starts_with("PLAIN : X Y Z A"));
    }
}
