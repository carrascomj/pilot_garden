use bevy::prelude::Vec3;

pub const BUMP_DISTANCE: f32 = 20.0;
pub const SPEED: f32 = 20.0;
pub const GRAVITY: f32 = 400.0;
pub const GROUND_Y: f32 = 2.8;
pub const CAMERA_SENSITIVITY: f32 = 0.003;
pub const PLAYER_HALF_EXTENTS: Vec3 = Vec3::new(0.8, GROUND_Y, 0.8);
pub const MAX_VELOCITY: Vec3 = Vec3::new(5.0, 10.0, 5.0);
