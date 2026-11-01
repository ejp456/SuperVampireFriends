# RON Schemas, Placeholders & Walkability Rules

## 1. `SceneCatalog` Schema (`assets/scenes/<story_id>_scenes.ron`)

Parsed into `crate::world::SceneCatalog` (`src/world.rs`).

```ron
SceneCatalog(
    story_name: "The Clocktower Requiem",   // REQUIRED: non-empty human-readable story name!
    initial_scene: "clocktower_belfry",     // Must match a SceneClass.id in `scenes`
    initial_spawn: (-45.0, -40.0),          // Must NOT overlap any solid wall/prop/NPC/clue!
    scenes: [
        SceneClass(
            id: "clocktower_belfry",
            title: "St. Jude — Belfry Landing",
            environment: Room,              // Enum: Room | Alley | Street
            walk_bounds: (292.0, 218.0),    // Player clamped to [-x + 13, +x - 13], [-y + 13, +y - 13]
            camera_pan_bounds: (95.0, 75.0),
            rain_drops: 0,                  // 0 for interiors; ~45–68 for rainy Alley/Street scenes
            surfaces: [
                SceneSurface(
                    pos: (0.0, 0.0),
                    size: (610.0, 464.0),
                    z: 0.0,
                    normal_rgba: (0.12, 0.10, 0.11, 1.0),
                    blood_sight_rgba: (0.05, 0.05, 0.09, 1.0),
                    neon_flicker: None,     // Or Some(0.12) for flickering neon reflections/lamps
                    sprite_path: None,      // Optional: e.g. Some("props/crimson_rug.png")
                ),
            ],
            walls: [
                // Split perimeter walls around doorways so doorway triggers & spawns aren't blocked!
                SceneWall(pos: (0.0, 232.0), size: (630.0, 20.0), color_rgb: (0.20, 0.18, 0.22), solid: true),
                SceneWall(pos: (-305.0, 0.0), size: (20.0, 484.0), color_rgb: (0.20, 0.18, 0.22), solid: true),
                SceneWall(pos: (305.0, 0.0), size: (20.0, 484.0), color_rgb: (0.20, 0.18, 0.22), solid: true),
                SceneWall(pos: (-180.0, -232.0), size: (268.0, 20.0), color_rgb: (0.20, 0.18, 0.22), solid: true),
                SceneWall(pos: (180.0, -232.0), size: (268.0, 20.0), color_rgb: (0.20, 0.18, 0.22), solid: true),
            ],
            props: [
                SceneProp(
                    label: None,
                    sprite_path: Some("props/detective_desk.png"),
                    pos: (-150.0, 90.0),
                    size: (112.0, 74.0),
                    collider_half: Some((46.0, 26.0)), // Tighter footprint than visual sprite!
                    rotation_deg: 0.0,                 // Optional: #[serde(default)]
                    z: 2.5,
                    color_rgba: (0.45, 0.32, 0.22, 1.0),
                    solid: true,
                ),
            ],
            characters: [
                SceneCharacter(
                    id: "vale",
                    display_name: "Dr. Evelyn Vale",
                    nameplate: Some("Dr. Vale"),       // None for corpses, Some("...") for living NPCs
                    sprite_path: "sprites/vale.png",
                    fallback_rgb: (0.64, 0.84, 0.82),
                    pos: (130.0, 75.0),
                    sprite_size: (68.0, 68.0),
                    collider_half: Some((14.0, 14.0)), // None for corpses on rugs/slabs if walkable nearby
                    resting_bpm: Some(74.0),           // None for dead bodies; Some(65.0..90.0) for living
                    dialogue_node: "talk_vale_hub",    // MUST exist in CaseData.nodes!
                    interact_radius: 74.0,
                    nameplate_rgb: (0.76, 0.94, 0.90),
                ),
            ],
            clues: [
                SceneClue(
                    id: "hidden_altar_clue",
                    label: "Ritual Candelabra",
                    node_id: "inspect_candelabra",     // MUST exist in CaseData.nodes!
                    sprite_path: Some("props/occult_candelabra.png"),
                    pos: (210.0, -140.0),
                    size: (56.0, 66.0),
                    z: 2.5,
                    color_rgba: (0.95, 0.82, 0.48, 0.92),
                    collider_half: Some((18.0, 20.0)),
                    interact_radius: 74.0,
                    // None = always visible; Some("clue_id") = hidden until [Tab] Supernatural Sense is active OR clue_id is discovered!
                    requires_blood_sight_unless_clue: Some("clue_wax_seal"),
                    blood_echo_alpha: Some(0.65),
                ),
            ],
            blood_echoes: [
                // Invisible in normal sight; glows in each detective's signature color when [Tab] is active!
                SceneBloodEcho(
                    pos: (120.0, -80.0),
                    size: (44.0, 22.0),
                    alpha: 0.55,
                    phase_offset: 0.6,
                    sprite_path: Some("props/echo_smear.png"),
                    rotation_deg: -25.0,
                ),
            ],
            doorways: [
                SceneDoorwaySpec(
                    id: "door_belfry_to_street",
                    label: "Down to Canal Street",
                    pos: (0.0, -212.0),
                    size: (74.0, 18.0),
                    trigger_radius: 50.0,
                    target_scene_id: "canal_street",   // MUST exist in `scenes`!
                    target_spawn: (0.0, 165.0),        // MUST be walkable in target scene!
                    glow_rgb: (0.92, 0.76, 0.42),
                ),
            ],
        ),
    ],
)
```

### Critical Walkability & Collider Rules
`cargo test` runs `point_blocked_in_bounds(pos, 13.0, walk_bounds, &static_obstacle_boxes())` on `initial_spawn` and every `doorway.target_spawn`:
- **Player half-extent is `13.0` px** (`26×26` foot box).
- Any spawn `(sx, sy)` MUST satisfy:
  - `sx.abs() + 13.0 < walk_bounds.0` and `sy.abs() + 13.0 < walk_bounds.1`
  - For every solid wall, solid prop, character with `collider_half`, and clue with `collider_half`:
    `(sx - cx).abs() >= (13.0 + hx)` **OR** `(sy - cy).abs() >= (13.0 + hy)`.
- Place `target_spawn` ~45–55 px *inside* the room from the doorway trigger (`pos`) so the detective arrives cleanly inside the room without immediately re-overlapping a perimeter wall.

---

## 2. `CaseData` Schema (`assets/cases/<story_id>.ron`)

Parsed into `crate::narrative::CaseData` (`src/narrative.rs`).

> **IMPORTANT RON STRUCT NAME**: Inside `DialogueChoice`, the skill-check struct is named **`SkillCheckSpec(...)`**, NOT `SkillCheck(...)`!

```ron
CaseData(
    title: "THE CLOCKTOWER REQUIEM",
    subtitle: "4:10 AM",
    opening_node: "intro_arrival",          // Fired automatically when story loads!
    clues: [
        ClueDef(
            id: "clue_wax_seal",
            title: "Bellfounder's Wax Seal",
            summary: "Crimson sealing wax stamped with a broken hourglass.",
        ),
    ],
    nodes: [
        DialogueNode(
            id: "intro_arrival",
            speaker: "DR. EVELYN VALE",     // Uppercase speaker header; see portrait rules below
            active_suspect_id: Some("vale"),// Links right-pane Heartbeat/Aura Monitor to SceneCharacter.id!
            text: "\"You're out late, {detective}. Awaken {sense} if you must, just watch your {strain}.\"",
            grants_clues: [],               // Every ID here MUST exist in `clues`!
            pulse_trigger: Some(PulseTrigger(
                npc_id: "vale",
                target_bpm: 74.0,           // >= 120.0 or is_lie_spike: true triggers red/accent spike!
                is_lie_spike: false,
            )),
            hunger_delta: 0.0,              // +0.3 adds supernatural strain; -0.6 cools strain
            passive_voices: [
                PassiveVoice(
                    faculty: Some(Deduction),       // Option<Faculty>: None | Some(Deduction | Sanguine | Nocturne | Pulse)
                    min_stat: 2,
                    requires_character: None,       // Optional: Some(Richey | LuckerAndRon | Pinckney | Zombie)
                    requires_blood_sight: false,
                    text: "Something about the rope fray looks cut, not snapped.",
                ),
            ],
            choices: [
                DialogueChoice(
                    label: "[Pulse] Press her on the belltower key.",
                    next_node: "",
                    requires_clue: Some("clue_wax_seal"), // Optional
                    hide_if_clue: None,                   // Optional
                    requires_character: None,             // Optional: character-exclusive ability!
                    requires_blood_sight: false,          // Optional: only shown while [Tab] is ON!
                    skill_check: Some(SkillCheckSpec(
                        id: "check_press_vale",
                        faculty: Pulse,                   // Deduction | Sanguine | Nocturne | Pulse
                        dc: 10,                           // 2d6 + effective_stat >= dc (DC 9 easy, 10–11 medium, 12 hard)
                        hunger_cost: 0.3,
                        success_node: "vale_cracks",      // MUST exist in `nodes`!
                        failure_node: "vale_deflects",    // MUST exist in `nodes`!
                    )),
                ),
                DialogueChoice(
                    label: "Step away and search the room.",
                    next_node: "LEAVE_CONVERSATION",      // Special sentinel closing active dialogue card
                ),
            ],
        ),
    ],
)
```

---

## 3. Dynamic Detective Placeholders, Enums & Portraits

### Text Placeholders (Auto-formatted for Active Detective)
| Placeholder | `Richey` (Vampire) | `LuckerAndRon` (Ghost/Host) | `Pinckney` (Werewolf) | `Zombie` (Det. Guinan) |
| :--- | :--- | :--- | :--- | :--- |
| `{detective}` | `Richey` | `Ron` | `Pinckney` | `Guinan` |
| `{sense}` | `Vampire Senses` | `Phantom Sight` | `Lupine Scent` | `Mortis Sight` |
| `{strain}` | `Hunger` | `Ecto-Strain` | `Feral Rage` | `Rigor Decay` |

### `Faculty` Enum Variants (`src/stats.rs`)
Always use these exact 4 variant identifiers in RON (`Deduction`, `Sanguine`, `Nocturne`, `Pulse`). The UI automatically renames them per species:
- `Deduction`: `Deduction` / `Casework` / `Instinct` / `Forensics`
- `Sanguine`: `Sanguine` / `Mediumship` / `Bloodhound` / `Necropsy`
- `Nocturne`: `Nocturne` / `Poltergeist` / `Pack Alpha` / `Dread`
- `Pulse`: `Pulse` / `Empathy` / `Pack Bond` / `Deadpan`

### Dialogue Speaker Portrait Matching (`PortraitAssets::for_speaker` in `src/ui.rs`)
If `DialogueNode.speaker` contains:
- `"CLARA"` -> displays `portraits/clara_portrait.png`
- `"MILLER"` -> displays `portraits/miller_portrait.png`
- `"VALE"` -> displays `portraits/vale_portrait.png`
- `"ECHO IN RON"` -> displays `portraits/lucker_ron_portrait.png`
- `"PINCKNEY"` -> displays `portraits/pinckney_portrait.png`
- `"GUINAN"` -> displays `portraits/zombie_portrait.png`
*(To add a brand-new suspect portrait, place `assets/portraits/<name>_portrait.png` and add a branch in `PortraitAssets` in `src/ui.rs`.)*
