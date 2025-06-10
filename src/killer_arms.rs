//! Mechanic for killer arms that appear at night at kill the player.

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
                )
                    .run_if(in_state(GameState::Above)),
            );
    }
}

/// Marker for the (invisible, target of IK) bone that points at the player to kill.
#[derive(Component)]
pub struct KillerPoint;
#[derive(Component)]
pub struct KillerTimer(pub Timer);
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
        let mut timer = TimerComp::from_elapsed(show_time);
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
            *point = point.with_translation(look_at);
        }
    }
}

const MAGENTA: Color = Color::srgb(1.00, 0.30, 0.90);

fn activate_lasers(
    killers: Query<&TimerComp, (With<KillerArm>, Without<KillerHead>)>,
    mut killer_heads: Query<&mut TimerComp, (With<KillerHead>, Without<KillerArm>)>,
) {
    for dodgy_timer in &killers {
        // TODO: should go down the hierarchy.
        if dodgy_timer.0.just_finished() {
            for mut ik_bone_timer in &mut killer_heads {
                if ik_bone_timer.0.paused() {
                    ik_bone_timer.0.unpause();
                    ik_bone_timer.0.reset();
                }
            }
        }
    }
}

fn draw_lasers(
    mut gizmos: Gizmos,
    killer_points: Query<(&GlobalTransform, &TimerComp), (With<KillerHead>, Without<Player>)>,
    player: Single<&Transform, (With<Player>, Without<KillerPoint>)>,
) {
    for (trans, timer) in killer_points {
        if timer.0.finished() {
            gizmos.line(
                trans.translation(),
                player.translation - Vec3::Y,
                MAGENTA,
                // Color::BLACK.mix(&MAGENTA, timer.0.fraction()),
            );
        }
    }
}
