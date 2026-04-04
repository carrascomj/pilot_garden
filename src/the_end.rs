//! The end screen.

use bevy::prelude::*;

use crate::{config::GameState, menu::Winner, player_movement::Player};

pub struct EndPlugin;

impl Plugin for EndPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (fade_in_flash, move_player_in_end_screen, fade_out)
                .run_if(in_state(GameState::EndScreen)),
        )
        .add_systems(Update, check_for_escape.run_if(in_state(GameState::Below)))
        .add_systems(OnEnter(GameState::EndScreen), move_to_end_screen);
    }
}

/// Check if the player has tried to traspassed the exit door.
fn check_for_escape(
    mut next_state: ResMut<NextState<GameState>>,
    mut winner: ResMut<Winner>,
    player: Single<&Transform, With<Player>>,
) {
    {}
    // hardcoded
    const DOOR_POS: Vec2 = Vec2::new(-37.1173, 110.93);
    let Vec3 { x: p_x, z: p_z, .. } = player.translation;
    let distance = DOOR_POS.distance_squared((p_x, p_z).into());
    // cherry picked
    if distance < 6. {
        next_state.set(GameState::EndScreen);
        winner.0 = true;
    }
}

#[derive(Component)]
pub struct FlashScreen(Timer);
#[derive(Component)]
struct FadeOut;
#[derive(Resource)]
struct PlayerMovementCurve(CubicCurve<Vec3>);

/// Spawns scene and setup the entities for movement for the final The End screen.
fn move_to_end_screen(
    mut commands: Commands,
    player: Single<(&mut Transform, &mut Projection), With<Player>>,
    asset_server: ResMut<AssetServer>,
) {
    // end scene
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("end_screen.glb"))),
        DespawnOnExit(GameState::EndScreen),
    ));
    // hardcoded from blender
    let p0 = Vec3::new(-36.675315856933594, -26.088350296020508, 110.93441772460938);
    let p1 = Vec3::new(-37.117340087890625, -26.088350296020508, 110.93441772460938);
    let p2 = Vec3::new(-36.10966491699219, 43.229583740234375, 167.854248046875);
    let p3 = Vec3::new(-36.117340087890625, -26.088350296020508, 236.85617065429688);
    commands.insert_resource(PlayerMovementCurve(
        CubicBezier::new([[p0, p1, p2, p3]]).to_curve().unwrap(),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::Srgba(Srgba {
                red: 0.95,
                green: 0.2,
                blue: 0.4,
                alpha: 1.0,
            }),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(p0.x + 40., p2.y, 160.)
            .looking_at(Vec3::new(p2.x - 20., p0.y, p2.z), Vec3::NEG_Y),
        DespawnOnExit(GameState::EndScreen),
    ));

    // UI for flash (blinded from outside light)
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
        FlashScreen(Timer::from_seconds(4.0, TimerMode::Once)),
        BackgroundColor(Color::WHITE),
        DespawnOnExit(GameState::EndScreen),
    ));
    // UI for showing the end screen
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        FadeOut,
        BackgroundColor(Color::BLACK.with_alpha(0.)),
        DespawnOnExit(GameState::EndScreen),
        children![(
            Text::new("The End"),
            TextFont {
                font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
                font_size: 60.0,
                ..default()
            },
            TextColor(Color::WHITE.with_alpha(0.)),
        )],
    ));
    let (mut player_transform, mut projection) = player.into_inner();

    player_transform.look_to(Vec3::Z, Vec3::Y);
    player_transform.translation.x = -37.5155;
    player_transform.translation.z = 111.69;
    let Projection::Perspective(perspective) = projection.as_mut() else {
        return;
    };
    perspective.fov = 80.0_f32.to_radians();
}

/// Move the player, only allow for front and back movement.
///
/// Normal movement at [`move_player`] is disabled for EndScreen.
fn move_player_in_end_screen(
    time: Res<Time>,
    curve: Res<PlayerMovementCurve>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut Transform, With<Player>>,
    mut progress: Local<f32>,
) {
    let mut input = 0f32;
    if keyboard_input.pressed(KeyCode::KeyW) {
        input += 1.;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        input -= 1.;
    }
    *progress += input.clamp(-1., 1.) * time.delta_secs() * 0.1; // advance along the curve
    *progress = progress.clamp(0.05, 1.0);
    player.translation = curve.0.position(*progress);
    // look in the direction of the curve to make the horizon effect
    let tangent = curve.0.velocity(*progress);
    player.look_to(tangent, Vec3::Y);
}

fn fade_in_flash(time: Res<Time>, flash: Single<(&mut FlashScreen, &mut BackgroundColor)>) {
    let (mut timer, mut color) = flash.into_inner();
    if timer.0.just_finished() {
        color.0.set_alpha(0.);
    } else {
        timer.0.tick(time.delta());
        let u = timer.0.fraction_remaining();
        color.0.set_alpha(u);
    }
}

/// Fade out to the end with player position.
fn fade_out(
    mut next_state: ResMut<NextState<GameState>>,
    mut fade_out: Single<(&Children, &mut BackgroundColor), With<FadeOut>>,
    player: Single<(&Transform, &mut Projection), With<Player>>,
    mut the_end_text: Query<&mut TextColor>,
) {
    let (player_transform, mut projection) = player.into_inner();
    let end_percent = (player_transform.translation.z - 111.69) / (190.89 - 111.69);
    fade_out.1.0.set_alpha(end_percent);
    if end_percent > 0.8 {
        for child in fade_out.0 {
            if let Ok(mut text_color) = the_end_text.get_mut(*child) {
                text_color.0.set_alpha((end_percent - 0.8) / 0.2);
            }
        }
    }
    if end_percent > 0.98 {
        next_state.set(GameState::Menu);
        let Projection::Perspective(perspective) = projection.as_mut() else {
            return;
        };
        // default
        perspective.fov = 1.0471976;
    }
}
