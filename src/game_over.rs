//! Game over screen.

use bevy::prelude::*;

use crate::{
    config::GameState,
    emoji_particles::{EmojiBurst, SecretRevealed},
    player_movement::Player,
    world_timer::TimerComp,
};

/// Game over screen for when the player dies.
pub struct GameOver;

impl Plugin for GameOver {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), show_game_over)
            .add_systems(
                Update,
                fade_in_game_over.run_if(in_state(GameState::GameOver)),
            );
    }
}

#[derive(Component)]
struct GameOverScreen;

fn show_game_over(
    mut commands: Commands,
    mut secret_rev: ResMut<SecretRevealed>,
    asset_server: ResMut<AssetServer>,
    mut emoji_event: EventWriter<EmojiBurst>,
) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        StateScoped(GameState::GameOver),
        GameOverScreen,
        BackgroundColor(Color::srgba(0.08, 0.03, 0.05, 0.3)), // overlay
        TimerComp(Timer::from_seconds(8.0, TimerMode::Once)),
        children![(
            Text::new("Reconnecting..."),
            TextFont {
                font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                font_size: 60.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.2, 0.2)),
            TextShadow::default(),
        )],
    ));
    emoji_event.write(EmojiBurst {
        velocity: 3000.,
        percent: 0.8,
    });
    secret_rev.0 = false;
}

fn fade_in_game_over(
    mut next_state: ResMut<NextState<GameState>>,
    mut screen: Single<(&mut BackgroundColor, &TimerComp), With<GameOverScreen>>,
    mut player: Single<&mut Transform, With<Player>>,
) {
    if screen.1.0.finished() {
        next_state.set(GameState::Menu);
    }
    // fade in animation
    let t = screen.1.0.fraction();
    screen.0.0.set_alpha(t * 0.7 + 0.3);

    if t < 0.195 {
        // on the first second, rotate the player and make it fall down
        // a bit sketchy to avoid saving player state, faster at the beginning
        player.rotation *= Quat::from_rotation_z((1. - t) * 5. * 0.02);
    }
}
