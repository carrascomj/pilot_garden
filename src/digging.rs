//! Systems for digable terrain.

use std::f32::consts::PI;

use crate::config::{GameState, REST_ROT};
use crate::gnomes::{GnomeMachine, PlatformMover};
use crate::player_movement::Collider;
use crate::point_raycast::RayBlocker;
use crate::world_timer::TimerComp;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

/// Initializes colliders for dirt, attaches markers for digging
/// and implements digging capabilities for [`Diggable`] entities.
pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_dirt_colliders)
            .add_observer(add_collectibles)
            .add_event::<SeedsPlaced>()
            .add_systems(
                Update,
                (animate_interaction, remove_when_life_depleted).run_if(in_state(GameState::Above)),
            )
            .add_observer(remove_the_platform_gnome)
            // tools animation might be playing while in Below already
            .add_systems(
                Update,
                remove_animation.run_if(not(in_state(GameState::Menu))),
            )
            .add_systems(
                PostUpdate,
                add_colliders_to_diggables
                    .after(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::Above)),
            );
    }
}

// Markers for entities that can be interacted with.
/// Can be removed with the shovel.
#[derive(Component)]
pub struct Diggable;
/// Can be removed with the Mining Pick.
#[derive(Component)]
pub struct Minable;
#[derive(Component)]
pub enum Life {
    Left(u8),
    JustSpawned,
}
/// Can be taken (shovel, mining pick, food)
#[derive(Component, PartialEq)]
pub enum Collectible {
    Shovel,
    MiningPick,
    Seeds,
    Food,
}

impl Collectible {
    // const fn on_hand_poses(&self) -> (Vec3, Quat) {
    pub fn on_hand_poses(&self) -> (Vec3, Quat) {
        let seeds_rot = Quat::from_euler(
            EulerRot::YXZ,
            -0.4,  // yaw   -90°  (tip forward)
            -0.10, // pitch -20°  (look slightly down along it)
            -0.3,  // roll  +14°  (handle tilt)
        );
        match self {
            Collectible::MiningPick => {
                const PICK_OFFSET: Vec3 = Vec3::new(1.4, -0.2, -1.8);
                (PICK_OFFSET, REST_ROT)
            }
            Collectible::Seeds => {
                const SEED_OFFSET: Vec3 = Vec3::new(1.6, -0.25, -2.8); // X right, Y up, Z forward
                (SEED_OFFSET, seeds_rot)
            }
            _ => {
                const FPS_OFFSET: Vec3 = Vec3::new(1.4, -0.25, -1.8); // X right, Y up, Z forward
                (FPS_OFFSET, REST_ROT)
            }
        }
    }
}

#[derive(Component)]
pub struct OneSizeCollider;
#[derive(Component)]
struct FakeGround;

/// Attaches [`Diggable`] markers to gltf objects named as `crop_ground`.
fn add_dirt_colliders(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    add_names: Query<(Entity, &Name), (Added<Name>, Without<Diggable>)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let _e = trigger.target();
    for (ent, name) in add_names.iter() {
        if name.as_str().starts_with("crop_ground") {
            commands.entity(ent).insert((Diggable {}, OneSizeCollider));
        }
        if name.as_str() == "crop_ground_special" {
            // invisible meshes to protect the SPECIAL diggable tile below de gnome
            commands
                .entity(ent)
                .insert(FakeGround)
                .with_child((
                    Mesh3d(meshes.add(Cuboid::new(1., 4., 1.))),
                    Transform::from_translation(Vec3::new(-0.8, -0.9, 0.5)),
                    RayBlocker,
                ))
                .with_child((
                    Mesh3d(meshes.add(Cuboid::new(1., 4., 1.))),
                    Transform::from_translation(Vec3::new(0.8, -0.9, 0.5)),
                    RayBlocker,
                ));
        }
    }
}

fn remove_the_platform_gnome(
    _trigger: Trigger<OnRemove, FakeGround>,
    mut commands: Commands,
    platform_mover: Single<Entity, With<PlatformMover>>,
) {
    commands
        .entity(platform_mover.entity())
        .insert(RemoveTimer::new());
}

/// Calculate and attach colliders to [`Diggable`] entities.
fn add_colliders_to_diggables(
    mut commands: Commands,
    diggables: Populated<(Entity, &GlobalTransform), (With<OneSizeCollider>, Without<Collider>)>,
) {
    let size = Vec3::new(1.0, 0.5, 1.0);

    for (ent, trans) in diggables.iter() {
        commands.entity(ent).insert(Collider::from_translation(
            trans.translation() + Vec3::Y * 0.5,
            size,
        ));
    }
}

#[derive(Event)]
pub struct SeedsPlaced {
    pub hit_position: Vec3,
}

#[derive(Component)]
#[require(TimerComp::from_elapsed(0.35))]
pub struct OnHand {
    pub active: bool,
}

impl OnHand {
    fn new() -> Self {
        Self { active: false }
    }
}

#[derive(Component)]
pub struct CollectibleParent;

/// Attach [`Collectible`] markers to Shovel and MiningPick on spawn from gltf.
fn add_collectibles(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    added_names: Query<(Entity, &Name), (Without<Collectible>, Added<Name>)>,
) {
    let _e = trigger.target();
    for (entity, name) in added_names.iter() {
        match name.as_str() {
            "Shovel" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Shovel)
                    .insert(OnHand::new());
            }
            "MiningPick" => {
                commands
                    .entity(entity)
                    .insert(Collectible::MiningPick)
                    .insert(OnHand::new());
            }
            "Food" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Food)
                    .insert(OnHand::new());
            }
            "Seeds" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Seeds)
                    .insert(OnHand::new());
            }
            _ => (),
        }
    }
}

fn wave(t: f32) -> f32 {
    (t * PI).sin()
}

fn animate_interaction(mut bones: Query<(&mut Transform, &OnHand, &TimerComp, &Collectible)>) {
    for (mut transform, on_hand, timer, collectible) in &mut bones {
        // the children its the mesh, transforms are better
        // applied to the parent object in the gltf since it has
        // absolute coordinates
        let (rest_pos, rest_rot) = collectible.on_hand_poses();
        if !on_hand.active {
            continue;
        } else if timer.0.finished() {
            transform.translation = rest_pos;
            transform.rotation = rest_rot;
            continue;
        }
        // normalised time in the [0, 1] animation range
        let u = timer.0.fraction();

        let (translation, rotation) = match collectible {
            Collectible::Shovel => {
                let a = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
                let t = rest_pos
                    + Vec3::new(
                        0.0,
                        -0.35 * a, // dip down
                        -0.45 * a, // and forward
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, -1.0 * a, 0.5, 0.0);
                (t, r)
            }
            Collectible::MiningPick => {
                let curve = CubicCardinalSpline {
                    tension: 0.4,
                    control_points: [
                        rest_pos,
                        rest_pos + Vec3::new(0.35, 0.35, 0.84),
                        rest_pos + Vec3::new(-0.5, -0.35, -1.),
                        rest_pos,
                    ]
                    .into(),
                }
                .to_curve()
                .expect("Should work");
                let t = curve.position(u * 2.);
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, -0.4 * wave(u), 0.5, 0.0);
                (t, r)
            }
            Collectible::Food | Collectible::Seeds => {
                let a = wave(u);
                let t = rest_pos
                    + Vec3::new(
                        -0.15 * a, // toward centre
                        0.25 * a,  // up to mouth
                        1.7 * a,   // closer to camera
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::YXZ, 0.15 * a, -0.10 * a, 0.10 * a);
                (t, r)
            }
        };

        transform.translation = translation;
        transform.rotation = rotation;
    }
}

// `RemoveTimer` owns its own [`Timer`] because it will be inserted in a
// existing Entity, presumably with a [`TimerComp`] already that would
// clash with it already.
#[derive(Component)]
pub struct RemoveTimer(Timer);

impl RemoveTimer {
    pub fn new() -> Self {
        Self(Timer::from_seconds(0.8, TimerMode::Once))
    }
}

/// Decrease in life (which can only be decreased) -> small scale decrease.
///
/// If life == 0, add a [`RemoveTimer`] that will play an animation and despawn
/// the entity afterwards at [`remove_animation`].
fn remove_when_life_depleted(
    mut commands: Commands,
    mut lifes: Query<(Entity, &mut Transform, &mut Life, Option<&mut GnomeMachine>), Changed<Life>>,
) {
    for (entity, mut trans, mut life, maybe_gnome) in lifes.iter_mut() {
        if let Life::Left(count) = life.as_ref() {
            let is_gnome = maybe_gnome.is_none();
            if count <= &0 {
                if let Some(mut gnome) = maybe_gnome {
                    gnome.next_state();
                } else {
                    commands.entity(entity).insert(RemoveTimer::new());
                }
            }
            if count < &3 && is_gnome {
                trans.scale *= 0.9;
            }
        } else {
            *life = Life::Left(1);
        }
    }
}

fn remove_animation(
    mut commands: Commands,
    time: Res<Time>,
    mut to_remove: Query<(Entity, &mut Transform, &mut RemoveTimer)>,
) {
    for (ent, mut trans, mut rm_timer) in &mut to_remove {
        if rm_timer.0.just_finished() {
            commands.entity(ent).despawn();
        } else {
            rm_timer.0.tick(time.delta());
            let u = rm_timer.0.fraction();
            trans.scale = (1. - u) * Vec3::ONE + u * Vec3::ZERO;
            trans.rotation *= Quat::from_rotation_y(0.2);
        }
    }
}
