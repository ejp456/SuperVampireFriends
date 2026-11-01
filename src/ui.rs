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
    cypher::{
        format_caesar_wheel_lines, ActiveCypherModal, CypherKind, SubmitCypherGuessEvent,
    },
    narrative::{CaseLibrary, ClueLedger, NarrativeState, SelectChoiceEvent},
    net::{DisconnectNetEvent, HostGameEvent, JoinGameEvent, NetRole, NetSession},
    senses::{BloodSight, SuspectHeartbeat},
    stats::{DetectiveId, DetectiveSheet, Faculty},
    stories::{ActiveStory, SelectStoryEvent, StoryCatalog, StorySummary, StorySystems},
    world::{ActiveScene, NearbyHotspot, SCENE_WIDTH_FRACTION},
};

#[derive(Resource, Default)]
pub struct LedgerModalOpen(pub bool);

/// Opens first on game launch so players pick which story (`SceneCatalog` + `CaseData`)
/// to play or host, then advances directly to `CharacterSelectModalOpen`.
/// Can be reopened anytime with `[V]` or the bottom HUD `[V] Story` button.
#[derive(Resource)]
pub struct StorySelectModalOpen(pub bool);

impl Default for StorySelectModalOpen {
    fn default() -> Self {
        Self(true)
    }
}

/// Opens right after a story is chosen on the Story Selection screen,
/// and can be reopened anytime with `[C]` or the bottom HUD button.
#[derive(Resource, Default)]
pub struct CharacterSelectModalOpen(pub bool);

#[derive(Resource, Default)]
pub struct MultiplayerModalOpen {
    pub open: bool,
    pub code_input: String,
}

#[derive(Resource, Default, Clone)]
pub struct PortraitAssets {
    pub richey: Option<Handle<Image>>,
    pub lucker_ron: Option<Handle<Image>>,
    pub pinckney: Option<Handle<Image>>,
    pub zombie: Option<Handle<Image>>,
    pub clara: Option<Handle<Image>>,
    pub miller: Option<Handle<Image>>,
    pub vale: Option<Handle<Image>>,
    pub sabine: Option<Handle<Image>>,
    pub vespera: Option<Handle<Image>>,
    pub corso: Option<Handle<Image>>,
}

impl PortraitAssets {
    pub fn for_detective(&self, character: DetectiveId) -> Option<Handle<Image>> {
        match character {
            DetectiveId::Richey => self.richey.clone(),
            DetectiveId::LuckerAndRon => self.lucker_ron.clone(),
            DetectiveId::Pinckney => self.pinckney.clone(),
            DetectiveId::Zombie => self.zombie.clone(),
        }
    }

    pub fn for_speaker(&self, speaker: &str) -> Option<Handle<Image>> {
        if speaker.contains("CLARA") {
            self.clara.clone()
        } else if speaker.contains("MILLER") {
            self.miller.clone()
        } else if speaker.contains("VALE") {
            self.vale.clone()
        } else if speaker.contains("SABINE") {
            self.sabine.clone()
        } else if speaker.contains("VESPERA") {
            self.vespera.clone()
        } else if speaker.contains("CORSO") {
            self.corso.clone()
        } else if speaker.contains("RICHEY") {
            self.richey.clone()
        } else if speaker.contains("ECHO IN RON") {
            self.lucker_ron.clone()
        } else if speaker.contains("PINCKNEY") {
            self.pinckney.clone()
        } else if speaker.contains("GUINAN") {
            self.zombie.clone()
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
struct OpenStoryBarBtn;

#[derive(Component)]
struct OpenStoryBarBtnText;

#[derive(Component)]
struct OpenCharBarBtn;

#[derive(Component)]
struct OpenCharBarBtnText;

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

#[derive(Component)]
struct CharSelectModalRoot;

#[derive(Component)]
struct CharSelectStoryBannerText;

#[derive(Component)]
struct BackToStorySelectBtn;

#[derive(Component)]
struct StorySelectModalRoot;

#[derive(Component)]
struct StorySelectCard {
    story_id: String,
    accent_rgb: (f32, f32, f32),
}

#[derive(Component)]
struct StorySelectCardStatusText {
    story_id: String,
    index_one_based: usize,
}

#[derive(Component)]
struct CloseStorySelectBtn;

#[derive(Component)]
struct CharSelectCard {
    character: DetectiveId,
}

#[derive(Component)]
struct CharSelectCardStatusText {
    character: DetectiveId,
}

#[derive(Component)]
struct CloseCharSelectBtn;

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
struct CypherModalRoot;

#[derive(Component)]
struct CypherModalContent;

#[derive(Component, Clone, Copy)]
enum CypherBtnAction {
    Close,
    ShiftLeft,
    ShiftRight,
    ToggleTip,
    TypeChar(char),
    Space,
    Backspace,
    Clear,
    Submit,
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
    last_character: Option<DetectiveId>,
    last_cypher_bonus: i32,
}

pub struct NoirUiPlugin;

impl Plugin for NoirUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LedgerModalOpen>()
            .init_resource::<StorySelectModalOpen>()
            .init_resource::<CharacterSelectModalOpen>()
            .init_resource::<MultiplayerModalOpen>()
            .init_resource::<PortraitAssets>()
            .add_systems(
                Startup,
                (load_portrait_assets, setup_ui_camera, build_split_screen_ui).chain(),
            )
            .add_systems(
                Update,
                (
                    handle_hud_shortcuts_and_coop_typing,
                    handle_story_select_interactions,
                    handle_char_select_interactions,
                    handle_mp_buttons,
                    handle_cypher_modal_buttons,
                )
                    .before(StorySystems),
            )
            .add_systems(
                Update,
                (
                    update_minimal_hud,
                    update_story_select_modal_ui,
                    update_char_select_modal_ui,
                    update_mp_modal_ui,
                    sync_cypher_modal_ui,
                    update_pulse_monitor_widget,
                    sync_current_exchange_and_choices,
                    handle_choice_button_clicks,
                    sync_ledger_modal,
                )
                    .after(StorySystems),
            );
    }
}

fn load_portrait_assets(
    mut portraits: ResMut<PortraitAssets>,
    asset_server: Option<Res<AssetServer>>,
) {
    if let Some(server) = asset_server {
        portraits.richey = Some(server.load(DetectiveId::Richey.portrait_path()));
        portraits.lucker_ron = Some(server.load(DetectiveId::LuckerAndRon.portrait_path()));
        portraits.pinckney = Some(server.load(DetectiveId::Pinckney.portrait_path()));
        portraits.zombie = Some(server.load(DetectiveId::Zombie.portrait_path()));
        portraits.clara = Some(server.load("portraits/clara_portrait.png"));
        portraits.miller = Some(server.load("portraits/miller_portrait.png"));
        portraits.vale = Some(server.load("portraits/vale_portrait.png"));
        portraits.sabine = Some(server.load("portraits/sabine_portrait.png"));
        portraits.vespera = Some(server.load("portraits/vespera_portrait.png"));
        portraits.corso = Some(server.load("portraits/corso_portrait.png"));
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

fn build_split_screen_ui(
    mut commands: Commands,
    portraits: Res<PortraitAssets>,
    story_catalog: Res<StoryCatalog>,
) {
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
                    BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.85)),
                ))
                .with_children(|top| {
                    top.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 12.2,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.86, 0.92)),
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

                        mp.spawn(Node {
                            column_gap: Val::Px(8.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|jrow| {
                            spawn_mp_btn(jrow, "[Enter] Join by Code", MpButtonAction::JoinTyped);
                            spawn_mp_btn(
                                jrow,
                                "Quick Join Same PC (LOCAL)",
                                MpButtonAction::JoinLocal,
                            );
                        });
                    });

                    // 3. Interactive Cypher Decoder Modal
                    mid.spawn((
                        Node {
                            width: Val::Percent(92.0),
                            max_height: Val::Percent(90.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.0)),
                            row_gap: Val::Px(10.0),
                            border: UiRect::all(Val::Px(2.0)),
                            overflow: Overflow::scroll_y(),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.06, 0.09, 0.98)),
                        BorderColor(Color::srgb(0.88, 0.72, 0.36)),
                        CypherModalRoot,
                    ))
                    .with_children(|c_modal| {
                        c_modal.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(9.0),
                                ..default()
                            },
                            CypherModalContent,
                        ));
                    });
                });

                // Minimal 1-line bottom prompt + Character Select & Co-op buttons
                left.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.85)),
                ))
                .with_children(|bot| {
                    bot.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 12.2,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.82, 0.78)),
                        BottomPromptText,
                    ));

                    bot.spawn(Node {
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|btns| {
                        btns.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.14, 0.10)),
                            BorderColor(Color::srgb(0.84, 0.68, 0.38)),
                            OpenStoryBarBtn,
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("[V] Story"),
                                TextFont {
                                    font_size: 11.8,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.88, 0.62)),
                                OpenStoryBarBtnText,
                            ));
                        });

                        btns.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.13, 0.20)),
                            BorderColor(Color::srgb(0.76, 0.56, 0.36)),
                            OpenCharBarBtn,
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("[C] Detective"),
                                TextFont {
                                    font_size: 11.8,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.96, 0.86, 0.64)),
                                OpenCharBarBtnText,
                            ));
                        });

                        btns.spawn((
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
                                    font_size: 11.8,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.82, 0.90, 0.98)),
                                OpenCoopBarBtnText,
                            ));
                        });
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

            // ================= FULL-SCREEN STORY & CHARACTER SELECTION OVERLAYS =================
            build_story_select_overlay(root, &story_catalog);
            build_character_select_overlay(root, &portraits);
        });
}

fn build_story_select_overlay(root: &mut ChildBuilder, story_catalog: &StoryCatalog) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(Val::Px(24.0)),
            display: Display::Flex,
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.025, 0.045, 0.95)),
        ZIndex(35),
        StorySelectModalRoot,
    ))
    .with_children(|backdrop| {
        backdrop
            .spawn((
                Node {
                    width: Val::Percent(92.0),
                    max_width: Val::Px(1120.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(24.0)),
                    row_gap: Val::Px(16.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.07, 0.10, 0.99)),
                BorderColor(Color::srgb(0.84, 0.66, 0.38)),
            ))
            .with_children(|panel| {
                // Header Row
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|titles| {
                            titles.spawn((
                                Text::new("STEP 1 — SELECT A NOIR STORY TO PLAY OR HOST"),
                                TextFont {
                                    font_size: 18.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.96, 0.84, 0.54)),
                            ));
                            titles.spawn((
                                Text::new(
                                    "Pick a story (click or press 1–3) to load its SceneCatalog & Case File, then choose your Detective. Press [V] anytime to switch stories.",
                                ),
                                TextFont {
                                    font_size: 12.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.76, 0.78, 0.86)),
                            ));
                        });

                        hdr.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(13.0), Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.16, 0.16, 0.22)),
                            BorderColor(Color::srgb(0.46, 0.50, 0.62)),
                            CloseStorySelectBtn,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("[Esc] Resume Active Case"),
                                TextFont {
                                    font_size: 12.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.86, 0.88, 0.94)),
                            ));
                        });
                    });

                // Story Cards Row
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(16.0),
                        align_items: AlignItems::Stretch,
                        ..default()
                    })
                    .with_children(|row| {
                        for (idx, story) in story_catalog.stories.iter().enumerate() {
                            spawn_story_select_card(row, story, idx + 1);
                        }
                    });
            });
    });
}

fn spawn_story_select_card(parent: &mut ChildBuilder, story: &StorySummary, num: usize) {
    let accent = Color::srgb(story.accent_rgb.0, story.accent_rgb.1, story.accent_rgb.2);
    let scenes_joined = story.scene_titles.join("   •   ");

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(50.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.10, 0.14)),
            BorderColor(accent.with_alpha(0.6)),
            StorySelectCard {
                story_id: story.id.clone(),
                accent_rgb: story.accent_rgb,
            },
        ))
        .with_children(|card| {
            card.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    Text::new(format!("[{num}]  {}", story.story_name.to_uppercase())),
                    TextFont {
                        font_size: 17.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.97, 0.94, 0.88)),
                ));

                let cypher_meta = if story.cypher_count > 0 {
                    format!(
                        "   •   {} Cypher{}",
                        story.cypher_count,
                        if story.cypher_count == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                };
                top.spawn((
                    Text::new(format!(
                        "{}   |   {} Scenes   •   {} Clues{}",
                        story.setting_line,
                        story.scene_titles.len(),
                        story.clue_count,
                        cypher_meta
                    )),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(accent),
                ));

                top.spawn((
                    Text::new(story.synopsis.clone()),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.85, 0.90)),
                ));

                // SceneCatalog manifest box
                top.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(11.0), Val::Px(8.0)),
                        row_gap: Val::Px(4.0),
                        border: UiRect::left(Val::Px(2.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
                    BorderColor(accent),
                ))
                .with_children(|sc_box| {
                    sc_box.spawn((
                        Text::new(format!(
                            "SCENE CATALOG — \"{}\"",
                            story.story_name
                        )),
                        TextFont {
                            font_size: 11.2,
                            ..default()
                        },
                        TextColor(Color::srgb(0.94, 0.82, 0.54)),
                    ));
                    sc_box.spawn((
                        Text::new(scenes_joined),
                        TextFont {
                            font_size: 11.8,
                            ..default()
                        },
                        TextColor(Color::srgb(0.78, 0.80, 0.88)),
                    ));
                });
            });

            card.spawn((
                Text::new(format!(
                    "Click or Press [{num}] to Load SceneCatalog & Pick Detective →"
                )),
                TextFont {
                    font_size: 12.5,
                    ..default()
                },
                TextColor(accent),
                StorySelectCardStatusText {
                    story_id: story.id.clone(),
                    index_one_based: num,
                },
            ));
        });
}

fn build_character_select_overlay(root: &mut ChildBuilder, portraits: &PortraitAssets) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(Val::Px(24.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.03, 0.05, 0.92)),
        ZIndex(30),
        CharSelectModalRoot,
    ))
    .with_children(|backdrop| {
        backdrop
            .spawn((
                Node {
                    width: Val::Percent(92.0),
                    max_width: Val::Px(1120.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(22.0)),
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.07, 0.10, 0.99)),
                BorderColor(Color::srgb(0.76, 0.60, 0.36)),
            ))
            .with_children(|panel| {
                // Header row
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(3.0),
                            ..default()
                        })
                        .with_children(|titles| {
                            titles.spawn((
                                Text::new("STEP 2 — SELECT YOUR SUPERNATURAL DETECTIVE"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.96, 0.84, 0.54)),
                                CharSelectStoryBannerText,
                            ));
                            titles.spawn((
                                Text::new(
                                    "Click a detective (or press 1–4 / F1–F4) to take the case. Press [C] anytime to switch.",
                                ),
                                TextFont {
                                    font_size: 12.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.74, 0.76, 0.84)),
                            ));
                        });

                        hdr.spawn(Node {
                            column_gap: Val::Px(8.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|actions| {
                            actions
                                .spawn((
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                        border: UiRect::all(Val::Px(1.5)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.18, 0.14, 0.12)),
                                    BorderColor(Color::srgb(0.82, 0.64, 0.38)),
                                    BackToStorySelectBtn,
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        Text::new("← [V] Stories"),
                                        TextFont {
                                            font_size: 12.8,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.98, 0.88, 0.66)),
                                    ));
                                });

                            actions
                                .spawn((
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                        border: UiRect::all(Val::Px(1.5)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.18, 0.24, 0.18)),
                                    BorderColor(Color::srgb(0.52, 0.84, 0.62)),
                                    CloseCharSelectBtn,
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        Text::new("[Enter / Esc] Begin Investigation"),
                                        TextFont {
                                            font_size: 13.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.88, 0.98, 0.90)),
                                    ));
                                });
                        });
                    });

                // 2x2 Grid of Detective Cards (2 rows of 2 cards)
                let rows = [
                    [DetectiveId::Richey, DetectiveId::LuckerAndRon],
                    [DetectiveId::Pinckney, DetectiveId::Zombie],
                ];

                for pair in rows {
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(14.0),
                            ..default()
                        })
                        .with_children(|row| {
                            for (offset, det) in pair.iter().copied().enumerate() {
                                let num = match det {
                                    DetectiveId::Richey => 1,
                                    DetectiveId::LuckerAndRon => 2,
                                    DetectiveId::Pinckney => 3,
                                    DetectiveId::Zombie => 4,
                                };
                                let _ = offset;
                                spawn_detective_select_card(row, det, num, portraits);
                            }
                        });
                }
            });
    });
}

fn spawn_detective_select_card(
    parent: &mut ChildBuilder,
    det: DetectiveId,
    num: usize,
    portraits: &PortraitAssets,
) {
    let accent = det.accent_color();
    let (ded, san, noc, pul) = det.base_stats();

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(50.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(9.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.10, 0.14)),
            BorderColor(accent.with_alpha(0.55)),
            CharSelectCard { character: det },
        ))
        .with_children(|card| {
            // Top Row: Portrait + Name + Species Badge
            card.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|top| {
                if let Some(port) = portraits.for_detective(det) {
                    top.spawn((
                        ImageNode::new(port),
                        Node {
                            width: Val::Px(78.0),
                            height: Val::Px(78.0),
                            flex_shrink: 0.0,
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BorderColor(accent),
                    ));
                }

                top.spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|info| {
                    info.spawn((
                        Text::new(format!(
                            "[{num} / {}]  {}",
                            det.hotkey_label(),
                            det.display_name()
                        )),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.94, 0.90)),
                    ));
                    info.spawn((
                        Text::new(det.species_badge()),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(accent),
                    ));
                    info.spawn((
                        Text::new(det.bio()),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.78, 0.80, 0.86)),
                    ));
                });
            });

            // Stats Strip
            card.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
            ))
            .with_children(|st| {
                st.spawn((
                    Text::new(format!(
                        "{} {ded}   •   {} {san}   •   {} {noc}   •   {} {pul}   |   Sense: {}",
                        Faculty::Deduction.code_for(det),
                        Faculty::Sanguine.code_for(det),
                        Faculty::Nocturne.code_for(det),
                        Faculty::Pulse.code_for(det),
                        det.sense_name()
                    )),
                    TextFont {
                        font_size: 11.8,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.86, 0.76)),
                ));
            });

            // Special Abilities Summary
            card.spawn((
                Text::new(det.abilities_summary()),
                TextFont {
                    font_size: 11.8,
                    ..default()
                },
                TextColor(Color::srgb(0.84, 0.86, 0.92)),
            ));

            // Footer Selection Status
            card.spawn((
                Text::new("Click to Select & Investigate"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(accent),
                CharSelectCardStatusText { character: det },
            ));
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

fn apply_selected_character(sheet: &mut DetectiveSheet, chosen: DetectiveId) {
    let current_strain = sheet.hunger;
    let current_cypher_bonus = sheet.cypher_bonus;
    *sheet = DetectiveSheet::from_character(chosen);
    sheet.hunger = current_strain;
    sheet.cypher_bonus = current_cypher_bonus;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_hud_shortcuts_and_coop_typing(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    (story_catalog, active_story): (Res<StoryCatalog>, Res<ActiveStory>),
    (mut story_open, mut ledger_open, mut char_open, mut mp_open, mut cypher_modal): (
        ResMut<StorySelectModalOpen>,
        ResMut<LedgerModalOpen>,
        ResMut<CharacterSelectModalOpen>,
        ResMut<MultiplayerModalOpen>,
        Option<ResMut<ActiveCypherModal>>,
    ),
    mut sheet: ResMut<DetectiveSheet>,
    mut nar_state: ResMut<NarrativeState>,
    (mut join_writer, mut story_writer, mut submit_cypher_writer): (
        EventWriter<JoinGameEvent>,
        EventWriter<SelectStoryEvent>,
        EventWriter<SubmitCypherGuessEvent>,
    ),
    mut bar_btns: ParamSet<(
        Query<&Interaction, (Changed<Interaction>, With<OpenStoryBarBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OpenCharBarBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OpenCoopBarBtn>)>,
    )>,
) {
    for inter in &bar_btns.p0() {
        if *inter == Interaction::Pressed {
            story_open.0 = !story_open.0;
            if story_open.0 {
                char_open.0 = false;
                mp_open.open = false;
                ledger_open.0 = false;
                if let Some(ref mut cm) = cypher_modal {
                    cm.close();
                }
            }
        }
    }

    for inter in &bar_btns.p1() {
        if *inter == Interaction::Pressed {
            char_open.0 = !char_open.0;
            if char_open.0 {
                story_open.0 = false;
                mp_open.open = false;
                ledger_open.0 = false;
                if let Some(ref mut cm) = cypher_modal {
                    cm.close();
                }
            }
        }
    }

    for inter in &bar_btns.p2() {
        if *inter == Interaction::Pressed {
            mp_open.open = !mp_open.open;
            if mp_open.open {
                story_open.0 = false;
                char_open.0 = false;
                ledger_open.0 = false;
                if let Some(ref mut cm) = cypher_modal {
                    cm.close();
                }
            }
        }
    }

    if let Some(ref mut cm) = cypher_modal {
        if cm.open {
            let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
            for ev in key_events.read() {
                if ev.state != ButtonState::Pressed {
                    continue;
                }
                match &ev.logical_key {
                    Key::Escape => {
                        cm.close();
                    }
                    Key::Backspace => {
                        cm.typed_input.pop();
                        cm.revision += 1;
                    }
                    Key::Enter => {
                        submit_cypher_writer.send(SubmitCypherGuessEvent);
                    }
                    Key::Space => {
                        if !cm.typed_input.ends_with(' ') && cm.typed_input.len() < 36 {
                            cm.typed_input.push(' ');
                            cm.revision += 1;
                        }
                    }
                    Key::ArrowLeft => {
                        cm.dial_shift = (cm.dial_shift + 25) % 26;
                        cm.revision += 1;
                    }
                    Key::ArrowRight => {
                        cm.dial_shift = (cm.dial_shift + 1) % 26;
                        cm.revision += 1;
                    }
                    Key::Character(s) => {
                        if !ctrl {
                            for ch in s.chars() {
                                if ch == '?' {
                                    cm.show_extra_tip = !cm.show_extra_tip;
                                    cm.revision += 1;
                                } else if (ch.is_ascii_alphanumeric() || ch == ' ')
                                    && cm.typed_input.len() < 36
                                {
                                    cm.typed_input.push(ch.to_ascii_uppercase());
                                    cm.revision += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            return;
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

    if story_open.0 {
        if keys.just_pressed(KeyCode::Escape) {
            story_open.0 = false;
            return;
        }
        if keys.just_pressed(KeyCode::Enter) {
            story_writer.send(SelectStoryEvent::pick_and_choose_character(
                active_story.story_id.clone(),
            ));
            return;
        }
        let digit_keys = [
            (KeyCode::Digit1, 0usize),
            (KeyCode::Digit2, 1),
            (KeyCode::Digit3, 2),
            (KeyCode::Digit4, 3),
        ];
        for (kc, idx) in digit_keys {
            if keys.just_pressed(kc) {
                if let Some(st) = story_catalog.stories.get(idx) {
                    story_writer.send(SelectStoryEvent::pick_and_choose_character(st.id.clone()));
                    return;
                }
            }
        }
        return;
    }

    if char_open.0 {
        if keys.just_pressed(KeyCode::KeyV) {
            char_open.0 = false;
            story_open.0 = true;
            return;
        }
        if keys.just_pressed(KeyCode::Escape)
            || keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::KeyC)
        {
            char_open.0 = false;
            return;
        }
        let modal_pick = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::F1) {
            Some(DetectiveId::Richey)
        } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::F2) {
            Some(DetectiveId::LuckerAndRon)
        } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::F3) {
            Some(DetectiveId::Pinckney)
        } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::F4) {
            Some(DetectiveId::Zombie)
        } else {
            None
        };
        if let Some(chosen) = modal_pick {
            apply_selected_character(&mut sheet, chosen);
            nar_state.generation += 1;
            char_open.0 = false;
            return;
        }
        return;
    }

    if keys.just_pressed(KeyCode::KeyV) {
        story_open.0 = true;
        char_open.0 = false;
        ledger_open.0 = false;
        mp_open.open = false;
        return;
    }

    if keys.just_pressed(KeyCode::KeyC) {
        char_open.0 = !char_open.0;
        if char_open.0 {
            story_open.0 = false;
            ledger_open.0 = false;
            mp_open.open = false;
        }
        return;
    }

    if keys.just_pressed(KeyCode::KeyM) {
        mp_open.open = true;
        story_open.0 = false;
        char_open.0 = false;
        ledger_open.0 = false;
        return;
    }

    if keys.just_pressed(KeyCode::KeyL) {
        ledger_open.0 = !ledger_open.0;
    }

    let chosen_fn = if keys.just_pressed(KeyCode::F1) {
        Some(DetectiveId::Richey)
    } else if keys.just_pressed(KeyCode::F2) {
        Some(DetectiveId::LuckerAndRon)
    } else if keys.just_pressed(KeyCode::F3) {
        Some(DetectiveId::Pinckney)
    } else if keys.just_pressed(KeyCode::F4) {
        Some(DetectiveId::Zombie)
    } else {
        None
    };

    if let Some(chosen) = chosen_fn {
        apply_selected_character(&mut sheet, chosen);
        nar_state.generation += 1;
    }
}

#[allow(clippy::type_complexity)]
fn handle_story_select_interactions(
    mut interactions: ParamSet<(
        Query<(&Interaction, &StorySelectCard, &mut BackgroundColor), Changed<Interaction>>,
        Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<CloseStorySelectBtn>)>,
        Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<BackToStorySelectBtn>)>,
    )>,
    mut story_open: ResMut<StorySelectModalOpen>,
    mut char_open: ResMut<CharacterSelectModalOpen>,
    mut story_writer: EventWriter<SelectStoryEvent>,
) {
    let mut picked_story: Option<String> = None;
    for (inter, card, mut bg) in &mut interactions.p0() {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.20, 0.18, 0.28);
                picked_story = Some(card.story_id.clone());
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.15, 0.15, 0.21);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.10, 0.10, 0.14);
            }
        }
    }
    if let Some(story_id) = picked_story {
        story_writer.send(SelectStoryEvent::pick_and_choose_character(story_id));
    }

    for (inter, mut bg) in &mut interactions.p1() {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.24, 0.24, 0.34);
                story_open.0 = false;
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.20, 0.20, 0.28);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.16, 0.16, 0.22);
            }
        }
    }

    for (inter, mut bg) in &mut interactions.p2() {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.28, 0.22, 0.16);
                char_open.0 = false;
                story_open.0 = true;
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.24, 0.19, 0.15);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.18, 0.14, 0.12);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_story_select_modal_ui(
    story_open: Res<StorySelectModalOpen>,
    active_story: Res<ActiveStory>,
    active_scene: Res<ActiveScene>,
    mut modal_q: Query<&mut Node, With<StorySelectModalRoot>>,
    mut cards_q: Query<(&StorySelectCard, &mut BorderColor)>,
    mut text_set: ParamSet<(
        Query<(&StorySelectCardStatusText, &mut Text)>,
        Query<&mut Text, With<OpenStoryBarBtnText>>,
        Query<&mut Text, With<CharSelectStoryBannerText>>,
    )>,
) {
    if let Ok(mut node) = modal_q.get_single_mut() {
        node.display = if story_open.0 {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (card, mut border) in &mut cards_q {
        let accent = Color::srgb(card.accent_rgb.0, card.accent_rgb.1, card.accent_rgb.2);
        border.0 = if card.story_id == active_story.story_id {
            accent
        } else {
            accent.with_alpha(0.35)
        };
    }

    for (st, mut txt) in &mut text_set.p0() {
        let n = st.index_one_based;
        txt.0 = if st.story_id == active_story.story_id {
            format!("★ LOADED STORY — Click or Press [{n} / Enter] to Pick Detective →")
        } else {
            format!("Click or Press [{n}] to Load SceneCatalog & Pick Detective →")
        };
    }

    let story_label = if !active_scene.story_name.is_empty() {
        active_scene.story_name.as_str()
    } else {
        active_story.story_name.as_str()
    };

    if let Ok(mut bar_txt) = text_set.p1().get_single_mut() {
        bar_txt.0 = format!("[V] {story_label}");
    }

    if let Ok(mut banner_txt) = text_set.p2().get_single_mut() {
        banner_txt.0 = format!(
            "STEP 2 — SELECT YOUR SUPERNATURAL DETECTIVE  •  Story: {story_label}"
        );
    }
}

#[allow(clippy::type_complexity)]
fn handle_char_select_interactions(
    mut card_q: Query<
        (&Interaction, &CharSelectCard, &mut BackgroundColor),
        (Changed<Interaction>, Without<CloseCharSelectBtn>),
    >,
    mut close_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<CloseCharSelectBtn>, Without<CharSelectCard>),
    >,
    mut char_open: ResMut<CharacterSelectModalOpen>,
    mut sheet: ResMut<DetectiveSheet>,
    mut nar_state: ResMut<NarrativeState>,
) {
    for (inter, card, mut bg) in &mut card_q {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.20, 0.18, 0.28);
                apply_selected_character(&mut sheet, card.character);
                nar_state.generation += 1;
                char_open.0 = false;
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.15, 0.15, 0.21);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.10, 0.10, 0.14);
            }
        }
    }

    for (inter, mut bg) in &mut close_q {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.26, 0.38, 0.26);
                char_open.0 = false;
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.22, 0.30, 0.22);
            }
            Interaction::None => {
                bg.0 = Color::srgb(0.18, 0.24, 0.18);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_char_select_modal_ui(
    char_open: Res<CharacterSelectModalOpen>,
    sheet: Res<DetectiveSheet>,
    mut modal_q: Query<&mut Node, With<CharSelectModalRoot>>,
    mut cards_q: Query<(&CharSelectCard, &mut BorderColor)>,
    mut status_q: Query<(&CharSelectCardStatusText, &mut Text), Without<OpenCharBarBtnText>>,
    mut bar_btn_q: Query<&mut Text, (With<OpenCharBarBtnText>, Without<CharSelectCardStatusText>)>,
) {
    if let Ok(mut node) = modal_q.get_single_mut() {
        node.display = if char_open.0 {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (card, mut border) in &mut cards_q {
        let accent = card.character.accent_color();
        border.0 = if card.character == sheet.character {
            accent
        } else {
            accent.with_alpha(0.35)
        };
    }

    for (st, mut txt) in &mut status_q {
        txt.0 = if st.character == sheet.character {
            "★ ACTIVE DETECTIVE — Click or Press [Enter] to Play".to_string()
        } else {
            "Click to Select & Investigate".to_string()
        };
    }

    if let Ok(mut btxt) = bar_btn_q.get_single_mut() {
        btxt.0 = format!("[C] {}", sheet.character.display_name());
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

#[allow(clippy::type_complexity)]
fn update_mp_modal_ui(
    mp_open: Res<MultiplayerModalOpen>,
    session: Res<NetSession>,
    mut modal_q: Query<&mut Node, With<MpModalRoot>>,
    mut status_q: Query<
        &mut Text,
        (
            With<MpStatusText>,
            Without<MpInputText>,
            Without<OpenCoopBarBtnText>,
        ),
    >,
    mut input_q: Query<
        &mut Text,
        (
            With<MpInputText>,
            Without<MpStatusText>,
            Without<OpenCoopBarBtnText>,
        ),
    >,
    mut bar_btn_q: Query<
        &mut Text,
        (
            With<OpenCoopBarBtnText>,
            Without<MpStatusText>,
            Without<MpInputText>,
        ),
    >,
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
            NetRole::Hosting => format!(
                "[M] Code: {} ({})",
                session.invite_code,
                session.peers.len() + 1
            ),
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
        let sight_tag = if blood_sight.active {
            format!("  •  {} ON", sheet.character.sense_name().to_uppercase())
        } else {
            String::new()
        };
        let cypher_tag = if sheet.cypher_bonus > 0 {
            format!("  •  CYPHER +{}", sheet.cypher_bonus)
        } else {
            String::new()
        };
        let loc_title = match (
            active_scene.story_name.is_empty(),
            active_scene.current_title.is_empty(),
        ) {
            (false, false) => format!("{} ({})", active_scene.current_title, active_scene.story_name),
            (true, false) => active_scene.current_title.clone(),
            _ => library.title.clone(),
        };
        txt.0 = format!(
            "{}  |  {}  |  {} {}  {} {}  {} {}  {} {}  |  {} {}{}{}",
            sheet.character.display_name(),
            loc_title,
            Faculty::Deduction.code_for(sheet.character),
            sheet.effective_stat(Faculty::Deduction),
            Faculty::Sanguine.code_for(sheet.character),
            sheet.effective_stat(Faculty::Sanguine),
            Faculty::Nocturne.code_for(sheet.character),
            sheet.effective_stat(Faculty::Nocturne),
            Faculty::Pulse.code_for(sheet.character),
            sheet.effective_stat(Faculty::Pulse),
            sheet.character.strain_label(),
            bar,
            cypher_tag,
            sight_tag,
        );
    }

    if let Ok(mut txt) = prompt_txt.get_single_mut() {
        if let Some(lbl) = &nearby.label {
            txt.0 = format!("Click / [E] {lbl}");
        } else {
            let cypher_status = if library.cypher_order.is_empty() {
                String::new()
            } else {
                format!(
                    "  •  Cyphers ({}/{})",
                    ledger.solved_cyphers.len(),
                    library.cypher_order.len()
                )
            };
            txt.0 = format!(
                "Click to Walk  •  Tab: {}  •  L: Clues ({}/{}){}",
                sheet.character.sense_name(),
                ledger.discovered.len(),
                library.clue_order.len(),
                cypher_status
            );
        }
    }
}

fn spawn_cypher_btn(
    parent: &mut ChildBuilder,
    label: &str,
    action: CypherBtnAction,
    bg_rgb: (f32, f32, f32),
    border_rgb: (f32, f32, f32),
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(bg_rgb.0, bg_rgb.1, bg_rgb.2)),
            BorderColor(Color::srgb(border_rgb.0, border_rgb.1, border_rgb.2)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: 11.8,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.93, 0.88)),
            ));
        });
}

fn handle_cypher_modal_buttons(
    mut interactions: Query<
        (&Interaction, &CypherBtnAction, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut cypher_modal: Option<ResMut<ActiveCypherModal>>,
    mut submit_writer: EventWriter<SubmitCypherGuessEvent>,
) {
    let Some(ref mut cm) = cypher_modal else {
        return;
    };
    if !cm.open {
        return;
    }

    for (inter, action, mut bg) in &mut interactions {
        match *inter {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.32, 0.26, 0.16);
                match *action {
                    CypherBtnAction::Close => {
                        cm.close();
                    }
                    CypherBtnAction::ShiftLeft => {
                        cm.dial_shift = (cm.dial_shift + 25) % 26;
                        cm.revision += 1;
                    }
                    CypherBtnAction::ShiftRight => {
                        cm.dial_shift = (cm.dial_shift + 1) % 26;
                        cm.revision += 1;
                    }
                    CypherBtnAction::ToggleTip => {
                        cm.show_extra_tip = !cm.show_extra_tip;
                        cm.revision += 1;
                    }
                    CypherBtnAction::TypeChar(ch) => {
                        if cm.typed_input.len() < 36 {
                            cm.typed_input.push(ch);
                            cm.revision += 1;
                        }
                    }
                    CypherBtnAction::Space => {
                        if !cm.typed_input.ends_with(' ') && cm.typed_input.len() < 36 {
                            cm.typed_input.push(' ');
                            cm.revision += 1;
                        }
                    }
                    CypherBtnAction::Backspace => {
                        cm.typed_input.pop();
                        cm.revision += 1;
                    }
                    CypherBtnAction::Clear => {
                        cm.typed_input.clear();
                        cm.revision += 1;
                    }
                    CypherBtnAction::Submit => {
                        submit_writer.send(SubmitCypherGuessEvent);
                    }
                }
            }
            Interaction::Hovered => {
                bg.0 = Color::srgb(0.24, 0.21, 0.18);
            }
            Interaction::None => {
                bg.0 = match action {
                    CypherBtnAction::Submit => Color::srgb(0.16, 0.26, 0.18),
                    CypherBtnAction::Close => Color::srgb(0.20, 0.14, 0.16),
                    _ => Color::srgb(0.14, 0.14, 0.19),
                };
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_cypher_modal_ui(
    mut commands: Commands,
    cypher_modal: Option<Res<ActiveCypherModal>>,
    library: Res<CaseLibrary>,
    ledger: Res<ClueLedger>,
    mut root_q: Query<&mut Node, With<CypherModalRoot>>,
    content_q: Query<(Entity, Option<&Children>), With<CypherModalContent>>,
    mut last_rev: Local<u64>,
    mut last_open: Local<bool>,
) {
    let Some(cm) = cypher_modal else {
        return;
    };

    if let Ok(mut root_node) = root_q.get_single_mut() {
        root_node.display = if cm.open {
            Display::Flex
        } else {
            Display::None
        };
    }

    if cm.revision == *last_rev && cm.open == *last_open {
        return;
    }
    *last_rev = cm.revision;
    *last_open = cm.open;

    let Ok((content_entity, children)) = content_q.get_single() else {
        return;
    };
    if let Some(children) = children {
        let list: Vec<Entity> = children.iter().copied().collect();
        for ch in list {
            commands.entity(ch).despawn_recursive();
        }
    }

    if !cm.open {
        return;
    }
    let Some(def) = library.cyphers.get(&cm.cypher_id) else {
        return;
    };

    let already_solved = ledger.is_cypher_solved(&def.id);
    let cipher_txt = def.ciphertext();
    let pattern_hint = def.word_pattern_hint();
    let extra_tip = def.detective_tip();

    commands.entity(content_entity).with_children(|col| {
        // 1. Header Bar
        col.spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|hdr| {
            hdr.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|titles| {
                titles.spawn((
                    Text::new(format!("🔐 {}", def.title.to_uppercase())),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.86, 0.48)),
                ));
                titles.spawn((
                    Text::new(format!(
                        "{}   •   REWARD: +STRONG CLUE & +{} ALL DICE ROLLS",
                        def.kind.badge_label(),
                        def.roll_bonus
                    )),
                    TextFont {
                        font_size: 11.2,
                        ..default()
                    },
                    TextColor(Color::srgb(0.62, 0.88, 0.74)),
                ));
            });

            hdr.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|btns| {
                spawn_cypher_btn(
                    btns,
                    "[?] Forensic Tip",
                    CypherBtnAction::ToggleTip,
                    (0.16, 0.16, 0.24),
                    (0.54, 0.62, 0.86),
                );
                spawn_cypher_btn(
                    btns,
                    "[Esc] Close",
                    CypherBtnAction::Close,
                    (0.20, 0.14, 0.16),
                    (0.68, 0.44, 0.48),
                );
            });
        });

        // 2. Scene Context & Margin Hint
        col.spawn((
            Text::new(def.flavor.clone()),
            TextFont {
                font_size: 12.2,
                ..default()
            },
            TextColor(Color::srgb(0.84, 0.83, 0.88)),
        ));
        col.spawn((
            Text::new(format!("Margin Note: {}", def.hint)),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::srgb(0.96, 0.78, 0.44)),
        ));
        if cm.show_extra_tip && !extra_tip.is_empty() {
            col.spawn((
                Text::new(extra_tip),
                TextFont {
                    font_size: 11.8,
                    ..default()
                },
                TextColor(Color::srgb(0.58, 0.86, 0.96)),
            ));
        }

        // 3. Encrypted Ciphertext Display Box
        col.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.035, 0.055)),
            BorderColor(Color::srgb(0.76, 0.60, 0.28)),
        ))
        .with_children(|cbox| {
            cbox.spawn((
                Text::new("INTERCEPTED CIPHERTEXT:"),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(Color::srgb(0.68, 0.66, 0.62)),
            ));
            cbox.spawn((
                Text::new(cipher_txt),
                TextFont {
                    font_size: 17.5,
                    ..default()
                },
                TextColor(Color::srgb(0.99, 0.88, 0.46)),
            ));
        });

        // 4. Interactive Key Aid (Caesar Wheel / A1Z26 / Atbash)
        col.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
            BorderColor(Color::srgb(0.34, 0.38, 0.50)),
        ))
        .with_children(|kbox| match def.kind {
            CypherKind::Caesar { .. } => {
                kbox.spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|drow| {
                    spawn_cypher_btn(
                        drow,
                        "◄ Dial -1",
                        CypherBtnAction::ShiftLeft,
                        (0.14, 0.16, 0.22),
                        (0.48, 0.58, 0.80),
                    );
                    drow.spawn((
                        Text::new(format!(
                            "CAESAR KEY WHEEL — Shift Offset: +{}  (◄/► or Arrow Keys)",
                            cm.dial_shift
                        )),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.76, 0.86, 0.98)),
                    ));
                    spawn_cypher_btn(
                        drow,
                        "Dial +1 ►",
                        CypherBtnAction::ShiftRight,
                        (0.14, 0.16, 0.22),
                        (0.48, 0.58, 0.80),
                    );
                });
                let (c_line, p_line) = format_caesar_wheel_lines(cm.dial_shift);
                kbox.spawn((
                    Text::new(c_line),
                    TextFont {
                        font_size: 11.4,
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.82, 0.46)),
                ));
                kbox.spawn((
                    Text::new(p_line),
                    TextFont {
                        font_size: 11.4,
                        ..default()
                    },
                    TextColor(Color::srgb(0.58, 0.94, 0.72)),
                ));
            }
            CypherKind::A1Z26 => {
                kbox.spawn((
                    Text::new("TELEGRAPH NUMBER CODE KEY (1 = A ... 26 = Z,  '/' separates words):"),
                    TextFont {
                        font_size: 11.2,
                        ..default()
                    },
                    TextColor(Color::srgb(0.76, 0.86, 0.98)),
                ));
                kbox.spawn((
                    Text::new("A=1  B=2  C=3  D=4  E=5  F=6  G=7  H=8  I=9  J=10  K=11  L=12  M=13"),
                    TextFont {
                        font_size: 11.4,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.90, 0.94)),
                ));
                kbox.spawn((
                    Text::new("N=14 O=15 P=16 Q=17 R=18 S=19 T=20 U=21 V=22 W=23 X=24 Y=25 Z=26"),
                    TextFont {
                        font_size: 11.4,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.90, 0.94)),
                ));
            }
            CypherKind::Atbash => {
                kbox.spawn((
                    Text::new("ALCHEMICAL MIRROR KEY (Each letter swaps with its opposite):"),
                    TextFont {
                        font_size: 11.2,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.76, 0.98)),
                ));
                kbox.spawn((
                    Text::new("A↔Z  B↔Y  C↔X  D↔W  E↔V  F↔U  G↔T  H↔S  I↔R  J↔Q  K↔P  L↔O  M↔N"),
                    TextFont {
                        font_size: 11.6,
                        ..default()
                    },
                    TextColor(Color::srgb(0.90, 0.88, 0.96)),
                ));
            }
        });

        // 5. Decoded Plaintext Input Box
        col.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.05, 0.07)),
            BorderColor(if already_solved {
                Color::srgb(0.42, 0.88, 0.60)
            } else {
                Color::srgb(0.48, 0.66, 0.88)
            }),
        ))
        .with_children(|ibox| {
            ibox.spawn((
                Text::new(format!("WORD PATTERN:  {pattern_hint}")),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.68, 0.74, 0.84)),
            ));
            let display_typed = if cm.typed_input.is_empty() {
                "Type decoded plaintext on keyboard or click letters below: _".to_string()
            } else if already_solved {
                format!("DECODED:  {}", cm.typed_input)
            } else {
                format!("DECODED:  {}_", cm.typed_input)
            };
            ibox.spawn((
                Text::new(display_typed),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(if already_solved {
                    Color::srgb(0.56, 0.96, 0.72)
                } else {
                    Color::srgb(0.95, 0.96, 0.99)
                }),
            ));
        });

        // 6. Clickable Typewriter Keypad & Submit Action Bar (when unsolved)
        if !already_solved {
            for row_chars in ["ABCDEFGHIJKLM", "NOPQRSTUVWXYZ"] {
                col.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|krow| {
                    for ch in row_chars.chars() {
                        let s = ch.to_string();
                        spawn_cypher_btn(
                            krow,
                            &s,
                            CypherBtnAction::TypeChar(ch),
                            (0.14, 0.14, 0.19),
                            (0.34, 0.36, 0.46),
                        );
                    }
                });
            }

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|arow| {
                arow.spawn(Node {
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|left_btns| {
                    spawn_cypher_btn(
                        left_btns,
                        "[Space]",
                        CypherBtnAction::Space,
                        (0.14, 0.14, 0.19),
                        (0.38, 0.42, 0.54),
                    );
                    spawn_cypher_btn(
                        left_btns,
                        "[⌫ Back]",
                        CypherBtnAction::Backspace,
                        (0.14, 0.14, 0.19),
                        (0.38, 0.42, 0.54),
                    );
                    spawn_cypher_btn(
                        left_btns,
                        "[Clear]",
                        CypherBtnAction::Clear,
                        (0.14, 0.14, 0.19),
                        (0.38, 0.42, 0.54),
                    );
                });

                spawn_cypher_btn(
                    arow,
                    "[Enter] Crack Cypher →",
                    CypherBtnAction::Submit,
                    (0.16, 0.28, 0.18),
                    (0.48, 0.88, 0.62),
                );
            });
        }

        // 7. Feedback / Cracked Reward Banner
        if let Some((msg, ok)) = &cm.feedback {
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(11.0), Val::Px(8.0)),
                    row_gap: Val::Px(5.0),
                    border: UiRect::left(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(if *ok {
                    Color::srgb(0.08, 0.18, 0.12)
                } else {
                    Color::srgb(0.20, 0.10, 0.11)
                }),
                BorderColor(if *ok {
                    Color::srgb(0.46, 0.92, 0.64)
                } else {
                    Color::srgb(0.92, 0.44, 0.42)
                }),
            ))
            .with_children(|fbox| {
                fbox.spawn((
                    Text::new(msg.clone()),
                    TextFont {
                        font_size: 12.6,
                        ..default()
                    },
                    TextColor(if *ok {
                        Color::srgb(0.64, 0.98, 0.76)
                    } else {
                        Color::srgb(0.96, 0.64, 0.60)
                    }),
                ));
                if *ok {
                    fbox.spawn((
                        Text::new(def.solved_text.clone()),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.92, 0.88)),
                    ));
                }
            });
        }
    });
}

#[allow(clippy::type_complexity)]
fn update_pulse_monitor_widget(
    nar_state: Res<NarrativeState>,
    sheet: Res<DetectiveSheet>,
    heartbeats: Query<&SuspectHeartbeat>,
    mut card_q: Query<&mut Node, (With<PulseMonitorCard>, Without<PulseMonitorBarFill>)>,
    mut text_q: Query<(&mut Text, &mut TextColor), With<PulseMonitorText>>,
    mut bar_q: Query<
        (&mut Node, &mut BackgroundColor),
        (With<PulseMonitorBarFill>, Without<PulseMonitorCard>),
    >,
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
            sheet.character.accent_color()
        } else {
            Color::srgb(0.48, 0.86, 0.72)
        };

        let intensity_pct = ((hb.current_bpm - 50.0) * 1.05).clamp(12.0, 99.0);
        let readout = match sheet.character {
            DetectiveId::Richey => {
                let state_word = if spike { "LIE / PANIC SPIKE" } else { "STEADY" };
                format!(
                    "♥ Blood Pulse: {} — {:.0} BPM (base {:.0}) [{state_word}]",
                    hb.display_name, hb.current_bpm, hb.resting_bpm
                )
            }
            DetectiveId::LuckerAndRon => {
                let state_word = if spike { "GUILT TREMOR" } else { "CALM AURA" };
                format!(
                    "◈ Spirit Aura: {} — {:.0}% Resonance [{state_word}]",
                    hb.display_name, intensity_pct
                )
            }
            DetectiveId::Pinckney => {
                let state_word = if spike { "FEAR-SWEAT SPIKE" } else { "CALM SCENT" };
                format!(
                    "◆ Lupine Scent: {} — {:.0}% Adrenaline [{state_word}]",
                    hb.display_name, intensity_pct
                )
            }
            DetectiveId::Zombie => {
                let state_word = if spike { "NERVE PANIC" } else { "STILL NERVES" };
                format!(
                    "❖ Mortis Read: {} — {:.0}% Nerve Tremor [{state_word}]",
                    hb.display_name, intensity_pct
                )
            }
        };

        if let Ok((mut t, mut c)) = text_q.get_single_mut() {
            t.0 = readout;
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
    let char_changed = tracker.last_character != Some(sheet.character);
    let bonus_changed = tracker.last_cypher_bonus != sheet.cypher_bonus;
    if nar_state.generation == tracker.last_generation && !char_changed && !bonus_changed {
        return;
    }
    tracker.last_generation = nar_state.generation;
    tracker.last_character = Some(sheet.character);
    tracker.last_cypher_bonus = sheet.cypher_bonus;

    let char_accent = sheet.character.accent_color();

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
                    Text::new(format!(
                        "{} on the scene — click anywhere to walk, or click a suspect / clue to inspect.",
                        sheet.character.display_name()
                    )),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.62, 0.64, 0.72)),
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
                        BorderColor(char_accent),
                    ))
                    .with_children(|me_row| {
                        if let Some(port) = portraits.for_detective(sheet.character) {
                            me_row.spawn((
                                ImageNode::new(port),
                                Node {
                                    width: Val::Px(64.0),
                                    height: Val::Px(64.0),
                                    flex_shrink: 0.0,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor(char_accent),
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
                                    Text::new(sheet.character.dialogue_you_header()),
                                    TextFont {
                                        font_size: 11.5,
                                        ..default()
                                    },
                                    TextColor(char_accent),
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
                    let fac_name = sc.faculty.name_for(sheet.character);
                    if resolved.locked_check_failed {
                        format!("{}. [{fac_name} — Failed] ", idx + 1)
                    } else {
                        let pct = sheet.success_chance_percent(sc.faculty, sc.dc);
                        if sheet.cypher_bonus > 0 {
                            format!(
                                "{}. [{fac_name} {pct}% (+{} Cypher)] ",
                                idx + 1,
                                sheet.cypher_bonus
                            )
                        } else {
                            format!("{}. [{fac_name} {pct}%] ", idx + 1)
                        }
                    }
                } else if resolved.raw.open_cypher.is_some() {
                    format!("{}. [CYPHER] ", idx + 1)
                } else {
                    format!("{}. ", idx + 1)
                };

                let text_color = if resolved.locked_check_failed {
                    Color::srgb(0.48, 0.45, 0.50)
                } else if resolved.raw.open_cypher.is_some() {
                    Color::srgb(0.98, 0.84, 0.44)
                } else if let Some(req_char) = resolved.raw.requires_character {
                    req_char.accent_color()
                } else if let Some(sc) = &resolved.raw.skill_check {
                    sc.faculty.color_for(sheet.character)
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
                        Text::new(format!("{prefix}{}", resolved.formatted_label)),
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
    story_open: Res<StorySelectModalOpen>,
    char_open: Res<CharacterSelectModalOpen>,
    cypher_modal: Option<Res<ActiveCypherModal>>,
    mut interactions: Query<
        (&Interaction, &ChoiceButtonIndex, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut writer: EventWriter<SelectChoiceEvent>,
) {
    if story_open.0 || char_open.0 || cypher_modal.as_deref().is_some_and(|m| m.open) {
        return;
    }
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

#[allow(clippy::too_many_arguments)]
fn sync_ledger_modal(
    mut commands: Commands,
    ledger_open: Res<LedgerModalOpen>,
    sheet: Res<DetectiveSheet>,
    ledger: Res<ClueLedger>,
    library: Res<CaseLibrary>,
    mut modal_root_q: Query<&mut Node, With<LedgerModalRoot>>,
    content_q: Query<(Entity, Option<&Children>), With<LedgerModalContent>>,
    mut last_count: Local<usize>,
    mut last_cypher_bonus: Local<i32>,
    mut last_char: Local<Option<DetectiveId>>,
    mut last_story_title: Local<String>,
    mut initialized: Local<bool>,
) {
    if let Ok(mut root_node) = modal_root_q.get_single_mut() {
        root_node.display = if ledger_open.0 {
            Display::Flex
        } else {
            Display::None
        };
    }

    if *initialized
        && *last_count == ledger.discovered.len()
        && *last_cypher_bonus == sheet.cypher_bonus
        && *last_char == Some(sheet.character)
        && *last_story_title == library.title
    {
        return;
    }
    *initialized = true;
    *last_count = ledger.discovered.len();
    *last_cypher_bonus = sheet.cypher_bonus;
    *last_char = Some(sheet.character);
    *last_story_title = library.title.clone();

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
        if sheet.cypher_bonus > 0 {
            col.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::left(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.18, 0.12)),
                BorderColor(Color::srgb(0.46, 0.92, 0.62)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(format!(
                        "★ DECODED CYPHER INSIGHT ACTIVE: +{} Bonus to All 2d6 Skill Checks ({}/{} Cyphers Cracked)",
                        sheet.cypher_bonus,
                        ledger.solved_cyphers.len(),
                        library.cypher_order.len().max(ledger.solved_cyphers.len())
                    )),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.64, 0.98, 0.76)),
                ));
            });
        }
        col.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|fac_box| {
            for fac in Faculty::ALL {
                fac_box.spawn((
                    Text::new(format!(
                        "• {}: {}",
                        fac.name_for(sheet.character),
                        fac.desc_for(sheet.character)
                    )),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(fac.color_for(sheet.character)),
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
