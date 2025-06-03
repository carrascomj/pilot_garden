use crate::config::{CAMERA_SENSITIVITY, GRAVITY, GROUND_Y, PLAYER_HALF_EXTENTS, SPEED};
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use std::f32::consts::FRAC_PI_2;

#[derive(Component)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player).add_systems(
            Update,
            (move_player, advance_physics, interpolate_rendered_transform),
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
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.25, 0.2, 0.5)),
            ..Default::default()
        },
        Transform::from_xyz(-4.0, GROUND_Y, 8.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        AccumulatedInput::default(),
        Velocity::default(),
        PhysicalTranslation::default(),
        PreviousPhysicalTranslation::default(),
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
    // the position of the camera should only act on x and z axes.
    input.y = 0.0;

    // stay on ground: flatten the vector and renormalize
    velocity.0 = input.normalize_or_zero() * SPEED;
    // simple jump impulse
    if keyboard_input.just_pressed(KeyCode::Space)
        && (transform.translation.y.abs() - GROUND_Y) < 0.01
    {
        velocity.y += 300.0; // tweak jump strength
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Collider {
    pub min: Vec3,
    pub max: Vec3,
}

impl Collider {
    #[inline]
    pub fn contains(&self, p: Vec3) -> bool {
        (self.min.x..=self.max.x).contains(&p.x)
            && (self.min.y..=self.max.y).contains(&p.y)
            && (self.min.z..=self.max.z).contains(&p.z)
    }

    #[inline]
    pub fn intersects(&self, other: &Collider) -> bool {
        !(self.max.x < other.min.x
            || self.min.x > other.max.x
            || self.max.y < other.min.y
            || self.min.y > other.max.y
            || self.max.z < other.min.z
            || self.min.z > other.max.z)
    }
}

impl From<(&Cuboid, &Transform)> for Collider {
    fn from((cuboid, t): (&Cuboid, &Transform)) -> Self {
        // half_extents already includes the object's local scale
        let half = cuboid.half_size * t.scale;
        let centre = t.translation;

        Collider {
            min: centre - half,
            max: centre + half,
        }
    }
}

fn player_aabb(pos: Vec3) -> Collider {
    Collider {
        min: pos - PLAYER_HALF_EXTENTS,
        max: pos + PLAYER_HALF_EXTENTS,
    }
}

/// Advance the physics simulation by one fixed timestep. This may run zero or multiple times per frame.
///
/// Note that since this runs in `FixedUpdate`, `Res<Time>` would be `Res<Time<Fixed>>` automatically.
/// We are being explicit here for clarity.
fn advance_physics(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(
        &mut PhysicalTranslation,
        &mut PreviousPhysicalTranslation,
        &mut AccumulatedInput,
        &mut Velocity,
    )>,
    colliders: Query<&Collider>,
) {
    let dt = fixed_time.delta_secs();
    if let Ok((
        mut current_physical_translation,
        mut previous_physical_translation,
        mut input,
        mut velocity,
    )) = query.single_mut()
    {
        velocity.y -= GRAVITY * dt;
        previous_physical_translation.0 = current_physical_translation.0;
        current_physical_translation.0 += velocity.0 * fixed_time.delta_secs();

        let player_bb_next = player_aabb(current_physical_translation.0);
        for col in &colliders {
            if player_bb_next.intersects(col) {
                // naive resolution: snap back to previous position
                // and wipe velocity on the colliding axis.
                current_physical_translation.0 = previous_physical_translation.0;
                velocity.0.x = 0.0;
                velocity.0.z = 0.0;
                break;
            }
        }
        /* --- ground-clamp ----------------------------------------- */
        if current_physical_translation.y < GROUND_Y {
            current_physical_translation.y = GROUND_Y; // snap to floor
            if velocity.y < 0.0 {
                // cancel downward speed
                velocity.y = 0.0;
            }
        }

        // Reset the input accumulator, as we are currently consuming all input that happened since the last fixed timestep.
        input.0 = Vec3::ZERO;
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
