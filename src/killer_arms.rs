//! Mechanic for killer arms that appear at night at kill the player.

use bevy::{ecs::query::QueryFilter, prelude::*};
use bevy_mod_inverse_kinematics::{IkConstraint, InverseKinematicsPlugin};

use crate::{
    config::GameState,
    dodgy::Dodgy,
    player_movement::Player,
    point_raycast::UIDot,
    world_timer::{ShowOnAlarmTime, TimerComp},
};

pub struct KillerArmPlugin;

impl Plugin for KillerArmPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InverseKinematicsPlugin)
            .add_systems(OnEnter(GameState::Above), spawn_killing_arm)
            .add_systems(
                Update,
                (
                    setup_inverse_kinematics,
                    point_at_player,
                    activate_lasers,
                    draw_lasers,
                    show_dots_lasers,
                    spawn_killing_beam,
                )
                    .run_if(in_state(GameState::Above)),
            )
            .add_systems(Update, move_to)
            .add_event::<BeamOrder>()
            .init_resource::<ShowDots>();
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

/// Entities with this will move to `to` over time.delta() and then
/// be despawned.
#[derive(Component)]
struct MoveTo {
    to: Vec3,
    will_kill: bool,
}

fn spawn_killing_arm(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut killerarm_handle: Local<Option<Handle<Scene>>>,
) {
    // hold a handle to the streelight scene between calls
    if killerarm_handle.is_none() {
        *killerarm_handle =
            Some(asset_server.load(GltfAssetLabel::Scene(0).from_asset("killing_arm.glb")));
    }
    let killer_arm = (*killerarm_handle)
        .as_ref()
        .expect("This is always loaded before");
    for killer_position in [
        Vec3::new(12., 6., -22.),
        // Vec3::new(-4., 8., 26.),
        Vec3::new(39., 5.5, 0.),
        Vec3::new(-19., 5.5, 0.),
    ] {
        let show_time = 25.;
        let mut timer = TimerComp::from_elapsed(show_time);
        timer.0.pause();
        let init_pos = killer_position - (Vec3::Y * 100.);
        commands.spawn((
            SceneRoot(killer_arm.clone()),
            StateScoped(GameState::Above),
            KillerArm,
            Transform::from_translation(init_pos),
            timer,
            ShowOnAlarmTime::as_false(),
            Dodgy {
                init_pos,
                last_pos: killer_position,
                go_back: false,
                ignore_viewing: true,
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
                        chain_length: 11,
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

pub fn find_entity<F: QueryFilter, F2: QueryFilter>(
    root: Entity,
    look_for_name: &str,
    parents: Query<&Children, F>,
    names: Query<(Entity, &Name), F2>,
) -> Result<Entity, ()> {
    if let Ok(children) = parents.get(root) {
        for child in children {
            if let Ok((maybe_bone_ent, name)) = names.get(*child) {
                if name.as_str().starts_with(look_for_name) {
                    // base case
                    return Ok(maybe_bone_ent);
                } else if let Ok(found) = find_entity(maybe_bone_ent, look_for_name, parents, names)
                {
                    return Ok(found);
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

/// Activate the laser timers (one for Killing and one for showing the laser)
/// by walking down the hierarchy:
///
/// KillerArm, SceneInstance
/// |----> RandomEntity
///        |---> Armature (has name)
///              |----> IK bone
///              |----> Bone
///                     |----> Bone
///                            |----> Bone
///                                   |----> Bone with KillerHead Component
fn activate_lasers(
    killers: Query<
        (&TimerComp, &ShowOnAlarmTime, &Children),
        (With<KillerArm>, Without<KillerHead>),
    >,
    mut killer_heads: Query<
        (&mut TimerComp, &mut KillerTimer),
        (With<KillerHead>, Without<KillerArm>),
    >,
    parents: Query<&Children>,
    names: Query<(Entity, &Name)>,
) {
    for (dodgy_timer, show, children) in &killers {
        if dodgy_timer.0.just_finished() && show.show {
            for child in children {
                if let Ok(down_children) = parents.get(*child) {
                    for down_child in down_children {
                        if let Ok(ik_bone) = find_entity(*down_child, "ik_target", parents, names) {
                            if let Ok((mut laser_timer, mut killer_timer)) =
                                killer_heads.get_mut(ik_bone)
                            {
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
            }
        }
        // }
    }
}

#[derive(Default, Resource)]
struct ShowDots {
    left: bool,
    right: bool,
    bottom: bool,
}

fn draw_lasers(
    mut gizmos: Gizmos,
    mut ray_cast: MeshRayCast,
    mut ev_beam_order: EventWriter<BeamOrder>,
    mut show_dots: ResMut<ShowDots>,
    time: Res<Time>,
    mut killer_points: Query<
        (&GlobalTransform, &TimerComp, &mut KillerTimer),
        (With<KillerHead>, Without<Player>),
    >,
    player: Single<(Entity, &Transform), (With<Player>, Without<KillerPoint>)>,
) {
    let (mut right, mut left, mut bottom) = (false, false, false);
    // guard for only setting one beam to kill the player (and avoid setting GameOver more than once)
    let mut will_kill = false;
    for (trans, timer, mut killer_timer) in &mut killer_points {
        if timer.0.finished() && killer_timer.can_kill {
            let ray_pos = trans.translation();
            let start = ray_pos;
            let end = player.1.translation;
            let ray_dir = (end - start).normalize();

            let ray = Ray3d::new(ray_pos, Dir3::new(ray_dir).unwrap());
            if let Some((target_entity, hit)) = ray_cast
                .cast_ray(ray, &MeshRayCastSettings::default().always_early_exit())
                .first()
            {
                let mult = if *target_entity == player.0 { 1.5 } else { 0. };
                gizmos.line(
                    ray_pos,
                    hit.point - mult * Vec3::Y,
                    // MAGENTA,
                    Color::BLACK.mix(&MAGENTA, killer_timer.timer.fraction()),
                );
                if killer_timer.timer.just_finished() && killer_timer.can_kill {
                    let is_player = player.0 == *target_entity;
                    ev_beam_order.write(BeamOrder {
                        from: ray_pos,
                        to: hit.point,
                        will_kill: is_player && !will_kill,
                    });
                    if is_player {
                        will_kill = true;
                    }
                    killer_timer.can_kill = false;
                }
                // check if UI should show direction of lasers
                if *target_entity == player.0 {
                    // direction *from* the player towards the killer head
                    let from_player = start - end;
                    let v2 = Vec2::new(from_player.x, from_player.z).normalize();

                    // choose the dominant axis
                    if v2.x.abs() >= v2.y.abs() {
                        if v2.x > 0.0 {
                            right = true;
                        } else {
                            left = true;
                        }
                    } else if v2.y < 0.0 {
                        bottom = true;
                    }
                }
            }
        }
        if !killer_timer.timer.finished() {
            killer_timer.timer.tick(time.delta());
        }
    }
    if right != show_dots.right {
        show_dots.right = right;
    }
    if left != show_dots.left {
        show_dots.left = left;
    }
    if bottom != show_dots.bottom {
        show_dots.bottom = bottom;
    }
}

fn show_dots_lasers(show_dots: Res<ShowDots>, mut ui_dots: Query<(&mut Visibility, &UIDot)>) {
    if show_dots.is_changed() {
        for (mut vis, dot) in &mut ui_dots {
            match dot {
                UIDot::Left => {
                    *vis = if show_dots.left {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                UIDot::Right => {
                    *vis = if show_dots.right {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                UIDot::Bottom => {
                    *vis = if show_dots.bottom {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
    }
}

#[derive(Event)]
struct BeamOrder {
    from: Vec3,
    to: Vec3,
    will_kill: bool,
}

fn spawn_killing_beam(
    mut commands: Commands,
    mut ev_beam_order: EventReader<BeamOrder>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // state to handle mesh and materials across runs
    mut handles: Local<(Option<Handle<Mesh>>, Option<Handle<StandardMaterial>>)>,
) {
    for BeamOrder {
        from, // Vec3
        to,   // Vec3
        will_kill,
    } in ev_beam_order.read()
    {
        if let (None, None) = *handles {
            handles.0 = Some(meshes.add(Sphere::new(0.05)));
            handles.1 = Some(materials.add(StandardMaterial {
                base_color: MAGENTA.with_alpha(0.9),
                diffuse_transmission: 0.1,
                alpha_mode: AlphaMode::Blend,
                // emissive: LinearRgba::rgb(2.00, 0.6, 1.8),
                ..default()
            }));
        }
        commands.spawn((
            // righ-angle rotation towards to
            Transform::from_translation(*from),
            MoveTo {
                to: *to,
                will_kill: *will_kill,
            },
            Mesh3d(handles.0.as_ref().expect("works").clone()),
            MeshMaterial3d(handles.1.as_ref().expect("works").clone()),
        ));
    }
}

fn move_to(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut transforms: Populated<(Entity, &mut Transform, &MoveTo)>,
) {
    let delta = time.delta_secs();
    const BEAM_SPEED: f32 = 20.;
    for (entity, mut trans, move_to) in transforms.iter_mut() {
        let dir = trans.translation - move_to.to;
        let distance = dir.length_squared();
        if distance > 0.05 {
            trans.translation -= dir.normalize() * delta * BEAM_SPEED;
        } else {
            if move_to.will_kill {
                next_state.set(GameState::GameOver);
            }
        }
        if distance < 0.08 {
            trans.scale += delta * BEAM_SPEED;
        }
        if trans.scale.x > 85. {
            commands.entity(entity).despawn();
        }
    }
}
