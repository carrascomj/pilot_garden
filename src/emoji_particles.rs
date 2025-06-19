//! 2D particle system for emojis (streaming-like?), on
//! performed irrelevant actions that the "viewers" would like.

use bevy::prelude::*;
use fastrand::Rng;

use crate::config::GameState;

/// 2D camera overlaid on top of the 3D camera that shows a particle
/// system of emojis. The emojis are all spawned out of the screen
/// and shown when the particle physics allow.
pub struct EmojiPlugin;

impl Plugin for EmojiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleNoise>()
            .init_resource::<ParticleTimer>()
            .init_resource::<SecretRevealed>()
            .add_event::<EmojiBurst>()
            .add_systems(Startup, setup_emoji_particles)
            .add_systems(
                FixedUpdate,
                (
                    receive_particle_velocity,
                    apply_delayed_velocity,
                    resolve_particle_physics,
                )
                    .run_if(not(in_state(GameState::Menu)))
                    .run_if(|secret_rev: Res<SecretRevealed>| !secret_rev.0),
            )
            .add_observer(
                |_trig: Trigger<OnRemove, Secret>, mut sc: ResMut<SecretRevealed>| {
                    println!("hello secret");
                    sc.0 = true;
                },
            );
    }
}

fn setup_emoji_particles(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    window: Single<&Window>,
) {
    commands.spawn((
        Camera {
            hdr: false,
            // Camera3D is 0, this is rendered later, on top
            order: 1,
            ..Default::default()
        },
        Camera2d::default(),
    ));
    const EMOJI_PATHS: [&str; 11] = [
        "emojis/banana.png",
        "emojis/bananas.png",
        "emojis/confetti.png",
        "emojis/heart.png",
        "emojis/heart_eyes.png",
        "emojis/impressed_left.png",
        "emojis/laugh_left.png",
        "emojis/pick.png",
        "emojis/smile_right.png",
        "emojis/thumbs_up.png",
        "emojis/wide_smile.png",
    ];

    let emoji_handles: smallvec::SmallVec<[_; 11]> = EMOJI_PATHS
        .iter()
        .map(|path| asset_server.load(*path))
        .collect();
    let floor_2d = window.resolution.physical_height() as f32;
    let walls_2d = window.resolution.physical_width() as f32;

    commands.spawn_batch((0..100).enumerate().map(move |(i, _)| {
        let sprite_handle = emoji_handles[i % 11].clone();
        (
            Sprite {
                image: sprite_handle,
                ..default()
            },
            // TODO: remember to apply small delta to z to avoid z-order fighting
            Transform::from_xyz(-walls_2d / 2. * 0.9, -floor_2d / 2. - 64., 0.),
            Velocity2d(Vec2::ZERO),
        )
    }));
}

#[derive(Component)]
#[require(PreVelocity2d)]
struct Velocity2d(Vec2);
/// Receives velocity to be applied, which will be transmitted to
/// [`Velocity2d`] given a delay `time_fraction`.
#[derive(Component, Default)]
struct PreVelocity2d {
    velocity: Vec2,
    time_fraction: f32,
}
#[derive(Resource)]
struct ParticleTimer(Timer);
#[derive(Resource, Default)]
pub struct SecretRevealed(pub bool);
/// Marker for entities that are a secret.
///
/// On despawn, `Secret` entities stop the emoji system and
/// (TODO) play some violins.
#[derive(Component)]
pub struct Secret;

impl Default for ParticleTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Once))
    }
}

fn resolve_particle_physics(
    time: Res<Time>,
    window: Single<&Window>,
    mut query: Query<(&mut Transform, &mut Velocity2d)>,
) {
    const GRAVITY2D: f32 = 1000.;
    const FRICTION: f32 = 2.;
    let dt = time.delta_secs();
    let height = window.resolution.physical_height();
    let floor = -((height / 2) as f32) - 64.;
    let ceiling = -floor;
    let width = window.resolution.physical_width();
    let left_wall = -((width / 2) as f32);
    let right_wall = left_wall * 0.6;
    // println!("left -> {left_wall}");
    // println!("right -> {right_wall}");
    for (mut trans, mut velocity) in &mut query {
        if velocity.0.y > 0. {
            velocity.0 = velocity.0 - velocity.0 * FRICTION * dt;
        }
        velocity.0.y -= GRAVITY2D * dt;
        if trans.translation.y <= floor {
            trans.translation.y = floor;
            velocity.0.y = velocity.0.y.max(0.);
            trans.rotation = Quat::IDENTITY;
        }
        if trans.translation.y >= ceiling {
            trans.translation.y = -floor;
            velocity.0.y = velocity.0.y.min(0.0);
        }
        if trans.translation.x >= right_wall {
            trans.translation.x = right_wall;
            velocity.0.x = -velocity.0.x;
        } else if trans.translation.x <= left_wall {
            trans.translation.x = left_wall;
            velocity.0.x = -velocity.0.x;
        }
        trans.translation.x += velocity.0.x * dt;
        trans.translation.y += velocity.0.y * dt;
    }
}

#[derive(Event)]
pub struct EmojiBurst {
    // velocity to apply to the emojis
    pub velocity: f32,
    // number of emojis to apply the velocity more or less
    pub percent: f32,
}

#[derive(Resource)]
struct ParticleNoise {
    rng: Rng,
}

impl Default for ParticleNoise {
    fn default() -> ParticleNoise {
        ParticleNoise {
            rng: fastrand::Rng::new(),
        }
    }
}

impl ParticleNoise {
    /// Marsaglia’s polar method for standard normal.
    fn gaussian(&mut self) -> f32 {
        loop {
            // fastrand::f64() gives you a uniform [0, 1) double
            let u1 = 2.0 * self.sample() - 1.0; // uniform (-1,1)
            let u2 = 2.0 * self.sample() - 1.0; // uniform (-1,1)
            let s = u1 * u1 + u2 * u2;
            if s == 0.0 || s >= 1.0 {
                continue;
            }
            // Only one ln/sqrt per accepted point:
            let factor = (-2.0 * s.ln() / s).sqrt();
            return u1 * factor; // one N(0,1)
        }
    }
    fn sample(&mut self) -> f32 {
        self.rng.f32()
    }
}

/// Sets pre-velocity to be applied from incoming events.
fn receive_particle_velocity(
    time: Res<Time>,
    mut burst_event: EventReader<EmojiBurst>,
    mut timer: ResMut<ParticleTimer>,
    mut noise: ResMut<ParticleNoise>,
    mut query: Query<&mut PreVelocity2d>,
) {
    timer.0.tick(time.delta());
    for EmojiBurst { velocity, percent } in burst_event.read() {
        timer.0.reset();
        let base_velocity = Vec2::Y * velocity;
        query.iter_mut().for_each(|mut vel2d| {
            let u = &noise.sample();
            if percent > u {
                let gauss = noise.gaussian() * 600.;
                vel2d.velocity.x += base_velocity.x + gauss;
                vel2d.velocity.y += base_velocity.y + gauss.abs();
                vel2d.time_fraction = u / percent;
            }
        })
    }
}

/// Apply velocity from pre-velocity at a given timer fraction.
fn apply_delayed_velocity(
    timer: Res<ParticleTimer>,
    mut query: Query<(&mut PreVelocity2d, &mut Velocity2d)>,
) {
    if !timer.0.finished() {
        let u = timer.0.fraction();
        for (mut pre_vel, mut vel) in &mut query {
            if pre_vel.velocity != Vec2::ZERO {
                if u > pre_vel.time_fraction {
                    // default of Vec2 is ZERO
                    vel.0 = std::mem::take(&mut pre_vel.velocity);
                }
            }
        }
    }
}
