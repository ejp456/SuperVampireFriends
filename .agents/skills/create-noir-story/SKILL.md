---
name: create-noir-story
description: >-
  Interactive story-creation wizard for authoring new playable noir mysteries in
  Crimson Ledger (vampire_noir_rpg). Use this skill whenever a developer asks to
  create, design, scaffold, or add a new story, case, mystery, or SceneCatalog,
  or triggers /create-noir-story.
---

# Create Noir Story Wizard

This skill guides developers step-by-step through designing, authoring, registering, and validating a new playable mystery in **Crimson Ledger** so it appears on the **Story Selection** screen, loads its own `SceneCatalog` (with `story_name`), works with all 4 supernatural detectives, and syncs in Co-op multiplayer.

## Architecture Overview

Every playable story in Crimson Ledger consists of three parts:
1. **Scene Catalog RON** (`assets/scenes/<story_id>_scenes.ron`): Defines `SceneCatalog(story_name: "...", initial_scene: "...", initial_spawn: (...), scenes: [...])` with linked rooms/alleys/streets, props, NPCs, clues, Blood Sight echoes, and doorways.
2. **Case Narrative RON** (`assets/cases/<story_id>.ron`): Defines `CaseData(title: "...", subtitle: "...", opening_node: "...", clues: [...], nodes: [...])` with branching interrogations, `SkillCheckSpec` rolls, `PulseTrigger` spikes, and endings.
3. **Story Registration** (`src/stories.rs`): Adds a `StoryEntry` (with `include_str!` embedded fallbacks for WebAssembly) to `STORIES` so both native and browser builds list and host the story immediately.

Consult these reference files as needed during authoring:
- **Exact RON Schemas, Placeholders & Walkability Rules**: [references/ron_schemas_and_rules.md](./references/ron_schemas_and_rules.md)
- **Ready-to-Use Sprites, Props & Custom Asset Pipeline**: [references/asset_catalog.md](./references/asset_catalog.md)
- **Starter Templates**: [resources/case_template.ron](./resources/case_template.ron) and [resources/scenes_template.ron](./resources/scenes_template.ron)
- **Scaffolding Helper Script**: [scripts/scaffold_story.ps1](./scripts/scaffold_story.ps1)

---

## Interactive 5-Step Wizard Workflow

Guide the developer **one phase at a time** using `ask_question` (or plain text for open-ended brainstorming). If the developer already supplied details in their initial prompt, skip the answered questions and advance directly to the next phase!

### Phase 1: Mystery Concept, ID & Culprit
If the core concept is not yet defined, ask the developer (via `ask_question` or a concise prompt) to establish:
1. **Story Premise / Theme** (offer 3 evocative supernatural noir pitches via `ask_question` if they want inspiration, or let them pitch their own).
2. **`story_name` & `story_id`**:
   - Inspect `src/stories.rs` first to find the next unused ID (e.g., `case_03`).
   - Confirm the human-readable `story_name` (e.g., `"The Clocktower Requiem"`) and night timestamp/setting line (e.g., `"4:10 AM  •  St. Jude Belltower & Canal"`).
3. **Victim, Suspects & Twist**: Confirm who died, how the crime scene was staged, which living NPC is lying (pulse spikes!), and who is telling a partial truth.

Wait for confirmation before Phase 2.

### Phase 2: Linked Scene Layout & Prop Kit
Read [references/asset_catalog.md](./references/asset_catalog.md) and use `ask_question` to align on:
1. **Scene Count & Environments**: Recommend **3 linked scenes** mixing `Room`, `Alley`, and `Street` (with rain & neon puddles on exterior scenes).
2. **Visual Assets**:
   - Ask whether to build entirely from the **30+ existing noir props & character sprites** in `assets/props/` and `assets/sprites/`, OR generate new custom pixel-art props/NPCs via the `image-generator` subagent + `src/bin/prep_sprites.rs`.
3. **Doorway Graph**: Confirm how the player walks between the scenes (every scene must have at least 1 `SceneDoorwaySpec` whose `target_spawn` lands on unblocked floor!).

Wait for confirmation before Phase 3.

### Phase 3: Clues, Supernatural Senses & Detective Exclusives
Confirm the investigative mechanics using `ask_question`:
1. **Clue Web (6–9 clues recommended) & Scene Cypher Puzzle**:
   - Include at least **1 supernatural-sense-gated hotspot** (`requires_blood_sight_unless_clue: Some("<clue_id>")` paired with a `SceneBloodEcho` trail leading to it when the player presses `[Tab]`).
   - Include **1 interactive `SceneCypher` + `CypherDef`** (`Caesar(shift: N)`, `A1Z26`, or `Atbash`) placed on a desk, briefcase, or cabinet—solving it grants a strong `ClueDef` and raises the player's 2d6 skill-check roll modifier (`roll_bonus: 2`).
2. **All 4 Playable Detectives**:
   - Use `{detective}`, `{sense}`, and `{strain}` placeholders in dialogue so NPCs dynamically address whichever character the player picks!
   - Include `PassiveVoice` whispers or `requires_character` exclusive options tailored to:
     - `Richey` (Vampire — arterial iron, vintage blood notes, `Sanguine` / `Nocturne`)
     - `LuckerAndRon` (Ghost Partner + Living Host — death-echo séances, spirit whispers)
     - `Pinckney` (Werewolf — lupine spoor tracking, fear-sweat & chemical scent trails)
     - `Zombie` (Det. Guinan — mortis psychometry on corpses/objects, post-mortem lividity)
3. **Endings**: Confirm 2–4 endings (e.g., airtight `Deduction` arrest, supernatural `Nocturne`expose, botched/compromised arrest, or corrupt cover-up).

---

## Phase 4: Scaffold, Author & Register the Story

Once the design is approved:

1. **Read Schema Rules**: Read [references/ron_schemas_and_rules.md](./references/ron_schemas_and_rules.md) carefully to avoid RON type-name or collider pitfalls.
2. **Scaffold or Write the Files**:
   - You can scaffold starter files with:
     ```powershell
     powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/create-noir-story/scripts/scaffold_story.ps1 -StoryId "case_03" -StoryName "The Clocktower Requiem"
     ```
   - Or write `assets/scenes/<story_id>_scenes.ron` and `assets/cases/<story_id>.ron` directly.
   - **CRITICAL**: Ensure `SceneCatalog(...)` in `assets/scenes/<story_id>_scenes.ron` sets `story_name: "<Story Name>"` at the top!
3. **Register in `src/stories.rs`**:
   - Add `const EMBEDDED_<UPPER_ID>: &str = include_str!("../assets/cases/<story_id>.ron");`
   - Add `const EMBEDDED_<UPPER_ID>_SCENES: &str = include_str!("../assets/scenes/<story_id>_scenes.ron");`
   - Append a `StoryEntry` to `pub const STORIES: &[StoryEntry]` in `src/stories.rs` with a distinct `accent_rgb`.
   - *(Optional)* If you added new NPC portrait PNGs in `assets/portraits/`, add a field to `PortraitAssets` and match the speaker's uppercase name in `PortraitAssets::for_speaker` inside `src/ui.rs`.

---

## Phase 5: Mandatory Validation & Browser Build

Always run these verification steps before declaring the new story finished:

1. **Run the Automated Story & Walkability Suite**:
   ```powershell
   cargo test --bin vampire_noir_rpg
   ```
   This automatically validates **every** story in `STORIES` for:
   - Non-empty `SceneCatalog.story_name` and valid `initial_scene`
   - Unblocked 2D A* walkability (`player_half = 13.0`) at `initial_spawn` and every `SceneDoorwaySpec.target_spawn`
   - Valid `dialogue_node` / `node_id` links for every NPC, clue, `DialogueChoice`, and `SkillCheckSpec`
   - Existence and valid dimensions of every referenced sprite PNG under `assets/`

2. **Rebuild WebAssembly (if browser play is active)**:
   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File build_web.ps1
   ```
   And increment the `?v=<N>` cache-buster query string on `./pkg/vampire_noir_rpg.js` and `./pkg/vampire_noir_rpg_bg.wasm` in `web/index.html` so refreshing `http://127.0.0.1:8080` loads the new story immediately.
