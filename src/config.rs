use bevy::prelude::{Quat, States, Vec3};

pub const BUMP_DISTANCE: f32 = 20.0;
pub const SPEED: f32 = 10.0;
pub const GRAVITY: f32 = 250.0;
pub const GROUND_Y: f32 = 2.8;
pub const CAMERA_SENSITIVITY: f32 = 0.003;
pub const PLAYER_HALF_EXTENTS: Vec3 = Vec3::new(0.4, GROUND_Y, 0.4);
pub const INTERACTION_DISTANCE: f32 = 8.0;
pub const TOOL_ANIM_TIME: f32 = 0.35;
pub const SHOVEL_DURABILITY: usize = 3;
/// Equivalent to (but const)
///
/// ```norun
/// Quat::from_euler(
///     EulerRot::YXZ,
///     -std::f32::consts::FRAC_PI_2, // yaw   -90°  (tip forward)
///     -0.35,                        // pitch -20°  (look slightly down along it)
///     0.25,                         // roll  +14°  (handle tilt)
/// )
/// ```
pub const REST_ROT: Quat = Quat::from_xyzw(-0.20896433, -0.6755249, -0.035340607, 0.7062231);
/// Equivalent to (but const)
///
/// ```norun
/// Quat::from_euler(
///     EulerRot::YXZ,
///     -0.4,  // yaw   -90°  (tip forward)
///     -0.10, // pitch -20°  (look slightly down along it)
///     -0.3,  // roll  +14°  (handle tilt)
/// );
/// ```
pub const SEEDS_ROT: Quat = Quat::from_xyzw(-0.018781215, -0.2035129, -0.15609413, 0.96636665);
/// Bounds to limit the spawning on things in the crop ground.
pub const MIN_CROP_BOUNDS: Vec3 = Vec3::new(18.6, -20.0, -8.5);
pub const MAX_CROP_BOUNDS: Vec3 = Vec3::new(29.5, 20.0, 0.);
pub const START_POS: Vec3 = Vec3::new(-6.0, 2.8, 8.0);

#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Default)]
#[states(scoped_entities)]
pub enum GameState {
    #[default]
    Menu,
    GameOver,
    Above,
    Below,
    EndScreen,
}
