//! Game mechanic for a timer that forces the player to go to sleep.

use std::{
    f32::consts::{FRAC_PI_2, PI},
    time::Duration,
};

use bevy::{core_pipeline::Skybox, prelude::*, render::view::NoFrustumCulling};

use crate::{
    Capsule, ToolBench,
    config::GameState,
    dodgy::{ArchAnimation, Dodgy},
    killer_arms::KillerHead,
    player_movement::Player,
};

/// Introduces a global timer that makes the day turn into night.
/// At night, the player is hinted to go to the capsule and start again.
///
/// Also, centralizes all timer advancement.
pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Above), spawn_sun)
            .add_systems(
                Update,
                (
                    orbit_sun,
                    show_alarm,
                    lit_lamps,
                    sleep_in_capsule,
                    restart_day,
                    respawn_tooltip,
                )
                    .run_if(in_state(GameState::Above)),
            )
            .add_systems(
                PreUpdate,
                advance_timers.run_if(not(in_state(GameState::Menu))),
            );
    }
}

#[derive(Component)]
pub struct TimerComp(pub Timer);

impl TimerComp {
    pub fn from_elapsed(secs: f32) -> Self {
        let dur = Duration::from_secs_f32(secs);
        let mut timer = Timer::new(dur, TimerMode::Once);
        timer.set_elapsed(dur);
        Self(timer)
    }
}

fn advance_timers(time: Res<Time>, mut timers: Query<&mut TimerComp>) {
    for mut timer in timers.iter_mut() {
        timer.0.tick(time.delta());
    }
}

#[derive(Component)]
struct Sun {
    start_angle: f32,
    end_angle: f32,
    rad: f32,
}

/// Marker for the timer that marks the night. When finished,
/// starts to show night-only dodger elements (streetlights, etc.).
#[derive(Component)]
struct NightTimer;

/// Marker for elements that appear at night.
#[derive(Component)]
pub struct ShowOnAlarmTime {
    /// If true, time to show the lights.
    pub show: bool,
    /// If true, swap `init_pos` and `last_pos` of [`Dodgy`] next time is read.
    swap_pos: bool,
}

impl ShowOnAlarmTime {
    pub fn as_false() -> Self {
        Self {
            show: false,
            swap_pos: false,
        }
    }
}

#[derive(Component)]
struct OnlyOnNight;

fn spawn_sun(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut streetlight_handle: Local<Option<Handle<Scene>>>,
) {
    let day_secs = 160.;
    let init_pos = Vec3::new(10.0, 4.0, 30.);
    let sun_radius = 100.;
    let light = DirectionalLight {
        color: Color::Srgba(Srgba {
            red: 0.95,
            green: 0.2,
            blue: 0.4,
            alpha: 1.0,
        }),
        shadows_enabled: true,
        ..default()
    };
    let start_angle = FRAC_PI_2 - 0.4;

    commands
        .spawn((
            StateScoped(GameState::Above),
            Transform::from_translation(init_pos).looking_at(Vec3::X * 10.0, Vec3::NEG_Y),
            TimerComp(Timer::new(
                Duration::from_secs(day_secs as u64),
                TimerMode::Repeating,
            )),
            Sun {
                start_angle,
                end_angle: start_angle - 2. * PI,
                rad: sun_radius,
            },
        ))
        .with_child((light, NoFrustumCulling));
    // only for the night
    commands.spawn((
        StateScoped(GameState::Above),
        NightTimer,
        TimerComp(Timer::new(
            Duration::from_secs((day_secs * 0.52) as u64),
            TimerMode::Once,
        )),
    ));

    // Streetlight to be shown at night
    let cub = ConicalFrustum {
        radius_top: 0.45,
        radius_bottom: 0.52,
        height: 0.8,
    };
    let bulb_mesh = Mesh3d(meshes.add(cub));
    let bulb_color = Vec3::new(0.6, 0.8, 0.6);
    let bulb_color_more = Vec3::new(30., 30., 30.);
    let bulb_material = MeshMaterial3d(materials.add(StandardMaterial {
        base_color: Color::srgb_from_array(bulb_color.into()),
        emissive: LinearRgba::rgb(bulb_color_more.x, bulb_color_more.y, bulb_color_more.z),
        unlit: true,
        diffuse_transmission: 1.0,
        ..default()
    }));
    // hold a handle to the streelight scene between calls
    if streetlight_handle.is_none() {
        *streetlight_handle = Some(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("streetlight.gltf#Streetlight")),
        );
    }
    let streetlight = (*streetlight_handle)
        .as_ref()
        .expect("This is always loaded before");
    for init_pos in [
        Vec3::new(-7.0, -11., -8.0),
        Vec3::new(29.5, -11., 8.7),
        Vec3::new(29.5, -11., -8.),
    ] {
        let mut timer = TimerComp::from_elapsed(2.5);
        timer.0.pause();
        commands.spawn((
            Dodgy {
                init_pos,
                last_pos: init_pos + Vec3::Y * 11.,
                go_back: false,
                ignore_viewing: false,
            },
            StateScoped(GameState::Above),
            ShowOnAlarmTime::as_false(),
            // since the light would disappear if not looking at it
            timer,
            Transform::from_translation(init_pos),
            SceneRoot(streetlight.clone()),
            children![(
                Visibility::Hidden,
                NoFrustumCulling,
                OnlyOnNight,
                SpotLight {
                    color: Color::srgb(0.98, 0.93, 0.5),
                    intensity: 200_000.0,
                    // avoid casting shadows over the streetlight mesh
                    shadow_depth_bias: 3.0,
                    range: 30., // penumbra size
                    inner_angle: 90_f32.to_radians(),
                    outer_angle: 90_f32.to_radians(),
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(0., 8.3, 0.).looking_at(Vec3::Y * -8.3, Vec3::NEG_Y),
                children![(
                    bulb_material.clone(),
                    bulb_mesh.clone(),
                    // cancel rotation of parent light
                    Transform::from_rotation(Quat::from_rotation_x(-PI / 2.)),
                )]
            )],
        ));
    }

    // commands.spawn((
    //     Text::new("0"),
    //     TextFont {
    //         font: asset_server.load("fonts/Silkscreen-Bold.ttf"),
    //         font_size: 15.0,
    //         ..default()
    //     },
    //     TextColor(Color::WHITE),
    //     TextShadow::default(),
    //     ReportAngle,
    // ));
}

#[derive(Component)]
struct ReportAngle;

/// Move the sun, keeping the radius around the Z origin and moving only Z and Y.
fn orbit_sun(
    mut sun_query: Query<(&mut Transform, &Sun, &TimerComp, &Children)>,
    mut lights: Query<&mut DirectionalLight>,
    mut skybox: Query<&mut Skybox>,
    // mut rep: Single<&mut Text, With<ReportAngle>>,
) {
    let Ok((mut trans, sun, timer, children)) = sun_query.single_mut() else {
        return;
    };
    let mut light = None;
    for child in children {
        light = lights.get_mut(*child).ok();
        break;
    }

    let Some(mut light) = light else {
        return;
    };
    let u = timer.0.fraction();
    let theta = sun.start_angle + u * (sun.end_angle - sun.start_angle);

    // YZ-plane parametric circle
    let new_y = sun.rad * theta.cos();
    let new_z = sun.rad * theta.sin();
    let new_tr = Vec3::new(trans.translation.x, new_y, new_z);

    *trans = Transform::from_translation(new_tr).looking_at(Vec3::X * 10.0, Vec3::NEG_Y);

    // - 0.00-0.60 -> “day”    (bright light-red)
    // - 0.60-0.85 -> “sunset” (orange)
    // - 0.85-1.00 -> “night”  (deep blue)

    const MIDDAY_COLOUR: Vec3 = Vec3::new(0.8, 0.20, 0.40); // bright light-red
    const DAWN_COLOUR: Vec3 = Vec3::new(1.0, 0., 0.); // blue

    const DAY_END: f32 = 0.44; // 60 % of the timer → end of “day”
    const SUNSET_END: f32 = 0.52; // 85 % of the timer → end of “sunset”
    const DAY_UP: f32 = 0.95; // the sun comes up again

    // [PI, 0] and [0, -PI] -> [0, 1] and [1, 0]
    let polar_u = ((PI - if theta < 0. { -theta } else { theta }) / PI).clamp(0., 1.);
    let theta_phi = (1. + (theta % (2. * PI)).cos()) / 2.;
    // rep.0 = format!("u=[{u:.2}]; Light=[{:.2}]", light.illuminance);

    // Day -> hold the bright-red colour
    let rgb = DAWN_COLOUR.lerp(MIDDAY_COLOUR, polar_u);
    light.color = Color::linear_rgb(rgb.x, rgb.y, rgb.z);

    // Dim the light at night so shadows disappear
    // full strength by day, 25 % at sunset,  5 % at night.
    let light_multiplier = if u < DAY_END || u > DAY_UP {
        1.0
    } else {
        1.0 - 0.95 * (u - DAY_END) / (SUNSET_END - DAY_END)
    };
    light.illuminance = 10_000.0 * light_multiplier;

    // rotate skybox
    if let Ok(mut sky) = skybox.single_mut() {
        sky.rotation = Quat::from_rotation_x(theta);
        sky.brightness = if u < DAY_END || u > DAY_UP {
            1500. * theta_phi + 20.
        } else {
            (1500. * theta_phi * light_multiplier).max(20.)
        };
    }
}

// NIGHT LOGIC

/// Show [`Dodgy`] elements when the night is near and it's time to
/// go back to the capsule.
fn show_alarm(
    mut commands: Commands,
    timer: Query<&TimerComp, With<NightTimer>>,
    mut dodgers: Query<
        (
            &mut TimerComp,
            &mut Dodgy,
            &mut ShowOnAlarmTime,
            Option<&ToolBench>,
        ),
        Without<NightTimer>,
    >,
    mut capsule: Single<(Entity, &Transform, &mut Capsule)>,
) {
    let Ok(alarm) = timer.single() else {
        return;
    };
    if alarm.0.just_finished() {
        for (mut dodgy_timer, mut dodgy, mut show, maybe_tools) in dodgers.iter_mut() {
            dodgy_timer.0.reset();
            dodgy_timer.0.unpause();
            show.show = true;
            // special case, don't swap for the ToolBench if it is not already up
            if maybe_tools.is_none() || dodgy.last_pos.y > dodgy.init_pos.y {
                if show.swap_pos {
                    *dodgy = Dodgy {
                        init_pos: dodgy.last_pos,
                        last_pos: dodgy.init_pos,
                        go_back: dodgy.go_back,
                        ignore_viewing: dodgy.ignore_viewing,
                    };
                }
            }
        }
        // show capsule.
        let mut cmd = commands.entity(capsule.0);
        cmd.remove::<TimerComp>();
        cmd.remove::<ArchAnimation>();
        let trans = capsule.1.translation;
        cmd.insert(Dodgy {
            init_pos: trans,
            last_pos: trans + Vec3::Y * 10.,
            go_back: false,
            ignore_viewing: false,
        });
        capsule.2.active = true;
    }
}

fn sleep_in_capsule(
    mut next_state: ResMut<NextState<GameState>>,
    mut capsule: Single<(&Transform, &mut Capsule), Without<Player>>,
    player_q: Query<&Transform, (Without<Capsule>, With<Player>)>,
) {
    if capsule.1.active {
        if let Ok(player_trans) = player_q.single() {
            if player_trans
                .translation
                .distance_squared(capsule.0.translation)
                < 2.
            {
                next_state.set(GameState::Menu);
                capsule.1.active = false;
            }
        }
    }
}

fn lit_lamps(
    show_parents: Query<(&TimerComp, &Children, &ShowOnAlarmTime)>,
    mut lamps: Query<&mut Visibility, With<SpotLight>>,
) {
    for (timer, children, show) in show_parents {
        // using just_finished alone is unreliable
        if timer.0.just_finished() && show.show {
            for child in children {
                if let Ok(mut vis) = lamps.get_mut(*child) {
                    vis.toggle_visible_hidden();
                }
            }
        }
    }
}

// DAY LOGIC

/// All the logic when a a day is restarted:
///
/// * Night timer restarts.
/// * ShowOnAlarmTime are hidden.
/// * Capsule is hidden.
/// * Tools are replenished.
/// * Laser Timers are restarted.
fn restart_day(
    mut commands: Commands,
    sun_timer: Single<&TimerComp, (With<Sun>, Without<NightTimer>, Without<KillerHead>)>,
    mut night_timer: Single<&mut TimerComp, (With<NightTimer>, Without<Sun>, Without<KillerHead>)>,
    mut dodgers: Query<
        (
            &mut TimerComp,
            &mut ShowOnAlarmTime,
            &mut Dodgy,
            &Children,
            Option<&ToolBench>,
        ),
        (Without<NightTimer>, Without<Sun>, Without<KillerHead>),
    >,
    mut lamps: Query<&mut Visibility, (With<SpotLight>, With<OnlyOnNight>)>,
    mut capsule: Single<(Entity, &Transform, &mut Capsule)>,
    mut killers: Query<&mut TimerComp, (With<KillerHead>, Without<NightTimer>, Without<Sun>)>,
) {
    if sun_timer.0.just_finished() {
        night_timer.0.reset();
        // hide all dodgy elements that where shown on sun (`ShowOnAlarmTime`)
        for (mut timer, mut show, mut dodgy, children, maybe_tools) in &mut dodgers {
            show.show = false;
            // special case, don't swap for the ToolBench
            if maybe_tools.is_none() {
                *dodgy = Dodgy {
                    init_pos: dodgy.last_pos,
                    last_pos: dodgy.init_pos,
                    go_back: dodgy.go_back,
                    ignore_viewing: dodgy.ignore_viewing,
                };
            }

            // if the show up again, sawp init_pos and last_pos again
            show.swap_pos = true;
            // hide them
            timer.0.reset();
            timer.0.unpause();
            for child in children {
                if let Ok(mut vis) = lamps.get_mut(*child) {
                    vis.toggle_visible_hidden();
                }
            }
        }

        // hide capsule
        let mut cmd = commands.entity(capsule.0);
        cmd.remove::<TimerComp>();
        cmd.remove::<Dodgy>();
        let trans = capsule.1.translation;
        commands.entity(capsule.0).insert((
            ArchAnimation {
                init_pos: trans,
                last_pos: trans - Vec3::Y * 10.,
                peak_y: 5.,
            },
            TimerComp(Timer::from_seconds(2.0, TimerMode::Once)),
        ));
        capsule.2.active = true;

        // deactivate lasers
        for mut laser_timer in &mut killers {
            laser_timer.0.pause();
            laser_timer.0.reset();
        }
    }
}

fn respawn_tooltip(
    mut commands: Commands,
    existing_tooltip: Single<(Entity, &TimerComp, &ShowOnAlarmTime), With<ToolBench>>,
    asset_server: Res<AssetServer>,
    mut tooltip_handle: Local<Option<Handle<Scene>>>,
) {
    if existing_tooltip.1.0.just_finished() && existing_tooltip.2.show {
        if tooltip_handle.is_none() {
            *tooltip_handle =
                Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("tools.glb")));
        }
        commands.entity(existing_tooltip.0).despawn();
        let mut timer = TimerComp::from_elapsed(2.5);
        timer.0.pause();
        // this is the initial position of the scene
        // the tooltip inside the scene is put to match bush.gltf
        let init_pos = Vec3::new(0., -11., 0.);
        commands.spawn((
            SceneRoot((*tooltip_handle).as_ref().expect("works").clone()),
            timer,
            Transform::from_translation(init_pos),
            ToolBench,
            ShowOnAlarmTime::as_false(),
            Dodgy {
                init_pos,
                last_pos: init_pos + Vec3::Y * 11.,
                go_back: false,
                ignore_viewing: false,
            },
        ));
    }
}
