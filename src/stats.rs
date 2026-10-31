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

    pub fn name(&self) -> &'static str {
        match self {
            Faculty::Deduction => "DEDUCTION",
            Faculty::Sanguine => "SANGUINE",
            Faculty::Nocturne => "NOCTURNE",
            Faculty::Pulse => "PULSE",
        }
    }

    pub fn short_desc(&self) -> &'static str {
        match self {
            Faculty::Deduction => "Ballistics, timelines, cold forensic geometry",
            Faculty::Sanguine => "Blood chemistry, iron scent, echoes of violence",
            Faculty::Nocturne => "1940s streetwise cynicism, underworld etiquette",
            Faculty::Pulse => "Predator acoustics, heartbeat lies, intimidation",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Faculty::Deduction => Color::srgb(0.47, 0.72, 0.92),
            Faculty::Sanguine => Color::srgb(0.88, 0.24, 0.30),
            Faculty::Nocturne => Color::srgb(0.86, 0.68, 0.34),
            Faculty::Pulse => Color::srgb(0.76, 0.44, 0.88),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    ForensicHound,
    BloodMystic,
    MidnightEnforcer,
}

impl Archetype {
    pub fn title(&self) -> &'static str {
        match self {
            Archetype::ForensicHound => "The Forensic Hound",
            Archetype::BloodMystic => "The Blood Seer",
            Archetype::MidnightEnforcer => "The Midnight Enforcer",
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct DetectiveSheet {
    pub archetype_title: String,
    pub deduction: i32,
    pub sanguine: i32,
    pub nocturne: i32,
    pub pulse: i32,
    /// Ranges from 0.0 (steady) to 5.0 (ravenous).
    /// High hunger (>= 3.0) sharpens Sanguine & Pulse (+1) at the cost of Deduction (-1).
    pub hunger: f32,
}

impl Default for DetectiveSheet {
    fn default() -> Self {
        Self::from_archetype(Archetype::ForensicHound)
    }
}

impl DetectiveSheet {
    pub fn from_archetype(archetype: Archetype) -> Self {
        let (deduction, sanguine, nocturne, pulse) = match archetype {
            Archetype::ForensicHound => (4, 2, 2, 3),
            Archetype::BloodMystic => (2, 4, 3, 2),
            Archetype::MidnightEnforcer => (2, 2, 3, 4),
        };
        Self {
            archetype_title: archetype.title().to_string(),
            deduction,
            sanguine,
            nocturne,
            pulse,
            hunger: 1.0,
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
        if self.hunger >= 3.0 {
            match faculty {
                Faculty::Sanguine | Faculty::Pulse => base + 1,
                Faculty::Deduction => (base - 1).max(0),
                Faculty::Nocturne => base,
            }
        } else {
            base
        }
    }

    pub fn hunger_pips(&self) -> u8 {
        self.hunger.floor().clamp(0.0, 5.0) as u8
    }

    pub fn add_hunger(&mut self, delta: f32) {
        self.hunger = (self.hunger + delta).clamp(0.0, 5.0);
    }

    /// Exact Disco Elysium-style 2d6 probability where (1,1) always fails and (6,6) always succeeds.
    pub fn success_chance_percent(&self, faculty: Faculty, dc: i32) -> u8 {
        let modifier = self.effective_stat(faculty);
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
        let modifier = self.effective_stat(faculty);
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
    fn hunger_modifies_predatory_and_rational_faculties() {
        let mut sheet = DetectiveSheet::from_archetype(Archetype::ForensicHound);
        assert_eq!(sheet.effective_stat(Faculty::Deduction), 4);
        assert_eq!(sheet.effective_stat(Faculty::Pulse), 3);

        sheet.hunger = 3.5;
        assert_eq!(sheet.effective_stat(Faculty::Deduction), 3);
        assert_eq!(sheet.effective_stat(Faculty::Pulse), 4);
        assert_eq!(sheet.effective_stat(Faculty::Sanguine), 3);
    }

    #[test]
    fn probability_stays_bounded_between_3_and_97_percent() {
        let sheet = DetectiveSheet::default();
        assert_eq!(sheet.success_chance_percent(Faculty::Deduction, 2), 97);
        assert_eq!(sheet.success_chance_percent(Faculty::Deduction, 30), 3);
    }
}
