//! Setup for cameras that render to a texture, as surveillance cameras.

use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4, PI},
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
    GameOverRemove,
    audio::{AudioStart, DrumsToStop},
    config::GameState,
    digging::{Collectible, OnHand},
    gnomes::GnomeMachine,
    world_timer::TimerComp,
};

/// Spawn some cameras, set them up to render to their screens (textures)
/// and control the change given buttons
pub struct SurveillancePlugin;

impl Plugin for SurveillancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderMaterials>()
            .add_event::<ButtonActivated>()
            .add_event::<TurnTheLights>()
            .add_systems(
                OnEnter(GameState::Below),
                (setup_surveillance_camera, setup_spotlights_below),
            )
            .add_systems(Startup, setup_surveillance_screenshots)
            .add_systems(OnEnter(GameState::Above), link_screens_to_above)
            .add_systems(Update, take_snapshots.run_if(in_state(GameState::Above)))
            .add_systems(
                Update,
                (show_player_on_screen, switch_lights).run_if(in_state(GameState::Below)),
            );
    }
}

/// Marker for big screens in the studio that can be changed
/// to focus themselves when pressing the button.
#[derive(Component)]
pub struct BigScreen(usize);
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
) {
    let mut to_above: [Option<Handle<StandardMaterial>>; 4] = [None, None, None, None];
    for (screen_rot, screen_size, screen_pos, ab_idx) in [
        (
            Quat::from_rotation_z(0.),
            [20.8, 10.7],
            Vec3::new(-78.3, -17., 80.59),
            0,
        ),
        (
            Quat::from_rotation_x(FRAC_PI_4),
            [20.8, 5.2],
            Vec3::new(-76.457, -8.63, 80.59),
            // Vec3::new(0., 10., 0.),
            1,
        ),
        (
            Quat::from_rotation_y(-FRAC_PI_4),
            [7.17, 10.],
            Vec3::new(-75.6, -17., 66.4),
            // Vec3::new(0., 10., 0.),
            2,
        ),
        (
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
        to_above[ab_idx] = Some(materials.add(StandardMaterial {
            base_color_texture: Some(image_handle.clone()),
            reflectance: 0.02,
            unlit: true,
            ..default()
        }));
        // spawn the plane with the material containing the texture
        let screen = Rectangle::new(screen_size[0], screen_size[1]);
        let quad_handle = meshes.add(screen);
        let screen_trans = Transform::from_translation(screen_pos)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2) * screen_rot);
        let material = to_above[ab_idx].as_ref().unwrap();

        commands.spawn((
            Mesh3d(quad_handle),
            MeshMaterial3d(material.clone()),
            screen_trans,
            BigScreen(ab_idx),
        ));
        // two screens in the safe zone
        if ab_idx == 1 {
            let quad_handle = meshes.add(Rectangle::new(3.4, 1.85));
            let screen_trans = Transform::from_xyz(23.121, 2.4487, 13.8);

            commands.spawn((
                Mesh3d(quad_handle),
                MeshMaterial3d(material.clone()),
                screen_trans,
                BigScreen(ab_idx),
            ));
        } else if ab_idx == 3 {
            let quad_handle = meshes.add(Rectangle::new(3.4, 1.85));
            let screen_trans = Transform::from_xyz(23.121, 0.041005, 13.8);

            commands.spawn((
                Mesh3d(quad_handle),
                MeshMaterial3d(material.clone()),
                screen_trans,
                BigScreen(ab_idx),
            ));
        }
    }

    let mut timer = Timer::from_seconds(30., TimerMode::Once);
    timer.set_elapsed(Duration::from_secs(22));
    let take_timer = Timer::from_seconds(0.1, TimerMode::Once);
    commands.spawn(SnapshotTimer {
        no_snapshot: timer,
        take_timer,
    });
    commands.insert_resource(RenderMaterials {
        to_button: None,
        to_above: to_above,
    });
}

/// Link the rendering target from the above [`Snapshoter`] cameras
/// to the screens.
///
/// Will run only if the snapshot cameras do not exist (on Startup and after
/// going to below, which removes them).
fn link_screens_to_above(
    mut commands: Commands,
    render_materials: Res<RenderMaterials>,
    screens: Query<(Entity, &BigScreen)>,
    materials: Res<Assets<StandardMaterial>>,
    curr_snapshoters: Query<&Snapshoter>,
) {
    const SNAP: [(Vec3, Vec3); 4] = [
        (Vec3::new(10., 30., 1.), Vec3::new(10., -30., 1.)),
        (Vec3::new(-9.699, 5., 6.4251), Vec3::new(23., 3., -4.)),
        (Vec3::new(18., 5., 7.), Vec3::new(27., 1., 8.2)),
        (Vec3::new(17., 5.2, 11.), Vec3::new(24.202, 1., -6.52)),
    ];
    if !curr_snapshoters.is_empty() {
        return;
    }
    let mut cam_not_setup = [true, true, true, true];
    for (screen_ent, BigScreen(idx)) in screens {
        let material = render_materials.to_above[*idx].as_ref().unwrap();
        commands
            .entity(screen_ent)
            .insert(MeshMaterial3d(material.clone()));

        if cam_not_setup[*idx] {
            cam_not_setup[*idx] = false;
            let Some(Some(image_handle)) = materials
                .get(material)
                .map(|m| m.base_color_texture.clone())
            else {
                return;
            };
            let mut cam_trans =
                Transform::from_translation(SNAP[*idx].0).looking_at(SNAP[*idx].1, Vec3::Y);
            if *idx == 0 {
                cam_trans.rotate(Quat::from_rotation_y(FRAC_PI_2));
            }
            commands.spawn((
                Snapshoter,
                Camera3d::default(),
                Exposure { ev100: 4. },
                Camera {
                    target: image_handle.into(),
                    clear_color: Color::WHITE.into(),
                    is_active: false,
                    ..default()
                },
                cam_trans,
            ));
        }
    }
}

/// This is the camera that looks at the button and replaces
/// all screens when the button is pressed.
fn setup_surveillance_camera(
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
        GameOverRemove,
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
        GameOverRemove,
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
        GameOverRemove,
        Transform::from_xyz(-65., -23.8, 80.),
        Collectible::Button,
        OnHand::new(),
        TimerComp::from_elapsed(1.),
        children!((button_material, button_mesh,)),
    ));
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

#[derive(Component)]
pub struct SwitchableLight(Timer);

/// Switchable.
fn setup_spotlights_below(mut commands: Commands) {
    let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
    timer.pause();
    commands.spawn((
        GameOverRemove,
        SpotLight {
            color: Color::srgb(1.0, 1.0, 1.0),
            intensity: 1000_000.0,
            // avoid casting shadows over the streetlight mesh
            shadow_depth_bias: 1.0,
            range: 100., // penumbra size
            outer_angle: PI,
            shadows_enabled: true,
            ..default()
        },
        SwitchableLight(timer),
        Transform::from_xyz(-45., -12., 86.).looking_to(Vec3::NEG_Y, Vec3::Y),
    ));
}

#[derive(Event)]
/// Big button below that
///
/// * [x] activates the lights;
/// * [x] changes cameras; and
/// * [x] activates gnomes.
pub struct ButtonActivated;
#[derive(Event)]
pub enum TurnTheLights {
    On,
    Off,
}

/// Make the big screens show the camera looking at the player.
fn show_player_on_screen(
    mut button_reader: EventReader<ButtonActivated>,
    mut light_switch_writer: EventWriter<TurnTheLights>,
    mut commands: Commands,
    render_materials: Res<RenderMaterials>,
    mut big_screens: Query<Entity, With<BigScreen>>,
    mut gnomes: Query<&mut GnomeMachine>,
) {
    for _ev in button_reader.read() {
        for big_screen in &mut big_screens {
            commands.entity(big_screen).insert(MeshMaterial3d(
                render_materials.to_button.as_ref().unwrap().clone(),
            ));
        }
        light_switch_writer.write(TurnTheLights::On);
        for mut gnome in &mut gnomes {
            gnome.waiting_for_attack();
        }
    }
}

fn switch_lights(
    mut commands: Commands,
    time: Res<Time>,
    mut ligth_switch_reader: EventReader<TurnTheLights>,
    mut audio_event: EventWriter<AudioStart>,
    mut lights: Query<(&mut Visibility, &mut SwitchableLight)>,
    mut gnomes: Query<&mut GnomeMachine>,
    mut ambient_light: ResMut<AmbientLight>,
    drums_audio: Query<Entity, With<DrumsToStop>>,
) {
    for (mut light, mut timer) in &mut lights {
        timer.0.tick(time.delta());
        for ev in ligth_switch_reader.read() {
            let (vis, inactivate) = match ev {
                TurnTheLights::On => (Visibility::Visible, false),
                TurnTheLights::Off => (Visibility::Hidden, true),
            };
            timer.0.unpause();
            timer.0.reset();
            *light = vis;
            if inactivate {
                audio_event.write(AudioStart::SwitchOff);
                // so that the gnomes won't attack the player while
                // the light is off
                audio_event.write(AudioStart::DrumChase);
                for mut gnome in &mut gnomes {
                    gnome.deactivate();
                }
            } else {
                for audio in &drums_audio {
                    commands.entity(audio).despawn();
                }
                audio_event.write(AudioStart::SwitchOn);
            }
        }
        if !timer.0.finished() {
            let u = match *light {
                Visibility::Visible => timer.0.fraction() + 0.05,
                _ => timer.0.fraction_remaining() - 0.05,
            };
            ambient_light.brightness = u.clamp(0., 1.) * 200.;
        }
    }
}
