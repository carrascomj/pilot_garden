//! Mechanics related to Gnomes.

use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;

use crate::{
    GameOverRemove,
    audio::AudioStart,
    config::{GameState, TOOL_ANIM_TIME},
    digging::{Life, Minable},
    killer_arms::find_entity,
    player_movement::{Collider, Player},
    point_raycast::RayBlocker,
    surveillance::TurnTheLights,
};

const GNOME_SIZE: Vec3 = Vec3::new(0.5, 1.5, 0.5);
const GNOME_VIEW_DISTANCE_POW2: f32 = 169.;
const COS_THRESHOLD: f32 = 0.70710677; // cos(PI / 4)
const GNOME_VELOCITY: f32 = 25.;
const ALERTER_POSITIONS: [Vec2; 8] = [
    Vec2::new(31., -12.2),
    Vec2::new(31., -4.),
    Vec2::new(25.685, -4.),
    Vec2::new(25.685, 20.),
    Vec2::new(25.685, 40.),
    Vec2::new(25., 75.),
    Vec2::new(-25., 75.),
    Vec2::new(-47.415, 96.),
];

/// Spawn gnomes were relevant and control the state machine,
/// transitions and animations of the gnomes.
pub struct GnomePlugin;

impl Plugin for GnomePlugin {
    fn build(&self, app: &mut App) {
        // above gnomes are persistent over runs
        app.init_resource::<KilledPlayer>()
            .add_systems(Startup, spawn_persistent_moves)
            // below gnome are statescoped to Below
            .add_systems(OnEnter(GameState::Below), spawn_gnomes_below)
            .add_systems(
                Update,
                setup_platform_moving.run_if(in_state(GameState::Menu)),
            )
            .add_systems(
                Update,
                (setup_animations, move_gnome, remove_minable_on_die)
                    .run_if(not(in_state(GameState::Menu))),
            );
    }
}

/// State for the gnomes.
#[derive(PartialEq)]
pub enum GnomeState {
    Inactive,
    Active,
    Deactivated,
    LoadingBanana,
    DroppingBanana(Timer),
    Moving {
        timer: Timer,
        /// Vec2 since the gnomes do not fly
        spline: CubicCurve<Vec2>,
    },
    Attacking,
    WaitingForAttack {
        timer: Timer,
        already_looking: bool,
    },
    Dying,
    WaitingToDie(Timer),
}

/// State machine for the gnomes
#[derive(Component)]
pub struct GnomeMachine {
    state: GnomeState,
    is_changed: bool,
}

impl GnomeMachine {
    fn new() -> Self {
        Self {
            state: GnomeState::Inactive,
            is_changed: false,
        }
    }
    pub fn next_state(&mut self) {
        self.state = match self.state {
            GnomeState::Dying | GnomeState::Attacking => return,
            GnomeState::Inactive => {
                GnomeState::WaitingToDie(Timer::from_seconds(TOOL_ANIM_TIME, TimerMode::Once))
            }
            GnomeState::LoadingBanana => {
                GnomeState::DroppingBanana(Timer::from_seconds(2., TimerMode::Once))
            }
            GnomeState::DroppingBanana(_) => GnomeState::Moving {
                timer: Timer::from_seconds(14., TimerMode::Once),
                spline: CubicCardinalSpline {
                    tension: 0.1,
                    control_points: ALERTER_POSITIONS.into(),
                }
                .to_curve()
                .expect("Spline failed to resolve."),
            },
            GnomeState::Active => GnomeState::Attacking,
            GnomeState::Deactivated => GnomeState::WaitingForAttack {
                timer: Timer::from_seconds(3., TimerMode::Once),
                already_looking: false,
            },
            GnomeState::WaitingForAttack { .. } => GnomeState::Attacking,
            GnomeState::WaitingToDie { .. } => GnomeState::Dying,
            GnomeState::Moving { .. } => GnomeState::Active,
        };
        self.is_changed = true;
    }

    pub fn waiting_for_attack(&mut self) {
        self.state = match self.state {
            GnomeState::Dying | GnomeState::Attacking | GnomeState::Inactive => return,
            _ => GnomeState::WaitingForAttack {
                timer: Timer::from_seconds(3., TimerMode::Once),
                already_looking: false,
            },
        };
        self.is_changed = true;
    }

    pub fn deactivate(&mut self) {
        self.state = match self.state {
            GnomeState::Active => GnomeState::Deactivated,
            _ => return,
        };
        self.is_changed = true;
    }
}

#[derive(Resource)]
struct Animations {
    animations: Vec<AnimationNodeIndex>,
    graph_handle: Handle<AnimationGraph>,
}
#[derive(Resource)]
struct GnomeHandle {
    handle: Handle<Scene>,
}

/// Marker for entities that have an ([`AnimationPlayer`], [`AnimationTransitions`])
/// enitity as a child.
#[derive(Component)]
struct HasAnimationChild(Option<Entity>);
#[derive(Component)]
pub struct PlatformMover;

/// The gnomes above are tame and can only die.
fn spawn_persistent_moves(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    const GNOME_PATH: &str = "higher_poly_gnome.glb";
    let gnome = asset_server.load(GltfAssetLabel::Scene(0).from_asset(GNOME_PATH));
    commands.insert_resource(GnomeHandle {
        handle: gnome.clone(),
    });

    // save animations in resource to add them later after the gnome scene is loaded
    let (graph, node_indices) = AnimationGraph::from_clips([
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(GNOME_PATH)), // die
        asset_server.load(GltfAssetLabel::Animation(1).from_asset(GNOME_PATH)), // idle
        asset_server.load(GltfAssetLabel::Animation(2).from_asset(GNOME_PATH)), // run
        asset_server.load(GltfAssetLabel::Animation(3).from_asset(GNOME_PATH)), // wow
    ]);
    let graph_handle = graphs.add(graph);
    commands.insert_resource(Animations {
        animations: node_indices,
        graph_handle,
    });
    let mut first = true;
    for (x, y, z) in [(27., 0.1, 8.6), (24., -2.2, 25.6)] {
        let mut ent_comm = commands.spawn((
            SceneRoot(gnome.clone()),
            HasAnimationChild(None),
            GnomeMachine::new(),
            Collider::from_translation(Vec3::new(x, y, z) + Vec3::Y * 0.5, GNOME_SIZE),
            Transform::from_xyz(x, y, z),
            Minable,
            Life::JustSpawned,
        ));
        if first {
            let cub = Cuboid::new(0.8, GNOME_SIZE.y * 0.8, 0.8);
            ent_comm
                // the entity will move some invisible meshes on dying, allowing
                // for the shove
                .insert(PlatformMover)
                // invisible mesh to make pointing with the pick more lenient (in the feet)
                .with_child((Transform::from_xyz(0., 0.35, 0.1), Mesh3d(meshes.add(cub))));
            first = false;
        }
    }
}

fn spawn_gnomes_below(
    mut commands: Commands,
    gnome_asset: Res<GnomeHandle>,
    mut killed_player: ResMut<KilledPlayer>,
) {
    killed_player.0 = false;
    // gnome just below the crops that will run to the studio on sight
    let (x, y, z) = (31., -26.3, -12.2);
    commands.spawn((
        SceneRoot(gnome_asset.handle.clone()),
        GameOverRemove,
        HasAnimationChild(None),
        GnomeMachine {
            state: GnomeState::LoadingBanana,
            is_changed: false,
        },
        Collider::from_translation(Vec3::new(x, y, z) + Vec3::Y * 0.5, GNOME_SIZE),
        Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(2.3)),
        Minable,
        Life::JustSpawned,
    ));
    // rest of gnomes, in the studio
    for (x, z, rot_y) in [
        (-49.425, 97.876, 2.6),
        (-62.296, 92.201, PI),
        (-54.163, 103., 0.),
        (-69.222, 97.496, 0.),
        (-45.882, 67.5, 0.),
        (-39.256, 66.342, 1.6),
        (-33.2, 66.342, 1.6),
        (-42.138, 98.156, 1.6),
        (-35.981, 69.914, 2.3),
        (-42.138, 98.156, 1.6),
        (-42.138, 91.192, 1.6),
        (-31.138, 90.192, 1.9),
    ] {
        commands.spawn((
            SceneRoot(gnome_asset.handle.clone()),
            GameOverRemove,
            HasAnimationChild(None),
            GnomeMachine {
                state: GnomeState::Active,
                is_changed: false,
            },
            Collider::from_translation(Vec3::new(x, -26.3, z) + Vec3::Y * 0.5, GNOME_SIZE),
            Transform::from_xyz(x, -26.3, z).with_rotation(Quat::from_rotation_y(rot_y)),
        ));
    }
}

fn setup_animations(
    mut commands: Commands,
    animations: Res<Animations>,
    mut anim_parents: Query<(Entity, &mut HasAnimationChild)>,
    children: Query<&Children>,
    mut anim_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (parent, mut has_animation_child) in anim_parents
        .iter_mut()
        .filter(|(_, has_anim)| has_anim.0.is_none())
    {
        for child in children.iter_descendants(parent) {
            if let Ok((entity, mut player)) = anim_players.get_mut(child) {
                let mut transitions = AnimationTransitions::new();

                // Make sure to start the animation via the `AnimationTransitions`
                // component. The `AnimationTransitions` component wants to manage all
                // the animations and will get confused if the animations are started
                // directly via the `AnimationPlayer`.
                transitions.play(&mut player, animations.animations[1], Duration::ZERO); // start at idle

                commands
                    .entity(entity)
                    .insert(transitions)
                    .insert(AnimationGraphHandle(animations.graph_handle.clone()));
                has_animation_child.0 = Some(entity);
            }
        }
    }
}
/// So that it does not show the crosshair green after it dies.
fn remove_minable_on_die(
    mut commands: Commands,
    gnomes: Query<(Entity, &GnomeMachine), With<Minable>>,
) {
    for (entity, gnome) in &gnomes {
        if gnome.state == GnomeState::Dying {
            commands.entity(entity).remove::<Minable>();
        }
    }
}

#[derive(Resource, Default)]
struct KilledPlayer(bool);

/// Control movement of the gnome based on its state.
fn move_gnome(
    time: Res<Time>,
    animations: Res<Animations>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut killed_player: ResMut<KilledPlayer>,
    mut light_switch_event: EventWriter<TurnTheLights>,
    mut audio_event: EventWriter<AudioStart>,
    mut gnomes: Query<(&mut GnomeMachine, &HasAnimationChild, &mut Transform)>,
    mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    player_transform: Query<&Transform, (With<Player>, Without<GnomeMachine>)>,
) {
    for (mut gnome, has_anim, mut transform) in &mut gnomes {
        // change the animation itself
        if gnome.is_changed {
            if let Some(anim_entity) = has_anim.0 {
                let (anim_index, speed) = match &mut gnome.state {
                    GnomeState::Dying => (0, 1.),
                    GnomeState::Attacking => (2, 1.),
                    GnomeState::DroppingBanana(_) => (3, 1.),
                    GnomeState::Moving { timer, spline } => {
                        timer.unpause();
                        // set speed proportional to the velocity
                        let speed =
                            spline.velocity(timer.fraction() * spline.segments().len() as f32);
                        (2, (speed.length_squared() / 1000.).clamp(1., 10.))
                    }
                    GnomeState::WaitingToDie(timer) => {
                        timer.unpause();
                        (1, 1.)
                    }
                    _ => (1, 1.),
                };
                if let Ok((mut player, mut transitions)) = animation_players.get_mut(anim_entity) {
                    let active_animation = transitions.play(
                        &mut player,
                        animations.animations[anim_index],
                        Duration::from_millis(100),
                    );
                    // if running, repeat forever, it is more menacing in Game Over
                    // that the gnomes just keep moving towards the player and the
                    // player won't the see the gnome at the end of `Moving`.
                    if anim_index == 2 {
                        active_animation.repeat();
                        active_animation.set_speed(speed);
                    }
                    active_animation.replay();
                }
            }
            gnome.is_changed = false;
        }
        // handle movement (only across x, z axis)
        match &mut gnome.state {
            GnomeState::Moving { timer, spline } => {
                if !timer.finished() {
                    timer.tick(time.delta());
                    let u = timer.fraction(); // [0,1]
                    // let t = u * spline.segments().len() as f32;
                    let t = u * spline.segments().len() as f32;
                    let Vec2 { x, y: z } = spline.position(t);
                    let Vec2 { x: dx, y: dz } = spline.velocity(t);
                    let direction = Vec3::new(dx, 0.0, dz);
                    if direction.length_squared() > 0.01 {
                        let direction = direction.normalize();
                        let next_position = Vec3::new(x, transform.translation.y, z);
                        *transform = transform
                            .with_translation(next_position)
                            .looking_at(next_position + direction, Vec3::Y);
                    }
                } else {
                    gnome.next_state();
                }
            }
            GnomeState::Attacking => {
                if let Ok(target) = player_transform.single() {
                    let delta = time.delta().as_secs_f32();
                    let dir = (target.translation - transform.translation).with_y(0.);
                    if dir.length_squared() > 5. {
                        let dir = dir.normalize();
                        transform.translation =
                            transform.translation + (dir * delta * GNOME_VELOCITY);
                        transform.look_to(dir, Vec3::Y);
                    } else if !killed_player.0 {
                        killed_player.0 = true;
                        next_game_state.set(GameState::GameOver);
                        break;
                    };
                }
            }
            GnomeState::Active | GnomeState::LoadingBanana => {
                // check visibility of player given some distance threshold
                // and minimum angle with respect the forward direction of gnome
                let Ok(target) = player_transform.single() else {
                    continue;
                };
                let mut to_player = target.translation - transform.translation;

                if to_player.length_squared() > GNOME_VIEW_DISTANCE_POW2 {
                    continue; // player too far
                }

                let mut forward: Vec3 = transform.forward().into();
                forward.y = 0.0;

                if forward.length_squared() < 1e-6 {
                    continue; // no horizontal forward
                }

                to_player.y = 0.0;
                // a . b = |a| |b| cos(ɑ)
                let dir_norm = to_player.normalize();
                let fwd_norm = forward.normalize();

                // a . b = cos(ɑ)
                let dot = fwd_norm.dot(dir_norm);
                if dot >= COS_THRESHOLD {
                    audio_event.write(AudioStart::UhOh);
                    gnome.next_state();
                }
            }
            GnomeState::DroppingBanana(timer) => {
                if !timer.finished() {
                    timer.tick(time.delta());
                    let Ok(target) = player_transform.single() else {
                        continue;
                    };
                    let dir = (target.translation - transform.translation).with_y(0.);
                    transform.look_to(dir, Vec3::Y);
                } else {
                    light_switch_event.write(TurnTheLights::Off);
                    gnome.next_state()
                }
            }
            GnomeState::WaitingForAttack {
                timer,
                already_looking,
            } => {
                // this is a special state for when the player has just
                // pressed the button: wait until the player sees the gnomes, or
                // otherwise (if the player tries to cheat by not looking) just
                // attack when the player pass certain threshold
                let Ok(target) = player_transform.single() else {
                    continue;
                };
                let dir = (target.translation - transform.translation).with_y(0.);
                let dir = dir.normalize();
                transform.look_to(dir, Vec3::Y);

                if !*already_looking {
                    // harcoded, the player facing the button perfectly is [1, 0, 0,]
                    // turned around is [-1., 0., 0.], so 0.5 ~ 90 degrees to the gnomes
                    let player_looking = target.forward().x > 0.5;
                    let player_trying_to_flee = target.translation.x > -45.;
                    *already_looking = player_looking || player_trying_to_flee;
                    continue;
                }
                if !timer.finished() {
                    // wait for  dramatic effect
                    timer.tick(time.delta());
                } else {
                    audio_event.write(AudioStart::Goat);
                    // and finally attack the player
                    gnome.next_state()
                }
            }
            GnomeState::WaitingToDie(timer) => {
                timer.tick(time.delta());
                if timer.finished() {
                    audio_event.write(AudioStart::GnomeDying);
                    gnome.next_state();
                }
            }
            _ => (),
        }
    }
}

/// Setup up a rayblocking mesh that moves with the gnome
/// die animation (targetting the bone `platform_mover`).
///
/// As usual for GLTF, walk recursively from the [`SceneRoot`]
/// until we find the bone.
fn setup_platform_moving(
    mut commands: Commands,
    gnome_mover: Query<&Children, With<PlatformMover>>,
    parents: Query<&Children>,
    names: Query<(Entity, &Name)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    for children in &gnome_mover {
        for child in children {
            if let Ok(down_children) = parents.get(*child) {
                for down_child in down_children {
                    if let Ok(ik_bone) = find_entity(*down_child, "platform_mover", parents, names)
                    {
                        if let Ok((entity, _)) = names.get(ik_bone) {
                            *done = true;
                            commands.entity(entity).with_child((
                                Mesh3d(meshes.add(Cuboid::new(2.2, 6., 2.2))),
                                Transform::from_translation(Vec3::new(-0.3, -3.4, -1.2)),
                                // Transform::from_translation(Vec3::new(0., 3., 7.1))
                                //     .with_scale(Vec3::new(4.623, 2.2, 4.623)),
                                RayBlocker,
                            ));
                        }
                    }
                }
            }
        }
    }
}
