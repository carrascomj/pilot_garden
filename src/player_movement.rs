use crate::config::{CAMERA_SENSITIVITY, GRAVITY, GameState, PLAYER_HALF_EXTENTS, SPEED};
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Component)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, move_player)
            .add_systems(
                Update,
                (advance_physics, interpolate_rendered_transform)
                    .run_if(not(in_state(GameState::Menu))),
            );
    }
}

/// A vector representing the player's input, accumulated over all frames that ran
/// since the last time the physics simulation was advanced.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct AccumulatedInput(Vec3);

/// A vector representing the player's velocity in the physics simulation.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct Velocity(Vec3);

/// The actual position of the player in the physics simulation.
/// This is separate from the `Transform`, which is merely a visual representation.
///
/// If you want to make sure that this component is always initialized
/// with the same value as the `Transform`'s translation, you can
/// use a [component lifecycle hook](https://docs.rs/bevy/0.14.0/bevy/ecs/component/struct.ComponentHooks.html)
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct PhysicalTranslation(Vec3);

/// The value [`PhysicalTranslation`] had in the last fixed timestep.
/// Used for interpolation in the `interpolate_rendered_transform` system.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct PreviousPhysicalTranslation(Vec3);

fn spawn_player(mut commands: Commands) {
    // Create the player with the camera (FPS-like)
    let start_pos = Vec3::new(-2.0, 10.0, 4.0);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.0,
            ..default()
        }),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.25, 0.2, 0.5)),
            ..Default::default()
        },
        Transform::from_translation(start_pos).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        AccumulatedInput::default(),
        Velocity::default(),
        PhysicalTranslation(start_pos),
        PreviousPhysicalTranslation(start_pos),
        Player {},
    ));
}

/// Player movement system.
fn move_player(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    player: Single<(&mut Transform, &mut AccumulatedInput, &mut Velocity), With<Player>>,
) {
    let (mut transform, mut input, mut velocity) = player.into_inner();

    let delta = accumulated_mouse_motion.delta;

    if delta != Vec2::ZERO {
        // Note that we are not multiplying by delta_time here.
        // The reason is that for mouse movement, we already get the full movement that happened since the last frame.
        // This means that if we multiply by delta_time, we will get a smaller rotation than intended by the user.
        // This situation is reversed when reading e.g. analog input from a gamepad however, where the same rules
        // as for keyboard input apply. Such an input should be multiplied by delta_time to get the intended rotation
        // independent of the framerate.
        let delta_yaw = -delta.x * CAMERA_SENSITIVITY;
        let delta_pitch = -delta.y * CAMERA_SENSITIVITY;

        let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
        let yaw = yaw + delta_yaw;

        // If the pitch was ±¹⁄₂ π, the camera would look straight up or down.
        // When the user wants to move the camera back to the horizon, which way should the camera face?
        // The camera has no way of knowing what direction was "forward" before landing in that extreme position,
        // so the direction picked will for all intents and purposes be arbitrary.
        // Another issue is that for mathematical reasons, the yaw will effectively be flipped when the pitch is at the extremes.
        // To not run into these issues, we clamp the pitch to a safe range.
        const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
        let pitch = (pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    };

    if keyboard_input.pressed(KeyCode::KeyW) {
        input.0 += transform.rotation * Vec3::NEG_Z;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        input.0 -= transform.rotation * Vec3::NEG_Z;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        input.0 -= transform.rotation * Vec3::X;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        input.0 += transform.rotation * Vec3::X;
    }

    // stay on ground: flatten the vector and renormalize
    let input_normalized = input.normalize_or_zero() * SPEED;
    velocity.0.x = input_normalized.x;
    velocity.0.z = input_normalized.z;
    let grounded = velocity.0.y.abs() < 0.00001;

    if keyboard_input.just_pressed(KeyCode::Space) && grounded {
        velocity.y += 50.0;
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Collider {
    pub min: Vec3,
    pub max: Vec3,
}

impl Collider {
    #[inline]
    pub fn intersects(&self, other: &Collider) -> bool {
        !(self.max.x < other.min.x
            || self.min.x > other.max.x
            || self.max.y < other.min.y
            || self.min.y > other.max.y
            || self.max.z < other.min.z
            || self.min.z > other.max.z)
    }

    /// half_size as done by a `Cuboid`.
    pub fn from_translation(translation: Vec3, half_size: Vec3) -> Self {
        Collider {
            min: translation - half_size,
            max: translation + half_size,
        }
    }
}

impl From<(&Cuboid, &Transform)> for Collider {
    fn from((cuboid, t): (&Cuboid, &Transform)) -> Self {
        let half = cuboid.half_size * t.scale;
        let centre = t.translation;

        Collider::from_translation(centre, half)
    }
}

fn player_aabb(pos: Vec3) -> Collider {
    Collider {
        min: pos - PLAYER_HALF_EXTENTS,
        max: pos + PLAYER_HALF_EXTENTS,
    }
}

/// Advance the physics simulation by one fixed timestep. This may run zero or multiple times per frame.
/// Collisions are also integrated here.
fn advance_physics(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<
        (
            &mut PhysicalTranslation,
            &mut PreviousPhysicalTranslation,
            &mut AccumulatedInput,
            &mut Velocity,
        ),
        With<Player>,
    >,
    colliders: Query<&Collider>,
) {
    const SKIN: f32 = 0.001; // small offset to prevent re-intersection
    let dt = fixed_time.delta_secs();

    if let Ok((mut pos, mut prev_pos, mut input, mut vel)) = query.single_mut() {
        // ------------------------------------------------ integrate forces --
        vel.y -= GRAVITY * dt;
        prev_pos.0 = pos.0;

        // Work on a local copy first
        let mut next = pos.0;

        // -------------------------------------------------- Y axis first ----
        next.y += vel.y * dt;
        let bb_y = player_aabb(next);

        for col in &colliders {
            if bb_y.intersects(col) {
                if vel.y > 0.0 {
                    // hit ceiling
                    next.y = col.min.y - PLAYER_HALF_EXTENTS.y - SKIN;
                } else {
                    // landed on something
                    next.y = col.max.y + PLAYER_HALF_EXTENTS.y + SKIN;
                }
                vel.y = vel.y.max(0.0);
                break; // Y resolved, no need to test others
            }
        }

        // -------------------------------------------------- X axis ----------
        next.x += vel.x * dt;
        let bb_x = player_aabb(next);

        for col in &colliders {
            if bb_x.intersects(col) {
                next.x = prev_pos.x; // snap back only on this axis
                vel.x = 0.0;
                break;
            }
        }

        // -------------------------------------------------- Z axis ----------
        next.z += vel.z * dt;
        let bb_z = player_aabb(next);

        for col in &colliders {
            if bb_z.intersects(col) {
                next.z = prev_pos.z;
                vel.z = 0.0;
                break;
            }
        }

        // commit
        pos.0 = next;
        input.0 = Vec3::ZERO; // clear accumulator
    }
}

fn interpolate_rendered_transform(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(
        &mut Transform,
        &PhysicalTranslation,
        &PreviousPhysicalTranslation,
    )>,
) {
    for (mut transform, current_physical_translation, previous_physical_translation) in
        query.iter_mut()
    {
        let previous = previous_physical_translation.0;
        let current = current_physical_translation.0;
        // The overstep fraction is a value between 0 and 1 that tells us how far we are between two fixed timesteps.
        let alpha = fixed_time.overstep_fraction();

        let rendered_translation = previous.lerp(current, alpha);
        transform.translation = rendered_translation;
    }
}
