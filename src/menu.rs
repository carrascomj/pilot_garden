use bevy::{
    ecs::message::{MessageReader, MessageWriter},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    window::CursorOptions,
};

use crate::{
    Capsule,
    config::GameState,
    digging::{Collectible, OnHand},
    dodgy::{ArchAnimation, Dodgy},
    emoji_particles::SecretRevealed,
    world_timer::TimerComp,
};

pub struct GameMenu;

// colors for button interactions
const BUTTON_COLOR: Color = Color::srgb(1.0, 0.3, 0.9); // cyber pink
const HOVER_COLOR: Color = Color::srgb(0.3, 1.0, 0.9); // cyber blue
const PRESSED_COLOR: Color = Color::srgb(1.0, 1.0, 1.0); // white blue
const RESOLUTIONS: &[(u32, u32)] = &[
    (1280, 720),  // 16:9
    (1024, 768),  // 4:3
    (1600, 900),  // 16:9
    (1280, 960),  // 4:3
    (1920, 1080), // 16:9
    (1600, 1200), // 4:3
    (2560, 1440), // 16:9
    (1920, 1440), // 4:3
];

impl Plugin for GameMenu {
    fn build(&self, app: &mut App) {
        app.add_message::<ApplyResolution>()
            .init_resource::<Winner>()
            .add_systems(
                OnEnter(GameState::Menu),
                (spawn_game_menu, spawn_settings_menu),
            )
            .add_systems(Startup, spawn_exit_menu)
            .add_systems(Update, button_system)
            .add_systems(Update, toggle_escape_menus)
            .add_systems(Update, apply_resolution.run_if(in_state(GameState::Menu)))
            // will run even after GameState menu since it has to play the animation for awakening
            .add_systems(Last, update_time)
            .add_plugins(UiMaterialPlugin::<HibernationMaterial>::default());
    }
}

/// Attached to button entities to decide action on press.
#[derive(Component)]
pub enum ButtonAction {
    StartGame,
    ShowSettings,
    SettingsPrev,
    SettingsNext,
    SettingsAccept,
    Exit,
}

/// Marker for start menu.
#[derive(Component)]
struct StartMenu;
#[derive(Component)]
struct RemoveOnStart;

/// Settings menu markers to change the window resolution.
#[derive(Component)]
struct SettingsMenu;
#[derive(Component)]
struct ResolutionText;
#[derive(Component)]
struct SelectedResolution(usize);

#[derive(Resource, Default)]
pub struct Winner(pub bool);

fn spawn_game_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ui_materials: ResMut<Assets<HibernationMaterial>>,
    winner: Res<Winner>,
    mut secret_rev: ResMut<SecretRevealed>,
    mut cursor_options: Single<&mut CursorOptions>,
    collectibles: Query<Entity, (With<Collectible>, With<OnHand>)>,
    mut dodgers: Query<(&Dodgy, &mut Transform, &mut TimerComp), Without<Capsule>>,
) {
    // first, remove all onhand collectibles. Otherwise, the
    // player could just die with a full shovel and beat the game
    for ent in &collectibles {
        commands.entity(ent).despawn()
    }
    // reset persistent dodgers if they were above
    for (dodgy, mut trans, mut timer) in &mut dodgers {
        timer.0.pause();
        trans.translation = dodgy.init_pos;
    }
    secret_rev.0 = false;
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Block,
            ..default()
        },
        StartMenu,
        BackgroundColor(Color::BLACK), // fog colour
        children![
            (
                Node {
                    display: Display::Flex,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_self: AlignSelf::Center,
                    justify_self: JustifySelf::Center,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                MaterialNode(ui_materials.add(HibernationMaterial {
                    time: 0.0,
                    disolve_time: -1.0,
                })),
                children![(
                    Node {
                        width: Val::Px(350.0),
                        height: Val::Px(300.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    RemoveOnStart,
                    children![
                        (
                            Button,
                            ButtonAction::StartGame,
                            Node {
                                width: Val::Px(150.0),
                                height: Val::Px(65.0),
                                border: UiRect::all(Val::Px(5.0)),
                                // horizontally center child text
                                justify_content: JustifyContent::Center,
                                // vertically center child text
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(BUTTON_COLOR),
                            BoxShadow::new(
                                BUTTON_COLOR.with_alpha(0.2),
                                Val::Percent(0.),
                                Val::Percent(0.),
                                Val::Percent(3.0),
                                Val::Px(3.0),
                            ),
                            BackgroundColor(Color::NONE),
                            children![(
                                Text::new("START"),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                            )]
                        ),
                        (
                            Button,
                            ButtonAction::ShowSettings,
                            Node {
                                width: Val::Px(220.0),
                                height: Val::Px(65.0),
                                border: UiRect::all(Val::Px(5.0)),
                                // horizontally center child text
                                justify_content: JustifyContent::Center,
                                // vertically center child text
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(BUTTON_COLOR),
                            BoxShadow::new(
                                BUTTON_COLOR.with_alpha(0.2),
                                Val::Percent(0.),
                                Val::Percent(0.),
                                Val::Percent(3.0),
                                Val::Px(3.0),
                            ),
                            children![(
                                Text::new("SETTINGS"),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                            )]
                        ),
                        (
                            Button,
                            ButtonAction::Exit,
                            Node {
                                width: Val::Px(150.0),
                                height: Val::Px(65.0),
                                border: UiRect::all(Val::Px(5.0)),
                                // horizontally center child text
                                justify_content: JustifyContent::Center,
                                // vertically center child text
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(BUTTON_COLOR),
                            BoxShadow::new(
                                BUTTON_COLOR.with_alpha(0.2),
                                Val::Percent(0.),
                                Val::Percent(0.),
                                Val::Percent(3.0),
                                Val::Px(3.0),
                            ),
                            children![(
                                Text::new("EXIT"),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                            )]
                        )
                    ]
                ),]
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Vw(1.0),
                    bottom: Val::Vh(1.0),
                    ..default()
                },
                Text::new(if winner.0 {
                    "WINNER: TRUE"
                } else {
                    "WINNER: FALSE"
                }),
                TextFont {
                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(if winner.0 {
                    Color::srgb(0.2, 0.9, 0.2)
                } else {
                    Color::srgb(0.9, 0.2, 0.2)
                }),
                TextShadow::default(),
                RemoveOnStart,
            ),
        ],
    ));
    **cursor_options = CursorOptions {
        visible: true,
        grab_mode: bevy::window::CursorGrabMode::Locked,
        ..default()
    };
}

/// Maintaing material updated with time and trigger awakening
/// effect on shader when it comes the time: disolve time is made positive
/// when exiting [`GameState::Menu`] by [`eyes_wide_open`].
fn update_time(
    mut commands: Commands,
    time: Res<Time>,
    mut next_state: ResMut<NextState<GameState>>,
    mut ui_materials: ResMut<Assets<HibernationMaterial>>,
    mut state_menu: Single<(Entity, &mut BackgroundColor), With<StartMenu>>,
    capsule: Single<(Entity, &Transform), With<Capsule>>,
) {
    for (_, material) in ui_materials.iter_mut() {
        if material.disolve_time > 0. {
            let diff = material.time - material.disolve_time;
            // dissolve the background with the shader after the eyes open
            let alpha = 1. - (diff - 3.0).clamp(0., 5.) / 5.0;
            state_menu.1.0.set_alpha(alpha);
            if diff > 5.0 {
                next_state.set(GameState::Above);
                commands.entity(state_menu.0).despawn();
                //  slide the capsule into the floor
                let trans = capsule.1.translation;
                commands.entity(capsule.0).insert((
                    ArchAnimation {
                        init_pos: trans,
                        last_pos: trans - Vec3::Y * 10.,
                        peak_y: 5.,
                    },
                    TimerComp(Timer::from_seconds(2.0, TimerMode::Once)),
                ));
            }
        }
        material.time = time.elapsed_secs();
    }
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct HibernationMaterial {
    #[uniform(0)]
    time: f32,
    #[uniform(1)]
    disolve_time: f32,
}

const SHADER_PATH: &str = "shaders/hibernation_eyes_fog_ui.wgsl";

impl UiMaterial for HibernationMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }
}

fn button_system(
    mut commands: Commands,
    mut app_exit_events: MessageWriter<AppExit>,
    mut interaction_query: Populated<
        (
            &Interaction,
            &mut BoxShadow,
            &mut BorderColor,
            &Children,
            &ButtonAction,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut TextColor>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut ui_materials: ResMut<Assets<HibernationMaterial>>,
    to_rm_on_start: Query<Entity, With<RemoveOnStart>>,
    mut settings_q: Query<(Entity, &mut SelectedResolution, &mut Visibility), With<SettingsMenu>>,
    mut res_text_q: Query<&mut Text, With<ResolutionText>>,
    mut apply_res: MessageWriter<ApplyResolution>,
) {
    for (interaction, mut box_shadow, mut border_color, children, action) in
        interaction_query.iter_mut()
    {
        let mut text_color = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                box_shadow.0[0].color = PRESSED_COLOR.with_alpha(0.3).into();
                *border_color = PRESSED_COLOR.into();
                *text_color = PRESSED_COLOR.into();
                match action {
                    ButtonAction::StartGame => {
                        **cursor_options = CursorOptions {
                            visible: false,
                            grab_mode: bevy::window::CursorGrabMode::Locked,
                            ..default()
                        };
                        for (_, material) in ui_materials.iter_mut() {
                            material.disolve_time = material.time;
                        }
                        for to_rm in &to_rm_on_start {
                            commands.entity(to_rm).despawn();
                        }
                    }
                    ButtonAction::ShowSettings => {
                        if let Ok((_, _, mut vis)) = settings_q.single_mut() {
                            vis.toggle_visible_hidden();
                        }
                    }
                    ButtonAction::SettingsPrev => {
                        if let Ok((_, mut sel, _)) = settings_q.single_mut() {
                            let len = RESOLUTIONS.len();
                            sel.0 = (sel.0 + len - 1) % len;
                            if let Ok(mut t) = res_text_q.single_mut() {
                                *t = Text::new(format!(
                                    "{}x{}",
                                    RESOLUTIONS[sel.0].0, RESOLUTIONS[sel.0].1
                                ));
                            }
                        }
                    }
                    ButtonAction::SettingsNext => {
                        if let Ok((_, mut sel, _)) = settings_q.single_mut() {
                            sel.0 = (sel.0 + 1) % RESOLUTIONS.len();
                            if let Ok(mut t) = res_text_q.single_mut() {
                                *t = Text::new(format!(
                                    "{}x{}",
                                    RESOLUTIONS[sel.0].0, RESOLUTIONS[sel.0].1
                                ));
                            }
                        }
                    }
                    ButtonAction::SettingsAccept => {
                        if let Ok((_, sel, mut vis)) = settings_q.single_mut() {
                            let (w, h) = RESOLUTIONS[sel.0];
                            apply_res.write(ApplyResolution(UVec2::new(w, h)));
                            *vis = Visibility::Hidden;
                        }
                    }
                    ButtonAction::Exit => {
                        app_exit_events.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered => {
                box_shadow.0[0].color = HOVER_COLOR.with_alpha(0.3).into();
                *border_color = HOVER_COLOR.into();
                *text_color = HOVER_COLOR.into();
            }
            Interaction::None => {
                box_shadow.0[0].color = BUTTON_COLOR.with_alpha(0.3).into();
                *border_color = BUTTON_COLOR.into();
                *text_color = BUTTON_COLOR.into();
            }
        }
    }
}

#[derive(Component)]
struct ExitEscMenu;

fn spawn_exit_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Node {
            display: Display::Flex,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_self: AlignSelf::Center,
            justify_self: JustifySelf::Center,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ExitEscMenu,
        Visibility::Hidden,
        BackgroundColor(Color::BLACK.with_alpha(0.8)),
        children![(
            Node {
                width: Val::Px(300.0),
                height: Val::Px(250.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(BUTTON_COLOR.with_alpha(0.8)),
            Outline {
                width: Val::Px(6.),
                offset: Val::Px(6.),
                color: BUTTON_COLOR,
            },
            BoxShadow::new(
                BUTTON_COLOR.with_alpha(0.2),
                Val::Percent(0.),
                Val::Percent(0.),
                Val::Percent(3.0),
                Val::Px(3.0),
            ),
            children![
                (
                    Button,
                    ButtonAction::Exit,
                    Node {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(BUTTON_COLOR),
                    BoxShadow::new(
                        BUTTON_COLOR.with_alpha(0.2),
                        Val::Percent(0.),
                        Val::Percent(0.),
                        Val::Percent(3.0),
                        Val::Px(3.0),
                    ),
                    children![(
                        Text::new("EXIT"),
                        TextFont {
                            font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                            font_size: 33.0,
                            ..default()
                        },
                        TextColor(BUTTON_COLOR),
                        TextShadow::default(),
                    )]
                ),
                (
                    Text::new("<ESC> to hide menu"),
                    TextFont {
                        font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                        font_size: 28.0,
                        ..default()
                    },
                    TextLayout {
                        justify: Justify::Center,
                        ..default()
                    },
                    TextColor(BUTTON_COLOR),
                    TextShadow::default(),
                )
            ]
        ),],
    ));
}

/// Show/hide Exit menu or, if inside Settings menu, hide it.
fn toggle_escape_menus(
    mut cursor_options: Single<&mut CursorOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut exit_menu_query: Query<&mut Visibility, (With<ExitEscMenu>, Without<SettingsMenu>)>,
    mut settings_menu_query: Query<&mut Visibility, (With<SettingsMenu>, Without<ExitEscMenu>)>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        for mut exit_menu in &mut exit_menu_query {
            if cursor_options.visible && *exit_menu == Visibility::Hidden {
                // in a menu already, do not trigger Esc menu
                break;
            }
            let cursor_visible = match *exit_menu {
                Visibility::Visible => false,
                _ => true,
            };
            cursor_options.visible = cursor_visible;
            exit_menu.toggle_visible_hidden();
        }
        for mut settings_menu in &mut settings_menu_query {
            if let Visibility::Visible = *settings_menu {
                *settings_menu = Visibility::Hidden;
                return;
            }
        }
    }
}

/// Settings menu to choose the resolution.
fn spawn_settings_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    window: Single<&Window>,
) {
    // pick current / nearest-by-height
    let (w, h) = (
        window.resolution.width() as u32,
        window.resolution.height() as u32,
    );
    let mut idx = 0usize;
    let mut best = u32::MAX;
    for (i, &(rw, rh)) in RESOLUTIONS.iter().enumerate() {
        if rw == w && rh == h {
            idx = i;
            break;
        }
        let d = rh.abs_diff(h);
        if d < best {
            best = d;
            idx = i;
        }
    }
    let label = format!("{}x{}", RESOLUTIONS[idx].0, RESOLUTIONS[idx].1);

    commands.spawn((
        Node {
            display: Display::Flex,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        SettingsMenu,
        SelectedResolution(idx),
        Visibility::Hidden,
        BackgroundColor(Color::BLACK.with_alpha(0.8)),
        DespawnOnExit(GameState::Menu),
        ZIndex(10),
        children![(
            Node {
                width: Val::Px(360.0),
                height: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(BUTTON_COLOR.with_alpha(0.8)),
            Outline {
                width: Val::Px(6.0),
                offset: Val::Px(6.0),
                color: BUTTON_COLOR
            },
            BoxShadow::new(
                BUTTON_COLOR.with_alpha(0.2),
                Val::Percent(0.),
                Val::Percent(0.),
                Val::Percent(3.0),
                Val::Px(3.0),
            ),
            children![
                (
                    // row 1: <  [current res]  >
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(65.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    children![
                        (
                            Button,
                            ButtonAction::SettingsPrev,
                            Node {
                                width: Val::Px(65.0),
                                height: Val::Px(65.0),
                                border: UiRect::all(Val::Px(5.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(BUTTON_COLOR),
                            BoxShadow::new(
                                BUTTON_COLOR.with_alpha(0.2),
                                Val::Percent(0.),
                                Val::Percent(0.),
                                Val::Percent(3.0),
                                Val::Px(3.0),
                            ),
                            children![(
                                Text::new("<"),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                            )]
                        ),
                        (
                            Node {
                                width: Val::Px(160.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            children![(
                                Text::new(label),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 28.0,
                                    ..default()
                                },
                                TextLayout {
                                    justify: Justify::Center,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                                ResolutionText,
                            )]
                        ),
                        (
                            Button,
                            ButtonAction::SettingsNext,
                            Node {
                                width: Val::Px(65.0),
                                height: Val::Px(65.0),
                                border: UiRect::all(Val::Px(5.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(BUTTON_COLOR),
                            BoxShadow::new(
                                BUTTON_COLOR.with_alpha(0.2),
                                Val::Percent(0.),
                                Val::Percent(0.),
                                Val::Percent(3.0),
                                Val::Px(3.0),
                            ),
                            children![(
                                Text::new(">"),
                                TextFont {
                                    font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(BUTTON_COLOR),
                                TextShadow::default(),
                            )]
                        )
                    ]
                ),
                (
                    // row 2: ACCEPT (smaller)
                    Button,
                    ButtonAction::SettingsAccept,
                    Node {
                        width: Val::Px(150.0),
                        height: Val::Px(45.0),
                        border: UiRect::all(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(BUTTON_COLOR),
                    BoxShadow::new(
                        BUTTON_COLOR.with_alpha(0.2),
                        Val::Percent(0.),
                        Val::Percent(0.),
                        Val::Percent(3.0),
                        Val::Px(3.0),
                    ),
                    children![(
                        Text::new("ACCEPT"),
                        TextFont {
                            font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(BUTTON_COLOR),
                        TextShadow::default(),
                    )]
                )
            ]
        )],
    ));
}

#[derive(Message)]
struct ApplyResolution(pub UVec2);

// FIXME: this freezes the screen for some reason.
fn apply_resolution(mut evr: MessageReader<ApplyResolution>, mut window: Single<&mut Window>) {
    for e in evr.read() {
        window.resolution.set(e.0.x as f32, e.0.y as f32);
    }
}
