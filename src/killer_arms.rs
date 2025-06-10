//! Mechanic for killer arms that appear at night at kill the player.

use std::time::Duration;

use bevy::{ecs::query::QueryFilter, prelude::*};
use bevy_mod_inverse_kinematics::{IkConstraint, InverseKinematicsPlugin};

use crate::{
    config::GameState,
    dodgy::Dodgy,
    player_movement::Player,
    world_timer::{ShowOnAlarmTime, TimerComp},
};

pub struct KillerArmPlugin;

impl Plugin for KillerArmPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InverseKinematicsPlugin)
            .add_systems(
                OnEnter(GameState::Above),
                (spawn_killing_arm, setup_inverse_kinematics),
            )
            .add_systems(
                Update,
                (
                    setup_inverse_kinematics,
                    point_at_player,
                    activate_lasers,
                    draw_lasers,
                    // kill_with_lasers,
                )
                    .run_if(in_state(GameState::Above)),
            );
    }
}

/// Marker for the (invisible, target of IK) bone that points at the player to kill.
#[derive(Component)]
pub struct KillerPoint;
#[derive(Component)]
pub struct KillerTimer {
    pub timer: Timer,
    pub can_kill: bool,
}
/// The bone of the visible head of the bone (object of IK).
#[derive(Component)]
pub struct KillerHead;
/// The parent of the GLTF containing the killer arm.
#[derive(Component)]
struct KillerArm;

fn spawn_killing_arm(mut commands: Commands, asset_server: Res<AssetServer>) {
    for killer_position in [
        Vec3::new(8., 0., -15.),
        Vec3::new(8., 0., 15.),
        Vec3::new(34.5, 0., 0.),
        Vec3::new(-14., 0., 0.),
    ] {
        let show_time = 10.;
        let dur = Duration::from_secs_f32(show_time);
        let mut timer = TimerComp(Timer::new(dur, TimerMode::Once));
        timer.0.set_elapsed(Duration::ZERO);
        timer.0.pause();
        let init_pos = killer_position - (Vec3::Y * 100.);
        commands.spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("killing_arm.glb"))),
            StateScoped(GameState::Above),
            KillerArm,
            Transform::from_translation(init_pos),
            timer,
            // laser_timer,
            ShowOnAlarmTime::as_false(),
            Dodgy {
                init_pos,
                last_pos: killer_position,
                go_back: false,
            },
        ));
    }
}

fn setup_inverse_kinematics(
    mut commands: Commands,
    killer_bones: Query<(Entity, &ChildOf), Added<KillerPoint>>,
    parents: Query<&Children, Without<KillerPoint>>,
    names: Query<(Entity, &Name), (Without<KillerPoint>, Without<IkConstraint>)>,
) {
    for (killer_bone, bone_parent) in killer_bones.iter() {
        // go up one in the hierarchy and then down again to all the bone children
        if let Ok(children) = parents.get(bone_parent.0) {
            for child in children {
                if let Ok(ik_bone) = find_entity(*child, "ik_target", parents, names) {
                    commands.entity(ik_bone).insert(IkConstraint {
                        chain_length: 4,
                        iterations: 300,
                        target: killer_bone,
                        pole_target: None,
                        pole_angle: -std::f32::consts::FRAC_PI_2,
                        enabled: true,
                    });
                }
            }
        }
    }
}

fn find_entity<F: QueryFilter, F2: QueryFilter>(
    root: Entity,
    look_for_name: &str,
    parents: Query<&Children, F>,
    names: Query<(Entity, &Name), F2>,
) -> Result<Entity, ()> {
    if let Ok(children) = parents.get(root) {
        for child in children {
            if let Ok((ik_bone, name)) = names.get(*child) {
                if name.as_str().starts_with(look_for_name) {
                    // base case
                    return Ok(ik_bone);
                } else {
                    return find_entity(ik_bone, look_for_name, parents, names);
                }
            }
        }
        return Err(());
    } else {
        // base case, nothing found
        return Err(());
    }
}

fn point_at_player(
    mut killer_points: Query<(&mut Transform, &ChildOf), (With<KillerPoint>, Without<Player>)>,
    player: Single<&Transform, (With<Player>, Without<KillerPoint>)>,
    parents: Query<&GlobalTransform, (Without<Player>, Without<KillerPoint>)>,
) {
    for (mut point, parent) in &mut killer_points {
        if let Ok(parent_trans) = parents.get(parent.0) {
            // substract parent
            let look_at = player.translation - parent_trans.translation();
            *point = point.with_translation(look_at).looking_at(look_at, Vec3::Y);
        }
    }
}

const MAGENTA: Color = Color::srgb(1.00, 0.30, 0.90);

fn activate_lasers(
    killers: Query<(&TimerComp, &ShowOnAlarmTime), (With<KillerArm>, Without<KillerHead>)>,
    mut killer_heads: Query<
        (&mut TimerComp, &mut KillerTimer),
        (With<KillerHead>, Without<KillerArm>),
    >,
) {
    for (dodgy_timer, show) in &killers {
        // TODO: should go down the hierarchy.
        if dodgy_timer.0.just_finished() && show.show {
            for (mut laser_timer, mut killer_timer) in &mut killer_heads {
                if laser_timer.0.paused() {
                    laser_timer.0.unpause();
                    laser_timer.0.reset();
                    killer_timer.timer.unpause();
                    killer_timer.can_kill = true;
                }
            }
        }
    }
}

fn draw_lasers(
    mut gizmos: Gizmos,
    mut ray_cast: MeshRayCast,
    mut next_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut killer_points: Query<
        (&GlobalTransform, &TimerComp, &mut KillerTimer),
        (With<KillerHead>, Without<Player>),
    >,
    player: Single<(Entity, &Transform), (With<Player>, Without<KillerPoint>)>,
) {
    for (trans, timer, mut killer_timer) in &mut killer_points {
        if timer.0.finished() {
            let ray_pos = trans.translation();
            let start = ray_pos;
            let end = player.1.translation;
            let ray_dir = (end - start).normalize();

            let ray = Ray3d::new(ray_pos, Dir3::new(ray_dir).unwrap());

            if let Some((target_entity, hit)) = ray_cast
                .cast_ray(ray, &MeshRayCastSettings::default().always_early_exit())
                .first()
            {
                gizmos.line(
                    trans.translation(),
                    hit.point - Vec3::Y,
                    // MAGENTA,
                    Color::BLACK.mix(&MAGENTA, killer_timer.timer.fraction()),
                );
                if killer_timer.timer.just_finished() && killer_timer.can_kill {
                    if player.0 == *target_entity {
                        next_state.set(GameState::Menu);
                    } else {
                        killer_timer.can_kill = false;
                    }
                }
            }
        }
        if !killer_timer.timer.finished() {
            killer_timer.timer.tick(time.delta());
        }
    }
}
