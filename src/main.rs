use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
};
use menu::GameMenu;
use std::f32::consts::TAU;

mod config;
mod digging;
mod dodgy;
mod menu;
mod player_movement;
mod world_timer;

use digging::{DiggingPlugin, Life, Minable, remove_on_click};
use dodgy::DodgyPlugin;
use player_movement::{Collider, Player, PlayerPlugin};
use world_timer::{DayNightPlugin, TimerComp};

use config::{BUMP_DISTANCE, GROUND_Y, GameState};

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
                ..default()
            }),
            ..default()
        }),))
        .init_state::<GameState>()
        .insert_resource(AmbientLight {
            brightness: 40.0,
            ..default()
        })
        .add_systems(Startup, (setup, setup_colliders))
        .add_systems(
            Update,
            (
                find_main_bone,
                trigger_main_bone_animation,
                animate_main_bone,
            )
                .run_if(not(in_state(GameState::Menu))),
        )
        // custom game mechanics
        .add_plugins((
            PlayerPlugin,
            DodgyPlugin,
            DiggingPlugin,
            GameMenu,
            DayNightPlugin,
        ))
        .add_plugins(MaterialPlugin::<CapsuleMaterial>::default())
        .run();
}

/// Setup colliders. Since the setup is very simple, we set colliders manually;
/// they only interact with the player; and they are independent from the 3D models.
fn setup_colliders(mut commands: Commands) {
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
        commands.spawn((cub_transform, cub_collider));
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CapsuleMaterial>>,
) {
    // spawn main scene with all bushes, crops, etc.
    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("bush.gltf")),
    ));

    // spawn the capsule, which is generated with a material with a shader
    const POS: Vec3 = Vec3::new(-6.0, 3.0, 8.0);
    commands
        .spawn((
            Mesh3d(meshes.add(Cylinder::new(1., 4.))),
            MeshMaterial3d(materials.add(CapsuleMaterial {})),
            Transform::from_translation(POS).with_rotation(Quat::from_rotation_y(3.14)),
            Collider::from_translation(POS, Vec3::new(1.0, 3.0, 1.0)),
            Capsule { active: true },
        ))
        .observe(menu_on_click);
}

/// Marker for the capsule so we can check if we clicked it
/// and activate the menu again.
#[derive(Component)]
pub struct Capsule {
    pub active: bool,
}

/// This needs more polish, with an animation or something.
fn menu_on_click(
    _trigger: Trigger<Pointer<Pressed>>,
    mut next_state: ResMut<NextState<GameState>>,
    capsule: Single<&Capsule>,
) {
    if capsule.active {
        next_state.set(GameState::Menu);
    }
}

/// The player may bump with objects. If they have a "main" bone, this will cause a
/// small procedural animation that twists the object back and forth.
/// Marker for main bone, added after loading the gltf if a bone with name "main" exists.
#[derive(Component)]
#[require(TimerComp::from_elapsed(0.5))]
struct MainBone {
    rest_rot: Quat,
    active: bool,
}

fn find_main_bone(
    mut commands: Commands,
    new_names: Populated<(Entity, &Name, &Transform), Added<Name>>,
) {
    for (entity, name, transform) in new_names.iter() {
        if name.as_str().starts_with("main") {
            commands.entity(entity).insert(MainBone {
                rest_rot: transform.rotation,
                active: true,
            });
        } else if name.as_str() == "fakebush" {
            // fake bushes can be removed with the mining pick
            commands
                .entity(entity)
                .insert((Minable {}, Life::JustSpawned))
                .observe(remove_on_click);
        }
    }
}

fn trigger_main_bone_animation(
    player: Single<&Transform, With<Player>>,
    mut transforms: Query<(&GlobalTransform, &mut MainBone, &mut TimerComp), Without<Player>>, // all transforms
) {
    let transform = player.into_inner();
    for (parent_t, mut main_bone, mut timer) in &mut transforms {
        if parent_t
            .translation()
            .distance_squared(transform.translation)
            < BUMP_DISTANCE
        {
            if timer.0.finished() && main_bone.active {
                timer.0.unpause();
                timer.0.reset();
                // only trigger the animation once after entering the bump distance
                main_bone.active = false;
            }
        } else {
            main_bone.active = true;
        }
    }
}

fn animate_main_bone(mut bones: Query<(&mut Transform, &MainBone, &TimerComp)>) {
    const AMP: f32 = 0.30; // max rad
    const FREQ: f32 = 3.0; // oscillations per second
    const DECAY: f32 = 2.5; // bigger -> stops sooner

    for (mut transform, bone, timer) in &mut bones {
        if !timer.0.finished() {
            let u = timer.0.fraction();
            // damped wobble: sin curve multiplied by an exponential decay
            let angle = AMP * (TAU * FREQ * u).sin() * (-DECAY * u).exp();
            transform.rotation = bone.rest_rot * Quat::from_rotation_x(angle);
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct CapsuleMaterial {}

const SHADER: &str = "shaders/capsule_fog.wgsl";

impl Material for CapsuleMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
