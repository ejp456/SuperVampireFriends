use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Faculty {
    Deduction,
    Sanguine,
    Nocturne,
    Pulse,
}

impl Faculty {
    pub const ALL: [Faculty; 4] = [
        Faculty::Deduction,
        Faculty::Sanguine,
        Faculty::Nocturne,
        Faculty::Pulse,
    ];

    pub fn name_for(&self, character: DetectiveId) -> &'static str {
        match self {
            Faculty::Deduction => "DEDUCTION",
            Faculty::Nocturne => "NOCTURNE",
            Faculty::Sanguine => match character {
                DetectiveId::Richey => "SANGUINE",
                DetectiveId::LuckerAndRon => "ECTOPLASM",
                DetectiveId::Pinckney => "LUPINE SCENT",
                DetectiveId::Zombie => "MORTIS",
            },
            Faculty::Pulse => match character {
                DetectiveId::Richey => "PULSE",
                DetectiveId::LuckerAndRon => "SPIRIT AURA",
                DetectiveId::Pinckney => "FERAL",
                DetectiveId::Zombie => "DEAD NERVE",
            },
        }
    }

    pub fn code_for(&self, character: DetectiveId) -> &'static str {
        match self {
            Faculty::Deduction => "DED",
            Faculty::Nocturne => "NOC",
            Faculty::Sanguine => match character {
                DetectiveId::Richey => "SAN",
                DetectiveId::LuckerAndRon => "ECT",
                DetectiveId::Pinckney => "SCT",
                DetectiveId::Zombie => "MOR",
            },
            Faculty::Pulse => match character {
                DetectiveId::Richey => "PUL",
                DetectiveId::LuckerAndRon => "AUR",
                DetectiveId::Pinckney => "FER",
                DetectiveId::Zombie => "NRV",
            },
        }
    }

    pub fn desc_for(&self, character: DetectiveId) -> &'static str {
        match self {
            Faculty::Deduction => "Ballistics, timelines, cold forensic geometry",
            Faculty::Nocturne => "1940s streetwise cynicism, underworld & veil lore",
            Faculty::Sanguine => match character {
                DetectiveId::Richey => "Blood chemistry, arterial iron, crimson echoes",
                DetectiveId::LuckerAndRon => "Ghost mediumship, death echoes, séance channeling",
                DetectiveId::Pinckney => "Wolf bloodhound tracking, cordite, perfume & spoor",
                DetectiveId::Zombie => "Undead psychometry, lividity, autopsy tissue recall",
            },
            Faculty::Pulse => match character {
                DetectiveId::Richey => "Predator acoustics, heartbeat lies, vampire dread",
                DetectiveId::LuckerAndRon => "Guilt aura resonance, possession voice, spirit pressure",
                DetectiveId::Pinckney => "Fear-sweat adrenaline, alpha pack presence, lupine snarl",
                DetectiveId::Zombie => "Pulse-free stillness, bullet-proof grit, morgue dread",
            },
        }
    }

    pub fn color_for(&self, character: DetectiveId) -> Color {
        match self {
            Faculty::Deduction => Color::srgb(0.47, 0.72, 0.92),
            Faculty::Nocturne => Color::srgb(0.86, 0.68, 0.34),
            Faculty::Sanguine => character.accent_color(),
            Faculty::Pulse => match character {
                DetectiveId::Pinckney => Color::srgb(0.96, 0.54, 0.24),
                _ => Color::srgb(0.76, 0.44, 0.88),
            },
        }
    }
}

/// The 4 selectable supernatural detectives in Crimson Ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DetectiveId {
    #[default]
    Richey,
    LuckerAndRon,
    Pinckney,
    Zombie,
}

impl DetectiveId {
    #[allow(dead_code)]
    pub const ALL: [DetectiveId; 4] = [
        DetectiveId::Richey,
        DetectiveId::LuckerAndRon,
        DetectiveId::Pinckney,
        DetectiveId::Zombie,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "Detective Richey",
            DetectiveId::LuckerAndRon => "Det. Lucker & Ron",
            DetectiveId::Pinckney => "Detective Pinckney",
            DetectiveId::Zombie => "Detective Guinan",
        }
    }

    pub fn species_badge(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "VAMPIRE",
            DetectiveId::LuckerAndRon => "GHOST (LUCKER) + HUMAN (RON)",
            DetectiveId::Pinckney => "WEREWOLF",
            DetectiveId::Zombie => "ZOMBIE REVENANT",
        }
    }

    pub fn dialogue_you_header(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "YOU — DET. RICHEY (VAMPIRE)",
            DetectiveId::LuckerAndRon => "YOU — RON & GHOST DET. LUCKER",
            DetectiveId::Pinckney => "YOU — DET. PINCKNEY (WEREWOLF)",
            DetectiveId::Zombie => "YOU — DET. GUINAN (ZOMBIE)",
        }
    }

    /// How suspects address the player in dialogue (`{detective}`).
    pub fn npc_address(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "Richey",
            DetectiveId::LuckerAndRon => "Ron",
            DetectiveId::Pinckney => "Pinckney",
            DetectiveId::Zombie => "Guinan",
        }
    }

    pub fn sense_name(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "Vampire Senses",
            DetectiveId::LuckerAndRon => "Phantom Sight",
            DetectiveId::Pinckney => "Lupine Scent",
            DetectiveId::Zombie => "Mortis Sight",
        }
    }

    pub fn strain_label(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "THIRST",
            DetectiveId::LuckerAndRon => "SYNC",
            DetectiveId::Pinckney => "FURY",
            DetectiveId::Zombie => "DECAY",
        }
    }

    pub fn hotkey_label(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "F1",
            DetectiveId::LuckerAndRon => "F2",
            DetectiveId::Pinckney => "F3",
            DetectiveId::Zombie => "F4",
        }
    }

    pub fn accent_color(&self) -> Color {
        match self {
            DetectiveId::Richey => Color::srgb(0.94, 0.20, 0.30),
            DetectiveId::LuckerAndRon => Color::srgb(0.36, 0.86, 0.98),
            DetectiveId::Pinckney => Color::srgb(0.98, 0.70, 0.22),
            DetectiveId::Zombie => Color::srgb(0.74, 0.48, 0.96),
        }
    }

    pub fn walk_speed(&self) -> f32 {
        match self {
            DetectiveId::Richey => 205.0,
            DetectiveId::LuckerAndRon => 208.0,
            DetectiveId::Pinckney => 234.0,
            DetectiveId::Zombie => 188.0,
        }
    }

    pub fn sprite_path(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "sprites/richey.png",
            DetectiveId::LuckerAndRon => "sprites/lucker_ron.png",
            DetectiveId::Pinckney => "sprites/pinckney.png",
            DetectiveId::Zombie => "sprites/zombie.png",
        }
    }

    pub fn portrait_path(&self) -> &'static str {
        match self {
            DetectiveId::Richey => "portraits/richey_portrait.png",
            DetectiveId::LuckerAndRon => "portraits/lucker_ron_portrait.png",
            DetectiveId::Pinckney => "portraits/pinckney_portrait.png",
            DetectiveId::Zombie => "portraits/zombie_portrait.png",
        }
    }

    pub fn bio(&self) -> &'static str {
        match self {
            DetectiveId::Richey => {
                "Night-shift homicide vampire. Reads scrubbed crimson trails and hears every skipped heartbeat."
            }
            DetectiveId::LuckerAndRon => {
                "Ghost ace Det. Lucker rides shotgun inside nervous human host Ron—whispering clues from beyond."
            }
            DetectiveId::Pinckney => {
                "Harbor werewolf investigator. Tracks cordite, perfume & fear-sweat through pouring rain."
            }
            DetectiveId::Zombie => {
                "Stitched morgue revenant who walked off the slab. Reconstructs wounds via undead psychometry."
            }
        }
    }

    pub fn abilities_summary(&self) -> &'static str {
        match self {
            DetectiveId::Richey => {
                "• [Tab] Vampire Senses: Blood trails & live BPM lie rings\n• Crimson Gaze & Blood Chemistry (High Thirst: +SAN/+PUL, -DED)"
            }
            DetectiveId::LuckerAndRon => {
                "• [Tab] Phantom Sight: Death echoes & spirit guilt auras\n• Corpse Séance & Ghost Possession (High Sync: +DED/+NOC, -AUR)"
            }
            DetectiveId::Pinckney => {
                "• [Tab] Lupine Scent: Scent plumes & fear-adrenaline spikes\n• Fast prowler stride & Werewolf Snarl (High Fury: +2 FER/+1 SCT, -DED/-NOC)"
            }
            DetectiveId::Zombie => {
                "• [Tab] Mortis Sight: Tissue psychometry & autopsy lividity\n• Bullet-proof nerve & Corpse Recall (High Decay: +DED/+MOR, -NOC)"
            }
        }
    }

    pub fn base_stats(&self) -> (i32, i32, i32, i32) {
        match self {
            // (Deduction, Supernatural Sense [SAN/ECT/SCT/MOR], Nocturne, Presence [PUL/AUR/FER/NRV])
            DetectiveId::Richey => (3, 4, 2, 3),
            DetectiveId::LuckerAndRon => (4, 2, 4, 2),
            DetectiveId::Pinckney => (2, 3, 2, 5),
            DetectiveId::Zombie => (4, 4, 3, 1),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct DetectiveSheet {
    pub character: DetectiveId,
    pub deduction: i32,
    pub sanguine: i32,
    pub nocturne: i32,
    pub pulse: i32,
    /// Supernatural strain (Thirst / Sync / Fury / Decay), from 0.0 to 5.0.
    pub hunger: f32,
    /// Case-wide dice roll bonus earned by cracking cypher puzzles in the active story.
    pub cypher_bonus: i32,
}

impl Default for DetectiveSheet {
    fn default() -> Self {
        Self::from_character(DetectiveId::Richey)
    }
}

impl DetectiveSheet {
    pub fn from_character(character: DetectiveId) -> Self {
        let (deduction, sanguine, nocturne, pulse) = character.base_stats();
        Self {
            character,
            deduction,
            sanguine,
            nocturne,
            pulse,
            hunger: 1.0,
            cypher_bonus: 0,
        }
    }

    pub fn base_stat(&self, faculty: Faculty) -> i32 {
        match faculty {
            Faculty::Deduction => self.deduction,
            Faculty::Sanguine => self.sanguine,
            Faculty::Nocturne => self.nocturne,
            Faculty::Pulse => self.pulse,
        }
    }

    pub fn effective_stat(&self, faculty: Faculty) -> i32 {
        let base = self.base_stat(faculty);
        if self.hunger < 3.0 {
            return base;
        }
        match self.character {
            DetectiveId::Richey => match faculty {
                Faculty::Sanguine | Faculty::Pulse => base + 1,
                Faculty::Deduction => (base - 1).max(0),
                Faculty::Nocturne => base,
            },
            DetectiveId::LuckerAndRon => match faculty {
                Faculty::Deduction | Faculty::Nocturne => base + 1,
                Faculty::Pulse => (base - 1).max(0),
                Faculty::Sanguine => base,
            },
            DetectiveId::Pinckney => match faculty {
                Faculty::Pulse => base + 2,
                Faculty::Sanguine => base + 1,
                Faculty::Deduction | Faculty::Nocturne => (base - 1).max(0),
            },
            DetectiveId::Zombie => match faculty {
                Faculty::Deduction | Faculty::Sanguine => base + 1,
                Faculty::Nocturne => (base - 1).max(0),
                Faculty::Pulse => base,
            },
        }
    }

    pub fn hunger_pips(&self) -> u8 {
        self.hunger.floor().clamp(0.0, 5.0) as u8
    }

    pub fn add_hunger(&mut self, delta: f32) {
        self.hunger = (self.hunger + delta).clamp(0.0, 5.0);
    }

    /// Total modifier applied to 2d6 skill checks (`effective_stat` + solved `cypher_bonus`).
    pub fn roll_modifier(&self, faculty: Faculty) -> i32 {
        self.effective_stat(faculty) + self.cypher_bonus
    }

    /// Exact Disco Elysium-style 2d6 probability where (1,1) always fails and (6,6) always succeeds.
    pub fn success_chance_percent(&self, faculty: Faculty, dc: i32) -> u8 {
        let modifier = self.roll_modifier(faculty);
        let mut wins = 0u32;
        for d1 in 1..=6u8 {
            for d2 in 1..=6u8 {
                if evaluate_roll(d1, d2, modifier, dc).succeeded {
                    wins += 1;
                }
            }
        }
        ((wins * 100 + 18) / 36) as u8
    }

    pub fn roll_check(&self, faculty: Faculty, dc: i32) -> RollResult {
        let d1 = fastrand::u8(1..=6);
        let d2 = fastrand::u8(1..=6);
        let modifier = self.roll_modifier(faculty);
        evaluate_roll(d1, d2, modifier, dc)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollResult {
    pub die1: u8,
    pub die2: u8,
    pub modifier: i32,
    pub total: i32,
    pub dc: i32,
    pub succeeded: bool,
    pub critical_success: bool,
    pub critical_failure: bool,
}

pub fn evaluate_roll(die1: u8, die2: u8, modifier: i32, dc: i32) -> RollResult {
    let critical_failure = die1 == 1 && die2 == 1;
    let critical_success = die1 == 6 && die2 == 6;
    let total = (die1 as i32) + (die2 as i32) + modifier;
    let succeeded = if critical_failure {
        false
    } else if critical_success {
        true
    } else {
        total >= dc
    };

    RollResult {
        die1,
        die2,
        modifier,
        total,
        dc,
        succeeded,
        critical_success,
        critical_failure,
    }
}

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DetectiveSheet>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_eyes_always_fails_and_boxcars_always_succeeds() {
        let snake = evaluate_roll(1, 1, 10, 5);
        assert!(!snake.succeeded);
        assert!(snake.critical_failure);

        let boxcars = evaluate_roll(6, 6, 0, 20);
        assert!(boxcars.succeeded);
        assert!(boxcars.critical_success);
    }

    #[test]
    fn non_vampires_do_not_expose_vampire_sanguine_stat_labels() {
        assert_eq!(Faculty::Sanguine.name_for(DetectiveId::Richey), "SANGUINE");
        assert_eq!(Faculty::Sanguine.code_for(DetectiveId::Richey), "SAN");

        assert_eq!(Faculty::Sanguine.name_for(DetectiveId::Pinckney), "LUPINE SCENT");
        assert_eq!(Faculty::Sanguine.code_for(DetectiveId::Pinckney), "SCT");
        assert_eq!(Faculty::Pulse.name_for(DetectiveId::Pinckney), "FERAL");

        assert_eq!(Faculty::Sanguine.name_for(DetectiveId::LuckerAndRon), "ECTOPLASM");
        assert_eq!(Faculty::Sanguine.name_for(DetectiveId::Zombie), "MORTIS");
    }

    #[test]
    fn each_detective_has_unique_supernatural_overdrive_modifiers() {
        let mut richey = DetectiveSheet::from_character(DetectiveId::Richey);
        richey.hunger = 3.5;
        assert_eq!(richey.effective_stat(Faculty::Sanguine), 5);
        assert_eq!(richey.effective_stat(Faculty::Pulse), 4);
        assert_eq!(richey.effective_stat(Faculty::Deduction), 2);

        let mut lucker = DetectiveSheet::from_character(DetectiveId::LuckerAndRon);
        lucker.hunger = 3.5;
        assert_eq!(lucker.effective_stat(Faculty::Deduction), 5);
        assert_eq!(lucker.effective_stat(Faculty::Nocturne), 5);
        assert_eq!(lucker.effective_stat(Faculty::Pulse), 1);

        let mut pinckney = DetectiveSheet::from_character(DetectiveId::Pinckney);
        pinckney.hunger = 3.5;
        assert_eq!(pinckney.effective_stat(Faculty::Pulse), 7);
        assert_eq!(pinckney.effective_stat(Faculty::Sanguine), 4);
        assert_eq!(pinckney.effective_stat(Faculty::Deduction), 1);

        let mut zombie = DetectiveSheet::from_character(DetectiveId::Zombie);
        zombie.hunger = 3.5;
        assert_eq!(zombie.effective_stat(Faculty::Deduction), 5);
        assert_eq!(zombie.effective_stat(Faculty::Sanguine), 5);
        assert_eq!(zombie.effective_stat(Faculty::Nocturne), 2);
    }

    #[test]
    fn probability_stays_bounded_between_3_and_97_percent() {
        let sheet = DetectiveSheet::default();
        assert_eq!(sheet.success_chance_percent(Faculty::Deduction, 2), 97);
        assert_eq!(sheet.success_chance_percent(Faculty::Deduction, 30), 3);
    }

    #[test]
    fn cypher_bonus_raises_roll_modifier_and_success_chance() {
        let mut sheet = DetectiveSheet::from_character(DetectiveId::Richey);
        // Richey base Pulse = 3; vs DC 10 needs 7+ on 2d6 -> 21/36 = 58%
        let base_chance = sheet.success_chance_percent(Faculty::Pulse, 10);
        assert_eq!(base_chance, 58);

        sheet.cypher_bonus = 2;
        assert_eq!(sheet.roll_modifier(Faculty::Pulse), 5);
        // With +2 cypher bonus, modifier = 5; vs DC 10 needs 5+ on 2d6 -> 30/36 = 83%
        let boosted_chance = sheet.success_chance_percent(Faculty::Pulse, 10);
        assert_eq!(boosted_chance, 83);
        assert!(boosted_chance > base_chance);
    }
}
