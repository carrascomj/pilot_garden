use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
    },
};
use menu::GameMenu;
use std::{f32::consts::TAU, time::Duration};

mod config;
mod digging;
mod dodgy;
mod gnomes;
mod killer_arms;
mod menu;
mod player_movement;
mod point_raycast;
mod world_timer;

use digging::{DiggingPlugin, Life, Minable, OneSizeCollider};
use dodgy::{Dodgy, DodgyPlugin};
use gnomes::GnomePlugin;
use killer_arms::{KillerArmPlugin, KillerHead, KillerPoint, KillerTimer};
use player_movement::{Collider, Player, PlayerPlugin};
use point_raycast::FirstPersonPickerPlugin;
use world_timer::{DayNightPlugin, ShowOnAlarmTime, TimerComp};

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
        .add_systems(Startup, (setup, setup_colliders))
        .add_systems(OnEnter(GameState::Menu), (spawn_tool_bench, spawn_crops))
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
            DayNightPlugin,
            DiggingPlugin,
            DodgyPlugin,
            FirstPersonPickerPlugin,
            GameMenu,
            GnomePlugin,
            KillerArmPlugin,
            PlayerPlugin,
        ))
        .add_plugins(MaterialPlugin::<CapsuleMaterial>::default())
        .run();
}

/// Setup colliders. Since the setup is very simple, we set colliders manually;
/// they only interact with the player; and they are independent from the 3D models.
fn setup_colliders(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    for (half_x, half_z, x, z, mult_y, y) in [
        // special right wall to surround the fake bush
        (28.0, 2.0, 3.0, 10.0, 8.0, 1.0),
        (10.0, 2.0, 24.0, 10.0, 8.0, 1.0),
        // walls around the garden
        (38.0, 2.0, 11.0, -9.5, 8.0, 1.0),
        (2.0, 20.0, -9.5, 0.0, 8.0, 1.0),
        (2.0, 20.0, 30.5, 0.0, 8.0, 1.0),
        // fences
        (0.5, 10.0, 18.2, -4.0, 2.0, 1.0),
        (7.5, 0.5, 26.0, 1.35, 2.0, 1.0),
        // floor (subdivided to accomodate dirt colliders)
        (26.0, 22.0, 5.0, 0.0, 1.0, -1.0),
        (12.0, 12.0, 24.0, 7.0, 1.0, -1.0),
        // fake bush safe zone
        (20.0, 20.0, 24.0, 20.0, 1.0, -4.0),
    ] {
        let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
        let cub_transform = Transform::from_xyz(x, y, z);
        let cub_collider = Collider::from((&cub, &cub_transform));
        commands.spawn((cub_transform, cub_collider));
    }
    let (half_x, half_z, x, z, mult_y, y) = (38.0, 2.0, 11.0, 10.0, 30., -20.0);

    // invisible mesh to protect the player from lasers
    let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
    let cub_transform = Transform::from_xyz(x, y, z);
    commands.spawn((Mesh3d(meshes.add(cub)), cub_transform));
}

/// Marker for the capsule so we can check if we clicked it
/// and activate the menu again.
#[derive(Component)]
pub struct Capsule {
    pub active: bool,
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
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1., 4.))),
        MeshMaterial3d(materials.add(CapsuleMaterial {})),
        Transform::from_translation(POS).with_rotation(Quat::from_rotation_y(3.14)),
        Capsule { active: false },
    ));

    // light in safe zone
    commands.spawn((
        PointLight {
            color: Color::Srgba(Srgba {
                red: 1.0,
                green: 0.3,
                blue: 0.9,
                alpha: 1.0,
            }),
            range: 10.,
            radius: 10.,
            intensity: 200_000.,
            shadows_enabled: true,
            ..default()
        },
        Visibility::Visible,
        Transform::from_xyz(21., 2.13, 23.4).looking_at(Vec3::NEG_Y, Vec3::NEG_Y),
    ));
}

#[derive(Component)]
pub struct SomeLights;

/// Marker for the tools.
#[derive(Component)]
pub struct ToolBench;

fn spawn_tool_bench(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing_tooltip: Query<Entity, With<ToolBench>>,
) {
    for tooltip in &existing_tooltip {
        commands.entity(tooltip).despawn();
    }

    // spawn the tools
    let mut timer = TimerComp::from_elapsed(2.5);
    timer.0.pause();
    // this is the initial position of the scene
    // the tooltip inside the scene is put to match bush.gltf
    let init_pos = Vec3::new(0., 0., 0.);
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("tools.glb"))),
        timer,
        ToolBench,
        ShowOnAlarmTime::as_false(),
        Dodgy {
            init_pos,
            last_pos: init_pos - Vec3::Y * 11.,
            go_back: false,
            ignore_viewing: false,
        },
    ));
}

/// Marker for the crop container.
#[derive(Component)]
pub struct Crop;

fn spawn_crops(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing_crop: Query<Entity, With<Crop>>,
) {
    for tooltip in &existing_crop {
        commands.entity(tooltip).despawn();
    }
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("crops.glb"))),
        Crop,
    ));
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
        } else if name.as_str().starts_with("one_sized") {
            // fake bushes can be removed with the mining pick
            commands.entity(entity).insert(OneSizeCollider);
        } else if name.as_str() == "fakebush" {
            // fake bushes can be removed with the mining pick
            const BUSH_TRANS: Vec3 = Vec3::new(18., 2., 10.);
            const SIZE: Vec3 = Vec3::new(1.0, 2.0, 1.0);
            commands.entity(entity).insert((
                Minable {},
                Life::JustSpawned,
                Collider::from_translation(BUSH_TRANS + Vec3::Y * 0.5, SIZE),
            ));
        } else if name.as_str() == "point_bone" {
            // this is the object bone of an IK
            commands.entity(entity).insert(KillerPoint);
        } else if name.as_str() == "ik_target" {
            // this is the target bone of an IK
            let dur = Duration::from_secs_f32(1.);
            let mut laser_timer = TimerComp(Timer::new(dur, TimerMode::Once));
            laser_timer.0.pause();
            let dur = Duration::from_secs_f32(5.);
            let mut killer_timer = KillerTimer {
                timer: Timer::new(dur, TimerMode::Once),
                can_kill: false,
            };
            killer_timer.timer.pause();
            commands.entity(entity).insert((
                KillerHead,
                laser_timer,
                killer_timer,
                StateScoped(GameState::Above),
            ));
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

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // draw both faces
        Ok(())
    }

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn opaque_render_method(&self) -> bevy::pbr::OpaqueRendererMethod {
        bevy::pbr::OpaqueRendererMethod::Forward
    }

    fn depth_bias(&self) -> f32 {
        0.0
    }

    fn reads_view_transmission_texture(&self) -> bool {
        false
    }

    fn prepass_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn prepass_fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn deferred_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }
}
