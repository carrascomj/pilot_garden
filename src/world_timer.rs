//! Game mechanic for a timer that forces the player to go to sleep.

use std::{
    f32::consts::{FRAC_PI_4, PI},
    time::Duration,
};

use bevy::{
    prelude::*,
    render::{
        mesh::{SphereKind, SphereMeshBuilder},
        view::NoFrustumCulling,
    },
};

use crate::{config::GameState, dodgy::Dodgy};

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
                ((orbit_sun, show_alarm, lit_lamps).run_if(not(in_state(GameState::Menu))),),
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

#[derive(Component)]
struct ShowOnAlarmTime(bool);

fn spawn_sun(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let day_secs = 120.;
    let init_pos = Vec3::new(10.0, 4.0, 30.);
    let hyp = (init_pos.z * init_pos.z + init_pos.y * init_pos.y).sqrt();
    commands.spawn((
        StateScoped(GameState::Above),
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
        Transform::from_translation(init_pos).looking_at(Vec3::X * 10.0, Vec3::NEG_Y),
        TimerComp(Timer::new(
            Duration::from_secs(day_secs as u64),
            TimerMode::Once,
        )),
        Sun {
            start_angle: FRAC_PI_4,
            end_angle: -PI + 1.5,
            rad: hyp,
        },
    ));

    // Streetlight to be shown at night
    let sphere = SphereMeshBuilder::new(0.4, SphereKind::Ico { subdivisions: 4 }).build();
    let bulb_mesh = Mesh3d(meshes.add(sphere));
    let bulb_color = Vec3::new(0.8, 0.8, 0.8);
    let bulb_color_more = Vec3::new(0.95, 0.93, 0.5);
    let bulb_material = MeshMaterial3d(materials.add(StandardMaterial {
        base_color: Color::srgb_from_array(bulb_color.into()),
        emissive: LinearRgba::rgb(bulb_color_more.x, bulb_color_more.y, bulb_color_more.z),
        unlit: true,
        diffuse_transmission: 1.0,
        ..default()
    }));

    for init_pos in [Vec3::new(-6.0, -11., -8.0), Vec3::new(29.5, -11., 10.0)] {
        let mut timer = TimerComp::from_elapsed(2.5);
        timer.0.pause();
        commands
            .spawn((
                Dodgy {
                    init_pos,
                    last_pos: init_pos + Vec3::Y * 11.,
                    go_back: false,
                },
                ShowOnAlarmTime(false),
                // since the light would disappear if not looking at it
                NoFrustumCulling,
                timer,
                Transform::from_translation(init_pos),
                SceneRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("streetlight.gltf#Streetlight")),
                ),
            ))
            .with_child((
                Visibility::Hidden,
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
                bulb_material.clone(),
                bulb_mesh.clone(),
                Transform::from_xyz(0., 8.3, 0.).looking_at(Vec3::Y * -8.3, Vec3::NEG_Y),
            ));
    }
}

/// Move the sun, keeping the radius around the Z origin and moving only Z and Y.
fn orbit_sun(mut sun_query: Query<(&mut Transform, &Sun, &TimerComp, &mut DirectionalLight)>) {
    let Ok((mut trans, sun, timer, mut light)) = sun_query.single_mut() else {
        return;
    };

    let u = timer.0.fraction();
    let theta = sun.start_angle + u * (sun.end_angle - sun.start_angle);

    // YZ-plane parametric circle
    let new_y = sun.rad * theta.cos();
    let new_z = sun.rad * theta.sin();
    let new_tr = Vec3::new(trans.translation.x, new_y, new_z);

    *trans = Transform::from_translation(new_tr).looking_at(Vec3::X * 10.0, Vec3::NEG_Y);

    // - 0.00-0.60  → “day”    (bright light-red)
    // - 0.60-0.85  → “sunset” (orange)
    // - 0.85-1.00  → “night”  (deep blue)

    const DAY_COLOUR: Vec3 = Vec3::new(0.95, 0.20, 0.40); // bright light-red
    const SUNSET_COLOUR: Vec3 = Vec3::new(1.00, 0.55, 0.10); // orange
    const NIGHT_COLOUR: Vec3 = Vec3::new(0.10, 0.15, 0.55); // blue

    const DAY_END: f32 = 0.60; // 60 % of the timer → end of “day”
    const SUNSET_END: f32 = 0.85; // 85 % of the timer → end of “sunset”

    let rgb = if u < DAY_END {
        // Day → hold the bright-red colour (or lerp to something else if you like)
        DAY_COLOUR
    } else if u < SUNSET_END {
        // Day → Sunset
        let t = (u - DAY_END) / (SUNSET_END - DAY_END); // 0‥1
        DAY_COLOUR.lerp(SUNSET_COLOUR, t)
    } else {
        // Sunset → Night
        let t = (u - SUNSET_END) / (1.0 - SUNSET_END); // 0‥1
        SUNSET_COLOUR.lerp(NIGHT_COLOUR, t)
    };
    light.color = Color::linear_rgb(rgb.x, rgb.y, rgb.z);

    // Dim the light at night so shadows disappear
    // full strength by day, 25 % at sunset,  5 % at night.
    light.illuminance = 10_000.0
        * if u < DAY_END {
            1.0
        } else if u < SUNSET_END {
            1.0 - 0.75 * (u - DAY_END) / (SUNSET_END - DAY_END)
        } else {
            0.25 - 0.20 * (u - SUNSET_END) / (1.0 - SUNSET_END)
        }
        .max(0.05); // never pitch-black unless you want it
}

/// Show [`Dodgy`] elements when the night is near and it's time to
/// go back to the capsule.
fn show_alarm(
    timer: Query<&TimerComp, With<Sun>>,
    mut dodgers: Query<(&mut TimerComp, &mut ShowOnAlarmTime), Without<Sun>>,
) {
    let Ok(alarm) = timer.single() else {
        return;
    };
    if alarm.0.just_finished() {
        for (mut dodgy_timer, mut show) in dodgers.iter_mut() {
            dodgy_timer.0.reset();
            dodgy_timer.0.unpause();
            show.0 = true;
        }
    }
}

fn lit_lamps(
    show_parents: Query<(&TimerComp, &Children, &ShowOnAlarmTime)>,
    mut lamps: Query<&mut Visibility, With<SpotLight>>,
) {
    for (timer, children, show) in show_parents {
        // using just_finished alone is unreliable
        if timer.0.just_finished() && show.0 {
            for child in children {
                if let Ok(mut vis) = lamps.get_mut(*child) {
                    *vis = Visibility::Visible;
                }
            }
        }
    }
}
