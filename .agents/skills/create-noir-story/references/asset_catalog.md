# Asset Catalog & Custom Sprite Pipeline

All paths below are relative to `assets/` (matching Bevy's `AssetServer` paths in `.ron` files).

## 1. Character Sprites & Portraits

### Playable Detectives (Managed Automatically)
- `sprites/richey.png` + `portraits/richey_portrait.png`
- `sprites/lucker_ron.png` + `portraits/lucker_ron_portrait.png`
- `sprites/pinckney.png` + `portraits/pinckney_portrait.png`
- `sprites/zombie.png` + `portraits/zombie_portrait.png`

### Reusable NPCs & Victims
| Sprite Path | Portrait Path | Recommended `sprite_size` | Recommended `collider_half` | Archetype Ideas |
| :--- | :--- | :--- | :--- | :--- |
| `sprites/clara.png` | `portraits/clara_portrait.png` | `(68.0, 68.0)` | `Some((14.0, 14.0))` | Lounge singer, night clerk, heiress, witness |
| `sprites/miller.png` | `portraits/miller_portrait.png` | `(68.0, 68.0)` | `Some((14.0, 14.0))` | Detective Sgt., Harbor Lt., crooked beat cop, watchman |
| `sprites/vale.png` | `portraits/vale_portrait.png` | `(68.0, 68.0)` | `Some((14.0, 14.0))` | Night coroner, occult scholar, archivist, syndicate fixer |
| `sprites/vance_corpse.png` | *(none)* | `(78.0, 78.0)`–`(82.0, 82.0)` | `None` | Murder victim on rug, autopsy slab, alley, or dock |

---

## 2. Blood Forensics & Supernatural `[Tab]` Echo Sprites (`assets/props/`)

### Mundane Blood Surfaces / Clues
- `props/blood_pool.png` — Coagulated arterial blood puddle (`size: (36.0, 28.0)` to `(56.0, 42.0)`)
- `props/blood_drag_smear.png` — Scrubbed heel/body drag smear (`size: (54.0, 24.0)`)
- `props/blood_drops.png` — Spatter droplets (`size: (36.0, 24.0)`)
- `props/blood_bootprint.png` — Bloody boot tread (`size: (32.0, 20.0)`, use `rotation_deg` to aim trail!)

### Supernatural `[Tab]` Echoes (`SceneBloodEcho`, auto-tinted per Detective!)
*(White/alpha grayscale masks tinted crimson for Richey, cyan ectoplasm for Lucker & Ron, amber spoor for Pinckney, violet mortis for Zombie Guinan)*
- `props/echo_pool.png` — Glowing kill-site or ritual residue pool (`size: (58.0, 44.0)`)
- `props/echo_smear.png` — Directional drag/carry trail (`size: (56.0, 22.0)`, `rotation_deg`)
- `props/echo_drops.png` — Glowing droplet / ectoplasm / scent trail (`size: (44.0, 22.0)`, `rotation_deg`)
- `props/echo_bootprint.png` — Spectral / scent footprint (`size: (36.0, 22.0)`, `rotation_deg`)

---

## 3. Noir Scene-Setting Props (`assets/props/`)

| Asset Path | Typical `size` | Typical `collider_half` (`solid: true`) | Best Environments |
| :--- | :--- | :--- | :--- |
| `props/detective_desk.png` | `(112.0, 74.0)` | `Some((46.0, 26.0))` | Precinct, office, coroner study |
| `props/filing_cabinet.png` | `(56.0, 84.0)` | `Some((22.0, 34.0))` | Office, morgue, archives |
| `props/morgue_autopsy_table.png` | `(128.0, 74.0)` | `Some((54.0, 24.0))` | Morgue, clinic, secret lab |
| `props/occult_candelabra.png` | `(56.0, 66.0)` | `Some((18.0, 20.0))` | Sanctum, hidden alcove, velvet study |
| `props/crime_chalk_markers.png` | `(54.0, 42.0)` | `None` (`solid: false`) | Any crime scene floor |
| `props/scattered_papers.png` | `(36.0, 30.0)` | `None` (`solid: false`) | Ledger clues, desk top, alley |
| `props/crimson_rug.png` | `(180.0, 120.0)` | Surface (`z: 0.7..0.8`) | Lounge, office, mansion parlor |
| `props/velvet_curtain.png` | `(220.0, 62.0)` | `None` (`solid: false`, `z: 3.4`) | Stage, VIP booth, cold-storage curtain |
| `props/mahogany_bar.png` | `(84.0, 185.0)` | `Some((21.0, 92.5))` | Speakeasy, lounge, hotel bar |
| `props/overturned_poker_table.png` | `(96.0, 78.0)` | `Some((38.0, 28.0))` | Backroom struggle, gambling den |
| `props/club_table_chairs.png` | `(96.0, 72.0)` | `Some((38.0, 26.0))` | Lounge, diner, interrogation room |
| `props/vintage_jukebox.png` | `(60.0, 86.0)` | `Some((24.0, 34.0))` | Bar, diner, evidence room |
| `props/neon_lounge_sign.png` | `(132.0, 52.0)` | `None` (`solid: false`, `z: 3.3`) | Exterior brick wall above doorway |
| `props/alley_dumpster.png` | `(112.0, 76.0)` | `Some((48.0, 28.0))` | Service alley, behind club |
| `props/metal_trash_cans.png` | `(58.0, 56.0)` | `Some((22.0, 20.0))` | Alley, street corner, pier |
| `props/rusted_drainpipe.png` | `(34.0, 96.0)` | `Some((12.0, 20.0))` | Alley wall, warehouse |
| `props/wooden_crates.png` | `(86.0, 78.0)` | `Some((34.0, 28.0))` | Docks, warehouse, alley |
| `props/harbor_bollard_barrels.png` | `(88.0, 74.0)` | `Some((32.0, 24.0))` | Waterfront, pier, smuggling wharf |
| `props/phone_booth.png` | `(58.0, 96.0)` | `Some((20.0, 28.0))` | Street corner, pier, precinct |
| `props/fire_hydrant.png` | `(36.0, 48.0)` | `Some((12.0, 14.0))` | Sidewalk curb, waterfront |
| `props/streetlamp.png` | `(44.0, 108.0)` | `Some((10.0, 12.0))` | Sidewalk, fogbound pier |
| `props/police_cruiser.png` | `(148.0, 74.0)` | `Some((62.0, 26.0))` | Street, crime cordon, harbor apron |

---

## 4. Generating & Chroma-Keying New Custom Assets (Optional)

If a new story needs bespoke props or new NPC sprites/portraits:
1. Delegate generation to the `image-generator` subagent requesting **1940s dark pixel-art noir top-down 3/4 orthographic perspective** isolated on a flat solid `#FF00FF` magenta (for green/warm props) or `#00FF00` green (for crimson/purple props) background with zero ground drop-shadows outside the object.
2. Add the raw image entry to `src/bin/prep_sprites.rs` (or process using the same `process_prop_tight` chroma flood-fill + tight bounding-box crop) so the output PNG in `assets/props/<name>.png` or `assets/sprites/<name>.png` has clean `0`-alpha transparency.
3. Run `cargo test --bin vampire_noir_rpg` to confirm all sprite PNGs decode cleanly.
