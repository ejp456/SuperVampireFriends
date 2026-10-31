#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};

use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
    render::{camera::ClearColorConfig, view::RenderLayers},
};

use crate::{
    narrative::{CaseLibrary, ClueLedger, NarrativeState, SelectChoiceEvent},
    net::{DisconnectNetEvent, HostGameEvent, JoinGameEvent, NetRole, NetSession},
    senses::{BloodSight, SuspectHeartbeat},
    stats::{Archetype, DetectiveSheet, Faculty},
    world::{ActiveScene, NearbyHotspot, SCENE_WIDTH_FRACTION},
};

#[derive(Resource, Default)]
pub struct LedgerModalOpen(pub bool);

#[derive(Resource, Default)]
pub struct MultiplayerModalOpen {
    pub open: bool,
    pub code_input: String,
}

#[derive(Resource, Default, Clone)]
pub struct PortraitAssets {
    pub vale: Option<Handle<Image>>,
    pub clara: Option<Handle<Image>>,
    pub miller: Option<Handle<Image>>,
}

impl PortraitAssets {
    pub fn for_speaker(&self, speaker: &str) -> Option<Handle<Image>> {
        if speaker.contains("CLARA") {
            self.clara.clone()
        } else if speaker.contains("MILLER") {
            self.miller.clone()
        } else {
            None
        }
    }
}

#[derive(Component)]
struct HudStatsText;

#[derive(Component)]
struct BottomPromptText;

#[derive(Component)]
struct OpenCoopBarBtn;

#[derive(Component)]
struct OpenCoopBarBtnText;

#[derive(Component)]
struct LedgerModalRoot;

#[derive(Component)]
struct LedgerModalContent;

#[derive(Component)]
struct MpModalRoot;

#[derive(Component)]
struct MpStatusText;

#[derive(Component)]
struct MpInputText;

#[derive(Component, Clone, Copy)]
enum MpButtonAction {
    Host,
    CopyCode,
    JoinTyped,
    JoinLocal,
    Disconnect,
    Close,
}

#[derive(Component)]
struct PulseMonitorCard;

#[derive(Component)]
struct PulseMonitorText;

#[derive(Component)]
struct PulseMonitorBarFill;

#[derive(Component)]
struct ExchangeStage;

#[derive(Component)]
struct ChoicesContainer;

#[derive(Component)]
struct ChoiceButtonIndex(usize);

#[derive(Default)]
struct UiSyncTracker {
    last_generation: u64,
    last_archetype: String,
}

pub struct NoirUiPlugin;

impl Plugin for NoirUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LedgerModalOpen>()
            .init_resource::<MultiplayerModalOpen>()
            .init_resource::<PortraitAssets>()
            .add_systems(
                Startup,
                (load_portrait_assets, setup_ui_camera, build_split_screen_ui),
            )
            .add_systems(
                Update,
                (
                    handle_hud_shortcuts_and_coop_typing,
                    handle_mp_buttons,
                    update_minimal_hud,
                    update_mp_modal_ui,
                    update_pulse_monitor_widget,
                    sync_current_exchange_and_choices,
                    handle_choice_button_clicks,
                    sync_ledger_modal,
                ),
            );
    }
}

fn load_portrait_assets(
    mut portraits: ResMut<PortraitAssets>,
    asset_server: Option<Res<AssetServer>>,
) {
    if let Some(server) = asset_server {
        portraits.vale = Some(server.load("portraits/vale_portrait.png"));
        portraits.clara = Some(server.load("portraits/clara_portrait.png"));
        portraits.miller = Some(server.load("portraits/miller_portrait.png"));
    }
}

fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
        RenderLayers::layer(1),
    ));
}

fn build_split_screen_ui(mut commands: Commands) {
    let left_pct = SCENE_WIDTH_FRACTION * 100.0;
    let right_pct = 100.0 - left_pct;

    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|root| {
            // ================= LEFT PANE: CLEAN SCENE OVERLAY =================
            root.spawn(Node {
                width: Val::Percent(left_pct),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|left| {
                // Single minimal top stat line
                left.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.82)),
                ))
                .with_children(|top| {
                    top.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.82, 0.84, 0.90)),
                        HudStatsText,
                    ));
                });

                // Center Modal Layer (Clue Ledger OR Multiplayer Invite Modal)
                left.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                })
                .with_children(|mid| {
                    // 1. Clue Ledger Modal ([L])
                    mid.spawn((
                        Node {
                            width: Val::Percent(84.0),
                            max_height: Val::Percent(80.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.0)),
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.5)),
                            overflow: Overflow::scroll_y(),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.07, 0.07, 0.10, 0.96)),
                        BorderColor(Color::srgb(0.76, 0.60, 0.34)),
                        LedgerModalRoot,
                    ))
                    .with_children(|modal| {
                        modal.spawn((
                            Text::new("CLUE LEDGER  [Press L to Close]"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.94, 0.80, 0.50)),
                        ));
                        modal.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },
                            LedgerModalContent,
                        ));
                    });

                    // 2. Co-op Multiplayer Invite Modal ([M])
                    mid.spawn((
                        Node {
                            width: Val::Percent(86.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(18.0)),
                            row_gap: Val::Px(12.0),
                            border: UiRect::all(Val::Px(1.5)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.07, 0.07, 0.11, 0.97)),
                        BorderColor(Color::srgb(0.56, 0.72, 0.92)),
                        MpModalRoot,
                    ))
                    .with_children(|mp| {
                        mp.spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new("CO-OP DETECTIVE MULTIPLAYER"),
                                TextFont {
                                    font_size: 14.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.68, 0.84, 0.98)),
                            ));
                            spawn_mp_btn(hdr, "[Esc] Close", MpButtonAction::Close);
                        });

                        mp.spawn((
                            Text::new("Offline"),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.94, 0.84, 0.56)),
                            MpStatusText,
                        ));

                        // Host row
                        mp.spawn(Node {
                            column_gap: Val::Px(8.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|hrow| {
                            spawn_mp_btn(hrow, "Host Investigation", MpButtonAction::Host);
                            spawn_mp_btn(hrow, "Copy Invite Code", MpButtonAction::CopyCode);
                            spawn_mp_btn(hrow, "Disconnect", MpButtonAction::Disconnect);
                        });

                        // Join input box
                        mp.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.04, 0.04, 0.06)),
                            BorderColor(Color::srgb(0.38, 0.48, 0.64)),
                        ))
                        .with_children(|ibox| {
                            ibox.spawn((
                                Text::new("Invite Code: _"),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.92, 0.94, 0.98)),
                                MpInputText,
                            ));
                        });

                        // Join buttons row
                        mp.spawn(Node {
                            column_gap: Val::Px(8.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|jrow| {
                            spawn_mp_btn(jrow, "[Enter] Join by Code", MpButtonAction::JoinTyped);
                            spawn_mp_btn(jrow, "Quick Join Same PC (LOCAL)", MpButtonAction::JoinLocal);
                        });
                    });
                });

                // Minimal 1-line bottom prompt + Co-op button
                left.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.82)),
                ))
                .with_children(|bot| {
                    bot.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.82, 0.78)),
                        BottomPromptText,
                    ));

                    bot.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.16, 0.22)),
                        BorderColor(Color::srgb(0.42, 0.58, 0.78)),
                        OpenCoopBarBtn,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("[M] Co-op"),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.90, 0.98)),
                            OpenCoopBarBtnText,
                        ));
                    });
                });
            });

            // ================= RIGHT PANE: ONE-EXCHANGE-AT-A-TIME DIALOGUE =================
            root.spawn((
                Node {
                    width: Val::Percent(right_pct),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    border: UiRect::left(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.065, 0.065, 0.085)),
                BorderColor(Color::srgb(0.42, 0.32, 0.22)),
            ))
            .with_children(|right| {
                right
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                            row_gap: Val::Px(5.0),
                            border: UiRect::bottom(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.09, 0.08, 0.11)),
                        BorderColor(Color::srgb(0.28, 0.22, 0.28)),
                        PulseMonitorCard,
                    ))
                    .with_children(|pm| {
                        pm.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: 12.5,
                                ..default()
                            },
                            TextColor(Color::srgb(0.75, 0.75, 0.80)),
                            PulseMonitorText,
                        ));
                        pm.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(5.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.15, 0.14, 0.18)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent(20.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.55, 0.20, 0.28)),
                                PulseMonitorBarFill,
                            ));
                        });
                    });

                right.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(Val::Px(22.0), Val::Px(18.0)),
                        row_gap: Val::Px(16.0),
                        ..default()
                    },
                    ExchangeStage,
                ));

                right.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                        row_gap: Val::Px(7.0),
                        border: UiRect::top(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.085, 0.082, 0.108)),
                    BorderColor(Color::srgb(0.28, 0.24, 0.22)),
                    ChoicesContainer,
                ));
            });
        });
}

fn spawn_mp_btn(parent: &mut ChildBuilder, label: &str, action: MpButtonAction) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(11.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.16, 0.22)),
            BorderColor(Color::srgb(0.42, 0.50, 0.66)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: 12.5,
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.92, 0.96)),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn handle_hud_shortcuts_and_coop_typing(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut ledger_open: ResMut<LedgerModalOpen>,
    mut mp_open: ResMut<MultiplayerModalOpen>,
    mut sheet: ResMut<DetectiveSheet>,
    mut nar_state: ResMut<NarrativeState>,
    mut join_writer: EventWriter<JoinGameEvent>,
    coop_bar_btn: Query<&Interaction, (Changed<Interaction>, With<OpenCoopBarBtn>)>,
) {
    for inter in &coop_bar_btn {
        if *inter == Interaction::Pressed {
            mp_open.open = !mp_open.open;
            if mp_open.open {
                ledger_open.0 = false;
            }
        }
    }

    if mp_open.open {
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if ctrl && keys.just_pressed(KeyCode::KeyV) {
            if let Some(pasted) = read_windows_clipboard() {
                for ch in pasted.trim().chars() {
                    if is_allowed_invite_char(ch) && mp_open.code_input.len() < 24 {
                        mp_open.code_input.push(ch.to_ascii_uppercase());
                    }
                }
            }
            key_events.clear();
            return;
        }

        for ev in key_events.read() {
            if ev.state != ButtonState::Pressed {
                continue;
            }
            match &ev.logical_key {
                Key::Escape => {
                    mp_open.open = false;
                }
                Key::Backspace => {
                    mp_open.code_input.pop();
                }
                Key::Enter => {
                    if !mp_open.code_input.trim().is_empty() {
                        join_writer.send(JoinGameEvent(mp_open.code_input.clone()));
                    }
                }
                Key::Character(s) => {
                    if !ctrl {
                        for ch in s.chars() {
                            if is_allowed_invite_char(ch) && mp_open.code_input.len() < 24 {
                                mp_open.code_input.push(ch.to_ascii_uppercase());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        return;
    }

    key_events.clear();

    if keys.just_pressed(KeyCode::KeyM) {
        mp_open.open = true;
        ledger_open.0 = false;
        return;
    }

    if keys.just_pressed(KeyCode::KeyL) {
        ledger_open.0 = !ledger_open.0;
    }

    let chosen_arch = if keys.just_pressed(KeyCode::F1) {
        Some(Archetype::ForensicHound)
    } else if keys.just_pressed(KeyCode::F2) {
        Some(Archetype::BloodMystic)
    } else if keys.just_pressed(KeyCode::F3) {
        Some(Archetype::MidnightEnforcer)
    } else {
        None
    };

    if let Some(arch) = chosen_arch {
        let current_hunger = sheet.hunger;
        *sheet = DetectiveSheet::from_archetype(arch);
        sheet.hunger = current_hunger;
        nar_state.generation += 1;
    }
}

fn is_allowed_invite_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '.' || ch == ':'
}

#[allow(clippy::too_many_arguments)]
fn handle_mp_buttons(
    mut interactions: Query<
        (&Interaction, &MpButtonAction, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut mp_open: ResMut<MultiplayerModalOpen>,
    mut session: ResMut<NetSession>,
    mut host_w: EventWriter<HostGameEvent>,
    mut join_w: EventWriter<JoinGameEvent>,
    mut disc_w: EventWriter<DisconnectNetEvent>,
) {
    for (inter, action, mut bg) in &mut interactions {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.28, 0.34, 0.48);
                match action {
                    MpButtonAction::Host => {
                        host_w.send(HostGameEvent);
                    }
                    MpButtonAction::CopyCode => {
                        if !session.invite_code.is_empty() {
                            copy_to_windows_clipboard(&session.invite_code);
                            session.status_message = format!(
                                "Copied Invite Code '{}' to clipboard!",
                                session.invite_code
                            );
                        }
                    }
                    MpButtonAction::JoinTyped => {
                        if !mp_open.code_input.trim().is_empty() {
                            join_w.send(JoinGameEvent(mp_open.code_input.clone()));
                        }
                    }
                    MpButtonAction::JoinLocal => {
                        mp_open.code_input = "LOCAL".to_string();
                        join_w.send(JoinGameEvent("LOCAL".to_string()));
                    }
                    MpButtonAction::Disconnect => {
                        disc_w.send(DisconnectNetEvent);
                    }
                    MpButtonAction::Close => {
                        mp_open.open = false;
                    }
                }
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.22, 0.25, 0.34);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.15, 0.16, 0.22);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_to_windows_clipboard(text: &str) {
    let mut cmd = Command::new("clip");
    cmd.stdin(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    if let Ok(mut child) = cmd.spawn() {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

#[cfg(target_arch = "wasm32")]
fn copy_to_windows_clipboard(_text: &str) {}

#[cfg(not(target_arch = "wasm32"))]
fn read_windows_clipboard() -> Option<String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", "Get-Clipboard"]);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = cmd.output().ok()?;
    String::from_utf8(out.stdout).ok()
}

#[cfg(target_arch = "wasm32")]
fn read_windows_clipboard() -> Option<String> {
    None
}

fn update_mp_modal_ui(
    mp_open: Res<MultiplayerModalOpen>,
    session: Res<NetSession>,
    mut modal_q: Query<&mut Node, With<MpModalRoot>>,
    mut status_q: Query<&mut Text, (With<MpStatusText>, Without<MpInputText>, Without<OpenCoopBarBtnText>)>,
    mut input_q: Query<&mut Text, (With<MpInputText>, Without<MpStatusText>, Without<OpenCoopBarBtnText>)>,
    mut bar_btn_q: Query<&mut Text, (With<OpenCoopBarBtnText>, Without<MpStatusText>, Without<MpInputText>)>,
) {
    if let Ok(mut node) = modal_q.get_single_mut() {
        node.display = if mp_open.open {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut st) = status_q.get_single_mut() {
        st.0 = session.status_message.clone();
    }

    if let Ok(mut inp) = input_q.get_single_mut() {
        if mp_open.code_input.is_empty() {
            inp.0 = "Type or Ctrl+V Invite Code: _  (e.g. XXXXX-XXXXX or LOCAL)".to_string();
        } else {
            inp.0 = format!("Invite Code: {}_", mp_open.code_input);
        }
    }

    if let Ok(mut btxt) = bar_btn_q.get_single_mut() {
        btxt.0 = match session.role {
            NetRole::Offline => "[M] Co-op".to_string(),
            NetRole::Hosting => format!("[M] Code: {} ({})", session.invite_code, session.peers.len() + 1),
            NetRole::Client => format!("[M] Co-op ({})", session.peers.len() + 1),
        };
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_minimal_hud(
    sheet: Res<DetectiveSheet>,
    blood_sight: Res<BloodSight>,
    active_scene: Res<ActiveScene>,
    nearby: Res<NearbyHotspot>,
    ledger: Res<ClueLedger>,
    library: Res<CaseLibrary>,
    mut stats_txt: Query<&mut Text, (With<HudStatsText>, Without<BottomPromptText>)>,
    mut prompt_txt: Query<&mut Text, (With<BottomPromptText>, Without<HudStatsText>)>,
) {
    if let Ok(mut txt) = stats_txt.get_single_mut() {
        let pips = sheet.hunger_pips();
        let mut bar = String::new();
        for i in 0..5 {
            bar.push(if i < pips { '◆' } else { '◇' });
        }
        let sight_tag = if blood_sight.active { "  •  BLOOD SIGHT ON" } else { "" };
        let loc_title = if active_scene.current_title.is_empty() {
            library.title.as_str()
        } else {
            active_scene.current_title.as_str()
        };
        txt.0 = format!(
            "{}  |  DED {}  SAN {}  NOC {}  PUL {}  |  HUNGER {}{}",
            loc_title,
            sheet.effective_stat(Faculty::Deduction),
            sheet.effective_stat(Faculty::Sanguine),
            sheet.effective_stat(Faculty::Nocturne),
            sheet.effective_stat(Faculty::Pulse),
            bar,
            sight_tag,
        );
    }

    if let Ok(mut txt) = prompt_txt.get_single_mut() {
        if let Some(lbl) = &nearby.label {
            txt.0 = format!("Click / [E] {lbl}");
        } else {
            txt.0 = format!(
                "Click to Walk & Inspect   •   Tab: Blood Sight   •   L: Clues ({}/{})",
                ledger.discovered.len(),
                library.clue_order.len()
            );
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_pulse_monitor_widget(
    nar_state: Res<NarrativeState>,
    heartbeats: Query<&SuspectHeartbeat>,
    mut card_q: Query<&mut Node, (With<PulseMonitorCard>, Without<PulseMonitorBarFill>)>,
    mut text_q: Query<(&mut Text, &mut TextColor), With<PulseMonitorText>>,
    mut bar_q: Query<(&mut Node, &mut BackgroundColor), (With<PulseMonitorBarFill>, Without<PulseMonitorCard>)>,
) {
    let active_hb = nar_state
        .active_suspect_id
        .as_ref()
        .and_then(|id| heartbeats.iter().find(|h| &h.npc_id == id));

    let Ok(mut card_node) = card_q.get_single_mut() else {
        return;
    };

    if let Some(hb) = active_hb {
        card_node.display = Display::Flex;
        let spike = hb.is_lie_spike || hb.current_bpm >= 120.0;
        let accent = if spike {
            Color::srgb(1.0, 0.26, 0.34)
        } else {
            Color::srgb(0.48, 0.86, 0.72)
        };
        let state_word = if spike { "LIE / PANIC SPIKE" } else { "STEADY" };

        if let Ok((mut t, mut c)) = text_q.get_single_mut() {
            t.0 = format!(
                "♥ {} — {:.0} BPM (base {:.0}) [{}]",
                hb.display_name, hb.current_bpm, hb.resting_bpm, state_word
            );
            c.0 = accent;
        }
        if let Ok((mut bar_node, mut bg)) = bar_q.get_single_mut() {
            let pct = (20.0 + hb.last_beat_flash * 80.0).clamp(12.0, 100.0);
            bar_node.width = Val::Percent(pct);
            bg.0 = accent;
        }
    } else {
        card_node.display = Display::None;
    }
}

#[allow(clippy::type_complexity)]
fn sync_current_exchange_and_choices(
    mut commands: Commands,
    nar_state: Res<NarrativeState>,
    sheet: Res<DetectiveSheet>,
    portraits: Res<PortraitAssets>,
    stage_q: Query<(Entity, Option<&Children>), (With<ExchangeStage>, Without<ChoicesContainer>)>,
    choices_q: Query<(Entity, Option<&Children>), (With<ChoicesContainer>, Without<ExchangeStage>)>,
    mut tracker: Local<UiSyncTracker>,
) {
    let arch_changed = tracker.last_archetype != sheet.archetype_title;
    if nar_state.generation == tracker.last_generation && !arch_changed {
        return;
    }
    tracker.last_generation = nar_state.generation;
    tracker.last_archetype = sheet.archetype_title.clone();

    if let Ok((stage_entity, children)) = stage_q.get_single() {
        if let Some(children) = children {
            let list: Vec<Entity> = children.iter().copied().collect();
            for ch in list {
                commands.entity(ch).despawn_recursive();
            }
        }

        commands.entity(stage_entity).with_children(|stage| {
            if nar_state.current_node_id.is_empty() {
                stage.spawn((
                    Text::new("Click anywhere to walk, or click a suspect / clue to inspect."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.54, 0.54, 0.62)),
                ));
                return;
            }

            if let Some(my_line) = &nar_state.my_line {
                stage
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                            border: UiRect::left(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.13, 0.18, 0.90)),
                        BorderColor(Color::srgb(0.46, 0.66, 0.86)),
                    ))
                    .with_children(|me_row| {
                        if let Some(port) = &portraits.vale {
                            me_row.spawn((
                                ImageNode::new(port.clone()),
                                Node {
                                    width: Val::Px(64.0),
                                    height: Val::Px(64.0),
                                    flex_shrink: 0.0,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor(Color::srgb(0.42, 0.58, 0.78)),
                            ));
                        }

                        me_row
                            .spawn(Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|me_box| {
                                me_box.spawn((
                                    Text::new("YOU — HARLAN VALE"),
                                    TextFont {
                                        font_size: 11.5,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.56, 0.76, 0.94)),
                                ));
                                me_box.spawn((
                                    Text::new(my_line.clone()),
                                    TextFont {
                                        font_size: 15.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.88, 0.91, 0.95)),
                                ));
                            });
                    });
            }

            stage
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(14.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(12.0)),
                        border: UiRect::left(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.14, 0.12, 0.16, 0.95)),
                    BorderColor(Color::srgb(0.82, 0.62, 0.34)),
                ))
                .with_children(|them_row| {
                    if let Some(port) = portraits.for_speaker(&nar_state.other_speaker) {
                        them_row.spawn((
                            ImageNode::new(port),
                            Node {
                                width: Val::Px(74.0),
                                height: Val::Px(74.0),
                                flex_shrink: 0.0,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor(Color::srgb(0.76, 0.58, 0.32)),
                        ));
                    }

                    them_row
                        .spawn(Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|them_box| {
                            them_box.spawn((
                                Text::new(nar_state.other_speaker.clone()),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.96, 0.82, 0.54)),
                            ));
                            them_box.spawn((
                                Text::new(nar_state.other_line.clone()),
                                TextFont {
                                    font_size: 15.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.94, 0.92, 0.88)),
                            ));

                            if let Some((whisper, color)) = &nar_state.whisper_line {
                                them_box.spawn((
                                    Text::new(whisper.clone()),
                                    TextFont {
                                        font_size: 12.5,
                                        ..default()
                                    },
                                    TextColor(*color),
                                ));
                            }

                            if let Some((badge, ok)) = &nar_state.outcome_badge {
                                let badge_color = if *ok {
                                    Color::srgb(0.52, 0.88, 0.66)
                                } else {
                                    Color::srgb(0.94, 0.48, 0.44)
                                };
                                them_box.spawn((
                                    Text::new(badge.clone()),
                                    TextFont {
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor(badge_color),
                                ));
                            }
                        });
                });
        });
    }

    if let Ok((choices_entity, children)) = choices_q.get_single() {
        if let Some(children) = children {
            let list: Vec<Entity> = children.iter().copied().collect();
            for ch in list {
                commands.entity(ch).despawn_recursive();
            }
        }

        commands.entity(choices_entity).with_children(|tray| {
            for (idx, resolved) in nar_state.current_choices.iter().enumerate() {
                let prefix = if let Some(sc) = &resolved.raw.skill_check {
                    if resolved.locked_check_failed {
                        format!("{}. [{} — Failed] ", idx + 1, sc.faculty.name())
                    } else {
                        let pct = sheet.success_chance_percent(sc.faculty, sc.dc);
                        format!("{}. [{} {}%] ", idx + 1, sc.faculty.name(), pct)
                    }
                } else {
                    format!("{}. ", idx + 1)
                };

                let text_color = if resolved.locked_check_failed {
                    Color::srgb(0.48, 0.45, 0.50)
                } else if let Some(sc) = &resolved.raw.skill_check {
                    sc.faculty.color()
                } else {
                    Color::srgb(0.92, 0.86, 0.74)
                };

                tray.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.13, 0.12, 0.16)),
                    BorderColor(Color::srgb(0.30, 0.27, 0.34)),
                    ChoiceButtonIndex(idx),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(format!("{prefix}{}", resolved.raw.label)),
                        TextFont {
                            font_size: 13.5,
                            ..default()
                        },
                        TextColor(text_color),
                    ));
                });
            }
        });
    }
}

fn handle_choice_button_clicks(
    mut interactions: Query<
        (&Interaction, &ChoiceButtonIndex, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut writer: EventWriter<SelectChoiceEvent>,
) {
    for (inter, idx, mut bg) in &mut interactions {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.28, 0.16, 0.20);
                writer.send(SelectChoiceEvent(idx.0));
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.20, 0.17, 0.24);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.13, 0.12, 0.16);
            }
        }
    }
}

fn sync_ledger_modal(
    mut commands: Commands,
    ledger_open: Res<LedgerModalOpen>,
    ledger: Res<ClueLedger>,
    library: Res<CaseLibrary>,
    mut modal_root_q: Query<&mut Node, With<LedgerModalRoot>>,
    content_q: Query<(Entity, Option<&Children>), With<LedgerModalContent>>,
    mut last_count: Local<usize>,
    mut initialized: Local<bool>,
) {
    if let Ok(mut root_node) = modal_root_q.get_single_mut() {
        root_node.display = if ledger_open.0 {
            Display::Flex
        } else {
            Display::None
        };
    }

    if *initialized && *last_count == ledger.discovered.len() {
        return;
    }
    *initialized = true;
    *last_count = ledger.discovered.len();

    let Ok((content_entity, children)) = content_q.get_single() else {
        return;
    };
    if let Some(children) = children {
        let list: Vec<Entity> = children.iter().copied().collect();
        for ch in list {
            commands.entity(ch).despawn_recursive();
        }
    }

    commands.entity(content_entity).with_children(|col| {
        col.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|fac_box| {
            for fac in Faculty::ALL {
                fac_box.spawn((
                    Text::new(format!("• {}: {}", fac.name(), fac.short_desc())),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(fac.color()),
                ));
            }
        });

        if ledger.discovered.is_empty() {
            col.spawn((
                Text::new("No clues recorded yet."),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.76)),
            ));
            return;
        }

        for clue_id in &ledger.discovered {
            if let Some(def) = library.clues.get(clue_id) {
                col.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::left(Val::Px(2.5)),
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.11, 0.15)),
                    BorderColor(Color::srgb(0.86, 0.68, 0.34)),
                ))
                .with_children(|item| {
                    item.spawn((
                        Text::new(def.title.clone()),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.84, 0.52)),
                    ));
                    item.spawn((
                        Text::new(def.summary.clone()),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.82, 0.86)),
                    ));
                });
            }
        }
    });
}
