//! Entry point of the game, call the other plugins, load some main scenes from
//! GLTF and tag specific GLTF entities based with components based on their names.
use bevy::{
    ecs::message::MessageWriter,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};
use menu::GameMenu;
use std::{f32::consts::TAU, time::Duration};

mod audio;
mod config;
mod digging;
mod dodgy;
mod emoji_particles;
mod game_over;
mod gnomes;
mod input_mapping;
mod killer_arms;
mod laser;
mod menu;
mod player_movement;
mod point_raycast;
mod surveillance;
mod the_end;
mod world_timer;

use audio::AudioPlugin;
use digging::{DiggingPlugin, Life, Minable, OneSizeCollider};
use dodgy::{Dodgy, DodgyPlugin};
use emoji_particles::EmojiPlugin;
use game_over::GameOver;
use gnomes::GnomePlugin;
use input_mapping::InputMappingPlugin;
use killer_arms::{KillerArmPlugin, KillerHead, KillerPoint, KillerTimer};
use laser::LaserPlugin;
use player_movement::{Collider, Player, PlayerPlugin};
use point_raycast::FirstPersonPickerPlugin;
use surveillance::SurveillancePlugin;
use the_end::EndPlugin;
use world_timer::{DayNightPlugin, ShowOnAlarmTime, TimerComp};

use config::{BUMP_DISTANCE, GROUND_Y, GameState};

use crate::{
    audio::AudioStart,
    digging::{Diggable, FakeGround, was_removed_by_player},
    emoji_particles::{Secret, SecretRevealed},
    point_raycast::RayBlocker,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pilot Garden".into(),
                    name: Some("Pilot Garden".into()),
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
            })
            .set(ImagePlugin::default_nearest()),))
        .init_state::<GameState>()
        .add_systems(Startup, (setup_shared_colliders, setup_shared_meshes))
        .add_systems(
            OnEnter(GameState::Menu),
            (
                setup_colliders_above,
                setup_above_scene,
                spawn_tool_bench,
                spawn_crops,
            ),
        )
        .add_systems(
            OnEnter(GameState::Below),
            (setup_colliders_below, setup_below),
        )
        .add_systems(OnExit(GameState::GameOver), remove_on_game_over)
        .add_systems(OnEnter(GameState::EndScreen), remove_on_game_over)
        .add_systems(
            Update,
            (
                tag_gltf_on_add,
                trigger_main_bone_animation,
                animate_main_bone,
            )
                .run_if(not(in_state(GameState::Menu))),
        )
        .add_systems(Update, transit_to_below.run_if(in_state(GameState::Above)))
        // custom game mechanics
        .add_plugins((
            AudioPlugin,
            DayNightPlugin,
            DiggingPlugin,
            DodgyPlugin,
            EmojiPlugin,
            FirstPersonPickerPlugin,
            GameMenu,
            GameOver,
            GnomePlugin,
            InputMappingPlugin,
            KillerArmPlugin,
            LaserPlugin,
            PlayerPlugin,
            SurveillancePlugin,
            EndPlugin,
        ))
        .add_plugins(MaterialPlugin::<CapsuleMaterial>::default())
        .run();
}

/// Setup colliders. Since the setup is very simple, we set colliders manually;
/// they only interact with the player; and they are independent from the 3D models.
fn setup_colliders_above(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
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
        // table
        (2., 2., 9.9080, 8.2921, 1., 1.254),
        // floor (subdivided to accomodate dirt colliders, rest in shared colliders)
        (26.0, 22.0, 5.0, 0.0, 1.0, -1.2),
        // fake bush safe zone
        (20.0, 20.0, 24.0, 20.0, 1.0, -4.0), // floor
        (2., 20., 16., 20.3106, 8.0, -2.0),  // right wall
        (2., 20., 25., 20.3106, 8.0, -2.0),  // left wall
        (14., 2., 23., 25., 8.0, -2.0),      // front wall
        (8., 3., 23., 12.5, 8.0, -2.0),      // back wall
        // good ending exit
        (1., 1., 25., 7.78, 1.0, -4.0), // back wall
        (1., 1., 25., 5.78, 1.0, -4.0), // back wall
        // crops walls
        (12., 8., 24.367, -3.1745, 1.0, -8.), // main floor
        (6., 2., 27.367, -8.1745, 1.0, -8.),  // right of passage floor
        (4., 2., 20.367, -8.1745, 1.0, -8.),  // left of passage floor
    ] {
        let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
        let cub_transform = Transform::from_xyz(x, y, z);
        let cub_collider = Collider::from((&cub, &cub_transform));
        commands.spawn((cub_transform, cub_collider, DespawnOnExit(GameState::Above)));
    }
    let (half_x, half_z, x, z, mult_y, y) = (38.0, 2.0, 11.0, 10.0, 30., -20.0);

    // invisible mesh to protect the player from lasers
    let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
    let cub_transform = Transform::from_xyz(x, y, z);
    commands.spawn((
        Mesh3d(meshes.add(cub)),
        cub_transform,
        DespawnOnExit(GameState::Above),
    ));
}

fn setup_shared_colliders(mut commands: Commands) {
    for (half_x, half_z, x, z, mult_y, y) in [
        // floor escape (subdivided to accomodate dirt colliders)
        (12.0, 5.85, 24.0, 3.9, 4.0, -5.4),
        (8.0, 4.1, 22.0, 8.9, 4.0, -5.4),
        (2., 3.5, 29., 8.6, 4.0, -5.4),
        (2., 2., 27., 9.8, 4.0, -5.4),
        // initial floor below
        (14., 100., 28.0, 36., 1.0, -27.5),
    ] {
        let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
        let cub_transform = Transform::from_xyz(x, y, z);
        let cub_collider = Collider::from((&cub, &cub_transform));
        commands.spawn((cub_transform, cub_collider));
    }
}

fn setup_colliders_below(mut commands: Commands) {
    for (half_x, half_z, x, z, mult_y, y) in [
        // walls below initial zone
        (2., 10., 20.5, -7.6, 6.0, -27.5),
        (2., 10., 31.5, -7.6, 6.0, -27.5),
        (38.0, 2.0, 19., -12.5, 8.0, -27.5),
        // first corridor door
        (6.0, 2.0, 30.5, -2., 8.0, -27.5),
        (6.0, 2.0, 21.5, -2., 8.0, -27.5),
        // first corridor
        (2., 73., 21.5, 34., 6.0, -27.5),
        (2., 90., 29., 44., 6.0, -27.5),
        // second corridor
        (44.0, 2.0, 7., 77.5, 8.0, -27.5), // left wall
        (36.0, 2.0, 3., 70., 12.0, -27.5), // right wall
        (40.0, 8., 2., 74.5, 1.0, -27.5),  // floor
        // studio
        (68.0, 52.0, -49., 85., 1.0, -27.5),   // floor
        (68.0, 2.0, -49., 62., 6., -27.5),     // left wall
        (68.0, 2.0, -49., 110., 6., -27.5),    // right wall
        (2.0, 52., -78.5, 85., 6., -27.5),     // front wall
        (2.0, 34., -15.5, 95., 6., -27.5),     // bottom wall left
        (2.0, 12.0, -15.5, 65., 6., -27.5),    // bottom right
        (20.0, 2.0, -34.927, 100., 3., -23.0), // left block wall large
        (2.0, 10.0, -45., 105.76, 3., -23.0),  // left block wall small
        // button
        (2., 2., -65., 80., 1., -25.129),
    ] {
        let cub = Cuboid::new(half_x, GROUND_Y * mult_y, half_z);
        let cub_transform = Transform::from_xyz(x, y, z);
        let cub_collider = Collider::from((&cub, &cub_transform));

        commands.spawn((cub_transform, cub_collider, DespawnOnExit(GameState::Below)));
    }
}

/// If the player has managed to dig enough, switch to below.
fn transit_to_below(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut secret_rev: ResMut<SecretRevealed>,
    mut ambient_light: ResMut<GlobalAmbientLight>,
    player: Single<&Transform, With<Player>>,
    main_scenes: Query<Entity, With<MainScene>>,
) {
    if player.translation.y < -8. {
        next_state.set(GameState::Below);
        for ent in &main_scenes {
            commands.entity(ent).despawn();
        }
        secret_rev.0 = true;
        // just to see a bit better in the interiors
        // although this will be overriden by the gnomes sometimes
        ambient_light.brightness = 200.;
    }
}

/// Marker for the capsule so we can check if we clicked it
/// and activate the menu again.
#[derive(Component)]
pub struct Capsule {
    pub active: bool,
    pub shown_pos: Vec3,
    pub hidden_pos: Vec3,
}
/// Marker for main scene to respawn it if it does not exist.
#[derive(Component)]
struct MainScene;

/// Spawn main initial scene with all bushes, crops, etc.
fn setup_above_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut bush_handle: Local<Option<Handle<Scene>>>,
    main_scene: Query<Entity, With<MainScene>>,
) {
    if main_scene.iter().len() > 0 {
        return;
    }
    if bush_handle.is_none() {
        *bush_handle = Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("bush.gltf")));
    }

    commands.spawn((
        SceneRoot(bush_handle.as_ref().expect("initialized").clone()),
        MainScene,
    ));

    // light in safe zone
    commands.spawn((
        MainScene,
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

/// Spawn the connections between the above and below scens
/// and the capsule, which is generated with a material with a shader.
fn setup_shared_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<CapsuleMaterial>>,
) {
    const POS: Vec3 = Vec3::new(-6.0, 3.0, 8.0);
    let hidden_pos = POS - Vec3::Y * 10.0;
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1., 4.))),
        MeshMaterial3d(materials.add(CapsuleMaterial {})),
        Transform::from_translation(POS).with_rotation(Quat::from_rotation_y(3.14)),
        Capsule {
            active: false,
            shown_pos: POS,
            hidden_pos,
        },
    ));
    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("shared.glb")),
    ));
}

#[derive(Component)]
pub struct GameOverRemove;

fn remove_on_game_over(mut commands: Commands, to_rm: Query<Entity, With<GameOverRemove>>) {
    for ent in to_rm {
        commands.entity(ent).despawn();
    }
}

fn setup_below(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut below_handle: Local<Option<Handle<Scene>>>,
) {
    if below_handle.is_none() {
        *below_handle = Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("below.glb")));
    }
    commands.spawn((
        SceneRoot(below_handle.as_ref().expect("initialized already").clone()),
        GameOverRemove,
    ));
    commands.spawn_batch([
        (
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
            Transform::from_xyz(26.0, -20.0, 76.4),
            DespawnOnExit(GameState::Below),
        ),
        (
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
            Transform::from_xyz(30.0, -21.4, -3.8),
            DespawnOnExit(GameState::Below),
        ),
        (
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
            Transform::from_xyz(30.0, -21.4, -11.8),
            DespawnOnExit(GameState::Below),
        ),
        (
            PointLight {
                color: Color::Srgba(Srgba {
                    red: 0.9,
                    green: 0.9,
                    blue: 0.7,
                    alpha: 1.0,
                }),
                range: 10.,
                radius: 3.,
                intensity: 700.,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(27.0, -15., 7.8),
            DespawnOnExit(GameState::Below),
        ),
        (
            PointLight {
                color: Color::Srgba(Srgba {
                    red: 0.9,
                    green: 0.9,
                    blue: 0.7,
                    alpha: 1.0,
                }),
                range: 10.,
                radius: 3.,
                intensity: 7000.,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(23.56, -15., -7.75),
            DespawnOnExit(GameState::Below),
        ),
    ]);
}

#[derive(Component)]
pub struct SomeLights;

/// Marker for the tools.
#[derive(Component)]
pub struct ToolBench;

pub const TOOL_BENCH_VISIBLE_POS: Vec3 = Vec3::ZERO;
pub const TOOL_BENCH_HIDDEN_POS: Vec3 = Vec3::new(0.0, -11.0, 0.0);

fn spawn_tool_bench(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing_tooltip: Query<Entity, With<ToolBench>>,
    mut tooltip_handle: Local<Option<Handle<Scene>>>,
) {
    for tooltip in &existing_tooltip {
        commands.entity(tooltip).despawn();
    }
    if tooltip_handle.is_none() {
        *tooltip_handle = Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("tools.glb")));
    }
    let tooltip = (*tooltip_handle)
        .as_ref()
        .expect("This is always loaded before");

    // spawn the tools
    let mut timer = TimerComp::from_elapsed(2.5);
    timer.0.pause();
    commands.spawn((
        SceneRoot(tooltip.clone()),
        timer,
        Transform::from_translation(TOOL_BENCH_VISIBLE_POS),
        ToolBench,
        ShowOnAlarmTime::as_false(),
        Dodgy {
            init_pos: TOOL_BENCH_VISIBLE_POS,
            last_pos: TOOL_BENCH_HIDDEN_POS,
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
    mut crop_handle: Local<Option<Handle<Scene>>>,
) {
    // hold a handle to the streelight scene between calls
    for crop in &existing_crop {
        commands.entity(crop).despawn();
    }
    if crop_handle.is_none() {
        *crop_handle = Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("crops.glb")));
    }
    let crop_scene = (*crop_handle)
        .as_ref()
        .expect("This is always loaded before");

    commands.spawn((SceneRoot(crop_scene.clone()), Crop));
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

/// Help function to check if a parent in the hierarchy has a name.
fn find_recursive_parent<'a>(
    root: Entity,
    look_for_name: &'a [&'a str],
    child_of: Query<&ChildOf>,
    names: Query<(Entity, &Name)>,
) -> Result<&'a str, ()> {
    if let Ok(child) = child_of.get(root) {
        if let Ok((ent, name)) = names.get(child.0) {
            for this_name in look_for_name {
                if name.as_str().starts_with(this_name) {
                    // base case
                    return Ok(this_name);
                } else if let Ok(found) = find_recursive_parent(ent, look_for_name, child_of, names)
                {
                    return Ok(found);
                }
            }
        }
        return Err(());
    } else {
        return Err(());
    }
}

/// Tag specific GLTF entities based with components based on their names
/// after they load.
fn tag_gltf_on_add(
    mut commands: Commands,
    new_names: Populated<(Entity, &Name, &Transform), Added<Name>>,
    mut meshes: ResMut<Assets<Mesh>>,
    child_of: Query<&ChildOf>,
    names: Query<(Entity, &Name)>,
) {
    for (entity, name, transform) in new_names.iter() {
        // cases that happen at most once on a scene are exactly matched
        match name.as_str() {
            "point_bone" => {
                commands.entity(entity).insert(KillerPoint);
            }
            "ik_target" => {
                // this is the target bone of an IK
                let dur = Duration::from_secs_f32(1.);
                let mut laser_timer = TimerComp(Timer::new(dur, TimerMode::Once));
                laser_timer.0.pause();
                let dur = Duration::from_secs_f32(10.);
                let mut killer_timer = KillerTimer {
                    timer: Timer::new(dur, TimerMode::Once),
                    can_kill: false,
                };
                killer_timer.timer.pause();
                commands.entity(entity).insert((
                    KillerHead,
                    laser_timer,
                    killer_timer,
                    DespawnOnExit(GameState::Above),
                ));
            }
            "fakebush" => {
                // fake bush that can be removed with the mining pick
                const BUSH_TRANS: Vec3 = Vec3::new(18., 2., 10.);
                const SIZE: Vec3 = Vec3::new(1.0, 4.0, 1.0);
                commands.entity(entity).insert((
                    Minable {},
                    Life::JustSpawned,
                    Collider::from_translation(BUSH_TRANS + Vec3::Y * 3., SIZE),
                    Secret,
                ));
            }
            "crop_ground_special" => {
                // invisible meshes to protect the SPECIAL diggable tile below de gnome
                commands
                    .entity(entity)
                    .insert(FakeGround)
                    .observe(was_removed_by_player)
                    .with_child((
                        Mesh3d(meshes.add(Cuboid::new(1., 4., 1.))),
                        Transform::from_translation(Vec3::new(-0.8, -0.9, 0.5)),
                        RayBlocker,
                    ))
                    .with_child((
                        Mesh3d(meshes.add(Cuboid::new(1., 4., 1.))),
                        Transform::from_translation(Vec3::new(0.8, -0.9, 0.5)),
                        RayBlocker,
                    ));
            }
            _ => {}
        };
        // multiple prefixed added entities at the same time
        // will get these components added
        if name.as_str().starts_with("main") {
            let parent_name =
                find_recursive_parent(entity, &["bush", "rig_fence", "rock"], child_of, names);
            // little wiggle on distance with player
            commands.entity(entity).insert((
                MainBone {
                    rest_rot: transform.rotation,
                    active: true,
                },
                BoneAudio::new(parent_name),
            ));
        } else if name.as_str().starts_with("one_sized") {
            commands.entity(entity).insert(OneSizeCollider);
        } else if name.as_str().starts_with("crop_ground") {
            // this if-else is outside of the match to be able
            // to add this also to "crop_ground_special"
            commands
                .entity(entity)
                .insert((Diggable {}, OneSizeCollider));
        } else if name.as_str().starts_with("dodgy") {
            // this if-else is outside of the match to be able
            // to add this also to "crop_ground_special"
            let init_pos = transform.translation;
            let last_pos = init_pos + (Vec3::Y * 20.);
            commands.entity(entity).insert((
                ShowOnAlarmTime::as_false(),
                TimerComp::from_elapsed(7.),
                Dodgy {
                    init_pos,
                    last_pos,
                    go_back: false,
                    ignore_viewing: false,
                },
            ));
        }
    }
}

#[derive(Component)]
enum BoneAudio {
    Bush,
    Rock,
    Fence,
}

impl BoneAudio {
    fn new(may_name: Result<&str, ()>) -> Self {
        match may_name {
            Ok(name) => {
                if name.starts_with("bush") {
                    Self::Bush
                } else if name.starts_with("rig_fence") {
                    Self::Fence
                } else {
                    Self::Rock
                }
            }
            _ => Self::Rock,
        }
    }

    fn to_audio(&self) -> AudioStart {
        match self {
            BoneAudio::Bush => AudioStart::Bush,
            BoneAudio::Rock => AudioStart::Rock,
            BoneAudio::Fence => AudioStart::Fence,
        }
    }
}

fn trigger_main_bone_animation(
    player: Single<&Transform, With<Player>>,
    mut audio_event: MessageWriter<AudioStart>,
    mut transforms: Query<
        (&GlobalTransform, &mut MainBone, &mut TimerComp, &BoneAudio),
        Without<Player>,
    >, // all transforms
) {
    let transform = player.into_inner();
    for (parent_t, mut main_bone, mut timer, bone_audio) in &mut transforms {
        if parent_t
            .translation()
            .distance_squared(transform.translation)
            < BUMP_DISTANCE
        {
            if timer.0.is_finished() && main_bone.active {
                timer.0.unpause();
                timer.0.reset();
                audio_event.write(bone_audio.to_audio());
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
        if !timer.0.is_finished() {
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
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &bevy::mesh::MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // draw both faces
        Ok(())
    }
}
