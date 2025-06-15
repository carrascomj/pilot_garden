//! Setup for cameras that render to a texture, as surveillance cameras.

use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4},
    time::Duration,
};

use bevy::{
    prelude::*,
    render::{
        camera::{CameraOutputMode, Exposure},
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};

use crate::{
    config::GameState,
    digging::{Collectible, OnHand},
    world_timer::TimerComp,
};

/// Spawn some cameras, set them up to render to their screens (textures)
/// and control the change given buttons
pub struct SurveillancePlugin;

impl Plugin for SurveillancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderMaterials>()
            .add_event::<ButtonActivated>()
            .add_systems(OnEnter(GameState::Below), setup_surveillance_cameras)
            .add_systems(OnExit(GameState::Menu), setup_surveillance_screenshots)
            .add_systems(Update, take_snapshots.run_if(in_state(GameState::Above)))
            .add_systems(
                Update,
                show_player_on_screen.run_if(in_state(GameState::Below)),
            );
    }
}

/// Marker for big screens in the studio that can be changed
/// to focus themselves when pressing the button.
#[derive(Component)]
pub struct BigScreen;
/// Marker for the camera that is looking at the button, only
/// active after the player presses the button.
#[derive(Component)]
pub struct LookingAtButton;
/// Marker for cameras that are only active for a frame every N
/// seconds to take snapshots and put them in screens.
#[derive(Component)]
pub struct Snapshoter;
#[derive(Component)]
pub struct SnapshotTimer {
    no_snapshot: Timer,
    take_timer: Timer,
}

#[derive(Resource, Default)]
struct RenderMaterials {
    to_button: Option<Handle<StandardMaterial>>,
    to_above: [Option<Handle<StandardMaterial>>; 4],
}

/// Spawn cameras that take screenshot every 30s, inactive otherwise.
/// This is much computationally cheaper than running N cameras
/// all the time.
fn setup_surveillance_screenshots(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut render_materials: ResMut<RenderMaterials>,
) {
    for (cam_pos, cam_target, screen_rot, screen_size, screen_pos, ab_idx) in [
        (
            Vec3::new(10., 30., 1.),
            Vec3::new(10., -30., 1.),
            Quat::from_rotation_z(0.),
            [20.8, 10.7],
            Vec3::new(-78.3, -17., 80.59),
            0,
        ),
        (
            Vec3::new(-9.699, 5., 6.4251),
            Vec3::new(23., 3., -4.),
            Quat::from_rotation_x(FRAC_PI_4),
            [20.8, 5.2],
            Vec3::new(-76.457, -8.63, 80.59),
            // Vec3::new(0., 10., 0.),
            1,
        ),
        (
            Vec3::new(18., 5., 7.),
            Vec3::new(27., 1., 8.2),
            Quat::from_rotation_y(-FRAC_PI_4),
            [7.17, 10.],
            Vec3::new(-75.6, -17., 66.4),
            // Vec3::new(0., 10., 0.),
            2,
        ),
        (
            Vec3::new(17., 5.2, 11.),
            Vec3::new(24.202, 1., -6.52),
            Quat::from_rotation_y(FRAC_PI_4),
            [7.17, 10.],
            Vec3::new(-75.6, -17., 94.5),
            // Vec3::new(0., 10., 0.),
            3,
        ),
    ] {
        // big screen below
        let size = if ab_idx < 2 {
            Extent3d {
                width: 1024,
                height: 512,
                ..default()
            }
        } else {
            Extent3d {
                width: 350,
                height: 500,
                ..default()
            }
        };
        let mut image = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let image_handle = images.add(image);
        if render_materials.to_above[ab_idx].is_none() {
            render_materials.to_above[ab_idx] = Some(materials.add(StandardMaterial {
                base_color_texture: Some(image_handle.clone()),
                reflectance: 0.02,
                unlit: true,
                ..default()
            }));
        }
        let mut cam_trans = Transform::from_translation(cam_pos).looking_at(cam_target, Vec3::Y);
        if ab_idx == 0 {
            cam_trans.rotate(Quat::from_rotation_y(FRAC_PI_2));
        }
        commands.spawn((
            Snapshoter,
            Camera3d::default(),
            Exposure { ev100: 4. },
            Camera {
                target: image_handle.clone().into(),
                clear_color: Color::WHITE.into(),
                is_active: false,
                ..default()
            },
            cam_trans,
        ));
        // spawn the plane with the material containing the texture
        let screen = Rectangle::new(screen_size[0], screen_size[1]);
        let quad_handle = meshes.add(screen);
        let screen_trans = Transform::from_translation(screen_pos)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2) * screen_rot);

        commands.spawn((
            Mesh3d(quad_handle),
            MeshMaterial3d(render_materials.to_above[ab_idx].as_ref().unwrap().clone()),
            screen_trans,
            BigScreen,
        ));
    }

    let mut timer = Timer::from_seconds(30., TimerMode::Once);
    timer.set_elapsed(Duration::from_secs(22));
    let take_timer = Timer::from_seconds(0.1, TimerMode::Once);
    commands.spawn(SnapshotTimer {
        no_snapshot: timer,
        take_timer,
    });
}

fn setup_surveillance_cameras(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut render_materials: ResMut<RenderMaterials>,
    mut sur_cameras: Query<Entity, With<Snapshoter>>,
) {
    // remove all surveillance snapshot cameras since we don't need
    // them anymore
    for ent in &mut sur_cameras {
        commands.entity(ent).despawn();
    }

    // big screen below
    let size = Extent3d {
        width: 2048,
        height: 1024,
        ..default()
    };

    // Material with the texture that is looking at the button
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // need to set these texture usage flags in order to use the image as a render target
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image_handle = images.add(image);

    if render_materials.to_button.is_none() {
        render_materials.to_button = Some(materials.add(StandardMaterial {
            base_color_texture: Some(image_handle.clone()),
            reflectance: 0.02,
            unlit: true,
            ..default()
        }));
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            target: image_handle.clone().into(),
            clear_color: Color::WHITE.into(),
            ..default()
        },
        LookingAtButton,
        // looking at the central big screen
        Transform::from_xyz(-55., -20., 86.).looking_at(Vec3::new(-67., -25., 80.), Vec3::Y),
    ));

    // the button light
    commands.spawn((
        SpotLight {
            color: Color::Srgba(Srgba {
                red: 0.957,
                green: 0.668,
                blue: 0.668,
                alpha: 1.0,
            }),
            shadows_enabled: true,
            intensity: 500_000.,
            range: 10.0,
            ..default()
        },
        Transform::from_translation(Vec3::new(-63., -20., 80.))
            .looking_at(Vec3::new(-65., -25., 80.59), Vec3::Y),
    ));
    // the button itself, on top of the button holder
    let button_mesh = Mesh3d(meshes.add(Cylinder::new(0.2, 0.2)));
    let button_material = MeshMaterial3d(materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.3, 0.3),
        ..default()
    }));
    commands.spawn((
        Transform::from_xyz(-65., -23.8, 80.),
        Collectible::Button,
        OnHand::new(),
        TimerComp::from_elapsed(1.),
        children!((button_material, button_mesh,)),
    ));
}

#[derive(Event)]
/// Big button below that
///
/// * [] activates the lights;
/// * [x] changes cameras; and
/// * [] activates gnomes.
pub struct ButtonActivated;

/// Make the big screens show the camera looking at the player.
fn show_player_on_screen(
    mut event_reader: EventReader<ButtonActivated>,
    mut commands: Commands,
    render_materials: Res<RenderMaterials>,
    mut big_screens: Query<Entity, With<BigScreen>>,
) {
    for _ev in event_reader.read() {
        for big_screen in &mut big_screens {
            commands.entity(big_screen).insert(MeshMaterial3d(
                render_materials.to_button.as_ref().unwrap().clone(),
            ));
        }
    }
}

/// Make the [`Snapshoter`] cameras active from time to
/// time to put the camera in the screen.
fn take_snapshots(
    time: Res<Time>,
    mut snap_timer: Single<&mut SnapshotTimer>,
    mut sur_cameras: Query<&mut Camera, With<Snapshoter>>,
) {
    snap_timer.no_snapshot.tick(time.delta());
    snap_timer.take_timer.tick(time.delta());
    if snap_timer.no_snapshot.just_finished() {
        snap_timer.take_timer.reset();
        snap_timer.no_snapshot.reset();
    }
    if !snap_timer.take_timer.finished() {
        for mut cam in &mut sur_cameras {
            if !cam.is_active {
                cam.is_active = true;
                cam.output_mode = CameraOutputMode::default();
            }
        }
    } else {
        for mut cam in &mut sur_cameras {
            if cam.is_active {
                cam.is_active = false;
                cam.output_mode = CameraOutputMode::Skip;
            }
        }
    }
}
