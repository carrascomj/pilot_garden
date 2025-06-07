use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    window::CursorOptions,
};

use crate::{
    config::GameState,
    player_movement::{Player, Velocity},
};

pub struct GameMenu;

// colors for button interactions
const BUTTON_COLOR: Color = Color::srgb(1.0, 0.3, 0.9); // cyber pink
const PRESSED_COLOR: Color = Color::srgb(0.3, 1.0, 0.9); // cyber blue
const HOVER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0); // white blue

impl Plugin for GameMenu {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), spawn_game_menu)
            .add_systems(Update, button_system.run_if(in_state(GameState::Menu)))
            // will run even after GameState menu since it has to play the animation for awakening
            .add_systems(Last, update_time)
            .add_systems(OnExit(GameState::Menu), eyes_wide_open)
            .add_plugins(UiMaterialPlugin::<HibernationMaterial>::default());
    }
}

/// Attached to button entities to decide action on press.
#[derive(Component)]
enum ButtonAction {
    StartGame,
    ShowSettings,
}

/// Marker for start menu.
#[derive(Component)]
struct StartMenu;

fn spawn_game_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ui_materials: ResMut<Assets<HibernationMaterial>>,
) {
    commands.spawn((
        StartMenu,
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
        BackgroundColor(Color::BLACK), // fog colour
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
            StateScoped(GameState::Menu),
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
                    BorderColor(BUTTON_COLOR),
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
                    BorderColor(BUTTON_COLOR),
                    BoxShadow::new(
                        BUTTON_COLOR.with_alpha(0.2),
                        Val::Percent(0.),
                        Val::Percent(0.),
                        Val::Percent(3.0),
                        Val::Px(3.0),
                    ),
                    BackgroundColor(Color::NONE),
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
            ]
        )],
        (
            Text::new("WINNER: FALSE"),
            TextFont {
                font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                font_size: 33.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.2, 0.2)),
            TextShadow::default(),
        ),
    ));
}

/// Maintaing material updated with time and trigger awakening
/// effect on shader when it comes the time: disolve time is made positive
/// when exiting [`GameState::Menu`] by [`eyes_wide_open`].
fn update_time(
    mut commands: Commands,
    time: Res<Time>,
    mut ui_materials: ResMut<Assets<HibernationMaterial>>,
    mut state_menu: Single<(Entity, &mut BackgroundColor), With<StartMenu>>,
    mut player: Single<&mut Velocity, With<Player>>,
) {
    for (_, material) in ui_materials.iter_mut() {
        if material.disolve_time > 0. {
            let diff = material.time - material.disolve_time;
            // dissolve the background with the shader after the eyes open
            let alpha = 1. - (diff - 3.0).clamp(0., 5.) / 5.0;
            state_menu.1.0.set_alpha(alpha);
            if diff > 0.0 && player.y <= 2.0 {
                player.0.x += 100.0;
                player.0.y += 10.0;
            }
            if diff > 5.0 {
                commands.entity(state_menu.0).despawn()
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
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
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
    mut window: Single<&mut Window>,
) {
    for (interaction, mut box_shadow, mut border_color, children, action) in &mut interaction_query
    {
        let mut text_color = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                box_shadow.0[0].color = PRESSED_COLOR.with_alpha(0.3).into();
                *border_color = PRESSED_COLOR.into();
                *text_color = PRESSED_COLOR.into();
                match action {
                    ButtonAction::StartGame => {
                        next_state.set(GameState::Above);
                        window.cursor_options = CursorOptions {
                            visible: false,
                            grab_mode: bevy::window::CursorGrabMode::Locked,
                            ..default()
                        };
                    }
                    _ => (),
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

/// Change disolve time, which is a uniform that will make the
/// shade "open the eyes" and increase the opacity of the overlay.
fn eyes_wide_open(mut ui_materials: ResMut<Assets<HibernationMaterial>>) {
    for (_, material) in ui_materials.iter_mut() {
        material.disolve_time = material.time;
    }
}
