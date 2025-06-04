use bevy::prelude::*;
use std::f32::consts::TAU;
use std::time::Duration;

mod config;
mod digging;
mod player_movement;

use digging::DiggingPlugin;
use player_movement::{Collider, Player, PlayerPlugin};

use config::{BUMP_DISTANCE, GROUND_Y};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "And you will be happy".into(),
                name: Some("And you will be happy".into()),
                // Tells Wasm to resize the window according to the available canvas
                fit_canvas_to_parent: true,
                // Tells Wasm not to override default event handling, like F5, Ctrl+R etc.
                prevent_default_event_handling: false,
                enabled_buttons: bevy::window::EnabledButtons {
                    maximize: false,
                    ..Default::default()
                },
                cursor_options: bevy::window::CursorOptions {
                    visible: false,
                    grab_mode: bevy::window::CursorGrabMode::Locked,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }),))
        .insert_resource(AmbientLight {
            brightness: 300.0,
            ..default()
        })
        .add_systems(Startup, (setup, setup_colliders))
        .add_systems(PostUpdate, find_main_bone)
        .add_systems(Update, (trigger_main_bone_animation, animate_main_bone))
        // custom game mechanics
        .add_plugins((PlayerPlugin, DiggingPlugin))
        .run();
}

/// Setup colliders. Since the setup is very simple, we set colliders manually;
/// they only interact with the player; and they are independent from the 3D models.
fn setup_colliders(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    for (half_x, half_z, x, z, mult_y, y) in [
        // walls around the garden
        (38.0, 2.0, 11.0, 10.0, 8.0, 1.0),
        (38.0, 2.0, 11.0, -9.5, 8.0, 1.0),
        (2.0, 20.0, -9.5, 0.0, 8.0, 1.0),
        (2.0, 20.0, 30.5, 0.0, 8.0, 1.0),
        // fences
        (0.5, 10.0, 18.2, -4.0, 2.0, 1.0),
        (7.5, 0.5, 26.0, 1.35, 2.0, 1.0),
        // floor (subdivided to accomodate dirt colliders)
        (26.0, 20.0, 5.0, 0.0, 1.0, -1.0),
        (12.0, 10.0, 24.0, 6.0, 1.0, -1.0),
    ] {
        let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
        let cub_transform = Transform::from_xyz(x, y, z);
        let cub_collider = Collider::from((&cub, &cub_transform));
        let cub_mesh = meshes.add(cub);
        commands.spawn((Mesh3d(cub_mesh), cub_transform, cub_collider));
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn the first scene in `models/SimpleSkin/SimpleSkin.gltf`
    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("bush.gltf")),
    ));

    commands.spawn((
        DirectionalLight {
            color: Color::Srgba(Srgba {
                red: 0.95,
                green: 0.64,
                blue: 0.75,
                alpha: 1.0,
            }),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 8.0).looking_at(Vec3::X * 10.0, Vec3::NEG_Y),
    ));
}

/// The player may bump with objects. If they have a "main" bone, this will cause a
/// small procedural animation that twists the object back and forth.
/// Marker for main bone, added after loading the gltf if a bone with name "main" exists.
#[derive(Component)]
struct MainBone {
    timer: Timer,
    rest_rot: Quat,
    active: bool,
}

fn find_main_bone(
    mut commands: Commands,
    new_names: Populated<(Entity, &Name, &Transform), Added<Name>>,
) {
    for (entity, name, transform) in new_names.iter() {
        if name.as_str().starts_with("main") {
            let mut timer = Timer::new(Duration::from_millis(500), TimerMode::Once);
            timer.set_elapsed(Duration::from_millis(500));
            commands.entity(entity).insert(MainBone {
                timer,
                rest_rot: transform.rotation,
                active: true,
            });
        }
    }
}

fn trigger_main_bone_animation(
    time: Res<Time>,
    player: Single<&Transform, With<Player>>,
    mut transforms: Query<(&GlobalTransform, &mut MainBone), Without<Player>>, // all transforms
) {
    let transform = player.into_inner();
    for (parent_t, mut main_bone) in &mut transforms {
        main_bone.timer.tick(time.delta());
        if parent_t
            .translation()
            .distance_squared(transform.translation)
            < BUMP_DISTANCE
        {
            if main_bone.timer.finished() && main_bone.active {
                main_bone.timer.unpause();
                main_bone.timer.reset();
                // only trigger the animation once after entering the bump distance
                main_bone.active = false;
            }
        } else {
            main_bone.active = true;
        }
    }
}

fn animate_main_bone(mut bones: Query<(&mut Transform, &MainBone)>) {
    const AMP: f32 = 0.30; // max rad
    const FREQ: f32 = 3.0; // oscillations per second
    const DECAY: f32 = 2.5; // bigger -> stops sooner

    for (mut transform, bone) in &mut bones {
        if !bone.timer.finished() {
            let u = bone.timer.elapsed().as_secs_f32() / bone.timer.duration().as_secs_f32();

            // damped wobble: sin curve multiplied by an exponential decay
            let angle = AMP * (TAU * FREQ * u).sin() * (-DECAY * u).exp();

            transform.rotation = bone.rest_rot * Quat::from_rotation_x(angle);
        }
    }
}
