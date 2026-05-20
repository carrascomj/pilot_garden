use crate::{
    config::{CAMERA_SENSITIVITY, GRAVITY, GameState, PLAYER_HALF_EXTENTS, SPEED, START_POS},
    input_mapping::InputActions,
};
use bevy::{
    core_pipeline::{Skybox, tonemapping::Tonemapping},
    input::mouse::AccumulatedMouseMotion,
    light::NotShadowCaster,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Component)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DidFixedTimestepRunThisFrame>()
            .add_systems(Startup, spawn_player)
            .add_systems(OnEnter(GameState::Menu), reset_player)
            .add_systems(PreUpdate, clear_fixed_timestep_flag)
            .add_systems(FixedPreUpdate, set_fixed_time_step_flag)
            .add_systems(FixedUpdate, advance_physics)
            .add_systems(Update, load_skybox.run_if(in_state(GameState::Above)))
            .add_systems(
                RunFixedMainLoop,
                (
                    // player input movement since last physics update is accumulated
                    // just before the advance_physics `FixedUpdate` and cleared later if/when the
                    // fixed main loop systems happened
                    move_player.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                    (
                        clear_input.run_if(did_fixed_timestep_run_this_frame),
                        interpolate_rendered_transform,
                    )
                        .chain()
                        .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                )
                    .run_if(not(in_state(GameState::Menu)))
                    .run_if(not(in_state(GameState::GameOver)))
                    .run_if(not(in_state(GameState::EndScreen))),
            );
    }
}

/// A vector representing the player's input, accumulated over all frames that ran
/// since the last time the physics simulation was advanced.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default)]
struct AccumulatedInput {
    movement: Vec3,
    jump: bool,
}

/// A vector representing the player's velocity in the physics simulation.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

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

/// Create the player with the camera (FPS-like)
fn spawn_player(mut commands: Commands, asset_server: ResMut<AssetServer>) {
    let mesh: Handle<Mesh> = asset_server.load(
        GltfAssetLabel::Primitive {
            mesh: 0,
            primitive: 0,
        }
        .from_asset("player.glb#Mesh0/Primitive0"),
    );
    let material: Handle<StandardMaterial> = asset_server.load(
        GltfAssetLabel::Material {
            index: 0,
            is_scale_inverted: false,
        }
        .from_asset("player.glb#Material0"),
    );
    commands.spawn((
        Tonemapping::BlenderFilmic,
        Camera3d::default(),
        NotShadowCaster,
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.0,
            ..default()
        }),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.25, 0.2, 0.5)),
            order: 0,
            ..Default::default()
        },
        Transform::from_translation(START_POS).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        AccumulatedInput::default(),
        Velocity::default(),
        PhysicalTranslation(START_POS),
        PreviousPhysicalTranslation(START_POS),
        Player {},
        Mesh3d(mesh),
        MeshMaterial3d(material),
    ));
    let handle = asset_server.load("skybox.png");
    commands.insert_resource(LoadingSkybox { handle })
}

#[derive(Resource)]
struct LoadingSkybox {
    handle: Handle<Image>,
}

fn load_skybox(
    mut commands: Commands,
    mut loading_sky: ResMut<LoadingSkybox>,
    asset_server: ResMut<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    player: Single<Entity, With<Player>>,
    mut loaded: Local<bool>,
) {
    // load 6 vertically stacked squares from PNG and turn them into a cube
    if *loaded || !asset_server.is_loaded(loading_sky.handle.id()) {
        return;
    }
    *loaded = true;
    let image = images.get_mut(&mut loading_sky.handle).unwrap();
    let array_layers = 6;
    let _ = image.reinterpret_stacked_2d_as_array(array_layers);
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    commands.entity(player.entity()).insert(Skybox {
        image: loading_sky.handle.clone(),
        brightness: 1000.,
        ..default()
    });
}

fn reset_player(
    mut ambient_light: ResMut<GlobalAmbientLight>,
    mut player: Single<
        (
            &mut Transform,
            &mut PhysicalTranslation,
            &mut PreviousPhysicalTranslation,
            &mut Velocity,
        ),
        With<Player>,
    >,
) {
    // default
    ambient_light.brightness = 80.;

    *player.0 =
        Transform::from_translation(START_POS).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    player.1.0 = START_POS;
    player.2.0 = START_POS;
    player.3.0 = Vec3::new(0., 0., 0.);
}

/// Player movement system.
fn move_player(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    pressed: Res<InputActions>,
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
    // ignore pitch/roll for movement so forward speed is constant on the ground plane.
    let mut forward = transform.rotation * Vec3::NEG_Z;
    forward.y = 0.0;
    forward = forward.normalize_or_zero();

    let mut right = transform.rotation * Vec3::X;
    right.y = 0.0;
    right = right.normalize_or_zero();

    input.movement = Vec3::ZERO;
    input.jump = pressed.jump;

    if pressed.up {
        input.movement += forward;
    }
    if pressed.down {
        input.movement -= forward;
    }
    if pressed.left {
        input.movement -= right;
    }
    if pressed.right {
        input.movement += right;
    }

    // stay on ground: flatten the vector and renormalize
    let input_normalized = input.movement.normalize_or_zero() * SPEED;
    velocity.0.x = input_normalized.x;
    velocity.0.z = input_normalized.z;
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

    if let Ok((mut pos, mut prev_pos, input, mut vel)) = query.single_mut() {
        // ------------------------------------------------ integrate forces --
        let grounded = vel.y.abs() < 0.00001;
        if input.jump && grounded {
            vel.y += 50.0;
        }
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
    }
}

#[derive(Resource, Debug, Deref, DerefMut, Default)]
struct DidFixedTimestepRunThisFrame(bool);

fn clear_fixed_timestep_flag(
    mut did_fixed_timestep_run_this_frame: ResMut<DidFixedTimestepRunThisFrame>,
) {
    did_fixed_timestep_run_this_frame.0 = false;
}

fn set_fixed_time_step_flag(
    mut did_fixed_timestep_run_this_frame: ResMut<DidFixedTimestepRunThisFrame>,
) {
    did_fixed_timestep_run_this_frame.0 = true;
}

fn did_fixed_timestep_run_this_frame(
    did_fixed_timestep_run_this_frame: Res<DidFixedTimestepRunThisFrame>,
) -> bool {
    did_fixed_timestep_run_this_frame.0
}

fn clear_input(mut input: Single<&mut AccumulatedInput, With<Player>>) {
    **input = AccumulatedInput::default();
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
